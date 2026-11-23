//! Many-to-many link writes: the checks that run before the transaction and
//! the writes that run inside it, after the object itself was saved. Every
//! junction row is addressed by the pair of the two sides' referenced column
//! values (not by `ObjectKey`), because a foreign key may reference a unique
//! column that is not the primary key.

use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection, ErrorCode, OptionalExtension};

use super::key::conditions;
use super::read::{Eqs, Reader};
use super::save::{check_changes, issue, resolve_references, Checked, Phase};
use super::sql;
use super::types::{LinkChange, LinkError, LinkRef, ObjectKey, ObjectView};
use super::version::version;
use super::SessionError;
use crate::config::RelationMode;
use crate::model::{M2mSide, ResolvedTable};
use crate::rules::{FieldError, FieldIssueCode};
use crate::schema::Table;

pub(crate) enum Op<'a> {
    Remove {
        version: Option<&'a str>,
    },
    Update {
        version: &'a str,
        changes: Vec<Checked<'a>>,
    },
    Add {
        changes: Vec<Checked<'a>>,
    },
}

/// One staged link change that passed the checks.
pub(crate) struct Planned<'a> {
    side: &'a M2mSide,
    other: &'a ObjectKey,
    op: Op<'a>,
}

fn link_error<'e>(
    errors: &'e mut Vec<LinkError>,
    relation: &str,
    other: &ObjectKey,
) -> &'e mut LinkError {
    let at = errors
        .iter()
        .position(|e| e.relation == relation && &e.other == other);
    let at = at.unwrap_or_else(|| {
        errors.push(LinkError {
            relation: relation.to_owned(),
            other: other.clone(),
            fields: Vec::new(),
            issues: Vec::new(),
        });
        errors.len() - 1
    });
    &mut errors[at]
}

/// The link checks. A relation that is not one of the table's, or one that
/// cannot be changed, is refused outright; every other problem is collected
/// as a [`LinkError`] so nothing is written and the UI sees them together.
/// Removes come first in the plan, then updates, then adds, per relation.
pub(crate) fn plan<'a>(
    rt: &'a ResolvedTable,
    links: &'a [LinkChange],
    phase: Phase,
) -> Result<(Vec<Planned<'a>>, Vec<LinkError>), SessionError> {
    let mut planned = Vec::new();
    let mut errors: Vec<LinkError> = Vec::new();
    let mut seen: Vec<(&str, &ObjectKey)> = Vec::new();
    for lc in links {
        let side = rt
            .many_to_many
            .iter()
            .find(|s| s.relation == lc.relation)
            .ok_or_else(|| SessionError::UnknownRelation(lc.relation.clone()))?;
        let any = !(lc.add.is_empty() && lc.update.is_empty() && lc.remove.is_empty());
        if any && side.mode != RelationMode::Editable {
            return Err(SessionError::NotEditable {
                reason: "This relation cannot be changed.".to_owned(),
            });
        }
        if !lc.add.is_empty() && !side.can_add {
            return Err(SessionError::NotEditable {
                reason: "Links cannot be added to this relation.".to_owned(),
            });
        }
        let relation = lc.relation.as_str();
        let not_allowed = |errors: &mut Vec<LinkError>, other: &ObjectKey, text: &str| {
            link_error(errors, relation, other)
                .issues
                .push(issue(FieldIssueCode::NotAllowed, text));
        };
        let mut first = |errors: &mut Vec<LinkError>, other: &'a ObjectKey| {
            if seen.contains(&(relation, other)) {
                not_allowed(errors, other, "This link is listed more than once.");
                false
            } else {
                seen.push((relation, other));
                true
            }
        };
        for r in &lc.remove {
            if phase == Phase::Create {
                not_allowed(
                    &mut errors,
                    &r.other,
                    "A new object has no links to remove.",
                );
            } else if first(&mut errors, &r.other) {
                planned.push(Planned {
                    side,
                    other: &r.other,
                    op: Op::Remove {
                        version: r.version.as_deref(),
                    },
                });
            }
        }
        for u in &lc.update {
            if phase == Phase::Create {
                not_allowed(
                    &mut errors,
                    &u.other,
                    "A new object has no links to change.",
                );
            } else if first(&mut errors, &u.other) {
                let (changes, errs) =
                    check_changes(&side.link_fields, &u.fields, Phase::Update, &[]);
                if errs.is_empty() {
                    planned.push(Planned {
                        side,
                        other: &u.other,
                        op: Op::Update {
                            version: &u.version,
                            changes,
                        },
                    });
                } else {
                    link_error(&mut errors, relation, &u.other)
                        .fields
                        .extend(errs);
                }
            }
        }
        for a in &lc.add {
            if first(&mut errors, &a.other) {
                let (changes, errs) =
                    check_changes(&side.link_fields, &a.fields, Phase::Create, &[]);
                if errs.is_empty() {
                    planned.push(Planned {
                        side,
                        other: &a.other,
                        op: Op::Add { changes },
                    });
                } else {
                    link_error(&mut errors, relation, &a.other)
                        .fields
                        .extend(errs);
                }
            }
        }
    }
    Ok((planned, errors))
}

