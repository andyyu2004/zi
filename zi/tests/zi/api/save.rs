use crate::new_cx;

#[tokio::test]
async fn save() -> zi::Result<()> {
    let cx = new_cx("").await;

    let path = cx.tempfile("abc").unwrap();
    let buf = cx.with(|editor| editor.open(path, zi::OpenFlags::ACTIVE)).await?.await?;

    cx.with(move |editor| {
        assert!(!editor[buf].flags().contains(zi::BufferFlags::DIRTY));
        editor.edit(buf, &zi::Deltas::insert_at(3, "def".to_string()));
        assert!(editor[buf].flags().contains(zi::BufferFlags::DIRTY));
    })
    .await;

    cx.with(move |editor| editor.save(buf, zi::SaveFlags::empty())).await.await?;
    cx.with(move |editor| {
        assert!(!editor[buf].flags().contains(zi::BufferFlags::DIRTY));
        assert_eq!(editor[buf].text().to_string(), "abcdef\n");
    })
    .await;

    cx.cleanup().await;

    Ok(())
}
