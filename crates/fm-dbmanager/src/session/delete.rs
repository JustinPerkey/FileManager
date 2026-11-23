//! Deleting one object: the impact preview (what every referencing table
//! would lose, hidden ones counted) and the confirmed delete.

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params_from_iter, ErrorCode};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::cell::Row;
use super::key::{identity_idents, key_of};
use super::label::render;
use super::read::{Eqs, Reader};
use super::save::begin_guarded;
use super::sql;
use super::types::ObjectKey;
use super::version::version;
use super::{Session, SessionError};
use crate::config::TableMode;
use crate::model::M2mSide;
use crate::schema::{FkAction, ForeignKey, Table};

/// Affected rows enumerated before the impact is cut short.
const ROW_LIMIT: u32 = 10_000;
/// Labels shown per group.
const SAMPLE: usize = 10;

/// What the database does to the rows that reference a deleted one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum OnDeleteAction {
    Cascade,
    SetNull,
    SetDefault,
    Restrict,
    NoAction,
}

impl OnDeleteAction {
    fn tag(self) -> u8 {
        match self {
            OnDeleteAction::Cascade => 0,
            OnDeleteAction::SetNull => 1,
            OnDeleteAction::SetDefault => 2,
            OnDeleteAction::Restrict => 3,
            OnDeleteAction::NoAction => 4,
        }
    }
}

impl From<FkAction> for OnDeleteAction {
    fn from(a: FkAction) -> Self {
        match a {
            FkAction::Cascade => OnDeleteAction::Cascade,
            FkAction::SetNull => OnDeleteAction::SetNull,
            FkAction::SetDefault => OnDeleteAction::SetDefault,
            FkAction::Restrict => OnDeleteAction::Restrict,
            FkAction::NoAction => OnDeleteAction::NoAction,
        }
    }
}

