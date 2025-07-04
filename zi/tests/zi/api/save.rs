use crate::new;

#[tokio::test]
async fn save() -> zi::Result<()> {
    let cx = new("").await;

    let path = cx.tempfile("abc").unwrap();
    let buf = cx.open(&path, zi::OpenFlags::empty()).await?;

    cx.with(move |editor| {
        assert!(!editor[buf].flags().contains(zi::BufferFlags::DIRTY));
        assert!(!editor[buf].flags().contains(zi::BufferFlags::READONLY));
        editor.edit(buf, &zi::Deltas::insert_at(3, "def".to_string())).unwrap();
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
