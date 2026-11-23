//! Saving one object's fields: the checks, the reference resolution and the
//! single transaction. The helpers are `pub(crate)` and assume nothing about
//! the fields belonging to an object table, so link writes reuse them.

use rusqlite::types::Value;
use std::path::{Path, PathBuf};

use rusqlite::{
    params_from_iter, Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior,
};

use super::backup::ensure_backup;
use super::key::conditions;
use super::links;
use super::read::Reader;
use super::sql;
use super::types::{FieldChange, FieldInput, ObjectKey, SaveRequest, SaveResult};
use super::{Session, SessionError};
use crate::config::{FieldMode, TableMode};
use crate::model::{Model, ResolvedField, ResolvedTable};
use crate::rules::{validate, FieldError, FieldIssue, FieldIssueCode};
use crate::schema::{Identity, Table};
use crate::value::DbValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Create,
    Update,
}

/// A change that passed the checks. A `Value` is the one `validate` returned.
pub(crate) enum Input {
    Value(DbValue),
    Reference(Option<ObjectKey>),
}

pub(crate) struct Checked<'a> {
    pub field: &'a ResolvedField,
    pub input: Input,
}

/// A change ready to write: the column and the value to bind.
pub(crate) struct Bound<'a> {
    pub field: &'a ResolvedField,
    pub value: Value,
}

pub(crate) fn issue(code: FieldIssueCode, message: &str) -> FieldIssue {
    FieldIssue {
        code,
        message: message.to_owned(),
        limit: None,
    }
}

fn push_issue(errors: &mut Vec<FieldError>, column: &str, issue: FieldIssue) {
    if let Some(e) = errors.iter_mut().find(|e| e.column == column) {
        e.issues.push(issue);
    } else {
        errors.push(FieldError {
            column: column.to_owned(),
            issues: vec![issue],
        });
    }
}

/// Steps 3 to 5 over any resolved field set: unknown, duplicate, not
/// editable and wrongly typed changes, `validate` of each plain value (only
/// changed fields are validated), and, on a create, every
/// `required_on_create` field, and every field whose column index is in
/// `also_required`, left out or set to NULL. All issues are returned together.
pub(crate) fn check_changes<'a>(
    fields: &'a [ResolvedField],
    changes: &[FieldChange],
    phase: Phase,
    also_required: &[usize],
) -> (Vec<Checked<'a>>, Vec<FieldError>) {
    let mut checked: Vec<Checked<'a>> = Vec::new();
    let mut errors: Vec<FieldError> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for change in changes {
        let column = change.column.as_str();
        let found = fields
            .iter()
            .find(|f| f.name == column && f.mode != FieldMode::Hidden);
        let Some(field) = found else {
            push_issue(
                &mut errors,
                column,
                issue(FieldIssueCode::UnknownField, "There is no such field."),
            );
            continue;
        };
        if seen.contains(&column) {
            push_issue(
                &mut errors,
                column,
                issue(
                    FieldIssueCode::UnknownField,
                    "The field was given more than once.",
                ),
            );
            continue;
        }
        seen.push(column);
        let allowed = match phase {
            Phase::Update => field.mode == FieldMode::Editable,
            Phase::Create => matches!(field.mode, FieldMode::Editable | FieldMode::CreateOnly),
        };
        if !allowed {
            push_issue(
                &mut errors,
                column,
                issue(FieldIssueCode::NotEditable, "This field cannot be changed."),
            );
            continue;
        }
        let wrong = || issue(FieldIssueCode::WrongType, "The value has the wrong type.");
        match (&change.input, field.foreign_key.is_some()) {
            (FieldInput::Value { value }, false) => {
                if matches!(value, DbValue::Blob { .. }) {
                    push_issue(&mut errors, column, wrong());
                    continue;
                }
                match validate(&field.rule, value) {
                    Ok(DbValue::Null)
                        if phase == Phase::Create && also_required.contains(&field.column) =>
                    {
                        push_issue(
                            &mut errors,
                            column,
                            issue(FieldIssueCode::Required, "A value is required."),
                        );
                    }
                    Ok(v) => checked.push(Checked {
                        field,
                        input: Input::Value(v),
                    }),
                    Err(issues) => {
                        for i in issues {
                            push_issue(&mut errors, column, i);
                        }
                    }
                }
            }
            (FieldInput::Reference { key }, true) => {
                if key.is_none() && !field.rule.nullable {
                    push_issue(
                        &mut errors,
                        column,
                        issue(FieldIssueCode::Required, "A value is required."),
                    );
                } else {
                    checked.push(Checked {
                        field,
                        input: Input::Reference(key.clone()),
                    });
                }
            }
            _ => push_issue(&mut errors, column, wrong()),
        }
    }
    if phase == Phase::Create {
        for f in fields {
            if (f.required_on_create || also_required.contains(&f.column))
                && !seen.contains(&f.name.as_str())
            {
                push_issue(
                    &mut errors,
                    &f.name,
                    issue(FieldIssueCode::Required, "A value is required."),
                );
            }
        }
    }
    (checked, errors)
}

