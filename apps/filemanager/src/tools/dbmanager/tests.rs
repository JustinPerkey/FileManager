//! The Database Manager model's tests: restoring, history, lists, one object
//! with its "referenced by" and many-to-many pages, opening, dropping, the
//! copy and the fixtures. Model tests run against a [`FixtureEnv`] (a real
//! core over a database in a temp dir) and [`FakePlatform`].

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use fm_dbmanager::config::TableMode;
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
