use super::*;

const EXAMPLE: &str = include_str!("../../../../examples/dbmanager/example.toml");
const SCHEMA: &str = include_str!("../../../../examples/dbmanager/schema.sql");
const DOC: &str = include_str!("../../../../docs/dbmanager-config.md");

fn errors(r: &ConfigReport) -> Vec<&ConfigDiagnostic> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect()
}

fn pos(d: &ConfigDiagnostic) -> (u32, u32) {
    (d.line.unwrap(), d.col.unwrap())
}

/// The single error of `text`, as (line, col).
fn one_error(text: &str) -> (u32, u32) {
    let r = parse(text);
    let errs = errors(&r);
    assert_eq!(errs.len(), 1, "{:?}", r.diagnostics);
    pos(errs[0])
}

fn table<'a>(r: &'a ConfigReport, name: &str) -> &'a TableEntry {
    r.config
        .tables
        .iter()
        .find(|t| match t {
            TableEntry::Valid(t) => t.name == name,
            TableEntry::Rejected { name: n, .. } => n == name,
        })
        .unwrap_or_else(|| panic!("no table {name}"))
}

fn valid_table<'a>(r: &'a ConfigReport, name: &str) -> &'a TableConfig {
    match table(r, name) {
        TableEntry::Valid(t) => t,
        other @ TableEntry::Rejected { .. } => panic!("{other:?}"),
    }
}

fn field<'a>(t: &'a TableConfig, name: &str) -> &'a FieldEntry {
    t.fields
        .iter()
        .find(|f| match f {
            FieldEntry::Valid(f) => f.name == name,
            FieldEntry::Rejected { name: n, .. } => n == name,
        })
        .unwrap_or_else(|| panic!("no field {name}"))
}

fn valid_field<'a>(t: &'a TableConfig, name: &str) -> &'a FieldConfig {
    match field(t, name) {
        FieldEntry::Valid(f) => f,
        other @ FieldEntry::Rejected { .. } => panic!("{other:?}"),
    }
}

/// A config with one table `t` whose field `f` has the extra lines.
fn field_cfg(extra: &str) -> String {
    format!("version = 1\n[tables.t.fields.f]\n{extra}\n")
}

fn parse_field(extra: &str) -> ConfigReport {
    parse(&field_cfg(extra))
}

fn field_rejected(extra: &str) -> (u32, u32) {
    let r = parse_field(extra);
    let t = valid_table(&r, "t");
    assert!(
        matches!(field(t, "f"), FieldEntry::Rejected { .. }),
        "{extra}"
    );
    assert_eq!(r.fail_closed, None);
    pos(errors(&r)[0])
}

