use ropey::Rope;

slotmap::new_key_type! {
    pub struct BufferId;
}

pub struct Buffer {
    id: BufferId,
    text: Rope,
}

impl Buffer {
    pub fn new(id: BufferId, text: Rope) -> Self {
        Self { id, text }
    }
}
