use std::env;

use crate::new;

#[tokio::test]
async fn test_open() -> zi::Result<()> {
    let cx = new("").await;
    let existing_path = tempfile::NamedTempFile::new()?.into_temp_path();
    cx.with({
        let existing_path = existing_path.to_path_buf().to_owned();
        move |editor| editor.open_active(existing_path)
    })
    .await?
    .await?;

    let Err(_) = cx.open(env::temp_dir(), zi::OpenFlags::empty()).await else {
        panic!("should not be allowed to open directories")
    };

    let non_existing_path = env::temp_dir().join("zi-non-existing-file");
    assert!(!non_existing_path.exists());

    let Err(_) = cx.open(&non_existing_path, zi::OpenFlags::READONLY).await else {
        panic!("should not be allowed to open non-existing files in readonly mode")
    };

    let buf = cx
        .open(&non_existing_path, zi::OpenFlags::ACTIVE)
        .await
        .expect("should be allowed to open new files");

    cx.with(move |editor| assert_eq!(editor.buffer(buf).text().to_string(), "")).await;
    assert!(!non_existing_path.exists(), "should not create new file until saved");

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_open_replace_readonly_with_writable() -> zi::Result<()> {
    let cx = new("").await;
    let path = tempfile::NamedTempFile::new()?.into_temp_path();
    let readonly = cx.open(&path, zi::OpenFlags::READONLY).await?;
    let buf = cx.open(&path, zi::OpenFlags::empty()).await?;

    cx.with(move |editor| {
        // Should be able to replace an open readonly buffer with a writable one
        assert_eq!(buf, readonly, "the buffer should retain it's identity");

        editor.insert_char(zi::Active, 'a');
        assert!(editor.get_error().is_none());
    })
    .await;
    cx.cleanup().await;
    Ok(())
}
