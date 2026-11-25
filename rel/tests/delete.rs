//! Deletes whose cascades overlap.

use rel::{Database, Id, Relation};

#[derive(Relation)]
struct Buffer {
    text: String,
}

#[derive(Relation)]
struct Group {
    #[references(Buffer, on_delete = cascade)]
    buffer: Id<Buffer>,
}

#[derive(Relation)]
struct View {
    #[references(Buffer, on_delete = cascade)]
    buffer: Id<Buffer>,
    #[references(Group, on_delete = set_null)]
    group: Option<Id<Group>>,
}

#[derive(Relation)]
struct Selection {
    // Indexed explicitly, and ordered: foreign keys reuse an index on their column.
    #[index(ordered)]
    #[references(View, on_delete = cascade)]
    view: Id<View>,
    #[references(Buffer, on_delete = cascade)]
    buffer: Id<Buffer>,
}

#[derive(Database, Default)]
struct Editor {
    buffers: Relation<Buffer>,
    groups: Relation<Group>,
    views: Relation<View>,
    selections: Relation<Selection>,
}

#[test]
fn a_row_reached_by_two_cascades_is_removed_once() {
    let mut db = Editor::default();
    let buffer = db.insert(Buffer { text: "a".into() }).unwrap();
    let view = db.insert(View { buffer, group: None }).unwrap();
    // Deleting the buffer reaches the selection directly and through its view.
    db.insert(Selection { view, buffer }).unwrap();

    db.delete(buffer).unwrap();
    assert!(db.views.is_empty());
    assert!(db.selections.is_empty());
}

#[test]
fn a_cascade_through_an_ordered_foreign_key_index() {
    let mut db = Editor::default();
    let a = db.insert(Buffer { text: "a".into() }).unwrap();
    let b = db.insert(Buffer { text: "b".into() }).unwrap();
    let view = db.insert(View { buffer: a, group: None }).unwrap();
    let other = db.insert(View { buffer: a, group: None }).unwrap();
    db.insert(Selection { view, buffer: b }).unwrap();
    let kept = db.insert(Selection { view: other, buffer: b }).unwrap();

    assert_eq!(db.selections.by_view(view..=view).count(), 1);
    db.delete(view).unwrap();
    assert_eq!(db.selections.ids().collect::<Vec<_>>(), [kept]);
}

#[test]
fn a_row_removed_by_a_cascade_is_not_also_nulled() {
    let mut db = Editor::default();
    let buffer = db.insert(Buffer { text: "a".into() }).unwrap();
    let group = db.insert(Group { buffer }).unwrap();
    // The view goes by cascade from the buffer, and would be nulled for the group.
    db.insert(View { buffer, group: Some(group) }).unwrap();

    db.delete(buffer).unwrap();
    assert!(db.groups.is_empty());
    assert!(db.views.is_empty());
}
