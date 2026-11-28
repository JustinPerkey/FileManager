//! The Database Manager model's tests: restoring, history, lists, one object
//! with its "referenced by" and many-to-many pages, opening, dropping, the
//! copy and the fixtures. Model tests run against a [`FixtureEnv`] (a real
//! core over a database in a temp dir) and [`FakePlatform`].

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use fm_dbmanager::config::{ConfigDiagnostic, DiagnosticSeverity, TableMode};
use fm_dbmanager::session::ObjectKey;
use fm_dbmanager::tool::{DbmanagerError, DbmanagerErrorKind, DbmanagerSnapshot};
use gpui::{AppContext as _, Context, Entity, TestAppContext};

use super::backend::DbmanagerBackend;
use super::fixtures::{DbmanagerFixture, FixtureDbmanager, FixtureEnv};
use super::model::{
    DbmanagerModel, FocusHint, ObjectLoad, Place, ViewError, MAX_HISTORY, PAGE_SIZE,
};
use super::text;
use super::Hooks;
use crate::app::drop::DropTarget;
use crate::app::text::PICK_FAILED;
use crate::platform::fake::{FakePlatform, PlatformCall};
use crate::platform::{self, PickError, PickKind};
use crate::tools::ToolHooks as _;

// ---- helpers ----

fn start_boxed(
    cx: &mut TestAppContext,
    backend: Box<dyn DbmanagerBackend>,
) -> (Entity<DbmanagerModel>, FakePlatform) {
    cx.executor().allow_parking();
    let fake = FakePlatform::new();
    let installed = fake.clone();
    cx.update(|cx| platform::set_platform(cx, installed));
    let model = cx.new(|cx| DbmanagerModel::new(backend, cx));
    (model, fake)
}

fn start(cx: &mut TestAppContext, env: &FixtureEnv) -> (Entity<DbmanagerModel>, FakePlatform) {
    start_boxed(cx, Box::new(env.backend()))
}

fn read<R>(
    cx: &mut TestAppContext,
    model: &Entity<DbmanagerModel>,
    f: impl FnOnce(&DbmanagerModel) -> R,
) -> R {
    model.read_with(cx, |m, _| f(m))
}

fn act<R>(
    cx: &mut TestAppContext,
    model: &Entity<DbmanagerModel>,
    f: impl FnOnce(&mut DbmanagerModel, &mut Context<DbmanagerModel>) -> R,
) -> R {
    model.update(cx, f)
}

/// Runs everything queued until no request is in flight.
fn idle(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if read(cx, model, |m| !m.loading() && !m.busy()) {
            cx.run_until_parked();
            return;
        }
        assert!(Instant::now() < deadline, "the model never went idle");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn hint(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>) -> Option<FocusHint> {
    act(cx, model, |m, _| m.take_focus_hint())
}

fn loaded() -> FixtureEnv {
    FixtureEnv::new(DbmanagerFixture::Loaded)
}

fn started(cx: &mut TestAppContext) -> (FixtureEnv, Entity<DbmanagerModel>, FakePlatform) {
    let env = loaded();
    let (model, fake) = start(cx, &env);
    idle(cx, &model);
    (env, model, fake)
}

fn rowid(n: i64) -> ObjectKey {
    ObjectKey::Rowid {
        value: n.to_string(),
    }
}

fn table(name: &str) -> Place {
    Place::Table {
        table: name.to_owned(),
    }
}

fn place(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>) -> Place {
    read(cx, model, |m| m.place().clone())
}

fn choose(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>, name: &str) {
    act(cx, model, |m, cx| m.choose_table(name, cx));
    idle(cx, model);
}

fn open_object(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>, name: &str, n: i64) {
    act(cx, model, |m, cx| m.open_object(name, rowid(n), cx));
    idle(cx, model);
}

fn snapshot(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>) -> DbmanagerSnapshot {
    read(cx, model, |m| m.snapshot().cloned()).expect("a snapshot")
}

fn relation_to(
    cx: &mut TestAppContext,
    model: &Entity<DbmanagerModel>,
    on: &str,
    other: &str,
) -> String {
    read(cx, model, |m| {
        m.table_view(on)
            .and_then(|t| t.referenced_by.iter().find(|r| r.table == other))
            .map(|r| r.relation.clone())
    })
    .unwrap_or_else(|| panic!("{on} is referenced by {other}"))
}

// ---- restoring and history ----

#[gpui::test]
fn restores_and_resets_history_to_first_table(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    let first = read(cx, &model, |m| m.listed_tables()[0].name.clone());
    assert!(snapshot(cx, &model).open);
    assert_eq!(place(cx, &model), table(&first));
    assert!(read(cx, &model, |m| !m.can_back() && !m.can_forward()));
    let page = read(cx, &model, |m| {
        m.list_state(&first).and_then(|l| l.page.clone())
    });
    assert!(page.is_some_and(|p| p.total > 0));
    // Opening another database starts the history over.
    choose(cx, &model, "member");
    assert!(read(cx, &model, DbmanagerModel::can_back));
    let path = snapshot(cx, &model).database.unwrap().path;
    act(cx, &model, |m, cx| m.open_path(path, cx));
    idle(cx, &model);
    assert_eq!(place(cx, &model), table(&first));
    assert!(read(cx, &model, |m| !m.can_back()));
}

#[gpui::test]
fn nothing_listed_without_a_database(cx: &mut TestAppContext) {
    let env = FixtureEnv::new(DbmanagerFixture::Empty);
    let (model, _) = start(cx, &env);
    idle(cx, &model);
    assert!(!snapshot(cx, &model).open);
    assert_eq!(place(cx, &model), Place::Tables);
    assert!(read(cx, &model, |m| m.listed_tables().is_empty()));
    assert!(read(cx, &model, |m| m.object().is_none()));
}

#[gpui::test]
fn listed_tables_leave_out_pickable_and_hidden(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    let names: Vec<_> = read(cx, &model, |m| {
        m.listed_tables().iter().map(|t| t.name.clone()).collect()
    });
    assert!(names.contains(&"author".to_owned()));
    assert!(!names.contains(&"publisher".to_owned()), "pickable");
    assert!(!names.contains(&"audit_log".to_owned()), "hidden");
    assert!(read(cx, &model, |m| m.table_view("publisher").is_some()));
}

#[gpui::test]
fn history_push_back_forward_truncate(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    let first = place(cx, &model);
    choose(cx, &model, "book");
    open_object(cx, &model, "book", 2);
    assert!(matches!(place(cx, &model), Place::Object { .. }));
    assert_eq!(
        read(cx, &model, |m| m.history().len()),
        3,
        "first table, book, object"
    );

    act(cx, &model, DbmanagerModel::back);
    idle(cx, &model);
    assert_eq!(place(cx, &model), table("book"));
    assert!(read(cx, &model, |m| m.can_back() && m.can_forward()));
    act(cx, &model, DbmanagerModel::forward);
    idle(cx, &model);
    assert!(matches!(place(cx, &model), Place::Object { .. }));
    assert!(read(cx, &model, |m| !m.can_forward()));

    // Pushing from the middle drops the forward entries.
    act(cx, &model, DbmanagerModel::back);
    act(cx, &model, DbmanagerModel::back);
    idle(cx, &model);
    assert_eq!(place(cx, &model), first);
    choose(cx, &model, "member");
    assert_eq!(read(cx, &model, |m| m.history().len()), 2);
    assert!(read(cx, &model, |m| !m.can_forward()));
    // The same place is not pushed twice.
    choose(cx, &model, "member");
    assert_eq!(read(cx, &model, |m| m.history().len()), 2);
    // A table that is not in the database is ignored.
    choose(cx, &model, "no_such_table");
    assert_eq!(place(cx, &model), table("member"));
}

#[gpui::test]
fn history_capped_at_50(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    for i in 0..60 {
        let name = if i % 2 == 0 { "book" } else { "member" };
        act(cx, &model, |m, cx| m.choose_table(name, cx));
    }
    idle(cx, &model);
    assert_eq!(MAX_HISTORY, 50);
    assert_eq!(read(cx, &model, |m| m.history().len()), MAX_HISTORY);
    let mut steps = 0;
    while read(cx, &model, DbmanagerModel::can_back) {
        act(cx, &model, DbmanagerModel::back);
        steps += 1;
    }
    idle(cx, &model);
    assert_eq!(steps, MAX_HISTORY - 1);
    // The newest entry is still the last pushed.
    while read(cx, &model, DbmanagerModel::can_forward) {
        act(cx, &model, DbmanagerModel::forward);
    }
    assert_eq!(place(cx, &model), table("member"));
}

#[gpui::test]
fn deleted_object_entry_shows_gone(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_object(cx, &model, "member", 2);
    assert!(read(cx, &model, |m| matches!(
        m.object(),
        Some(ObjectLoad::Loaded(_))
    )));
    choose(cx, &model, "book");
    env.exec("DELETE FROM loan WHERE member_id = 2; DELETE FROM member WHERE id = 2;");
    let before = read(cx, &model, |m| m.history().to_vec());
    act(cx, &model, DbmanagerModel::back);
    idle(cx, &model);
    assert!(matches!(place(cx, &model), Place::Object { .. }));
    assert_eq!(
        read(cx, &model, |m| m.object().cloned()),
        Some(ObjectLoad::Gone)
    );
    assert_eq!(read(cx, &model, |m| m.history().to_vec()), before, "kept");
    assert!(read(cx, &model, |m| m.error().is_none()), "not an error");
}

#[gpui::test]
fn object_label_is_filled_in_when_read(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_object(cx, &model, "member", 1);
    let Place::Object { label, .. } = place(cx, &model) else {
        panic!("an object place");
    };
    assert_eq!(label, "Priya Raman");
    assert_eq!(hint(cx, &model), Some(FocusHint::SheetHeading));
    assert_eq!(hint(cx, &model), None, "once");
}

#[gpui::test]
fn place_changes_raise_focus_hints(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    assert_eq!(hint(cx, &model), None, "restoring takes no focus");
    choose(cx, &model, "book");
    assert_eq!(hint(cx, &model), Some(FocusHint::ListHeading));
    open_object(cx, &model, "book", 1);
    assert_eq!(hint(cx, &model), Some(FocusHint::SheetHeading));
    act(cx, &model, |m, cx| m.open_object("book", rowid(1), cx));
    idle(cx, &model);
    assert_eq!(hint(cx, &model), None, "no new place");
}

// ---- lists ----

#[gpui::test]
fn list_state_kept_per_table(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    choose(cx, &model, "author");
    act(cx, &model, |m, cx| m.set_search("author", "Tomas", cx));
    idle(cx, &model);
    let author = read(cx, &model, |m| m.list_state("author").cloned()).unwrap();
    assert_eq!(author.search, "Tomas");
    assert_eq!(author.page.as_ref().map(|p| p.total), Some(1));

    choose(cx, &model, "book");
    act(cx, &model, |m, cx| m.set_offset("book", 2, cx));
    idle(cx, &model);
    let book = read(cx, &model, |m| m.list_state("book").cloned()).unwrap();
    assert_eq!((book.search.as_str(), book.offset), ("", 2));
    assert_eq!(book.page.as_ref().map(|p| p.items.len()), Some(3));

    // Returning shows each as left.
    choose(cx, &model, "author");
    let again = read(cx, &model, |m| m.list_state("author").cloned()).unwrap();
    assert_eq!(again.search, "Tomas");
    assert_eq!(again.page.as_ref().map(|p| p.total), Some(1));
    // A new search starts at the first page; clearing it lists everything.
    act(cx, &model, |m, cx| m.set_search("book", "", cx));
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m.list_state("book").map(|l| l.offset)),
        Some(0)
    );
    act(cx, &model, |m, cx| m.set_search("author", "", cx));
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m
            .list_state("author")
            .and_then(|l| l.page.as_ref().map(|p| p.total))),
        Some(3)
    );
}

#[gpui::test]
fn only_newest_list_request_applies(cx: &mut TestAppContext) {
    let env = loaded();
    let handle = cx.update(crate::jobs::WorkerGate::install_closed);
    let (model, _) = start(cx, &env);
    handle.open();
    idle(cx, &model);
    handle.close();
    choose_without_waiting(cx, &model, "book");
    // Two searches queue behind the list read of the table choice.
    act(cx, &model, |m, cx| m.set_search("book", "Quiet", cx));
    act(cx, &model, |m, cx| m.set_search("book", "Salt", cx));
    cx.run_until_parked();
    // The choice's read and the first search answer; neither is the newest
    // request, so neither shows.
    handle.release_one();
    handle.release_one();
    cx.run_until_parked();
    let shown = read(cx, &model, |m| m.list_state("book").cloned()).unwrap();
    assert!(shown.page.is_none(), "stale answers are discarded");
    assert!(shown.loading);
    handle.release_one();
    cx.run_until_parked();
    let shown = read(cx, &model, |m| m.list_state("book").cloned()).unwrap();
    assert!(!shown.loading);
    let labels: Vec<_> = shown
        .page
        .unwrap()
        .items
        .into_iter()
        .map(|i| i.label)
        .collect();
    assert_eq!(labels, ["Salt and Iron"]);
}

fn choose_without_waiting(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>, name: &str) {
    act(cx, model, |m, cx| m.choose_table(name, cx));
}

// ---- referenced by and many-to-many ----

#[gpui::test]
fn referenced_by_closed_until_opened_and_pages(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    // Ada has 2 books; give her 30 more, and add an author nobody refers to.
    let mut sql = String::new();
    for i in 0..30 {
        write!(
            sql,
            "INSERT INTO book (title, isbn, author_id) VALUES ('Extra {i:02}', '978100000{i:04}', 1);"
        )
        .unwrap();
    }
    sql.push_str("INSERT INTO author (id, first_name, last_name) VALUES (9, 'No', 'Books');");
    env.exec(&sql);
    open_object(cx, &model, "author", 1);
    let relation = relation_to(cx, &model, "author", "book");

    assert!(
        read(cx, &model, |m| m.referenced_by(&relation).is_none()),
        "closed"
    );
    act(cx, &model, |m, cx| m.open_referenced_by(&relation, cx));
    idle(cx, &model);
    let list = read(cx, &model, |m| m.referenced_by(&relation).cloned()).unwrap();
    assert!(list.open && !list.loading);
    assert_eq!(list.total, 32);
    assert_eq!(list.items.len(), PAGE_SIZE as usize);

    act(cx, &model, |m, cx| m.show_more_referenced_by(&relation, cx));
    idle(cx, &model);
    let list = read(cx, &model, |m| m.referenced_by(&relation).cloned()).unwrap();
    assert_eq!(list.items.len(), 32);
    let mut keys: Vec<_> = list.items.iter().map(|i| i.key.clone()).collect();
    keys.dedup();
    assert_eq!(keys.len(), 32, "no row twice");
    act(cx, &model, |m, cx| m.show_more_referenced_by(&relation, cx));
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m
            .referenced_by(&relation)
            .map(|l| l.items.len())),
        Some(32),
        "nothing more to show"
    );

    // A count of 0 is not openable.
    open_object(cx, &model, "author", 9);
    act(cx, &model, |m, cx| m.open_referenced_by(&relation, cx));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.referenced_by(&relation).is_none()));
    // Entries do not outlive their object.
    act(cx, &model, DbmanagerModel::back);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.referenced_by(&relation).is_none()));
}

