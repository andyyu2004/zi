use std::sync::OnceLock;

use stdx::merge::Merge;

use crate::editor::Action;
use crate::input::KeyEvent;
use crate::keymap::Keymap;
use crate::{hashmap, motion, trie, Active, Direction, Editor, Mode, Operator, VerticalAlignment};

pub(super) fn new() -> Keymap {
    static KEYMAP: OnceLock<Keymap<Mode, KeyEvent, Action>> = OnceLock::new();

    // maybe should rewrite all as functions
    fn delete_operator(editor: &mut Editor) {
        editor.set_mode(Mode::OperatorPending(Operator::Delete));
    }

    fn change_operator_pending(editor: &mut Editor) {
        editor.set_mode(Mode::OperatorPending(Operator::Change));
    }

    fn yank_operator_pending(editor: &mut Editor) {
        editor.set_mode(Mode::OperatorPending(Operator::Yank));
    }

    fn insert_mode(editor: &mut Editor) {
        editor.set_mode(Mode::Insert);
    }

    fn command_mode(editor: &mut Editor) {
        editor.set_mode(Mode::Command);
    }

    fn insert_newline(editor: &mut Editor) {
        editor.insert_char_at_cursor('\n');
    }

    fn normal_mode(editor: &mut Editor) {
        editor.set_mode(Mode::Normal);
    }

    fn prev_char(editor: &mut Editor) {
        editor.motion(motion::PrevChar);
    }

    fn next_char(editor: &mut Editor) {
        editor.motion(motion::NextChar);
    }

    fn move_up(editor: &mut Editor) {
        editor.move_cursor(Active, Direction::Up, 1);
    }

    fn move_down(editor: &mut Editor) {
        editor.move_cursor(Active, Direction::Down, 1);
    }

    fn goto_definition(editor: &mut Editor) {
        editor.goto_definition();
    }

    fn goto_start(editor: &mut Editor) {
        editor.scroll(Active, Direction::Up, u32::MAX);
    }

    fn goto_end(editor: &mut Editor) {
        editor.scroll(Active, Direction::Down, u32::MAX);
    }

    fn align_view_top(editor: &mut Editor) {
        let view = editor.view(Active).id();
        editor.align_view(view, VerticalAlignment::Top);
    }

    fn align_view_center(editor: &mut Editor) {
        let view = editor.view(Active).id();
        editor.align_view(view, VerticalAlignment::Center);
    }

    fn align_view_bottom(editor: &mut Editor) {
        let view = editor.view(Active).id();
        editor.align_view(view, VerticalAlignment::Bottom);
    }

    fn open_newline(editor: &mut Editor) {
        editor.set_mode(Mode::Insert);
        editor.set_cursor(Active, editor.cursor(Active).with_col(u32::MAX));
        editor.insert_char_at_cursor('\n');
    }

    fn next_token(editor: &mut Editor) {
        editor.motion(motion::NextToken);
    }

    fn prev_token(editor: &mut Editor) {
        editor.motion(motion::PrevToken);
    }

    fn next_word(editor: &mut Editor) {
        editor.motion(motion::NextWord);
    }

    fn prev_word(editor: &mut Editor) {
        editor.motion(motion::PrevWord);
    }

    fn text_object_current_line_inclusive(editor: &mut Editor) {
        editor.text_object(zi_textobject::Line::inclusive());
    }

    fn text_object_current_line_exclusive(editor: &mut Editor) {
        editor.text_object(zi_textobject::Line::exclusive());
    }

    fn append_eol(editor: &mut Editor) {
        editor.set_cursor(Active, editor.cursor(Active).with_col(u32::MAX));
        editor.set_mode(Mode::Insert);
        editor.move_cursor(Active, Direction::Right, 1);
    }

    fn append(editor: &mut Editor) {
        editor.set_mode(Mode::Insert);
        editor.move_cursor(Active, Direction::Right, 1);
    }

    fn scroll_line_down(editor: &mut Editor) {
        editor.scroll(Active, Direction::Down, 1);
    }

    fn scroll_line_up(editor: &mut Editor) {
        editor.scroll(Active, Direction::Up, 1);
    }

    fn scroll_down(editor: &mut Editor) {
        editor.scroll(Active, Direction::Down, 20);
    }

    fn scroll_up(editor: &mut Editor) {
        editor.scroll(Active, Direction::Up, 20);
    }

    fn open_file_picker(editor: &mut Editor) {
        editor.open_file_picker(".");
    }

    fn open_global_search(editor: &mut Editor) {
        editor.open_global_search(".");
    }

    fn open_file_explorer(editor: &mut Editor) {
        editor.open_file_explorer(".");
    }

    fn split_vertical(editor: &mut Editor) {
        editor.split(Active, Direction::Right, tui::Constraint::Fill(1));
    }

    fn split_horizontal(editor: &mut Editor) {
        editor.split(Active, Direction::Down, tui::Constraint::Fill(1));
    }

    fn focus_left(editor: &mut Editor) {
        editor.focus_direction(Direction::Left);
    }

    fn focus_right(editor: &mut Editor) {
        editor.focus_direction(Direction::Right);
    }

    fn focus_up(editor: &mut Editor) {
        editor.focus_direction(Direction::Up);
    }

    fn focus_down(editor: &mut Editor) {
        editor.focus_direction(Direction::Down);
    }

    fn view_only(editor: &mut Editor) {
        editor.view_only(editor.view(Active).id());
    }

    fn undo(editor: &mut Editor) {
        editor.undo(Active);
    }

    fn redo(editor: &mut Editor) {
        editor.redo(Active);
    }

    macro_rules! action {
        ($($name:ident),*) => {
            $(
                fn $name(editor: &mut Editor) {
                    editor.$name();
                }
            )*
        };
    }

    action!(jump_prev, jump_next, inspect, delete_char_backward, execute_command, open_jump_list);

    // Apparently the key event parser is slow, so we need to cache the keymap to help fuzzing run faster.
    KEYMAP
        .get_or_init(|| {
            let operator_pending_trie = trie!({
                "<ESC>" | "<C-c>" => normal_mode,
                "w" => next_word,
                "W" => next_token,
                "b" => prev_word,
                "B" => prev_token,
                "h" => prev_char,
                "l" => next_char,
            });

            Keymap::from(hashmap! {
                Mode::Command => trie!({
                    "<ESC>" | "<C-c>" => normal_mode,
                    "<BS>" => delete_char_backward,
                    "<CR>" => execute_command,
                }),
                Mode::Insert => trie!({
                    "<ESC>" | "<C-c>" => normal_mode,
                    "<CR>" => insert_newline,
                    "<BS>" => delete_char_backward,
                    "f" => {
                        "d" => normal_mode,
                    },
                }),
                Mode::OperatorPending(Operator::Delete) => operator_pending_trie.clone().merge(trie!({
                    "d" => text_object_current_line_inclusive,
                })),
                Mode::OperatorPending(Operator::Change) => operator_pending_trie.clone().merge(trie!({
                    "c" => text_object_current_line_exclusive,
                })),
                Mode::OperatorPending(Operator::Yank) => operator_pending_trie.merge(trie!({
                    "y" => text_object_current_line_exclusive,
                })),
                Mode::Normal => trie!({
                    "<C-o>" => jump_prev,
                    "<C-i>" => jump_next,
                    "<C-d>" => scroll_down,
                    "<C-u>" => scroll_up,
                    "<C-e>" => scroll_line_down,
                    "<C-y>" => scroll_line_up,
                    "d" => delete_operator,
                    "c" => change_operator_pending,
                    "y" => yank_operator_pending,
                    ":" => command_mode,
                    "i" => insert_mode,
                    "h" => prev_char,
                    "l" => next_char,
                    "j" => move_down,
                    "k" => move_up,
                    "o" => open_newline,
                    "w" => next_word,
                    "b" => prev_word,
                    "W" => next_token,
                    "B" => prev_token,
                    "a" => append,
                    "A" => append_eol,
                    "u" => undo,
                    "<C-r>" => redo,
                    "<C-h>" => focus_left,
                    "<C-j>" => focus_down,
                    "<C-k>" => focus_up,
                    "<C-l>" => focus_right,
                    "-" => open_file_explorer,
                    "G" => goto_end,
                    "<space>" => {
                        "e" => open_file_explorer,
                        "o" => open_file_picker,
                        "j" => open_jump_list,
                        "/" => open_global_search,
                    },
                    "g" => {
                        "d" => goto_definition,
                        "g" => goto_start,
                    },
                    "t" => {
                        "s" => inspect,
                    },
                    "z" => {
                        "t" => align_view_top,
                        "z" => align_view_center,
                        "b" => align_view_bottom,
                    },
                    "<C-w>" => {
                        "o" => view_only,
                        "v" | "<C-v>" => split_vertical,
                        "s" | "<C-s>" => split_horizontal,
                        "h" | "<C-h>" => focus_left,
                        "k" | "<C-k>" => focus_up,
                        "j" | "<C-j>" => focus_down,
                        "l" | "<C-l>" => focus_right,
                    },
                }),
            })
        })
        .clone()
}
