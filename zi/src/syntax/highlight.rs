use std::borrow::Cow;

pub struct SyntaxTheme {
    highlights: Vec<(Cow<'static, str>, Style)>,
}

impl Default for SyntaxTheme {
    fn default() -> Self {
        Self {
            highlights: [("comment", Style::with_fg(Color::rgba(0x00_80_00_FF)))]
                .into_iter()
                .map(|(name, style)| (name.into(), style))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
}

impl Style {
    pub fn with_fg(fg: Color) -> Self {
        Self { fg: Some(fg), ..Default::default() }
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum Color {
    Rgb(u8, u8, u8),
}

impl Color {
    pub fn rgba(hex: u32) -> Self {
        let r = (hex >> 24) & 0xFF;
        let g = (hex >> 16) & 0xFF;
        let b = (hex >> 8) & 0xFF;
        let a = hex & 0xFF;
        assert_eq!(a, 0, "alpha channel not supported");
        Color::Rgb(r as u8, g as u8, b as u8)
    }
}

#[derive(Clone, Debug)]
pub struct HighlightMap(Box<[HighlightId]>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighlightId(pub u32);

const DEFAULT_SYNTAX_HIGHLIGHT_ID: HighlightId = HighlightId(u32::MAX);

impl HighlightMap {
    pub(crate) fn new(capture_names: &[&str], theme: &SyntaxTheme) -> Self {
        // For each capture name in the highlight query, find the longest
        // key in the theme's syntax styles that matches all of the
        // dot-separated components of the capture name.
        HighlightMap(
            capture_names
                .iter()
                .map(|capture_name| {
                    theme
                        .highlights
                        .iter()
                        .enumerate()
                        .filter_map(|(i, (key, _))| {
                            let mut len = 0;
                            let mut capture_parts = capture_name.split('.');
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
                        .map_or(DEFAULT_SYNTAX_HIGHLIGHT_ID, |(i, _)| HighlightId(i as u32))
                })
                .collect(),
        )
    }

    pub fn get(&self, capture_id: u32) -> HighlightId {
        self.0.get(capture_id as usize).copied().unwrap_or(DEFAULT_SYNTAX_HIGHLIGHT_ID)
    }
}

impl HighlightId {
    pub(crate) fn is_default(self) -> bool {
        self == DEFAULT_SYNTAX_HIGHLIGHT_ID
    }

    pub fn style(self, theme: &SyntaxTheme) -> Option<Style> {
        theme.highlights.get(self.0 as usize).map(|(_, style)| style).copied()
    }

    pub fn name(self, theme: &SyntaxTheme) -> Option<&str> {
        theme.highlights.get(self.0 as usize).map(|name| name.0.as_ref())
    }
}

impl Default for HighlightMap {
    fn default() -> Self {
        Self([].into())
    }
}

impl Default for HighlightId {
    fn default() -> Self {
        DEFAULT_SYNTAX_HIGHLIGHT_ID
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_highlight_map() {
        let theme = SyntaxTheme {
            highlights: [
                ("function", Style::default()),
                ("function.method", Style::default()),
                ("function.async", Style::default()),
                ("variable.builtin.self.rust", Style::default()),
                ("variable.builtin", Style::default()),
                ("variable", Style::default()),
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