#[gpui::test]
fn m2m_linked_only_defaults_by_relation_mode(cx: &mut TestAppContext) {
    let env = loaded();
    std::fs::write(
        &env.config,
        "version = 1\n[tables.genre]\nmode = \"read-only\"\n",
    )
    .unwrap();
    let (model, _) = start(cx, &env);
    idle(cx, &model);

    open_object(cx, &model, "book", 2);
    let relation = read(cx, &model, |m| {
        m.table_view("book").unwrap().many_to_many[0]
            .relation
            .clone()
    });
    let editable = read(cx, &model, |m| m.m2m(&relation).cloned()).unwrap();
    assert!(
        !editable.linked_only,
        "editable relations list every option"
    );
    let page = editable.page.unwrap();
    assert_eq!(page.total, 4);
    assert_eq!(page.linked_total, 2);

    let genre_key = ObjectKey::Primary {
        values: vec![fm_dbmanager::value::DbValue::Text("fiction".into())],
    };
    act(cx, &model, |m, cx| m.open_object("genre", genre_key, cx));
    idle(cx, &model);
    let relation = read(cx, &model, |m| {
        m.table_view("genre").unwrap().many_to_many[0]
            .relation
            .clone()
    });
    let read_only = read(cx, &model, |m| m.m2m(&relation).cloned()).unwrap();
    assert!(
        read_only.linked_only,
        "read-only relations list linked only"
    );
    assert_eq!(read_only.page.unwrap().total, 2);

    // Changing the query reads the page it names.
    act(cx, &model, |m, cx| {
        m.set_m2m_query(&relation, "", false, 0, cx);
    });
    idle(cx, &model);
    let all = read(cx, &model, |m| m.m2m(&relation).cloned()).unwrap();
    assert!(!all.linked_only);
    assert_eq!(all.page.unwrap().total, 5);
    act(cx, &model, |m, cx| {
        m.set_m2m_query(&relation, "Quiet", false, 0, cx);
    });
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m
            .m2m(&relation)
            .and_then(|s| s.page.as_ref().map(|p| p.total))),
        Some(1)
    );
}

// ---- pickable ----

#[gpui::test]
fn pickable_target_opens_read_only(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    assert_eq!(
        read(cx, &model, |m| m.table_view("publisher").map(|t| t.mode)),
        Some(TableMode::Pickable)
    );
    open_object(cx, &model, "book", 1);
    act(cx, &model, |m, cx| m.follow("publisher_id", cx));
    idle(cx, &model);
    let Place::Object { table, label, .. } = place(cx, &model) else {
        panic!("following opens the publisher");
    };
    assert_eq!(
        (table.as_str(), label.as_str()),
        ("publisher", "Northwind Press")
    );
    assert!(read(cx, &model, |m| matches!(
        m.object(),
        Some(ObjectLoad::Loaded(_))
    )));
    assert_eq!(
        read(cx, &model, |m| m.table_read_only_reason("publisher")),
        Some(text::TABLE_PICKABLE.to_owned())
    );
    assert_eq!(
        read(cx, &model, |m| m.table_read_only_reason("author")),
        None
    );
    // A field that is not a reference follows nowhere.
    act(cx, &model, DbmanagerModel::back);
    idle(cx, &model);
    let before = read(cx, &model, |m| m.history().len());
    act(cx, &model, |m, cx| m.follow("title", cx));
    act(cx, &model, |m, cx| m.follow("nope", cx));
    assert_eq!(read(cx, &model, |m| m.history().len()), before);
}

// ---- drops ----

fn second_database(env: &FixtureEnv) -> PathBuf {
    let path = env.dir().join("second.db");
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE note (id INTEGER PRIMARY KEY, body TEXT NOT NULL);")
        .unwrap();
    path
}

fn hooks(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>) -> Hooks {
    let _ = cx;
    Hooks {
        model: model.clone(),
    }
}

#[gpui::test]
fn drop_opens_single_database(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    let hooks = hooks(cx, &model);
    let target = cx.update(|cx| hooks.drop_target(cx));
    assert_eq!(
        target,
        DropTarget::Enabled {
            label: text::DROP_LABEL.into()
        }
    );
    let second = second_database(&env);
    let dropped = second.clone();
    let (window, vcx) = cx.add_window_view(|_, _| gpui::Empty);
    let _ = window;
    vcx.update(|window, cx| hooks.dropped(vec![dropped], window, cx));
    idle(cx, &model);
    assert_eq!(snapshot(cx, &model).database.unwrap().path, second);
    assert_eq!(place(cx, &model), table("note"));
    assert_eq!(snapshot(cx, &model).recent.len(), 2);
}

#[gpui::test]
fn dropped_toml_is_banner(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    let before = snapshot(cx, &model);
    let config = env.config.clone();
    act(cx, &model, |m, cx| m.dropped(vec![config], cx));
    idle(cx, &model);
    let error = read(cx, &model, |m| m.error().cloned()).expect("a banner");
    let ViewError::Dbmanager { error: e, .. } = &error else {
        panic!("a core error: {error:?}");
    };
    assert_eq!(e.kind, DbmanagerErrorKind::ConfigNotDroppable);
    assert!(
        error
            .copy()
            .contains(before.config.display.as_deref().unwrap()),
        "names the config's location: {}",
        error.copy()
    );
    assert_eq!(snapshot(cx, &model), before, "the database stays open");
    assert_eq!(hint(cx, &model), Some(FocusHint::ErrorBanner));

    let generation = read(cx, &model, DbmanagerModel::error_generation);
    let (a, b) = (env.db.clone(), second_database(&env));
    act(cx, &model, |m, cx| m.dropped(vec![a, b], cx));
    idle(cx, &model);
    let kind = read(cx, &model, |m| match m.error() {
        Some(ViewError::Dbmanager { error, .. }) => Some(error.kind),
        _ => None,
    });
    assert_eq!(kind, Some(DbmanagerErrorKind::DropAmbiguous));
    assert!(read(cx, &model, DbmanagerModel::error_generation) > generation);
    act(cx, &model, DbmanagerModel::dismiss_error);
    assert!(read(cx, &model, |m| m.error().is_none()));
}

// ---- activation ----

#[gpui::test]
fn activated_keeps_place_and_history(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    choose(cx, &model, "book");
    open_object(cx, &model, "book", 2);
    let (history, here) = read(cx, &model, |m| (m.history().to_vec(), m.place().clone()));
    let config = env.config.clone();
    act(cx, &model, |m, cx| m.dropped(vec![config], cx));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.error().is_some()
        && m.announcement().is_none()));
    act(cx, &model, DbmanagerModel::reload);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.announcement().is_some()));

    act(cx, &model, DbmanagerModel::activated);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.error().is_none()
        && m.announcement().is_none()));
    assert_eq!(read(cx, &model, |m| m.history().to_vec()), history);
    assert_eq!(place(cx, &model), here);
    assert!(read(cx, &model, |m| matches!(
        m.object(),
        Some(ObjectLoad::Loaded(_))
    )));
}

#[gpui::test]
fn reload_keeps_the_place_and_picks_up_the_config(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    choose(cx, &model, "author");
    assert!(read(cx, &model, |m| m.table_read_only_reason("author")).is_none());
    std::fs::write(
        &env.config,
        "version = 1\n[defaults]\ntable_mode = \"read-only\"\n",
    )
    .unwrap();
    assert!(
        snapshot(cx, &model).config_changed || {
            act(cx, &model, DbmanagerModel::activated);
            idle(cx, &model);
            snapshot(cx, &model).config_changed
        }
    );
    act(cx, &model, DbmanagerModel::reload);
    idle(cx, &model);
    assert_eq!(place(cx, &model), table("author"));
    assert!(read(cx, &model, |m| m.table_read_only_reason("author")).is_some());
    assert!(!snapshot(cx, &model).config_changed);
}

#[gpui::test]
fn close_database_goes_to_the_table_list(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    act(cx, &model, DbmanagerModel::close_database);
    idle(cx, &model);
    assert!(!snapshot(cx, &model).open);
    assert_eq!(place(cx, &model), Place::Tables);
    assert_eq!(
        read(cx, &model, |m| m.announcement().map(str::to_owned)),
        Some(text::CLOSED_SENTENCE.to_owned())
    );
}

#[gpui::test]
fn recent_databases_open_by_position(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    let second = second_database(&env);
    act(cx, &model, |m, cx| m.open_path(second.clone(), cx));
    idle(cx, &model);
    assert_eq!(snapshot(cx, &model).recent.len(), 2);
    act(cx, &model, |m, cx| m.open_recent(1, cx));
    idle(cx, &model);
    assert_eq!(snapshot(cx, &model).database.unwrap().path, env.db);
    // A position that is not in the list is a banner, not a panic.
    act(cx, &model, |m, cx| m.open_recent(7, cx));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.error().is_some()));
    assert_eq!(snapshot(cx, &model).database.unwrap().path, env.db);
}

#[gpui::test]
fn superseded_open_keeps_the_backend_state(cx: &mut TestAppContext) {
    let env = FixtureEnv::new(DbmanagerFixture::Empty);
    let handle = cx.update(crate::jobs::WorkerGate::install_closed);
    let (model, _) = start(cx, &env);
    // The snapshot fetch of `new` is queued first; then A opens (stale), and
    // B (newest) fails. The backend keeps A.
    let missing = env.dir().join("missing.db");
    act(cx, &model, |m, cx| m.open_path(env.db.clone(), cx));
    act(cx, &model, |m, cx| m.open_path(missing, cx));
    handle.open();
    idle(cx, &model);
    let shown = snapshot(cx, &model);
    assert!(shown.open);
    assert_eq!(shown.database.unwrap().path, env.db);
    let first = read(cx, &model, |m| m.listed_tables()[0].name.clone());
    assert_eq!(place(cx, &model), table(&first));
    assert!(read(cx, &model, |m| m
        .list_state(&first)
        .is_some_and(|l| l.page.is_some())));
    assert!(matches!(
        read(cx, &model, |m| m.error().cloned()),
        Some(ViewError::Dbmanager { .. })
    ));
}

#[gpui::test]
fn panicked_command_refetches_snapshot(cx: &mut TestAppContext) {
    let env = FixtureEnv::new(DbmanagerFixture::Empty);
    let handle = cx.update(crate::jobs::WorkerGate::install_closed);
    let (model, _) = start(cx, &env);
    let bad = env.dir().join("panic.db");
    env.panic_on_open_of(&bad);
    act(cx, &model, |m, cx| m.open_path(env.db.clone(), cx));
    act(cx, &model, |m, cx| m.open_path(bad, cx));
    handle.open();
    idle(cx, &model);
    let shown = snapshot(cx, &model);
    assert!(shown.open);
    assert_eq!(shown.database.unwrap().path, env.db);
    assert!(matches!(
        read(cx, &model, |m| m.error().cloned()),
        Some(ViewError::Unexpected { .. })
    ));
}

// ---- the open dialog ----

#[gpui::test]
fn open_passes_database_filters(cx: &mut TestAppContext) {
    let (env, model, fake) = started(cx);
    act(cx, &model, DbmanagerModel::open);
    idle(cx, &model);
    let calls = fake.calls();
    let [PlatformCall::Pick(spec)] = calls.as_slice() else {
        panic!("one dialog: {calls:?}");
    };
    assert_eq!(spec.kind, PickKind::File);
    let filters: Vec<_> = spec
        .filters
        .iter()
        .map(|f| (f.name.as_str(), f.patterns.clone()))
        .collect();
    assert_eq!(
        filters,
        [
            (
                "SQLite database",
                vec![
                    "*.db".to_owned(),
                    "*.sqlite".into(),
                    "*.sqlite3".into(),
                    "*.db3".into()
                ]
            ),
            ("All files", vec!["*.*".to_owned()]),
        ]
    );
    assert_eq!(spec.start_dir.as_deref(), Some(env.db.parent().unwrap()));
    assert!(
        read(cx, &model, |m| m.error().is_none()),
        "a cancel changes nothing"
    );

    // A pick opens it.
    let second = second_database(&env);
    fake.answer_pick(Ok(Some(second.clone())));
    act(cx, &model, DbmanagerModel::open);
    idle(cx, &model);
    assert_eq!(snapshot(cx, &model).database.unwrap().path, second);
}

#[gpui::test]
fn open_dialog_error_shows_dialog_failure_copy(cx: &mut TestAppContext) {
    let (_env, model, fake) = started(cx);
    fake.answer_pick(Err(PickError::Failed {
        detail: "no shell".into(),
    }));
    act(cx, &model, DbmanagerModel::open);
    idle(cx, &model);
    let error = read(cx, &model, |m| m.error().cloned()).unwrap();
    assert_eq!(error.copy(), PICK_FAILED);
    assert_eq!(error.detail(), "no shell");
}

// ---- loading, failures ----

#[gpui::test]
fn loading_is_observable_with_closed_gate(cx: &mut TestAppContext) {
    let handle = cx.update(crate::jobs::WorkerGate::install_closed);
    let env = loaded();
    let (model, _) = start(cx, &env);
    cx.run_until_parked();
    assert!(read(cx, &model, |m| m.loading() && m.snapshot().is_none()));
    handle.open();
    idle(cx, &model);
    assert!(read(cx, &model, |m| !m.loading() && m.snapshot().is_some()));
}

/// Panics in `list` once.
struct Flaky {
    inner: FixtureDbmanager,
    panic_list: bool,
}

