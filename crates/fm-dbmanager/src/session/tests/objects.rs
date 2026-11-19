use super::*;

#[test]
fn object_fields_and_links() {
    let fx = Fixture::example();
    fx.exec("UPDATE book SET audio_length = 5400 WHERE id = 1;");
    let sess = fx.session();
    let o = sess.object("book", &rk(1)).unwrap();
    assert_eq!(o.table, "book");
    assert_eq!(o.key, rk(1));
    assert_eq!(o.label, "The Quiet Harbour");
    let order: Vec<&str> = o.fields.iter().take(4).map(|f| f.column.as_str()).collect();
    assert_eq!(order, ["title", "author_id", "isbn", "format"]);
    assert_eq!(field(&o, "in_print").checked, Some(true));
    assert_eq!(field(&o, "revision").value, DbValue::Integer("1".into()));
    let link = field(&o, "author_id").link.as_ref().unwrap();
    assert_eq!(link.table, "author");
    assert_eq!(link.key, Some(rk(1)));
    assert_eq!(link.label.as_deref(), Some("Lindqvist, Ada"));
    assert!(link.navigable);
    assert_eq!(
        serde_json::to_string(&field(&o, "audio_length").value).unwrap(),
        r#"{"type":"duration","value":"1h30m"}"#
    );
    assert!(field(&o, "audio_length").problem.is_none());
    // NULL publisher: no link
    let o4 = sess.object("book", &rk(4)).unwrap();
    assert!(field(&o4, "publisher_id").link.is_none());
    assert_eq!(field(&o4, "in_print").checked, Some(false));
    assert_eq!(field(&o4, "audio_length").value, DbValue::Null);
    assert_eq!(field(&o4, "audio_length").checked, None);
}

#[test]
fn object_hides_hidden() {
    let fx = Fixture::example();
    let sess = fx.session();
    let o = sess.object("author", &rk(1)).unwrap();
    assert!(o.fields.iter().all(|f| f.column != "internal_note"));
    let json = serde_json::to_string(&o).unwrap();
    assert!(!json.contains("internal_note"));
    assert!(!json.contains("prefers email"));
    let views = serde_json::to_string(&sess.table_views()).unwrap();
    assert!(!views.contains("internal_note"));
    // hidden table never listed
    assert!(matches!(
        sess.list("audit_log", &lq(None, 0, 10)),
        Err(SessionError::UnknownTable(_))
    ));
    assert!(!json.contains("audit_log"));
    // a FK into a hidden table is a plain value with no link
    let g = Fixture::new(
        "CREATE TABLE secret (id INTEGER PRIMARY KEY, name TEXT);\
         CREATE TABLE doc (id INTEGER PRIMARY KEY, title TEXT, secret_id INTEGER REFERENCES secret(id));\
         INSERT INTO secret VALUES (1, 'hush'); INSERT INTO doc VALUES (1, 'd', 1);",
        Some("version = 1\n[tables.secret]\nmode = \"hidden\"\n"),
    );
    let gs = g.session();
    let d = gs.object("doc", &rk(1)).unwrap();
    assert!(field(&d, "secret_id").link.is_none());
    assert!(!serde_json::to_string(&d).unwrap().contains("secret\""));
}

fn problem_of(fx: &Fixture, sql: &str, table: &str, id: i64, column: &str) -> Option<ValueProblem> {
    fx.exec(sql);
    let sess = fx.session();
    field(&sess.object(table, &rk(id)).unwrap(), column).problem
}

#[test]
fn value_problems() {
    let fx = Fixture::example();
    let p = |sql, table, id, col| problem_of(&fx, sql, table, id, col);
    assert_eq!(p("SELECT 1;", "book", 2, "pages"), None);
    assert_eq!(
        p(
            "UPDATE loan SET loan_period = 'abc' WHERE id = 1;",
            "loan",
            1,
            "loan_period"
        ),
        Some(ValueProblem::WrongType)
    );
    assert_eq!(
        p(
            "UPDATE book SET audio_length = 30 WHERE id = 1;",
            "book",
            1,
            "audio_length"
        ),
        Some(ValueProblem::OutOfBounds)
    );
    assert_eq!(
        p(
            "UPDATE book SET in_print = 2 WHERE id = 2;",
            "book",
            2,
            "in_print"
        ),
        Some(ValueProblem::NotBool)
    );
    assert_eq!(
        p(
            "UPDATE book SET pages = 'many' WHERE id = 2;",
            "book",
            2,
            "pages"
        ),
        Some(ValueProblem::WrongType)
    );
    assert_eq!(
        p(
            "UPDATE book SET pages = 0 WHERE id = 5;",
            "book",
            5,
            "pages"
        ),
        Some(ValueProblem::OutOfBounds)
    );
    assert_eq!(
        p(
            "UPDATE book SET format = 'scroll' WHERE id = 5;",
            "book",
            5,
            "format"
        ),
        Some(ValueProblem::OutOfBounds)
    );
    assert_eq!(
        p(
            "UPDATE book SET publisher_id = 99 WHERE id = 3;",
            "book",
            3,
            "publisher_id"
        ),
        Some(ValueProblem::DanglingReference)
    );
    let sess = fx.session();
    let o = sess.object("book", &rk(3)).unwrap();
    let link = field(&o, "publisher_id").link.as_ref().unwrap();
    assert!(link.key.is_none() && link.label.is_none());
    assert_eq!(field(&o, "in_print").checked, Some(true));
    // a bool with a problem is not ticked
    let o2 = sess.object("book", &rk(2)).unwrap();
    assert_eq!(field(&o2, "in_print").checked, None);
}