/// True when the request changes no link at all.
pub(crate) fn is_empty(links: &[LinkChange]) -> bool {
    links
        .iter()
        .all(|l| l.add.is_empty() && l.update.is_empty() && l.remove.is_empty())
}

/// One column of the row `key` names, `None` when the row is missing or the
/// value is NULL.
fn column_value(
    conn: &Connection,
    table: &Table,
    key: &ObjectKey,
    column: usize,
) -> Result<Option<Value>, SessionError> {
    let Some(eqs) = conditions(table, key) else {
        return Ok(None);
    };
    let mut sel = sql::Select::new(&table.name);
    sel.columns = vec![&table.columns[column].name];
    sel.filters = eqs
        .into_iter()
        .map(|(c, v)| sql::Filter::Eq(c, v))
        .collect();
    let q = sel.rows();
    let value = conn
        .query_row(&q.sql, params_from_iter(q.params.iter()), |r| {
            r.get::<_, Value>(0)
        })
        .optional()?;
    Ok(value.filter(|v| *v != Value::Null))
}

struct Ctx<'r, 'a> {
    reader: &'r Reader<'a>,
    /// The object as read before any write (`None` on a create).
    existing: Option<&'r ObjectView>,
    this_key: &'r ObjectKey,
}

impl Ctx<'_, '_> {
    /// A link conflict. `current` is the object as stored before this save
    /// (never its uncommitted edits). A create has none: a junction row that
    /// already exists there is an orphan row, a data problem, so it is a
    /// `Constraint`.
    fn conflict(&self, side: &M2mSide, other: &ObjectKey, junction: &Table) -> SessionError {
        match self.existing {
            Some(current) => SessionError::Conflict {
                current: Box::new(current.clone()),
                link: Some(LinkRef {
                    relation: side.relation.clone(),
                    other: other.clone(),
                }),
            },
            None => SessionError::Constraint {
                message: format!(
                    "An existing row in {} already refers to this value (an orphan row, \
                     since foreign keys were off). Nothing was changed.",
                    junction.name.as_str()
                ),
            },
        }
    }

    /// A failed junction write. A unique failure on exactly the pair is a
    /// conflict; one on exactly one changed link field, or a missing value
    /// for one, is an issue on that link; anything else is the database's
    /// message.
    fn failed(
        &self,
        err: rusqlite::Error,
        side: &M2mSide,
        other: &ObjectKey,
        junction: &Table,
        changed: &[&str],
    ) -> SessionError {
        if let rusqlite::Error::SqliteFailure(f, Some(message)) = &err {
            let unique = f.code == ErrorCode::ConstraintViolation
                && matches!(
                    f.extended_code,
                    rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                        | rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
                );
            if let (true, Some((_, named))) = (unique, message.split_once(" constraint failed: ")) {
                let mut entries: Vec<&str> = named.split(", ").collect();
                entries.sort_unstable();
                let mut pair = [
                    format!(
                        "{}.{}",
                        junction.name.as_str(),
                        junction.columns[side.this_column].name.as_str()
                    ),
                    format!(
                        "{}.{}",
                        junction.name.as_str(),
                        junction.columns[side.other_column].name.as_str()
                    ),
                ];
                pair.sort_unstable();
                if entries == pair {
                    return self.conflict(side, other, junction);
                }
            }
        }
        match super::save::map_constraint(err, junction.name.as_str(), changed) {
            SessionError::Validation { fields, .. } => SessionError::Validation {
                fields: Vec::new(),
                links: vec![LinkError {
                    relation: side.relation.clone(),
                    other: other.clone(),
                    fields,
                    issues: Vec::new(),
                }],
            },
            e => e,
        }
    }
}

