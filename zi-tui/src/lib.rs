mod element;
mod sequence;

use std::borrow::Cow;
use std::iter::Peekable;
use std::marker::PhantomData;

pub use ratatui::backend::Backend;
pub use ratatui::buffer::Buffer;
pub use ratatui::layout::{Constraint, Direction, Layout, Rect};
pub use ratatui::style::{Color, Style};
pub use ratatui::text::{Line, Span, Text};
pub use ratatui::widgets::{Clear, Widget};
pub use ratatui::{backend, Frame, Terminal};

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

pub struct Lines<'a, I: Iterator> {
    /// The 0-indexed line number to start with
    line_start: usize,
    /// The width of the line number column
    line_nr_width: usize,
    tab_width: u8,
    chunks: Peekable<I>,
    _marker: PhantomData<&'a ()>,
}

impl<I: Iterator> Lines<'_, I> {
    pub fn new(line_start: usize, line_nr_width: usize, tab_width: u8, chunks: I) -> Self {
        Self {
            line_start,
            line_nr_width,
            tab_width,
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
            spans.push(Span::styled(
                format!("{:width$} ", self.line_start + i + 1, width = self.line_nr_width),
                Style::new().fg(Color::Rgb(0x58, 0x6e, 0x75)),
            ));

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
