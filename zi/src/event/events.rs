use zi_core::{BufferId, Mode, ViewId};
use zi_text::{AnyText, Deltas};

use super::{AsyncEvent, Event};
use crate::LanguageServiceId;

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

#[derive(Debug, Clone)]
pub struct DidInitializeLanguageService {
    pub service_id: LanguageServiceId,
}

impl Event for DidInitializeLanguageService {}

#[derive(Debug, Clone)]
pub struct DidYankText {
    pub buf: BufferId,
    pub range: std::ops::Range<usize>,
}

impl Event for DidYankText {}

#[derive(Debug, Clone)]
pub struct WillSaveBuffer {
    pub buf: BufferId,
}

impl AsyncEvent for WillSaveBuffer {}
