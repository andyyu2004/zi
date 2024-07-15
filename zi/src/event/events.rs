use super::AsyncEvent;
use crate::BufferId;

#[derive(Debug, Clone)]
pub struct WillSaveBuffer {
    pub buf: BufferId,
}

impl AsyncEvent for WillSaveBuffer {}
