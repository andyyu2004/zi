use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use tui::{CrosstermBackend, Terminal};
use zi::Editor;

#[tokio::test]
async fn it_works() -> anyhow::Result<()> {
    snapshot("empty", |_editor| Ok(())).await?;

    snapshot("numbers", |editor| {
        editor.open("tests/integration/testdata/numbers.txt")?;
        Ok(())
    })
    .await?;

    Ok(())
}

#[tokio::test]
async fn syntax_highlight() -> anyhow::Result<()> {
    snapshot("go", |editor| {
        editor.open("tests/integration/testdata/main.go")?;
        Ok(())
    })
    .await?;

    snapshot("rust", |editor| {
        editor.open("tests/integration/testdata/main.rs")?;
        Ok(())
    })
    .await?;

    Ok(())
}

#[tokio::test]
async fn scroll() -> anyhow::Result<()> {
    snapshot("scroll text", |editor| {
        editor.open("tests/integration/testdata/numbers.txt")?;
        editor.scroll_active_view(zi::Direction::Down, 50);
        Ok(())
    })
    .await?;

    // The above doesn't test highlighting works with scroll
    snapshot("scroll rust minimal", |editor| {
        editor.open("tests/integration/testdata/minimal.rs")?;
        editor.scroll_active_view(zi::Direction::Down, 1);
        Ok(())
    })
    .await?;

    snapshot("scroll go", |editor| {
        editor.open("tests/integration/testdata/main.go")?;
        editor.scroll_active_view(zi::Direction::Down, 9);
        Ok(())
    })
    .await?;

    Ok(())
}

async fn snapshot(
    name: &'static str,
    f: impl FnOnce(&mut Editor) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    static NAMES: OnceLock<Mutex<HashSet<&OsStr>>> = OnceLock::new();
    let mut names = NAMES.get_or_init(Default::default).lock().unwrap();
    assert!(names.insert(name.as_ref()), "duplicate snapshot name: {name}",);

    let mut bytes = vec![];

    {
        let mut term = Terminal::new(CrosstermBackend::new(&mut bytes))?;
        let (mut editor, _tasks, _) = Editor::new(term.size()?.into());
        f(&mut editor)?;
        term.draw(|f| editor.render(f))?;
    }

    let name = name.replace(|c: char| c.is_whitespace(), "-");
    let dir = PathBuf::from("tests/integration/snapshots");
    let path = dir.join(format!("{name}.ansi"));

    let mut expected = vec![];
    if path.exists() {
        std::fs::File::open(&path)?.read_to_end(&mut expected)?;
    } else {
        std::fs::write(path, &bytes)?;
        return Ok(());
    }

    if std::env::var("UPDATE_EXPECT").is_ok() {
        std::fs::write(path, &bytes)?;
    } else {
        assert_eq!(bytes, expected);
    }

    // TODO delete all the snapshots files that are not used
    // This needs to run as cleanup, not per test
    // std::fs::read_dir(dir)?
    //     .filter_map(|entry| entry.ok())
    //     .filter(|entry| {
    //         let mut name = PathBuf::from(entry.file_name());
    //         name.set_extension("");
    //         !names.contains(name.as_os_str())
    //     })
    //     .try_for_each(|entry| std::fs::remove_file(entry.path()))?;

    Ok(())
}
