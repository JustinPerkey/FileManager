use super::save::{baks, codes, config_with, dur, int, one, raw, reference, request, text};
use super::*;
use crate::rules::FieldIssueCode;
use crate::session::{
    FieldChange, LinkAdd, LinkChange, LinkError, LinkRemove, LinkUpdate, SaveRequest,
};

const CONTRIB: &str = "m2m:contribution";
const GENRE: &str = "m2m:book_genre";

fn change(column: &str, input: FieldInput) -> FieldChange {
    FieldChange {
        column: column.into(),
        input,
    }
}

fn add(other: ObjectKey, fields: Vec<(&str, FieldInput)>) -> LinkAdd {
    LinkAdd {
        other,
        fields: fields.into_iter().map(|(c, i)| change(c, i)).collect(),
    }
}

fn lc(
    relation: &str,
    add: Vec<LinkAdd>,
    update: Vec<LinkUpdate>,
    remove: Vec<LinkRemove>,
) -> LinkChange {
    LinkChange {
        relation: relation.into(),
        add,
        update,
        remove,
    }
}

fn rm(other: ObjectKey, version: Option<String>) -> LinkRemove {
    LinkRemove { other, version }
}

fn upd(other: ObjectKey, version: String, fields: Vec<(&str, FieldInput)>) -> LinkUpdate {
    LinkUpdate {
        other,
        version,
        fields: fields.into_iter().map(|(c, i)| change(c, i)).collect(),
    }
}

/// An update of `book` (or other table) `id` that changes only links.
fn links_req(s: &Session, table: &str, id: i64, links: Vec<LinkChange>) -> SaveRequest {
    let mut r = request(s, table, Some(rk(id)), vec![]);
    r.links = links;
    r
}

fn link_errors(e: SessionError) -> Vec<LinkError> {
    match e {
        SessionError::Validation { links, .. } => links,
        other => panic!("expected Validation, got {other:?}"),
    }
}

fn conflict_link(e: SessionError) -> crate::session::LinkRef {
    match e {
        SessionError::Conflict { link, .. } => link.expect("a link conflict"),
        other => panic!("expected Conflict, got {other:?}"),
    }
}

fn options(s: &Session, table: &str, key: &ObjectKey, relation: &str) -> M2mPage {
    s.m2m_options(table, Some(key), relation, &mq(None, 0, 100))
        .unwrap()
}

fn option_for<'a>(p: &'a M2mPage, key: &ObjectKey) -> &'a M2mOption {
    p.items.iter().find(|i| &i.key == key).unwrap()
}

fn link_value(o: &M2mOption, column: &str) -> DbValue {
    o.link_values
        .as_ref()
        .unwrap()
        .iter()
        .find(|f| f.column == column)
        .unwrap()
        .value
        .clone()
}

fn snapshot(fx: &Fixture) -> (String, String, String) {
    let c = raw(fx);
    let dump = |sql: &str| -> String {
        let mut st = c.prepare(sql).unwrap();
        let n = st.column_count();
        let rows = st
            .query_map([], |r| {
                (0..n)
                    .map(|i| {
                        r.get::<_, rusqlite::types::Value>(i)
                            .map(|v| format!("{v:?}"))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap();
        format!("{:?}", rows.map(Result::unwrap).collect::<Vec<_>>())
    };
    (
        dump("SELECT * FROM book ORDER BY id"),
        dump("SELECT * FROM book_genre ORDER BY 1, 2"),
        dump("SELECT * FROM contribution ORDER BY 1, 2"),
    )
}

// ---- adding ----

#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "short names for request, session and page"
)]
fn link_add_with_fields() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = links_req(
        &s,
        "book",
        1,
        vec![lc(
            CONTRIB,
            vec![add(
                rk(1),
                vec![("role", text("translator")), ("position", int(1))],
            )],
            vec![],
            vec![],
        )],
    );
    let saved = s.save(&r).unwrap();
    assert!(saved.backup_created.is_some());
    let p = options(&s, "book", &rk(1), CONTRIB);
    let o = option_for(&p, &rk(1));
    assert!(o.linked);
    assert_eq!(link_value(o, "role"), DbValue::Text("translator".into()));
    assert_eq!(link_value(o, "position"), DbValue::Integer("1".into()));
    // the author's side shows the same link
    let q = options(&s, "author", &rk(1), CONTRIB);
    let o = option_for(&q, &rk(1));
    assert!(o.linked);
    assert_eq!(link_value(o, "role"), DbValue::Text("translator".into()));
    assert_eq!(
        saved
            .object
            .many_to_many
            .iter()
            .find(|r| r.relation == CONTRIB)
            .unwrap()
            .count,
        2
    );
}

#[test]
fn link_add_uses_defaults() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = links_req(
        &s,
        "book",
        4,
        vec![lc(CONTRIB, vec![add(rk(2), vec![])], vec![], vec![])],
    );
    s.save(&r).unwrap();
    assert_eq!(
        one::<String>(
            &fx,
            "SELECT role FROM contribution WHERE book_id = 4 AND author_id = 2"
        ),
        "editor"
    );
}

