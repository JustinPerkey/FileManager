//! Every read: lists, objects, relations, checklists. Runs inside one read
//! transaction opened by `Session`; SQL text comes only from `sql.rs`.

use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};

use super::cell::{Cell, Row};
use super::key::{conditions, identity_idents, key_of};
use super::label::{read_value, render};
use super::sql::{Filter, Link, Query, Select};
use super::types::{
    FieldValue, LinkTarget, ListQuery, M2mOption, M2mPage, M2mQuery, ObjectKey, ObjectPage,
    ObjectSummary, ObjectView, RelationCount, ValueProblem,
};
use super::version::version;
use super::SessionError;
use crate::config::FieldMode;
use crate::model::{M2mSide, Model, ResolvedField, ResolvedTable, ReverseRelation};
use crate::schema::{Ident, Identity, Table};

const MAX_LIMIT: u32 = 500;

pub(super) struct Reader<'a> {
    pub conn: &'a Connection,
    pub model: &'a Model,
}

/// What a summary (key and label) of a row needs from its table.
struct Summary {
    columns: Vec<usize>,
    with_rowid: bool,
}

type Eqs<'a> = Vec<(&'a Ident, Value)>;

fn eq_filters<'a>(eqs: &Eqs<'a>) -> Vec<Filter<'a>> {
    eqs.iter().map(|(c, v)| Filter::Eq(c, v.clone())).collect()
}

