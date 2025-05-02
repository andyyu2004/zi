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
pub use ratatui::style::{Color, Modifier, Style};
pub use ratatui::text::{Line, Span, Text};
pub use ratatui::widgets::{
    Clear, List, ListDirection, ListItem, ListState, StatefulWidget, StatefulWidgetRef, Widget,
    WidgetRef,
};
pub use ratatui::{Frame, Terminal, backend};

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

pub struct TestFrame {
    buffer: Buffer,
}

impl TestFrame {
    pub fn new(width: u16, height: u16) -> Self {
        Self { buffer: Buffer::empty(Rect::new(0, 0, width, height)) }
    }
}

impl DynFrame for TestFrame {
    fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    fn set_cursor(&mut self, _x: u16, _y: u16) {}
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
                "unknown line number style: {s} (expected `absolute`, `none`, or `relative`)"
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

impl<'a, I> Lines<'a, I>
where
    I: Iterator<Item = (usize, Cow<'a, str>, Style)>,
{
    /// Render the lines to the buffer returning the width of the line numbers.
    pub fn render_(mut self, area: Rect, buf: &mut Buffer) -> usize {
        fn count_digits(n: usize) -> usize {
            1 + n.abs_diff(0).checked_ilog10().unwrap_or_default() as usize
        }

        fn replace_tabs(tab_width: usize, span: &mut Span<'_>) {
            if span.content.contains('\t') {
                span.content =
                    span.content.replace('\t', &format!("{:width$}", "", width = tab_width)).into();
            }
        }

        let mut lines = vec![];
        const SPACE: &str = " ";
        let mut number_width = match self.line_number_style {
            LineNumberStyle::None => SPACE.len(),
            _ => self.min_number_width as usize,
        };

        while let Some(&(i, ..)) = self.chunks.peek() {
            if i >= area.height as usize {
                break;
            }

            // Include placeholder span to replace with line number.
            let mut spans = vec![Span::raw("")];

            number_width = number_width.max(
                1 + match self.line_number_style {
                    LineNumberStyle::None => 0,
                    LineNumberStyle::Absolute => count_digits(self.line_offset + i + 1),
                    LineNumberStyle::Relative => count_digits(self.cursor_line + 1),
                },
            );

            while let Some(&(j, ref text, style)) = self.chunks.peek() {
                if j != i {
                    assert!(j > i);
                    break;
                }

                spans.push(Span::styled(text.trim_end_matches('\n').to_owned(), style));
                self.chunks.next();
            }

            // Tabs are currently not rendered at all. We replace them with spaces for rendering purposes.
            // https://github.com/ratatui-org/ratatui/issues/876
            spans.iter_mut().for_each(|span| replace_tabs(self.tab_width as usize, span));

            lines.push(Line::default().spans(spans));
        }

        assert!(number_width > 0, "number_width should include room for one space");

        for (i, line) in lines.iter_mut().enumerate() {
            // Set line number spans for each line.
            let style = Style::new().fg(Color::Rgb(0x58, 0x6e, 0x75));
            let line_number_span = match self.line_number_style {
                LineNumberStyle::Relative => {
                    let number = match self.cursor_line.cmp(&(i + self.line_offset)) {
                        cmp::Ordering::Less => i + self.line_offset - self.cursor_line,
                        cmp::Ordering::Equal => self.cursor_line + 1,
                        cmp::Ordering::Greater => self.cursor_line - i - self.line_offset,
                    };

                    Span::styled(format!(" {:width$} ", number, width = number_width - 1), style)
                }
                LineNumberStyle::Absolute => Span::styled(
                    format!(" {:width$} ", self.line_offset + i + 1, width = number_width - 1),
                    style,
                ),
                LineNumberStyle::None => Span::styled("  ", style),
            };

            line.spans[0] = line_number_span;
        }

        lines.iter().enumerate().for_each(|(i, line)| {
            buf.set_line(area.x, area.y + i as u16, line, area.width);
        });

        // + 1 for the ever present left padding space
        1 + number_width
    }
}

impl<'a, I> Widget for Lines<'a, I>
where
    I: Iterator<Item = (usize, Cow<'a, str>, Style)>,
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_(area, buf);
    }
}
