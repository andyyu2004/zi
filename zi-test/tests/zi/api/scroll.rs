use zi::Direction::*;

use crate::new;

#[tokio::test]
async fn cursor_scrolls_when_out_of_bounds() {
    let cx = new("1\n2\n3\n4\n5\n").with_size((10, 4)).await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.scroll_by(zi::Active, Down, 1);
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
        editor.scroll_by(zi::Active, Down, 1);
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

        editor.scroll_by(zi::Active, Down, 1);
        assert_eq!(editor.view(zi::Active).offset(), (1, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (1, 0));
        assert_eq!(editor.cursor_line(), "bar");

        editor.scroll_by(zi::Active, Up, 1);
        assert_eq!(editor.cursor_line(), "foo");
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));

        // Bounds check above
        editor.scroll_by(zi::Active, Up, 1);
        assert_eq!(editor.cursor_line(), "foo");
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));

        // Bounds check below
        // Should not be able to scroll where all text is not visible.
        // There should always be at least one line visible.
        editor.scroll_by(zi::Active, Down, 1);
        assert_eq!(editor.view(zi::Active).offset(), (1, 0));
        assert_eq!(editor.cursor_line(), "bar");

        editor.scroll_by(zi::Active, Up, 50);
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (0, 0));
        assert_eq!(editor.cursor_line(), "foo");

        editor.scroll_by(zi::Active, Down, 50);
        assert_eq!(editor.view(zi::Active).offset(), (1, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (1, 0));
        assert_eq!(editor.cursor_line(), "bar");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn scroll_overflow() {
    let cx = new("a\n".repeat(20)).await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.move_cursor(zi::Active, Down, 1);
        assert_eq!(editor.cursor(zi::Active), (1, 0));

        editor.scroll_by(zi::Active, Down, usize::MAX);
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
        editor.scroll_by(zi::Active, Down, 20);

        // Ensure the cursor is still following the scroll
        assert_eq!(editor.view(zi::Active).offset(), (4, 0));
        assert_eq!(editor.view(zi::Active).cursor(), (4, 0));
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn scroll_horizontal_wide_chars() {
    let cx = new("日本xy").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        let (x0, _) = editor.cursor_viewport_coords();

        editor.scroll_by(zi::Active, Right, 2);
        assert_eq!(editor.view(zi::Active).offset(), (0, 2), "offset is in cells");
        assert_eq!(editor.cursor(zi::Active), (0, 3), "本 starts at cell 2");
        assert_eq!(editor.cursor_viewport_coords().0, x0);

        editor.scroll_by(zi::Active, Right, 1);
        assert_eq!(
            editor.cursor(zi::Active),
            (0, 6),
            "cell 3 is inside 本, so the cursor goes to x"
        );
        assert_eq!(editor.cursor_viewport_coords().0, x0 + 1);

        editor.scroll_by(zi::Active, Left, 3);
        assert_eq!(editor.view(zi::Active).offset(), (0, 0));
        assert_eq!(editor.cursor(zi::Active), (0, 3));
    })
    .await;
    cx.cleanup().await;
}