/// The rows of one table affected through one foreign key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a boundary type the UI reads field by field; the flags are independent facts"
)]
pub struct ImpactGroup {
    /// SQL name.
    pub table: String,
    /// The display name; the SQL name for a hidden table without one. For a
    /// visible many-to-many link group, the relation's heading on the deleted
    /// object's side.
    pub table_display_name: String,
    pub hidden: bool,
    /// The foreign-key column display name(s), joined by `, ` (SQL names for
    /// hidden tables and columns).
    pub via: String,
    pub action: OnDeleteAction,
    /// Rows affected through this foreign key.
    #[ts(type = "number")]
    pub count: u64,
    /// 1 references the object directly; 2 and up, through cascades.
    pub depth: u32,
    /// `count` is a lower bound.
    pub truncated: bool,
    /// Up to 10 labels; empty when hidden.
    pub sample: Vec<String>,
    /// The rows are those of a junction used as many-to-many.
    pub links: bool,
    /// Links whose junction has extra columns: their values are lost.
    pub link_data: bool,
    /// This group makes the delete fail.
    pub blocks: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct DeleteImpact {
    pub table: String,
    pub key: ObjectKey,
    pub label: String,
    pub version: String,
    /// Breadth-first: depth, then table, then via.
    pub groups: Vec<ImpactGroup>,
    /// Any group blocks.
    pub blocked: bool,
    pub truncated: bool,
    /// The object's table or any affected table has triggers.
    pub has_triggers: bool,
    /// A hash of everything above except labels.
    pub token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub table: String,
    pub label: String,
    pub impact: DeleteImpact,
    pub backup_created: Option<String>,
}

/// How a group shows its samples.
enum Shape<'m> {
    /// Labels of the affected rows, in this table.
    Rows(&'m crate::model::ResolvedTable),
    /// Labels of the objects on the other side of each removed link.
    Link(&'m M2mSide),
    /// No labels.
    Hidden,
}

fn token(version: &str, groups: &[ImpactGroup]) -> String {
    fn text(h: &mut Sha256, s: &str) {
        h.update((s.len() as u64).to_be_bytes());
        h.update(s.as_bytes());
    }
    let mut h = Sha256::new();
    text(&mut h, version);
    for g in groups {
        text(&mut h, &g.table);
        text(&mut h, &g.via);
        h.update([g.action.tag()]);
        h.update(g.count.to_be_bytes());
        h.update(g.depth.to_be_bytes());
        h.update([
            u8::from(g.truncated),
            u8::from(g.blocks),
            u8::from(g.links),
            u8::from(g.link_data),
        ]);
    }
    fm_core::to_hex(&h.finalize())
}

impl<'a> Reader<'a> {
    /// Rows of `st` matching `eqs`, whole, in identity order, at most
    /// `limit`.
    fn rows_where(
        &self,
        st: &'a Table,
        eqs: Eqs<'a>,
        limit: u32,
    ) -> Result<Vec<Row>, SessionError> {
        let all: Vec<usize> = (0..st.columns.len()).collect();
        let mut sel = sql::Select::new(&st.name);
        sel.columns = st.columns.iter().map(|c| &c.name).collect();
        let with_rowid = if let crate::schema::Identity::Rowid { accessor, .. } = &st.identity {
            sel.columns.push(accessor);
            true
        } else {
            false
        };
        sel.filters = eqs
            .into_iter()
            .map(|(c, v)| sql::Filter::Eq(c, v))
            .collect();
        sel.order = identity_idents(st)
            .into_iter()
            .map(|i| (i, false))
            .collect();
        sel.page = Some((limit, 0));
        let rows = self.fetch(&sel.rows())?;
        Ok(rows
            .iter()
            .map(|cells| Row::new(st.columns.len(), &all, with_rowid, cells))
            .collect())
    }

    /// The label of the object on the other side of a removed link.
    fn other_label(
        &self,
        side: &M2mSide,
        junction_row: &Row,
    ) -> Result<Option<String>, SessionError> {
        let schema = self.model.schema();
        let other_schema = &schema.tables[side.other_table];
        let Some(other_res) = self.model.table_at(side.other_table) else {
            return Ok(None);
        };
        let cell = junction_row.cell(side.other_column);
        if cell.is_null() {
            return Ok(None);
        }
        let eqs: Eqs<'a> = vec![(&other_schema.columns[side.other_key].name, cell.param())];
        let found = self.rows_where(other_schema, eqs, 1)?;
        Ok(found
            .first()
            .map(|row| render(other_res, &key_of(other_schema, row), row)))
    }

    fn shape(&self, deleted: usize, ci: usize, fk: &ForeignKey) -> Shape<'a> {
        let facts = self.model.facts(ci);
        if facts.hidden {
            return Shape::Hidden;
        }
        let relation_column = facts
            .junction
            .as_ref()
            .filter(|j| fk.from.len() == 1 && j.columns.contains(&fk.from[0]));
        if let Some(j) = relation_column {
            let side = self.model.table_at(deleted).and_then(|t| {
                t.many_to_many
                    .iter()
                    .find(|s| s.junction == ci && s.this_column == fk.from[0])
            });
            return match side {
                Some(s) if !j.relation_hidden && self.model.table_at(s.other_table).is_some() => {
                    Shape::Link(s)
                }
                _ => Shape::Hidden,
            };
        }
        match self.model.table_at(ci) {
            Some(rt) => Shape::Rows(rt),
            None => Shape::Hidden,
        }
    }

    /// The impact of deleting the object `key` of `table`.
    #[expect(
        clippy::too_many_lines,
        reason = "one breadth-first pass; splitting it would pass a dozen locals around"
    )]
    pub(super) fn impact(
        &self,
        table: &str,
        key: &ObjectKey,
    ) -> Result<DeleteImpact, SessionError> {
        let schema = self.model.schema();
        let rt = self.table(table)?;
        let st = self.schema_table(rt);
        let eqs = Self::key_conditions(st, key)?;
        let root = self
            .full_row(st, &eqs)?
            .ok_or(SessionError::UnknownObject)?;
        let root_key = key_of(st, &root);
        let root_version = version((0..st.columns.len()).map(|c| root.cell(c)));
        let label = render(rt, &root_key, &root);

        let mut scheduled: HashMap<usize, HashSet<ObjectKey>> = HashMap::new();
        scheduled
            .entry(rt.schema_index)
            .or_default()
            .insert(root_key.clone());
        let mut frontier: BTreeMap<usize, Vec<Row>> = BTreeMap::new();
        frontier.insert(rt.schema_index, vec![root]);
        let mut remaining = ROW_LIMIT;
        let mut truncated = false;
        let mut groups: Vec<ImpactGroup> = Vec::new();
        let mut has_triggers = st.has_triggers;
        let mut depth = 1_u32;

        while !frontier.is_empty() && !truncated {
            let mut next: BTreeMap<usize, Vec<Row>> = BTreeMap::new();
            for (&ti, rows) in &frontier {
                for (ci, fk) in schema.referencing(ti) {
                    let Some(target) = &fk.target else { continue };
                    let ct = &schema.tables[ci];
                    let facts = self.model.facts(ci);
                    let shape = self.shape(ti, ci, fk);
                    let junction = facts
                        .junction
                        .as_ref()
                        .filter(|j| fk.from.len() == 1 && j.columns.contains(&fk.from[0]));
                    let mut group = ImpactGroup {
                        table: ct.name.as_str().to_owned(),
                        table_display_name: match &shape {
                            Shape::Link(side) => side.heading.clone(),
                            _ => facts.display_name.clone(),
                        },
                        hidden: matches!(shape, Shape::Hidden),
                        via: fk
                            .from
                            .iter()
                            .map(|&c| facts.column_names[c].as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                        action: fk.on_delete.into(),
                        count: 0,
                        depth,
                        truncated: false,
                        sample: Vec::new(),
                        links: junction.is_some(),
                        link_data: junction.is_some_and(|j| j.link_data),
                        blocks: false,
                    };
                    let cascades = fk.on_delete == FkAction::Cascade;
                    'rows: for src in rows {
                        let eqs: Option<Eqs<'a>> = fk
                            .from
                            .iter()
                            .zip(&target.columns)
                            .map(|(&f, &t)| {
                                let cell = src.cell(t);
                                (!cell.is_null()).then(|| (&ct.columns[f].name, cell.param()))
                            })
                            .collect();
                        let Some(eqs) = eqs else { continue };
                        let seen = scheduled.get(&ci).map_or(0, HashSet::len);
                        let fetch = u32::try_from(seen)
                            .unwrap_or(u32::MAX)
                            .saturating_add(remaining)
                            .saturating_add(1);
                        for row in self.rows_where(ct, eqs, fetch)? {
                            let rkey = key_of(ct, &row);
                            if scheduled.get(&ci).is_some_and(|s| s.contains(&rkey)) {
                                continue;
                            }
                            if remaining == 0 {
                                group.truncated = true;
                                truncated = true;
                                break 'rows;
                            }
                            remaining -= 1;
                            group.count += 1;
                            if group.sample.len() < SAMPLE {
                                match &shape {
                                    Shape::Rows(crt) => group.sample.push(render(crt, &rkey, &row)),
                                    Shape::Link(side) => {
                                        group.sample.extend(self.other_label(side, &row)?);
                                    }
                                    Shape::Hidden => {}
                                }
                            }
                            if cascades {
                                scheduled.entry(ci).or_default().insert(rkey);
                                next.entry(ci).or_default().push(row);
                            }
                        }
                    }
                    if group.count == 0 {
                        continue;
                    }
                    group.blocks = match fk.on_delete {
                        FkAction::Restrict | FkAction::NoAction => true,
                        FkAction::SetNull => fk.from.iter().any(|&c| ct.columns[c].not_null),
                        FkAction::Cascade | FkAction::SetDefault => false,
                    };
                    has_triggers |= ct.has_triggers;
                    groups.push(group);
                    if truncated {
                        break;
                    }
                }
                if truncated {
                    break;
                }
            }
            frontier = next;
            depth += 1;
        }
        groups.sort_by(|a, b| (a.depth, &a.table, &a.via).cmp(&(b.depth, &b.table, &b.via)));
        let blocked = groups.iter().any(|g| g.blocks);
        let token = token(&root_version, &groups);
        Ok(DeleteImpact {
            table: rt.name.clone(),
            key: root_key,
            label,
            version: root_version,
            groups,
            blocked,
            truncated,
            has_triggers,
            token,
        })
    }
}

