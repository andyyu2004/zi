use std::sync::OnceLock;

use stdx::merge::Merge;

use crate::editor::{Action, SaveFlags, set_error_if};
use crate::input::KeyEvent;
use crate::keymap::Keymap;
use crate::{
    Active, Direction, Editor, Mark, Mode, Operator, VerticalAlignment, hashmap, motion, trie,
};

pub(super) fn new() -> Keymap {
    static KEYMAP: OnceLock<Keymap<Mode, KeyEvent, Action>> = OnceLock::new();

    fn delete_operator_pending(editor: &mut Editor) {
        editor.set_mode(Mode::OperatorPending(Operator::Delete));
    }

    fn change_operator_pending(editor: &mut Editor) {
        editor.set_mode(Mode::OperatorPending(Operator::Change));
    }

    fn yank_operator_pending(editor: &mut Editor) {
        editor.set_mode(Mode::OperatorPending(Operator::Yank));
    }

    fn delete_till_end_of_line(editor: &mut Editor) {
        delete_operator_pending(editor);
        set_error_if!(editor, editor.text_object(Active, zi_textobject::Until('\n')));
    }

    fn change_till_end_of_line(editor: &mut Editor) {
        change_operator_pending(editor);
        set_error_if!(editor, editor.text_object(Active, zi_textobject::Until('\n')));
    }

    fn insert_mode(editor: &mut Editor) {
        editor.set_mode(Mode::Insert);
    }

    fn command_mode(editor: &mut Editor) {
        editor.set_mode(Mode::Command);
    }

    fn insert_newline(editor: &mut Editor) {
        set_error_if!(editor, editor.insert_char(Active, '\n'));
    }

    fn normal_mode(editor: &mut Editor) {
        editor.set_mode(Mode::Normal);
    }

    fn prev_line(editor: &mut Editor) {
        set_error_if!(editor, editor.motion(Active, motion::PrevLine))
    }

    fn next_line(editor: &mut Editor) {
        set_error_if!(editor, editor.motion(Active, motion::NextLine))
    }

    fn prev_char(editor: &mut Editor) {
        set_error_if!(editor, editor.motion(Active, motion::PrevChar))
    }

    fn next_char(editor: &mut Editor) {
        set_error_if!(editor, editor.motion(Active, motion::NextChar))
    }

    fn goto_definition(editor: &mut Editor) {
        let fut = editor.goto_definition(Active);
        editor.spawn("go to definition", fut);
    }

    fn goto_declaration(editor: &mut Editor) {
        let fut = editor.goto_declaration(Active);
        editor.spawn("go to declaration", fut);
    }

    fn goto_implementation(editor: &mut Editor) {
        let fut = editor.goto_implementation(Active);
        editor.spawn("go to implementation", fut);
    }

    fn goto_type_definition(editor: &mut Editor) {
        let fut = editor.goto_type_definition(Active);
        editor.spawn("go to type definition", fut);
    }

    fn find_references(editor: &mut Editor) {
        let fut = editor.goto_references(Active);
        editor.spawn("find references", fut);
    }

    fn goto_start(editor: &mut Editor) {
        editor.scroll(Active, Direction::Up, usize::MAX);
    }

    fn goto_end(editor: &mut Editor) {
        editor.scroll(Active, Direction::Down, usize::MAX);
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
        editor.set_cursor(Active, editor.cursor(Active).with_col(usize::MAX));
        set_error_if!(editor, editor.insert_char(Active, '\n'));
    }

    fn next_token(editor: &mut Editor) {
        set_error_if!(editor, editor.motion(Active, motion::NextToken));
    }

    fn prev_token(editor: &mut Editor) {
        set_error_if!(editor, editor.motion(Active, motion::PrevToken));
    }

    fn next_word(editor: &mut Editor) {
        set_error_if!(editor, editor.motion(Active, motion::NextWord))
    }

    fn prev_word(editor: &mut Editor) {
        set_error_if!(editor, editor.motion(Active, motion::PrevWord));
    }

    fn matchit(editor: &mut Editor) {
        set_error_if!(editor, editor.motion(Active, motion::MatchIt))
    }

    fn text_object_current_line_inclusive(editor: &mut Editor) {
        set_error_if!(editor, editor.text_object(Active, zi_textobject::Line::inclusive()));
    }

    fn text_object_current_line_exclusive(editor: &mut Editor) {
        set_error_if!(editor, editor.text_object(Active, zi_textobject::Line::exclusive()));
    }

    fn append_eol(editor: &mut Editor) {
        editor.set_cursor(Active, editor.cursor(Active).with_col(usize::MAX));
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

    fn open_file_picker_here(editor: &mut Editor) {
        match editor.buffer(Active).path() {
            Some(path) if path.is_dir() => editor.open_file_picker(path),
            Some(path) => {
                editor.open_file_picker(path.parent().expect("file must have parent path"))
            }
            None => editor.open_file_picker("."),
        };
    }

    fn open_global_search(editor: &mut Editor) {
        editor.open_global_search(".");
    }

    fn open_file_explorer(editor: &mut Editor) {
        if let Some(path) = editor.buffer(Active).path().as_ref().and_then(|p| p.parent()) {
            editor.open_file_explorer(path);
        } else {
            editor.open_file_explorer(".");
        }
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
        editor.view_only(editor.view(Active).id())
    }

    fn undo(editor: &mut Editor) {
        set_error_if!(editor, editor.undo(Active))
    }

    fn redo(editor: &mut Editor) {
        set_error_if!(editor, editor.redo(Active))
    }

    fn search(editor: &mut Editor) {
        let _ = editor.search("");
    }

    fn save(editor: &mut Editor) {
        let fut = editor.save(Active, SaveFlags::empty());
        editor.spawn("save", fut);
    }

    fn backspace(editor: &mut Editor) {
        set_error_if!(editor, editor.delete_char(Active));
    }

    fn jump_forward(editor: &mut Editor) {
        editor.jump_forward(Active);
    }

    fn jump_back(editor: &mut Editor) {
        editor.jump_back(Active);
    }

    fn inspect(editor: &mut Editor) {
        editor.inspect(Active);
    }

    fn open_jump_list(editor: &mut Editor) {
        editor.open_jump_list(Active);
    }

    fn open_diagnostics(editor: &mut Editor) {
        editor.open_diagnostics();
    }

    fn open_marks(editor: &mut Editor) {
        editor.open_marks(Active);
    }

    fn tab(editor: &mut Editor) {
        set_error_if!(editor, editor.tab())
    }

    fn backtab(editor: &mut Editor) {
        set_error_if!(editor, editor.backtab())
    }

    fn trigger_completion(editor: &mut Editor) {
        editor.trigger_completion(None)
    }

    fn execute_buffered_command(editor: &mut Editor) {
        set_error_if!(editor, editor.execute_buffered_command());
    }

    fn goto_next_match(editor: &mut Editor) {
        editor.goto_next_match();
    }

    fn goto_prev_match(editor: &mut Editor) {
        editor.goto_prev_match();
    }

    fn tmp_create_mark_test(editor: &mut Editor) {
        let cursor = editor.cursor(Active);
        let byte = editor.buffer(Active).text().point_to_byte(cursor);
        let hl = editor.highlight_id_by_name(crate::syntax::HighlightName::ERROR);
        editor.create_mark(
            Active,
            editor.default_namespace(),
            Mark::builder(byte).hl(hl).width(5).start_bias(zi_marktree::Bias::Left),
        );
    }

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
                "k" => prev_line,
                "j" => next_line,
                "l" => next_char,
            });

            Keymap::from(hashmap! {
                Mode::Command => trie!({
                    "<ESC>" | "<C-c>" => normal_mode,
                    "<BS>" => backspace,
                    "<CR>" => execute_buffered_command,
                }),
                Mode::Insert => trie!({
                    "<ESC>" | "<C-c>" => normal_mode,
                    "<C-Space>" => trigger_completion,
                    "<CR>" => insert_newline,
                    "<BS>" => backspace,
                    "<Tab>" => tab,
                    "<S-Tab>" => backtab,
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
                    "<C-s>" => save,
                    "<C-o>" => jump_back,
                    "<C-i>" => jump_forward,
                    "<C-d>" => scroll_down,
                    "<C-u>" => scroll_up,
                    "<C-e>" => scroll_line_down,
                    "<C-y>" => scroll_line_up,
                    "<Tab>" => tab,
                    "m" => tmp_create_mark_test,
                    "d" => delete_operator_pending,
                    "c" => change_operator_pending,
                    "y" => yank_operator_pending,
                    "C" => change_till_end_of_line,
                    "D" => delete_till_end_of_line,
                    "%" => matchit,
                    ":" => command_mode,
                    "/" => search,
                    "i" => insert_mode,
                    "h" => prev_char,
                    "l" => next_char,
                    "j" => next_line,
                    "k" => prev_line,
                    // "j" => move_down,
                    // "k" => move_up,
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
                    "n" => goto_next_match,
                    "N" => goto_prev_match,
                    "G" => goto_end,
                    "<space>" => {
                        "e" => open_file_explorer,
                        "o" => open_file_picker,
                        "f" => open_file_picker_here,
                        "j" => open_jump_list,
                        "l" => open_diagnostics,
                        "m" => open_marks,
                        "/" => open_global_search,
                    },
                    "g" => {
                        "d" => goto_definition,
                        "D" => goto_declaration,
                        "i" => goto_implementation,
                        "t" => goto_type_definition,
                        "r" => find_references,
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
