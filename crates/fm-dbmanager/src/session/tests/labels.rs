use super::*;

const HIDE_ID: &str = "version = 1\n[tables.t.fields.id]\nmode = \"hidden\"\n";

/// `t` (id hidden or not), `u` pointing at `t`, and a junction `tu`.
fn rowid_schema(id: i64) -> String {
    format!(
        "CREATE TABLE t(id INTEGER PRIMARY KEY, x TEXT);
         CREATE TABLE u(id INTEGER PRIMARY KEY, name TEXT, t_id INTEGER REFERENCES t(id));
         CREATE TABLE tu(t_id INTEGER NOT NULL REFERENCES t(id),
                         u_id INTEGER NOT NULL REFERENCES u(id),
                         PRIMARY KEY(t_id, u_id));
         INSERT INTO t VALUES({id}, NULL);
         INSERT INTO u VALUES(1, 'U one', {id});
         INSERT INTO tu VALUES({id}, 1);"
    )
}

fn display(s: &Session, table: &str) -> String {
    s.table_views()
        .iter()
        .find(|t| t.name == table)
        .unwrap()
        .display_name
        .clone()
}

/// Every label reachable for `t` from the session, with `secret` absent.
fn check_all(sess: &Session, t_key: &ObjectKey, u_key: &ObjectKey, secret: &str) {
    let name = display(sess, "t");
    let mut seen: Vec<String> = Vec::new();
    // list items and the picker (the FK target's list)
    let page = sess.list("t", &lq(None, 0, 10)).unwrap();
    seen.extend(page.items.iter().map(|i| i.label.clone()));
    // the object label
    let o = sess.object("t", t_key).unwrap();
    seen.push(o.label.clone());
    // an FK link label from another table
    let u = sess.object("u", u_key).unwrap();
    let link = field(&u, "t_id").link.clone().unwrap();
    seen.push(link.label.clone().unwrap());
    // an m2m option, from the other side
    let m = sess
        .m2m_options("u", Some(u_key), "m2m:tu", &mq(None, 0, 10))
        .unwrap();
    assert!(m.items[0].linked);
    seen.extend(m.items.iter().map(|i| i.label.clone()));
    assert_eq!(seen.len(), 4);
    for label in &seen {
        assert_eq!(label, &name);
        assert!(!label.contains(secret));
    }
    // t's own view never carries its hidden id (u's FK column is u's own visible value)
    let json = serde_json::to_string(&(&o.fields, &o.label, &link)).unwrap();
    let json = json.replace(&serde_json::to_string(t_key).unwrap(), "");
    assert!(!json.contains(secret), "{json}");
}

#[test]
fn fallback_label_hides_hidden_rowid_alias() {
    let fx = Fixture::new(&rowid_schema(7777), Some(HIDE_ID));
    let sess = fx.session();
    check_all(&sess, &rk(7777), &rk(1), "7777");
}

#[test]
fn fallback_label_hides_hidden_without_rowid_pk() {
    let fx = Fixture::new(
        "CREATE TABLE t(k TEXT PRIMARY KEY, x TEXT) WITHOUT ROWID;
         CREATE TABLE u(id INTEGER PRIMARY KEY, name TEXT, t_id TEXT REFERENCES t(k));
         CREATE TABLE tu(t_id TEXT NOT NULL REFERENCES t(k),
                         u_id INTEGER NOT NULL REFERENCES u(id),
                         PRIMARY KEY(t_id, u_id));
         INSERT INTO t VALUES('sekrit-pk', NULL);
         INSERT INTO u VALUES(1, 'U one', 'sekrit-pk');
         INSERT INTO tu VALUES('sekrit-pk', 1);",
        Some("version = 1\n[tables.t.fields.k]\nmode = \"hidden\"\n"),
    );
    let sess = fx.session();
    check_all(&sess, &pk("sekrit-pk"), &rk(1), "sekrit-pk");
}

#[test]
fn fallback_label_shows_visible_key() {
    let fx = Fixture::new(&rowid_schema(7777), None);
    let sess = fx.session();
    let name = display(&sess, "t");
    let page = sess.list("t", &lq(None, 0, 10)).unwrap();
    assert_eq!(labels(&page), [format!("{name} #7777")]);
    assert_eq!(
        sess.object("t", &rk(7777)).unwrap().label,
        format!("{name} #7777")
    );
}

#[test]
fn blob_key_row_is_listed_but_unknown() {
    let fx = Fixture::new(
        "CREATE TABLE b(k BLOB PRIMARY KEY, x TEXT) WITHOUT ROWID;
         INSERT INTO b VALUES(X'4445414442454546', NULL);",
        None,
    );
    let sess = fx.session();
    let page = sess.list("b", &lq(None, 0, 10)).unwrap();
    assert_eq!(page.total, 1);
    assert!(!page.items[0].label.contains("4445"));
    assert!(!page.items[0].label.contains("DEAD"));
    let key = page.items[0].key.clone();
    assert!(matches!(
        sess.object("b", &key),
        Err(SessionError::UnknownObject)
    ));
}

#[test]
fn non_utf8_text_key_fails_gracefully() {
    let fx = Fixture::new(
        "CREATE TABLE t(k TEXT PRIMARY KEY, name TEXT) WITHOUT ROWID;
         CREATE TABLE c(id INTEGER PRIMARY KEY, t_k TEXT REFERENCES t(k));
         INSERT INTO t VALUES(CAST(X'66ff67' AS TEXT), 'T');
         INSERT INTO c VALUES(1, CAST(X'66ff67' AS TEXT));",
        None,
    );
    let sess = fx.session();
    let page = sess.list("t", &lq(None, 0, 10)).unwrap();
    assert_eq!(page.total, 1);
    assert!(matches!(
        sess.object("t", &page.items[0].key),
        Err(SessionError::UnknownObject)
    ));
    let c = sess.object("c", &rk(1)).unwrap();
    let f = field(&c, "t_k");
    assert_eq!(f.problem, Some(ValueProblem::DanglingReference));
    assert!(f.link.as_ref().is_none_or(|l| l.key.is_none()));
}