/// Step 9: binds every checked change. A reference must name an existing
/// row of the foreign key's target table; the stored value is that row's
/// referenced column. The outer error is a database error, the inner one
/// the `referenceMissing` issues, collected.
pub(crate) fn resolve_references<'a>(
    conn: &Connection,
    model: &Model,
    checked: Vec<Checked<'a>>,
) -> Result<Result<Vec<Bound<'a>>, Vec<FieldError>>, SessionError> {
    let mut bound = Vec::with_capacity(checked.len());
    let mut errors = Vec::new();
    for c in checked {
        let value = match c.input {
            Input::Value(v) => v.to_sql(&c.field.rule),
            Input::Reference(None) => Value::Null,
            Input::Reference(Some(key)) => {
                let Some(v) = referenced_value(conn, model, c.field, &key)? else {
                    push_issue(
                        &mut errors,
                        &c.field.name,
                        issue(
                            FieldIssueCode::ReferenceMissing,
                            "The chosen object no longer exists.",
                        ),
                    );
                    continue;
                };
                v
            }
        };
        bound.push(Bound {
            field: c.field,
            value,
        });
    }
    Ok(if errors.is_empty() {
        Ok(bound)
    } else {
        Err(errors)
    })
}

fn referenced_value(
    conn: &Connection,
    model: &Model,
    field: &ResolvedField,
    key: &ObjectKey,
) -> Result<Option<Value>, SessionError> {
    let Some(fk) = &field.foreign_key else {
        return Ok(None);
    };
    let (Some(target), Some(&column)) = (model.schema().tables.get(fk.table), fk.columns.first())
    else {
        return Ok(None);
    };
    let Some(eqs) = conditions(target, key) else {
        return Ok(None);
    };
    let mut sel = sql::Select::new(&target.name);
    sel.columns = vec![&target.columns[column].name];
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

/// Maps a SQLite failure of a write. A unique or not-null violation that
/// names exactly one of `changed_columns` of `table` becomes an issue on it;
/// any other constraint failure carries SQLite's message.
pub(crate) fn map_constraint(
    err: rusqlite::Error,
    table: &str,
    changed_columns: &[&str],
) -> SessionError {
    let rusqlite::Error::SqliteFailure(failure, message) = &err else {
        return err.into();
    };
    if failure.code != ErrorCode::ConstraintViolation {
        return err.into();
    }
    let message = message.clone().unwrap_or_else(|| err.to_string());
    let code = match failure.extended_code {
        rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE | rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY => {
            Some((FieldIssueCode::Unique, "This value is already used."))
        }
        rusqlite::ffi::SQLITE_CONSTRAINT_NOTNULL => {
            Some((FieldIssueCode::Required, "A value is required."))
        }
        _ => None,
    };
    if let Some((code, text)) = code {
        if let Some((_, named)) = message.split_once(" constraint failed: ") {
            let entries: Vec<&str> = named.split(", ").collect();
            if let [only] = entries.as_slice() {
                if let Some(c) = changed_columns
                    .iter()
                    .find(|c| *only == format!("{table}.{c}"))
                {
                    return SessionError::Validation {
                        fields: vec![FieldError {
                            column: (*c).to_owned(),
                            issues: vec![issue(code, text)],
                        }],
                        links: Vec::new(),
                    };
                }
            }
        }
    }
    SessionError::Constraint { message }
}

fn to_db_value(v: &Value) -> Option<DbValue> {
    match v {
        Value::Null => Some(DbValue::Null),
        Value::Integer(n) => Some(DbValue::Integer(n.to_string())),
        Value::Real(x) => Some(DbValue::Real(x.to_string())),
        Value::Text(s) => Some(DbValue::Text(s.clone())),
        Value::Blob(_) => None,
    }
}

/// The key of the saved object: the rowid for a rowid table, else the
/// primary-key values, written or (on an update) carried over.
fn saved_key(
    st: &Table,
    bound: &[Bound<'_>],
    created_rowid: i64,
    old: Option<&ObjectKey>,
) -> Result<ObjectKey, SessionError> {
    match (&st.identity, old) {
        (Identity::Rowid { .. }, Some(old)) => Ok(old.clone()),
        (Identity::Rowid { .. }, None) => Ok(ObjectKey::Rowid {
            value: created_rowid.to_string(),
        }),
        (Identity::PrimaryKey { columns }, old) => {
            let carried = match old {
                Some(ObjectKey::Primary { values }) => Some(values),
                _ => None,
            };
            let mut values = Vec::with_capacity(columns.len());
            for (i, &c) in columns.iter().enumerate() {
                let written = bound.iter().find(|b| b.field.column == c);
                let v = match written {
                    Some(b) => to_db_value(&b.value),
                    None => carried.and_then(|v| v.get(i).cloned()),
                };
                values.push(v.ok_or_else(|| {
                    SessionError::Sqlite("the saved object's key cannot be determined".to_owned())
                })?);
            }
            Ok(ObjectKey::Primary { values })
        }
    }
}

/// How often the backup is retaken when another connection commits between
/// the copy and `BEGIN IMMEDIATE`.
const RACE_RETRIES: u32 = 3;

/// Takes the session's backup if it has none, then `BEGIN IMMEDIATE`. The
/// backup cannot be taken inside the transaction (SQLite answers `Busy`), so
/// `PRAGMA data_version` is read before the copy and after the lock: when it
/// changed, another connection committed in between and the copy misses that
/// commit. The transaction is rolled back, the copy (never announced) is
/// deleted and the loop retakes it; after `RACE_RETRIES` tries the answer is
/// `Busy`. `after_backup` is a test seam that runs between the copy and the
/// lock.
fn begin_guarded<'c>(
    conn: &'c Connection,
    database: &Path,
    backup: &mut Option<PathBuf>,
    stamp: Option<&str>,
    after_backup: &mut dyn FnMut(),
) -> Result<Transaction<'c>, SessionError> {
    for _ in 0..RACE_RETRIES {
        let took = backup.is_none();
        let before = sql::data_version(conn)?;
        ensure_backup(backup, stamp, conn, database)?;
        after_backup();
        let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
        if !took || sql::data_version(&tx)? == before {
            return Ok(tx);
        }
        drop(tx);
        if let Some(stale) = backup.take() {
            std::fs::remove_file(&stale).map_err(|e| SessionError::BackupFailed {
                detail: format!("could not remove a stale backup: {e}"),
            })?;
        }
    }
    Err(SessionError::Busy)
}

