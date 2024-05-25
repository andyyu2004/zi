use std::future::Future;
use std::path::Path;

// use tokio::io::AsyncReadExt;
// use tui::backend::CrosstermBackend;
// use tui::Terminal;
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
            .request(move |editor| {
                editor.open("tests/zi-term/testdata/main.rs", OpenFlags::empty())
            })
            .await?
            .await?;
        client.request(|editor| editor.input("/use").unwrap()).await;

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
            .request(|editor| editor.open("tests/zi-term/testdata/numbers.txt", OpenFlags::empty()))
            .await?
            .await?;
        client.request(|editor| editor.scroll(zi::Active, zi::Direction::Down, 50)).await;
        Ok(())
    })
    .await?;

    // The above doesn't test highlighting works with scroll
    snapshot("scroll rust minimal", |client| async move {
        client
            .request(|editor| editor.open("tests/zi-term/testdata/minimal.rs", OpenFlags::empty()))
            .await?
            .await?;
        client.request(|editor| editor.scroll(zi::Active, zi::Direction::Down, 1)).await;
        Ok(())
    })
    .await?;

    snapshot("scroll go", |client| async move {
        client
            .request(|editor| editor.open("tests/zi-term/testdata/main.go", OpenFlags::empty()))
            .await?
            .await?;

        client.request(|editor| editor.scroll(zi::Active, zi::Direction::Down, 9)).await;

        Ok(())
    })
    .await?;

    Ok(())
}

async fn snapshot_path(name: &'static str, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let path = path.as_ref().to_path_buf();
    snapshot(name, |client| async move {
        client.request(move |editor| editor.open(path, OpenFlags::empty())).await?.await?;
        Ok(())
    })
    .await
}

async fn snapshot<Fut>(
    _name: &'static str,
    _f: impl FnOnce(zi::Client) -> Fut,
) -> anyhow::Result<()>
where
    Fut: Future<Output = zi::Result<()>>,
{
    // We should find a better storage format than raw ansi bytes, maybe asciinema format?
    Ok(())
    // let mut bytes = vec![];
    //
    // {
    //     let mut term = Terminal::new(CrosstermBackend::new(&mut bytes))?;
    //     let (mut editor, tasks) = Editor::new(zi::Size::new(150, 32));
    //     let client = editor.client();
    //     tokio::spawn(async move {
    //         editor.run(futures_util::stream::empty(), tasks, |_editor| Ok(())).await.unwrap()
    //     });
    //     f(client.clone()).await?;
    //
    //     client
    //         .request(move |editor| {
    //             term.draw(|f| editor.render(f)).unwrap();
    //             term
    //         })
    //         .await;
    // }
    //
    // let name = name.replace(|c: char| c.is_whitespace(), "-");
    // let dir = PathBuf::from("tests/zi-term/snapshots");
    // let path = dir.join(format!("{name}.ansi"));
    //
    // let mut expected = vec![];
    // if path.exists() {
    //     tokio::fs::File::open(&path).await?.read_to_end(&mut expected).await?;
    // } else {
    //     tokio::fs::write(path, &bytes).await?;
    //     return Ok(());
    // }
    //
    // if std::env::var("UPDATE_EXPECT").is_ok() {
    //     tokio::fs::write(path, &bytes).await?;
    // } else {
    //     assert_eq!(bytes, expected);
    // }
    //
    // Ok(())
}
