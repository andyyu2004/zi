use std::env;

use super::new;

#[tokio::test]
async fn test_open() -> zi::Result<()> {
    let mut editor = new("").await;
    let existing_path = tempfile::NamedTempFile::new()?.into_temp_path();
    editor.open_active(&existing_path)?.await?;

    let Err(_) = editor.open(env::temp_dir(), zi::OpenFlags::empty()) else {
        panic!("should not be allowed to open directories")
    };

    let non_existing_path = env::temp_dir().join("zi-non-existing-file");
    assert!(!non_existing_path.exists());

    let Err(_) = editor.open(&non_existing_path, zi::OpenFlags::READONLY) else {
        panic!("should not be allowed to open non-existing files in readonly mode")
    };

    let buf =
        editor.open_active(&non_existing_path)?.await.expect("should be allowed to open new files");
    assert_eq!(editor.buffer(buf).text().to_string(), "");

    assert!(!non_existing_path.exists(), "should not create new file until saved");
    Ok(())
}

#[tokio::test]
async fn test_open_replace_readonly_with_writable() -> zi::Result<()> {
    let mut editor = new("").await;
    let path = tempfile::NamedTempFile::new()?.into_temp_path();

    let readonly = editor.open(&path, zi::OpenFlags::READONLY)?.await?;

    // Should be able to replace an open readonly buffer with a writable one
    let buf = editor.open(&path, zi::OpenFlags::empty())?.await?;
    assert_eq!(buf, readonly, "the buffer should retain it's identity");

    editor.insert_char(zi::Active, 'a');
    assert!(editor.get_error().is_none());

    Ok(())
}
