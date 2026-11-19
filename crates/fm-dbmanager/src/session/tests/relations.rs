use super::*;

fn count(o: &ObjectView, relation_prefix: &str) -> u64 {
    o.referenced_by
        .iter()
        .chain(&o.many_to_many)
        .find(|r| r.relation.starts_with(relation_prefix))
        .unwrap_or_else(|| panic!("no relation {relation_prefix}"))
        .count
}

#[test]
fn relation_counts() {
    let fx = Fixture::example();
    let sess = fx.session();
    let a1 = sess.object("author", &rk(1)).unwrap();
    assert_eq!(a1.referenced_by.len(), 1);
    assert_eq!(count(&a1, "fk:book:"), 2);
    assert_eq!(count(&a1, "m2m:contribution"), 1);
    let b2 = sess.object("book", &rk(2)).unwrap();
    assert_eq!(count(&b2, "fk:loan:"), 1);
    assert_eq!(count(&b2, "m2m:book_genre"), 2);
    assert_eq!(count(&b2, "m2m:contribution"), 1);
    let b4 = sess.object("book", &rk(4)).unwrap();
    assert_eq!(count(&b4, "fk:loan:"), 0);
    assert_eq!(count(&b4, "m2m:book_genre"), 0);
    let genre = sess.object("genre", &pk("fiction")).unwrap();
    assert_eq!(count(&genre, "m2m:book_genre"), 2);
    // order matches the table view
    let view = sess
        .table_views()
        .into_iter()
        .find(|t| t.name == "book")
        .unwrap();
    let rels: Vec<&str> = b2
        .referenced_by
        .iter()
        .map(|r| r.relation.as_str())
        .collect();
    let want: Vec<&str> = view
        .referenced_by
        .iter()
        .map(|r| r.relation.as_str())
        .collect();
    assert_eq!(rels, want);
}

fn linked(p: &M2mPage) -> Vec<&str> {
    p.items
        .iter()
        .filter(|i| i.linked)
        .map(|i| i.label.as_str())
        .collect()
}

#[test]
fn m2m_options_all_and_linked() {
    let fx = Fixture::example();
    let sess = fx.session();
    let all = sess
        .m2m_options("book", Some(&rk(2)), "m2m:book_genre", &mq(None, 0, 50))
        .unwrap();
    let names: Vec<&str> = all.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(names, ["Fiction", "History", "Poetry", "Science fiction"]);
    assert_eq!(linked(&all), ["Fiction", "History"]);
    assert_eq!((all.total, all.linked_total), (4, 2));
    assert_eq!(all.items[0].key, pk("fiction"));
    for i in &all.items {
        assert_eq!(i.link_values.is_some(), i.linked);
        assert_eq!(i.link_version.is_some(), i.linked);
        if i.linked {
            assert_eq!(i.link_values, Some(vec![]));
        }
    }
    let only = sess
        .m2m_options(
            "book",
            Some(&rk(2)),
            "m2m:book_genre",
            &M2mQuery {
                linked_only: true,
                ..mq(None, 0, 50)
            },
        )
        .unwrap();
    assert_eq!((only.total, only.linked_total), (2, 2));
    assert!(only.items.iter().all(|i| i.linked));
    // search and paging apply to the other side
    let page = sess
        .m2m_options("book", Some(&rk(2)), "m2m:book_genre", &mq(Some("I"), 1, 1))
        .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.offset, 1);
    // from the genre side
    let g = sess
        .m2m_options(
            "genre",
            Some(&pk("fiction")),
            "m2m:book_genre",
            &mq(None, 0, 50),
        )
        .unwrap();
    assert_eq!(g.total, 5);
    assert_eq!(linked(&g), ["Salt and Iron", "The Quiet Harbour"]);
}

#[test]
fn m2m_options_new_object() {
    let fx = Fixture::example();
    let sess = fx.session();
    let p = sess
        .m2m_options("book", None, "m2m:book_genre", &mq(None, 0, 50))
        .unwrap();
    assert_eq!((p.total, p.linked_total), (4, 0));
    assert!(p.items.iter().all(|i| !i.linked && i.link_values.is_none()));
    let p = sess
        .m2m_options(
            "book",
            None,
            "m2m:book_genre",
            &M2mQuery {
                linked_only: true,
                ..mq(None, 0, 50)
            },
        )
        .unwrap();
    assert_eq!(p.items.len(), 0);
    assert_eq!(p.total, 0);
}

