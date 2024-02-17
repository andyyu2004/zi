use crate::BufferId;

slotmap::new_key_type! {
    pub struct ViewId;
}

pub struct View {
    id: ViewId,
    buffer: BufferId,
}

impl View {
    #[inline]
    pub fn buffer(&self) -> BufferId {
        self.buffer
    }

    pub(crate) fn new(id: ViewId, buffer: BufferId) -> Self {
        Self { id, buffer }
    }
}
