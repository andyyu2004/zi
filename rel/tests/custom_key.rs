use rel::{Database, Id, Key, RawId, Relation};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
struct BufferId(u64);

impl Key for BufferId {
    fn from_raw(raw: RawId) -> Self {
        BufferId((u64::from(raw.generation()) << 32) | u64::from(raw.index()))
    }

    fn raw(self) -> RawId {
        RawId::new(self.0 as u32, (self.0 >> 32) as u32)
    }
}

#[derive(Relation)]
#[relation(key = BufferId)]
struct Buffer {
    id: BufferId,
    text: String,
}

#[derive(Relation)]
struct View {
    #[references(Buffer, on_delete = cascade)]
    buffer: BufferId,
}

#[derive(Database, Default)]
struct Editor {
    buffers: Relation<Buffer>,
    views: Relation<View>,
}

#[test]
fn custom_key_type() {
    let mut db = Editor::default();
    let a: BufferId = db.insert_with(|id| Buffer { id, text: "a".into() }).unwrap();
    let view: Id<View> = db.insert(View { buffer: a }).unwrap();

    assert_eq!(db.buffers[a].id, a);
    assert_eq!(db.get(a).map(|b| b.text.as_str()), Some("a"));
    assert_eq!(db.views.by_buffer(&a).map(|v| v.id()).collect::<Vec<_>>(), [view]);

    db.delete(a).unwrap();
    assert!(db.views.is_empty());
    assert!(db.get(a).is_none());
}
