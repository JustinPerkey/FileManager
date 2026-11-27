//! The draft: the edits to one open (or new) object, held by the model. Pure
//! logic with no GPUI types, so it is unit-tested alone.
//!
//! Every editable field has an editor state ([`Input`]). Whether a field is
//! **changed** is decided by converting the editor text to a [`DbValue`],
//! normalising it with [`validate`] and comparing it with the stored value, so
//! typing `1h 30m` over a stored `1h30m` is not a change. Validation is
//! `fm_dbmanager::rules::validate` only; nothing here validates a second way.

use fm_dbmanager::config::FieldMode;
use fm_dbmanager::model::{FieldView, TableView};
use fm_dbmanager::rules::{validate, FieldError, FieldIssue, FieldIssueCode, FieldKind};
use fm_dbmanager::session::{
    FieldChange, FieldInput, FieldValue, ObjectKey, ObjectView, SaveRequest,
};
use fm_dbmanager::value::DbValue;

use super::text;

/// What the view hands the model for one edit. References change only through
/// the picker methods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorInput {
    /// Integer, real, text and duration editors.
    Text(String),
    /// A bool editor; `None` is "not set" on a nullable field.
    Bool(Option<bool>),
    /// An allowed-values select; `None` is "Not set".
    Choice(Option<String>),
}

/// A picked reference: the key to store and the label to show.
#[derive(Clone, Debug)]
pub struct Pick {
    pub key: ObjectKey,
    /// Empty when not known.
    pub label: String,
}

/// One field's editor state.
#[derive(Clone, Debug)]
pub enum Input {
    Text(String),
    Bool(Option<bool>),
    Choice(Option<String>),
    Reference(Option<Pick>),
}

fn input_eq(a: &Input, b: &Input) -> bool {
    match (a, b) {
        (Input::Text(a), Input::Text(b)) => a == b,
        (Input::Bool(a), Input::Bool(b)) => a == b,
        (Input::Choice(a), Input::Choice(b)) => a == b,
        (Input::Reference(a), Input::Reference(b)) => {
            a.as_ref().map(|p| &p.key) == b.as_ref().map(|p| &p.key)
        }
        _ => false,
    }
}

fn same_variant(a: &Input, b: &Input) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Stored and entered values are the same value (reals compare as numbers).
fn same_value(a: &DbValue, b: &DbValue) -> bool {
    match (a, b) {
        (DbValue::Real(a), DbValue::Real(b)) => match (a.parse::<f64>(), b.parse::<f64>()) {
            (Ok(x), Ok(y)) => x.total_cmp(&y).is_eq(),
            _ => a == b,
        },
        _ => a == b,
    }
}

fn plain(value: &DbValue) -> Option<&str> {
    match value {
        DbValue::Integer(s) | DbValue::Real(s) | DbValue::Text(s) | DbValue::Duration(s) => Some(s),
        DbValue::Null | DbValue::Blob { .. } => None,
    }
}

fn required_issue() -> FieldIssue {
    FieldIssue {
        code: FieldIssueCode::Required,
        message: "A value is required.".to_owned(),
        limit: None,
    }
}

/// The editor state a stored (or default) value starts as.
fn input_from_value(view: &FieldView, value: &DbValue) -> Input {
    if view.kind == FieldKind::Bool {
        return Input::Bool(match value {
            DbValue::Integer(s) if s == "1" => Some(true),
            DbValue::Integer(s) if s == "0" => Some(false),
            _ => None,
        });
    }
    if view.rule.allowed.is_some() {
        return Input::Choice(plain(value).map(str::to_owned));
    }
    Input::Text(plain(value).unwrap_or_default().to_owned())
}