impl DbmanagerBackend for Flaky {
    fn session(&mut self) -> DbmanagerSnapshot {
        self.inner.session()
    }
    fn open(&mut self, path: &std::path::Path) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.inner.open(path)
    }
    fn open_recent(&mut self, index: usize) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.inner.open_recent(index)
    }
    fn open_dropped(&mut self, paths: Vec<PathBuf>) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.inner.open_dropped(paths)
    }
    fn reload(&mut self) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.inner.reload()
    }
    fn reload_config(&mut self) -> DbmanagerSnapshot {
        self.inner.reload_config()
    }
    fn edit_config_target(&mut self) -> Result<(PathBuf, bool), DbmanagerError> {
        self.inner.edit_config_target()
    }
    fn close(&mut self) -> DbmanagerSnapshot {
        self.inner.close()
    }
    fn list(
        &mut self,
        table: &str,
        query: &fm_dbmanager::session::ListQuery,
    ) -> Result<fm_dbmanager::session::ObjectPage, DbmanagerError> {
        assert!(!std::mem::take(&mut self.panic_list), "list blew up");
        self.inner.list(table, query)
    }
    fn object(
        &mut self,
        table: &str,
        key: &ObjectKey,
    ) -> Result<fm_dbmanager::session::ObjectView, DbmanagerError> {
        self.inner.object(table, key)
    }
    fn m2m_options(
        &mut self,
        table: &str,
        key: Option<&ObjectKey>,
        relation: &str,
        query: &fm_dbmanager::session::M2mQuery,
    ) -> Result<fm_dbmanager::session::M2mPage, DbmanagerError> {
        self.inner.m2m_options(table, key, relation, query)
    }
    fn save(
        &mut self,
        request: fm_dbmanager::session::SaveRequest,
    ) -> Result<fm_dbmanager::session::SaveResult, DbmanagerError> {
        self.inner.save(request)
    }
    fn delete_preview(
        &mut self,
        table: &str,
        key: &ObjectKey,
    ) -> Result<fm_dbmanager::session::DeleteImpact, DbmanagerError> {
        self.inner.delete_preview(table, key)
    }
    fn delete(
        &mut self,
        table: &str,
        key: &ObjectKey,
        version: &str,
        token: &str,
        config_hash: &str,
    ) -> Result<fm_dbmanager::session::DeleteResult, DbmanagerError> {
        self.inner.delete(table, key, version, token, config_hash)
    }
}

#[gpui::test]
fn a_panicking_read_is_an_unexpected_banner(cx: &mut TestAppContext) {
    let env = loaded();
    let flaky = Flaky {
        inner: env.backend(),
        panic_list: true,
    };
    let (model, _) = start_boxed(cx, Box::new(flaky));
    idle(cx, &model);
    let error = read(cx, &model, |m| m.error().cloned());
    assert!(
        matches!(error, Some(ViewError::Unexpected { .. })),
        "{error:?}"
    );
    // The worker still serves later calls.
    choose(cx, &model, "member");
    assert!(read(cx, &model, |m| {
        m.list_state("member").is_some_and(|l| l.page.is_some())
    }));
}

#[gpui::test]
fn a_read_error_is_a_banner(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    env.exec("DROP TABLE loan;");
    choose(cx, &model, "book");
    open_object(cx, &model, "book", 1);
    // The schema changed behind the session's back.
    let kind = read(cx, &model, |m| match m.error() {
        Some(ViewError::Dbmanager { error, .. }) => Some(error.kind),
        _ => None,
    });
    assert_eq!(kind, Some(DbmanagerErrorKind::SchemaChanged));
    assert_eq!(
        read(cx, &model, |m| m.object().cloned()),
        Some(ObjectLoad::Failed)
    );
}

// ---- the fixtures ----

fn state_of(cx: &mut TestAppContext, fixture: DbmanagerFixture) -> DbmanagerSnapshot {
    let env = FixtureEnv::new(fixture);
    let (model, _) = start(cx, &env);
    idle(cx, &model);
    snapshot(cx, &model)
}

#[gpui::test]
fn fixtures_cover_the_screenshot_states(cx: &mut TestAppContext) {
    use fm_dbmanager::config::ConfigStatus;

    let empty = state_of(cx, DbmanagerFixture::Empty);
    assert!(!empty.open && empty.database.is_none() && empty.recent.is_empty());
    assert_eq!(empty.config.status, ConfigStatus::Loaded);

    let loaded = state_of(cx, DbmanagerFixture::Loaded);
    assert!(loaded.open && loaded.state_warning.is_none());
    let shown = loaded.database.unwrap().display;
    assert!(
        shown.contains("fixture") || shown.contains("Fixture"),
        "{shown}"
    );
    assert!(shown.ends_with("library.db"));

    let missing = state_of(cx, DbmanagerFixture::ConfigMissing);
    assert!(missing.open);
    assert_eq!(missing.config.status, ConfigStatus::Missing);
    assert_eq!(
        text::config_notice(&missing).as_deref(),
        Some(text::CONFIG_MISSING)
    );

    let closed = state_of(cx, DbmanagerFixture::ConfigFailClosed);
    assert!(closed.open);
    let reason = closed.config_fail_closed.clone().expect("a reason");
    assert!(text::config_notice(&closed)
        .unwrap()
        .contains(reason.trim_end_matches('.')));
    assert!(closed.tables.iter().all(|t| !t.can_create));

    let warned = state_of(cx, DbmanagerFixture::StateWarning);
    assert!(!warned.open);
    assert!(warned.state_warning.is_some());
    assert_eq!(warned.recent.len(), 1, "the entry stays");
}

#[gpui::test]
fn fixture_dir_is_removed_with_the_backend(cx: &mut TestAppContext) {
    let env = loaded();
    let dir = env.dir().to_path_buf();
    let (model, _) = start(cx, &env);
    idle(cx, &model);
    drop(env);
    assert!(dir.exists(), "the model's backend keeps it alive");
    cx.update(|_| drop(model));
    cx.run_until_parked();
    // The worker owns the backend; give it a moment to stop.
    let deadline = Instant::now() + Duration::from_secs(5);
    while dir.exists() && Instant::now() < deadline {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!dir.exists(), "removed at the end");
}

// ---- the copy ----

#[test]
fn every_error_kind_has_a_sentence() {
    for kind in DbmanagerErrorKind::ALL {
        let e = DbmanagerError::new(kind, "detail");
        let copy = text::error_message(&e, Some("C:\\cfg\\dbmanager.toml"));
        assert!(copy.ends_with('.'), "{kind:?}: {copy}");
        assert!(copy.len() > 10, "{kind:?}");
        assert_ne!(copy, "detail");
    }
    let e = DbmanagerError::new(DbmanagerErrorKind::ConfigNotDroppable, "x");
    assert!(text::error_message(&e, Some("C:\\cfg\\dbmanager.toml"))
        .contains("C:\\cfg\\dbmanager.toml"));
    for kind in [
        DbmanagerErrorKind::ValidationFailed,
        DbmanagerErrorKind::Conflict,
        DbmanagerErrorKind::NotEditable,
        DbmanagerErrorKind::DatabaseReadOnly,
        DbmanagerErrorKind::BackupFailed,
        DbmanagerErrorKind::ConstraintFailed,
        DbmanagerErrorKind::ConfigChangedOnDisk,
    ] {
        let copy = text::error_message(&DbmanagerError::new(kind, "d"), None);
        assert!(copy.contains("Nothing was changed"), "{kind:?}: {copy}");
    }
}

#[test]
fn file_names_and_sentences() {
    assert_eq!(text::file_name("C:\\data\\library.db"), "library.db");
    assert_eq!(text::file_name("/a/b.db"), "b.db");
    assert_eq!(text::opened_sentence("C:\\x\\a.db"), "Opened a.db.");
    assert_eq!(
        text::database_read_only("the file is read-only"),
        "This database is read-only: the file is read-only."
    );
    assert_eq!(
        text::config_fail_closed("the config could not be read: gone."),
        "Everything is read-only: the config could not be read: gone."
    );
}

// ---- editing: helpers ----

use std::sync::{Arc, Mutex};

use fm_dbmanager::rules::FieldIssueCode;
use fm_dbmanager::session::{FieldInput, SaveRequest, SaveResult};
use fm_dbmanager::value::DbValue;

use super::draft::{EditorInput, Input};
use super::model::{Nav, Notice};

/// Forwards to a fixture backend, records every save request, and can answer
/// the first save `Busy`.
struct Recording {
    inner: FixtureDbmanager,
    saves: Arc<Mutex<Vec<SaveRequest>>>,
    busy_once: bool,
    /// Every delete call as (version, token, config hash).
    deletes: super::tests::DeleteLog,
    /// Answers the next delete with this error instead of running it.
    delete_error_once: Option<DbmanagerError>,
}

impl Recording {
    fn new(env: &FixtureEnv) -> (Self, Arc<Mutex<Vec<SaveRequest>>>) {
        let saves = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                inner: env.backend(),
                saves: saves.clone(),
                busy_once: false,
                deletes: Arc::new(Mutex::new(Vec::new())),
                delete_error_once: None,
            },
            saves,
        )
    }
}

impl DbmanagerBackend for Recording {
    fn session(&mut self) -> DbmanagerSnapshot {
        self.inner.session()
    }
    fn open(&mut self, path: &std::path::Path) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.inner.open(path)
    }
    fn open_recent(&mut self, index: usize) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.inner.open_recent(index)
    }
    fn open_dropped(&mut self, paths: Vec<PathBuf>) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.inner.open_dropped(paths)
    }
    fn reload(&mut self) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.inner.reload()
    }
    fn reload_config(&mut self) -> DbmanagerSnapshot {
        self.inner.reload_config()
    }
    fn edit_config_target(&mut self) -> Result<(PathBuf, bool), DbmanagerError> {
        self.inner.edit_config_target()
    }
    fn close(&mut self) -> DbmanagerSnapshot {
        self.inner.close()
    }
    fn list(
        &mut self,
        table: &str,
        query: &fm_dbmanager::session::ListQuery,
    ) -> Result<fm_dbmanager::session::ObjectPage, DbmanagerError> {
        self.inner.list(table, query)
    }
    fn object(
        &mut self,
        table: &str,
        key: &ObjectKey,
    ) -> Result<fm_dbmanager::session::ObjectView, DbmanagerError> {
        self.inner.object(table, key)
    }
    fn m2m_options(
        &mut self,
        table: &str,
        key: Option<&ObjectKey>,
        relation: &str,
        query: &fm_dbmanager::session::M2mQuery,
    ) -> Result<fm_dbmanager::session::M2mPage, DbmanagerError> {
        self.inner.m2m_options(table, key, relation, query)
    }
    fn save(&mut self, request: SaveRequest) -> Result<SaveResult, DbmanagerError> {
        self.saves.lock().unwrap().push(request.clone());
        if std::mem::take(&mut self.busy_once) {
            return Err(DbmanagerError::new(DbmanagerErrorKind::Busy, "locked"));
        }
        self.inner.save(request)
    }
    fn delete_preview(
        &mut self,
        table: &str,
        key: &ObjectKey,
    ) -> Result<fm_dbmanager::session::DeleteImpact, DbmanagerError> {
        self.inner.delete_preview(table, key)
    }
    fn delete(
        &mut self,
        table: &str,
        key: &ObjectKey,
        version: &str,
        token: &str,
        config_hash: &str,
    ) -> Result<fm_dbmanager::session::DeleteResult, DbmanagerError> {
        self.deletes.lock().unwrap().push((
            version.to_owned(),
            token.to_owned(),
            config_hash.to_owned(),
        ));
        if let Some(e) = self.delete_error_once.take() {
            return Err(e);
        }
        self.inner.delete(table, key, version, token, config_hash)
    }
}

type Attempt = Box<dyn Fn(&mut DbmanagerModel, &mut Context<DbmanagerModel>)>;

/// The book list and book `n` open, on a started model.
fn open_book(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>, n: i64) {
    choose(cx, model, "book");
    open_object(cx, model, "book", n);
}

fn type_in(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>, field: &str, text: &str) {
    act(cx, model, |m, cx| {
        m.edit(field, EditorInput::Text(text.to_owned()), cx);
    });
}

fn save_and_idle(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>) {
    act(cx, model, DbmanagerModel::save);
    idle(cx, model);
}

fn stored(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>, column: &str) -> DbValue {
    read(cx, model, |m| match m.object() {
        Some(ObjectLoad::Loaded(view)) => view
            .fields
            .iter()
            .find(|f| f.column == column)
            .map(|f| f.value.clone())
            .expect("the field"),
        other => panic!("no loaded object: {other:?}"),
    })
}

fn dirty(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>) -> bool {
    read(cx, model, DbmanagerModel::dirty)
}

fn error_kind(
    cx: &mut TestAppContext,
    model: &Entity<DbmanagerModel>,
) -> Option<DbmanagerErrorKind> {
    read(cx, model, |m| match m.error() {
        Some(ViewError::Dbmanager { error, .. }) => Some(error.kind),
        _ => None,
    })
}

fn issue_codes(
    cx: &mut TestAppContext,
    model: &Entity<DbmanagerModel>,
    field: &str,
) -> Vec<FieldIssueCode> {
    read(cx, model, |m| {
        m.draft()
            .map(|d| d.issues(field).iter().map(|i| i.code).collect())
            .unwrap_or_default()
    })
}

/// A new book with the required fields filled in (author picked).
fn fill_new_book(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>, isbn: &str) {
    act(cx, model, |m, cx| m.new_object("book", cx));
    idle(cx, model);
    type_in(cx, model, "title", "Created Book");
    type_in(cx, model, "isbn", isbn);
    act(cx, model, |m, cx| m.open_picker("author_id", cx));
    idle(cx, model);
    act(cx, model, |m, cx| m.pick(rowid(1), cx));
}

// ---- editing: validation and save ----

#[gpui::test]
fn edit_validates_on_change(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    assert!(!dirty(cx, &model));
    type_in(cx, &model, "pages", "abc");
    assert!(dirty(cx, &model));
    assert_eq!(
        issue_codes(cx, &model, "pages"),
        vec![FieldIssueCode::NotInteger]
    );
    assert!(read(cx, &model, |m| !m.draft().unwrap().shown("pages")));
    type_in(cx, &model, "pages", "321");
    assert_eq!(issue_codes(cx, &model, "pages").len(), 0);
    act(cx, &model, |m, cx| m.blur("pages", cx));
    type_in(cx, &model, "pages", "0");
    assert_eq!(
        issue_codes(cx, &model, "pages"),
        vec![FieldIssueCode::BelowMin],
        "the config's bounds apply"
    );
    assert!(read(cx, &model, |m| m.draft().unwrap().shown("pages")));
}

#[gpui::test]
fn save_refused_with_issues_shows_all(cx: &mut TestAppContext) {
    let env = loaded();
    let (backend, saves) = Recording::new(&env);
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "abc");
    type_in(cx, &model, "price", "x");
    hint(cx, &model);
    act(cx, &model, DbmanagerModel::save);
    idle(cx, &model);
    assert_eq!(hint(cx, &model), Some(FocusHint::ErrorSummary));
    assert!(saves.lock().unwrap().is_empty(), "nothing was sent");
    let shown = read(cx, &model, |m| m.draft().unwrap().summary().len());
    assert_eq!(shown, 2);
    assert!(!read(cx, &model, DbmanagerModel::saving));
    assert!(snapshot(cx, &model).backup_path.is_none());
}

