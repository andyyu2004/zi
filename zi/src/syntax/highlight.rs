use std::sync::Arc;

use zi_core::style::{style, Style};

#[derive(Clone)]
pub struct Theme {
    highlights: Vec<(HighlightName, Style)>,
    default_style: Style,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HighlightName(&'static str);

impl AsRef<str> for HighlightName {
    #[inline]
    fn as_ref(&self) -> &'static str {
        self.0
    }
}

impl From<&'static str> for HighlightName {
    #[inline]
    fn from(name: &'static str) -> Self {
        Self(name)
    }
}

macro_rules! declare_highlights {
    ($($name:ident = $value:literal,)*) => {
        $( pub const $name: Self = Self($value); )*
    };
}

impl HighlightName {
    declare_highlights! {
        BACKGROUND = "background",
        CURSORLINE = "cursorline",
        DIRECTORY = "directory",
        CURRENT_SEARCH = "search.current",
        SEARCH = "search",

        ERROR = "error",
        WARNING = "warning",
        INFO = "info",
        HINT = "hint",

        NAMESPACE = "namespace",
        MODULE = "module",
        MACRO = "function.macro",
        FUNCTION = "function",
        PROPERTY = "property",
        FIELD = "field",
        KEYWORD = "keyword",
        CONSTRUCTOR = "constructor",
        TYPE = "type",
        VARIABLE_BUILTIN = "variable.builtin",
        VARIABLE = "variable",
        FUNCTION_BUILTIN = "function.builtin",
        PARAMETER = "parameter",
        CONSTANT = "constant",
        CONSTANT_BUILTIN = "constant.builtin",
        INCLUDE = "include",
        ATTRIBUTE = "attribute",
        PREPROC = "preproc",
        METHOD = "method",
        METHOD_CALL = "method.call",
        PUNCTUATION_BRACKET = "punctuation.bracket",
        PUNCTUATION_SPECIAL = "punctuation.special",
        PUNCTUATION_DELIMITER = "punctuation.delimiter",
        STRING = "string",
        NUMBER = "number",
        COMMENT = "comment",
    }
}

impl Theme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_style(&self) -> Style {
        self.default_style
    }

    pub fn highlight_id_by_name(&self, name: impl AsRef<str>) -> HighlightId {
        let name = name.as_ref();
        self.highlights
            .iter()
            .position(|(key, _)| key.as_ref() == name)
            .map(|i| HighlightId(i as u32))
            .unwrap_or_default()
    }
}

macro_rules! hi {
    ($name:expr => $($tt:tt)*) => {
        ($name.into(), style!($($tt)*))
    };
}

