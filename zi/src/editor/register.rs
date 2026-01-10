use std::collections::BTreeMap;

use zi_textobject::TextObjectKind;

#[derive(Default)]
pub struct Registers {
    registers: BTreeMap<char, Register>,
}

impl Registers {
    pub const UNNAMED: char = '"';

    pub fn get(&self, name: char) -> Option<&Register> {
        self.registers.get(&name)
    }

    pub(crate) fn get_or_insert(&mut self, name: char) -> &mut Register {
        self.registers.entry(name).or_default()
    }
}

#[derive(Default, Clone)]
pub struct Register {
    pub kind: RegisterKind,
    pub content: String,
}

impl Register {
    pub(crate) fn set(&mut self, kind: impl Into<RegisterKind>, content: impl Into<String>) {
        self.kind = kind.into();
        self.content = content.into();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RegisterKind {
    #[default]
    Charwise,
    Linewise,
}

impl From<TextObjectKind> for RegisterKind {
    fn from(kind: TextObjectKind) -> Self {
        match kind {
            TextObjectKind::Charwise => RegisterKind::Charwise,
            TextObjectKind::Linewise => RegisterKind::Linewise,
        }
    }
}
