use std::borrow::Cow;
use std::sync::Arc;

use zi_core::style::{style, Style};

#[derive(Clone)]
pub struct Theme {
    highlights: Vec<(Cow<'static, str>, Style)>,
    default_style: Style,
}

pub enum HighlightName {}

impl HighlightName {
    pub const BACKGROUND: &'static str = "background";
    pub const CURSORLINE: &'static str = "cursorline";
    pub const DIRECTORY: &'static str = "directory";
    pub const CURRENT_SEARCH: &'static str = "search.current";
    pub const SEARCH: &'static str = "search";
    pub const ERROR: &'static str = "error";
    pub const WARNING: &'static str = "warning";
    pub const INFO: &'static str = "info";
    pub const HINT: &'static str = "hint";
}

impl Theme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_style(&self) -> Style {
        self.default_style
    }

    pub fn id_by_name(&self, name: impl AsRef<str>) -> HighlightId {
        let name = name.as_ref();
        self.highlights
            .iter()
            .position(|(key, _)| key == name)
            .map(|i| HighlightId(i as u32))
            .unwrap_or_default()
    }
}

macro_rules! hi {
    ($name:expr => $($tt:tt)*) => {
        (Cow::Borrowed($name), style!($($tt)*))
    };
}

impl Default for Theme {
    // Some butchered solarized-dark ish theme.
    fn default() -> Self {
        Self {
            default_style: style!(fg = 0x83949600),
            highlights: [
                hi!(HighlightName::BACKGROUND => bg=0x002b3600),
                hi!(HighlightName::CURSORLINE => bg=0x07364200),
                hi!(HighlightName::DIRECTORY => fg=0x268bd200),
                hi!(HighlightName::SEARCH => bg=0x00445400),
                hi!(HighlightName::CURRENT_SEARCH => fg=0xeb773400 bg=0x00445400),
                hi!(HighlightName::ERROR => underline),
                hi!(HighlightName::WARNING => underline),
                hi!(HighlightName::INFO => underline),
                hi!(HighlightName::HINT => underline),
                hi!("namespace" => fg=0x39a6b900),
                hi!("module" => fg=0x39a6b900),
                hi!("function.macro" => fg=0x298cba00),
                hi!("function" => fg=0x298cba00),
                hi!("property" => fg=0x41978900),
                hi!("field" => fg=0x41978900),
                hi!("keyword" => fg=0x527bd200),
                hi!("constructor" => fg=0xbf8a4a00),
                hi!("type" => fg=0x268bd200),
                hi!("variable.builtin" => fg=0xbf693000),
                hi!("variable" => fg=0x83949600),
                hi!("function.builtin" => fg=0x298cba00),
                hi!("parameter" => fg=0x4698b100),
                hi!("constant" => fg=0xbb8b5000),
                hi!("constant.builtin" => fg=0x41978900),
                hi!("include" => fg=0x527bd200),
                hi!("attribute" => fg=0xB8986800),
                hi!("preproc" => fg=0xB8986800),
                hi!("method" => fg=0x298cba00),
                hi!("method.call" => fg=0x298cba00),
                hi!("punctuation.bracket" => fg=0x86B1A100),
                hi!("punctuation.special" => fg=0x86B1A100),
                hi!("punctuation.delimiter" => fg=0x599c9700),
                hi!("string" => fg=0x2aa19800),
                hi!("number" => fg=0xcb4b1600),
                hi!("comment" => fg=0x586e7500),
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
                            for key_part in key.split('.') {
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
