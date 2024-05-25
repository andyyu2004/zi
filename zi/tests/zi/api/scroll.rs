use zi::Direction::*;

use crate::{new, new_cx_with_size};

#[tokio::test]
async fn cursor_scrolls_when_out_of_bounds() {
    let cx = new_cx_with_size(zi::Size::new(10, 4), "1\n2\n3\n4\n5\n").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.scroll(zi::Active, Down, 1);
        assert_eq!(editor.view(zi::Active).offset(), (1, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (1, 0));

        // scrolling the cursor off the screen should also scroll the view
        editor.move_cursor(zi::Active, Up, 1);
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (0, 0));

        editor.move_cursor(zi::Active, Down, 1);
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (1, 0));

        // scrolling down past the editor size
        editor.move_cursor(zi::Active, Down, 1);
        assert_eq!(editor.view(zi::Active).offset(), (1, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (2, 0));

        editor.move_cursor(zi::Active, Down, 2);
        assert_eq!(editor.view(zi::Active).offset(), (3, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (4, 0));
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn scroll() {
    let cx = new("").await;
    cx.with(|editor| {
        editor.scroll(zi::Active, Down, 1);
        assert_eq!(editor.cursor_line(), "");
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (0, 0));
    })
    .await;
    cx.cleanup().await;

    let cx = new("foo\nbar").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        assert_eq!(editor.cursor_line(), "foo");
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (0, 0));

        editor.scroll(zi::Active, Down, 1);
        assert_eq!(editor.view(zi::Active).offset(), (1, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (1, 0));
        assert_eq!(editor.cursor_line(), "bar");

        editor.scroll(zi::Active, Up, 1);
        assert_eq!(editor.cursor_line(), "foo");
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));

        // Bounds check above
        editor.scroll(zi::Active, Up, 1);
        assert_eq!(editor.cursor_line(), "foo");
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));

        // Bounds check below
        // Should not be able to scroll where all text is not visible.
        // There should always be at least one line visible.
        editor.scroll(zi::Active, Down, 1);
        assert_eq!(editor.view(zi::Active).offset(), (1, 0));
        assert_eq!(editor.cursor_line(), "bar");

        editor.scroll(zi::Active, Up, 50);
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (0, 0));
        assert_eq!(editor.cursor_line(), "foo");

        editor.scroll(zi::Active, Down, 50);
        assert_eq!(editor.view(zi::Active).offset(), (1, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (1, 0));
        assert_eq!(editor.cursor_line(), "bar");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn scroll_overflow() {
    let cx = new(&"a\n".repeat(20)).await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.move_cursor(zi::Active, Down, 1);
        assert_eq!(editor.cursor(zi::Active), (1, 0));

        editor.scroll(zi::Active, Down, usize::MAX);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn scroll_bounds_check() {
    let cx = new("1\n2\n3\n4\n5").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        // Move cursor down one so it's desyned from the scroll
        editor.move_cursor(zi::Active, Down, 1);
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (1, 0));

        // Scroll as far as possible
        editor.scroll(zi::Active, Down, 20);

        // Ensure the cursor is still following the scroll
        assert_eq!(editor.view(zi::Active).offset(), (4, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (4, 0));
    })
    .await;
    cx.cleanup().await;
}
