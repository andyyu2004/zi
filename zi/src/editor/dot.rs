use zi_input::KeyEvent;

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
    /// Keys pressed in Normal mode before a recording starts
    normal_keys: Vec<KeyEvent>,
}

impl Dot {
    /// Start recording a new change operation
    pub(super) fn start_recording(&mut self) {
        self.events.clear();
        self.events.append(&mut self.normal_keys);
        self.recording = true;
    }

    /// Save a key pressed in Normal mode (before we know if it's a change)
    pub(super) fn save_normal_key(&mut self, key: &KeyEvent) {
        if !self.replaying {
            self.normal_keys.push(key.clone());
        }
    }

    pub(super) fn clear_normal_keys(&mut self) {
        self.normal_keys.clear();
    }

    /// Finalize recording of a Normal mode change
    /// Called when a buffer change is detected while in Normal mode
    pub(super) fn finalize_normal_mode_change(&mut self) {
        if !self.normal_keys.is_empty() && !self.recording && !self.replaying {
            self.start_recording();
            self.stop_recording();
        }
    }

    /// Record a key event if we're currently recording and not replaying
    pub(super) fn maybe_record(&mut self, key: &KeyEvent) {
        if self.recording && !self.replaying {
            self.events.push(key.clone());
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
            (Mode::Normal, Mode::Insert)
                | (Mode::Normal, Mode::OperatorPending(_))
                | (Mode::Normal, Mode::ReplacePending)
        )
    }

    /// Check if a mode transition should stop recording
    pub(super) fn should_stop_recording(from: Mode, to: Mode) -> bool {
        matches!(to, Mode::Normal) && !matches!(from, Mode::Normal)
    }
}
