use crate::EncodedPointRange;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: EncodedPointRange,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum Severity {
    Hint,
    Info,
    Warning,
    Error,
}