/// Step 11: writes the planned link changes inside the transaction `conn`
/// belongs to. `this_key` is the saved object's key (just inserted, on a
/// create). Any failure returns an error and the caller rolls back.
pub(crate) fn apply(
    reader: &Reader<'_>,
    existing: Option<&ObjectView>,
    this_table: &Table,
    this_key: &ObjectKey,
    planned: Vec<Planned<'_>>,
) -> Result<(), SessionError> {
    let ctx = Ctx {
        reader,
        existing,
        this_key,
    };
    let mut errors: Vec<LinkError> = Vec::new();
    for p in planned {
        ctx.apply_one(this_table, p, &mut errors)?;
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(SessionError::Validation {
            fields: Vec::new(),
            links: errors,
        })
    }
}

impl Ctx<'_, '_> {
    /// One planned change. Issues that do not stop the save (a missing other
    /// object, a missing reference) go to `errors`; a conflict or database
    /// failure is returned.
    fn apply_one(
        &self,
        this_table: &Table,
        planned: Planned<'_>,
        errors: &mut Vec<LinkError>,
    ) -> Result<(), SessionError> {
        let Planned { side, other, op } = planned;
        let conn = self.reader.conn;
        let model = self.reader.model;
        let schema = model.schema();
        let junction = &schema.tables[side.junction];
        let this_id = &junction.columns[side.this_column].name;
        let other_id = &junction.columns[side.other_column].name;
        let this_value = column_value(conn, this_table, self.this_key, side.this_key)?
            .ok_or(SessionError::UnknownObject)?;
        let other_value = column_value(
            conn,
            &schema.tables[side.other_table],
            other,
            side.other_key,
        )?;
        let Some(other_value) = other_value else {
            if let Op::Add { .. } = op {
                link_error(errors, &side.relation, other).issues.push(issue(
                    FieldIssueCode::ReferenceMissing,
                    "The chosen object no longer exists.",
                ));
                return Ok(());
            }
            return Err(self.conflict(side, other, junction));
        };
        let eqs = || -> Eqs<'_> {
            vec![
                (this_id, this_value.clone()),
                (other_id, other_value.clone()),
            ]
        };
        // Duplicate junction pairs are unreachable under the pair-uniqueness
        // rule, so versioning only the first match is correct.
        let row = self.reader.full_row(junction, &eqs())?;
        let stored = row
            .as_ref()
            .map(|r| version((0..junction.columns.len()).map(|c| r.cell(c))));
        let run = |q: &sql::Query, changed: &[&str]| {
            conn.execute(&q.sql, params_from_iter(q.params.iter()))
                .map(|_| ())
                .map_err(|e| self.failed(e, side, other, junction, changed))
        };
        let names = |bound: &[super::save::Bound<'_>]| -> Vec<String> {
            bound
                .iter()
                .map(|b| junction.columns[b.field.column].name.as_str().to_owned())
                .collect()
        };
        match op {
            Op::Remove { version } => {
                if stored.is_none() || version.is_some_and(|v| stored.as_deref() != Some(v)) {
                    return Err(self.conflict(side, other, junction));
                }
                run(&sql::delete(&junction.name, eqs()), &[])
            }
            Op::Update { version, changes } => {
                if stored.as_deref() != Some(version) {
                    return Err(self.conflict(side, other, junction));
                }
                let Some(bound) = resolved(conn, model, changes, side, other, errors)? else {
                    return Ok(());
                };
                if bound.is_empty() {
                    return Ok(());
                }
                let sets = bound
                    .iter()
                    .map(|b| (&junction.columns[b.field.column].name, b.value.clone()))
                    .collect();
                let changed = names(&bound);
                let changed: Vec<&str> = changed.iter().map(String::as_str).collect();
                run(&sql::update(&junction.name, sets, eqs()), &changed)
            }
            Op::Add { changes } => {
                if stored.is_some() {
                    return Err(self.conflict(side, other, junction));
                }
                let Some(bound) = resolved(conn, model, changes, side, other, errors)? else {
                    return Ok(());
                };
                let mut values = vec![
                    (this_id, this_value.clone()),
                    (other_id, other_value.clone()),
                ];
                values.extend(
                    bound
                        .iter()
                        .map(|b| (&junction.columns[b.field.column].name, b.value.clone())),
                );
                let changed = names(&bound);
                let changed: Vec<&str> = changed.iter().map(String::as_str).collect();
                run(&sql::insert(&junction.name, values), &changed)
            }
        }
    }
}