#[gpui::test]
fn required_on_create_missing_refuses(cx: &mut TestAppContext) {
    let env = loaded();
    let (backend, saves) = Recording::new(&env);
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    choose(cx, &model, "book");
    act(cx, &model, |m, cx| m.new_object("book", cx));
    idle(cx, &model);
    hint(cx, &model);
    act(cx, &model, DbmanagerModel::save);
    idle(cx, &model);
    assert_eq!(hint(cx, &model), Some(FocusHint::ErrorSummary));
    assert!(saves.lock().unwrap().is_empty());
    for field in ["title", "isbn", "author_id"] {
        assert_eq!(
            issue_codes(cx, &model, field),
            vec![FieldIssueCode::Required],
            "{field}"
        );
    }
    assert!(matches!(place(cx, &model), Place::New { .. }));
}

#[gpui::test]
fn save_sends_changed_fields_with_hash(cx: &mut TestAppContext) {
    let env = loaded();
    let (backend, saves) = Recording::new(&env);
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "330");
    type_in(cx, &model, "price", "24.50");
    type_in(cx, &model, "price", "24.5");
    save_and_idle(cx, &model);
    let sent = saves.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].table, "book");
    assert_eq!(sent[0].config_hash, snapshot(cx, &model).config.hash);
    assert!(sent[0].version.is_some() && sent[0].key.is_some());
    assert_eq!(sent[0].changes.len(), 1, "price is not a change");
    assert_eq!(sent[0].changes[0].column, "pages");
    assert_eq!(
        sent[0].changes[0].input,
        FieldInput::Value {
            value: DbValue::Integer("330".into())
        }
    );
}

#[gpui::test]
fn saving_refuses_navigation_and_disables_drop(cx: &mut TestAppContext) {
    let env = loaded();
    let handle = cx.update(crate::jobs::WorkerGate::install_closed);
    let (model, _) = start(cx, &env);
    handle.open();
    idle(cx, &model);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "330");
    handle.close();
    act(cx, &model, DbmanagerModel::save);
    assert!(read(cx, &model, DbmanagerModel::saving));
    assert_eq!(
        read(cx, &model, DbmanagerModel::drop_availability),
        Err(text::REASON_SAVING)
    );
    let before = place(cx, &model);
    act(cx, &model, DbmanagerModel::back);
    act(cx, &model, |m, cx| m.choose_table("member", cx));
    act(cx, &model, |m, cx| m.new_object("author", cx));
    act(cx, &model, |m, cx| m.dropped(vec![env.db.clone()], cx));
    type_in(cx, &model, "pages", "999");
    assert_eq!(place(cx, &model), before);
    assert!(read(cx, &model, |m| m.leave_request().is_none()));
    assert_eq!(
        read(cx, &model, |m| m.draft().unwrap().value("pages")),
        Some(DbValue::Integer("330".into())),
        "editors are read-only while saving"
    );
    assert_eq!(
        read(cx, &model, DbmanagerModel::save_blocked_reason),
        Some(text::REASON_SAVING)
    );
    handle.open();
    idle(cx, &model);
    assert!(!read(cx, &model, DbmanagerModel::saving));
    assert_eq!(read(cx, &model, DbmanagerModel::drop_availability), Ok(()));
}

#[gpui::test]
fn save_success_clears_draft_and_rereads(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    // The list the user came from, read before the save.
    let label_before = read(cx, &model, |m| {
        m.list_state("book")
            .and_then(|l| l.page.as_ref())
            .and_then(|p| p.items.iter().find(|i| i.key == rowid(1)))
            .map(|i| i.label.clone())
    });
    assert_eq!(label_before.as_deref(), Some("The Quiet Harbour"));
    type_in(cx, &model, "title", "A New Title");
    type_in(cx, &model, "pages", "330");
    save_and_idle(cx, &model);
    assert!(!dirty(cx, &model));
    assert!(read(cx, &model, |m| m.draft().is_some()), "a clean draft");
    assert_eq!(stored(cx, &model, "pages"), DbValue::Integer("330".into()));
    assert_eq!(
        read(cx, &model, |m| m.announcement().map(str::to_owned)),
        Some("Saved.".into())
    );
    let label_after = read(cx, &model, |m| {
        m.list_state("book")
            .and_then(|l| l.page.as_ref())
            .and_then(|p| p.items.iter().find(|i| i.key == rowid(1)))
            .map(|i| i.label.clone())
    });
    assert_eq!(
        label_after.as_deref(),
        Some("A New Title"),
        "the list page was read again"
    );
    assert!(read(cx, &model, |m| m.error().is_none()
        && m.conflict().is_none()));
}

#[gpui::test]
fn first_save_shows_backup_notice(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_book(cx, &model, 1);
    assert!(read(cx, &model, |m| m.notice().is_none()));
    type_in(cx, &model, "pages", "330");
    save_and_idle(cx, &model);
    let notice = read(cx, &model, |m| m.notice().cloned());
    let Some(Notice::Backup { path }) = notice else {
        panic!("expected the backup notice, got {notice:?}");
    };
    assert!(path.contains(".bak"));
    let backup = snapshot(cx, &model)
        .backup_path
        .expect("the snapshot was read again");
    assert!(backup.exists());
    assert_eq!(
        std::fs::read_dir(env.dir())
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".bak"))
            .count(),
        1
    );
    // A second successful save clears the notice without a dismiss.
    type_in(cx, &model, "pages", "331");
    save_and_idle(cx, &model);
    assert!(
        read(cx, &model, |m| m.notice().is_none()),
        "only the first save has one, and the next save clears it"
    );
    act(cx, &model, DbmanagerModel::dismiss_notice);
    assert!(read(cx, &model, |m| m.notice().is_none()));
}

#[gpui::test]
fn server_validation_replaces_inline_until_edited(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    choose(cx, &model, "book");
    fill_new_book(cx, &model, "9780000000011");
    assert!(issue_codes(cx, &model, "isbn").is_empty(), "valid locally");
    save_and_idle(cx, &model);
    assert!(matches!(place(cx, &model), Place::New { .. }));
    assert_eq!(hint(cx, &model), Some(FocusHint::ErrorSummary));
    let shown = read(cx, &model, |m| {
        m.draft()
            .unwrap()
            .shown_issues("isbn")
            .iter()
            .map(|i| i.code)
            .collect::<Vec<_>>()
    });
    assert_eq!(shown, vec![FieldIssueCode::Unique]);
    type_in(cx, &model, "isbn", "9780000000105");
    assert!(read(cx, &model, |m| m
        .draft()
        .unwrap()
        .shown_issues("isbn")
        .is_empty()));
}

#[gpui::test]
fn conflict_panel_reapply_and_discard(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "title", "Mine");
    env.exec("UPDATE book SET title = 'Theirs', summary = 'edited elsewhere' WHERE id = 1");
    save_and_idle(cx, &model);
    let panel = read(cx, &model, |m| m.conflict().cloned()).expect("the conflict panel");
    assert_eq!(panel.label, "The Quiet Harbour");
    assert_eq!(panel.diff.fields.len(), 1);
    assert_eq!(panel.diff.fields[0].yours, "Mine");
    assert_eq!(panel.diff.fields[0].now, "Theirs");
    assert_eq!(panel.diff.others_changed, 1);
    assert!(dirty(cx, &model), "the draft is kept");
    // Save is refused until one is chosen.
    assert_eq!(
        read(cx, &model, DbmanagerModel::save_blocked_reason),
        Some("Resolve the conflict first.")
    );
    save_and_idle(cx, &model);
    assert!(read(cx, &model, |m| m.conflict().is_some()));
    // Re-apply: the draft sits on the new version and saves.
    act(cx, &model, DbmanagerModel::reapply);
    assert!(read(cx, &model, |m| m.conflict().is_none()));
    assert!(dirty(cx, &model));
    save_and_idle(cx, &model);
    assert_eq!(stored(cx, &model, "title"), DbValue::Text("Mine".into()));
    assert!(!dirty(cx, &model));
    // Discard: the other program's value is loaded.
    type_in(cx, &model, "title", "Mine again");
    env.exec("UPDATE book SET title = 'Theirs again' WHERE id = 1");
    save_and_idle(cx, &model);
    assert!(read(cx, &model, |m| m.conflict().is_some()));
    act(cx, &model, DbmanagerModel::discard_mine);
    assert!(read(cx, &model, |m| m.conflict().is_none()));
    assert!(!dirty(cx, &model));
    assert_eq!(
        stored(cx, &model, "title"),
        DbValue::Text("Theirs again".into())
    );
}

#[gpui::test]
fn config_changed_on_save_rechecks_without_retry(cx: &mut TestAppContext) {
    let env = loaded();
    let (backend, saves) = Recording::new(&env);
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "400");
    type_in(cx, &model, "title", "Kept edit");
    let old_hash = snapshot(cx, &model).config.hash;
    env.edit_config_book_pages_read_only();
    save_and_idle(cx, &model);
    assert_eq!(
        saves.lock().unwrap().len(),
        1,
        "never retried automatically"
    );
    assert_eq!(stored(cx, &model, "pages"), DbValue::Integer("320".into()));
    assert_eq!(
        stored(cx, &model, "title"),
        DbValue::Text("The Quiet Harbour".into())
    );
    assert_ne!(snapshot(cx, &model).config.hash, old_hash);
    let Some(Notice::ConfigChangedWhileEditing { dropped }) =
        read(cx, &model, |m| m.notice().cloned())
    else {
        panic!("expected the config-changed notice");
    };
    assert_eq!(dropped.len(), 1, "pages is read-only now: {dropped:?}");
    assert_eq!(
        read(cx, &model, DbmanagerModel::dirty_count),
        1,
        "the title edit stays"
    );
    assert!(read(cx, &model, |m| m
        .draft()
        .unwrap()
        .input("pages")
        .is_none()));
    // The next save carries the new hash and goes through.
    save_and_idle(cx, &model);
    assert_eq!(saves.lock().unwrap().len(), 2);
    assert_eq!(
        stored(cx, &model, "title"),
        DbValue::Text("Kept edit".into())
    );
}

#[gpui::test]
fn schema_changed_and_busy_banners(cx: &mut TestAppContext) {
    let env = loaded();
    let (backend, _saves) = Recording::new(&env);
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "330");
    env.exec("CREATE TABLE zz_new (a)");
    save_and_idle(cx, &model);
    assert_eq!(
        error_kind(cx, &model),
        Some(DbmanagerErrorKind::SchemaChanged)
    );
    assert!(dirty(cx, &model), "the draft is kept");
    assert!(!read(cx, &model, DbmanagerModel::can_try_again));
    // Busy offers Try again.
    let env = loaded();
    let (mut backend, _saves) = Recording::new(&env);
    backend.busy_once = true;
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "330");
    save_and_idle(cx, &model);
    assert_eq!(error_kind(cx, &model), Some(DbmanagerErrorKind::Busy));
    assert!(read(cx, &model, DbmanagerModel::can_try_again));
    assert!(dirty(cx, &model));
}

#[gpui::test]
fn busy_try_again_repeats_request(cx: &mut TestAppContext) {
    let env = loaded();
    let (mut backend, saves) = Recording::new(&env);
    backend.busy_once = true;
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "330");
    save_and_idle(cx, &model);
    act(cx, &model, DbmanagerModel::try_again);
    idle(cx, &model);
    let sent = saves.lock().unwrap().clone();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0], sent[1], "the same request");
    assert_eq!(stored(cx, &model, "pages"), DbValue::Integer("330".into()));
    assert!(read(cx, &model, |m| m.error().is_none() && !m.can_try_again()));
}

// ---- editing: leaving ----

#[gpui::test]
fn dirty_navigation_asks_and_confirm_discards(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_book(cx, &model, 1);
    let here = place(cx, &model);
    type_in(cx, &model, "pages", "330");
    // Every way out asks, and nothing moves.
    let attempts: Vec<(Attempt, Nav)> = vec![
        (
            Box::new(|m, cx| m.choose_table("member", cx)),
            Nav::Go(table("member")),
        ),
        (Box::new(DbmanagerModel::back), Nav::Back),
        (
            Box::new(|m, cx| m.open_object("book", rowid(2), cx)),
            Nav::Go(Place::Object {
                table: "book".into(),
                key: rowid(2),
                label: "Salt and Iron".into(),
            }),
        ),
        (
            Box::new(|m, cx| m.new_object("author", cx)),
            Nav::Go(Place::New {
                table: "author".into(),
            }),
        ),
        (Box::new(|m, cx| m.open_recent(0, cx)), Nav::OpenRecent(0)),
        (Box::new(DbmanagerModel::close_database), Nav::CloseDatabase),
        (Box::new(DbmanagerModel::reload), Nav::Reload),
        (
            Box::new({
                let db = env.db.clone();
                move |m, cx| m.open_path(db.clone(), cx)
            }),
            Nav::OpenPath(env.db.clone()),
        ),
        (
            Box::new({
                let db = env.db.clone();
                move |m, cx| m.dropped(vec![db.clone()], cx)
            }),
            Nav::Dropped(vec![env.db.clone()]),
        ),
    ];
    for (go, expected) in attempts {
        act(cx, &model, |m, cx| go(m, cx));
        idle(cx, &model);
        let request = read(cx, &model, |m| m.leave_request().cloned()).expect("asked");
        assert_eq!(request.label, "The Quiet Harbour");
        if let (Nav::Go(Place::Object { .. }), Nav::Go(Place::Object { table, key, .. })) =
            (&request.action, &expected)
        {
            assert!(
                matches!(&request.action, Nav::Go(Place::Object { table: t, key: k, .. }) if t == table && k == key)
            );
        } else {
            assert_eq!(request.action, expected);
        }
        assert_eq!(place(cx, &model), here);
        assert!(dirty(cx, &model));
    }
    // The reference cannot be followed here, but the fixture's author link
    // is a navigation too.
    act(cx, &model, |m, cx| m.choose_table("member", cx));
    act(cx, &model, DbmanagerModel::confirm_leave);
    idle(cx, &model);
    assert_eq!(place(cx, &model), table("member"));
    assert!(read(cx, &model, |m| m.draft().is_none()
        && m.leave_request().is_none()));
    // Back and Close ask too, and Close goes once confirmed.
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "331");
    act(cx, &model, DbmanagerModel::close_database);
    act(cx, &model, DbmanagerModel::confirm_leave);
    idle(cx, &model);
    assert!(!snapshot(cx, &model).open);
    assert_eq!(stored_pages_after_discard(&env), "320");
}

