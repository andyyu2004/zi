use zi::Active;

use crate::new;

#[tokio::test]
async fn closing_a_picker_removes_its_views_and_buffers() {
    let cx = new("").await;
    cx.with(|editor| {
        let original_view = editor.view(Active).id();
        let original_buf = editor.buffer(Active).id();
        let buffers_before = editor.buffers().count();

        for _ in 0..3 {
            editor.open_file_picker(".");
            let picker: Vec<_> = editor
                .views()
                .filter(|v| v.id() != original_view)
                .map(|v| (v.id(), v.buffer()))
                .collect();
            assert_eq!(picker.len(), 3, "picker, results and preview views");

            editor.close_view(Active);

            for (view, buf) in picker {
                assert!(!editor.has_view(view), "picker view {view:?} should be removed");
                assert!(!editor.has_buffer(buf), "picker buffer {buf:?} should be removed");
            }

            assert_eq!(editor.buffers().count(), buffers_before);
            assert_eq!(editor.buffer(Active).id(), original_buf);
        }
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn closing_a_view_keeps_a_file_buffer() {
    let cx = new("abc").await;
    cx.with(|editor| {
        let buf = editor.buffer(Active).id();
        let split = editor.split(Active, zi::Direction::Right, zi::Constraint::Fill(1));
        editor.close_view(split);

        assert!(!editor.has_view(split));
        assert!(editor.has_buffer(buf));
        assert_eq!(editor.buffer(Active).id(), buf);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn switching_away_from_a_transient_buffer_removes_it() {
    let cx = new("abc").await;
    cx.with(|editor| {
        let file_buf = editor.buffer(Active).id();
        editor.inspect(Active);
        let inspector_view =
            editor.views().find(|v| v.buffer() != file_buf).map(|v| v.id()).unwrap();
        let inspector_buf = editor.view(inspector_view).buffer();

        editor.set_buffer(inspector_view, file_buf);
        assert!(!editor.has_buffer(inspector_buf));
        assert!(editor.has_view(inspector_view));
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn jump_back_skips_a_removed_buffer() {
    let cx = new("abc").await;
    cx.with(|editor| {
        let file_buf = editor.buffer(Active).id();
        editor.inspect(Active);
        let inspector_view =
            editor.views().find(|v| v.buffer() != file_buf).map(|v| v.id()).unwrap();
        let inspector_buf = editor.view(inspector_view).buffer();
        editor.focus(inspector_view);

        editor.jump_to(zi::Location::new(file_buf, (0, 1)));
        assert!(!editor.has_buffer(inspector_buf));

        assert_eq!(editor.jump_back(Active), None);
        assert_eq!(editor.buffer(Active).id(), file_buf);
    })
    .await;
    cx.cleanup().await;
}
