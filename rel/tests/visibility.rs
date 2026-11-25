mod schema {
    use rel::{Id, Relation};

    #[derive(Relation)]
    pub struct Buffer {
        pub(crate) path: String,
        text: String,
    }

    #[derive(Relation)]
    pub(crate) struct View {
        #[references(Buffer, on_delete = cascade)]
        pub buffer: Id<Buffer>,
    }

    pub fn new_buffer(path: &str) -> Buffer {
        // Private fields' columns are visible where the field is.
        let _ = buffer::text;
        Buffer { path: path.into(), text: String::new() }
    }
}

use schema::{buffer, view};

#[test]
fn columns_are_as_visible_as_their_fields() {
    let b = schema::new_buffer("p");
    assert_eq!(buffer::path.get(&b), "p");
    assert_eq!(view::buffer.name(), "buffer");
}