/// The editor state of a field with the stored `value` (`None`: a new object,
/// which starts at the known default).
fn initial_input(view: &FieldView, value: Option<&FieldValue>) -> Input {
    if view.foreign_key.is_some() {
        return Input::Reference(value.and_then(|v| v.link.as_ref()).and_then(|l| {
            l.key.clone().map(|key| Pick {
                key,
                label: l.label.clone().unwrap_or_default(),
            })
        }));
    }
    match value {
        Some(v) => input_from_value(view, &v.value),
        None => input_from_value(view, view.default_value.as_ref().unwrap_or(&DbValue::Null)),
    }
}

fn has_editor(view: &FieldView, new: bool) -> bool {
    let mode_ok = match view.mode {
        FieldMode::Editable => true,
        FieldMode::CreateOnly => new,
        FieldMode::ReadOnly | FieldMode::Hidden => false,
    };
    mode_ok && !matches!(view.kind, FieldKind::Blob | FieldKind::Any)
}

fn typed(kind: FieldKind, text: &str) -> DbValue {
    let trimmed = text.trim();
    match kind {
        FieldKind::Integer | FieldKind::Bool | FieldKind::Real | FieldKind::Duration
            if trimmed.is_empty() =>
        {
            DbValue::Null
        }
        FieldKind::Integer | FieldKind::Bool => DbValue::Integer(trimmed.to_owned()),
        FieldKind::Real => DbValue::Real(trimmed.to_owned()),
        FieldKind::Duration => DbValue::Duration(trimmed.to_owned()),
        FieldKind::Text | FieldKind::Blob | FieldKind::Any => DbValue::Text(text.to_owned()),
    }
}

#[derive(Clone, Debug)]
struct DraftField {
    view: FieldView,
    /// The stored value; `Null` for a new object without a default.
    stored: DbValue,
    stored_key: Option<ObjectKey>,
    initial: Input,
    input: Input,
    blurred: bool,
    /// The backend's issues, shown until the field is edited again.
    server: Option<Vec<FieldIssue>>,
}

impl DraftField {
    fn raw(&self) -> DbValue {
        match &self.input {
            Input::Reference(_) | Input::Choice(None) => DbValue::Null,
            Input::Bool(b) => b.map_or(DbValue::Null, |b| {
                DbValue::Integer(if b { "1" } else { "0" }.to_owned())
            }),
            Input::Choice(Some(v)) => self
                .view
                .rule
                .allowed
                .iter()
                .flatten()
                .find(|a| plain(&a.value) == Some(v.as_str()))
                .map_or_else(|| typed(self.view.kind, v), |a| a.value.clone()),
            Input::Text(s) => typed(self.view.kind, s),
        }
    }

    fn normalised(&self) -> Result<DbValue, Vec<FieldIssue>> {
        validate(&self.view.rule, &self.raw())
    }

    fn changed(&self) -> bool {
        if let Input::Reference(pick) = &self.input {
            return pick.as_ref().map(|p| &p.key) != self.stored_key.as_ref();
        }
        if input_eq(&self.input, &self.initial) {
            return false;
        }
        self.normalised()
            .map_or(true, |v| !same_value(&v, &self.stored))
    }

    fn local_issues(&self, new: bool) -> Vec<FieldIssue> {
        let changed = self.changed();
        if new && self.view.required_on_create && !changed {
            return vec![required_issue()];
        }
        if !changed {
            return Vec::new();
        }
        match &self.input {
            Input::Reference(None) if !self.view.rule.nullable => vec![required_issue()],
            Input::Reference(_) => Vec::new(),
            _ => self.normalised().err().unwrap_or_default(),
        }
    }

    fn shown_text(&self, input: &Input) -> String {
        match input {
            Input::Text(s) if s.is_empty() => text::NOT_SET.to_owned(),
            Input::Text(s) => s.clone(),
            Input::Bool(Some(true)) => text::YES.to_owned(),
            Input::Bool(Some(false)) => text::NO.to_owned(),
            Input::Bool(None) | Input::Choice(None) | Input::Reference(None) => {
                text::NOT_SET.to_owned()
            }
            Input::Choice(Some(v)) => self
                .view
                .rule
                .allowed
                .iter()
                .flatten()
                .find(|a| plain(&a.value) == Some(v.as_str()))
                .map_or_else(|| v.clone(), |a| a.label.clone()),
            Input::Reference(Some(p)) if p.label.is_empty() => text::REFERENCE_UNNAMED.to_owned(),
            Input::Reference(Some(p)) => p.label.clone(),
        }
    }
}

