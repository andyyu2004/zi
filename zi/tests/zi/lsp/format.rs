use super::*;

#[tokio::test]
async fn lsp_format() {
    let cx = new_cx(zi::Size::new(10, 5), "").await;

    cx.with(|editor| {
        // editor.language_config_mut().languages.insert();
    })
    .await;
}
