use zi_input::{KeyCode, KeyEvent};

use super::Count;
use crate::Mode;

/// dot repeat state
#[derive(Debug, Default)]
pub(super) struct Dot {
    /// The recorded key events for the last change
    events: Vec<KeyEvent>,
    /// Whether we're currently recording a change
    recording: bool,
    /// Whether we're currently replaying (to prevent recording during replay)
    replaying: bool,
    /// The count of the last change, given separately from its keys
    count: Option<Count>,
    /// Keys pressed in Normal mode before a recording starts
    normal_keys: Vec<KeyEvent>,
    /// The count typed in Normal mode before a recording starts
    normal_count: Option<Count>,
    /// Whether the key being handled has made an edit that can be repeated
    edited: bool,
}

impl Dot {
    /// Start recording a new change operation
    pub(super) fn start_recording(&mut self) {
        self.events.clear();
        self.events.append(&mut self.normal_keys);
        self.count = self.normal_count.take();
        self.recording = true;
    }

    pub(super) fn record_input(&mut self, mode: Mode, key: &KeyEvent) {
        if self.replaying {
            return;
        }

        // A Normal-mode key may be the beginning of a change.
        // FIXME: assumes '.' is not remapped.
        if mode == Mode::Normal && !matches!(key.code(), KeyCode::Char('.')) {
            self.normal_keys.push(key.clone());
        }
        if self.recording {
            self.events.push(key.clone());
        }
    }

    pub(super) fn clear_normal_keys(&mut self) {
        self.normal_keys.clear();
        self.normal_count = None;
    }

    /// Replace the last saved key, a count key, with the count it set
    pub(super) fn save_count(&mut self, mode: Mode, count: Count) {
        if self.replaying {
            return;
        }

        if self.recording {
            self.events.pop();
            self.count = Some(count);
        } else if mode == Mode::Normal {
            self.normal_keys.pop();
            self.normal_count = Some(count);
        }
    }

    pub(super) fn count(&self) -> Option<Count> {
        self.count
    }

    pub(super) fn set_count(&mut self, count: Count) {
        self.count = Some(count);
    }

    pub(super) fn start_key(&mut self) {
        self.edited = false;
    }

    pub(super) fn did_edit(&mut self) {
        if !self.replaying {
            self.edited = true;
        }
    }

    /// Finalize recording of a Normal mode change
    /// Called after a key handled in Normal mode, records it if the key edited a buffer
    pub(super) fn finalize_normal_mode_change(&mut self) {
        if self.edited && !self.normal_keys.is_empty() && !self.recording && !self.replaying {
            self.start_recording();
            self.stop_recording();
        }
    }

    /// Start replaying (prevents recording during replay)
    pub(super) fn start_replaying(&mut self) {
        self.replaying = true;
    }

    /// Stop replaying
    pub(super) fn stop_replaying(&mut self) {
        self.replaying = false;
    }

    /// Check if we're currently replaying
    pub(super) fn is_replaying(&self) -> bool {
        self.replaying
    }

    /// Stop recording the current change
    pub(super) fn stop_recording(&mut self) {
        self.recording = false;
    }

    /// Get the recorded events to replay
    pub(super) fn events(&self) -> &[KeyEvent] {
        &self.events
    }

    /// Check if a mode transition should start recording
    pub(super) fn should_start_recording(from: Mode, to: Mode) -> bool {
        matches!(
            (from, to),
            (Mode::Normal, Mode::Insert | Mode::OperatorPending(_) | Mode::ReplacePending)
        )
    }

    /// Check if a mode transition should stop recording
    pub(super) fn should_stop_recording(from: Mode, to: Mode) -> bool {
        matches!(to, Mode::Normal) && !matches!(from, Mode::Normal)
    }
}
