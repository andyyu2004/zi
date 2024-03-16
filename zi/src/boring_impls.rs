// Kinda painful that we can't seem to control the derives? wit-bindgen has a setting called
// additional_derives but wasmtime's one doesn't.
//
// Try write the impls in an exhaustive way so any new cases or fields will be caught.

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::wit::exports::zi::api::command::{Arity, Command};
use crate::wit::exports::zi::api::lifecycle::InitializeResult;
use crate::Mode;

impl PartialEq for InitializeResult {
    fn eq(&self, Self { commands }: &Self) -> bool {
        self.commands == *commands
    }
}

impl PartialEq for Command {
    fn eq(&self, Self { name, arity, opts }: &Self) -> bool {
        self.name == *name && self.arity == *arity && self.opts == *opts
    }
}

impl PartialEq for Arity {
    fn eq(&self, Self { min, max }: &Self) -> bool {
        self.min == *min && self.max == *max
    }
}

impl PartialEq for Mode {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Mode::Normal => matches!(other, Mode::Normal),
            Mode::Insert => matches!(other, Mode::Insert),
            Mode::Command => matches!(other, Mode::Command),
            Mode::Visual => matches!(other, Mode::Visual),
        }
    }
}

impl Eq for Mode {}

impl Hash for Mode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Mode {
    fn default() -> Self {
        Mode::Normal
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Mode::Normal => "NORMAL",
                Mode::Command => "COMMAND",
                Mode::Insert => "INSERT",
                Mode::Visual => "VISUAL",
            }
        )
    }
}