/// One field of a conflict, as the panel lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictField {
    pub column: String,
    pub display_name: String,
    /// What the draft has, as shown.
    pub yours: String,
    /// What the object has now, as shown.
    pub now: String,
}

/// How a draft differs from the object as it is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictDiff {
    /// Every changed field of the draft, with both values.
    pub fields: Vec<ConflictField>,
    /// Unedited fields whose stored value differs now.
    pub others_changed: usize,
}

/// What a rebase did.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Rebase {
    /// Display names of the edited fields that were removed because the
    /// field is gone, no longer editable, or no longer fits its editor.
    pub dropped: Vec<String>,
}

/// The edits to one object (or the fields of a new one).
#[derive(Clone, Debug)]
pub struct Draft {
    table: String,
    key: Option<ObjectKey>,
    version: Option<String>,
    new: bool,
    fields: Vec<DraftField>,
    /// Every stored value of the object the draft is based on (all fields).
    base: Vec<FieldValue>,
    attempted: bool,
}

impl Draft {
    fn build(table: &TableView, view: Option<&ObjectView>) -> Vec<DraftField> {
        let new = view.is_none();
        table
            .fields
            .iter()
            .filter(|f| has_editor(f, new))
            .map(|f| {
                let value = view.and_then(|v| v.fields.iter().find(|x| x.column == f.column));
                let initial = initial_input(f, value);
                let stored = match value {
                    Some(v) => v.value.clone(),
                    None => f.default_value.clone().unwrap_or(DbValue::Null),
                };
                let stored_key = match &initial {
                    Input::Reference(p) => p.as_ref().map(|p| p.key.clone()),
                    _ => None,
                };
                DraftField {
                    view: f.clone(),
                    stored,
                    stored_key,
                    input: initial.clone(),
                    initial,
                    blurred: false,
                    server: None,
                }
            })
            .collect()
    }

    /// A clean draft of the stored object `view` of `table`.
    #[must_use]
    pub fn for_object(table: &TableView, view: &ObjectView) -> Draft {
        Draft {
            table: table.name.clone(),
            key: Some(view.key.clone()),
            version: Some(view.version.clone()),
            new: false,
            fields: Self::build(table, Some(view)),
            base: view.fields.clone(),
            attempted: false,
        }
    }

    /// A draft of a new object of `table`: defaults prefilled.
    #[must_use]
    pub fn for_new(table: &TableView) -> Draft {
        Draft {
            table: table.name.clone(),
            key: None,
            version: None,
            new: true,
            fields: Self::build(table, None),
            base: Vec::new(),
            attempted: false,
        }
    }

    #[must_use]
    pub fn table(&self) -> &str {
        &self.table
    }

    /// The object's key; `None` for a new object.
    #[must_use]
    pub fn key(&self) -> Option<&ObjectKey> {
        self.key.as_ref()
    }

    /// The version the draft is based on; `None` for a new object.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    #[must_use]
    pub fn is_new(&self) -> bool {
        self.new
    }

    fn field(&self, column: &str) -> Option<&DraftField> {
        self.fields.iter().find(|f| f.view.column == column)
    }

    fn field_mut(&mut self, column: &str) -> Option<&mut DraftField> {
        self.fields.iter_mut().find(|f| f.view.column == column)
    }

    /// The columns that have an editor, in display order.
    pub fn columns(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(|f| f.view.column.as_str())
    }

    /// The field's definition.
    #[must_use]
    pub fn field_view(&self, column: &str) -> Option<&FieldView> {
        self.field(column).map(|f| &f.view)
    }

