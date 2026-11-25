use rel::{Database, Error, Id, Join, Relation};

#[derive(Relation)]
struct Buffer {
    text: String,
}

#[derive(Relation)]
struct Group {
    #[index(unique)]
    url: String,
}

#[derive(Relation)]
struct View {
    #[references(Buffer, on_delete = cascade)]
    buffer: Id<Buffer>,
    #[references(Group, on_delete = set_null)]
    group: Option<Id<Group>>,
}

impl View {
    // A getter with a column's name: columns live in the `view` module, so they don't clash.
    fn group(&self) -> Option<Id<Group>> {
        self.group
    }
}

#[derive(Database, Default)]
struct Editor {
    buffers: Relation<Buffer>,
    groups: Relation<Group>,
    views: Relation<View>,
}

fn editor() -> (Editor, Id<Buffer>, Id<Group>) {
    let mut db = Editor::default();
    let buf = db.insert(Buffer { text: "a".into() }).unwrap();
    let group = db.insert(Group { url: "g".into() }).unwrap();
    (db, buf, group)
}

#[test]
fn optional_foreign_key_is_checked_when_set() {
    let (mut db, buf, group) = editor();
    db.insert(View { buffer: buf, group: None }).unwrap();
    db.insert(View { buffer: buf, group: Some(group) }).unwrap();

    db.delete(group).unwrap();
    assert!(matches!(
        db.insert(View { buffer: buf, group: Some(group) }),
        Err(Error::ForeignKey { column: "group", .. })
    ));
}

#[test]
fn deleting_the_parent_nulls_the_column() {
    let (mut db, buf, group) = editor();
    let a = db.insert(View { buffer: buf, group: Some(group) }).unwrap();
    let b = db.insert(View { buffer: buf, group: Some(group) }).unwrap();
    let c = db.insert(View { buffer: buf, group: None }).unwrap();

    db.delete(group).unwrap();

    assert_eq!(db.views.len(), 3);
    assert_eq!(db.views[a].group(), None);
    assert_eq!(db.views[b].group(), None);
    // The index moved too.
    assert_eq!(db.views.by_group(&Some(group)).count(), 0);
    let mut ungrouped = db.views.by_group(&None).map(|v| v.id()).collect::<Vec<_>>();
    ungrouped.sort();
    assert_eq!(ungrouped, [a, b, c]);
}

#[test]
fn a_row_removed_by_cascade_is_not_nulled() {
    let (mut db, buf, group) = editor();
    db.insert(View { buffer: buf, group: Some(group) }).unwrap();

    // Deleting the buffer cascades to the view; the group survives.
    db.delete(buf).unwrap();
    assert!(db.views.is_empty());
    assert!(db.groups.contains(group));
}

#[test]
fn set_can_null_and_move_an_optional_foreign_key() {
    let (mut db, buf, group) = editor();
    let other = db.insert(Group { url: "h".into() }).unwrap();
    let v = db.insert(View { buffer: buf, group: None }).unwrap();

    db.set(v, view::group, Some(group)).unwrap();
    db.set(v, view::group, Some(other)).unwrap();
    db.delete(group).unwrap();
    assert_eq!(db.views[v].group(), Some(other));
    db.set(v, view::group, None).unwrap();
    assert!(matches!(db.set(v, view::group, Some(group)), Err(Error::ForeignKey { .. })));
}

#[test]
fn join_through_an_optional_foreign_key() {
    let (mut db, buf, group) = editor();
    let v = db.insert(View { buffer: buf, group: Some(group) }).unwrap();
    db.insert(View { buffer: buf, group: None }).unwrap();

    let joined = db
        .groups
        .iter()
        .join(&db.views, view::group)
        .map(|(g, v)| (g.id(), v.id()))
        .collect::<Vec<_>>();
    assert_eq!(joined, [(group, v)]);
}