#[test]
fn m2m_options_link_values() {
    let fx = Fixture::example();
    let sess = fx.session();
    let p = sess
        .m2m_options("book", Some(&rk(1)), "m2m:contribution", &mq(None, 0, 50))
        .unwrap();
    let names: Vec<&str> = p.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(
        names,
        ["Castellan, Mira", "Lindqvist, Ada", "Okafor, Tomas"]
    );
    assert_eq!(linked(&p), ["Okafor, Tomas"]);
    let okafor = &p.items[2];
    let values = okafor.link_values.as_ref().unwrap();
    let cols: Vec<&str> = values.iter().map(|v| v.column.as_str()).collect();
    assert_eq!(cols, ["role", "position"]);
    assert_eq!(values[0].value, DbValue::Text("editor".into()));
    assert_eq!(values[1].value, DbValue::Null);
    assert!(okafor.link_version.is_some());
    // from the author side, the same link
    let a = sess
        .m2m_options("author", Some(&rk(2)), "m2m:contribution", &mq(None, 0, 50))
        .unwrap();
    let book1 = a.items.iter().find(|i| i.key == rk(1)).unwrap();
    assert!(book1.linked);
    assert_eq!(book1.link_values, okafor.link_values);
    assert_eq!(book1.link_version, okafor.link_version);
    // a link field with a problem is reported like an object field
    fx.exec("UPDATE contribution SET role = 'ghost' WHERE book_id = 1;");
    let sess = fx.session();
    let p = sess
        .m2m_options("book", Some(&rk(1)), "m2m:contribution", &mq(None, 0, 50))
        .unwrap();
    let v = p.items[2].link_values.as_ref().unwrap();
    assert_eq!(v[0].problem, Some(ValueProblem::OutOfBounds));
}

#[test]
fn link_version_tracks_junction_row() {
    let toml = EXAMPLE.replace(
        "[tables.contribution.fields.position]\nmin = 1\nmax = 99",
        "[tables.contribution.fields.position]\nmode = \"hidden\"",
    );
    assert_ne!(toml, EXAMPLE);
    let fx = Fixture::new(SCHEMA, Some(&toml));
    let fetch = |fx: &Fixture| {
        let page = fx
            .session()
            .m2m_options("book", Some(&rk(1)), "m2m:contribution", &mq(None, 0, 50))
            .unwrap();
        page.items.into_iter().find(|i| i.linked).unwrap()
    };
    let before = fetch(&fx);
    // the hidden link field is not shown
    assert_eq!(before.link_values.as_ref().unwrap().len(), 1);
    fx.exec("UPDATE contribution SET position = 7 WHERE book_id = 1 AND author_id = 2;");
    let after_hidden = fetch(&fx);
    assert_ne!(before.link_version, after_hidden.link_version);
    assert_eq!(before.link_values, after_hidden.link_values);
    fx.exec("UPDATE contribution SET role = 'translator' WHERE book_id = 1 AND author_id = 2;");
    assert_ne!(after_hidden.link_version, fetch(&fx).link_version);
}

#[test]
fn m2m_options_only() {
    let fx = Fixture::example();
    let sess = fx.session();
    let full = sess
        .m2m_options("book", Some(&rk(1)), "m2m:contribution", &mq(None, 0, 50))
        .unwrap();
    let want = full.items.iter().find(|i| i.key == rk(2)).unwrap().clone();
    let one = M2mQuery {
        only: Some(rk(2)),
        // ignored with `only`
        search: Some("zzz".into()),
        offset: 5,
        linked_only: true,
        ..mq(None, 0, 1)
    };
    let p = sess
        .m2m_options("book", Some(&rk(1)), "m2m:contribution", &one)
        .unwrap();
    assert_eq!(p.items, vec![want]);
    assert_eq!((p.total, p.linked_total), (1, 1));
    // an unlinked author
    let p = sess
        .m2m_options(
            "book",
            Some(&rk(1)),
            "m2m:contribution",
            &M2mQuery {
                only: Some(rk(3)),
                ..mq(None, 0, 1)
            },
        )
        .unwrap();
    assert!(!p.items[0].linked);
    assert_eq!(p.linked_total, 1);
    // a deleted author
    fx.exec("DELETE FROM contribution WHERE author_id = 3; DELETE FROM author WHERE id = 3;");
    let sess = fx.session();
    let p = sess
        .m2m_options(
            "book",
            Some(&rk(1)),
            "m2m:contribution",
            &M2mQuery {
                only: Some(rk(3)),
                ..mq(None, 0, 1)
            },
        )
        .unwrap();
    assert_eq!(p.items.len(), 0);
    assert_eq!((p.total, p.linked_total), (0, 1));
}
