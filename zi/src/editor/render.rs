use std::borrow::Cow;
use std::fmt;

use stdx::iter::IteratorExt;
use stdx::merge::Merge;
use tui::{Rect, StatefulWidget, Widget as _};
use zi_core::{IteratorRangeExt, Offset, PointRange};
use zi_text::{AnyTextSlice, PointRangeExt, Text, TextSlice};

use super::{get_ref, Editor, State};
use crate::completion::Completion;
use crate::editor::Resource;
use crate::syntax::HighlightName;
use crate::{Active, ViewId};

impl Editor {
    pub fn render(&mut self, frame: &mut impl tui::DynFrame) {
        let buffer_area = frame.buffer_mut().area;
        let tree_area = self.tree.area();
        assert!(buffer_area.height >= tree_area.height + Self::BOTTOM_BAR_HEIGHT);
        let client = self.client();

        tracing::debug!(%tree_area, %buffer_area, "render editor");

        // Only iterate over the views that are in the view tree, as otherwise they are definitely
        // not visible and we don't need to render them.
        self.tree.views().for_each(|view| {
            let view = &self.views[view];
            let buf = &mut self.buffers[view.buffer()];
            let area = self.tree.view_area(view.id());
            buf.pre_render(&client, view, area)
        });

        self.tree.render(self, frame.buffer_mut());

        // HACK probably there is a nicer way to not special case the cmd and statusline
        let (view, buf) = get_ref!(self);
        let path = buf.file_path();
        let path = path.as_ref().map(|p| p.display());
        let display = path
            .as_ref()
            .map_or_else(|| buf.url() as &dyn fmt::Display, |p| p as &dyn fmt::Display);

        let mut status_spans = vec![tui::Span::styled(
            format!("{}:{}:{} ", display, view.cursor().line() + 1_usize, view.cursor().col()),
            tui::Style::new()
                .fg(tui::Color::Rgb(0x88, 0x88, 0x88))
                .bg(tui::Color::Rgb(0x07, 0x36, 0x42)),
        )];

        // The error should probably go in the cmd line not the status line.
        if let Some(error) = &self.status_error {
            status_spans.push(tui::Span::styled(
                error,
                tui::Style::new()
                    .fg(tui::Color::Rgb(0xff, 0x00, 0x00))
                    .bg(tui::Color::Rgb(0x07, 0x36, 0x42)),
            ));
        }

        // FIXME probably a better way than manually padding the right
        status_spans.push(tui::Span::styled(
            " ".repeat(tree_area.width as usize),
            tui::Style::new()
                .fg(tui::Color::Rgb(0x88, 0x88, 0x88))
                .bg(tui::Color::Rgb(0x07, 0x36, 0x42)),
        ));

        let status = tui::Line::default().spans(status_spans);

        let cmd = tui::Text::styled(
            match &self.state {
                State::Command(state) => Cow::Borrowed(state.buffer.as_str()),
                State::Normal(..) | State::OperatorPending(..) => Cow::Borrowed(""),
                state => Cow::Owned(format!("-- {} --", state.mode())),
            },
            tui::Style::new()
                .fg(tui::Color::Rgb(0x88, 0x88, 0x88))
                .bg(tui::Color::Rgb(0x00, 0x2b, 0x36)),
        );

        let widget = tui::vstack([tui::Constraint::Max(1), tui::Constraint::Max(1)], (status, cmd));

        widget.render(
            tui::Rect {
                x: 0,
                y: tree_area.height,
                width: tree_area.width,
                height: Self::BOTTOM_BAR_HEIGHT,
            },
            frame.buffer_mut(),
        );

        let (x, y) = self.cursor_viewport_coords();
        let offset = match &self.state {
            State::Command(state) => {
                state.buffer.len().checked_sub(1).expect("should have a preceding `/` or `:`")
                    as u16
            }
            _ => view.number_width.get(),
        };

        frame.set_cursor(x + offset, y);
    }

    #[tracing::instrument(skip_all)]
    pub(crate) fn render_view(&self, area: Rect, surface: &mut tui::Buffer, view: ViewId) {
        assert_eq!(surface.area.intersection(area), area);

        let theme = self.theme();
        let theme = theme.read();
        let background = self
            .highlight_id_by_name(HighlightName::BACKGROUND)
            .style(&theme)
            .unwrap_or_else(|| theme.default_style());

        surface.set_style(area, background);
        let number_width = self.render_view_content(area, surface, view);
        self[view].number_width.set(number_width as u16);

        if view == self.view(Active).id() {
            self.render_completion(area, surface, view);
        }
    }

