use rel::{Id, Relation};

#[derive(Relation)]
struct Buffer {
    #[index]
    path: String,
}

#[test]
fn insert_without_constraints_is_infallible() {
    let mut buffers = Relation::<Buffer>::default();
    let a: Id<Buffer> = buffers.insert(Buffer { path: "a".into() });
    let b = buffers.insert_with(|_| Buffer { path: "a".into() });
    assert_ne!(a, b);
    assert_eq!(buffers.by_path("a").count(), 2);
}