#[test]
fn non_finite_real_is_wrong_type() {
    let fx = Fixture::example();
    fx.exec("UPDATE book SET price = 9e999 WHERE id = 1;");
    let sess = fx.session();
    let o = sess.object("book", &rk(1)).unwrap();
    let price = field(&o, "price");
    assert_eq!(
        serde_json::to_string(&price.value).unwrap(),
        r#"{"type":"real","value":"inf"}"#
    );
    assert_eq!(price.problem, Some(ValueProblem::WrongType));
}

#[test]
fn duration_values_and_labels() {
    let fx = Fixture::new(
        SCHEMA,
        Some(&EXAMPLE.replace(
            "label = \"{title}\"",
            "label = \"{title} ({audio_length})\"",
        )),
    );
    let sess = fx.session();
    let o = sess.object("book", &rk(1)).unwrap();
    assert_eq!(o.label, "The Quiet Harbour (7h30m)");
    assert_eq!(
        field(&o, "audio_length").value,
        DbValue::Duration("7h30m".into())
    );
    assert_eq!(
        sess.object("book", &rk(2)).unwrap().label,
        "Salt and Iron ()"
    );
    let loan = sess.object("loan", &rk(1)).unwrap();
    assert_eq!(
        field(&loan, "loan_period").value,
        DbValue::Duration("14d".into())
    );
    assert_eq!(field(&loan, "returned").checked, Some(false));
    // a duration column is not searched
    let p = sess.list("book", &lq(Some("7h"), 0, 10)).unwrap();
    assert_eq!(p.total, 0);
    let b = sess.list("book", &lq(Some("quiet"), 0, 10)).unwrap();
    assert_eq!(b.items[0].label, "The Quiet Harbour (7h30m)");
}

#[test]
fn version_tracks_every_column() {
    let fx = Fixture::example();
    let version = |fx: &Fixture| fx.session().object("author", &rk(1)).unwrap().version;
    let v0 = version(&fx);
    assert_eq!(v0, version(&fx));
    fx.exec("UPDATE author SET internal_note = 'changed' WHERE id = 1;");
    let v1 = version(&fx);
    assert_ne!(v0, v1);
    fx.exec("UPDATE author SET internal_note = 'changed' WHERE id = 1;");
    assert_eq!(v1, version(&fx));
    fx.exec("UPDATE author SET born = 1962 WHERE id = 1;");
    let v2 = version(&fx);
    assert_ne!(v1, v2);
    fx.exec("UPDATE author SET born = NULL WHERE id = 1;");
    assert_ne!(v2, version(&fx));
    // other rows do not matter
    fx.exec("UPDATE author SET born = 1 WHERE id = 2;");
    let v3 = version(&fx);
    fx.exec("UPDATE author SET born = 2 WHERE id = 2;");
    assert_eq!(v3, version(&fx));
}

#[test]
fn unknown_table_object_relation() {
    let fx = Fixture::example();
    let sess = fx.session();
    assert!(matches!(
        sess.list("nope", &lq(None, 0, 1)),
        Err(SessionError::UnknownTable(t)) if t == "nope"
    ));
    assert!(matches!(
        sess.object("audit_log", &rk(1)),
        Err(SessionError::UnknownTable(_))
    ));
    assert!(matches!(
        sess.object("author", &rk(99)),
        Err(SessionError::UnknownObject)
    ));
    assert!(matches!(
        sess.object("author", &pk("x")),
        Err(SessionError::UnknownObject)
    ));
    assert!(matches!(
        sess.object(
            "author",
            &ObjectKey::Rowid {
                value: "abc".into()
            }
        ),
        Err(SessionError::UnknownObject)
    ));
    // a relation that belongs to another table
    let loan_book = relation_to(&sess, "book", "loan");
    let mut q = lq(None, 0, 10);
    q.referencing = Some(ReferencingFilter {
        relation: loan_book,
        key: rk(1),
    });
    assert!(matches!(
        sess.list("author", &q),
        Err(SessionError::UnknownRelation(_))
    ));
    q.referencing.as_mut().unwrap().relation = "fk:book:77".into();
    assert!(matches!(
        sess.list("book", &q),
        Err(SessionError::UnknownRelation(_))
    ));
    assert!(matches!(
        sess.m2m_options("author", None, "m2m:book_genre", &mq(None, 0, 10)),
        Err(SessionError::UnknownRelation(_))
    ));
    assert!(matches!(
        sess.m2m_options("audit_log", None, "m2m:book_genre", &mq(None, 0, 10)),
        Err(SessionError::UnknownTable(_))
    ));
    assert!(matches!(
        sess.m2m_options("book", Some(&rk(99)), "m2m:book_genre", &mq(None, 0, 10)),
        Err(SessionError::UnknownObject)
    ));
}

#[test]
fn schema_change_detected() {
    let fx = Fixture::example();
    let sess = fx.session();
    assert!(sess.list("author", &lq(None, 0, 10)).is_ok());
    fx.exec("ALTER TABLE author ADD COLUMN extra TEXT;");
    assert!(matches!(
        sess.list("author", &lq(None, 0, 10)),
        Err(SessionError::SchemaChanged)
    ));
    assert!(matches!(
        sess.object("author", &rk(1)),
        Err(SessionError::SchemaChanged)
    ));
    // a new session sees the new column
    let s2 = fx.session();
    assert!(s2.object("author", &rk(1)).unwrap().fields.len() > 4);
}
