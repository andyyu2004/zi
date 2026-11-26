use crate::new;

#[tokio::test]
async fn visual_charwise_yank() {
    let cx = new("hello").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("vllly").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "hell");
        assert_eq!(reg.kind, zi::RegisterKind::Charwise);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_charwise_yank_backward() {
    let cx = new("hello").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 3));
        editor.input("vhhy").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "ell");
        assert_eq!(reg.kind, zi::RegisterKind::Charwise);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_line_yank() {
    let cx = new("first\nsecond\nthird").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("Vjy").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "first\nsecond\n");
        assert_eq!(reg.kind, zi::RegisterKind::Linewise);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_block_yank() {
    let cx = new("abc\ndef\nghi").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("<C-v>jjly").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "ab\nde\ngh");
        assert_eq!(reg.kind, zi::RegisterKind::Charwise);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_mode_escape() {
    let cx = new("hello").await;
    cx.with(|editor| {
        editor.input("v").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Visual);
        editor.input("<ESC>").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);

        editor.input("V").unwrap();
        assert_eq!(editor.mode(), zi::Mode::VisualLine);
        editor.input("<ESC>").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);

        editor.input("<C-v>").unwrap();
        assert_eq!(editor.mode(), zi::Mode::VisualBlock);
        editor.input("<ESC>").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_mode_switch() {
    let cx = new("hello").await;
    cx.with(|editor| {
        editor.input("v").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Visual);
        editor.input("V").unwrap();
        assert_eq!(editor.mode(), zi::Mode::VisualLine);
        editor.input("<C-v>").unwrap();
        assert_eq!(editor.mode(), zi::Mode::VisualBlock);
        editor.input("v").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Visual);
        editor.input("<ESC>").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_charwise_multiline_yank() {
    let cx = new("abc\ndef").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("vljy").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "abc\nde");
        assert_eq!(reg.kind, zi::RegisterKind::Charwise);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_block_yank_with_empty_lines() {
    let cx = new("abc\n\nghi").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("<C-v>jjly").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "ab\n\ngh");
        assert_eq!(reg.kind, zi::RegisterKind::Charwise);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_block_yank_with_short_lines() {
    let cx = new("abcde\nab\nabcde").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 1));
        editor.input("<C-v>jjllly").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "bcde\nb\nbcde");
        assert_eq!(reg.kind, zi::RegisterKind::Charwise);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_line_single_line_yank() {
    let cx = new("only").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("Vy").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "only\n");
        assert_eq!(reg.kind, zi::RegisterKind::Linewise);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_charwise_delete() {
    let cx = new("hello").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("vlld").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        assert_eq!(editor.text(zi::Active), "lo\n");
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "hel");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_charwise_delete_x() {
    let cx = new("hello").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("vllx").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        assert_eq!(editor.text(zi::Active), "lo\n");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_line_delete() {
    let cx = new("first\nsecond\nthird").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("Vjd").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        assert_eq!(editor.text(zi::Active), "third\n");
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "first\nsecond\n");
        assert_eq!(reg.kind, zi::RegisterKind::Linewise);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_block_delete() {
    let cx = new("abc\ndef\nghi").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("<C-v>jjld").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);
        assert_eq!(editor.text(zi::Active), "c\nf\ni\n");
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "ab\nde\ngh");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_charwise_change() {
    let cx = new("hello").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("vllc").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Insert);
        assert_eq!(editor.text(zi::Active), "lo\n");
        let reg = editor.register('"').unwrap();
        assert_eq!(reg.content, "hel");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn visual_line_change() {
    let cx = new("first\nsecond\nthird").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("Vjc").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Insert);
        assert_eq!(editor.text(zi::Active), "\nthird\n");
    })
    .await;
    cx.cleanup().await;
}
