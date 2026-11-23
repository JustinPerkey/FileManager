use super::save::{baks, one};
use super::*;

const ALLOW: &str = "version = 1\n[tables.parent]\nallow_delete = true\n";

/// Ada (author 1) has books 1 and 4 and illustrates book 3. Book 4 gets a
/// genre, a loan and a contribution of its own.
fn ada() -> Fixture {
    let fx = Fixture::example();
    fx.exec(
        "INSERT INTO book_genre VALUES (4, 'poetry');
         INSERT INTO loan (id, book_id, member_id) VALUES (5, 4, NULL);
         INSERT INTO contribution (book_id, author_id) VALUES (4, 3);",
    );
    fx
}

fn group<'a>(i: &'a DeleteImpact, table: &str, depth: u32) -> &'a ImpactGroup {
    i.groups
        .iter()
        .find(|g| g.table == table && g.depth == depth)
        .unwrap_or_else(|| panic!("no group {table} at depth {depth}: {:?}", i.groups))
}

fn sorted(g: &ImpactGroup) -> Vec<&str> {
    let mut v: Vec<&str> = g.sample.iter().map(String::as_str).collect();
    v.sort_unstable();
    v
}

fn run(s: &mut Session, table: &str, key: &ObjectKey) -> Result<DeleteResult, SessionError> {
    let impact = s.delete_preview(table, key).unwrap();
    let hash = s.config().hash().to_owned();
    s.delete(table, key, &impact.version, &impact.token, &hash)
}

fn parent_child(action: &str, not_null: &str) -> String {
    format!(
        "CREATE TABLE parent (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE child (id INTEGER PRIMARY KEY, pid INTEGER {not_null} REFERENCES parent(id) ON DELETE {action});
         INSERT INTO parent VALUES (1, 'p');
         INSERT INTO child VALUES (1, 1), (2, 1);"
    )
}

#[test]
fn impact_cascades_through_levels() {
    let fx = ada();
    let s = fx.session();
    let i = s.delete_preview("author", &rk(1)).unwrap();
    assert!(!i.blocked && !i.truncated);
    assert_eq!(i.label, "Lindqvist, Ada");
    let book = group(&i, "book", 1);
    assert_eq!(
        (book.action, book.count, book.hidden, book.links),
        (OnDeleteAction::Cascade, 2, false, false)
    );
    assert_eq!(sorted(book), ["A Short Winter", "The Quiet Harbour"]);
    let given = group(&i, "contribution", 1);
    assert_eq!(given.count, 1);
    assert_eq!(given.table_display_name, "Contributions");
    assert_eq!(given.sample, ["Maps of Nowhere"]);
    assert_eq!(group(&i, "book_genre", 2).count, 2);
    assert_eq!(group(&i, "loan", 2).count, 2);
    assert_eq!(group(&i, "contribution", 2).count, 2);
    assert_eq!(i.groups.len(), 5);
    let depths: Vec<u32> = i.groups.iter().map(|g| g.depth).collect();
    assert!(depths.is_sorted());
}

#[test]
fn impact_restrict_blocks() {
    let fx = Fixture::new(
        SCHEMA,
        Some(&EXAMPLE.replacen("mode = \"pickable\"", "allow_delete = true #", 1)),
    );
    let mut s = fx.session();
    let i = s.delete_preview("publisher", &rk(1)).unwrap();
    let g = group(&i, "book", 1);
    assert_eq!(
        (g.action, g.count, g.blocks),
        (OnDeleteAction::Restrict, 2, true)
    );
    assert!(i.blocked);
    let hash = s.config().hash().to_owned();
    let err = s
        .delete("publisher", &rk(1), &i.version, &i.token, &hash)
        .unwrap_err();
    assert!(matches!(err, SessionError::DeleteBlocked(_)), "{err:?}");
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM publisher"), 2);
}

