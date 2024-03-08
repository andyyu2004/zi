mod element;
mod sequence;

use std::borrow::Cow;
use std::iter::Peekable;
use std::marker::PhantomData;
use std::ops::Range;

use itertools::Itertools;
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

pub struct Lines<'a, I, H: Iterator> {
    /// The 0-indexed line number to start with
    line_start: usize,
    /// The width of the line number column
    line_nr_width: usize,
    tab_width: u8,
    lines: I,
    highlights: Peekable<H>,
    _marker: PhantomData<&'a ()>,
}

impl<I, H: Iterator> Lines<'_, I, H> {
    pub fn new(
        line_start: usize,
        line_nr_width: usize,
        tab_width: u8,
        lines: I,
        highlights: H,
    ) -> Self {
        Self {
            line_start,
            line_nr_width,
            tab_width,
            lines,
            highlights: highlights.peekable(),
            _marker: PhantomData,
        }
    }
}

impl<'a, I, H> Widget for Lines<'a, I, H>
where
    I: Iterator,
    I::Item: Into<Cow<'a, str>>,
    H: Iterator<Item = (Range<(usize, usize)>, Style)>,
{
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        for (pos, (i, line)) in self.lines.enumerate().with_position() {
            let line = line.into();

            if i >= area.height as usize {
                break;
            }

            if matches!(pos, itertools::Position::Last | itertools::Position::Only)
                && line.is_empty()
            {
                // Don't render the final empty line
                break;
            }

            let mut spans = vec![Span::styled(
                format!("{:width$} ", self.line_start + i + 1, width = self.line_nr_width),
                Style::new().fg(Color::Rgb(0x58, 0x6e, 0x75)),
            )];

            let mut j = 0;
            while let Some((range, style)) = self.highlights.peek() {
                if range.start.0 > i {
                    break;
                }

                let start = if range.start.0 == i { range.start.1 } else { 0 };
                if range.end.0 > i {
                    // If the highlight is a multi-line highlight,
                    // we style the entire line with that style and move on to highlight the next
                    // line (without next()ing the highlight iterator)
                    spans.push(Span::styled(&line[..], *style));
                    // set `j` here so we don't try to highlight the same range again
                    j = line.len();
                    break;
                }

                let (range, style) = self.highlights.next().expect("just peeked");
                let end = if range.end.0 == i { range.end.1 } else { line.len() };

                if start < j {
                    // Sometimes highlights can overlap, we just arbitrarily use the first one of that range
                    continue;
                }

                if start > j {
                    spans.push(Span::raw(&line[j..start]));
                }

                if end >= line.len() {
                    // We're allowed to highlight places with no text, so the range end might be out of bounds
                    // In which case, we add another span with the remaining space
                    spans.push(Span::styled(&line[start..], style));
                    spans.push(Span::styled(
                        format!("{:width$}", "", width = end - line.len()),
                        style,
                    ));
                } else {
                    spans.push(Span::styled(&line[start..end], style));
                }

                j = end;
            }

            // Add in a span for the rest of the line that wasn't highlighted
            if j < line.len() {
                spans.push(Span::raw(&line[j..]));
            }

            // Tabs are currently not rendered at all. We replace them with 4 spaces for rendering purposes.
            // https://github.com/ratatui-org/ratatui/issues/876
            for span in &mut spans {
                if span.content.contains('\t') {
                    span.content = span
                        .content
                        .replace('\t', &format!("{:width$}", "", width = self.tab_width as usize))
                        .into();
                }
            }

            let line = Line::default().spans(spans);
            // safe cast to u16 as we already checked that i < area.height (which is a u16)
            buf.set_line(area.x, area.y + i as u16, &line, area.width);
        }
    }
}
