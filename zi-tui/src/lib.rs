mod element;
mod sequence;

use std::borrow::Cow;
use std::iter::Peekable;
use std::marker::PhantomData;
use std::str::FromStr;
use std::{cmp, fmt};

pub use ratatui::backend::Backend;
pub use ratatui::buffer::Buffer;
pub use ratatui::layout::{Constraint, Direction, Layout, Rect};
pub use ratatui::style::{Color, Style};
pub use ratatui::text::{Line, Span, Text};
pub use ratatui::widgets::{Clear, Widget};
pub use ratatui::{backend, Frame, Terminal};

/// Convenience trait to allow replacing the frame with a faster implementation for testing.
pub trait DynFrame {
    fn buffer_mut(&mut self) -> &mut Buffer;

    fn set_cursor(&mut self, x: u16, y: u16);
}

impl DynFrame for Frame<'_> {
    #[inline]
    fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffer_mut()
    }

    #[inline]
    fn set_cursor(&mut self, x: u16, y: u16) {
        self.set_cursor(x, y)
    }
}

pub use self::element::Element;
pub use self::sequence::ElementSeq;

pub fn vstack<I, S>(constraints: I, seq: S) -> impl Element
where
    S: ElementSeq,
    I: IntoIterator,
    I::IntoIter: ExactSizeIterator,
    <I::IntoIter as Iterator>::Item: Into<Constraint>,
{
    let constraints = constraints.into_iter();
    assert_eq!(
        constraints.len(),
        seq.len(),
        "constraints length doesn't match the view sequence length: {} != {}",
        constraints.len(),
        seq.len()
    );
    let layout = Layout::vertical(constraints);
    Stack { layout, seq }
}

pub struct Stack<S> {
    layout: Layout,
    seq: S,
}

impl<S: ElementSeq> Widget for Stack<S>
where
    S: ElementSeq,
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        let areas = self.layout.split(area);
        self.seq.render(areas, buf);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineNumberStyle {
    Absolute,
    #[default]
    Relative,
    None,
}

impl FromStr for LineNumberStyle {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "abs" | "absolute" => Ok(Self::Absolute),
            "rel" | "relative" => Ok(Self::Relative),
            "none" | "off" => Ok(Self::None),
            _ => anyhow::bail!(
                "unknown line number style: {s} (expected `absolute`, `none`, or `relative)"
            ),
        }
    }
}

impl fmt::Display for LineNumberStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absolute => write!(f, "absolute"),
            Self::Relative => write!(f, "relative"),
            Self::None => write!(f, "none"),
        }
    }
}

pub struct Lines<'a, I: Iterator> {
    /// The 0-indexed line number to start with
    line_offset: usize,
    line_number_style: LineNumberStyle,
    tab_width: u8,
    min_number_width: u8,
    cursor_line: usize,
    chunks: Peekable<I>,
    _marker: PhantomData<&'a ()>,
}

impl<I: Iterator> Lines<'_, I> {
    pub fn new(
        line_offset: usize,
        cursor_line: usize,
        line_number_style: LineNumberStyle,
        tab_width: u8,
        min_number_width: u8,
        chunks: I,
    ) -> Self {
        Self {
            line_offset,
            line_number_style,
            tab_width,
            min_number_width,
            cursor_line,
            chunks: chunks.peekable(),
            _marker: PhantomData,
        }
    }
}

impl<'a, I> Widget for Lines<'a, I>
where
    I: Iterator<Item = (usize, Cow<'a, str>, Style)>,
{
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        let mut spans = vec![];
        while let Some(&(i, ..)) = self.chunks.peek() {
            if i >= area.height as usize {
                break;
            }

            assert!(spans.is_empty());
            // FIXME not handling where the line number is longer than the width
            let width = self.min_number_width.saturating_sub(1);
            let style = Style::new().fg(Color::Rgb(0x58, 0x6e, 0x75));

            let line_number_span = match self.line_number_style {
                LineNumberStyle::Relative => {
                    let number = match self.cursor_line.cmp(&(i + self.line_offset)) {
                        cmp::Ordering::Less => i + self.line_offset - self.cursor_line,
                        cmp::Ordering::Equal => self.cursor_line + 1,
                        cmp::Ordering::Greater => self.cursor_line - i - self.line_offset,
                    };

                    Span::styled(format!("{:width$} ", number, width = width as usize), style)
                }
                LineNumberStyle::Absolute => Span::styled(
                    format!("{:width$} ", self.line_offset + i + 1, width = width as usize),
                    style,
                ),
                LineNumberStyle::None => Span::styled(" ", style),
            };

            spans.push(line_number_span);

            while let Some(&(j, ref text, style)) = self.chunks.peek() {
                if j != i {
                    assert!(j > i);
                    break;
                }
                spans.push(Span::styled(text.clone(), style));
                self.chunks.next();
            }

            // Tabs are currently not rendered at all. We replace them with spaces for rendering purposes.
            // https://github.com/ratatui-org/ratatui/issues/876
            for span in &mut spans {
                if span.content.contains('\t') {
                    span.content = span
                        .content
                        .replace('\t', &format!("{:width$}", "", width = self.tab_width as usize))
                        .into();
                }
            }

            let line = Line::default().spans(std::mem::take(&mut spans));
            buf.set_line(area.x, area.y + i as u16, &line, area.width);

            // retake the span to reuse the allocation
            spans = line.spans;
            spans.clear();
        }
    }
}
