//! The Database Manager's TOML config: an overlay on the database's own
//! structure that says what is editable, read-only, hidden or create-only.
//! This module parses and checks what the config text alone can show; the
//! reference is `docs/dbmanager-config.md`.

mod model;
mod raw;
mod validate;

use toml::de::{DeTable, DeValue};

pub use model::{
    AllowedValues, At, Bound, Config, ConfigDiagnostic, Defaults, DiagnosticSeverity, DurationUnit,
    FieldConfig, FieldEntry, FieldMode, FieldType, Heading, LabelPart, LabelTemplate, Located,
    M2mConfig, M2mEntry, OrderBy, RelationMode, TableConfig, TableEntry, TableMode,
};

/// The outcome of [`parse`]. `config` always exists.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigReport {
    pub config: Config,
    /// Sorted by (line, col).
    pub diagnostics: Vec<ConfigDiagnostic>,
    /// `Some(short reason)` iff `config` is [`Config::fail_closed`] because
    /// the whole config could not be used ("syntax error at line 4",
    /// "unsupported version 2"). `None` for every other config, including an
    /// empty one. The UI shows this reason and must never infer fail-closed
    /// from diagnostic text.
    pub fail_closed: Option<String>,
}

fn fail_closed(reason: String, at: At, message: String) -> ConfigReport {
    ConfigReport {
        config: Config::fail_closed(),
        diagnostics: vec![ConfigDiagnostic {
            severity: DiagnosticSeverity::Error,
            message,
            line: Some(at.line),
            col: Some(at.col),
            table: None,
            column: None,
        }],
        fail_closed: Some(reason),
    }
}

/// Parses config text. A leading UTF-8 BOM is accepted. A syntax error or a
/// `version` other than 1 gives [`Config::fail_closed`] and one error; every
/// other problem is a diagnostic on the entry it belongs to.
#[must_use]
pub fn parse(text: &str) -> ConfigReport {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut ctx = validate::Ctx::new(text);
    let doc = match DeTable::parse(text) {
        Ok(doc) => doc,
        Err(e) => {
            let at = ctx.at(e.span().map_or(0, |s| s.start));
            return fail_closed(
                format!("syntax error at line {}", at.line),
                at,
                e.message().trim().to_string(),
            );
        }
    };
    let top = doc.get_ref();
    let version = top.iter().find(|(k, _)| validate::key_name(k) == "version");
    match version {
        None => ctx.warning(0, "missing `version`; read as version 1".into()),
        Some((_, v)) => {
            let supported = match v.get_ref() {
                DeValue::Integer(i) => {
                    if i64::from_str_radix(i.as_str(), i.radix()) == Ok(1) {
                        Ok(())
                    } else {
                        Err(format!("unsupported version {}", i.as_str()))
                    }
                }
                other => Err(format!("invalid version ({})", other.type_str())),
            };
            if let Err(reason) = supported {
                let at = ctx.at(v.span().start);
                let message = format!(
                    "{reason}: only `version = 1` is supported, so the whole config is ignored and everything is read-only"
                );
                return fail_closed(reason, at, message);
            }
        }
    }
    let config = raw::read(&mut ctx, top);
    ConfigReport {
        config,
        diagnostics: ctx.into_sorted(),
        fail_closed: None,
    }
}

#[cfg(test)]
mod tests;