#[test]
fn link_add_validation_collects() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let before = snapshot(&fx);
    let r = links_req(
        &s,
        "book",
        4,
        vec![lc(
            CONTRIB,
            vec![add(
                rk(2),
                vec![("role", text("author")), ("position", int(0))],
            )],
            vec![],
            vec![],
        )],
    );
    let errs = link_errors(s.save(&r).unwrap_err());
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].relation, CONTRIB);
    assert_eq!(errs[0].other, rk(2));
    assert_eq!(codes(&errs[0].fields, "role"), [FieldIssueCode::NotAllowed]);
    assert_eq!(
        codes(&errs[0].fields, "position"),
        [FieldIssueCode::BelowMin]
    );
    assert_eq!(snapshot(&fx), before);
    // an unknown field and a relation column are unknownField
    let r = links_req(
        &s,
        "book",
        4,
        vec![lc(
            CONTRIB,
            vec![add(rk(2), vec![("book_id", int(1)), ("nope", int(1))])],
            vec![],
            vec![],
        )],
    );
    let errs = link_errors(s.save(&r).unwrap_err());
    assert_eq!(
        codes(&errs[0].fields, "book_id"),
        [FieldIssueCode::UnknownField]
    );
    assert_eq!(
        codes(&errs[0].fields, "nope"),
        [FieldIssueCode::UnknownField]
    );
}

#[test]
fn link_add_existing_conflicts() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let before = snapshot(&fx);
    // book 2 / author 3 exists
    let r = links_req(
        &s,
        "book",
        2,
        vec![lc(CONTRIB, vec![add(rk(3), vec![])], vec![], vec![])],
    );
    let link = conflict_link(s.save(&r).unwrap_err());
    assert_eq!((link.relation.as_str(), link.other), (CONTRIB, rk(3)));
    // book 2 / genre fiction exists
    let r = links_req(
        &s,
        "book",
        2,
        vec![lc(GENRE, vec![add(pk("fiction"), vec![])], vec![], vec![])],
    );
    let link = conflict_link(s.save(&r).unwrap_err());
    assert_eq!((link.relation.as_str(), link.other), (GENRE, pk("fiction")));
    assert_eq!(snapshot(&fx), before);
}

#[test]
fn link_add_other_missing() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let before = snapshot(&fx);
    let r = links_req(
        &s,
        "book",
        4,
        vec![lc(CONTRIB, vec![add(rk(999), vec![])], vec![], vec![])],
    );
    let errs = link_errors(s.save(&r).unwrap_err());
    assert_eq!(errs[0].other, rk(999));
    assert_eq!(errs[0].issues[0].code, FieldIssueCode::ReferenceMissing);
    assert_eq!(snapshot(&fx), before);
}

// ---- editing and removing ----

#[test]
fn link_update_fields() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let p = options(&s, "book", &rk(2), CONTRIB);
    let version = option_for(&p, &rk(3)).link_version.clone().unwrap();
    let r = links_req(
        &s,
        "book",
        2,
        vec![lc(
            CONTRIB,
            vec![],
            vec![upd(rk(3), version, vec![("position", int(7))])],
            vec![],
        )],
    );
    s.save(&r).unwrap();
    assert_eq!(
        one::<i64>(
            &fx,
            "SELECT position FROM contribution WHERE book_id = 2 AND author_id = 3"
        ),
        7
    );
    assert_eq!(
        one::<String>(
            &fx,
            "SELECT role FROM contribution WHERE book_id = 2 AND author_id = 3"
        ),
        "translator"
    );
}