#[test]
fn example_parses_clean() {
    let r = parse(EXAMPLE);
    assert_eq!(r.diagnostics, vec![], "{:?}", r.diagnostics);
    assert_eq!(r.fail_closed, None);
    assert_eq!(r.config.defaults, Defaults::default());
    assert!(r
        .config
        .tables
        .iter()
        .all(|t| matches!(t, TableEntry::Valid(_))));
    assert_eq!(r.config.tables.len(), 8);
    assert!(r
        .config
        .many_to_many
        .iter()
        .all(|m| matches!(m, M2mEntry::Valid(_))));
    assert_eq!(r.config.many_to_many.len(), 2);

    let author = valid_table(&r, "author");
    assert_eq!(author.display_name.as_deref(), Some("Authors"));
    assert_eq!(author.object_name.as_deref(), Some("author"));
    assert_eq!(
        author.label.as_ref().unwrap().value.parts,
        vec![
            LabelPart::Column("last_name".into()),
            LabelPart::Literal(", ".into()),
            LabelPart::Column("first_name".into())
        ]
    );
    assert_eq!(author.order_by.len(), 2);
    assert!(author.allow_delete.as_ref().unwrap().value);
    assert_eq!(
        valid_field(author, "internal_note")
            .mode
            .as_ref()
            .unwrap()
            .value,
        FieldMode::Hidden
    );
    let book = valid_table(&r, "book");
    let audio = valid_field(book, "audio_length");
    assert_eq!(
        audio.field_type.as_ref().unwrap().value,
        FieldType::Duration
    );
    assert_eq!(audio.unit.as_ref().unwrap().value, DurationUnit::Seconds);
    assert_eq!(
        audio.min.as_ref().unwrap().value,
        Bound::Duration("1m".into())
    );
    assert_eq!(
        audio.max.as_ref().unwrap().value,
        Bound::Duration("100h".into())
    );
    assert_eq!(
        valid_field(book, "price").max.as_ref().unwrap().value,
        Bound::Float(1000.0)
    );
    assert_eq!(
        valid_field(book, "isbn").pattern.as_ref().unwrap().value,
        "97[89][0-9]{10}"
    );
    assert!(matches!(
        valid_field(book, "format").allowed.as_ref().unwrap().value,
        AllowedValues::Texts(ref v) if v.len() == 3 && v[0].1.as_deref() == Some("Hardcover")
    ));
    let TableEntry::Valid(hidden_log) = table(&r, "audit_log") else {
        panic!()
    };
    assert_eq!(hidden_log.mode.as_ref().unwrap().value, TableMode::Hidden);
    let M2mEntry::Valid(contribution) = &r.config.many_to_many[1] else {
        panic!()
    };
    assert_eq!(contribution.name, "contribution");
    assert_eq!(contribution.headings.len(), 2);
    assert_eq!(contribution.headings[0].side, "book");
    assert_eq!(contribution.headings[0].text, "Contributors");
}

#[test]
fn fail_closed_reason_only_when_fail_closed() {
    for text in [
        "",
        "version = 1\n",
        "version = 1\n[defaults]\ntable_mode = \"nope\"\n",
        "version = 1\n[tables.a]\nbogus = 1\n",
        "version = 1\nbogus = 1\n",
        EXAMPLE,
    ] {
        assert_eq!(parse(text).fail_closed, None, "{text}");
    }
    for text in [
        "version = 1\n[tables\n",
        "version = 2\n",
        "version = \"1\"\n",
    ] {
        let r = parse(text);
        assert!(r.fail_closed.is_some(), "{text}");
        assert_eq!(r.config, Config::fail_closed());
        assert_eq!(errors(&r).len(), 1);
    }
}

#[test]
fn object_name_key() {
    let r = parse("version = 1\n[tables.a]\nobject_name = \"author\"\n");
    assert_eq!(r.diagnostics, vec![]);
    assert_eq!(valid_table(&r, "a").object_name.as_deref(), Some("author"));
    assert_eq!(
        one_error("version = 1\n[tables.a]\nobject_name = \"  \"\n"),
        (3, 15)
    );
    assert_eq!(
        one_error("version = 1\n[tables.a]\nobject_name = \"\"\n"),
        (3, 15)
    );
    assert_eq!(
        one_error("version = 1\n[tables.a]\nobject_name = 3\n"),
        (3, 15)
    );
}

#[test]
fn schema_sql_runs() {
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("example.db")).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    let count = |t: &str| -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(count("publisher"), 2);
    assert_eq!(count("author"), 3);
    assert_eq!(count("book"), 5);
    assert_eq!(count("genre"), 4);
    assert!(count("book_genre") >= 4);
    assert_eq!(count("contribution"), 3);
    assert_eq!(count("member"), 3);
    assert_eq!(count("loan"), 4);
    assert_eq!(count("audit_log"), 2);
    let null_publisher: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM book WHERE publisher_id IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(null_publisher, 1);
    let audio: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM book WHERE audio_length IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(audio, 2);
    let null_member: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM loan WHERE member_id IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(null_member, 1);
    let translators: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM contribution WHERE role = 'translator' AND position = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(translators, 1);
    let violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
}

