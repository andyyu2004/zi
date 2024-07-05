use zi_text::{AnyText, Deltas};

use super::{AsyncEvent, Event};
use crate::{BufferId, Mode, ViewId};

#[derive(Debug, Clone)]
pub struct DidChangeBuffer {
    pub buf: BufferId,
    pub old_text: Box<dyn AnyText>,
    pub deltas: Deltas<'static>,
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

#[derive(Debug, Clone)]
pub struct WillChangeMode {
    pub from: Mode,
    pub to: Mode,
}

impl Event for WillChangeMode {}

#[derive(Debug, Clone)]
pub struct DidChangeMode {
    pub from: Mode,
    pub to: Mode,
}

impl Event for DidChangeMode {}