/// The database still holds the stored pages of book 1.
fn stored_pages_after_discard(env: &FixtureEnv) -> String {
    rusqlite::Connection::open(&env.db)
        .unwrap()
        .query_row("SELECT pages FROM book WHERE id = 1", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap()
        .to_string()
}

#[gpui::test]
fn following_a_reference_asks_when_dirty(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "330");
    act(cx, &model, |m, cx| m.follow("author_id", cx));
    assert!(read(cx, &model, |m| matches!(
        m.leave_request().map(|r| &r.action),
        Some(Nav::Go(Place::Object { table, .. })) if table == "author"
    )));
    act(cx, &model, DbmanagerModel::cancel_leave);
    act(cx, &model, DbmanagerModel::discard);
    act(cx, &model, |m, cx| m.follow("author_id", cx));
    idle(cx, &model);
    assert!(matches!(place(cx, &model), Place::Object { table, .. } if table == "author"));
}

#[gpui::test]
fn cancel_leave_keeps_draft(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "330");
    let here = place(cx, &model);
    act(cx, &model, |m, cx| m.choose_table("member", cx));
    assert!(read(cx, &model, |m| m.leave_request().is_some()));
    act(cx, &model, DbmanagerModel::cancel_leave);
    assert!(read(cx, &model, |m| m.leave_request().is_none()));
    assert!(dirty(cx, &model));
    assert_eq!(place(cx, &model), here);
    assert_eq!(
        read(cx, &model, |m| m.draft().unwrap().value("pages")),
        Some(DbValue::Integer("330".into()))
    );
    // Discard changes reverts without asking.
    act(cx, &model, DbmanagerModel::discard);
    assert!(!dirty(cx, &model));
    act(cx, &model, |m, cx| m.choose_table("member", cx));
    assert!(read(cx, &model, |m| m.leave_request().is_none()));
}

// ---- editing: create ----

#[gpui::test]
fn create_prefills_defaults_and_replaces_place(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    choose(cx, &model, "book");
    let history_before = read(cx, &model, |m| m.history().len());
    act(cx, &model, |m, cx| m.new_object("book", cx));
    idle(cx, &model);
    assert_eq!(hint(cx, &model), Some(FocusHint::SheetHeading));
    assert!(matches!(place(cx, &model), Place::New { .. }));
    read(cx, &model, |m| {
        let d = m.draft().expect("a draft");
        assert!(d.is_new() && !d.dirty());
        assert!(
            matches!(d.input("format"), Some(Input::Text(s) | Input::Choice(Some(s))) if s == "paperback")
        );
        assert!(matches!(d.input("in_print"), Some(Input::Bool(Some(true)))));
        assert!(
            d.input("revision").is_none(),
            "read-only fields have no editor"
        );
        assert!(d.input("isbn").is_some(), "create-only fields do");
    });
    act(cx, &model, DbmanagerModel::close_picker);
    act(cx, &model, DbmanagerModel::cancel_leave);
    // Fill in the required fields and create.
    type_in(cx, &model, "title", "Created Book");
    type_in(cx, &model, "isbn", "9780000000105");
    act(cx, &model, |m, cx| m.open_picker("author_id", cx));
    idle(cx, &model);
    act(cx, &model, |m, cx| m.pick(rowid(1), cx));
    hint(cx, &model);
    save_and_idle(cx, &model);
    assert_eq!(hint(cx, &model), Some(FocusHint::SheetHeading));
    let here = place(cx, &model);
    assert!(
        matches!(&here, Place::Object { table, label, .. } if table == "book" && label == "Created Book")
    );
    assert_eq!(
        read(cx, &model, |m| m.history().len()),
        history_before + 1,
        "the New entry was replaced, not pushed"
    );
    assert!(read(cx, &model, |m| !m.draft().unwrap().is_new()));
    assert_eq!(
        read(cx, &model, |m| m.announcement().map(str::to_owned)),
        Some("Created Created Book.".into())
    );
    assert_eq!(
        stored(cx, &model, "format"),
        DbValue::Text("paperback".into())
    );
    // The list the user came from has it.
    assert!(read(cx, &model, |m| m
        .list_state("book")
        .and_then(|l| l.page.as_ref())
        .is_some_and(|p| p
            .items
            .iter()
            .any(|i| i.label == "Created Book"))));
}

#[gpui::test]
fn new_button_noun_uses_object_name(cx: &mut TestAppContext) {
    assert_eq!(text::new_button(Some("book")), "New book");
    assert_eq!(text::new_button(None), "New");
    assert_eq!(text::new_button(Some("  ")), "New");
    assert_eq!(text::new_title(Some("book")), "New book");
    assert_eq!(text::new_title(None), "New object");
    let (_env, model, _) = started(cx);
    let (with, without) = read(cx, &model, |m| {
        (
            m.table_view("book").and_then(|t| t.object_name.clone()),
            m.table_view("book").map(|t| t.can_create),
        )
    });
    assert_eq!(with.as_deref(), Some("book"));
    assert_eq!(without, Some(true));
    act(cx, &model, |m, cx| m.new_object("book", cx));
    idle(cx, &model);
    assert_eq!(read(cx, &model, DbmanagerModel::draft_label), "New book");
    // A table that cannot create refuses.
    assert!(!read(cx, &model, |m| m.can_create("publisher")));
    let before = place(cx, &model);
    act(cx, &model, |m, cx| m.new_object("publisher", cx));
    assert_eq!(place(cx, &model), before);
}

#[gpui::test]
fn cancel_new_goes_back(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    choose(cx, &model, "book");
    act(cx, &model, |m, cx| m.new_object("book", cx));
    idle(cx, &model);
    type_in(cx, &model, "title", "Half a title");
    assert!(dirty(cx, &model));
    act(cx, &model, DbmanagerModel::cancel_new);
    idle(cx, &model);
    assert_eq!(place(cx, &model), table("book"));
    assert!(read(cx, &model, |m| m.draft().is_none()
        && m.leave_request().is_none()));
}

// ---- editing: inputs and the picker ----

#[gpui::test]
fn edit_inputs_convert_per_kind(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    let value = |cx: &mut TestAppContext, field: &str| {
        read(cx, &model, |m| m.draft().unwrap().value(field))
    };
    type_in(cx, &model, "pages", " 12 ");
    assert_eq!(value(cx, "pages"), Some(DbValue::Integer("12".into())));
    type_in(cx, &model, "pages", "");
    assert_eq!(
        value(cx, "pages"),
        Some(DbValue::Null),
        "empty nullable is not set"
    );
    type_in(cx, &model, "price", " 2.5");
    assert_eq!(value(cx, "price"), Some(DbValue::Real("2.5".into())));
    type_in(cx, &model, "title", "  padded  ");
    assert_eq!(
        value(cx, "title"),
        Some(DbValue::Text("  padded  ".into())),
        "text is not trimmed"
    );
    act(cx, &model, |m, cx| {
        m.edit("in_print", EditorInput::Bool(Some(false)), cx);
    });
    assert_eq!(value(cx, "in_print"), Some(DbValue::Integer("0".into())));
    act(cx, &model, |m, cx| {
        m.edit("in_print", EditorInput::Bool(None), cx);
    });
    assert_eq!(
        issue_codes(cx, &model, "in_print"),
        vec![FieldIssueCode::Required]
    );
    act(cx, &model, |m, cx| {
        m.edit("format", EditorInput::Choice(Some("ebook".into())), cx);
    });
    assert_eq!(value(cx, "format"), Some(DbValue::Text("ebook".into())));
    act(cx, &model, |m, cx| {
        m.edit("format", EditorInput::Choice(None), cx);
    });
    assert_eq!(
        issue_codes(cx, &model, "format"),
        vec![FieldIssueCode::Required]
    );
    // An input of the wrong kind for a field is ignored.
    act(cx, &model, |m, cx| {
        m.edit("title", EditorInput::Bool(Some(true)), cx);
    });
    assert_eq!(value(cx, "title"), Some(DbValue::Text("  padded  ".into())));
    // A duration is canonical once it loses focus, and 1h 30m over 7h30m is a change.
    type_in(cx, &model, "audio_length", "7h 30m");
    assert!(
        !read(cx, &model, |m| m.draft().unwrap().changed("audio_length")),
        "7h30m is stored"
    );
    type_in(cx, &model, "audio_length", "8h 15m");
    act(cx, &model, |m, cx| m.blur("audio_length", cx));
    assert!(read(cx, &model, |m| matches!(
        m.draft().unwrap().input("audio_length"),
        Some(Input::Text(s)) if s == "8h15m"
    )));
    // Escape reverts one field.
    act(cx, &model, |m, cx| m.revert_field("title", cx));
    assert!(!read(cx, &model, |m| m.draft().unwrap().changed("title")));
    // Stored-value problems never block other fields: nothing else to check
    // here, a clean field has no issues.
    assert_eq!(issue_codes(cx, &model, "summary").len(), 0);
}

#[gpui::test]
fn picker_has_its_own_query_and_keeps_list_state(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    choose(cx, &model, "book");
    act(cx, &model, |m, cx| m.set_search("book", "Salt", cx));
    idle(cx, &model);
    open_object(cx, &model, "book", 2);
    let list_before = read(cx, &model, |m| m.list_state("book").cloned());
    act(cx, &model, |m, cx| m.open_picker("author_id", cx));
    idle(cx, &model);
    let picker = read(cx, &model, |m| m.picker().cloned()).expect("the picker");
    assert_eq!(picker.table, "author");
    assert_eq!(picker.field, "author_id");
    assert_eq!((picker.items.len(), picker.total), (3, 3));
    assert!(!picker.loading && picker.error.is_none());
    // The newest search wins, and the table's own list is untouched.
    act(cx, &model, |m, cx| m.picker_search("Tomas", cx));
    act(cx, &model, |m, cx| m.picker_search("Ada", cx));
    idle(cx, &model);
    let picker = read(cx, &model, |m| m.picker().cloned()).unwrap();
    assert_eq!(picker.search, "Ada");
    assert_eq!(picker.items.len(), 1);
    assert_eq!(
        read(cx, &model, |m| m.list_state("book").cloned()),
        list_before
    );
    // Nothing more to show.
    act(cx, &model, DbmanagerModel::picker_more);
    idle(cx, &model);
    assert_eq!(read(cx, &model, |m| m.picker().unwrap().items.len()), 1);
    act(cx, &model, DbmanagerModel::close_picker);
    assert_eq!(
        hint(cx, &model),
        Some(FocusHint::PickerTrigger("author_id".into()))
    );
    assert!(read(cx, &model, |m| m.picker().is_none()));
    assert!(!dirty(cx, &model), "closing picks nothing");
}

#[gpui::test]
fn picker_pages_by_25(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    env.exec(
        "WITH RECURSIVE n(i) AS (SELECT 10 UNION ALL SELECT i + 1 FROM n WHERE i < 69) \
         INSERT INTO author (id, first_name, last_name) SELECT i, 'F', 'L' || i FROM n;",
    );
    open_book(cx, &model, 1);
    act(cx, &model, |m, cx| m.open_picker("author_id", cx));
    idle(cx, &model);
    let (loaded_now, total) = read(cx, &model, |m| {
        let p = m.picker().unwrap();
        (p.items.len(), p.total)
    });
    assert_eq!((loaded_now, total), (25, 63));
    act(cx, &model, DbmanagerModel::picker_more);
    idle(cx, &model);
    assert_eq!(read(cx, &model, |m| m.picker().unwrap().items.len()), 50);
}

#[gpui::test]
fn pick_stages_reference_and_hints_trigger(cx: &mut TestAppContext) {
    let env = loaded();
    let (backend, saves) = Recording::new(&env);
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    open_book(cx, &model, 1);
    act(cx, &model, |m, cx| m.open_picker("author_id", cx));
    idle(cx, &model);
    hint(cx, &model);
    act(cx, &model, |m, cx| m.pick(rowid(2), cx));
    assert_eq!(
        hint(cx, &model),
        Some(FocusHint::PickerTrigger("author_id".into()))
    );
    assert!(read(cx, &model, |m| m.picker().is_none()));
    assert!(read(cx, &model, |m| m
        .draft()
        .unwrap()
        .changed("author_id")));
    assert!(read(cx, &model, |m| matches!(
        m.draft().unwrap().input("author_id"),
        Some(Input::Reference(Some(p))) if p.key == rowid(2) && p.label == "Okafor, Tomas"
    )));
    save_and_idle(cx, &model);
    let sent = saves.lock().unwrap().clone();
    assert_eq!(sent[0].changes.len(), 1);
    assert_eq!(
        sent[0].changes[0].input,
        FieldInput::Reference {
            key: Some(rowid(2))
        }
    );
}

#[gpui::test]
fn clear_reference_only_when_nullable(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    act(cx, &model, |m, cx| m.clear_reference("author_id", cx));
    assert!(!dirty(cx, &model), "author is required");
    act(cx, &model, |m, cx| m.clear_reference("publisher_id", cx));
    assert!(read(cx, &model, |m| m
        .draft()
        .unwrap()
        .changed("publisher_id")));
    assert!(matches!(
        read(cx, &model, |m| m
            .draft()
            .unwrap()
            .input("publisher_id")
            .cloned()),
        Some(Input::Reference(None))
    ));
    assert_eq!(read(cx, &model, DbmanagerModel::dirty_count), 1);
}

// ---- editing: the close guard ----

#[gpui::test]
fn unsaved_work_reports_draft_for_close_guard(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    let hooks = hooks(cx, &model);
    assert_eq!(cx.update(|cx| hooks.unsaved_work(cx)).len(), 0);
    type_in(cx, &model, "pages", "330");
    let items = cx.update(|cx| hooks.unsaved_work(cx));
    assert_eq!(
        items,
        vec![gpui::SharedString::from(
            "Database Manager: your changes to \u{201c}The Quiet Harbour\u{201d} are not saved."
        )]
    );
    act(cx, &model, DbmanagerModel::discard);
    assert_eq!(cx.update(|cx| hooks.unsaved_work(cx)).len(), 0);
}

#[gpui::test]
fn close_guard_holds_window_with_dirty_draft(cx: &mut TestAppContext) {
    use crate::app::shell::test_support::open;
    use crate::tools::{self, ToolSlot};

    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "330");
    let given = model.clone();
    let (shell, vcx) = open(cx, move |window, app| {
        tools::build(window, app)
            .into_iter()
            .map(|slot| {
                if slot.id == tools::ToolId::Dbmanager {
                    ToolSlot {
                        hooks: Box::new(Hooks {
                            model: given.clone(),
                        }),
                        ..slot
                    }
                } else {
                    slot
                }
            })
            .collect()
    });
    assert!(!vcx.simulate_close(), "held");
    let request = shell.read_with(vcx, |s, _| s.close_request().cloned());
    assert_eq!(
        request.map(|r| r.items),
        Some(vec![(
            tools::ToolId::Dbmanager,
            "Database Manager: your changes to \u{201c}The Quiet Harbour\u{201d} are not saved."
                .into()
        )])
    );
}

// ---- editing: reloads ----