impl Session {
    /// Steps 1 and 2: the refusals that need no field checks. Returns the
    /// table (cloned, so the session can be borrowed mutably later) and the
    /// phase.
    fn permitted(&self, request: &SaveRequest) -> Result<(ResolvedTable, Phase), SessionError> {
        // Steps 1 and 2.
        if self.database.read_only_reason().is_some() {
            return Err(SessionError::ReadOnlyDatabase);
        }
        if request.config_hash != self.config.hash() || self.config_changed() {
            return Err(SessionError::ConfigChangedOnDisk);
        }
        let rt = self
            .model
            .table(&request.table)
            .cloned()
            .ok_or_else(|| SessionError::UnknownTable(request.table.clone()))?;
        let phase = if request.key.is_some() {
            Phase::Update
        } else {
            Phase::Create
        };
        let permitted = match phase {
            Phase::Update => rt.mode == TableMode::Editable,
            Phase::Create => rt.can_create,
        };
        if !permitted {
            return Err(SessionError::NotEditable {
                reason: match phase {
                    Phase::Update => "This table cannot be changed.".to_owned(),
                    Phase::Create => "New objects cannot be created in this table.".to_owned(),
                },
            });
        }

        Ok((rt, phase))
    }

    /// A save that changes nothing: only the version is checked.
    fn version_only(
        &self,
        request: &SaveRequest,
        key: &ObjectKey,
    ) -> Result<SaveResult, SessionError> {
        let current = self.object(&request.table, key)?;
        if Some(&current.version) != request.version.as_ref() {
            return Err(SessionError::Conflict {
                current: Box::new(current),
                link: None,
            });
        }
        Ok(SaveResult {
            object: current,
            backup_created: None,
        })
    }

