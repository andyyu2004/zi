use stdx::merge::Merge;

use super::Backend;
use crate::editor::{set_error_if, SaveFlags};
use crate::keymap::Keymap;
use crate::{
    hashmap, motion, trie, Active, Direction, Editor, Mark, Mode, Operator, VerticalAlignment,
};

pub(super) fn new<B: Backend>() -> Keymap {
    // maybe should rewrite all as functions
    fn delete_operator<B: Backend>(editor: &mut Editor<B>) {
        editor.set_mode(Mode::OperatorPending(Operator::Delete));
    }

    fn change_operator_pending<B: Backend>(editor: &mut Editor<B>) {
        editor.set_mode(Mode::OperatorPending(Operator::Change));
    }

    fn yank_operator_pending<B: Backend>(editor: &mut Editor<B>) {
        editor.set_mode(Mode::OperatorPending(Operator::Yank));
    }

    fn insert_mode<B: Backend>(editor: &mut Editor<B>) {
        editor.set_mode(Mode::Insert);
    }

    fn command_mode<B: Backend>(editor: &mut Editor<B>) {
        editor.set_mode(Mode::Command);
    }

    fn insert_newline<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.insert_char(Active, '\n'));
    }

    fn normal_mode<B: Backend>(editor: &mut Editor<B>) {
        editor.set_mode(Mode::Normal);
    }

    fn prev_line<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.motion(Active, motion::PrevLine))
    }

    fn next_line<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.motion(Active, motion::NextLine))
    }

    fn prev_char<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.motion(Active, motion::PrevChar))
    }

    fn next_char<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.motion(Active, motion::NextChar))
    }

    fn goto_definition<B: Backend>(editor: &mut Editor<B>) {
        let fut = editor.goto_definition(Active);
        editor.schedule("go to definition", fut);
    }

    fn goto_declaration<B: Backend>(editor: &mut Editor<B>) {
        let fut = editor.goto_declaration(Active);
        editor.schedule("go to declaration", fut);
    }

    fn goto_implementation<B: Backend>(editor: &mut Editor<B>) {
        let fut = editor.goto_implementation(Active);
        editor.schedule("go to implementation", fut);
    }

    fn goto_type_definition<B: Backend>(editor: &mut Editor<B>) {
        let fut = editor.goto_type_definition(Active);
        editor.schedule("go to type definition", fut);
    }

    fn find_references<B: Backend>(editor: &mut Editor<B>) {
        let fut = editor.goto_references(Active);
        editor.schedule("find references", fut);
    }

    fn goto_start<B: Backend>(editor: &mut Editor<B>) {
        editor.scroll(Active, Direction::Up, usize::MAX);
    }

    fn goto_end<B: Backend>(editor: &mut Editor<B>) {
        editor.scroll(Active, Direction::Down, usize::MAX);
    }

    fn align_view_top<B: Backend>(editor: &mut Editor<B>) {
        let view = editor.view(Active).id();
        editor.align_view(view, VerticalAlignment::Top);
    }

    fn align_view_center<B: Backend>(editor: &mut Editor<B>) {
        let view = editor.view(Active).id();
        editor.align_view(view, VerticalAlignment::Center);
    }

    fn align_view_bottom<B: Backend>(editor: &mut Editor<B>) {
        let view = editor.view(Active).id();
        editor.align_view(view, VerticalAlignment::Bottom);
    }

    fn open_newline<B: Backend>(editor: &mut Editor<B>) {
        editor.set_mode(Mode::Insert);
        editor.set_cursor(Active, editor.cursor(Active).with_col(usize::MAX));
        set_error_if!(editor, editor.insert_char(Active, '\n'));
    }

    fn next_token<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.motion(Active, motion::NextToken));
    }

    fn prev_token<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.motion(Active, motion::PrevToken));
    }

    fn next_word<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.motion(Active, motion::NextWord))
    }

    fn prev_word<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.motion(Active, motion::PrevWord));
    }

    fn text_object_current_line_inclusive<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.text_object(Active, zi_textobject::Line::inclusive()));
    }

    fn text_object_current_line_exclusive<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.text_object(Active, zi_textobject::Line::exclusive()));
    }

    fn append_eol<B: Backend>(editor: &mut Editor<B>) {
        editor.set_cursor(Active, editor.cursor(Active).with_col(usize::MAX));
        editor.set_mode(Mode::Insert);
        editor.move_cursor(Active, Direction::Right, 1);
    }

    fn append<B: Backend>(editor: &mut Editor<B>) {
        editor.set_mode(Mode::Insert);
        editor.move_cursor(Active, Direction::Right, 1);
    }

    fn scroll_line_down<B: Backend>(editor: &mut Editor<B>) {
        editor.scroll(Active, Direction::Down, 1);
    }

    fn scroll_line_up<B: Backend>(editor: &mut Editor<B>) {
        editor.scroll(Active, Direction::Up, 1);
    }

    fn scroll_down<B: Backend>(editor: &mut Editor<B>) {
        editor.scroll(Active, Direction::Down, 20);
    }

    fn scroll_up<B: Backend>(editor: &mut Editor<B>) {
        editor.scroll(Active, Direction::Up, 20);
    }

    fn open_file_picker<B: Backend>(editor: &mut Editor<B>) {
        editor.open_file_picker(".");
    }

    fn open_global_search<B: Backend>(editor: &mut Editor<B>) {
        editor.open_global_search(".");
    }

    fn open_file_explorer<B: Backend>(editor: &mut Editor<B>) {
        editor.open_file_explorer(".");
    }

    fn split_vertical<B: Backend>(editor: &mut Editor<B>) {
        editor.split(Active, Direction::Right, tui::Constraint::Fill(1));
    }

    fn split_horizontal<B: Backend>(editor: &mut Editor<B>) {
        editor.split(Active, Direction::Down, tui::Constraint::Fill(1));
    }

    fn focus_left<B: Backend>(editor: &mut Editor<B>) {
        editor.focus_direction(Direction::Left);
    }

    fn focus_right<B: Backend>(editor: &mut Editor<B>) {
        editor.focus_direction(Direction::Right);
    }

    fn focus_up<B: Backend>(editor: &mut Editor<B>) {
        editor.focus_direction(Direction::Up);
    }

    fn focus_down<B: Backend>(editor: &mut Editor<B>) {
        editor.focus_direction(Direction::Down);
    }

    fn view_only<B: Backend>(editor: &mut Editor<B>) {
        editor.view_only(editor.view(Active).id())
    }

    fn undo<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.undo(Active))
    }

    fn redo<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.redo(Active))
    }

    fn search<B: Backend>(editor: &mut Editor<B>) {
        let _ = editor.search("");
    }

    fn save<B: Backend>(editor: &mut Editor<B>) {
        let fut = editor.save(Active, SaveFlags::empty());
        editor.schedule("save", fut);
    }

    fn backspace<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.delete_char(Active));
    }

    fn jump_forward<B: Backend>(editor: &mut Editor<B>) {
        editor.jump_forward(Active);
    }

    fn jump_back<B: Backend>(editor: &mut Editor<B>) {
        editor.jump_back(Active);
    }

    fn inspect<B: Backend>(editor: &mut Editor<B>) {
        editor.inspect(Active);
    }

    fn open_jump_list<B: Backend>(editor: &mut Editor<B>) {
        editor.open_jump_list(Active);
    }

    fn open_diagnostics<B: Backend>(editor: &mut Editor<B>) {
        editor.open_diagnostics();
    }

    fn open_marks<B: Backend>(editor: &mut Editor<B>) {
        editor.open_marks(Active);
    }

    fn tab<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.tab(Active))
    }

    fn execute_buffered_command<B: Backend>(editor: &mut Editor<B>) {
        set_error_if!(editor, editor.execute_buffered_command());
    }

    fn goto_next_match<B: Backend>(editor: &mut Editor<B>) {
        editor.goto_next_match();
    }

    fn goto_prev_match<B: Backend>(editor: &mut Editor<B>) {
        editor.goto_prev_match();
    }

    fn tmp_create_mark_test<B: Backend>(editor: &mut Editor<B>) {
        let cursor = editor.cursor(Active);
        let byte = editor.buffer(Active).text().point_to_byte(cursor);
        let hl = editor.highlight_id_by_name(crate::syntax::HighlightName::ERROR);
        editor.create_mark(
            Active,
            editor.default_namespace(),
            Mark::builder(byte).hl(hl).width(5).start_bias(zi_marktree::Bias::Left),
        );
    }

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
            "<CR>" => insert_newline,
            "<BS>" => backspace,
            "<Tab>" => tab,
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
            "d" => delete_operator,
            "c" => change_operator_pending,
            "y" => yank_operator_pending,
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
}