#[test]
fn no_action_satisfied_by_cascade_does_not_block() {
    let fx = Fixture::new(
        "CREATE TABLE a (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE b (id INTEGER PRIMARY KEY, a_id INTEGER REFERENCES a(id) ON DELETE CASCADE);
         CREATE TABLE c (id INTEGER PRIMARY KEY,
             a_id INTEGER REFERENCES a(id) ON DELETE NO ACTION,
             b_id INTEGER REFERENCES b(id) ON DELETE CASCADE);
         INSERT INTO a VALUES (1, 'a');
         INSERT INTO b VALUES (1, 1);
         INSERT INTO c VALUES (1, 1, 1);",
        Some("version = 1\n[tables.a]\nallow_delete = true\n"),
    );
    let mut s = fx.session();
    let i = s.delete_preview("a", &rk(1)).unwrap();
    assert!(!i.blocked, "{:?}", i.groups);
    assert!(i.groups.iter().all(|g| !g.blocks));
    run(&mut s, "a", &rk(1)).unwrap();
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM c"), 0);
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM a"), 0);
}

#[test]
fn no_action_with_surviving_row_still_blocks() {
    let fx = Fixture::new(
        "CREATE TABLE a (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE c (id INTEGER PRIMARY KEY, a_id INTEGER REFERENCES a(id) ON DELETE NO ACTION);
         INSERT INTO a VALUES (1, 'a');
         INSERT INTO c VALUES (1, 1);",
        Some("version = 1\n[tables.a]\nallow_delete = true\n"),
    );
    let i = fx.session().delete_preview("a", &rk(1)).unwrap();
    assert!(i.blocked && group(&i, "c", 1).blocks);
}

#[test]
fn impact_set_null_allowed() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let i = s.delete_preview("member", &rk(1)).unwrap();
    let g = group(&i, "loan", 1);
    assert_eq!(
        (g.action, g.count, g.blocks),
        (OnDeleteAction::SetNull, 1, false)
    );
    assert!(!i.blocked);
    run(&mut s, "member", &rk(1)).unwrap();
    assert_eq!(
        one::<Option<i64>>(&fx, "SELECT member_id FROM loan WHERE id = 1"),
        None
    );
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM loan"), 4);
}

#[test]
fn impact_set_null_not_null_blocks() {
    let fx = Fixture::new(&parent_child("SET NULL", "NOT NULL"), Some(ALLOW));
    let s = fx.session();
    let i = s.delete_preview("parent", &rk(1)).unwrap();
    let g = group(&i, "child", 1);
    assert!(g.blocks && i.blocked);
    assert_eq!(g.action, OnDeleteAction::SetNull);
}

#[test]
fn impact_counts_hidden_tables_without_labels() {
    let sql = format!(
        "{SCHEMA}\nCREATE TABLE member_note (id INTEGER PRIMARY KEY, member_id INTEGER REFERENCES member(id) ON DELETE CASCADE, note TEXT);
         INSERT INTO member_note VALUES (1, 1, 'a'), (2, 1, 'b');"
    );
    let toml = format!("{EXAMPLE}\n[tables.member_note]\nmode = \"hidden\"\n");
    let fx = Fixture::new(&sql, Some(&toml));
    let s = fx.session();
    let i = s.delete_preview("member", &rk(1)).unwrap();
    let g = group(&i, "member_note", 1);
    assert!(g.hidden && g.sample.is_empty());
    assert_eq!(
        (g.count, g.via.as_str(), g.table_display_name.as_str()),
        (2, "member_id", "member_note")
    );
    // A hidden relation: its link group is hidden too.
    let toml = EXAMPLE.replacen(
        "headings = { book = \"Genres\", genre = \"Books\" }",
        "mode = \"hidden\"",
        1,
    );
    let fx = Fixture::new(SCHEMA, Some(&toml));
    let s = fx.session();
    let i = s.delete_preview("book", &rk(2)).unwrap();
    let g = group(&i, "book_genre", 1);
    assert!(g.hidden && g.links && g.sample.is_empty());
    assert_eq!(g.count, 2);
}

#[test]
fn impact_link_groups_and_link_data() {
    let fx = ada();
    let s = fx.session();
    let i = s.delete_preview("author", &rk(1)).unwrap();
    let given = group(&i, "contribution", 1);
    assert!(given.links && given.link_data && !given.hidden);
    let genres = group(&i, "book_genre", 2);
    assert!(genres.links, "{genres:?}");
    assert!(!genres.link_data && !genres.hidden, "{genres:?}");
    assert_eq!(genres.table_display_name, "Genres");
    assert_eq!(sorted(genres), ["Fiction", "Poetry"]);
    assert!(!group(&i, "loan", 2).links);
}