impl<'a> Reader<'a> {
    fn table(&self, name: &str) -> Result<&'a ResolvedTable, SessionError> {
        self.model
            .table(name)
            .ok_or_else(|| SessionError::UnknownTable(name.to_owned()))
    }

    fn schema_table(&self, rt: &ResolvedTable) -> &'a Table {
        &self.model.schema().tables[rt.schema_index]
    }

    fn fetch(&self, q: &Query) -> Result<Vec<Vec<Cell>>, SessionError> {
        let mut stmt = self.conn.prepare(&q.sql)?;
        let n = stmt.column_count();
        let rows = stmt.query_map(params_from_iter(q.params.iter()), |r| {
            (0..n)
                .map(|i| r.get_ref(i).map(Cell::from))
                .collect::<Result<Vec<_>, _>>()
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn count(&self, q: &Query) -> Result<u64, SessionError> {
        let n: i64 = self
            .conn
            .query_row(&q.sql, params_from_iter(q.params.iter()), |r| r.get(0))?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    /// The columns a key and a label need, plus `extra`.
    fn summary(rt: &ResolvedTable, extra: Option<usize>) -> Summary {
        let mut columns = rt.label.columns();
        if let Identity::PrimaryKey { columns: pk } = &rt.identity {
            columns.extend(pk);
        }
        columns.extend(extra);
        let mut seen = Vec::new();
        columns.retain(|c| {
            let new = !seen.contains(c);
            seen.push(*c);
            new
        });
        Summary {
            columns,
            with_rowid: matches!(rt.identity, Identity::Rowid { .. }),
        }
    }

    fn summary_select(st: &'a Table, s: &Summary) -> Select<'a> {
        let mut sel = Select::new(&st.name);
        sel.columns = s.columns.iter().map(|&c| &st.columns[c].name).collect();
        if let (true, Identity::Rowid { accessor, .. }) = (s.with_rowid, &st.identity) {
            sel.columns.push(accessor);
        }
        sel
    }

    fn order(rt: &ResolvedTable, st: &'a Table) -> Vec<(&'a Ident, bool)> {
        let mut order: Vec<(&Ident, bool)> = rt
            .order
            .iter()
            .map(|o| (&st.columns[o.column].name, o.descending))
            .collect();
        order.extend(identity_idents(st).into_iter().map(|i| (i, false)));
        order
    }

    fn key_conditions(st: &'a Table, key: &ObjectKey) -> Result<Eqs<'a>, SessionError> {
        conditions(st, key).ok_or(SessionError::UnknownObject)
    }

    fn summaries(
        rt: &ResolvedTable,
        st: &Table,
        s: &Summary,
        rows: &[Vec<Cell>],
    ) -> Vec<(ObjectKey, String, Row)> {
        rows.iter()
            .map(|cells| {
                let row = Row::new(st.columns.len(), &s.columns, s.with_rowid, cells);
                let key = key_of(st, &row);
                let label = render(rt, &key, &row);
                (key, label, row)
            })
            .collect()
    }

    fn search_filter(
        rt: &ResolvedTable,
        st: &'a Table,
        search: Option<&str>,
    ) -> Option<(Vec<&'a Ident>, String)> {
        let text = search?.trim();
        if text.is_empty() || rt.search.is_empty() {
            return None;
        }
        Some((
            rt.search.iter().map(|&c| &st.columns[c].name).collect(),
            text.to_owned(),
        ))
    }

    // ---- lists ----

    /// The equality conditions of a "referenced by" list.
    fn referencing(
        &self,
        rt: &ResolvedTable,
        st: &'a Table,
        relation: &str,
        key: &ObjectKey,
    ) -> Result<Eqs<'a>, SessionError> {
        let (owner, rel) = self
            .model
            .tables()
            .find_map(|o| {
                o.referenced_by
                    .iter()
                    .find(|r| r.relation == relation && r.table_name == rt.name)
                    .map(|r| (o, r))
            })
            .ok_or_else(|| SessionError::UnknownRelation(relation.to_owned()))?;
        let owner_st = self.schema_table(owner);
        let fk = &st.foreign_keys[rel.fk_index];
        let target = fk
            .target
            .as_ref()
            .ok_or_else(|| SessionError::UnknownRelation(relation.to_owned()))?;
        let mut sel = Select::new(&owner_st.name);
        sel.columns = target
            .columns
            .iter()
            .map(|&c| &owner_st.columns[c].name)
            .collect();
        sel.filters = eq_filters(&Self::key_conditions(owner_st, key)?);
        let rows = self.fetch(&sel.rows())?;
        let cells = rows.first().ok_or(SessionError::UnknownObject)?;
        Ok(fk
            .from
            .iter()
            .zip(cells)
            .map(|(&c, cell)| (&st.columns[c].name, cell.param()))
            .collect())
    }

    pub(super) fn list(&self, table: &str, query: &ListQuery) -> Result<ObjectPage, SessionError> {
        let rt = self.table(table)?;
        let st = self.schema_table(rt);
        let mut eqs: Eqs<'a> = Vec::new();
        if let Some(r) = &query.referencing {
            eqs = self.referencing(rt, st, &r.relation, &r.key)?;
        }
        let search = Self::search_filter(rt, st, query.search.as_deref());
        let filters = || {
            let mut f = eq_filters(&eqs);
            f.extend(
                search
                    .iter()
                    .map(|(cols, text)| Filter::Search(cols.clone(), text.clone())),
            );
            f
        };
        let mut count = Select::new(&st.name);
        count.filters = filters();
        let total = self.count(&count.count())?;

        let summary = Self::summary(rt, None);
        let mut sel = Self::summary_select(st, &summary);
        sel.filters = filters();
        sel.order = Self::order(rt, st);
        let limit = query.limit.clamp(1, MAX_LIMIT);
        sel.page = Some((limit, query.offset));
        let rows = self.fetch(&sel.rows())?;
        let items = Self::summaries(rt, st, &summary, &rows)
            .into_iter()
            .map(|(key, label, _)| ObjectSummary { key, label })
            .collect();
        Ok(ObjectPage {
            items,
            total,
            offset: query.offset,
        })
    }

    // ---- objects ----

    /// Every column of the row, as a [`Row`] (hidden ones included, for the
    /// version).
    fn full_row(&self, st: &'a Table, eqs: &Eqs<'a>) -> Result<Option<Row>, SessionError> {
        let all: Vec<usize> = (0..st.columns.len()).collect();
        let mut sel = Select::new(&st.name);
        sel.columns = st.columns.iter().map(|c| &c.name).collect();
        let with_rowid = if let Identity::Rowid { accessor, .. } = &st.identity {
            sel.columns.push(accessor);
            true
        } else {
            false
        };
        sel.filters = eq_filters(eqs);
        let rows = self.fetch(&sel.rows())?;
        Ok(rows
            .first()
            .map(|cells| Row::new(st.columns.len(), &all, with_rowid, cells)))
    }

    /// The target of a foreign-key field's value, if the field is a link.
    fn link(
        &self,
        st: &Table,
        field: &ResolvedField,
        row: &Row,
    ) -> Result<Option<LinkTarget>, SessionError> {
        let Some(fk) = &field.foreign_key else {
            return Ok(None);
        };
        let from = &st.foreign_keys[fk.fk_index].from;
        if from.iter().any(|&c| row.cell(c).is_null()) {
            return Ok(None);
        }
        let Some(target) = self.model.table_at(fk.table) else {
            return Ok(None);
        };
        let target_st = self.schema_table(target);
        let summary = Self::summary(target, None);
        let mut sel = Self::summary_select(target_st, &summary);
        sel.filters = fk
            .columns
            .iter()
            .zip(from)
            .map(|(&tc, &fc)| Filter::Eq(&target_st.columns[tc].name, row.cell(fc).param()))
            .collect();
        let rows = self.fetch(&sel.rows())?;
        let found = Self::summaries(target, target_st, &summary, &rows)
            .into_iter()
            .next();
        Ok(Some(LinkTarget {
            table: fk.table_name.clone(),
            key: found.as_ref().map(|(k, _, _)| k.clone()),
            label: found.map(|(_, l, _)| l),
            navigable: fk.navigable,
        }))
    }

    fn field_value(
        &self,
        st: &Table,
        field: &ResolvedField,
        row: &Row,
    ) -> Result<FieldValue, SessionError> {
        let (value, checked, mut problem) = read_value(field, row.cell(field.column));
        let link = self.link(st, field, row)?;
        if problem.is_none() && link.as_ref().is_some_and(|l| l.key.is_none()) {
            problem = Some(ValueProblem::DanglingReference);
        }
        Ok(FieldValue {
            column: field.name.clone(),
            value,
            checked,
            link,
            problem,
        })
    }

    fn reverse_count(&self, row: &Row, rel: &ReverseRelation) -> Result<u64, SessionError> {
        let referencing = &self.model.schema().tables[rel.table];
        let fk = &referencing.foreign_keys[rel.fk_index];
        let Some(target) = &fk.target else {
            return Ok(0);
        };
        debug_assert_eq!(fk.from.len(), target.columns.len());
        let mut sel = Select::new(&referencing.name);
        sel.filters = fk
            .from
            .iter()
            .zip(&target.columns)
            .map(|(&f, &t)| Filter::Eq(&referencing.columns[f].name, row.cell(t).param()))
            .collect();
        self.count(&sel.count())
    }

    /// The link that joins the listed (other) table to the row's value.
    fn link_of(&self, side: &M2mSide, this_value: Value) -> (&'a Table, Link<'a>) {
        let schema = self.model.schema();
        let junction = &schema.tables[side.junction];
        let other = &schema.tables[side.other_table];
        (
            other,
            Link {
                junction: &junction.name,
                junction_this: &junction.columns[side.this_column].name,
                junction_other: &junction.columns[side.other_column].name,
                other_key: &other.columns[side.other_key].name,
                this_value,
            },
        )
    }

    fn linked_count(&self, side: &M2mSide, row: &Row) -> Result<u64, SessionError> {
        let (other, link) = self.link_of(side, row.cell(side.this_key).param());
        let mut sel = Select::new(&other.name);
        sel.filters = vec![Filter::Linked(link)];
        self.count(&sel.count())
    }

    pub(super) fn object(&self, table: &str, key: &ObjectKey) -> Result<ObjectView, SessionError> {
        let rt = self.table(table)?;
        let st = self.schema_table(rt);
        let eqs = Self::key_conditions(st, key)?;
        let row = self
            .full_row(st, &eqs)?
            .ok_or(SessionError::UnknownObject)?;
        let key = key_of(st, &row);
        let fields = rt
            .fields
            .iter()
            .map(|f| self.field_value(st, f, &row))
            .collect::<Result<_, _>>()?;
        let referenced_by = rt
            .referenced_by
            .iter()
            .map(|r| {
                Ok(RelationCount {
                    relation: r.relation.clone(),
                    count: self.reverse_count(&row, r)?,
                })
            })
            .collect::<Result<_, SessionError>>()?;
        let many_to_many = rt
            .many_to_many
            .iter()
            .map(|s| {
                Ok(RelationCount {
                    relation: s.relation.clone(),
                    count: self.linked_count(s, &row)?,
                })
            })
            .collect::<Result<_, SessionError>>()?;
        Ok(ObjectView {
            table: rt.name.clone(),
            label: render(rt, &key, &row),
            version: version((0..st.columns.len()).map(|c| row.cell(c))),
            key,
            fields,
            referenced_by,
            many_to_many,
        })
    }

    // ---- checklists ----

    /// The visible link fields and version of the junction row that joins
    /// `row` (of the other table) to `this`.
    fn link_details(
        &self,
        side: &M2mSide,
        this: &Value,
        row: &Row,
    ) -> Result<(Option<Vec<FieldValue>>, Option<String>), SessionError> {
        let junction = &self.model.schema().tables[side.junction];
        let eqs: Eqs<'_> = vec![
            (&junction.columns[side.this_column].name, this.clone()),
            (
                &junction.columns[side.other_column].name,
                row.cell(side.other_key).param(),
            ),
        ];
        let Some(jrow) = self.full_row(junction, &eqs)? else {
            return Ok((None, None));
        };
        let values = side
            .link_fields
            .iter()
            .filter(|f| f.mode != FieldMode::Hidden)
            .map(|f| self.field_value(junction, f, &jrow))
            .collect::<Result<Vec<_>, _>>()?;
        let version = version((0..junction.columns.len()).map(|c| jrow.cell(c)));
        Ok((Some(values), Some(version)))
    }

    pub(super) fn m2m_options(
        &self,
        table: &str,
        key: Option<&ObjectKey>,
        relation: &str,
        query: &M2mQuery,
    ) -> Result<M2mPage, SessionError> {
        let rt = self.table(table)?;
        let st = self.schema_table(rt);
        let unknown = || SessionError::UnknownRelation(relation.to_owned());
        let side = rt
            .many_to_many
            .iter()
            .find(|s| s.relation == relation)
            .ok_or_else(unknown)?;
        let other_rt = self.model.table_at(side.other_table).ok_or_else(unknown)?;
        let other_schema = self.schema_table(other_rt);
        // The value of this object's side of the junction.
        let this_value = match key {
            None => None,
            Some(k) => {
                let mut sel = Select::new(&st.name);
                sel.columns = vec![&st.columns[side.this_key].name];
                sel.filters = eq_filters(&Self::key_conditions(st, k)?);
                let rows = self.fetch(&sel.rows())?;
                let cells = rows.first().ok_or(SessionError::UnknownObject)?;
                Some(cells[0].param())
            }
        };
        let linked_total = match &this_value {
            None => 0,
            Some(v) => {
                let (_, link) = self.link_of(side, v.clone());
                let mut sel = Select::new(&other_schema.name);
                sel.filters = vec![Filter::Linked(link)];
                self.count(&sel.count())?
            }
        };

        let summary = Self::summary(other_rt, Some(side.other_key));
        let only = match &query.only {
            Some(k) => Some(Self::key_conditions(other_schema, k)?),
            None => None,
        };
        let search = Self::search_filter(other_rt, other_schema, query.search.as_deref());
        let linked_only = query.linked_only && only.is_none();
        let filters = || -> Vec<Filter<'a>> {
            if let Some(eqs) = &only {
                return eq_filters(eqs);
            }
            let mut f: Vec<Filter<'a>> = search
                .iter()
                .map(|(cols, text)| Filter::Search(cols.clone(), text.clone()))
                .collect();
            if let (true, Some(v)) = (linked_only, &this_value) {
                f.push(Filter::Linked(self.link_of(side, v.clone()).1));
            }
            f
        };
        if linked_only && this_value.is_none() {
            return Ok(M2mPage {
                items: Vec::new(),
                total: 0,
                linked_total,
                offset: query.offset,
            });
        }
        let mut count = Select::new(&other_schema.name);
        count.filters = filters();
        let total = self.count(&count.count())?;

        let mut sel = Self::summary_select(other_schema, &summary);
        sel.filters = filters();
        sel.linked_flag = this_value.as_ref().map(|v| self.link_of(side, v.clone()).1);
        if only.is_none() {
            sel.order = Self::order(other_rt, other_schema);
            sel.page = Some((query.limit.clamp(1, MAX_LIMIT), query.offset));
        }
        let rows = self.fetch(&sel.rows())?;
        let flags: Vec<bool> = rows
            .iter()
            .map(|r| matches!(r.last(), Some(Cell::Integer(n)) if this_value.is_some() && *n != 0))
            .collect();
        let summaries = Self::summaries(other_rt, other_schema, &summary, &rows);
        let mut items = Vec::with_capacity(summaries.len());
        for ((okey, label, row), linked) in summaries.into_iter().zip(flags) {
            let (link_values, link_version) = match (linked, &this_value) {
                (true, Some(this)) => self.link_details(side, this, &row)?,
                _ => (None, None),
            };
            items.push(M2mOption {
                key: okey,
                label,
                linked,
                link_values,
                link_version,
            });
        }
        Ok(M2mPage {
            items,
            total,
            linked_total,
            offset: query.offset,
        })
    }
}