    /// Saves one object's field changes (an update, or a create when
    /// `request.key` is `None`) in one transaction, after the session's
    /// one-time backup. Nothing is written unless everything is.
    ///
    /// # Errors
    ///
    /// Read-only database, stale config, unknown or not editable table,
    /// invalid changes, a stale version, a failed backup, a changed schema, a
    /// busy database or a constraint failure. Every error leaves the
    /// database as it was.
    pub fn save(&mut self, request: &SaveRequest) -> Result<SaveResult, SessionError> {
        let (rt, phase) = self.permitted(request)?;

        // Steps 3 to 5.
        let also_required = match phase {
            Phase::Create => self.model.schema().tables[rt.schema_index].required_key_columns(),
            Phase::Update => Vec::new(),
        };
        let (checked, errors) = check_changes(&rt.fields, &request.changes, phase, &also_required);
        let (planned, link_errors) = links::plan(&rt, &request.links, phase)?;
        if !errors.is_empty() || !link_errors.is_empty() {
            return Err(SessionError::Validation {
                fields: errors,
                links: link_errors,
            });
        }
        let links_empty = links::is_empty(&request.links);
        if let (Some(key), true) = (&request.key, request.changes.is_empty() && links_empty) {
            return self.version_only(request, key);
        }

        // Steps 6 to 12.
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
        let schema_version = model.schema().schema_version;
        let st = &model.schema().tables[rt.schema_index];
        let table_name = st.name.as_str();
        let changed: Vec<&str> = request.changes.iter().map(|c| c.column.as_str()).collect();
        let db_path = database.path().to_path_buf();
        // Steps 6 and 7: the backup, then `BEGIN IMMEDIATE`, guarded.
        let tx = begin_guarded(database.conn(), &db_path, backup, stamp.as_deref(), hook)?;
        if sql::schema_version(&tx)? != schema_version {
            return Err(SessionError::SchemaChanged);
        }
        let reader = Reader { conn: &tx, model };
        // Step 8's read, before any write: every conflict reports it.
        let existing = if let Some(key) = &request.key {
            let existing = reader.object(&request.table, key)?;
            if Some(&existing.version) != request.version.as_ref() {
                return Err(SessionError::Conflict {
                    current: Box::new(existing),
                    link: None,
                });
            }
            Some(existing)
        } else {
            None
        };
        let bound = match resolve_references(&tx, model, checked)? {
            Ok(b) => b,
            Err(fields) => {
                return Err(SessionError::Validation {
                    fields,
                    links: Vec::new(),
                })
            }
        };
        let values: Vec<_> = bound
            .iter()
            .map(|b| (&st.columns[b.field.column].name, b.value.clone()))
            .collect();
        if request.key.is_none() || !values.is_empty() {
            let write = match &request.key {
                Some(key) => {
                    let wheres = conditions(st, key).ok_or(SessionError::UnknownObject)?;
                    sql::update(&st.name, values, wheres)
                }
                None => sql::insert(&st.name, values),
            };
            tx.execute(&write.sql, params_from_iter(write.params.iter()))
                .map_err(|e| map_constraint(e, table_name, &changed))?;
        }
        let key = saved_key(st, &bound, tx.last_insert_rowid(), request.key.as_ref())?;
        // Step 11: the links.
        links::apply(&reader, existing.as_ref(), st, &key, planned)?;
        let object = reader.object(&request.table, &key)?;
        tx.commit()
            .map_err(|e| map_constraint(e, table_name, &changed))?;
        let backup_created = if *backup_announced {
            None
        } else {
            *backup_announced = true;
            backup.as_ref().map(|p| p.display().to_string())
        };
        Ok(SaveResult {
            object,
            backup_created,
        })
    }
}