#[test]
fn empty_config_defaults_editable() {
    let r = parse("");
    assert_eq!(r.config, Config::default());
    assert_eq!(r.config.defaults.table_mode, TableMode::Editable);
    assert_eq!(r.config.defaults.field_mode, FieldMode::Editable);
    assert_eq!(r.config.defaults.relation_mode, RelationMode::Editable);
    assert!(r.config.tables.is_empty() && r.config.many_to_many.is_empty());
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(r.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(r.fail_closed, None);
}

#[test]
fn syntax_error_ignores_config() {
    let r = parse("version = 1\n[defaults]\ntable_mode = \"hidden\"\n[tables\n");
    assert_eq!(r.config, Config::fail_closed());
    assert_eq!(r.config.defaults.table_mode, TableMode::ReadOnly);
    assert_eq!(r.config.defaults.field_mode, FieldMode::ReadOnly);
    assert_eq!(r.config.defaults.relation_mode, RelationMode::ReadOnly);
    assert_eq!(errors(&r).len(), 1);
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(pos(&r.diagnostics[0]).0, 4);
    assert_eq!(r.fail_closed.as_deref(), Some("syntax error at line 4"));
}

#[test]
fn unsupported_version_ignores_config() {
    let r = parse("version = 2\n[tables.a]\nmode = \"hidden\"\n");
    assert_eq!(r.config, Config::fail_closed());
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(pos(&r.diagnostics[0]), (1, 11));
    assert_eq!(r.fail_closed.as_deref(), Some("unsupported version 2"));
}

#[test]
fn missing_version_warns() {
    let r = parse("[tables.a]\nmode = \"hidden\"\n");
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(r.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert!(r.diagnostics[0].message.contains("version"));
    assert_eq!(r.fail_closed, None);
    assert_eq!(
        valid_table(&r, "a").mode.as_ref().unwrap().value,
        TableMode::Hidden
    );
}

#[test]
fn invalid_defaults_fall_back() {
    let r = parse(
        "version = 1\n[defaults]\nfield_mode = \"create-only\"\n[tables.a]\nmode = \"hidden\"\n",
    );
    assert_eq!(r.config.defaults, Config::fail_closed().defaults);
    assert_eq!(errors(&r).len(), 1);
    assert_eq!(pos(errors(&r)[0]), (3, 14));
    assert_eq!(r.fail_closed, None);
    assert_eq!(
        valid_table(&r, "a").mode.as_ref().unwrap().value,
        TableMode::Hidden
    );
    // an unknown key in [defaults] is the same
    let r = parse("version = 1\n[defaults]\ntable_mod = \"hidden\"\n");
    assert_eq!(pos(errors(&r)[0]), (3, 1));
    assert_eq!(r.config.defaults.table_mode, TableMode::ReadOnly);
    // one valid, one invalid: all three narrow
    let r = parse("version = 1\n[defaults]\ntable_mode = \"hidden\"\nrelation_mode = 3\n");
    assert_eq!(r.config.defaults.table_mode, TableMode::ReadOnly);
    // a section of the wrong type narrows too
    let r = parse("version = 1\ntables = 1\n");
    assert_eq!(pos(errors(&r)[0]), (2, 10));
    assert_eq!(r.config.defaults.field_mode, FieldMode::ReadOnly);
    assert_eq!(r.fail_closed, None);
}

#[test]
fn explicit_flags_recorded() {
    let r = parse(
        "version = 1\n[tables.a]\nmode = \"read-only\"\nallow_create = false\nallow_delete = true\n[tables.b]\n",
    );
    let first = valid_table(&r, "a");
    let mode = first.mode.as_ref().unwrap();
    assert_eq!(
        (mode.value, mode.at),
        (TableMode::ReadOnly, At { line: 3, col: 1 })
    );
    let create = first.allow_create.as_ref().unwrap();
    assert_eq!((create.value, create.at), (false, At { line: 4, col: 1 }));
    assert!(first.allow_delete.as_ref().unwrap().value);
    assert_eq!(first.at, At { line: 2, col: 9 });
    let second = valid_table(&r, "b");
    assert!(
        second.mode.is_none() && second.allow_create.is_none() && second.allow_delete.is_none()
    );
    let r = parse_field("mode = \"create-only\"\nmin = 1\n");
    let f = valid_field(valid_table(&r, "t"), "f");
    assert_eq!(f.mode.as_ref().unwrap().value, FieldMode::CreateOnly);
    assert_eq!(f.min.as_ref().unwrap().at, At { line: 4, col: 1 });
    assert!(f.max.is_none() && f.nullable.is_none());
}

#[test]
fn unknown_key_rejects_entry_with_position() {
    let r = parse("version = 1\n[tables.a]\nmode = \"editable\"\n  colour = 1\n");
    assert!(
        matches!(table(&r, "a"), TableEntry::Rejected { hidden: false, at, .. } if *at == At { line: 2, col: 9 })
    );
    assert_eq!(errors(&r).len(), 1);
    assert_eq!(pos(errors(&r)[0]), (4, 3));
    assert_eq!(field_rejected("colour = 1"), (3, 1));
    assert_eq!(one_error("version = 1\nextra = 1\n"), (2, 1));
    // only the entry with the error is rejected
    let r = parse("version = 1\n[tables.a]\nbogus = 1\n[tables.b]\nmode = \"hidden\"\n");
    assert!(matches!(table(&r, "a"), TableEntry::Rejected { .. }));
    assert!(matches!(table(&r, "b"), TableEntry::Valid(_)));
    // a table entry that is not a table
    let r = parse("version = 1\n[tables]\na = 3\n");
    assert!(matches!(
        table(&r, "a"),
        TableEntry::Rejected { hidden: false, .. }
    ));
    assert_eq!(pos(errors(&r)[0]), (3, 5));
    // a table's error does not reject its valid sibling fields' diagnostics
    let r = parse("version = 1\n[tables.a]\nbogus = 1\n[tables.a.fields.x]\nbogus = 2\n");
    assert_eq!(errors(&r).len(), 2);
}

#[test]
fn bad_mode_rejects_entry() {
    let r = parse("version = 1\n[tables.a]\nmode = \"create-only\"\n");
    assert!(matches!(
        table(&r, "a"),
        TableEntry::Rejected { hidden: false, .. }
    ));
    assert_eq!(pos(errors(&r)[0]), (3, 8));
    assert_eq!(field_rejected("mode = \"frozen\""), (3, 8));
    let r = parse("version = 1\n[many_to_many.j]\nmode = \"create-only\"\n");
    assert!(matches!(
        r.config.many_to_many[0],
        M2mEntry::Rejected { .. }
    ));
    assert_eq!(pos(errors(&r)[0]), (3, 8));
}

#[test]
fn rejected_hidden_entry_stays_hidden() {
    let r = parse("version = 1\n[tables.a]\nmode = \"hidden\"\nbogus = 1\n");
    assert!(matches!(
        table(&r, "a"),
        TableEntry::Rejected { hidden: true, .. }
    ));
    // an invalid mode is not "hidden"
    let r = parse("version = 1\n[tables.a]\nmode = \"hiden\"\nbogus = 1\n");
    assert!(matches!(
        table(&r, "a"),
        TableEntry::Rejected { hidden: false, .. }
    ));
    // visible mode
    let r = parse("version = 1\n[tables.a]\nmode = \"editable\"\nbogus = 1\n");
    assert!(matches!(
        table(&r, "a"),
        TableEntry::Rejected { hidden: false, .. }
    ));
    let r = parse_field("mode = \"hidden\"\nmin = 5\nmax = 1\n");
    let t = valid_table(&r, "t");
    assert!(matches!(
        field(t, "f"),
        FieldEntry::Rejected { hidden: true, .. }
    ));
    let r = parse_field("mode = \"create-only\"\nmin = 5\nmax = 1\n");
    assert!(matches!(
        field(valid_table(&r, "t"), "f"),
        FieldEntry::Rejected { hidden: false, .. }
    ));
}

#[test]
fn min_greater_than_max() {
    assert_eq!(field_rejected("min = 5\nmax = 1"), (3, 1));
    assert_eq!(field_rejected("min = 5.5\nmax = 1"), (3, 1));
    assert_eq!(field_rejected("min = 5\nmax = 1.5"), (3, 1));
    assert_eq!(field_rejected("min = 1.5\nmax = 1.25"), (3, 1));
    assert_eq!(field_rejected("min = \"2h\"\nmax = \"90m\""), (3, 1));
    assert_eq!(field_rejected("min = 7\nmax = 3\n"), (3, 1));
    // equal is fine, and different forms are left to the schema step
    for ok in [
        "min = 1\nmax = 1",
        "min = \"1h\"\nmax = \"60m\"",
        "min = \"2h\"\nmax = 1",
        "min = 90\nmax = \"1m\"",
        "min = \"-1d\"\nmax = \"1d\"",
    ] {
        let r = parse_field(ok);
        assert_eq!(r.diagnostics, vec![], "{ok}");
    }
    // non-finite and out-of-range bounds
    assert_eq!(field_rejected("min = nan"), (3, 7));
    assert_eq!(field_rejected("max = 99999999999999999999"), (3, 7));
    assert_eq!(field_rejected("min = true"), (3, 7));
}

#[test]
fn length_bounds_invalid() {
    assert_eq!(field_rejected("min_length = 5\nmax_length = 1"), (3, 1));
    assert_eq!(field_rejected("min_length = -1"), (3, 14));
    assert_eq!(field_rejected("max_length = -1"), (3, 14));
    assert_eq!(field_rejected("max_length = 4294967296"), (3, 14));
    assert_eq!(field_rejected("max_length = 1.5"), (3, 14));
    assert_eq!(field_rejected("max_length = \"3\""), (3, 14));
    let r = parse_field("min_length = 0\nmax_length = 4294967295");
    assert_eq!(r.diagnostics, vec![]);
    let f = valid_field(valid_table(&r, "t"), "f");
    assert_eq!(f.max_length.as_ref().unwrap().value, u32::MAX);
}

#[test]
fn duration_bound_grammar() {
    for (text, expected) in [
        ("\"1h30m\"", Bound::Duration("1h30m".into())),
        ("\"1h 30m\"", Bound::Duration("1h 30m".into())),
        ("\"-2d\"", Bound::Duration("-2d".into())),
        ("\"250ms\"", Bound::Duration("250ms".into())),
        (
            "\"1d 2h 3m 4s 5ms\"",
            Bound::Duration("1d 2h 3m 4s 5ms".into()),
        ),
        ("90", Bound::Int(90)),
    ] {
        let r = parse_field(&format!("type = \"duration\"\nmin = {text}"));
        assert_eq!(r.diagnostics, vec![], "{text}");
        let f = valid_field(valid_table(&r, "t"), "f");
        assert_eq!(f.min.as_ref().unwrap().value, expected, "{text}");
    }
    for bad in [
        "\"1m1h\"",
        "\"1h  30m\"",
        "\" 1h\"",
        "\"1h \"",
        "\"1.5h\"",
        "\"1H\"",
        "\"h\"",
        "\"\"",
        "\"-\"",
        "\"1h30\"",
        "\"1h1h\"",
        "\"1x\"",
        "\"--1h\"",
        "\"1 h\"",
    ] {
        assert_eq!(
            field_rejected(&format!("type = \"duration\"\nmin = {bad}")),
            (4, 7),
            "{bad}"
        );
    }
}

#[test]
fn duration_parser_values() {
    use super::duration::{parse, Parsed};
    assert_eq!(parse("1h30m"), Ok(Parsed::Millis(5_400_000)));
    assert_eq!(parse("-2d"), Ok(Parsed::Millis(-172_800_000)));
    assert_eq!(parse("250ms"), Ok(Parsed::Millis(250)));
    assert_eq!(parse("-90"), Ok(Parsed::Count(-90)));
    assert!(parse("99999999999999999999999999999999999999999d").is_err());
}

#[test]
fn unit_requires_duration_type() {
    assert_eq!(field_rejected("unit = \"seconds\""), (3, 1));
    assert_eq!(
        field_rejected("type = \"integer\"\nunit = \"seconds\""),
        (4, 1)
    );
    assert_eq!(
        field_rejected("unit = \"fortnights\"\ntype = \"duration\""),
        (3, 8)
    );
    let r = parse_field("type = \"duration\"\nunit = \"days\"");
    assert_eq!(r.diagnostics, vec![]);
    assert_eq!(field_rejected("type = \"money\""), (3, 8));
    // an invalid `type` is one error, not two
    let r = parse_field("type = \"money\"\nunit = \"days\"");
    assert_eq!(errors(&r).len(), 1);
}

#[test]
fn pattern_must_compile() {
    assert_eq!(field_rejected("pattern = '[a-'"), (3, 11));
    assert_eq!(field_rejected("pattern = '(x'"), (3, 11));
    // unanchored alternation is anchored as a whole
    let r = parse_field("pattern = 'a|b'");
    assert_eq!(r.diagnostics, vec![]);
    assert_eq!(field_rejected("pattern = 3"), (3, 11));
}

#[test]
fn allowed_forms() {
    let texts = |extra: &str| {
        let r = parse_field(extra);
        assert_eq!(r.diagnostics, vec![], "{extra}");
        valid_field(valid_table(&r, "t"), "f")
            .allowed
            .as_ref()
            .unwrap()
            .value
            .clone()
    };
    assert_eq!(
        texts("allowed = [\"a\", \"b\"]"),
        AllowedValues::Texts(vec![("a".into(), None), ("b".into(), None)])
    );
    assert_eq!(
        texts("allowed = [1, 2]"),
        AllowedValues::Integers(vec![(1, None), (2, None)])
    );
    assert_eq!(
        texts("allowed = [{ value = \"a\", label = \"Alpha\" }, { value = \"b\" }]"),
        AllowedValues::Texts(vec![("a".into(), Some("Alpha".into())), ("b".into(), None)])
    );
    assert_eq!(
        texts("allowed = [{ value = 1, label = \"One\" }, 2]"),
        AllowedValues::Integers(vec![(1, Some("One".into())), (2, None)])
    );
}

#[test]
fn allowed_invalid() {
    assert_eq!(field_rejected("allowed = []"), (3, 11));
    assert_eq!(field_rejected("allowed = [\"a\", 1]"), (3, 11));
    assert_eq!(field_rejected("allowed = [\"a\", \"a\"]"), (3, 17));
    assert_eq!(field_rejected("allowed = [1, 2, 1]"), (3, 18));
    assert_eq!(
        field_rejected("allowed = [{ value = \"a\" }, { value = \"a\", label = \"x\" }]"),
        (3, 39)
    );
    assert_eq!(
        field_rejected("allowed = [{ value = \"a\", colour = 1 }]"),
        (3, 27)
    );
    assert_eq!(field_rejected("allowed = [{ label = \"a\" }]"), (3, 12));
    assert_eq!(field_rejected("allowed = [1.5]"), (3, 12));
    assert_eq!(
        field_rejected("allowed = [{ value = \"a\", label = 1 }]"),
        (3, 35)
    );
    assert_eq!(field_rejected("allowed = \"a\""), (3, 11));
    assert_eq!(
        field_rejected("allowed = [{ value = \"a\", label = \"  \" }]"),
        (3, 35)
    );
    // mixed via inline tables
    assert_eq!(
        field_rejected("allowed = [{ value = 1 }, { value = \"a\" }]"),
        (3, 11)
    );
}

#[test]
fn label_template_parsing() {
    use LabelPart::{Column, Literal};
    let parts = |l: &str| {
        let r = parse(&format!("version = 1\n[tables.a]\nlabel = '{l}'\n"));
        assert_eq!(r.diagnostics, vec![], "{l}");
        valid_table(&r, "a")
            .label
            .as_ref()
            .unwrap()
            .value
            .parts
            .clone()
    };
    assert_eq!(
        parts("{a} {b}"),
        vec![Column("a".into()), Literal(" ".into()), Column("b".into())]
    );
    assert_eq!(
        parts("{{literal}} {a}"),
        vec![Literal("{literal} ".into()), Column("a".into())]
    );
    assert_eq!(parts("plain"), vec![Literal("plain".into())]);
    for bad in ["{a", "a}", "{}", "{ }", "{a{b}", "}}}", "{{{"] {
        let r = parse(&format!("version = 1\n[tables.a]\nlabel = '{bad}'\n"));
        assert!(
            matches!(table(&r, "a"), TableEntry::Rejected { .. }),
            "{bad}"
        );
        assert_eq!(pos(errors(&r)[0]), (3, 9), "{bad}");
    }
}

#[test]
fn order_by_forms() {
    let r = parse(
        "version = 1\n[tables.a]\norder_by = [\"x\", { column = \"y\", descending = true }, { column = \"z\" }]\nfield_order = [\"b\", \"a\"]\n",
    );
    assert_eq!(r.diagnostics, vec![]);
    let a = valid_table(&r, "a");
    let cols: Vec<_> = a
        .order_by
        .iter()
        .map(|o| (o.column.as_str(), o.descending))
        .collect();
    assert_eq!(cols, vec![("x", false), ("y", true), ("z", false)]);
    assert_eq!(a.order_by[0].at, At { line: 3, col: 13 });
    assert_eq!(a.field_order.len(), 2);
    assert_eq!(a.field_order[1].value, "a");
    for (text, at) in [
        ("order_by = [1]", (3, 13)),
        ("order_by = \"x\"", (3, 12)),
        ("order_by = [{ descending = true }]", (3, 13)),
        ("order_by = [{ column = \"x\", sort = 1 }]", (3, 29)),
        ("order_by = [{ column = \"x\", descending = 1 }]", (3, 42)),
        ("order_by = [\"\"]", (3, 13)),
        ("field_order = [1]", (3, 16)),
        ("field_order = \"a\"", (3, 15)),
    ] {
        let t = format!("version = 1\n[tables.a]\n{text}\n");
        let r = parse(&t);
        assert!(
            matches!(table(&r, "a"), TableEntry::Rejected { .. }),
            "{text}"
        );
        assert_eq!(pos(errors(&r)[0]), at, "{text}");
    }
}

#[test]
fn m2m_entry_rules() {
    let r = parse(
        "version = 1\n[many_to_many.j]\nenabled = false\nmode = \"read-only\"\ncolumns = [\"a\", \"b\"]\nheadings = { x = \"X\", y = \"Y\" }\n",
    );
    assert_eq!(r.diagnostics, vec![]);
    let M2mEntry::Valid(m) = &r.config.many_to_many[0] else {
        panic!()
    };
    assert!(!m.enabled.as_ref().unwrap().value);
    assert_eq!(m.mode.as_ref().unwrap().value, RelationMode::ReadOnly);
    assert_eq!(
        m.columns.as_ref().unwrap().value,
        ["a".to_string(), "b".to_string()]
    );
    assert_eq!(m.headings.len(), 2);
    assert_eq!(m.at, At { line: 2, col: 15 });
    for (text, at) in [
        ("columns = [\"a\"]", (3, 11)),
        ("columns = [\"a\", \"b\", \"c\"]", (3, 11)),
        ("columns = [\"a\", \"a\"]", (3, 11)),
        ("columns = [\"a\", 1]", (3, 17)),
        ("columns = \"a\"", (3, 11)),
        ("headings = { x = \"\" }", (3, 18)),
        ("headings = { x = 1 }", (3, 18)),
        ("headings = \"x\"", (3, 12)),
        ("colour = 1", (3, 1)),
        ("enabled = 1", (3, 11)),
    ] {
        let r = parse(&format!("version = 1\n[many_to_many.j]\n{text}\n"));
        assert!(
            matches!(&r.config.many_to_many[0], M2mEntry::Rejected { name, .. } if name == "j"),
            "{text}"
        );
        assert_eq!(pos(errors(&r)[0]), at, "{text}");
    }
    let r = parse("version = 1\n[many_to_many]\nj = 1\n");
    assert!(matches!(
        r.config.many_to_many[0],
        M2mEntry::Rejected { .. }
    ));
}

#[test]
fn bom_accepted() {
    let r = parse(&format!("\u{feff}{EXAMPLE}"));
    assert_eq!(r.diagnostics, vec![]);
    let r = parse("\u{feff}version = 1\n[tables.a]\nbogus = 1\n");
    assert_eq!(pos(errors(&r)[0]), (3, 1));
    let r = parse("\u{feff}version = 1\nbogus = 1\n");
    assert_eq!(pos(errors(&r)[0]), (2, 1));
}

#[test]
fn columns_count_characters() {
    // a multi-byte character before the key still gives a character column
    let r = parse("version = 1\n[tables.a]\ndisplay_name = \"ééé\"\nobject_name = 3\n");
    assert_eq!(pos(errors(&r)[0]), (4, 15));
    let r = parse("version = 1\n[tables.\"é\"]\nbogus = 1\n");
    assert_eq!(pos(errors(&r)[0]), (3, 1));
    let r = parse("version = 1\n[tables.\"é\"]\n");
    assert!(matches!(table(&r, "é"), TableEntry::Valid(_)));
    assert_eq!(valid_table(&r, "é").at, At { line: 2, col: 9 });
}

#[test]
fn diagnostics_are_sorted() {
    let r = parse("version = 1\n[tables.b]\nz = 1\n[tables.a]\ny = 1\nx = 1\n");
    let order: Vec<_> = r.diagnostics.iter().map(pos).collect();
    assert_eq!(order, vec![(3, 1), (5, 1), (6, 1)]);
}

#[test]
fn doc_contains_example() {
    assert!(
        DOC.contains(EXAMPLE),
        "docs/dbmanager-config.md is missing the example verbatim"
    );
}

const STARTER: &str = include_str!("../../../../examples/dbmanager/starter.toml");

#[test]
fn starter_is_inert() {
    let r = parse(STARTER);
    assert_eq!(r.diagnostics, vec![]);
    assert_eq!(r.config, Config::default());
    assert_eq!(r.fail_closed, None);
}

#[test]
fn pickable_mode() {
    let r = parse("version = 1\n[tables.a]\nmode = \"pickable\"\n");
    assert_eq!(r.diagnostics, vec![]);
    assert_eq!(
        valid_table(&r, "a").mode.as_ref().unwrap().value,
        TableMode::Pickable
    );
    let r = parse(
        "version = 1\n[defaults]\ntable_mode = \"pickable\"\n[tables.a]\nmode = \"hidden\"\n",
    );
    assert_eq!(errors(&r).len(), 1);
    assert_eq!(pos(errors(&r)[0]), (3, 14));
    assert_eq!(r.config.defaults, Config::fail_closed().defaults);
    assert_eq!(r.fail_closed, None);
    assert_eq!(
        valid_table(&r, "a").mode.as_ref().unwrap().value,
        TableMode::Hidden
    );
}

#[test]
fn rejected_table_keeps_field_entries() {
    let r = parse(
        "version = 1\n[tables.author]\nmode = \"editable\"\nbogus = 1\n[tables.author.fields.internal_note]\nmode = \"hidden\"\n",
    );
    assert_eq!(errors(&r).len(), 1);
    assert_eq!(pos(errors(&r)[0]), (4, 1));
    let TableEntry::Rejected { fields, hidden, .. } = table(&r, "author") else {
        panic!("{:?}", table(&r, "author"));
    };
    assert!(!hidden);
    assert_eq!(fields.len(), 1);
    let FieldEntry::Valid(f) = &fields[0] else {
        panic!("{fields:?}");
    };
    assert_eq!(f.name, "internal_note");
    assert_eq!(f.mode.as_ref().unwrap().value, FieldMode::Hidden);
}

#[test]
fn unknown_top_level_key_narrows_defaults() {
    let r =
        parse("version = 1\n[table.secrets]\nmode = \"hidden\"\n[tables.a]\nmode = \"hidden\"\n");
    assert_eq!(errors(&r).len(), 1);
    assert_eq!(pos(errors(&r)[0]), (2, 2));
    assert_eq!(r.config.defaults, Config::fail_closed().defaults);
    assert_eq!(r.fail_closed, None);
    assert_eq!(
        valid_table(&r, "a").mode.as_ref().unwrap().value,
        TableMode::Hidden
    );
}