#[gpui::test]
fn reload_rebases_draft_and_drops_read_only_fields(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "400");
    type_in(cx, &model, "title", "Edited");
    cx.update(|cx| model.update(cx, DbmanagerModel::inject_file_changed));
    idle(cx, &model);
    let Some(Notice::ConfigReloaded { dropped }) = read(cx, &model, |m| m.notice().cloned()) else {
        panic!("expected the reload notice");
    };
    assert_eq!(dropped.len(), 1);
    assert!(read(cx, &model, |m| m.notice().unwrap().copy())
        .starts_with("The config was reloaded. These changes no longer apply and were removed: "));
    assert_eq!(
        read(cx, &model, DbmanagerModel::dirty_count),
        1,
        "the title edit stays"
    );
    assert!(read(cx, &model, |m| m
        .draft()
        .unwrap()
        .input("pages")
        .is_none()));
}

#[gpui::test]
fn reload_with_changed_object_raises_the_conflict(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "title", "Mine");
    env.exec("UPDATE book SET title = 'Theirs' WHERE id = 1");
    act(cx, &model, DbmanagerModel::recheck_after_reload);
    idle(cx, &model);
    let panel = read(cx, &model, |m| m.conflict().cloned()).expect("a conflict");
    assert_eq!(panel.diff.fields[0].now, "Theirs");
    assert!(dirty(cx, &model));
    assert_eq!(
        read(cx, &model, |m| m.notice().unwrap().copy()),
        "The config was reloaded."
    );
}

#[gpui::test]
fn reload_with_gone_object_drops_the_draft(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_book(cx, &model, 4);
    type_in(cx, &model, "title", "Mine");
    env.exec("DELETE FROM book WHERE id = 4");
    act(cx, &model, DbmanagerModel::recheck_after_reload);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.draft().is_none()));
    assert!(matches!(
        read(cx, &model, |m| m.object().cloned()),
        Some(ObjectLoad::Gone)
    ));
    assert!(matches!(
        read(cx, &model, |m| m.notice().cloned()),
        Some(Notice::ConfigReloadedGone { .. })
    ));
}

// ---- opening ----

#[gpui::test]
fn racing_open_still_raises_list_heading_hint(cx: &mut TestAppContext) {
    // Through the gate: open(A) is queued, then a fetch; they run in order.
    let env = FixtureEnv::new(DbmanagerFixture::Empty);
    let handle = cx.update(crate::jobs::WorkerGate::install_closed);
    let (model, _) = start(cx, &env);
    act(cx, &model, |m, cx| m.open_path(env.db.clone(), cx));
    act(cx, &model, DbmanagerModel::activated);
    handle.open();
    idle(cx, &model);
    assert_eq!(hint(cx, &model), Some(FocusHint::ListHeading));

    // Deterministically: the fetch's snapshot is applied first, so the
    // open's own snapshot is dropped as older, but the database changed.
    let env = FixtureEnv::new(DbmanagerFixture::Empty);
    let (model, _) = start(cx, &env);
    idle(cx, &model);
    act(cx, &model, |m, cx| m.open_path(env.db.clone(), cx));
    idle(cx, &model);
    let mut opened = snapshot(cx, &model);
    opened.database.as_mut().unwrap().path = env.dir().join("other.db");
    hint(cx, &model);
    act(cx, &model, |m, cx| m.race_open_for_tests(&opened, cx));
    idle(cx, &model);
    assert_eq!(hint(cx, &model), Some(FocusHint::ListHeading));
    assert!(snapshot(cx, &model).open);
}

// ---- many-to-many staging ----

use super::draft::{LinkStage, RebaseOutcome};
use super::model::{DeleteState, StagedRow};

const GENRES: &str = "m2m:book_genre";
const CONTRIB: &str = "m2m:contribution";

fn gk(code: &str) -> ObjectKey {
    ObjectKey::Primary {
        values: vec![DbValue::Text(code.into())],
    }
}

fn db_count(env: &FixtureEnv, sql: &str) -> i64 {
    rusqlite::Connection::open(&env.db)
        .unwrap()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}

fn toggle(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>, rel: &str, key: &ObjectKey) {
    act(cx, model, |m, cx| m.toggle_link(rel, key, cx));
}

fn edit_link(
    cx: &mut TestAppContext,
    model: &Entity<DbmanagerModel>,
    rel: &str,
    key: &ObjectKey,
    field: &str,
    text: &str,
) {
    act(cx, model, |m, cx| {
        m.edit_link_field(rel, key, field, EditorInput::Text(text.to_owned()), cx);
    });
}

fn rows(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>, rel: &str) -> Vec<StagedRow> {
    read(cx, model, |m| m.staged_rows(rel))
}

#[gpui::test]
fn counts_line_reflects_staging(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 2);
    let line = |cx: &mut TestAppContext| read(cx, &model, |m| m.link_counts(GENRES).line());
    assert_eq!(line(cx), "2 linked");
    assert!(!dirty(cx, &model));
    toggle(cx, &model, GENRES, &gk("poetry"));
    toggle(cx, &model, GENRES, &gk("sci-fi"));
    toggle(cx, &model, GENRES, &gk("fiction"));
    assert_eq!(line(cx), "2 linked \u{b7} 2 to add \u{b7} 1 to remove");
    assert!(dirty(cx, &model), "staged links make the draft dirty");
    assert_eq!(read(cx, &model, DbmanagerModel::dirty_count), 3);
    assert!(read(cx, &model, |m| m.unsaved_description().is_some()));
    act(cx, &model, |m, cx| m.undo_link(GENRES, &gk("poetry"), cx));
    toggle(cx, &model, GENRES, &gk("fiction"));
    assert_eq!(line(cx), "2 linked \u{b7} 1 to add");
    assert_eq!(read(cx, &model, DbmanagerModel::dirty_count), 1);
}

#[gpui::test]
fn staged_rows_off_page_shown_once(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    toggle(cx, &model, GENRES, &gk("history"));
    act(cx, &model, |m, cx| {
        m.set_m2m_query(GENRES, "Poetry", false, 0, cx);
    });
    idle(cx, &model);
    toggle(cx, &model, GENRES, &gk("poetry"));
    let staged = rows(cx, &model, GENRES);
    assert_eq!(staged.len(), 2, "each staged row once: {staged:?}");
    let history = staged.iter().find(|r| r.other == gk("history")).unwrap();
    let poetry = staged.iter().find(|r| r.other == gk("poetry")).unwrap();
    assert!(history.off_page() && !history.on_page);
    assert!(poetry.on_page);
    assert_eq!(history.stage, LinkStage::Add);
    assert_eq!(history.label, "History");
    // The off-page row can be undone from its group.
    act(cx, &model, |m, cx| m.undo_link(GENRES, &gk("history"), cx));
    assert_eq!(rows(cx, &model, GENRES).len(), 1);
}

#[gpui::test]
fn removal_with_fields_is_announced_before_save(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_book(cx, &model, 2);
    toggle(cx, &model, CONTRIB, &rowid(3));
    let staged = rows(cx, &model, CONTRIB);
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].stage, LinkStage::Remove);
    let note = staged[0].removal_note.clone().expect("a removal note");
    assert!(
        note.starts_with("Will be removed with its fields: "),
        "{note}"
    );
    assert!(note.contains("Translator") && note.contains('1'), "{note}");
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM contribution WHERE book_id = 2"),
        1,
        "nothing is written before Save"
    );
    // A link with no fields has no note.
    toggle(cx, &model, GENRES, &gk("fiction"));
    assert!(rows(cx, &model, GENRES)[0].removal_note.is_none());
    // Ticking again undoes it.
    toggle(cx, &model, CONTRIB, &rowid(3));
    assert_eq!(rows(cx, &model, CONTRIB).len(), 0);
}

#[gpui::test]
fn save_sends_links_with_fields_in_one_request(cx: &mut TestAppContext) {
    let env = loaded();
    let (backend, saves) = Recording::new(&env);
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "333");
    toggle(cx, &model, GENRES, &gk("poetry"));
    toggle(cx, &model, GENRES, &gk("fiction"));
    toggle(cx, &model, CONTRIB, &rowid(3));
    edit_link(cx, &model, CONTRIB, &rowid(3), "position", "2");
    edit_link(cx, &model, CONTRIB, &rowid(2), "position", "4");
    save_and_idle(cx, &model);
    let sent = saves.lock().unwrap().clone();
    assert_eq!(sent.len(), 1, "one request");
    let request = &sent[0];
    assert_eq!(request.changes.len(), 1);
    let genres = request.links.iter().find(|l| l.relation == GENRES).unwrap();
    assert_eq!(genres.add[0].other, gk("poetry"));
    assert_eq!(genres.remove[0].other, gk("fiction"));
    assert!(genres.remove[0].version.is_some(), "carries the version");
    let contrib = request
        .links
        .iter()
        .find(|l| l.relation == CONTRIB)
        .unwrap();
    assert_eq!(contrib.add[0].other, rowid(3));
    assert_eq!(contrib.add[0].fields.len(), 1, "only the edited field");
    assert_eq!(contrib.update[0].other, rowid(2));
    assert_ne!(contrib.update[0].version, "");
    assert!(read(cx, &model, |m| m.error().is_none()));
    assert!(!dirty(cx, &model), "staging is cleared by the save");
    assert_eq!(rows(cx, &model, GENRES).len(), 0);
    assert_eq!(
        db_count(
            &env,
            "SELECT COUNT(*) FROM book_genre WHERE book_id = 1 AND genre_code = 'poetry'"
        ),
        1
    );
    assert_eq!(
        db_count(
            &env,
            "SELECT COUNT(*) FROM book_genre WHERE book_id = 1 AND genre_code = 'fiction'"
        ),
        0
    );
    assert_eq!(
        db_count(
            &env,
            "SELECT position FROM contribution WHERE book_id = 1 AND author_id = 3"
        ),
        2
    );
    assert_eq!(
        db_count(
            &env,
            "SELECT position FROM contribution WHERE book_id = 1 AND author_id = 2"
        ),
        4
    );
    assert_eq!(read(cx, &model, |m| m.link_counts(GENRES).linked), 1);
}

#[gpui::test]
fn link_errors_attach_to_rows(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_book(cx, &model, 1);
    toggle(cx, &model, GENRES, &gk("poetry"));
    // The genre goes away behind the model's back.
    env.exec("DELETE FROM book_genre WHERE genre_code = 'poetry'; DELETE FROM genre WHERE code = 'poetry'");
    save_and_idle(cx, &model);
    assert_eq!(hint(cx, &model), Some(FocusHint::ErrorSummary));
    assert!(read(cx, &model, |m| m.error().is_none()), "no banner");
    let issues = read(cx, &model, |m| {
        m.staged_link(GENRES, &gk("poetry"))
            .map(|l| l.link_issues().len())
    });
    assert_eq!(issues, Some(1), "the row carries the link-level issue");
    let summary = read(cx, &model, |m| m.draft().unwrap().links().summary());
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].other, gk("poetry"));
    assert!(dirty(cx, &model), "the change is kept");
}

#[gpui::test]
fn save_with_link_conflict_is_handled_not_bannered(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_book(cx, &model, 1);
    edit_link(cx, &model, CONTRIB, &rowid(2), "position", "3");
    env.exec("UPDATE contribution SET role = 'translator' WHERE book_id = 1 AND author_id = 2");
    save_and_idle(cx, &model);
    assert!(read(cx, &model, |m| m.error().is_none()), "no banner");
    assert!(read(cx, &model, |m| m.conflict().is_none()));
    let conflict = read(cx, &model, |m| m.link_conflict(CONTRIB).cloned()).expect("handled");
    assert_eq!(conflict.other, rowid(2));
    assert!(conflict.notice().ends_with(
        "link was changed by another program. Nothing was saved. Your change to it was set aside."
    ));
}

#[gpui::test]
fn link_conflict_keeps_other_edits_and_offers_reapply(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "331");
    toggle(cx, &model, GENRES, &gk("poetry"));
    edit_link(cx, &model, CONTRIB, &rowid(2), "position", "3");
    env.exec("UPDATE contribution SET role = 'translator' WHERE book_id = 1 AND author_id = 2");
    save_and_idle(cx, &model);
    assert_eq!(
        db_count(&env, "SELECT pages FROM book WHERE id = 1"),
        320,
        "nothing was saved"
    );
    let conflict = read(cx, &model, |m| m.link_conflict(CONTRIB).cloned()).unwrap();
    assert_eq!(conflict.rebased.outcome, RebaseOutcome::Staged);
    assert!(read(cx, &model, |m| m
        .staged_link(CONTRIB, &rowid(2))
        .is_none()));
    assert!(read(cx, &model, |m| m.link_conflict(GENRES).is_none()));
    assert_eq!(
        read(cx, &model, DbmanagerModel::dirty_count),
        2,
        "the pages edit and the genre add stay"
    );
    // Re-apply: an update against the fresh version, then it saves.
    act(cx, &model, |m, cx| m.reapply_link(CONTRIB, cx));
    assert!(read(cx, &model, |m| m.link_conflict(CONTRIB).is_none()));
    let staged = read(cx, &model, |m| {
        m.staged_link(CONTRIB, &rowid(2))
            .map(super::draft::StagedLink::stage)
    });
    assert_eq!(staged, Some(LinkStage::Update));
    save_and_idle(cx, &model);
    assert!(read(cx, &model, |m| m.error().is_none()));
    assert_eq!(
        db_count(
            &env,
            "SELECT position FROM contribution WHERE book_id = 1 AND author_id = 2"
        ),
        3
    );
    assert_eq!(db_count(&env, "SELECT pages FROM book WHERE id = 1"), 331);
    assert_eq!(
        db_count(
            &env,
            "SELECT COUNT(*) FROM contribution WHERE book_id = 1 AND role = 'translator'"
        ),
        1,
        "the other program's role stands"
    );
    // Discard it: the change is dropped and the rest saves.
    edit_link(cx, &model, CONTRIB, &rowid(2), "position", "9");
    env.exec("UPDATE contribution SET role = 'illustrator' WHERE book_id = 1 AND author_id = 2");
    save_and_idle(cx, &model);
    assert!(read(cx, &model, |m| m.link_conflict(CONTRIB).is_some()));
    act(cx, &model, |m, cx| m.discard_link(CONTRIB, cx));
    assert!(read(cx, &model, |m| m.link_conflict(CONTRIB).is_none()));
    assert!(!dirty(cx, &model));
    // An add that finds the link already there, with no values: a notice.
    toggle(cx, &model, CONTRIB, &rowid(3));
    env.exec("INSERT INTO contribution (book_id, author_id, role) VALUES (1, 3, 'editor')");
    save_and_idle(cx, &model);
    assert!(read(cx, &model, |m| m.link_conflict(CONTRIB).is_some()));
    act(cx, &model, |m, cx| m.reapply_link(CONTRIB, cx));
    assert_eq!(
        read(cx, &model, |m| m.notice().map(Notice::copy)),
        Some(text::LINK_ALREADY_LINKED.to_owned())
    );
    assert!(!dirty(cx, &model), "nothing left to stage");
    act(cx, &model, DbmanagerModel::dismiss_notice);
    assert!(read(cx, &model, |m| m.notice().is_none()));
}