impl Default for Theme {
    // Some butchered solarized-dark ish theme.
    fn default() -> Self {
        use HighlightName as Hl;
        Self {
            default_style: style!(fg = 0x83949600),
            highlights: [
                hi!(Hl::BACKGROUND => bg=0x002b3600),
                hi!(Hl::CURSORLINE => bg=0x07364200),
                hi!(Hl::DIRECTORY => fg=0x268bd200),
                hi!(Hl::SEARCH => bg=0x00445400),
                hi!(Hl::CURRENT_SEARCH => fg=0xeb773400 bg=0x00445400),
                hi!(Hl::ERROR => underline),
                hi!(Hl::WARNING => underline),
                hi!(Hl::INFO => underline),
                hi!(Hl::HINT => underline),
                hi!(Hl::NAMESPACE => fg=0x39a6b900),
                hi!(Hl::MODULE => fg=0x39a6b900),
                hi!(Hl::MACRO => fg=0x298cba00),
                hi!(Hl::FUNCTION => fg=0x298cba00),
                hi!(Hl::PROPERTY => fg=0x41978900),
                hi!(Hl::FIELD => fg=0x41978900),
                hi!(Hl::KEYWORD => fg=0x527bd200),
                hi!(Hl::CONSTRUCTOR => fg=0xbf8a4a00),
                hi!(Hl::TYPE => fg=0x268bd200),
                hi!(Hl::VARIABLE_BUILTIN => fg=0xbf693000),
                hi!(Hl::VARIABLE => fg=0x83949600),
                hi!(Hl::FUNCTION_BUILTIN => fg=0x298cba00),
                hi!(Hl::PARAMETER => fg=0x4698b100),
                hi!(Hl::CONSTANT => fg=0xbb8b5000),
                hi!(Hl::CONSTANT_BUILTIN => fg=0x41978900),
                hi!(Hl::INCLUDE => fg=0x527bd200),
                hi!(Hl::ATTRIBUTE => fg=0xB8986800),
                hi!(Hl::PREPROC => fg=0xB8986800),
                hi!(Hl::METHOD => fg=0x298cba00),
                hi!(Hl::METHOD_CALL => fg=0x298cba00),
                hi!(Hl::PUNCTUATION_BRACKET => fg=0x86B1A100),
                hi!(Hl::PUNCTUATION_SPECIAL => fg=0x86B1A100),
                hi!(Hl::PUNCTUATION_DELIMITER => fg=0x599c9700),
                hi!(Hl::STRING => fg=0x2aa19800),
                hi!(Hl::NUMBER => fg=0xcb4b1600),
                hi!(Hl::COMMENT => fg=0x586e7500),
            ]
            .into_iter()
            .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HighlightId(u32);

impl HighlightId {
    pub const DEFAULT: HighlightId = HighlightId(u32::MAX);
}

impl HighlightId {
    pub fn is_default(self) -> bool {
        self == Self::DEFAULT
    }

    pub fn style(self, theme: &Theme) -> Option<Style> {
        if self.is_default() {
            return Some(theme.default_style);
        }

        theme.highlights.get(self.0 as usize).map(|(_, style)| style).copied()
    }

    pub fn name(self, theme: &Theme) -> Option<&str> {
        theme.highlights.get(self.0 as usize).map(|name| name.0.as_ref())
    }
}

impl Default for HighlightId {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Debug)]
pub struct HighlightMap(Arc<[HighlightId]>);

impl HighlightMap {
    pub(crate) fn new(
        capture_names: impl IntoIterator<Item = impl AsRef<str>>,
        theme: &Theme,
    ) -> Self {
        // For each capture name in the highlight query, find the longest
        // key in the theme's syntax styles that matches all of the
        // dot-separated components of the capture name.
        HighlightMap(
            capture_names
                .into_iter()
                .map(|capture_name| {
                    theme
                        .highlights
                        .iter()
                        .enumerate()
                        .filter_map(|(i, (key, _))| {
                            let mut len = 0;
                            let mut capture_parts = capture_name.as_ref().split('.');
                            for key_part in key.as_ref().split('.') {
                                if capture_parts.any(|part| part == key_part) {
                                    len += 1;
                                } else {
                                    return None;
                                }
                            }
                            Some((i, len))
                        })
                        .max_by_key(|(_, len)| *len)
                        .map_or(HighlightId::DEFAULT, |(i, _)| HighlightId(i as u32))
                })
                .collect(),
        )
    }

    pub fn get(&self, capture_idx: u32) -> HighlightId {
        self.0.get(capture_idx as usize).copied().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn highlight_map() {
        let theme = Theme {
            default_style: Style::none(),
            highlights: [
                ("function", Style::none()),
                ("function.method", Style::none()),
                ("function.async", Style::none()),
                ("variable.builtin.self.rust", Style::none()),
                ("variable.builtin", Style::none()),
                ("variable", Style::none()),
            ]
            .into_iter()
            .map(|(name, color)| (name.into(), color))
            .collect(),
        };

        let capture_names = &["function.special", "function.async.rust", "variable.builtin.self"];

        let map = HighlightMap::new(capture_names, &theme);
        assert_eq!(map.get(0).name(&theme), Some("function"));
        assert_eq!(map.get(1).name(&theme), Some("function.async"));
        assert_eq!(map.get(2).name(&theme), Some("variable.builtin"));
    }
}