#[test]
fn link_update_stale_conflicts() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let p = options(&s, "book", &rk(2), CONTRIB);
    let version = option_for(&p, &rk(3)).link_version.clone().unwrap();
    fx.exec("UPDATE contribution SET position = 50 WHERE book_id = 2 AND author_id = 3;");
    let before = snapshot(&fx);
    let r = links_req(
        &s,
        "book",
        2,
        vec![lc(
            CONTRIB,
            vec![],
            vec![upd(rk(3), version, vec![("position", int(7))])],
            vec![],
        )],
    );
    let link = conflict_link(s.save(&r).unwrap_err());
    assert_eq!(link.other, rk(3));
    assert_eq!(snapshot(&fx), before);
}

#[test]
fn link_remove_with_and_without_version() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let p = options(&s, "book", &rk(2), GENRE);
    let v = option_for(&p, &pk("history")).link_version.clone().unwrap();
    let r = links_req(
        &s,
        "book",
        2,
        vec![lc(GENRE, vec![], vec![], vec![rm(pk("history"), Some(v))])],
    );
    s.save(&r).unwrap();
    assert_eq!(
        one::<i64>(&fx, "SELECT count(*) FROM book_genre WHERE book_id = 2"),
        1
    );
    // no version
    let r = links_req(
        &s,
        "book",
        2,
        vec![lc(GENRE, vec![], vec![], vec![rm(pk("fiction"), None)])],
    );
    s.save(&r).unwrap();
    assert_eq!(
        one::<i64>(&fx, "SELECT count(*) FROM book_genre WHERE book_id = 2"),
        0
    );
}

#[test]
fn link_remove_missing_conflicts() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let before = snapshot(&fx);
    let r = links_req(
        &s,
        "book",
        2,
        vec![lc(GENRE, vec![], vec![], vec![rm(pk("poetry"), None)])],
    );
    assert_eq!(conflict_link(s.save(&r).unwrap_err()).other, pk("poetry"));
    // stale version
    let r = links_req(
        &s,
        "book",
        2,
        vec![lc(
            GENRE,
            vec![],
            vec![],
            vec![rm(pk("fiction"), Some("stale".into()))],
        )],
    );
    assert_eq!(conflict_link(s.save(&r).unwrap_err()).other, pk("fiction"));
    // other object missing
    let r = links_req(
        &s,
        "book",
        2,
        vec![lc(CONTRIB, vec![], vec![], vec![rm(rk(999), None)])],
    );
    assert_eq!(conflict_link(s.save(&r).unwrap_err()).other, rk(999));
    assert_eq!(snapshot(&fx), before);
}

// ---- combined ----

#[test]
fn links_and_fields_one_transaction() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let p = options(&s, "book", &rk(2), CONTRIB);
    let cv = option_for(&p, &rk(3)).link_version.clone().unwrap();
    let mut r = request(&s, "book", Some(rk(2)), vec![("title", text("New title"))]);
    r.links = vec![
        lc(
            GENRE,
            vec![add(pk("poetry"), vec![]), add(pk("sci-fi"), vec![])],
            vec![],
            vec![rm(pk("history"), None)],
        ),
        lc(
            CONTRIB,
            vec![],
            vec![upd(rk(3), cv, vec![("position", int(9))])],
            vec![],
        ),
    ];
    let saved = s.save(&r).unwrap();
    assert_eq!(saved.object.label, "New title");
    assert_eq!(
        one::<i64>(&fx, "SELECT count(*) FROM book_genre WHERE book_id = 2"),
        3
    );
    assert_eq!(
        one::<i64>(&fx, "SELECT position FROM contribution WHERE book_id = 2"),
        9
    );

    // a failure in the last step undoes everything
    let before = snapshot(&fx);
    let mut r = request(&s, "book", Some(rk(2)), vec![("title", text("Again"))]);
    r.links = vec![
        lc(
            GENRE,
            vec![],
            vec![],
            vec![rm(pk("poetry"), None), rm(pk("sci-fi"), None)],
        ),
        lc(CONTRIB, vec![add(rk(3), vec![])], vec![], vec![]),
    ];
    let link = conflict_link(s.save(&r).unwrap_err());
    assert_eq!(link.relation, CONTRIB);
    assert_eq!(snapshot(&fx), before);
}