#[gpui::test]
fn create_forms_stage_adds_too(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    choose(cx, &model, "book");
    fill_new_book(cx, &model, "9780000000130");
    let page = read(cx, &model, |m| m.m2m(GENRES).and_then(|s| s.page.clone()));
    assert_eq!(page.expect("a page for the new object").linked_total, 0);
    toggle(cx, &model, GENRES, &gk("poetry"));
    assert_eq!(
        read(cx, &model, |m| m.link_counts(GENRES).line()),
        "0 linked \u{b7} 1 to add"
    );
    save_and_idle(cx, &model);
    assert!(read(cx, &model, |m| m.error().is_none()));
    assert!(matches!(place(cx, &model), Place::Object { .. }));
    assert_eq!(
        db_count(
            &env,
            "SELECT COUNT(*) FROM book_genre WHERE genre_code = 'poetry' AND book_id = (SELECT id FROM book WHERE isbn = '9780000000130')"
        ),
        1
    );
    assert_eq!(read(cx, &model, |m| m.link_counts(GENRES).linked), 1);
}

// ---- delete ----

fn delete_dialog(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>) -> DeleteState {
    read(cx, model, |m| m.delete_state().cloned()).expect("a delete state")
}

type DeleteLog = Arc<Mutex<Vec<(String, String, String)>>>;

fn member_recording(cx: &mut TestAppContext) -> (FixtureEnv, Entity<DbmanagerModel>, DeleteLog) {
    let env = loaded();
    let (backend, _) = Recording::new(&env);
    let deletes = backend.deletes.clone();
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    (env, model, deletes)
}

#[gpui::test]
fn delete_previews_then_confirms(cx: &mut TestAppContext) {
    let (env, model, deletes) = member_recording(cx);
    open_object(cx, &model, "member", 2);
    act(cx, &model, DbmanagerModel::request_delete);
    assert!(matches!(
        read(cx, &model, |m| m.delete_state().cloned()),
        Some(DeleteState::Checking { .. })
    ));
    idle(cx, &model);
    let DeleteState::Confirm(dialog) = delete_dialog(cx, &model) else {
        panic!("expected the confirmation");
    };
    assert_eq!(dialog.label, "Jonas Weber");
    assert!(!dialog.discards_draft);
    assert!(dialog.impact.groups.iter().any(|g| g.table == "loan"));
    assert!(deletes.lock().unwrap().is_empty(), "nothing before Confirm");
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM member WHERE id = 2"),
        1
    );
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    let calls = deletes.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, dialog.impact.version);
    assert_eq!(calls[0].1, dialog.impact.token);
    assert_eq!(calls[0].2, snapshot(cx, &model).config.hash);
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM member WHERE id = 2"),
        0
    );
    assert!(read(cx, &model, |m| m.delete_state().is_none()));
}

#[gpui::test]
fn blocked_delete_offers_only_close(cx: &mut TestAppContext) {
    let env = FixtureEnv::new(DbmanagerFixture::DeleteBlocked);
    let (backend, _) = Recording::new(&env);
    let deletes = backend.deletes.clone();
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    open_object(cx, &model, "publisher", 1);
    assert!(read(cx, &model, |m| m.can_delete("publisher")));
    act(cx, &model, DbmanagerModel::request_delete);
    idle(cx, &model);
    let DeleteState::Blocked(dialog) = delete_dialog(cx, &model) else {
        panic!("expected the blocked state");
    };
    assert_ne!(dialog.blocking_groups().len(), 0);
    assert_eq!(dialog.blocking_groups()[0].table, "book");
    assert!(!dialog.generic_block());
    // Confirm does nothing when blocked; Close ends it.
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    assert!(deletes.lock().unwrap().is_empty());
    assert!(matches!(
        read(cx, &model, |m| m.delete_state().cloned()),
        Some(DeleteState::Blocked(_))
    ));
    act(cx, &model, DbmanagerModel::cancel_delete);
    assert!(read(cx, &model, |m| m.delete_state().is_none()));
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM publisher WHERE id = 1"),
        1
    );
}

#[gpui::test]
fn unpredicted_block_shows_the_generic_line(cx: &mut TestAppContext) {
    let env = loaded();
    let (mut backend, _) = Recording::new(&env);
    backend.delete_error_once = Some(DbmanagerError::new(
        DbmanagerErrorKind::DeleteBlocked,
        "refused",
    ));
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    open_object(cx, &model, "member", 2);
    act(cx, &model, DbmanagerModel::request_delete);
    idle(cx, &model);
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    let DeleteState::Blocked(dialog) = delete_dialog(cx, &model) else {
        panic!("expected the blocked state");
    };
    assert!(dialog.generic_block());
    assert_eq!(
        text::DELETE_BLOCKED_GENERIC,
        "This object cannot be deleted."
    );
}

#[gpui::test]
fn impact_changed_keeps_dialog_with_new_token(cx: &mut TestAppContext) {
    let (env, model, deletes) = member_recording(cx);
    open_object(cx, &model, "member", 2);
    act(cx, &model, DbmanagerModel::request_delete);
    idle(cx, &model);
    let DeleteState::Confirm(first) = delete_dialog(cx, &model) else {
        panic!("expected the confirmation");
    };
    let _ = hint(cx, &model);
    env.exec(
        "INSERT INTO loan (id, book_id, member_id, loan_period, returned) VALUES (9, 1, 2, 100, 0)",
    );
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    let DeleteState::Confirm(second) = delete_dialog(cx, &model) else {
        panic!("stays open");
    };
    assert_eq!(
        second.notice,
        Some(super::model::DeleteDialogNotice::ImpactChanged)
    );
    assert_eq!(
        second.notice.unwrap().copy(),
        "What this delete affects changed. Review it again."
    );
    assert_ne!(second.impact.token, first.impact.token);
    assert_eq!(hint(cx, &model), Some(FocusHint::DeleteNotice));
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM member WHERE id = 2"),
        1
    );
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    let calls = deletes.lock().unwrap().clone();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].1, second.impact.token, "the new token");
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM member WHERE id = 2"),
        0
    );
}

#[gpui::test]
fn delete_conflict_shows_current(cx: &mut TestAppContext) {
    let (env, model, _) = member_recording(cx);
    open_object(cx, &model, "member", 2);
    act(cx, &model, DbmanagerModel::request_delete);
    idle(cx, &model);
    env.exec("UPDATE member SET name = 'Changed Name' WHERE id = 2");
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.delete_state().is_none()), "closes");
    assert!(matches!(place(cx, &model), Place::Object { .. }));
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM member WHERE id = 2"),
        1
    );
    assert_eq!(
        read(cx, &model, |m| m.notice().map(Notice::copy)),
        Some(
            "\u{201c}Jonas Weber\u{201d} was changed by another program. Nothing was deleted. Review it and try again."
                .to_owned()
        )
    );
    assert_eq!(
        stored(cx, &model, "name"),
        DbValue::Text("Changed Name".into()),
        "the sheet shows the current object"
    );
    assert!(read(cx, &model, |m| m.error().is_none()));
    assert!(read(cx, &model, |m| m.conflict().is_none()), "clean draft");
    // A dirty draft is kept, and the conflict panel is raised against current.
    type_in(cx, &model, "name", "My Edit");
    act(cx, &model, DbmanagerModel::request_delete);
    idle(cx, &model);
    env.exec("UPDATE member SET name = 'Changed Again' WHERE id = 2");
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.delete_state().is_none()), "closes");
    assert!(
        matches!(
            read(cx, &model, |m| m.notice().cloned()),
            Some(Notice::DeleteConflict { .. })
        ),
        "the notice"
    );
    assert!(dirty(cx, &model), "the edits survive");
    let panel = read(cx, &model, |m| m.conflict().cloned()).expect("the conflict panel");
    assert!(
        panel
            .diff
            .fields
            .iter()
            .any(|f| f.yours == "My Edit" && f.now == "Changed Again"),
        "against current"
    );
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM member WHERE id = 2"),
        1
    );
}

#[gpui::test]
fn delete_config_changed_reruns_preview(cx: &mut TestAppContext) {
    let (env, model, deletes) = member_recording(cx);
    open_object(cx, &model, "member", 2);
    act(cx, &model, DbmanagerModel::request_delete);
    idle(cx, &model);
    let old_hash = snapshot(cx, &model).config.hash;
    env.edit_config_book_pages_read_only();
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    assert_eq!(deletes.lock().unwrap().len(), 1, "never retried");
    let DeleteState::Confirm(dialog) = delete_dialog(cx, &model) else {
        panic!("stays open on the new preview");
    };
    assert_eq!(
        dialog.notice.map(super::model::DeleteDialogNotice::copy),
        Some("The config changed and was reloaded. Review the delete again.")
    );
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM member WHERE id = 2"),
        1
    );
    assert_ne!(snapshot(cx, &model).config.hash, old_hash);
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    let calls = deletes.lock().unwrap().clone();
    assert_eq!(calls.len(), 2);
    assert_ne!(calls[1].2, old_hash, "the new hash");
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM member WHERE id = 2"),
        0
    );
}

#[gpui::test]
fn delete_config_changed_rechecks_draft(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "331");
    act(cx, &model, DbmanagerModel::request_delete);
    idle(cx, &model);
    env.edit_config_book_pages_read_only();
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    assert!(
        read(cx, &model, |m| m.draft().unwrap().input("pages").is_none()),
        "the draft follows the new rules"
    );
}

#[gpui::test]
fn save_blocked_while_link_reread_in_flight(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "331");
    assert_eq!(read(cx, &model, DbmanagerModel::save_blocked_reason), None);
    act(cx, &model, |m, cx| {
        m.begin_link_conflict(
            fm_dbmanager::session::LinkRef {
                relation: CONTRIB.to_owned(),
                other: rowid(2),
            },
            cx,
        );
    });
    assert_eq!(
        read(cx, &model, DbmanagerModel::save_blocked_reason),
        Some(text::REASON_LINK_CHECK)
    );
    idle(cx, &model);
    assert_eq!(read(cx, &model, DbmanagerModel::save_blocked_reason), None);
}

#[gpui::test]
fn delete_success_replaces_place_and_notices(cx: &mut TestAppContext) {
    let (env, model, _) = member_recording(cx);
    open_object(cx, &model, "member", 2);
    let history = read(cx, &model, |m| m.history().len());
    act(cx, &model, DbmanagerModel::request_delete);
    idle(cx, &model);
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    assert_eq!(place(cx, &model), table("member"), "replaced by the table");
    assert_eq!(read(cx, &model, |m| m.history().len()), history);
    assert!(read(cx, &model, |m| m.object().is_none()));
    let Some(Notice::Deleted {
        label,
        related,
        backup,
        ..
    }) = read(cx, &model, |m| m.notice().cloned())
    else {
        panic!("expected the deleted notice");
    };
    assert_eq!(label, "Jonas Weber");
    assert_eq!(related, 1, "one loan was cleared");
    assert!(backup.is_some_and(|p| p.contains(".bak")), "first write");
    let copy = read(cx, &model, |m| m.notice().map(Notice::copy)).unwrap();
    assert!(
        copy.starts_with("Deleted \u{201c}Jonas Weber\u{201d} and 1 related row."),
        "{copy}"
    );
    let page = read(cx, &model, |m| {
        m.list_state("member").and_then(|l| l.page.clone())
    })
    .unwrap();
    assert!(page.items.iter().all(|i| i.key != rowid(2)), "list re-read");
    assert!(snapshot(cx, &model).backup_path.is_some());
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM member WHERE id = 2"),
        0
    );
}

#[gpui::test]
fn delete_with_dirty_draft_warns(cx: &mut TestAppContext) {
    let (_env, model, _) = member_recording(cx);
    open_object(cx, &model, "member", 2);
    type_in(cx, &model, "name", "Edited");
    act(cx, &model, DbmanagerModel::request_delete);
    idle(cx, &model);
    let DeleteState::Confirm(dialog) = delete_dialog(cx, &model) else {
        panic!("expected the confirmation");
    };
    assert!(dialog.discards_draft);
    assert_eq!(
        text::DELETE_DISCARDS_DRAFT,
        "Your unsaved changes to it are discarded."
    );
    // Cancel keeps the draft.
    act(cx, &model, DbmanagerModel::cancel_delete);
    assert!(dirty(cx, &model));
    act(cx, &model, DbmanagerModel::request_delete);
    idle(cx, &model);
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    assert_eq!(place(cx, &model), table("member"));
    assert!(!dirty(cx, &model));
}

#[gpui::test]
fn pickable_tables_never_delete(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_object(cx, &model, "publisher", 1);
    assert!(!read(cx, &model, |m| m.can_delete("publisher")));
    act(cx, &model, DbmanagerModel::request_delete);
    assert!(read(cx, &model, |m| m.delete_state().is_none()));
    act(cx, &model, |m, cx| {
        m.request_delete_of("publisher", rowid(2), "Harbour Books", cx);
    });
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.delete_state().is_none()));
    // A table that is not deletable in the config never does either.
    assert!(!read(cx, &model, |m| m.can_delete("genre")));
}

#[gpui::test]
fn request_delete_of_list_item_keeps_place(cx: &mut TestAppContext) {
    let (env, model, _) = member_recording(cx);
    choose(cx, &model, "member");
    act(cx, &model, |m, cx| {
        m.request_delete_of("member", rowid(3), "Lena Fischer", cx);
    });
    idle(cx, &model);
    let DeleteState::Confirm(dialog) = delete_dialog(cx, &model) else {
        panic!("expected the confirmation");
    };
    assert!(!dialog.discards_draft);
    assert_eq!(dialog.label, "Lena Fischer");
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    assert_eq!(place(cx, &model), table("member"), "the place stays");
    let page = read(cx, &model, |m| {
        m.list_state("member").and_then(|l| l.page.clone())
    })
    .unwrap();
    assert!(page.items.iter().all(|i| i.key != rowid(3)), "list re-read");
    assert!(matches!(
        read(cx, &model, |m| m.notice().cloned()),
        Some(Notice::Deleted { .. })
    ));
    assert_eq!(
        db_count(&env, "SELECT COUNT(*) FROM member WHERE id = 3"),
        0
    );
}

