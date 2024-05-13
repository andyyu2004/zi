use super::{AsyncEvent, Event};
use crate::{BufferId, ViewId};

#[derive(Debug, Clone)]
pub struct DidChangeBuffer {
    pub buf: BufferId,
}

impl Event for DidChangeBuffer {}

#[derive(Debug, Clone)]
pub struct DidOpenBuffer {
    pub buf: BufferId,
}

impl Event for DidOpenBuffer {}

#[derive(Debug, Clone)]
pub struct DidCloseView {
    pub view: ViewId,
}

impl Event for DidCloseView {}

#[derive(Debug, Clone)]
pub struct WillSaveBuffer {
    pub buf: BufferId,
}

impl AsyncEvent for WillSaveBuffer {}