    /// The field's editor state; `None` when the field has no editor.
    #[must_use]
    pub fn input(&self, column: &str) -> Option<&Input> {
        self.field(column).map(|f| &f.input)
    }

    /// The field's editor text converted to the value it would store, when
    /// valid.
    #[must_use]
    pub fn value(&self, column: &str) -> Option<DbValue> {
        let f = self.field(column)?;
        match &f.input {
            Input::Reference(_) => None,
            _ => f.normalised().ok(),
        }
    }

    /// The field's editor differs from the stored value.
    #[must_use]
    pub fn changed(&self, column: &str) -> bool {
        self.field(column).is_some_and(DraftField::changed)
    }

    /// The changed columns, in display order.
    #[must_use]
    pub fn changed_columns(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|f| f.changed())
            .map(|f| f.view.column.as_str())
            .collect()
    }

    /// Any field is changed.
    #[must_use]
    pub fn dirty(&self) -> bool {
        self.fields.iter().any(DraftField::changed)
    }

    /// How many fields are changed.
    #[must_use]
    pub fn dirty_count(&self) -> usize {
        self.fields.iter().filter(|f| f.changed()).count()
    }

    /// The field's issues: the backend's while they stand, else what
    /// `validate` finds in a changed field, else "required" for a
    /// required-on-create field of a new object that was not filled in.
    #[must_use]
    pub fn issues(&self, column: &str) -> Vec<FieldIssue> {
        let Some(f) = self.field(column) else {
            return Vec::new();
        };
        match &f.server {
            Some(issues) => issues.clone(),
            None => f.local_issues(self.new),
        }
    }

    /// The issues are shown: the field lost focus, a save was attempted, or
    /// the backend reported them.
    #[must_use]
    pub fn shown(&self, column: &str) -> bool {
        self.field(column)
            .is_some_and(|f| f.blurred || self.attempted || f.server.is_some())
    }

    /// The issues to show now (empty until [`Draft::shown`]).
    #[must_use]
    pub fn shown_issues(&self, column: &str) -> Vec<FieldIssue> {
        if self.shown(column) {
            self.issues(column)
        } else {
            Vec::new()
        }
    }

    /// Every field with issues to show, as (column, display name, issues):
    /// the error summary.
    #[must_use]
    pub fn summary(&self) -> Vec<(String, String, Vec<FieldIssue>)> {
        self.fields
            .iter()
            .filter_map(|f| {
                let issues = self.shown_issues(&f.view.column);
                (!issues.is_empty())
                    .then(|| (f.view.column.clone(), f.view.display_name.clone(), issues))
            })
            .collect()
    }

    /// Nothing the local checks find blocks a save.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.fields
            .iter()
            .all(|f| f.local_issues(self.new).is_empty())
    }

    /// A save was attempted: every issue shows, and the backend's earlier
    /// issues are forgotten (the next answer replaces them).
    pub fn attempt(&mut self) {
        self.attempted = true;
        for f in &mut self.fields {
            f.server = None;
        }
    }

    /// Replaces the inline issues of the fields the backend named. Returns
    /// whether any field matched.
    pub fn set_server_issues(&mut self, errors: &[FieldError]) -> bool {
        let mut any = false;
        for e in errors {
            if let Some(f) = self.field_mut(&e.column) {
                f.server = Some(e.issues.clone());
                any = true;
            }
        }
        any
    }

    /// Sets a field from an editor. An input of the wrong kind for the
    /// field is ignored. The backend's issues for the field are forgotten.
    pub fn set(&mut self, column: &str, input: EditorInput) {
        let Some(f) = self.field_mut(column) else {
            return;
        };
        let next = match input {
            EditorInput::Text(s) => Input::Text(s),
            EditorInput::Bool(b) => Input::Bool(b),
            EditorInput::Choice(c) => Input::Choice(c),
        };
        if same_variant(&next, &f.initial) {
            f.input = next;
            f.server = None;
        }
    }

    /// Stages a reference (or `None` for NULL) in a foreign-key field.
    pub fn set_reference(&mut self, column: &str, pick: Option<Pick>) {
        if let Some(f) = self.field_mut(column) {
            if matches!(f.initial, Input::Reference(_)) {
                f.input = Input::Reference(pick);
                f.server = None;
                f.blurred = true;
            }
        }
    }

    /// The field lost focus: its issues show, and a valid duration takes its
    /// canonical form.
    pub fn blur(&mut self, column: &str) {
        let Some(f) = self.field_mut(column) else {
            return;
        };
        f.blurred = true;
        if f.view.kind == FieldKind::Duration {
            if let (Input::Text(_), Ok(DbValue::Duration(canonical))) = (&f.input, f.normalised()) {
                f.input = Input::Text(canonical);
            }
        }
    }

    /// Puts one field back to its stored value.
    pub fn revert(&mut self, column: &str) {
        if let Some(f) = self.field_mut(column) {
            f.input = f.initial.clone();
            f.blurred = false;
            f.server = None;
        }
    }

    /// Puts every field back.
    pub fn revert_all(&mut self) {
        for f in &mut self.fields {
            f.input = f.initial.clone();
            f.blurred = false;
            f.server = None;
        }
        self.attempted = false;
    }

    /// The save request with the changed fields only. Call it when
    /// [`Draft::can_save`]; a changed field that does not validate is sent as
    /// typed and the backend refuses it.
    #[must_use]
    pub fn to_request(&self, config_hash: &str) -> SaveRequest {
        let changes = self
            .fields
            .iter()
            .filter(|f| f.changed())
            .map(|f| FieldChange {
                column: f.view.column.clone(),
                input: match &f.input {
                    Input::Reference(p) => FieldInput::Reference {
                        key: p.as_ref().map(|p| p.key.clone()),
                    },
                    _ => FieldInput::Value {
                        value: f.normalised().unwrap_or_else(|_| f.raw()),
                    },
                },
            })
            .collect();
        SaveRequest {
            table: self.table.clone(),
            key: self.key.clone(),
            version: self.version.clone(),
            config_hash: config_hash.to_owned(),
            changes,
            links: Vec::new(),
        }
    }

    /// Lays the draft's edits over a fresh read: `table` as it is now and
    /// `view`, the object as it is now (`None` for a new object). Edited
    /// fields keep their edits; unedited ones take the new stored values.
    /// Edits to fields that are gone, no longer editable, or whose editor no
    /// longer fits are dropped and listed. The new version becomes the
    /// draft's base, so this is also "re-apply my changes" after a conflict.
    pub fn rebase(&mut self, table: &TableView, view: Option<&ObjectView>) -> Rebase {
        let mut fields = Self::build(table, view);
        let mut dropped = Vec::new();
        for old in std::mem::take(&mut self.fields) {
            if !old.changed() {
                continue;
            }
            let Some(new) = fields.iter_mut().find(|f| f.view.column == old.view.column) else {
                dropped.push(old.view.display_name);
                continue;
            };
            match coerce(&old.input, &new.initial) {
                Some(input) => {
                    new.input = input;
                    new.blurred = old.blurred;
                }
                None => dropped.push(old.view.display_name),
            }
        }
        self.fields = fields;
        if let Some(view) = view {
            self.version = Some(view.version.clone());
            self.base.clone_from(&view.fields);
        }
        Rebase { dropped }
    }

    /// How the draft differs from `current`, the object as it is now.
    #[must_use]
    pub fn conflict_diff(&self, current: &ObjectView) -> ConflictDiff {
        let mut fields = Vec::new();
        for f in self.fields.iter().filter(|f| f.changed()) {
            let now = current
                .fields
                .iter()
                .find(|v| v.column == f.view.column)
                .map_or_else(
                    || text::NOT_SET.to_owned(),
                    |v| f.shown_text(&initial_input(&f.view, Some(v))),
                );
            fields.push(ConflictField {
                column: f.view.column.clone(),
                display_name: f.view.display_name.clone(),
                yours: f.shown_text(&f.input),
                now,
            });
        }
        let others_changed = current
            .fields
            .iter()
            .filter(|now| !fields.iter().any(|c| c.column == now.column))
            .filter(|now| {
                self.base
                    .iter()
                    .find(|b| b.column == now.column)
                    .is_some_and(|b| {
                        b.value != now.value
                            || b.link.as_ref().and_then(|l| l.key.as_ref())
                                != now.link.as_ref().and_then(|l| l.key.as_ref())
                    })
            })
            .count();
        ConflictDiff {
            fields,
            others_changed,
        }
    }
}