#[test]
fn create_with_links() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let mut r = request(
        &s,
        "book",
        None,
        vec![
            ("title", text("Fresh")),
            ("isbn", text("9780000000099")),
            ("author_id", reference(1)),
        ],
    );
    r.links = vec![
        lc(
            GENRE,
            vec![add(pk("fiction"), vec![]), add(pk("poetry"), vec![])],
            vec![],
            vec![],
        ),
        lc(
            CONTRIB,
            vec![add(rk(2), vec![("position", int(3))])],
            vec![],
            vec![],
        ),
    ];
    let saved = s.save(&r).unwrap();
    let key = saved.object.key.clone();
    let count = |rel: &str| {
        saved
            .object
            .many_to_many
            .iter()
            .find(|r| r.relation == rel)
            .unwrap()
            .count
    };
    assert_eq!((count(GENRE), count(CONTRIB)), (2, 1));
    let p = s
        .m2m_options("book", Some(&key), CONTRIB, &mq(None, 0, 100))
        .unwrap();
    assert_eq!(
        link_value(option_for(&p, &rk(2)), "position"),
        DbValue::Integer("3".into())
    );

    // update and remove are refused on a create
    let before = snapshot(&fx);
    let mut r = request(
        &s,
        "book",
        None,
        vec![
            ("title", text("Other")),
            ("isbn", text("9780000000103")),
            ("author_id", reference(1)),
        ],
    );
    r.links = vec![lc(
        GENRE,
        vec![],
        vec![upd(pk("fiction"), "v".into(), vec![])],
        vec![rm(pk("poetry"), None)],
    )];
    let errs = link_errors(s.save(&r).unwrap_err());
    assert_eq!(errs.len(), 2);
    assert!(errs
        .iter()
        .all(|e| e.issues[0].code == FieldIssueCode::NotAllowed));
    assert_eq!(snapshot(&fx), before);
}

#[test]
fn create_rejects_update_remove() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let mut r = request(
        &s,
        "genre",
        None,
        vec![("code", text("new")), ("name", text("New"))],
    );
    r.links = vec![lc(GENRE, vec![], vec![], vec![rm(rk(1), None)])];
    let errs = link_errors(s.save(&r).unwrap_err());
    assert_eq!(errs[0].issues[0].code, FieldIssueCode::NotAllowed);
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM genre"), 4);
}

// ---- restrictions ----

fn example_with(from: &str, to: &str) -> Fixture {
    assert!(EXAMPLE.contains(from));
    Fixture::new(SCHEMA, Some(&EXAMPLE.replacen(from, to, 1)))
}

#[test]
fn create_only_link_field() {
    let fx = example_with(
        "[tables.contribution.fields.position]\nmin = 1",
        "[tables.contribution.fields.position]\nmode = \"create-only\"\nmin = 1",
    );
    let mut s = fx.session();
    let p = options(&s, "book", &rk(2), CONTRIB);
    let v = option_for(&p, &rk(3)).link_version.clone().unwrap();
    let r = links_req(
        &s,
        "book",
        2,
        vec![lc(
            CONTRIB,
            vec![],
            vec![upd(rk(3), v, vec![("position", int(5))])],
            vec![],
        )],
    );
    let errs = link_errors(s.save(&r).unwrap_err());
    assert_eq!(
        codes(&errs[0].fields, "position"),
        [FieldIssueCode::NotEditable]
    );
    let r = links_req(
        &s,
        "book",
        4,
        vec![lc(
            CONTRIB,
            vec![add(rk(2), vec![("position", int(5))])],
            vec![],
            vec![],
        )],
    );
    s.save(&r).unwrap();
    assert_eq!(
        one::<i64>(&fx, "SELECT position FROM contribution WHERE book_id = 4"),
        5
    );
}

