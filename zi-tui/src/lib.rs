mod element;
mod sequence;

use std::borrow::Cow;
use std::marker::PhantomData;

pub use ratatui::backend::{Backend, CrosstermBackend};
pub use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
pub use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::Line;
pub use ratatui::text::Text;
pub use ratatui::widgets::Widget;
pub use ratatui::Terminal;

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

#[derive(Debug, Clone)]
pub struct Lines<'a, I> {
    lines: I,
    _marker: PhantomData<&'a ()>,
}

impl<I> Lines<'_, I> {
    pub fn new(lines: I) -> Self {
        Self { lines, _marker: PhantomData }
    }
}

impl<'a, I> Widget for Lines<'a, I>
where
    I: Iterator,
    I::Item: Into<Cow<'a, str>>,
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (i, line) in self.lines.enumerate() {
            if i >= area.height as usize {
                break;
            }
            let line = Line::default().spans([line]);
            // safe cast to u16 as we already checked that i < area.height (which is a u16)
            buf.set_line(area.x, area.y + i as u16, &line, area.width);
        }
    }
}
