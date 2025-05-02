use futures_core::future::BoxFuture;
use zi::lstypes;

use crate::new;

// TODO: We need to open a non-scratch buffer as we ignore any buffers that are not file-backed currently

struct Completions;

fn res() -> lstypes::CompletionResponse {
    lstypes::CompletionResponse {
        items: vec![
            zi::CompletionItem { label: "foo".to_string(), ..Default::default() },
            zi::CompletionItem { label: "bar".to_string(), ..Default::default() },
            zi::CompletionItem { label: "bazz".to_string(), ..Default::default() },
        ],
    }
}

impl zi::CompletionProvider for Completions {
    fn completions(
        &self,
        _editor: &mut zi::Editor,
        _params: lstypes::CompletionParams,
    ) -> BoxFuture<'static, zi::Result<lstypes::CompletionResponse>> {
        Box::pin(async move { Ok(res()) })
    }
}

#[tokio::test]
async fn request_completion() -> zi::Result<()> {
    let cx = new("").await;
    cx.open_tmp("", zi::OpenFlags::empty()).await?;
    let completions = cx
        .with(|editor| {
            editor.register_completion_provider(Completions);
            editor.request_completions(zi::Active)
        })
        .await
        .await?;

    assert_eq!(completions, res().items);

    cx.cleanup().await;
    Ok(())
}

fn completions(editor: &mut zi::Editor) -> Vec<zi::CompletionItem> {
    editor.completions().unwrap().cloned().collect()
}

#[tokio::test]
async fn trigger_completions() -> zi::Result<()> {
    let cx = new("").await;
    cx.open_tmp("", zi::OpenFlags::empty()).await?;
    cx.with(|editor| {
        editor.register_completion_provider(Completions);
        editor.set_mode(zi::Mode::Insert);
        editor.trigger_completion(None);
    })
    .await;

    cx.with(|editor| {
        assert_eq!(completions(editor), res().items);
        editor.insert_char(zi::Active, 'f').unwrap();

        assert_eq!(completions(editor), vec![zi::CompletionItem {
            label: "foo".to_string(),
            ..Default::default()
        }]);

        editor.delete_char(zi::Active).unwrap();
        assert_eq!(completions(editor), res().items);
    })
    .await;

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn tab_completion() -> zi::Result<()> {
    let cx = new("").await;
    cx.open_tmp("", zi::OpenFlags::empty()).await?;
    cx.with(|editor| {
        editor.register_completion_provider(Completions);
        editor.set_mode(zi::Mode::Insert);
        editor.trigger_completion(None);
    })
    .await;

    cx.with(|editor| {
        editor.insert_char(zi::Active, 'b').unwrap();

        assert_eq!(editor.text(zi::Active), "b\n");
        editor.tab().unwrap();
        assert_eq!(editor.text(zi::Active), "bar\n");

        editor.tab().unwrap();
        assert_eq!(editor.text(zi::Active), "bazz\n");

        // No cycling behaviour
        editor.tab().unwrap();
        assert_eq!(editor.text(zi::Active), "bazz\n");

        editor.backtab().unwrap();
        assert_eq!(editor.text(zi::Active), "bar\n");

        editor.backtab().unwrap();
        assert_eq!(editor.text(zi::Active), "b\n");

        editor.backtab().unwrap();
        assert_eq!(editor.text(zi::Active), "b\n");

        editor.tab().unwrap();
        assert_eq!(editor.text(zi::Active), "bar\n");
    })
    .await;

    cx.cleanup().await;
    Ok(())
}
