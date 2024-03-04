use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use stdx::merge::Merge;

pub struct Theme {
    highlights: Vec<(Cow<'static, str>, Style)>,
}

impl Theme {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            highlights: [
                ("line", None, Some(0x07364200)),
                ("namespace", Some(0x39a6b900), None),
                ("function.macro", Some(0x298cba00), None),
                ("function", Some(0x298cba00), None),
                ("property", Some(0x41978900), None),
                ("field", Some(0x41978900), None),
                ("keyword", Some(0x527bd200), None),
                ("constructor", Some(0xbf8a4a00), None),
                ("type", Some(0x268bd200), None),
                ("variable.builtin", Some(0xbf693000), None),
                ("variable", Some(0x39a6b900), None),
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
                let mut style = Style::default();
                if let Some(fg) = fg {
                    style.with_fg(Color::rgba(fg));
                }
                if let Some(bg) = bg {
                    style.with_bg(Color::rgba(bg));
                }
                (name.into(), style)
            })
            .collect(),
        }
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut components = Vec::with_capacity(2);

        if let Some(fg) = self.fg {
            components.push(format!("fg={fg}"));
        }

        if let Some(bg) = self.bg {
            components.push(format!("bg={bg}"));
        }

        write!(f, "{}", components.join(" "))
    }
}

impl FromStr for Style {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut style = Style::default();
        for part in s.split_whitespace() {
            let (key, value) = part
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("invalid style part: {part}"))?;

            match key {
                "fg" => style.with_fg(value.parse()?),
                "bg" => style.with_bg(value.parse()?),
                _ => return Err(anyhow::anyhow!("invalid style field: {part}")),
            };
        }
        Ok(style)
    }
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_fg(&mut self, fg: Color) -> &mut Self {
        self.fg = Some(fg);
        self
    }

    pub fn with_bg(&mut self, bg: Color) -> &mut Self {
        self.bg = Some(bg);
        self
    }
}

impl From<Style> for tui::Style {
    #[inline]
    fn from(s: Style) -> Self {
        tui::Style { fg: s.fg.map(Into::into), bg: s.bg.map(Into::into), ..Default::default() }
    }
}

impl Merge for Style {
    #[inline]
    fn merge(self, other: Self) -> Self {
        tracing::error!("merging styles: {:?} and {:?}", self, other);
        Self { fg: other.fg.or(self.fg), bg: other.bg.or(self.bg) }
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum Color {
    Rgb(u8, u8, u8),
}

impl FromStr for Color {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 7 || !s.starts_with('#') {
            anyhow::bail!("invalid color: {s}");
        }

        let r = u8::from_str_radix(&s[1..3], 16)?;
        let g = u8::from_str_radix(&s[3..5], 16)?;
        let b = u8::from_str_radix(&s[5..7], 16)?;
        Ok(Color::Rgb(r, g, b))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::Rgb(r, g, b) => write!(f, "#{:02x}{:02x}{:02x}", r, g, b),
        }
    }
}

impl From<Color> for tui::Color {
    #[inline]
    fn from(c: Color) -> Self {
        match c {
            Color::Rgb(r, g, b) => tui::Color::Rgb(r, g, b),
        }
    }
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
pub struct HighlightMap(Arc<[HighlightId]>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighlightId(pub u32);

const DEFAULT_SYNTAX_HIGHLIGHT_ID: HighlightId = HighlightId(u32::MAX);

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
    pub fn is_default(self) -> bool {
        self == DEFAULT_SYNTAX_HIGHLIGHT_ID
    }

    pub fn style(self, theme: &Theme) -> Option<Style> {
        theme.highlights.get(self.0 as usize).map(|(_, style)| style).copied()
    }

    pub fn name(self, theme: &Theme) -> Option<&str> {
        theme.highlights.get(self.0 as usize).map(|name| name.0.as_ref())
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
    fn highlight_map() {
        let theme = Theme {
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
