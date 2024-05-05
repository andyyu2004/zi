use std::borrow::Cow;
use std::sync::Arc;

use zi_core::style::{Color, Style};

pub struct Theme {
    highlights: Vec<(Cow<'static, str>, Style)>,
    default_style: Style,
}

pub enum HighlightName {}

impl HighlightName {
    pub const CURSORLINE: &'static str = "cursorline";
    pub const DIRECTORY: &'static str = "directory";
    pub const CURRENT_SEARCH: &'static str = "search.current";
    pub const SEARCH: &'static str = "search";
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

impl Default for Theme {
    fn default() -> Self {
        Self {
            default_style: Style { fg: Some(Color::rgba(0x83949600)), bg: None },
            highlights: [
                (HighlightName::CURSORLINE, None, Some(0x07364200)),
                (HighlightName::DIRECTORY, Some(0x268bd200), None),
                (HighlightName::SEARCH, Some(0xb5890000), None),
                (HighlightName::CURRENT_SEARCH, Some(0x39a6b900), None),
                ("namespace", Some(0x39a6b900), None),
                ("module", Some(0x39a6b900), None),
                ("function.macro", Some(0x298cba00), None),
                ("function", Some(0x298cba00), None),
                ("property", Some(0x41978900), None),
                ("field", Some(0x41978900), None),
                ("keyword", Some(0x527bd200), None),
                ("constructor", Some(0xbf8a4a00), None),
                ("type", Some(0x268bd200), None),
                ("variable.builtin", Some(0xbf693000), None),
                ("variable", Some(0x83949600), None),
                ("function.builtin", Some(0x298cba00), None),
                ("parameter", Some(0x4698b100), None),
                ("constant", Some(0xbb8b5000), None),
                ("constant.builtin", Some(0x41978900), None),
                ("include", Some(0x527bd200), None),
                ("attribute", Some(0xB8986800), None),
                ("preproc", Some(0xB8986800), None),
                ("method", Some(0x298cba00), None),
                ("method.call", Some(0x298cba00), None),
                ("punctuation.bracket", Some(0x86B1A100), None),
                ("punctuation.special", Some(0x86B1A100), None),
                ("punctuation.delimiter", Some(0x599c9700), None),
                ("string", Some(0x2aa19800), None),
                ("number", Some(0xcb4b1600), None),
                ("comment", Some(0x586e7500), None),
            ]
            .into_iter()
            .map(|(name, fg, bg)| {
                let mut style = Style::empty();
                style.fg = fg.map(Color::rgba);
                style.bg = bg.map(Color::rgba);
                (name.into(), style)
            })
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
            default_style: Style::empty(),
            highlights: [
                ("function", Style::empty()),
                ("function.method", Style::empty()),
                ("function.async", Style::empty()),
                ("variable.builtin.self.rust", Style::empty()),
                ("variable.builtin", Style::empty()),
                ("variable", Style::empty()),
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
