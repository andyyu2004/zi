use rel::{Database, Id, Relation};

#[derive(Relation)]
struct Buffer {
    text: String,
}

#[derive(Relation)]
struct View {
    #[references(Buffer, on_delete = restrict)]
    buffer: Id<Buffer>,
    cursor: usize,
}

#[derive(Database, Default)]
struct Db {
    buffers: Relation<Buffer>,
    views: Relation<View>,
}

fn main() {
    let mut db = Db::default();
    let a = db.insert(Buffer { text: String::new() }).unwrap();
    let b = db.insert(Buffer { text: String::new() }).unwrap();
    let view = db.insert(View { buffer: a, cursor: 0 }).unwrap();
    let v = db.views.get_mut(view).unwrap();
    *v.cursor = 1;
    *v.buffer = b;
}