#[test]
fn hidden_required_link_field_blocks_add() {
    // role has no default here, and the config hides it
    let sql = SCHEMA.replace("role TEXT NOT NULL DEFAULT 'editor'", "role TEXT NOT NULL");
    let toml = EXAMPLE.replacen(
        "[tables.contribution.fields.role]\n",
        "[tables.contribution.fields.role]\nmode = \"hidden\"\n",
        1,
    );
    let fx = Fixture::new(&sql, Some(&toml));
    let mut s = fx.session();
    let r = links_req(
        &s,
        "book",
        4,
        vec![lc(CONTRIB, vec![add(rk(2), vec![])], vec![], vec![])],
    );
    assert!(matches!(s.save(&r), Err(SessionError::NotEditable { .. })));
    // removing is refused too: the relation is read-only
    let r = links_req(
        &s,
        "book",
        1,
        vec![lc(CONTRIB, vec![], vec![], vec![rm(rk(2), None)])],
    );
    assert!(matches!(s.save(&r), Err(SessionError::NotEditable { .. })));
}

#[test]
fn read_only_relation_refused() {
    let fx = example_with(
        "headings = { book = \"Genres\", genre = \"Books\" }",
        "mode = \"read-only\"\nheadings = { book = \"Genres\", genre = \"Books\" }",
    );
    let mut s = fx.session();
    let before = snapshot(&fx);
    for lcs in [
        lc(GENRE, vec![add(pk("poetry"), vec![])], vec![], vec![]),
        lc(GENRE, vec![], vec![], vec![rm(pk("fiction"), None)]),
    ] {
        let r = links_req(&s, "book", 1, vec![lcs]);
        assert!(matches!(s.save(&r), Err(SessionError::NotEditable { .. })));
    }
    assert_eq!(snapshot(&fx), before);
    assert_eq!(baks(&fx).len(), 0);
}

const TAGS: &str = "
CREATE TABLE doc (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
CREATE TABLE tag (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
CREATE TABLE doc_tag (
  doc_id INTEGER NOT NULL REFERENCES doc(id),
  tag_id INTEGER NOT NULL REFERENCES tag(id),
  PRIMARY KEY (doc_id, tag_id));
INSERT INTO doc VALUES (1, 'd1'), (2, 'd2');
INSERT INTO tag VALUES (1, 't1'), (2, 't2');
INSERT INTO doc_tag VALUES (1, 1);
";

#[test]
fn read_only_side_refused() {
    let fx = Fixture::new(TAGS, None);
    config_with(&fx, "[tables.doc]\nmode = \"read-only\"\n");
    let mut s = fx.session();
    let mut r = request(&s, "tag", Some(rk(1)), vec![]);
    r.links = vec![lc("m2m:doc_tag", vec![add(rk(2), vec![])], vec![], vec![])];
    // the tag side is editable (doc, the other side, is only read-only)
    s.save(&r).unwrap();
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM doc_tag"), 2);
    // the read-only side refuses
    let r = links_req(
        &s,
        "doc",
        1,
        vec![lc("m2m:doc_tag", vec![add(rk(2), vec![])], vec![], vec![])],
    );
    assert!(matches!(s.save(&r), Err(SessionError::NotEditable { .. })));
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM doc_tag"), 2);
}

#[test]
fn pickable_other_side_links() {
    let fx = Fixture::new(TAGS, None);
    config_with(&fx, "[tables.tag]\nmode = \"pickable\"\n");
    let mut s = fx.session();
    let r = links_req(
        &s,
        "doc",
        2,
        vec![lc("m2m:doc_tag", vec![add(rk(2), vec![])], vec![], vec![])],
    );
    s.save(&r).unwrap();
    assert_eq!(
        one::<i64>(&fx, "SELECT count(*) FROM doc_tag WHERE doc_id = 2"),
        1
    );
    let r = links_req(
        &s,
        "doc",
        1,
        vec![lc("m2m:doc_tag", vec![], vec![], vec![rm(rk(1), None)])],
    );
    s.save(&r).unwrap();
    assert_eq!(
        one::<i64>(&fx, "SELECT count(*) FROM doc_tag WHERE doc_id = 1"),
        0
    );
}

#[test]
fn links_only_stale_config_hash() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let mut r = links_req(
        &s,
        "book",
        4,
        vec![lc(GENRE, vec![add(pk("poetry"), vec![])], vec![], vec![])],
    );
    r.config_hash = "stale".into();
    assert!(matches!(s.save(&r), Err(SessionError::ConfigChangedOnDisk)));
    assert_eq!(
        one::<i64>(&fx, "SELECT count(*) FROM book_genre WHERE book_id = 4"),
        0
    );
    assert_eq!(baks(&fx).len(), 0);
}

#[test]
fn unknown_relation() {
    let fx = Fixture::example();
    let mut s = fx.session();
    // contribution is not author's... it is; use a relation of nobody
    for relation in ["m2m:nothing", "fk:loan:book_id"] {
        let r = links_req(&s, "book", 1, vec![lc(relation, vec![], vec![], vec![])]);
        assert!(
            matches!(s.save(&r), Err(SessionError::UnknownRelation(_))),
            "{relation}"
        );
    }
    // a relation of another table
    let r = links_req(
        &s,
        "member",
        1,
        vec![lc(GENRE, vec![add(rk(1), vec![])], vec![], vec![])],
    );
    assert!(matches!(s.save(&r), Err(SessionError::UnknownRelation(_))));
    assert_eq!(baks(&fx).len(), 0);
}

#[test]
fn duplicate_other_rejected() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let before = snapshot(&fx);
    let r = links_req(
        &s,
        "book",
        4,
        vec![lc(
            GENRE,
            vec![add(pk("poetry"), vec![]), add(pk("poetry"), vec![])],
            vec![],
            vec![rm(pk("poetry"), None)],
        )],
    );
    let errs = link_errors(s.save(&r).unwrap_err());
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].issues[0].code, FieldIssueCode::NotAllowed);
    assert!(errs[0].issues[0].message.contains("more than once"));
    assert_eq!(snapshot(&fx), before);
}

