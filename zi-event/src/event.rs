use zi_core::{BufferId, Mode, ViewId};
use zi_text::{AnyText, Deltas};

use super::Event;

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

#[derive(Debug, Clone)]
pub struct DidInsertChar {
    pub view: ViewId,
    pub char: char,
}

impl Event for DidInsertChar {}

#[derive(Debug, Clone)]
pub struct DidDeleteChar {
    pub view: ViewId,
}

impl Event for DidDeleteChar {}

#[derive(Debug, Clone)]
pub struct DidSaveBuffer {
    pub buf: BufferId,
}

impl Event for DidSaveBuffer {}
