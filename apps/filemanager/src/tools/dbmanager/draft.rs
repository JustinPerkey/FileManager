//! The draft: the edits to one open (or new) object, held by the model. Pure
//! logic with no GPUI types, so it is unit-tested alone.
//!
//! Every editable field has an editor state ([`Input`]). Whether a field is
//! **changed** is decided by converting the editor text to a [`DbValue`],
//! normalising it with [`validate`] and comparing it with the stored value, so
//! typing `1h 30m` over a stored `1h30m` is not a change. Validation is
//! `fm_dbmanager::rules::validate` only; nothing here validates a second way.

use fm_dbmanager::config::{FieldMode, RelationMode};
use fm_dbmanager::model::{FieldView, TableView};
use fm_dbmanager::rules::{validate, FieldError, FieldIssue, FieldIssueCode, FieldKind};
use fm_dbmanager::session::{
    FieldChange, FieldInput, FieldValue, LinkAdd, LinkChange, LinkError, LinkRemove, LinkUpdate,
    M2mOption, ObjectKey, ObjectView, SaveRequest,
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
        shown_text(&self.view, input)
    }

    /// Sets the field from an editor. An input of the wrong kind for the
    /// field is ignored. The backend's issues for the field are forgotten.
    fn apply(&mut self, input: EditorInput) {
        let next = match input {
            EditorInput::Text(s) => Input::Text(s),
            EditorInput::Bool(b) => Input::Bool(b),
            EditorInput::Choice(c) => Input::Choice(c),
        };
        if same_variant(&next, &self.initial) {
            self.input = next;
            self.server = None;
        }
    }

    /// A duration that is valid takes its canonical form.
    fn canonicalise(&mut self) {
        if self.view.kind == FieldKind::Duration {
            if let (Input::Text(_), Ok(DbValue::Duration(canonical))) =
                (&self.input, self.normalised())
            {
                self.input = Input::Text(canonical);
            }
        }
    }

    /// The change this field sends: its value as `validate` normalises it,
    /// or as typed when it does not validate.
    fn to_change(&self) -> FieldChange {
        FieldChange {
            column: self.view.column.clone(),
            input: match &self.input {
                Input::Reference(p) => FieldInput::Reference {
                    key: p.as_ref().map(|p| p.key.clone()),
                },
                _ => FieldInput::Value {
                    value: self.normalised().unwrap_or_else(|_| self.raw()),
                },
            },
        }
    }
}

