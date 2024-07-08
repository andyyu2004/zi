use std::env;

use crate::new;

#[tokio::test]
async fn test_open() -> zi::Result<()> {
    let cx = new("").await;
    let existing_path = tempfile::NamedTempFile::new()?.into_temp_path();
    cx.open(&existing_path, zi::OpenFlags::empty()).await?;

    let Err(_) = cx.open(env::temp_dir(), zi::OpenFlags::empty()).await else {
        panic!("should not be allowed to open directories")
    };

    let non_existing_path = env::temp_dir().join("zi-non-existing-file");
    assert!(!non_existing_path.exists());

    let Err(_) = cx.open(&non_existing_path, zi::OpenFlags::READONLY).await else {
        panic!("should not be allowed to open non-existing files in readonly mode")
    };

    let buf = cx
        .open(&non_existing_path, zi::OpenFlags::empty())
        .await
        .expect("should be allowed to open new files");

    cx.with(move |editor| assert_eq!(editor.text(buf), "")).await;
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

        editor.insert_char(zi::Active, 'a').unwrap();
        assert!(editor.get_error().is_none());
    })
    .await;
    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_open_focus() -> zi::Result<()> {
    let cx = new("").await;
    let a = cx.tempfile("a")?;
    let b = cx.tempfile("b")?;

    cx.open(&a, zi::OpenFlags::empty()).await.unwrap();
    assert_eq!(&cx.with(|editor| editor.buffer(zi::Active).path().unwrap()).await, &a);

    cx.open(&b, zi::OpenFlags::empty()).await.unwrap();
    assert_eq!(&cx.with(|editor| editor.buffer(zi::Active).path().unwrap()).await, &b);

    // Should focus `a` again even if it's already open
    cx.open(&a, zi::OpenFlags::empty()).await.unwrap();
    assert_eq!(&cx.with(|editor| editor.buffer(zi::Active).path().unwrap()).await, &a);

    cx.cleanup().await;
    Ok(())
}
