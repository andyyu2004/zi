mod element;
mod sequence;

use std::borrow::Cow;
use std::iter::Peekable;
use std::marker::PhantomData;
use std::ops::Range;

pub use ratatui::backend::{Backend, CrosstermBackend};
pub use ratatui::buffer::Buffer;
pub use ratatui::layout::{Constraint, Direction, Layout, Rect};
pub use ratatui::style::{Color, Style};
pub use ratatui::text::Text;
use ratatui::text::{Line, Span};
pub use ratatui::widgets::Widget;
pub use ratatui::{Frame, Terminal};

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
    let layout = Layout::new(Direction::Vertical, constraints);
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
    tab_width: u8,
    lines: I,
    highlights: Peekable<H>,
    _marker: PhantomData<&'a ()>,
}

impl<I, H: Iterator> Lines<'_, I, H> {
    pub fn new(tab_width: u8, lines: I, highlights: H) -> Self {
        Self { tab_width, lines, highlights: highlights.peekable(), _marker: PhantomData }
    }
}

impl<'a, I, H> Widget for Lines<'a, I, H>
where
    I: Iterator,
    I::Item: Into<Cow<'a, str>>,
    H: Iterator<Item = (Range<(usize, usize)>, Style)>,
{
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        for (i, line) in self.lines.enumerate() {
            if i >= area.height as usize {
                break;
            }

            let line = line.into();
            let mut spans = vec![];

            if let Some((range, _)) = self.highlights.peek() {
                assert!(range.start.0 >= i, "highlights got behind?");
            }

            let mut j = 0;
            while let Some((range, _)) = self.highlights.peek() {
                if range.start.0 > i {
                    break;
                }

                let (range, style) = self.highlights.next().expect("just peeked");
                let start = range.start.1;
                let end = range.end.1;

                if start < j {
                    // Sometimes highlights can overlap, we just arbitrarily use the first one of that range
                    continue;
                }

                if start > j {
                    spans.push(Span::raw(&line[j..start]));
                }

                spans.push(Span::styled(&line[start..end], style));
                j = end;
            }

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