/// An edit carried to a field whose editor may have changed kind.
fn coerce(old: &Input, new_initial: &Input) -> Option<Input> {
    match (old, new_initial) {
        (Input::Text(_), Input::Text(_))
        | (Input::Bool(_), Input::Bool(_))
        | (Input::Choice(_), Input::Choice(_))
        | (Input::Reference(_), Input::Reference(_)) => Some(old.clone()),
        (Input::Text(s), Input::Choice(_)) => {
            Some(Input::Choice((!s.is_empty()).then(|| s.clone())))
        }
        (Input::Choice(c), Input::Text(_)) => Some(Input::Text(c.clone().unwrap_or_default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use fm_dbmanager::config::{DurationUnit, FieldMode};
    use fm_dbmanager::model::TableView;
    use fm_dbmanager::rules::FieldRule;
    use fm_dbmanager::session::RelationCount;

    use super::*;

    fn rule(kind: FieldKind, nullable: bool) -> FieldRule {
        FieldRule {
            kind,
            nullable,
            unit: (kind == FieldKind::Duration).then_some(DurationUnit::Seconds),
            min: None,
            max: None,
            min_length: None,
            max_length: None,
            pattern: None,
            allowed: None,
            empty_is_null: false,
        }
    }

    fn field(column: &str, kind: FieldKind, nullable: bool) -> FieldView {
        FieldView {
            column: column.into(),
            display_name: column.to_uppercase(),
            help: None,
            mode: FieldMode::Editable,
            kind,
            declared_type: String::new(),
            rule: rule(kind, nullable),
            is_key: false,
            required_on_create: false,
            has_default: false,
            default_value: None,
            multiline: false,
            foreign_key: None,
        }
    }

    fn table(fields: Vec<FieldView>) -> TableView {
        TableView {
            name: "t".into(),
            display_name: "T".into(),
            description: None,
            object_name: None,
            mode: fm_dbmanager::config::TableMode::Editable,
            can_create: true,
            can_delete: false,
            searchable: false,
            has_triggers: false,
            fields,
            referenced_by: Vec::new(),
            many_to_many: Vec::new(),
        }
    }

    fn fv(column: &str, value: DbValue) -> FieldValue {
        FieldValue {
            column: column.into(),
            value,
            checked: None,
            link: None,
            problem: None,
        }
    }

    fn object(version: &str, fields: Vec<FieldValue>) -> ObjectView {
        ObjectView {
            table: "t".into(),
            key: ObjectKey::Rowid { value: "1".into() },
            label: "One".into(),
            version: version.into(),
            fields,
            referenced_by: Vec::<RelationCount>::new(),
            many_to_many: Vec::new(),
        }
    }

    fn sample() -> (TableView, ObjectView) {
        let t = table(vec![
            field("name", FieldKind::Text, false),
            field("pages", FieldKind::Integer, true),
            field("length", FieldKind::Duration, true),
        ]);
        let o = object(
            "v1",
            vec![
                fv("name", DbValue::Text("Ada".into())),
                fv("pages", DbValue::Integer("10".into())),
                fv("length", DbValue::Duration("1h30m".into())),
            ],
        );
        (t, o)
    }

    fn text_in(s: &str) -> EditorInput {
        EditorInput::Text(s.into())
    }

    #[test]
    fn canonical_duration_is_not_a_change() {
        let (t, o) = sample();
        let mut d = Draft::for_object(&t, &o);
        d.set("length", text_in("1h 30m"));
        assert!(!d.changed("length"));
        d.set("length", text_in("90m"));
        assert!(!d.changed("length"), "same duration, other spelling");
        d.set("length", text_in("2h"));
        assert!(d.changed("length"));
        d.blur("length");
        assert!(matches!(d.input("length"), Some(Input::Text(s)) if s == "2h"));
        d.set("length", text_in("1h 45m"));
        d.blur("length");
        assert!(matches!(d.input("length"), Some(Input::Text(s)) if s == "1h45m"));
        // An invalid entry keeps what was typed.
        d.set("length", text_in("soon"));
        d.blur("length");
        assert!(matches!(d.input("length"), Some(Input::Text(s)) if s == "soon"));
        assert!(!d.can_save());
    }

    #[test]
    fn empty_nullable_is_null() {
        let (t, o) = sample();
        let mut d = Draft::for_object(&t, &o);
        d.set("pages", text_in("  "));
        assert!(d.changed("pages"));
        assert_eq!(d.value("pages"), Some(DbValue::Null));
        assert_eq!(d.issues("pages").len(), 0);
        // A field that is not nullable says it is required.
        let mut d2 = Draft::for_object(&table(vec![field("pages", FieldKind::Integer, false)]), &o);
        d2.set("pages", text_in(""));
        assert_eq!(d2.issues("pages")[0].code, FieldIssueCode::Required);
        // Numbers are trimmed and normalised: +10 is the stored 10.
        d.set("pages", text_in(" +10 "));
        assert!(!d.changed("pages"));
    }

    #[test]
    fn text_is_not_trimmed() {
        let (t, o) = sample();
        let mut d = Draft::for_object(&t, &o);
        d.set("name", text_in("Ada "));
        assert!(d.changed("name"));
        assert_eq!(d.value("name"), Some(DbValue::Text("Ada ".into())));
        let request = d.to_request("h");
        assert_eq!(
            request.changes[0].input,
            FieldInput::Value {
                value: DbValue::Text("Ada ".into())
            }
        );
    }

    #[test]
    fn issues_shown_after_blur_or_save() {
        let (t, o) = sample();
        let mut d = Draft::for_object(&t, &o);
        d.set("pages", text_in("many"));
        assert_eq!(d.issues("pages")[0].code, FieldIssueCode::NotInteger);
        assert!(!d.shown("pages"));
        assert_eq!(d.shown_issues("pages").len(), 0);
        d.blur("pages");
        assert_eq!(d.shown_issues("pages").len(), 1);
        let mut d = Draft::for_object(&t, &o);
        d.set("pages", text_in("many"));
        d.attempt();
        assert!(d.shown("pages"));
        assert_eq!(d.summary().len(), 1);
    }

    #[test]
    fn revert_field_and_all() {
        let (t, o) = sample();
        let mut d = Draft::for_object(&t, &o);
        d.set("name", text_in("Bea"));
        d.set("pages", text_in("11"));
        assert_eq!(d.dirty_count(), 2);
        d.revert("name");
        assert_eq!(d.changed_columns(), ["pages"]);
        d.attempt();
        d.revert_all();
        assert!(!d.dirty());
        assert!(!d.shown("pages"));
    }

    #[test]
    fn request_has_only_changed_fields() {
        let (t, o) = sample();
        let mut d = Draft::for_object(&t, &o);
        d.set("pages", text_in("12"));
        d.set("length", text_in("1h30m"));
        let r = d.to_request("hash-1");
        assert_eq!(r.config_hash, "hash-1");
        assert_eq!(r.version.as_deref(), Some("v1"));
        assert_eq!(r.key, Some(o.key.clone()));
        assert_eq!(r.changes.len(), 1);
        assert_eq!(r.changes[0].column, "pages");
        assert_eq!(
            r.changes[0].input,
            FieldInput::Value {
                value: DbValue::Integer("12".into())
            }
        );
    }

    #[test]
    fn rebase_drops_gone_and_read_only_fields() {
        let (t, o) = sample();
        let mut d = Draft::for_object(&t, &o);
        d.set("name", text_in("Bea"));
        d.set("pages", text_in("11"));
        d.set("length", text_in("2h"));
        // pages becomes read-only, length is gone.
        let mut t2 = t.clone();
        t2.fields[1].mode = FieldMode::ReadOnly;
        t2.fields.remove(2);
        let r = d.rebase(&t2, Some(&o));
        assert_eq!(r.dropped, ["PAGES", "LENGTH"]);
        assert_eq!(d.changed_columns(), ["name"]);
        assert!(d.input("pages").is_none() && d.input("length").is_none());
        // An unedited field that vanished is not listed.
        let mut d = Draft::for_object(&t, &o);
        d.set("name", text_in("Bea"));
        assert_eq!(d.rebase(&t2, Some(&o)).dropped.len(), 0);
    }

    #[test]
    fn conflict_diff_yours_now_others() {
        let (table, stored) = sample();
        let mut draft = Draft::for_object(&table, &stored);
        draft.set("name", text_in("Bea"));
        let now = object(
            "v2",
            vec![
                fv("name", DbValue::Text("Cy".into())),
                fv("pages", DbValue::Integer("99".into())),
                fv("length", DbValue::Duration("1h30m".into())),
            ],
        );
        let diff = draft.conflict_diff(&now);
        assert_eq!(
            diff.fields,
            [ConflictField {
                column: "name".into(),
                display_name: "NAME".into(),
                yours: "Bea".into(),
                now: "Cy".into(),
            }]
        );
        assert_eq!(diff.others_changed, 1, "pages");
        // Re-applying keeps my edit over the new values and the new version.
        let rebased = draft.rebase(&table, Some(&now));
        assert_eq!(rebased.dropped.len(), 0);
        assert_eq!(draft.version(), Some("v2"));
        assert_eq!(draft.changed_columns(), ["name"]);
        assert!(matches!(draft.input("pages"), Some(Input::Text(text)) if text == "99"));
    }

    #[test]
    fn new_draft_prefills_and_requires() {
        let mut title = field("title", FieldKind::Text, false);
        title.required_on_create = true;
        let mut format = field("format", FieldKind::Text, false);
        format.default_value = Some(DbValue::Text("paperback".into()));
        let mut d = Draft::for_new(&table(vec![title, format]));
        assert!(!d.dirty());
        assert!(matches!(d.input("format"), Some(Input::Text(s)) if s == "paperback"));
        assert!(!d.can_save(), "title is missing");
        assert_eq!(d.issues("title")[0].code, FieldIssueCode::Required);
        d.set("title", text_in("T"));
        assert!(d.can_save());
        let r = d.to_request("h");
        assert!(r.key.is_none());
        assert_eq!(r.changes.len(), 1, "the default is not sent");
    }

    #[test]
    fn server_issues_stand_until_edited() {
        let (t, o) = sample();
        let mut d = Draft::for_object(&t, &o);
        d.set("name", text_in("Bea"));
        let issue = FieldIssue {
            code: FieldIssueCode::Unique,
            message: "taken".into(),
            limit: None,
        };
        assert!(d.set_server_issues(&[FieldError {
            column: "name".into(),
            issues: vec![issue.clone()],
        }]));
        assert_eq!(d.shown_issues("name"), vec![issue]);
        d.set("name", text_in("Bee"));
        assert_eq!(d.shown_issues("name").len(), 0);
    }
}
