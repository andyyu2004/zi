use crate::PointRange;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub range: PointRange,
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