fn is_foreign_key_failure(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _)
            if f.code == ErrorCode::ConstraintViolation
                && f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY
    )
}

impl Session {
    /// What deleting the object would do to every table that references it,
    /// hidden tables counted.
    ///
    /// # Errors
    ///
    /// Unknown table or object, a changed schema, or a database error.
    pub fn delete_preview(
        &self,
        table: &str,
        key: &ObjectKey,
    ) -> Result<DeleteImpact, SessionError> {
        self.read(|r| r.impact(table, key))
    }

    /// Deletes the object, in one transaction after the session's one-time
    /// backup, if `version` and `token` still match what the caller saw and
    /// `config_hash` is the config it was shown under.
    ///
    /// # Errors
    ///
    /// Read-only database, stale config, unknown or not deletable table, a
    /// failed backup, a changed schema, a missing object, a changed version
    /// (`Conflict`), a changed impact (`ImpactChanged`), a delete the database
    /// refuses (`DeleteBlocked`), a busy database or another constraint
    /// failure. Every error leaves the database as it was.
    pub fn delete(
        &mut self,
        table: &str,
        key: &ObjectKey,
        version: &str,
        token: &str,
        config_hash: &str,
    ) -> Result<DeleteResult, SessionError> {
        if self.database.read_only_reason().is_some() {
            return Err(SessionError::ReadOnlyDatabase);
        }
        if config_hash != self.config.hash() || self.config_changed() {
            return Err(SessionError::ConfigChangedOnDisk);
        }
        let rt = self
            .model
            .table(table)
            .ok_or_else(|| SessionError::UnknownTable(table.to_owned()))?;
        if rt.mode != TableMode::Editable || !rt.can_delete {
            return Err(SessionError::NotEditable {
                reason: "Objects in this table cannot be deleted.".to_owned(),
            });
        }
        let schema_index = rt.schema_index;

        #[cfg(test)]
        let stamp = self.backup_stamp.clone();
        #[cfg(not(test))]
        let stamp: Option<String> = None;
        let Session {
            database,
            model,
            backup,
            backup_announced,
            #[cfg(test)]
            after_backup,
            ..
        } = self;
        let mut no_hook = || {};
        #[cfg(test)]
        let hook: &mut dyn FnMut() = match after_backup {
            Some(h) => h.as_mut(),
            None => &mut no_hook,
        };
        #[cfg(not(test))]
        let hook: &mut dyn FnMut() = &mut no_hook;
        let db_path = database.path().to_path_buf();
        let tx = begin_guarded(database.conn(), &db_path, backup, stamp.as_deref(), hook)?;
        if sql::schema_version(&tx)? != model.schema().schema_version {
            return Err(SessionError::SchemaChanged);
        }
        let reader = Reader {
            conn: &tx,
            model: &*model,
        };
        let current = reader.object(table, key)?;
        if current.version != version {
            return Err(SessionError::Conflict {
                current: Box::new(current),
                link: None,
            });
        }
        let impact = reader.impact(table, key)?;
        if impact.token != token {
            return Err(SessionError::ImpactChanged(Box::new(impact)));
        }
        let blocked = |impact: &DeleteImpact| {
            let mut impact = impact.clone();
            impact.blocked = true;
            SessionError::DeleteBlocked(Box::new(impact))
        };
        if impact.blocked {
            return Err(SessionError::DeleteBlocked(Box::new(impact)));
        }
        let st = &model.schema().tables[schema_index];
        let wheres = reader_conditions(st, key)?;
        let q = sql::delete(&st.name, wheres);
        if let Err(e) = tx.execute(&q.sql, params_from_iter(q.params.iter())) {
            return Err(if is_foreign_key_failure(&e) {
                blocked(&impact)
            } else {
                super::save::map_constraint(e, table, &[])
            });
        }
        if let Err(e) = tx.commit() {
            return Err(if is_foreign_key_failure(&e) {
                blocked(&impact)
            } else {
                super::save::map_constraint(e, table, &[])
            });
        }
        let backup_created = if *backup_announced {
            None
        } else {
            *backup_announced = true;
            backup.as_ref().map(|p| p.display().to_string())
        };
        Ok(DeleteResult {
            table: current.table,
            label: current.label,
            impact,
            backup_created,
        })
    }
}

fn reader_conditions<'t>(st: &'t Table, key: &ObjectKey) -> Result<Eqs<'t>, SessionError> {
    super::key::conditions(st, key).ok_or(SessionError::UnknownObject)
}