/// `resolve_references` for one link; a missing reference becomes link field
/// issues and `None`.
fn resolved<'c>(
    conn: &Connection,
    model: &crate::model::Model,
    changes: Vec<Checked<'c>>,
    side: &M2mSide,
    other: &ObjectKey,
    errors: &mut Vec<LinkError>,
) -> Result<Option<Vec<super::save::Bound<'c>>>, SessionError> {
    match resolve_references(conn, model, changes)? {
        Ok(b) => Ok(Some(b)),
        Err(fields) => {
            let fields: Vec<FieldError> = fields;
            link_error(errors, &side.relation, other)
                .fields
                .extend(fields);
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::Fixture;

    #[test]
    fn ctx_failed_maps_pairs_unit() {
        let fx = Fixture::example();
        let s = fx.session();
        let reader = Reader {
            conn: s.database.conn(),
            model: &s.model,
        };
        let book = s.model.table("book").unwrap();
        let side = book
            .many_to_many
            .iter()
            .find(|m| m.relation == "m2m:contribution")
            .unwrap();
        let junction = &s.model.schema().tables[side.junction];
        let current = s
            .object("book", &ObjectKey::Rowid { value: "1".into() })
            .unwrap();
        let ctx = Ctx {
            reader: &reader,
            existing: Some(&current),
            this_key: &current.key,
        };
        let other = ObjectKey::Rowid { value: "2".into() };
        let failure = |extended: i32, msg: &str| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(extended),
                Some(msg.to_owned()),
            )
        };
        let unique = rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE;
        let t = &junction.name;
        // the pair, in either order
        let pair = format!(
            "UNIQUE constraint failed: {t}.{}, {t}.{}",
            junction.columns[side.other_column].name, junction.columns[side.this_column].name
        );
        match ctx.failed(failure(unique, &pair), side, &other, junction, &["role"]) {
            SessionError::Conflict { link, current: c } => {
                assert_eq!(link.unwrap().other, other);
                assert_eq!(c.version, current.version);
            }
            e => panic!("expected Conflict, got {e:?}"),
        }
        // one link field
        let one = format!("UNIQUE constraint failed: {t}.role");
        match ctx.failed(failure(unique, &one), side, &other, junction, &["role"]) {
            SessionError::Validation { links, .. } => {
                assert_eq!(links.len(), 1);
                assert_eq!(links[0].relation, side.relation);
                assert_eq!(links[0].fields.len(), 1);
                assert_eq!(links[0].fields[0].column, "role");
            }
            e => panic!("expected Validation, got {e:?}"),
        }
        let check = ctx.failed(
            failure(
                rusqlite::ffi::SQLITE_CONSTRAINT_CHECK,
                "CHECK constraint failed: x",
            ),
            side,
            &other,
            junction,
            &["role"],
        );
        assert!(matches!(check, SessionError::Constraint { message } if message.contains("CHECK")));
    }
}
