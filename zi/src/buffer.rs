use ropey::Rope;

slotmap::new_key_type! {
    pub struct BufferId;
}

pub struct Buffer {
    id: BufferId,
    text: Rope,
}

impl Buffer {
    #[inline]
    pub fn new(id: BufferId, text: Rope) -> Self {
        Self { id, text }
    }

    #[inline]
    pub fn id(&self) -> BufferId {
        self.id
    }

    #[inline]
    pub fn text(&self) -> &Rope {
        &self.text
    }
}