/// `input` of the field `view`, as the user reads it.
fn shown_text(view: &FieldView, input: &Input) -> String {
    match input {
        Input::Text(s) if s.is_empty() => text::NOT_SET.to_owned(),
        Input::Text(s) => s.clone(),
        Input::Bool(Some(true)) => text::YES.to_owned(),
        Input::Bool(Some(false)) => text::NO.to_owned(),
        Input::Bool(None) | Input::Choice(None) | Input::Reference(None) => {
            text::NOT_SET.to_owned()
        }
        Input::Choice(Some(v)) => view
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

/// The editors of `views`: those that can be edited now. `values` are the
/// stored values of an existing object (or link); `None` is a new one, which
/// starts at the known defaults and also edits create-only fields.
fn build_fields(views: &[FieldView], values: Option<&[FieldValue]>) -> Vec<DraftField> {
    let new = values.is_none();
    views
        .iter()
        .filter(|f| has_editor(f, new))
        .map(|f| {
            let value = values.and_then(|v| v.iter().find(|x| x.column == f.column));
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
    links: StagedLinks,
}

impl Draft {
    fn build(table: &TableView, view: Option<&ObjectView>) -> Vec<DraftField> {
        build_fields(&table.fields, view.map(|v| v.fields.as_slice()))
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
            links: StagedLinks::default(),
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
            links: StagedLinks::default(),
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

    /// Any field is changed, or any link change is staged.
    #[must_use]
    pub fn dirty(&self) -> bool {
        self.fields.iter().any(DraftField::changed) || self.links.count() > 0
    }

    /// How many fields are changed plus how many link changes are staged.
    #[must_use]
    pub fn dirty_count(&self) -> usize {
        self.fields.iter().filter(|f| f.changed()).count() + self.links.count()
    }

    /// The staged many-to-many link changes.
    #[must_use]
    pub fn links(&self) -> &StagedLinks {
        &self.links
    }

    /// The staged many-to-many link changes, for the staging methods.
    pub fn links_mut(&mut self) -> &mut StagedLinks {
        &mut self.links
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
            && self.links.can_save()
    }

    /// A save was attempted: every issue shows, and the backend's earlier
    /// issues are forgotten (the next answer replaces them).
    pub fn attempt(&mut self) {
        self.attempted = true;
        for f in &mut self.fields {
            f.server = None;
        }
        self.links.attempt();
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
        if let Some(f) = self.field_mut(column) {
            f.apply(input);
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
        f.canonicalise();
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
        self.links.clear();
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
            .map(DraftField::to_change)
            .collect();
        SaveRequest {
            table: self.table.clone(),
            key: self.key.clone(),
            version: self.version.clone(),
            config_hash: config_hash.to_owned(),
            changes,
            links: self.links.to_changes(),
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
        dropped.extend(self.links.restage(table));
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

// ---- staged many-to-many links ----

/// Which change a staged link is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkStage {
    /// A new link ("Will be added").
    Add,
    /// New values for an existing link's fields ("Edited").
    Update,
    /// An existing link to delete ("Will be removed").
    Remove,
}

/// The stored link a staged change is based on.
#[derive(Clone, Debug)]
struct LinkBase {
    version: String,
    values: Vec<FieldValue>,
}

/// One staged change to one link, keyed by relation and the other object's
/// key. Its link-field editors are the same as a field's.
#[derive(Clone, Debug)]
pub struct StagedLink {
    relation: String,
    other: ObjectKey,
    /// The other object's label as read when it was staged.
    label: String,
    /// The existing link (`None`: an add).
    base: Option<LinkBase>,
    /// An existing link marked for removal.
    removed: bool,
    views: Vec<FieldView>,
    fields: Vec<DraftField>,
    attempted: bool,
    /// The backend's issues on the link itself.
    server: Vec<FieldIssue>,
}

impl StagedLink {
    /// A new link, link fields prefilled from their defaults.
    fn add(relation: &str, other: &ObjectKey, label: &str, views: &[FieldView]) -> StagedLink {
        StagedLink {
            relation: relation.to_owned(),
            other: other.clone(),
            label: label.to_owned(),
            base: None,
            removed: false,
            views: views.to_vec(),
            fields: build_fields(views, None),
            attempted: false,
            server: Vec::new(),
        }
    }

    /// A change to an existing link read as `version` with `values`.
    fn existing(
        relation: &str,
        other: &ObjectKey,
        label: &str,
        views: &[FieldView],
        version: &str,
        values: Vec<FieldValue>,
    ) -> StagedLink {
        StagedLink {
            relation: relation.to_owned(),
            other: other.clone(),
            label: label.to_owned(),
            fields: build_fields(views, Some(&values)),
            base: Some(LinkBase {
                version: version.to_owned(),
                values,
            }),
            removed: false,
            views: views.to_vec(),
            attempted: false,
            server: Vec::new(),
        }
    }

    /// A change to the existing link `option` (which must be linked).
    fn from_option(relation: &str, option: &M2mOption, views: &[FieldView]) -> Option<StagedLink> {
        let version = option.link_version.as_deref()?;
        Some(StagedLink::existing(
            relation,
            &option.key,
            &option.label,
            views,
            version,
            option.link_values.clone().unwrap_or_default(),
        ))
    }

    #[must_use]
    pub fn relation(&self) -> &str {
        &self.relation
    }

    #[must_use]
    pub fn other(&self) -> &ObjectKey {
        &self.other
    }

    /// The other object's label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The `link_version` the change is based on; `None` for an add.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.base.as_ref().map(|b| b.version.as_str())
    }

    /// Which change this is.
    #[must_use]
    pub fn stage(&self) -> LinkStage {
        match (&self.base, self.removed) {
            (None, _) => LinkStage::Add,
            (Some(_), true) => LinkStage::Remove,
            (Some(_), false) => LinkStage::Update,
        }
    }

    fn field(&self, column: &str) -> Option<&DraftField> {
        self.fields.iter().find(|f| f.view.column == column)
    }

    fn field_mut(&mut self, column: &str) -> Option<&mut DraftField> {
        self.fields.iter_mut().find(|f| f.view.column == column)
    }

    /// The link fields that have an editor now, in order. A create-only
    /// field has one on an add only.
    pub fn columns(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(|f| f.view.column.as_str())
    }

    /// The link field's editor state.
    #[must_use]
    pub fn input(&self, column: &str) -> Option<&Input> {
        self.field(column).map(|f| &f.input)
    }

    /// The link field's editor differs from its stored (or default) value.
    #[must_use]
    pub fn changed(&self, column: &str) -> bool {
        self.field(column).is_some_and(DraftField::changed)
    }

    fn any_changed(&self) -> bool {
        self.fields.iter().any(DraftField::changed)
    }

    /// An update that changes nothing: it is not a staged change.
    fn is_noop(&self) -> bool {
        self.stage() == LinkStage::Update && !self.any_changed()
    }

    /// The link field's issues (see [`Draft::issues`]).
    #[must_use]
    pub fn issues(&self, column: &str) -> Vec<FieldIssue> {
        let Some(f) = self.field(column) else {
            return Vec::new();
        };
        match &f.server {
            Some(issues) => issues.clone(),
            None => f.local_issues(self.base.is_none()),
        }
    }

    fn field_shown(&self, f: &DraftField) -> bool {
        f.blurred || self.attempted || f.server.is_some()
    }

    /// The issues to show now for a link field.
    #[must_use]
    pub fn shown_issues(&self, column: &str) -> Vec<FieldIssue> {
        match self.field(column) {
            Some(f) if self.field_shown(f) => self.issues(column),
            _ => Vec::new(),
        }
    }

    /// The backend's issues on the link itself (the other object is gone, or
    /// the link is not allowed).
    #[must_use]
    pub fn link_issues(&self) -> &[FieldIssue] {
        &self.server
    }

    /// Nothing the local checks find blocks saving this change.
    fn can_save(&self) -> bool {
        match self.stage() {
            LinkStage::Remove => true,
            LinkStage::Update if !self.any_changed() => true,
            _ => self
                .fields
                .iter()
                .all(|f| f.local_issues(self.base.is_none()).is_empty()),
        }
    }

    /// For a staged removal: each link field of the stored link as (display
    /// name, value as shown), for "Will be removed with its fields".
    #[must_use]
    pub fn removal_values(&self) -> Vec<(String, String)> {
        let Some(base) = &self.base else {
            return Vec::new();
        };
        if self.stage() != LinkStage::Remove {
            return Vec::new();
        }
        self.views
            .iter()
            .map(|v| {
                let value = base.values.iter().find(|x| x.column == v.column);
                (
                    v.display_name.clone(),
                    shown_text(v, &initial_input(v, value)),
                )
            })
            .collect()
    }

    /// The change this staged link sends, as a one-link [`LinkChange`].
    fn change(&self) -> LinkChange {
        let changed: Vec<FieldChange> = self
            .fields
            .iter()
            .filter(|f| f.changed())
            .map(DraftField::to_change)
            .collect();
        let mut out = LinkChange {
            relation: self.relation.clone(),
            add: Vec::new(),
            update: Vec::new(),
            remove: Vec::new(),
        };
        match (&self.base, self.removed) {
            (None, _) => out.add.push(LinkAdd {
                other: self.other.clone(),
                fields: changed,
            }),
            (Some(base), true) => out.remove.push(LinkRemove {
                other: self.other.clone(),
                version: Some(base.version.clone()),
            }),
            (Some(base), false) => out.update.push(LinkUpdate {
                other: self.other.clone(),
                version: base.version.clone(),
                fields: changed,
            }),
        }
        out
    }

    /// Copies the user's inputs from `from` into this link's editors, by
    /// column: only the changed ones, or every one.
    fn carry(&mut self, from: &StagedLink, only_changed: bool) {
        for old in &from.fields {
            if only_changed && !old.changed() {
                continue;
            }
            if let Some(new) = self.field_mut(&old.view.column) {
                if let Some(input) = coerce(&old.input, &new.initial) {
                    new.input = input;
                    new.blurred = old.blurred;
                }
            }
        }
    }
}

/// What one link's issues are, for the error summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkIssues {
    pub relation: String,
    pub other: ObjectKey,
    /// The other object's label.
    pub label: String,
    /// Link fields with issues: (column, display name, issues).
    pub fields: Vec<(String, String, Vec<FieldIssue>)>,
    /// Issues on the link itself.
    pub issues: Vec<FieldIssue>,
}

/// The staged changes to the many-to-many links of one object (or of a new
/// one), nothing written until Save. Each link appears once.
#[derive(Clone, Debug, Default)]
pub struct StagedLinks {
    items: Vec<StagedLink>,
}

impl StagedLinks {
    fn position(&self, relation: &str, other: &ObjectKey) -> Option<usize> {
        self.items
            .iter()
            .position(|l| l.relation == relation && &l.other == other)
    }

    /// The staged change to this link.
    #[must_use]
    pub fn get(&self, relation: &str, other: &ObjectKey) -> Option<&StagedLink> {
        self.position(relation, other).map(|i| &self.items[i])
    }

    /// Every staged change, in the order they were staged.
    pub fn iter(&self) -> impl Iterator<Item = &StagedLink> {
        self.items.iter()
    }

    /// The staged changes of one relation.
    pub fn for_relation<'a>(
        &'a self,
        relation: &'a str,
    ) -> impl Iterator<Item = &'a StagedLink> + 'a {
        self.items.iter().filter(move |l| l.relation == relation)
    }

    /// How many changes are staged.
    #[must_use]
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// How many links of `relation` are staged to be added, and to be
    /// removed.
    #[must_use]
    pub fn counts(&self, relation: &str) -> (usize, usize) {
        let of = |stage| {
            self.for_relation(relation)
                .filter(|l| l.stage() == stage)
                .count()
        };
        (of(LinkStage::Add), of(LinkStage::Remove))
    }

    /// Forgets every staged change.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Ticks or unticks `option`'s row. A new link is staged as an add
    /// prefilled from the link fields' defaults; an existing one as a
    /// removal. Toggling a staged add or removal again undoes it, keeping
    /// the link as it was (its values and version are those of the stored
    /// link); a removal of a link whose fields were edited goes back to the
    /// edit.
    pub fn toggle(&mut self, relation: &str, option: &M2mOption, views: &[FieldView]) {
        if self.flip(relation, &option.key) {
            return;
        }
        if option.linked {
            if let Some(mut link) = StagedLink::from_option(relation, option, views) {
                link.removed = true;
                self.items.push(link);
            }
        } else {
            self.items
                .push(StagedLink::add(relation, &option.key, &option.label, views));
        }
    }

    /// Toggles an already staged change (the row may be off the page): an
    /// add is dropped; a removal and an edit swap. Returns whether the link
    /// was staged.
    pub fn flip(&mut self, relation: &str, other: &ObjectKey) -> bool {
        let Some(i) = self.position(relation, other) else {
            return false;
        };
        if self.items[i].base.is_none() {
            self.items.remove(i);
        } else {
            let link = &mut self.items[i];
            link.removed = !link.removed;
            if link.is_noop() {
                self.items.remove(i);
            }
        }
        true
    }

    /// Edits a link field of `option`'s row. An existing link's edit stages
    /// an update with its `link_version`; a field with no editor on this row
    /// (create-only on an existing link), or a row staged for removal, is
    /// ignored. Editing an unlinked row that is not staged does nothing.
    pub fn edit(
        &mut self,
        relation: &str,
        option: &M2mOption,
        views: &[FieldView],
        column: &str,
        input: EditorInput,
    ) {
        let at = if let Some(i) = self.position(relation, &option.key) {
            i
        } else {
            if !option.linked {
                return;
            }
            let Some(link) = StagedLink::from_option(relation, option, views) else {
                return;
            };
            self.items.push(link);
            self.items.len() - 1
        };
        let link = &mut self.items[at];
        if !link.removed {
            if let Some(f) = link.field_mut(column) {
                f.apply(input);
            }
        }
        if link.is_noop() {
            self.items.remove(at);
        }
    }

    /// A link field lost focus: its issues show, and a valid duration takes
    /// its canonical form.
    pub fn blur(&mut self, relation: &str, other: &ObjectKey, column: &str) {
        if let Some(i) = self.position(relation, other) {
            if let Some(f) = self.items[i].field_mut(column) {
                f.blurred = true;
                f.canonicalise();
            }
        }
    }

    /// Drops the staged change to this link ("Undo"), returning it.
    pub fn undo(&mut self, relation: &str, other: &ObjectKey) -> Option<StagedLink> {
        let i = self.position(relation, other)?;
        Some(self.items.remove(i))
    }

    /// Stages `link`, replacing any change to the same link.
    pub fn insert(&mut self, link: StagedLink) {
        self.undo(&link.relation, &link.other);
        self.items.push(link);
    }

    /// Nothing the local checks find blocks a save.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.items.iter().all(StagedLink::can_save)
    }

    /// A save was attempted: every issue shows, and the backend's earlier
    /// issues are forgotten.
    pub fn attempt(&mut self) {
        for l in &mut self.items {
            l.attempted = true;
            l.server.clear();
            for f in &mut l.fields {
                f.server = None;
            }
        }
    }

    /// Attaches the backend's link errors to their rows. Returns whether any
    /// matched a staged link.
    pub fn set_errors(&mut self, errors: &[LinkError]) -> bool {
        let mut any = false;
        for e in errors {
            let Some(i) = self.position(&e.relation, &e.other) else {
                continue;
            };
            let link = &mut self.items[i];
            for fe in &e.fields {
                if let Some(f) = link.field_mut(&fe.column) {
                    f.server = Some(fe.issues.clone());
                }
            }
            link.server.clone_from(&e.issues);
            any = true;
        }
        any
    }

    /// Every staged link with issues to show: the error summary's link rows.
    #[must_use]
    pub fn summary(&self) -> Vec<LinkIssues> {
        self.items
            .iter()
            .filter_map(|l| {
                let fields: Vec<_> = l
                    .fields
                    .iter()
                    .filter_map(|f| {
                        let issues = l.shown_issues(&f.view.column);
                        (!issues.is_empty())
                            .then(|| (f.view.column.clone(), f.view.display_name.clone(), issues))
                    })
                    .collect();
                (!fields.is_empty() || !l.server.is_empty()).then(|| LinkIssues {
                    relation: l.relation.clone(),
                    other: l.other.clone(),
                    label: l.label.clone(),
                    fields,
                    issues: l.server.clone(),
                })
            })
            .collect()
    }

    /// The staged changes as the save sends them: one [`LinkChange`] per
    /// relation, in the order the relations were first staged.
    #[must_use]
    pub fn to_changes(&self) -> Vec<LinkChange> {
        let mut out: Vec<LinkChange> = Vec::new();
        for link in &self.items {
            let one = link.change();
            match out.iter_mut().find(|c| c.relation == one.relation) {
                Some(c) => {
                    c.add.extend(one.add);
                    c.update.extend(one.update);
                    c.remove.extend(one.remove);
                }
                None => out.push(one),
            }
        }
        out
    }

    /// Lays the staged changes over `table` as it is now: a relation that is
    /// gone or read-only drops its changes (listed by heading), and an edit
    /// to a link field that is gone, no longer editable or no longer fits its
    /// editor is dropped (listed by field name).
    fn restage(&mut self, table: &TableView) -> Vec<String> {
        let mut dropped: Vec<String> = Vec::new();
        let mut note = |name: &str| {
            if !dropped.iter().any(|d| d == name) {
                dropped.push(name.to_owned());
            }
        };
        let mut kept = Vec::new();
        for mut link in std::mem::take(&mut self.items) {
            let side = table
                .many_to_many
                .iter()
                .find(|m| m.relation == link.relation && m.mode != RelationMode::ReadOnly);
            let Some(side) = side else {
                let heading = table
                    .many_to_many
                    .iter()
                    .find(|m| m.relation == link.relation)
                    .map_or(link.relation.as_str(), |m| m.heading.as_str());
                note(heading);
                continue;
            };
            let mut fresh = StagedLink {
                views: side.link_fields.clone(),
                fields: build_fields(
                    &side.link_fields,
                    link.base.as_ref().map(|b| b.values.as_slice()),
                ),
                ..link.clone()
            };
            for old in link.fields.drain(..).filter(DraftField::changed) {
                match fresh.field_mut(&old.view.column) {
                    Some(new) => match coerce(&old.input, &new.initial) {
                        Some(input) => {
                            new.input = input;
                            new.blurred = old.blurred;
                        }
                        None => note(&old.view.display_name),
                    },
                    None => note(&old.view.display_name),
                }
            }
            if !fresh.is_noop() {
                kept.push(fresh);
            }
        }
        self.items = kept;
        dropped
    }
}

/// What a link-conflict rebase decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebaseOutcome {
    /// The change is staged again (see [`Rebased::staged`]).
    Staged,
    /// A staged add found the link already there, and the user had no link
    /// field values to apply.
    AlreadyLinked,
    /// A staged removal found the link already gone.
    AlreadyGone,
    /// A staged edit found the link already holding those values.
    AlreadyApplied,
}

/// The result of laying a set-aside link change over the link as it is now.
#[derive(Clone, Debug)]
pub struct Rebased {
    /// The change to stage; `None` when there is nothing left to do.
    pub staged: Option<StagedLink>,
    pub outcome: RebaseOutcome,
}

/// Re-bases a staged link change on `now`, the link's other-side option as
/// read again (`None`: the option is gone), keeping the user's link-field
/// values:
///
/// | Staged | The link now is | Result |
/// |---|---|---|
/// | add | linked | an update of the user's values against the fresh version (no changed fields: nothing) |
/// | update | not linked | an add with the user's values |
/// | update | linked, new version | an update against the fresh version |
/// | remove | not linked | nothing |
/// | remove | linked, new version | a remove with the fresh version |
#[must_use]
pub fn rebase_link(staged: &StagedLink, now: Option<&M2mOption>) -> Rebased {
    let linked = now.filter(|o| o.linked && o.link_version.is_some());
    let views = &staged.views;
    let done = |staged: Option<StagedLink>, outcome| Rebased { staged, outcome };
    match (staged.stage(), linked) {
        (LinkStage::Add, None) => done(Some(staged.clone()), RebaseOutcome::Staged),
        (LinkStage::Add, Some(option)) => {
            let Some(mut next) = StagedLink::from_option(&staged.relation, option, views) else {
                return done(None, RebaseOutcome::AlreadyLinked);
            };
            next.attempted = staged.attempted;
            next.carry(staged, true);
            if next.is_noop() {
                done(None, RebaseOutcome::AlreadyLinked)
            } else {
                done(Some(next), RebaseOutcome::Staged)
            }
        }
        (LinkStage::Update, None) => {
            let mut next = StagedLink::add(&staged.relation, &staged.other, &staged.label, views);
            next.attempted = staged.attempted;
            next.carry(staged, false);
            done(Some(next), RebaseOutcome::Staged)
        }
        (LinkStage::Update, Some(option)) => {
            let Some(mut next) = StagedLink::from_option(&staged.relation, option, views) else {
                return done(None, RebaseOutcome::AlreadyApplied);
            };
            next.attempted = staged.attempted;
            next.carry(staged, true);
            if next.is_noop() {
                done(None, RebaseOutcome::AlreadyApplied)
            } else {
                done(Some(next), RebaseOutcome::Staged)
            }
        }
        (LinkStage::Remove, None) => done(None, RebaseOutcome::AlreadyGone),
        (LinkStage::Remove, Some(option)) => {
            let Some(mut next) = StagedLink::from_option(&staged.relation, option, views) else {
                return done(None, RebaseOutcome::AlreadyGone);
            };
            next.carry(staged, true);
            next.removed = true;
            done(Some(next), RebaseOutcome::Staged)
        }
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

    // ---- staged links ----

    fn link_views() -> Vec<FieldView> {
        let mut role = field("role", FieldKind::Text, false);
        role.has_default = true;
        role.default_value = Some(DbValue::Text("editor".into()));
        let position = field("position", FieldKind::Integer, true);
        let mut note = field("note", FieldKind::Text, false);
        note.mode = FieldMode::CreateOnly;
        note.required_on_create = true;
        vec![role, position, note]
    }

    fn okey(n: i64) -> ObjectKey {
        ObjectKey::Rowid {
            value: n.to_string(),
        }
    }

    fn unlinked(n: i64) -> M2mOption {
        M2mOption {
            key: okey(n),
            label: format!("Other {n}"),
            linked: false,
            link_values: None,
            link_version: None,
        }
    }

    fn linked(n: i64, version: &str, role: &str, position: Option<&str>) -> M2mOption {
        M2mOption {
            key: okey(n),
            label: format!("Other {n}"),
            linked: true,
            link_values: Some(vec![
                fv("role", DbValue::Text(role.into())),
                fv(
                    "position",
                    position.map_or(DbValue::Null, |p| DbValue::Integer(p.into())),
                ),
            ]),
            link_version: Some(version.into()),
        }
    }

    const REL: &str = "m2m:j";

    #[test]
    fn toggle_twice_undoes_exactly() {
        let views = link_views();
        let mut l = StagedLinks::default();
        let free = unlinked(1);
        l.toggle(REL, &free, &views);
        assert_eq!(l.get(REL, &okey(1)).unwrap().stage(), LinkStage::Add);
        l.toggle(REL, &free, &views);
        assert_eq!(l.count(), 0);
        assert_eq!(l.to_changes().len(), 0);

        let held = linked(2, "v1", "translator", Some("1"));
        l.toggle(REL, &held, &views);
        let staged = l.get(REL, &okey(2)).unwrap();
        assert_eq!(staged.stage(), LinkStage::Remove);
        assert_eq!(staged.version(), Some("v1"));
        assert_eq!(
            staged.removal_values(),
            vec![
                ("ROLE".to_owned(), "translator".to_owned()),
                ("POSITION".to_owned(), "1".to_owned()),
                ("NOTE".to_owned(), "Not set".to_owned()),
            ]
        );
        let change = &l.to_changes()[0];
        assert_eq!(change.remove[0].version.as_deref(), Some("v1"));
        l.toggle(REL, &held, &views);
        assert_eq!(l.count(), 0, "back to the stored link, nothing staged");

        // An edited link that is unticked and ticked again keeps its edit.
        l.edit(REL, &held, &views, "position", text_in("5"));
        l.toggle(REL, &held, &views);
        assert_eq!(l.get(REL, &okey(2)).unwrap().stage(), LinkStage::Remove);
        l.toggle(REL, &held, &views);
        let back = l.get(REL, &okey(2)).unwrap();
        assert_eq!(back.stage(), LinkStage::Update);
        assert!(back.changed("position"));
        assert_eq!(back.version(), Some("v1"));
    }

    #[test]
    fn add_prefills_defaults() {
        let views = link_views();
        let mut l = StagedLinks::default();
        l.toggle(REL, &unlinked(1), &views);
        let staged = l.get(REL, &okey(1)).unwrap();
        assert!(matches!(staged.input("role"), Some(Input::Text(s)) if s == "editor"));
        l.edit(REL, &unlinked(1), &views, "note", text_in("n"));
        let add = &l.to_changes()[0].add[0];
        assert_eq!(add.fields.len(), 1, "the default is not sent: {add:?}");
        l.edit(REL, &unlinked(1), &views, "position", text_in("3"));
        let columns: Vec<_> = l.to_changes()[0].add[0]
            .fields
            .iter()
            .map(|f| f.column.clone())
            .collect();
        assert_eq!(columns, vec!["position", "note"]);
    }

    #[test]
    fn update_carries_link_version() {
        let views = link_views();
        let mut l = StagedLinks::default();
        let held = linked(2, "v7", "editor", None);
        l.edit(REL, &held, &views, "position", text_in("2"));
        assert_eq!(l.get(REL, &okey(2)).unwrap().stage(), LinkStage::Update);
        let change = &l.to_changes()[0];
        assert_eq!(change.update[0].version, "v7");
        assert_eq!(change.update[0].fields.len(), 1);
        // Typing the stored value back is not a change.
        l.edit(REL, &held, &views, "position", text_in(""));
        assert_eq!(l.count(), 0);
        // An unlinked row that is not staged cannot be edited.
        l.edit(REL, &unlinked(9), &views, "position", text_in("2"));
        assert_eq!(l.count(), 0);
    }

    #[test]
    fn create_only_link_field_editable_only_on_add() {
        let views = link_views();
        let mut l = StagedLinks::default();
        l.toggle(REL, &unlinked(1), &views);
        assert!(l.get(REL, &okey(1)).unwrap().columns().any(|c| c == "note"));
        let mut l2 = StagedLinks::default();
        let held = linked(2, "v1", "editor", None);
        l2.edit(REL, &held, &views, "note", text_in("x"));
        assert_eq!(l2.count(), 0, "no editor for it on an existing link");
        l2.edit(REL, &held, &views, "position", text_in("1"));
        let staged = l2.get(REL, &okey(2)).unwrap();
        assert!(!staged.columns().any(|c| c == "note"));
    }

    #[test]
    fn link_field_validation() {
        let views = link_views();
        let mut l = StagedLinks::default();
        l.toggle(REL, &unlinked(1), &views);
        // The required-on-create link field is missing.
        assert!(!l.can_save());
        let staged = l.get(REL, &okey(1)).unwrap();
        assert_eq!(staged.issues("note")[0].code, FieldIssueCode::Required);
        assert!(staged.shown_issues("note").is_empty(), "not shown yet");
        l.attempt();
        assert_eq!(
            l.get(REL, &okey(1)).unwrap().shown_issues("note")[0].code,
            FieldIssueCode::Required
        );
        assert_eq!(l.summary().len(), 1);
        l.edit(REL, &unlinked(1), &views, "note", text_in("n"));
        assert!(l.can_save());
        l.edit(REL, &unlinked(1), &views, "position", text_in("many"));
        assert!(!l.can_save());
        assert_eq!(
            l.get(REL, &okey(1)).unwrap().issues("position")[0].code,
            FieldIssueCode::NotInteger
        );
        // A removal needs no valid fields.
        let mut r = StagedLinks::default();
        r.toggle(REL, &linked(2, "v1", "editor", None), &views);
        assert!(r.can_save());
    }

    #[test]
    fn link_errors_attach_to_their_link() {
        let views = link_views();
        let mut l = StagedLinks::default();
        l.toggle(REL, &unlinked(1), &views);
        l.edit(REL, &unlinked(1), &views, "note", text_in("n"));
        let issue = |code| FieldIssue {
            code,
            message: "m".into(),
            limit: None,
        };
        assert!(l.set_errors(&[LinkError {
            relation: REL.into(),
            other: okey(1),
            fields: vec![FieldError {
                column: "position".into(),
                issues: vec![issue(FieldIssueCode::AboveMax)],
            }],
            issues: vec![issue(FieldIssueCode::NotAllowed)],
        }]));
        let staged = l.get(REL, &okey(1)).unwrap();
        assert_eq!(staged.shown_issues("position").len(), 1);
        assert_eq!(staged.link_issues().len(), 1);
        assert!(!l.set_errors(&[LinkError {
            relation: REL.into(),
            other: okey(5),
            fields: Vec::new(),
            issues: Vec::new(),
        }]));
    }

    #[test]
    fn rebase_add_now_linked() {
        let views = link_views();
        let mut l = StagedLinks::default();
        l.toggle(REL, &unlinked(1), &views);
        l.edit(REL, &unlinked(1), &views, "position", text_in("4"));
        let staged = l.get(REL, &okey(1)).unwrap().clone();
        let now = linked(1, "v9", "illustrator", None);
        let r = rebase_link(&staged, Some(&now));
        assert_eq!(r.outcome, RebaseOutcome::Staged);
        let next = r.staged.unwrap();
        assert_eq!(next.stage(), LinkStage::Update);
        assert_eq!(next.version(), Some("v9"));
        assert!(next.changed("position"));
        assert!(!next.changed("role"), "the fresh role stands");
        // No link-field values: nothing to apply.
        let mut bare = StagedLinks::default();
        bare.toggle(REL, &unlinked(1), &views);
        let r = rebase_link(bare.get(REL, &okey(1)).unwrap(), Some(&now));
        assert!(r.staged.is_none());
        assert_eq!(r.outcome, RebaseOutcome::AlreadyLinked);
    }

    #[test]
    fn rebase_update_now_unlinked() {
        let views = link_views();
        let mut l = StagedLinks::default();
        let held = linked(2, "v1", "translator", Some("1"));
        l.edit(REL, &held, &views, "position", text_in("8"));
        let staged = l.get(REL, &okey(2)).unwrap().clone();
        for now in [None, Some(unlinked(2))] {
            let r = rebase_link(&staged, now.as_ref());
            let next = r.staged.expect("an add");
            assert_eq!(next.stage(), LinkStage::Add);
            assert_eq!(next.version(), None);
            assert!(matches!(next.input("position"), Some(Input::Text(s)) if s == "8"));
            assert!(
                matches!(next.input("role"), Some(Input::Text(s)) if s == "translator"),
                "the user's whole values, not the defaults"
            );
        }
    }

    #[test]
    fn rebase_update_new_version() {
        let views = link_views();
        let mut l = StagedLinks::default();
        l.edit(
            REL,
            &linked(2, "v1", "editor", None),
            &views,
            "position",
            text_in("8"),
        );
        let staged = l.get(REL, &okey(2)).unwrap().clone();
        let now = linked(2, "v2", "translator", None);
        let next = rebase_link(&staged, Some(&now)).staged.unwrap();
        assert_eq!(next.stage(), LinkStage::Update);
        assert_eq!(next.version(), Some("v2"));
        assert!(next.changed("position"));
        assert!(matches!(next.input("role"), Some(Input::Text(s)) if s == "translator"));
        // The other program already made the same edit.
        let same = linked(2, "v3", "editor", Some("8"));
        let r = rebase_link(&staged, Some(&same));
        assert!(r.staged.is_none());
        assert_eq!(r.outcome, RebaseOutcome::AlreadyApplied);
    }

    #[test]
    fn rebase_remove_now_unlinked() {
        let views = link_views();
        let mut l = StagedLinks::default();
        l.toggle(REL, &linked(2, "v1", "editor", None), &views);
        let staged = l.get(REL, &okey(2)).unwrap().clone();
        for now in [None, Some(unlinked(2))] {
            let r = rebase_link(&staged, now.as_ref());
            assert!(r.staged.is_none());
            assert_eq!(r.outcome, RebaseOutcome::AlreadyGone);
        }
    }

    #[test]
    fn rebase_remove_new_version() {
        let views = link_views();
        let mut l = StagedLinks::default();
        l.toggle(REL, &linked(2, "v1", "editor", None), &views);
        let staged = l.get(REL, &okey(2)).unwrap().clone();
        let now = linked(2, "v5", "translator", Some("2"));
        let next = rebase_link(&staged, Some(&now)).staged.unwrap();
        assert_eq!(next.stage(), LinkStage::Remove);
        assert_eq!(next.version(), Some("v5"));
        let change = &StagedLinks { items: vec![next] }.to_changes()[0];
        assert_eq!(change.remove[0].version.as_deref(), Some("v5"));
    }

    #[test]
    fn draft_counts_and_reverts_staged_links() {
        let (t, o) = sample();
        let mut d = Draft::for_object(&t, &o);
        assert!(!d.dirty());
        d.links_mut().toggle(REL, &unlinked(1), &link_views());
        assert!(d.dirty());
        assert_eq!(d.dirty_count(), 1);
        d.set("name", text_in("Bea"));
        assert_eq!(d.dirty_count(), 2);
        let request = d.to_request("h");
        assert_eq!(request.links.len(), 1);
        d.revert_all();
        assert!(!d.dirty());
    }
}
