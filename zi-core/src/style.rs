use std::fmt;
use std::str::FromStr;

use stdx::merge::Merge;

// Don't implement default to avoid misuse. Should always get the default style off the theme.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub modifier: Modifier,
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
        let mut style = Style::none();
        for part in s.split_whitespace() {
            let (key, value) = part
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("invalid style part: {part}"))?;

            style = match key {
                "fg" => style.with_fg(value.parse()?),
                "bg" => style.with_bg(value.parse()?),
                _ => return Err(anyhow::anyhow!("invalid style field: {part}")),
            };
        }

        Ok(style)
    }
}

impl Style {
    pub const fn none() -> Self {
        Self { fg: None, bg: None, modifier: Modifier::empty() }
    }

    pub fn with_fg(mut self, fg: Color) -> Self {
        self.fg = Some(fg);
        self
    }

    pub fn with_bg(mut self, bg: Color) -> Self {
        self.bg = Some(bg);
        self
    }

    pub fn with_modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }
}

impl From<Style> for tui::Style {
    #[inline]
    fn from(s: Style) -> Self {
        tui::Style { fg: s.fg.map(Into::into), bg: s.bg.map(Into::into), ..Default::default() }
            .add_modifier(s.modifier)
    }
}

pub type Modifier = tui::Modifier;

impl Merge for Style {
    #[inline]
    fn merge(self, other: Self) -> Self {
        Self {
            fg: other.fg.or(self.fg),
            bg: other.bg.or(self.bg),
            modifier: self.modifier.union(other.modifier),
        }
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

#[macro_export]
macro_rules! modifier {
    () => {
        $crate::style::Modifier::empty()
    };
    (bold $($tt:tt)*) => {
        $crate::style::Modifier::BOLD | $crate::modifier!($($tt)*)
    };
    (dim $($tt:tt)*) => {
        $crate::style::Modifier::DIM | $crate::modifier!($($tt)*)
    };
    (italic $($tt:tt)*) => {
        $crate::style::Modifier::ITALIC | $crate::modifier!($($tt)*)
    };
    (underline $($tt:tt)*) => {
        $crate::style::Modifier::UNDERLINED | $crate::modifier!($($tt)*)
    };
    (slow_blink $($tt:tt)*) => {
        $crate::style::Modifier::SLOW_BLINK | $crate::modifier!($($tt)*)
    };
    (rapid_blink $($tt:tt)*) => {
        $crate::style::Modifier::RAPID_BLINK | $crate::modifier!($($tt)*)
    };
    (reverse $($tt:tt)*) => {
        $crate::style::Modifier::REVERSED | $crate::modifier!($($tt)*)
    };
    (hidden $($tt:tt)*) => {
        $crate::style::Modifier::HIDDEN | $crate::modifier!($($tt)*)
    };
    (crossed_out $($tt:tt)*) => {
        $crate::style::Modifier::CROSSED_OUT | $crate::modifier!($($tt)*)
    };
}

#[macro_export]
macro_rules! style {
    (fg=$fg:literal bg=$bg:literal $($tt:tt)*) => {
        $crate::style::Style::none()
            .with_fg($crate::style::Color::rgba($fg))
            .with_bg($crate::style::Color::rgba($bg))
            .with_modifier($crate::modifier!($($tt)*))
    };
    (fg=$fg:literal $($tt:tt)*) => {
        $crate::style::Style::none()
            .with_fg($crate::style::Color::rgba($fg))
            .with_modifier($crate::modifier!($($tt)*))
    };
    (bg=$bg:literal $($tt:tt)*) => {
        $crate::style::Style::none()
            .with_bg($crate::style::Color::rgba($bg))
            .with_modifier($crate::modifier!($($tt)*))
    };
    ($($tt:tt)*) => {
        $crate::style::Style::none().with_modifier($crate::modifier!($($tt)*))
    };
}

pub use {modifier, style};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_macro() {
        assert_eq!(style!(bold), Style::none().with_modifier(Modifier::BOLD));
        assert_eq!(style!(fg = 0x11223300), Style::none().with_fg(Color::Rgb(0x11, 0x22, 0x33)));
        assert_eq!(style!(bg = 0x11223300), Style::none().with_bg(Color::Rgb(0x11, 0x22, 0x33)));
        assert_eq!(
            style!(fg=0x11223300 bg=0x44556600),
            Style::none()
                .with_fg(Color::Rgb(0x11, 0x22, 0x33))
                .with_bg(Color::Rgb(0x44, 0x55, 0x66))
        );
        assert_eq!(
            style!(fg=0x11223300 bold),
            Style::none().with_fg(Color::Rgb(0x11, 0x22, 0x33)).with_modifier(Modifier::BOLD)
        );
        assert_eq!(
            style!(fg=0x11223300 bg=0x44556600 bold),
            Style::none()
                .with_fg(Color::Rgb(0x11, 0x22, 0x33))
                .with_bg(Color::Rgb(0x44, 0x55, 0x66))
                .with_modifier(Modifier::BOLD)
        );

        assert_eq!(style!(bold dim), Style::none().with_modifier(Modifier::BOLD | Modifier::DIM));
    }
}