const DURATIONS: &str = "
CREATE TABLE a (id INTEGER PRIMARY KEY, name TEXT);
CREATE TABLE b (id INTEGER PRIMARY KEY, name TEXT);
CREATE TABLE ab (
  a_id INTEGER NOT NULL REFERENCES a(id),
  b_id INTEGER NOT NULL REFERENCES b(id),
  span INTEGER,
  tag TEXT UNIQUE,
  score INTEGER CHECK (score IS NULL OR score < 100),
  PRIMARY KEY (a_id, b_id));
INSERT INTO a VALUES (1, 'a1'), (2, 'a2');
INSERT INTO b VALUES (1, 'b1'), (2, 'b2');
";

fn durations_fx() -> Fixture {
    let fx = Fixture::new(DURATIONS, None);
    config_with(
        &fx,
        "[many_to_many.ab]\n[tables.ab.fields.span]\ntype = \"duration\"\nunit = \"seconds\"\nmin = \"1m\"\n",
    );
    fx
}

#[test]
fn link_duration_field() {
    let fx = durations_fx();
    let mut s = fx.session();
    let r = links_req(
        &s,
        "a",
        1,
        vec![lc(
            "m2m:ab",
            vec![add(rk(1), vec![("span", dur("1h 30m"))])],
            vec![],
            vec![],
        )],
    );
    s.save(&r).unwrap();
    assert_eq!(one::<i64>(&fx, "SELECT span FROM ab"), 5400);
    let p = options(&s, "a", &rk(1), "m2m:ab");
    assert_eq!(
        link_value(option_for(&p, &rk(1)), "span"),
        DbValue::Duration("1h30m".into())
    );
    let r = links_req(
        &s,
        "a",
        1,
        vec![lc(
            "m2m:ab",
            vec![add(rk(2), vec![("span", dur("30s"))])],
            vec![],
            vec![],
        )],
    );
    let errs = link_errors(s.save(&r).unwrap_err());
    assert_eq!(codes(&errs[0].fields, "span"), [FieldIssueCode::BelowMin]);
}

#[test]
fn link_constraint_mapping() {
    let fx = durations_fx();
    let mut s = fx.session();
    let r = links_req(
        &s,
        "a",
        1,
        vec![lc(
            "m2m:ab",
            vec![add(rk(1), vec![("tag", text("x"))])],
            vec![],
            vec![],
        )],
    );
    s.save(&r).unwrap();
    // UNIQUE on one changed link field
    let before = one::<i64>(&fx, "SELECT count(*) FROM ab");
    let r = links_req(
        &s,
        "a",
        2,
        vec![lc(
            "m2m:ab",
            vec![add(rk(1), vec![("tag", text("x"))])],
            vec![],
            vec![],
        )],
    );
    let errs = link_errors(s.save(&r).unwrap_err());
    assert_eq!(errs[0].other, rk(1));
    assert_eq!(codes(&errs[0].fields, "tag"), [FieldIssueCode::Unique]);
    // CHECK: an unmapped constraint, and the object change rolls back too
    let mut r = request(&s, "a", Some(rk(2)), vec![("name", text("renamed"))]);
    r.links = vec![lc(
        "m2m:ab",
        vec![add(rk(2), vec![("score", int(500))])],
        vec![],
        vec![],
    )];
    let e = s.save(&r).unwrap_err();
    assert!(matches!(e, SessionError::Constraint { .. }), "{e:?}");
    assert_eq!(one::<String>(&fx, "SELECT name FROM a WHERE id = 2"), "a2");
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM ab"), before);
}

