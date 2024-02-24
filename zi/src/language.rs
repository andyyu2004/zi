use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanguageId(Cow<'static, str>);

impl LanguageId {
    pub const RUST: Self = Self(Cow::Borrowed("rust"));
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanguageServerId(Cow<'static, str>);

impl LanguageServerId {
    pub const RUST_ANALYZER: Self = Self(Cow::Borrowed("rust-analyzer"));
}
