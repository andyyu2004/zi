use std::fs::File;
use std::future::Future;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use asciicast::Asciicast;
use tui::backend::CrosstermBackend;
use tui::Terminal;
use zi::OpenFlags;

#[tokio::test]
async fn it_works() -> zi::Result<()> {
    snapshot("empty", |_| async { Ok(()) }).await?;

    snapshot_path("numbers", "tests/zi-term/testdata/numbers.txt").await?;
    Ok(())
}

#[tokio::test]
async fn buffer_search() -> zi::Result<()> {
    snapshot("buffer search", |client| async move {
        client
            .with(move |editor| editor.open("tests/zi-term/testdata/main.rs", OpenFlags::empty()))
            .await?
            .await?;
        client.with(|editor| editor.input("/use").unwrap()).await;

        Ok(())
    })
    .await
}

#[tokio::test]
async fn syntax_highlight() -> anyhow::Result<()> {
    snapshot_path("go", "tests/zi-term/testdata/main.go").await?;
    snapshot_path("rust", "tests/zi-term/testdata/main.rs").await?;
    snapshot_path("multiline highlight", "tests/zi-term/testdata/multiline-highlight.rs").await?;
    snapshot_path("multiline highlight 2", "tests/zi-term/testdata/multiline-highlight-2.rs")
        .await?;

    Ok(())
}

#[tokio::test]
async fn scroll() -> anyhow::Result<()> {
    snapshot("scroll text", |client| async move {
        client
            .with(|editor| editor.open("tests/zi-term/testdata/numbers.txt", OpenFlags::empty()))
            .await?
            .await?;
        client.with(|editor| editor.scroll(zi::Active, zi::Direction::Down, 50)).await;
        Ok(())
    })
    .await?;

    // The above doesn't test highlighting works with scroll
    snapshot("scroll rust minimal", |client| async move {
        client
            .with(|editor| editor.open("tests/zi-term/testdata/minimal.rs", OpenFlags::empty()))
            .await?
            .await?;
        client.with(|editor| editor.scroll(zi::Active, zi::Direction::Down, 1)).await;
        Ok(())
    })
    .await?;

    snapshot("scroll go", |client| async move {
        client
            .with(|editor| editor.open("tests/zi-term/testdata/main.go", OpenFlags::empty()))
            .await?
            .await?;

        client.with(|editor| editor.scroll(zi::Active, zi::Direction::Down, 9)).await;

        Ok(())
    })
    .await?;

    Ok(())
}

async fn snapshot_path(name: &'static str, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let path = path.as_ref().to_path_buf();
    snapshot(name, |client| async move {
        client.with(move |editor| editor.open(path, OpenFlags::empty())).await?.await?;
        Ok(())
    })
    .await
}

async fn snapshot<Fut>(name: &'static str, f: impl FnOnce(zi::Client) -> Fut) -> anyhow::Result<()>
where
    Fut: Future<Output = zi::Result<()>>,
{
    let (width, height) = (150, 32);
    let (mut editor, tasks) = zi::Editor::new((width, height));

    let client = editor.client();
    tokio::spawn(async move {
        editor.run(futures_util::stream::empty(), tasks, |_editor| Ok(())).await.unwrap()
    });
    f(client.clone()).await?;

    let bytes = client
        .with(move |editor| {
            let mut bytes = vec![];
            let mut term = Terminal::new(CrosstermBackend::new(&mut bytes))?;
            term.draw(|f| editor.render(f)).unwrap();
            drop(term);
            Ok::<_, zi::Error>(bytes)
        })
        .await?;

    let name = name.replace(|c: char| c.is_whitespace(), "-");
    let dir = PathBuf::from("tests/zi-term/asciicasts");

    let cast = Asciicast::new(
        width,
        height,
        [
            asciicast::Event {
                kind: asciicast::EventKind::Output(String::from_utf8(bytes).unwrap()),
                time_us: 0,
            },
            asciicast::Event { kind: asciicast::EventKind::Output(String::from("\n")), time_us: 1 },
        ],
    );

    let path = dir.join(format!("{name}.cast"));

    let mut expected = vec![];
    if path.exists() {
        File::open(&path)?.read_to_end(&mut expected)?;
    } else {
        return Ok(cast.write_to(File::create(&path)?)?);
    }

    if std::env::var("UPDATE_EXPECT").is_ok() {
        cast.write_to(File::create(&path)?)?;
    } else {
        let existing = Asciicast::read_from(BufReader::new(File::open(&path)?))?;
        assert_eq!(existing, cast);
    }

    Ok(())
}