#[test]
fn impact_self_reference_no_double_count() {
    let fx = Fixture::new(
        "CREATE TABLE node (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES node(id) ON DELETE CASCADE);
         INSERT INTO node VALUES (1, NULL), (2, 1), (3, 1), (4, 2), (5, 2);",
        Some("version = 1\n[tables.node]\nallow_delete = true\n"),
    );
    let mut s = fx.session();
    let i = s.delete_preview("node", &rk(1)).unwrap();
    assert_eq!(i.groups.len(), 2);
    assert_eq!(group(&i, "node", 1).count, 2);
    assert_eq!(group(&i, "node", 2).count, 2);
    run(&mut s, "node", &rk(1)).unwrap();
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM node"), 0);
}

#[test]
fn impact_truncates() {
    let fx = Fixture::new(&parent_child("CASCADE", "NOT NULL"), Some(ALLOW));
    fx.exec(
        "WITH RECURSIVE n(i) AS (SELECT 3 UNION ALL SELECT i + 1 FROM n WHERE i < 10100)
         INSERT INTO child SELECT i, 1 FROM n;",
    );
    let s = fx.session();
    let i = s.delete_preview("parent", &rk(1)).unwrap();
    assert!(i.truncated);
    let g = group(&i, "child", 1);
    assert!(g.truncated);
    assert_eq!(g.count, 10_000);
}

#[test]
fn impact_flags_triggers() {
    let fx = ada();
    let before = fx.session().delete_preview("author", &rk(1)).unwrap();
    assert!(!before.has_triggers);
    fx.exec("CREATE TRIGGER t AFTER DELETE ON loan BEGIN SELECT 1; END;");
    let after = fx.session().delete_preview("author", &rk(1)).unwrap();
    assert!(after.has_triggers);
    assert!(
        !fx.session()
            .delete_preview("genre", &pk("poetry"))
            .unwrap()
            .has_triggers
    );
}

#[test]
fn delete_applies_cascade() {
    let fx = ada();
    let mut s = fx.session();
    let r = run(&mut s, "author", &rk(1)).unwrap();
    assert_eq!(r.label, "Lindqvist, Ada");
    assert_eq!(r.impact.groups.len(), 5);
    for (sql, left) in [
        ("SELECT count(*) FROM author", 2),
        ("SELECT count(*) FROM book", 3),
        ("SELECT count(*) FROM book_genre", 4),
        ("SELECT count(*) FROM loan", 3),
        ("SELECT count(*) FROM contribution", 1),
    ] {
        assert_eq!(one::<i64>(&fx, sql), left, "{sql}");
    }
}

#[test]
fn delete_backs_up_first() {
    let fx = ada();
    let mut s = fx.session();
    let r = run(&mut s, "author", &rk(1)).unwrap();
    let path = r.backup_created.expect("first write announces the backup");
    assert_eq!(baks(&fx).len(), 1);
    let copy = rusqlite::Connection::open(&path).unwrap();
    let n: i64 = copy
        .query_row("SELECT count(*) FROM author", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 3);
    assert!(run(&mut s, "author", &rk(2))
        .unwrap()
        .backup_created
        .is_none());
}

#[test]
fn failed_first_delete_then_success_announces_backup() {
    let fx = ada();
    let mut s = fx.session();
    let i = s.delete_preview("author", &rk(1)).unwrap();
    fx.exec("UPDATE author SET born = 1 WHERE id = 1");
    let hash = s.config().hash().to_owned();
    let err = s
        .delete("author", &rk(1), &i.version, &i.token, &hash)
        .unwrap_err();
    assert!(
        matches!(err, SessionError::Conflict { link: None, .. }),
        "{err:?}"
    );
    assert!(s.backup_path().is_some());
    let r = run(&mut s, "author", &rk(1)).unwrap();
    assert!(r.backup_created.is_some());
}

