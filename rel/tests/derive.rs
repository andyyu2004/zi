//! Derive options and shapes the other tests don't use.

use rel::{Database, Relation};

/// A release. Doc comments are attributes the derive passes over.
#[derive(Relation, Debug)]
struct Release {
    /// The version number.
    #[index(unique, ordered)]
    version: u32,
}

/// Two words, so its columns module is `release_note`.
#[derive(Relation)]
struct ReleaseNote {
    text: String,
}

#[derive(Relation, Debug)]
#[index(columns(line, col))]
struct Mark {
    line: u32,
    col: u32,
}

#[derive(Relation)]
#[relation(columns = group_columns)]
struct ViewGroup {
    #[index]
    key: String,
}

// The default columns module name, taken by something else.
mod view_group {
    pub const NAME: &str = "taken";
}

mod unit {
    #[derive(Default)]
    pub struct Relation;
}

mod lifetime {
    #[derive(Default)]
    pub struct Relation<'a>(pub std::marker::PhantomData<&'a ()>);
}

#[derive(Database, Default)]
struct Db {
    releases: Relation<Release>,
    marks: Relation<Mark>,
    groups: Relation<ViewGroup>,
    // Named `Relation`, but not `Relation<T>`: ordinary state.
    #[expect(dead_code, reason = "checks the database derive skips this field")]
    unit: unit::Relation,
    #[expect(dead_code, reason = "checks the database derive skips this field")]
    lifetime: lifetime::Relation<'static>,
    #[expect(dead_code, reason = "checks the database derive skips this field")]
    tuple: (u32, u32),
}

#[test]
fn unique_ordered_index() {
    let mut db = Db::default();
    let ids = [3, 1, 2].map(|version| db.insert(Release { version }).unwrap());

    let versions = |iter: &mut dyn Iterator<Item = rel::Ref<'_, Release>>| {
        iter.map(|r| r.version).collect::<Vec<_>>()
    };
    assert_eq!(versions(&mut db.releases.by_version(2..)), [2, 3]);
    assert_eq!(versions(&mut db.releases.find_range(release::version, ..3)), [1, 2]);
    assert_eq!(db.releases.find(release::version, 3).map(|r| r.id()).collect::<Vec<_>>(), [ids[0]]);
    assert_eq!(
        db.insert(Release { version: 1 }),
        Err(rel::Error::Unique { relation: "Release", index: "version" })
    );
}

#[test]
fn multi_column_index_that_is_not_unique() {
    let mut db = Db::default();
    let a = db.marks.insert(Mark { line: 1, col: 2 });
    let b = db.marks.insert(Mark { line: 1, col: 2 });
    db.marks.insert(Mark { line: 1, col: 3 });

    let mut found = db.marks.by_line_and_col(&1, &2).map(|m| m.id()).collect::<Vec<_>>();
    found.sort();
    assert_eq!(found, [a, b]);
    assert_eq!(db.marks.by_line_and_col(&2, &2).count(), 0);
}

#[test]
fn columns_module_can_be_renamed() {
    let mut db = Db::default();
    let id = db.groups.insert(ViewGroup { key: "picker".into() });

    assert_eq!(group_columns::key.name(), "key");
    assert_eq!(
        db.groups.find(group_columns::key, "picker".into()).next().map(|g| g.id()),
        Some(id)
    );
    assert_eq!(view_group::NAME, "taken");
    assert_eq!(release_note::text.name(), "text");
}

mod outer {
    pub mod inner {
        pub mod schema {
            #[derive(rel::Relation)]
            pub struct Row {
                pub(in super::super) value: u32,
                pub(super) other: u32,
                pub(self) own: u32,
            }

            pub fn row() -> Row {
                Row { value: 1, other: 2, own: 3 }
            }

            pub fn own() -> u32 {
                *row::own.get(&row())
            }
        }

        pub fn other() -> u32 {
            *schema::row::other.get(&schema::row())
        }
    }

    pub fn value() -> u32 {
        *inner::schema::row::value.get(&inner::schema::row())
    }
}

#[test]
fn columns_are_as_visible_as_their_restricted_fields() {
    assert_eq!(outer::value(), 1);
    assert_eq!(outer::inner::other(), 2);
    assert_eq!(outer::inner::schema::own(), 3);
}