// ---- backup and identifiers ----

#[test]
fn links_only_save_backs_up() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = links_req(
        &s,
        "book",
        4,
        vec![lc(GENRE, vec![add(pk("poetry"), vec![])], vec![], vec![])],
    );
    let saved = s.save(&r).unwrap();
    assert!(saved.backup_created.is_some());
    assert_eq!(baks(&fx).len(), 1);
    let r = links_req(
        &s,
        "book",
        4,
        vec![lc(GENRE, vec![add(pk("fiction"), vec![])], vec![], vec![])],
    );
    assert!(s.save(&r).unwrap().backup_created.is_none());
    assert_eq!(baks(&fx).len(), 1);
}

#[test]
fn link_only_first_save_fails_then_announces_backup() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let bad = links_req(
        &s,
        "book",
        2,
        vec![lc(GENRE, vec![add(pk("fiction"), vec![])], vec![], vec![])],
    );
    assert!(matches!(s.save(&bad), Err(SessionError::Conflict { .. })));
    assert!(
        s.backup_path().is_some(),
        "the backup was taken before the failure"
    );
    let good = links_req(
        &s,
        "book",
        4,
        vec![lc(GENRE, vec![add(pk("poetry"), vec![])], vec![], vec![])],
    );
    let saved = s.save(&good).unwrap();
    assert_eq!(
        saved.backup_created,
        Some(s.backup_path().unwrap().display().to_string())
    );
    assert_eq!(baks(&fx).len(), 1);
}

#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "short names for request, session and page"
)]
fn awkward_identifiers_links() {
    let fx = Fixture::new(
        "CREATE TABLE \"with space\" (id INTEGER PRIMARY KEY, n TEXT);
         CREATE TABLE \"\u{fc}n\u{ef}\" (id INTEGER PRIMARY KEY, n TEXT);
         CREATE TABLE \"select\" (
           \"a\"\"b\" INTEGER NOT NULL REFERENCES \"with space\"(id),
           \"from\" INTEGER NOT NULL REFERENCES \"\u{fc}n\u{ef}\"(id),
           \"x y\" TEXT,
           PRIMARY KEY (\"a\"\"b\", \"from\"));
         INSERT INTO \"with space\" VALUES (1, 'one');
         INSERT INTO \"\u{fc}n\u{ef}\" VALUES (1, 'u'), (2, 'v');",
        None,
    );
    config_with(&fx, "[many_to_many.\"select\"]\n");
    let mut s = fx.session();
    let rel = "m2m:select";
    let r = links_req(
        &s,
        "with space",
        1,
        vec![lc(
            rel,
            vec![add(rk(1), vec![("x y", text("hi"))])],
            vec![],
            vec![],
        )],
    );
    s.save(&r).unwrap();
    let p = options(&s, "with space", &rk(1), rel);
    let o = option_for(&p, &rk(1));
    assert_eq!(link_value(o, "x y"), DbValue::Text("hi".into()));
    let v = o.link_version.clone().unwrap();
    let r = links_req(
        &s,
        "with space",
        1,
        vec![lc(
            rel,
            vec![add(rk(2), vec![])],
            vec![upd(rk(1), v, vec![("x y", text("yo"))])],
            vec![],
        )],
    );
    s.save(&r).unwrap();
    assert_eq!(
        one::<String>(&fx, "SELECT \"x y\" FROM \"select\" WHERE \"from\" = 1"),
        "yo"
    );
    let r = links_req(
        &s,
        "with space",
        1,
        vec![lc(rel, vec![], vec![], vec![rm(rk(1), None)])],
    );
    s.save(&r).unwrap();
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM \"select\""), 1);
}