#[test]
fn delete_blocked_changes_nothing() {
    let fx = Fixture::new(&parent_child("RESTRICT", ""), Some(ALLOW));
    let mut s = fx.session();
    let err = run(&mut s, "parent", &rk(1)).unwrap_err();
    match err {
        SessionError::DeleteBlocked(i) => assert!(i.blocked),
        other => panic!("{other:?}"),
    }
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM parent"), 1);
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM child"), 2);
}

#[test]
fn delete_impact_changed() {
    let fx = ada();
    let mut s = fx.session();
    let i = s.delete_preview("author", &rk(1)).unwrap();
    fx.exec("INSERT INTO loan (id, book_id) VALUES (9, 1)");
    let hash = s.config().hash().to_owned();
    let err = s
        .delete("author", &rk(1), &i.version, &i.token, &hash)
        .unwrap_err();
    match err {
        SessionError::ImpactChanged(fresh) => {
            assert_eq!(group(&fresh, "loan", 2).count, 3);
            assert_ne!(fresh.token, i.token);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM author"), 3);
}

#[test]
fn delete_conflict() {
    let fx = ada();
    let mut s = fx.session();
    let i = s.delete_preview("author", &rk(1)).unwrap();
    fx.exec("UPDATE author SET last_name = 'Changed' WHERE id = 1");
    let hash = s.config().hash().to_owned();
    let err = s
        .delete("author", &rk(1), &i.version, &i.token, &hash)
        .unwrap_err();
    assert!(
        matches!(err, SessionError::Conflict { link: None, .. }),
        "{err:?}"
    );
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM author"), 3);
}

#[test]
fn delete_not_allowed() {
    let fx = Fixture::new(SCHEMA, None);
    let mut s = fx.session();
    let err = run(&mut s, "member", &rk(1)).unwrap_err();
    assert!(matches!(err, SessionError::NotEditable { .. }), "{err:?}");
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM member"), 3);
    assert_eq!(baks(&fx).len(), 0);
}

#[test]
fn delete_off_by_default() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let err = run(&mut s, "genre", &pk("history")).unwrap_err();
    assert!(matches!(err, SessionError::NotEditable { .. }), "{err:?}");
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM genre"), 4);
}

#[test]
fn delete_stale_config_hash() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let i = s.delete_preview("member", &rk(1)).unwrap();
    let err = s
        .delete("member", &rk(1), &i.version, &i.token, "stale")
        .unwrap_err();
    assert!(matches!(err, SessionError::ConfigChangedOnDisk), "{err:?}");
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM member"), 3);
}

#[test]
fn delete_pickable_refused() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let err = run(&mut s, "publisher", &rk(2)).unwrap_err();
    assert!(matches!(err, SessionError::NotEditable { .. }), "{err:?}");
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM publisher"), 2);
}

#[test]
fn delete_read_only_database() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let i = s.delete_preview("member", &rk(1)).unwrap();
    s.database.force_read_only("test");
    let hash = s.config().hash().to_owned();
    let err = s
        .delete("member", &rk(1), &i.version, &i.token, &hash)
        .unwrap_err();
    assert!(matches!(err, SessionError::ReadOnlyDatabase), "{err:?}");
    assert_eq!(baks(&fx).len(), 0);
}

#[test]
fn delete_awkward_identifiers() {
    let fx = Fixture::new(
        "CREATE TABLE \"se\"\"lect\" (\"i\"\"d\" INTEGER PRIMARY KEY, \"with space\" TEXT);
         CREATE TABLE \"ch ild\" (id INTEGER PRIMARY KEY, \"p\"\"id\" INTEGER REFERENCES \"se\"\"lect\"(\"i\"\"d\") ON DELETE CASCADE);
         INSERT INTO \"se\"\"lect\" VALUES (1, 'a'), (2, 'b');
         INSERT INTO \"ch ild\" VALUES (1, 1), (2, 2);",
        Some("version = 1\n[tables.\"se\\\"lect\"]\nallow_delete = true\n"),
    );
    let mut s = fx.session();
    let i = s.delete_preview("se\"lect", &rk(1)).unwrap();
    let g = group(&i, "ch ild", 1);
    assert_eq!((g.count, g.via.as_str()), (1, "p\"id"));
    run(&mut s, "se\"lect", &rk(1)).unwrap();
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM \"se\"\"lect\""), 1);
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM \"ch ild\""), 1);
}
