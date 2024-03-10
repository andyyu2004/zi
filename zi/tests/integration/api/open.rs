use std::env;

use super::new;

#[test]
fn test_open() -> zi::Result<()> {
    let mut editor = new("");
    let existing_path = tempfile::NamedTempFile::new()?.into_temp_path();
    editor.open_active(&existing_path)?;

    editor
        .open(env::temp_dir(), zi::OpenFlags::empty())
        .expect_err("not allowed to open directories");

    let non_existing_path = env::temp_dir().join("zi-non-existing-file");
    assert!(!non_existing_path.exists());

    editor
        .open(&non_existing_path, zi::OpenFlags::READONLY)
        .expect_err("should not be allowed to open non-existing files in readonly mode");

    let buf = editor.open_active(&non_existing_path).expect("should be allowed to open new files");
    assert_eq!(editor.buffer(buf).text().to_string(), "\n");

    assert!(!non_existing_path.exists(), "should not create new file until saved");
    Ok(())
}
