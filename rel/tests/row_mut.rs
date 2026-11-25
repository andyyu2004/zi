use rel::{Database, Error, Id, Relation};

#[derive(Relation, Debug)]
struct Buffer {
    id: Id<Buffer>,
    #[index(unique)]
    path: String,
    text: String,
}

#[derive(Relation, Debug)]
struct View {
    #[references(Buffer, on_delete = restrict)]
    buffer: Id<Buffer>,
    cursor: usize,
}

#[derive(Database, Default)]
struct Editor {
    buffers: Relation<Buffer>,
    views: Relation<View>,
    #[expect(dead_code, reason = "checks the derive skips fields that are not relations")]
    not_a_relation: u32,
}

fn buffer(db: &mut Editor, path: &str) -> Id<Buffer> {
    db.insert_with(|id| Buffer { id, path: path.into(), text: String::new() }).unwrap()
}

fn setup() -> (Editor, Id<Buffer>, Id<Buffer>, Id<View>) {
    let mut db = Editor::default();
    let a = buffer(&mut db, "a");
    let b = buffer(&mut db, "b");
    let view = db.insert(View { buffer: a, cursor: 0 }).unwrap();
    (db, a, b, view)
}

#[test]
fn insert_with_gives_the_row_its_id() {
    let (db, a, b, _) = setup();
    assert_eq!(db.buffers[a].id, a);
    assert_eq!(db.buffers[b].id, b);
}

#[test]
fn refused_insert_with_does_not_leak_its_id() {
    let (mut db, _, _, _) = setup();
    let mut seen = None;
    let err = db.insert_with(|id| {
        seen = Some(id);
        Buffer { id, path: "a".into(), text: String::new() }
    });
    assert_eq!(err, Err(Error::Unique { relation: "Buffer", index: "path" }));
    let c = buffer(&mut db, "c");
    assert_eq!(seen.unwrap().raw().index(), c.raw().index(), "the slot is reused");
    assert!(db.buffers.get(seen.unwrap()).is_none(), "but the refused id stays dead");
}

#[test]
fn mutate_unindexed_columns_of_two_relations_at_once() {
    let (mut db, a, _, view) = setup();
    let v = db.views.get_mut(view).unwrap();
    let buf = db.buffers.get_mut(*v.buffer).unwrap();
    buf.text.push_str("hello");
    *v.cursor = 5;
    assert_eq!(db.buffers[a].text, "hello");
    assert_eq!(db.views[view].cursor, 5);
}

#[test]
fn set_reindexes() {
    let (mut db, a, _, _) = setup();
    assert_eq!(db.set(a, buffer::path, "c".to_string()), Ok("a".to_string()));
    assert!(db.buffers.by_path("a").is_none());
    assert_eq!(db.buffers.by_path("c").map(|b| b.id()), Some(a));
}

#[test]
fn set_refuses_a_unique_violation() {
    let (mut db, a, b, _) = setup();
    assert_eq!(
        db.set(a, buffer::path, "b".to_string()),
        Err(Error::Unique { relation: "Buffer", index: "path" })
    );
    assert_eq!(db.buffers[a].path, "a");
    assert_eq!(db.buffers.by_path("a").map(|r| r.id()), Some(a));
    assert_eq!(db.buffers.by_path("b").map(|r| r.id()), Some(b));
}

#[test]
fn set_refuses_a_dangling_foreign_key() {
    let (mut db, a, b, view) = setup();
    db.set(view, view::buffer, b).unwrap();
    db.delete(a).unwrap();
    assert_eq!(
        db.set(view, view::buffer, a),
        Err(Error::ForeignKey { relation: "View", column: "buffer", references: "Buffer" })
    );
    assert_eq!(db.views[view].buffer, b);
    assert_eq!(db.views.by_buffer(&b).count(), 1);
}

#[test]
fn set_moves_a_foreign_key_without_clone() {
    let (mut db, a, b, view) = setup();
    assert!(db.delete(a).is_err(), "the view still shows a");
    db.set(view, view::buffer, b).unwrap();
    db.delete(a).unwrap();
    assert_eq!(db.views.by_buffer(&b).count(), 1);
}