#[gpui::test]
fn delete_notice_hint_is_raised(cx: &mut TestAppContext) {
    let (_env, model, _) = member_recording(cx);
    choose(cx, &model, "member");
    act(cx, &model, |m, cx| {
        m.request_delete_of("member", rowid(3), "Lena Fischer", cx);
    });
    idle(cx, &model);
    let _ = hint(cx, &model);
    act(cx, &model, DbmanagerModel::confirm_delete);
    idle(cx, &model);
    assert_eq!(hint(cx, &model), Some(FocusHint::DeleteNotice));
}

// ---- M12c follow-up: leaving cancels a link re-read ----

#[gpui::test]
fn leaving_cancels_link_reread(cx: &mut TestAppContext) {
    let env = loaded();
    let handle = cx.update(crate::jobs::WorkerGate::install_closed);
    let (model, _) = start(cx, &env);
    handle.open();
    idle(cx, &model);
    open_book(cx, &model, 1);
    handle.close();
    // 1. The re-read of a link conflict starts and waits at the gate.
    act(cx, &model, |m, cx| {
        m.begin_link_conflict(
            fm_dbmanager::session::LinkRef {
                relation: CONTRIB.to_owned(),
                other: rowid(2),
            },
            cx,
        );
    });
    assert_eq!(
        read(cx, &model, DbmanagerModel::save_blocked_reason),
        Some(text::REASON_LINK_CHECK)
    );
    // 2. The user goes to a new book, whose draft exists at once.
    act(cx, &model, |m, cx| m.new_object("book", cx));
    assert!(matches!(place(cx, &model), Place::New { .. }));
    assert_eq!(
        read(cx, &model, DbmanagerModel::save_blocked_reason),
        None,
        "the re-read no longer holds this place's save"
    );
    // 3. The gate opens: the old answer arrives and is dropped.
    handle.open();
    idle(cx, &model);
    toggle(cx, &model, CONTRIB, &rowid(2));
    assert!(read(cx, &model, |m| m
        .staged_link(CONTRIB, &rowid(2))
        .is_some()));
    assert!(read(cx, &model, |m| m.link_conflict(CONTRIB).is_none()));
    assert!(read(cx, &model, |m| m.draft().is_some()));
    assert_eq!(read(cx, &model, DbmanagerModel::save_blocked_reason), None);
}

// ---- the config file: watcher, reload, Edit config ----

fn config_hash(cx: &mut TestAppContext, model: &Entity<DbmanagerModel>) -> String {
    snapshot(cx, model).config.hash
}

/// Pumps the executor until `done` holds (events from the real watcher come
/// from another thread).
fn wait_until(
    cx: &mut TestAppContext,
    model: &Entity<DbmanagerModel>,
    what: &str,
    done: impl Fn(&DbmanagerModel) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        cx.run_until_parked();
        if read(cx, model, |m| done(m) && !m.busy()) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[gpui::test]
fn first_snapshot_arms_watcher_once(cx: &mut TestAppContext) {
    // Off unless asked: a plain fixture model never arms.
    let (_env, plain, _) = started(cx);
    assert_eq!(read(cx, &plain, |m| m.watch_arms), 0);

    let env = loaded();
    cx.executor().allow_parking();
    let fake = FakePlatform::new();
    cx.update(|cx| platform::set_platform(cx, fake));
    let backend = Box::new(env.backend());
    let model = cx.new(|cx| DbmanagerModel::new(backend, cx).with_config_watcher());
    idle(cx, &model);
    wait_until(cx, &model, "the watcher", DbmanagerModel::config_watched);
    assert_eq!(read(cx, &model, |m| m.watch_arms), 1);
    // Later snapshots do not arm it again.
    act(cx, &model, DbmanagerModel::fetch_snapshot);
    idle(cx, &model);
    assert_eq!(read(cx, &model, |m| m.watch_arms), 1);
    cx.update(|_| drop(model));
    cx.run_until_parked();
}

#[gpui::test]
fn no_config_path_never_arms_watcher(cx: &mut TestAppContext) {
    let env = loaded();
    cx.executor().allow_parking();
    let fake = FakePlatform::new();
    cx.update(|cx| platform::set_platform(cx, fake));
    let backend = Box::new(env.backend_without_config_path());
    let model = cx.new(|cx| DbmanagerModel::new(backend, cx).with_config_watcher());
    idle(cx, &model);
    assert!(snapshot(cx, &model).config.path.is_none());
    act(cx, &model, DbmanagerModel::fetch_snapshot);
    idle(cx, &model);
    assert_eq!(read(cx, &model, |m| m.watch_arms), 0);
    assert!(!read(cx, &model, DbmanagerModel::config_watched));
}

#[gpui::test]
fn config_edit_reloads_and_rechecks_draft(cx: &mut TestAppContext) {
    let (env, model, _) = started(cx);
    act(cx, &model, DbmanagerModel::arm_config_watcher);
    wait_until(cx, &model, "the watcher", DbmanagerModel::config_watched);
    // Let the watch settle before the edit.
    std::thread::sleep(Duration::from_millis(300));
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "400");
    type_in(cx, &model, "title", "Edited");
    let before = config_hash(cx, &model);
    env.edit_config_book_pages_read_only();
    wait_until(cx, &model, "the reload notice", |m| m.notice().is_some());
    assert_ne!(config_hash(cx, &model), before);
    let Some(Notice::ConfigReloaded { dropped }) = read(cx, &model, |m| m.notice().cloned()) else {
        panic!("expected the reload notice");
    };
    assert_eq!(dropped.len(), 1);
    assert_eq!(read(cx, &model, DbmanagerModel::dirty_count), 1);
    assert!(read(cx, &model, |m| m.config_reloaded_notice().is_some()));
    // Drop the model (and its watcher) before the temp dir goes.
    cx.update(|_| drop(model));
    cx.run_until_parked();
}

#[gpui::test]
fn inject_file_changed_reloads(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    let before = config_hash(cx, &model);
    cx.update(|cx| model.update(cx, DbmanagerModel::inject_file_changed));
    idle(cx, &model);
    assert_ne!(config_hash(cx, &model), before);
    assert_eq!(
        read(cx, &model, |m| m.table_view("book").and_then(|t| t
            .fields
            .iter()
            .find(|f| f.column == "pages")
            .map(|f| f.mode))),
        Some(fm_dbmanager::config::FieldMode::ReadOnly)
    );
    assert!(
        snapshot(cx, &model).open,
        "the database stays open, not reopened"
    );
    // The same hash again changes nothing and raises no notice.
    act(cx, &model, DbmanagerModel::dismiss_notice);
    cx.update(|cx| model.update(cx, DbmanagerModel::config_file_changed));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.notice().is_none()));
}

#[gpui::test]
fn reload_drops_edits_that_no_longer_apply(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "400");
    cx.update(|cx| model.update(cx, DbmanagerModel::inject_file_changed));
    idle(cx, &model);
    let copy = read(cx, &model, DbmanagerModel::config_reloaded_notice).expect("a notice");
    assert!(
        copy.starts_with(
            "The config was reloaded. These changes no longer apply and were removed: "
        ),
        "{copy}"
    );
    assert!(!dirty(cx, &model));
}

#[gpui::test]
fn reload_waits_for_running_save(cx: &mut TestAppContext) {
    let env = loaded();
    let handle = cx.update(crate::jobs::WorkerGate::install_closed);
    let (model, _) = start(cx, &env);
    handle.open();
    idle(cx, &model);
    open_book(cx, &model, 1);
    type_in(cx, &model, "title", "Saved first");
    handle.close();
    act(cx, &model, DbmanagerModel::save);
    cx.update(|cx| model.update(cx, DbmanagerModel::inject_file_changed));
    cx.run_until_parked();
    assert!(read(cx, &model, DbmanagerModel::saving));
    // Only the save may run: the config edit and reload wait behind it.
    handle.release_one();
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(&env.config).unwrap(),
        include_str!("../../../../../examples/dbmanager/example.toml"),
        "the config is not touched while the save is queued ahead"
    );
    handle.open();
    idle(cx, &model);
    assert_eq!(
        db_count(
            &env,
            "SELECT COUNT(*) FROM book WHERE title = 'Saved first'"
        ),
        1
    );
    assert!(read(cx, &model, |m| m.error().is_none()));
    assert!(read(cx, &model, |m| !m.saving()));
}

#[gpui::test]
fn edit_config_needs_dev_mode(cx: &mut TestAppContext) {
    use crate::app::dev_mode::DevMode;

    let (env, model, fake) = started(cx);
    cx.update(DevMode::init);
    assert!(!cx.update(|cx| model.read(cx).can_edit_config(cx)));
    act(cx, &model, DbmanagerModel::edit_config);
    idle(cx, &model);
    assert!(fake.calls().is_empty(), "nothing opened");
    assert!(read(cx, &model, |m| m.notice().is_none()));
    cx.update(DevMode::toggle);
    assert!(cx.update(|cx| model.read(cx).can_edit_config(cx)));
    drop(env);
}

#[gpui::test]
fn edit_config_opens_with_system_and_notices_creation(cx: &mut TestAppContext) {
    use crate::app::dev_mode::DevMode;

    let env = FixtureEnv::new(DbmanagerFixture::ConfigMissing);
    let (model, fake) = start(cx, &env);
    idle(cx, &model);
    cx.update(DevMode::init);
    cx.update(DevMode::toggle);
    assert!(!env.config.exists());
    act(cx, &model, DbmanagerModel::edit_config);
    idle(cx, &model);
    assert!(env.config.is_file(), "the starter was created");
    assert_eq!(
        fake.calls(),
        vec![PlatformCall::OpenWithSystem(env.config.clone())]
    );
    let copy = read(cx, &model, |m| m.notice().map(Notice::copy)).expect("a notice");
    assert!(copy.starts_with("Created a starter config at "), "{copy}");
    assert!(copy.contains("dbmanager.toml"), "{copy}");
    // An existing file is opened as it is, with no notice.
    act(cx, &model, DbmanagerModel::dismiss_notice);
    let text = std::fs::read_to_string(&env.config).unwrap();
    act(cx, &model, DbmanagerModel::edit_config);
    idle(cx, &model);
    assert_eq!(fake.calls().len(), 2);
    assert!(read(cx, &model, |m| m.notice().is_none()));
    assert_eq!(std::fs::read_to_string(&env.config).unwrap(), text);
}

#[gpui::test]
fn edit_config_opener_failure_shows_opener_failed(cx: &mut TestAppContext) {
    use crate::app::dev_mode::DevMode;
    use crate::platform::OpenError;

    let (_env, model, fake) = started(cx);
    cx.update(DevMode::init);
    cx.update(DevMode::toggle);
    fake.answer_open(Err(OpenError {
        detail: "no handler for .toml".to_owned(),
    }));
    act(cx, &model, DbmanagerModel::edit_config);
    idle(cx, &model);
    let Some(ViewError::Dbmanager { error, .. }) = read(cx, &model, |m| m.error().cloned()) else {
        panic!("expected the banner");
    };
    assert_eq!(error.kind, DbmanagerErrorKind::OpenerFailed);
    assert_eq!(error.message, "no handler for .toml");
}

#[gpui::test]
fn fixtures_build_diagnostics_and_readonly_config_states(cx: &mut TestAppContext) {
    let diagnostics = state_of(cx, DbmanagerFixture::Diagnostics);
    assert!(diagnostics.open);
    assert_eq!(diagnostics.error_count, 1, "{:?}", diagnostics.diagnostics);
    assert_eq!(
        diagnostics.warning_count, 1,
        "{:?}",
        diagnostics.diagnostics
    );

    let readonly = state_of(cx, DbmanagerFixture::ReadonlyConfig);
    assert!(readonly.open);
    assert!(readonly.config_fail_closed.is_some());
    assert!(readonly.tables.iter().all(|t| !t.can_create));
    assert!(text::config_notice(&readonly).is_some());
}

#[test]
fn problems_copy_text() {
    assert_eq!(
        text::problems_summary(2, 1),
        "Config: 2 errors, 1 warning. Entries with errors are read-only."
    );
    assert_eq!(text::problems_summary(0, 2), "Config: 2 warnings.");
    let d = |line, col, table: Option<&str>, column: Option<&str>| ConfigDiagnostic {
        severity: DiagnosticSeverity::Error,
        message: String::new(),
        line,
        col,
        table: table.map(Into::into),
        column: column.map(Into::into),
    };
    assert_eq!(
        text::position(&d(Some(12), Some(3), None, None)),
        "Line 12, column 3"
    );
    assert_eq!(
        text::position(&d(None, None, Some("book"), Some("isbn"))),
        "Table book, field isbn"
    );
    assert_eq!(text::position(&d(None, None, None, None)), "");
}

#[gpui::test]
fn state_warning_dismissal_sticks_until_it_changes(cx: &mut TestAppContext) {
    let env = FixtureEnv::new(DbmanagerFixture::StateWarning);
    let (model, _) = start(cx, &env);
    idle(cx, &model);
    let warning = read(cx, &model, |m| m.state_warning().map(String::from)).expect("a warning");
    act(cx, &model, DbmanagerModel::dismiss_state_warning);
    assert_eq!(
        read(cx, &model, |m| m.state_warning().map(String::from)),
        None
    );
    // The same text stays dismissed across a reload.
    act(cx, &model, DbmanagerModel::reload);
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m.state_warning().map(String::from)),
        None
    );
    // A different text shows again.
    act(cx, &model, |m, _| {
        m.state_warning_dismissed = Some(format!("not {warning}"));
    });
    assert_eq!(
        read(cx, &model, |m| m.state_warning().map(String::from)),
        Some(warning)
    );
}

#[gpui::test]
fn escape_ignored_while_shortcuts_open(cx: &mut TestAppContext) {
    let (_env, model, _) = started(cx);
    open_book(cx, &model, 1);
    type_in(cx, &model, "pages", "330");
    act(cx, &model, |m, cx| m.choose_table("member", cx));
    assert!(read(cx, &model, |m| m.leave_request().is_some()));
    act(cx, &model, |m, cx| m.set_shortcuts_open(true, cx));
    act(cx, &model, DbmanagerModel::cancel_leave);
    assert!(
        read(cx, &model, |m| m.leave_request().is_some()),
        "the help took the key"
    );
    act(cx, &model, |m, cx| m.set_shortcuts_open(false, cx));
    act(cx, &model, DbmanagerModel::cancel_leave);
    assert!(read(cx, &model, |m| m.leave_request().is_none()));
    // A notice is held the same way.
    act(cx, &model, |m, cx| {
        m.notice = Some(Notice::LinkAlreadyGone);
        m.set_shortcuts_open(true, cx);
    });
    act(cx, &model, DbmanagerModel::dismiss_notice);
    assert!(read(cx, &model, |m| m.notice().is_some()));
    act(cx, &model, |m, cx| m.set_shortcuts_open(false, cx));
    act(cx, &model, DbmanagerModel::dismiss_notice);
    assert!(read(cx, &model, |m| m.notice().is_none()));
}