    fn render_completion(&self, view_area: Rect, surface: &mut tui::Buffer, view: ViewId) {
        let State::Insert(state) = &self.state else { return };
        let Completion::Active(state) = &state.completion else { return };

        if state.matches().len() == 0 {
            return;
        }

        let height = state.matches().take(20).len() as u16;
        let start_point = self.text(view).byte_to_point(state.start_byte());
        let offset = self[view].offset();
        let area = Rect {
            x: view_area.x + self[view].number_width.get() + start_point.col() as u16
                - offset.col as u16,
            y: view_area.y + start_point.line() as u16 - offset.line as u16 + 1,
            height,
            width: 50,
        }
        .intersection(view_area);

        tui::Clear.render(area, surface);
        let list = tui::List::new(state.matches().map(|item| {
            tui::ListItem::new(tui::Text::from(&*item.label).left_aligned()).style(
                tui::Style::default()
                    .bg(tui::Color::Rgb(0x07, 0x36, 0x42))
                    .fg(tui::Color::Rgb(0x88, 0x88, 0x88)),
            )
        }))
        .scroll_padding(3)
        .highlight_style(
            tui::Style::default()
                .bg(tui::Color::Rgb(0x00, 0x2b, 0x36))
                .fg(tui::Color::Rgb(0x88, 0x88, 0x88)),
        );

        StatefulWidget::render(list, area, surface, &mut state.widget_state());
    }

    fn render_view_content(&self, area: Rect, surface: &mut tui::Buffer, view: ViewId) -> usize {
        let theme = self.theme();
        let theme = theme.read();
        let mut query_cursor = tree_sitter::QueryCursor::new();
        query_cursor.set_match_limit(256);
        let view = &self[view];
        let buf = self.buffer(view.buffer());
        let text = buf.text();

        let line_offset = view.offset().line;
        let relevant_point_range =
            PointRange::new((line_offset, 0usize), (line_offset + area.height as usize, 0usize));
        let relevant_byte_range = {
            let start_byte = text.line_to_byte(line_offset);
            let end_byte = match text.try_line_to_byte(line_offset + area.height as usize) {
                Some(end) => end + text.line(line_offset).unwrap().len_bytes(),
                None => text.len_bytes(),
            };
            start_byte..end_byte
        };

        let syntax_highlights = buf
            .syntax_highlights(self, &mut query_cursor, relevant_point_range)
            .skip_while(|hl| hl.range.end().line() < line_offset)
            .filter_map(|hl| Some((hl.range, hl.id.style(&theme)?)));

        let mark_highlights = buf
            .marks(relevant_byte_range)
            .filter(|(_, range, _)| !range.is_empty())
            .filter_map(|(_, byte_range, mark)| {
                let style = mark.highlight().style(&theme)?;
                let point_range = text.byte_range_to_point_range(&byte_range);
                if !point_range.is_subrange_of(relevant_point_range) {
                    return None;
                }

                Some(point_range.explode(text).map(move |range| (range, style)))
            })
            .flatten();

        let overlay_highlights = buf
            .overlay_highlights(self, view, area.into())
            .skip_while(|hl| hl.range.end().line() < line_offset)
            .filter_map(|hl| Some((hl.range, hl.id.style(&theme)?)));

        let view_highlights = syntax_highlights
            .range_merge(overlay_highlights)
            .range_merge(mark_highlights)
            .inspect(|(range, style)| {
                tracing::trace!(%range, %style, "highlight");
            });

        let search_highlights = self
            .search_state
            .hlsearch
            .then(|| self.search_state.matches())
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter_map(|(i, mat)| {
                let range = text.byte_range_to_point_range(&mat.byte_range);
                if range.end().line() < line_offset {
                    return None;
                }

                let hl_name = if self.search_state.current_match_idx() == i {
                    HighlightName::CURRENT_SEARCH
                } else {
                    HighlightName::SEARCH
                };

                let style = self.highlight_id_by_name(hl_name).style(&theme)?;
                Some((range, style))
            });

        let highlights = view_highlights
            .range_merge(search_highlights)
            .map(|(range, style)| (range - Offset::new(line_offset, 0), style));

        let text = buf.text();
        let lines = text
            .line_slice(line_offset..)
            .lines()
            // We always want to render a line even if the buffer is empty.
            .default_if_empty(|| Box::new("") as Box<dyn AnyTextSlice<'_>>);

        let chunks = zi_text::annotate(lines, highlights);

        let lines = tui::Lines::new(
            line_offset,
            view.cursor().line(),
            *view.settings().line_number_style.read(),
            *buf.settings().tab_width.read(),
            *view.settings().line_number_width.read(),
            chunks.inspect(|(_, text, _)| tracing::trace!(?text, "render chunk")).map(
                |(line, text, style)| {
                    let default_style = theme.default_style();
                    // The merge is still necessary to fill in the missing fields in the style.
                    let style = default_style.merge(style.unwrap_or(default_style));
                    (line, text, style.into())
                },
            ),
        );

        lines.render_(area, surface)
    }
}
