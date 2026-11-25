//! Some performance tests to prevent regressions.
//! Not really a benchmark but more of a thing that should be noticed if tests randomly become slow.

use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::path::PathBuf;

use crate::new;

#[tokio::test]
async fn perf_readonly_large_file() -> zi::Result<()> {
    let path = create_file(2000);
    let cx = new("").await;
    let buf = cx.open(path, zi::OpenFlags::READONLY).await?;
    cx.with(move |editor| {
        // This is basically a test that we don't call `len_chars` or `len_lines` when scrolling and moving around.
        // If we do then this will take seconds, but it should be instant.
        for _ in 0..100 {
            editor.scroll(zi::Active, zi::Direction::Down, 20);
            editor.move_cursor(zi::Active, zi::Direction::Down, 20);
        }
        assert_eq!(editor.buffer(zi::Active).id(), buf);
    })
    .await;
    cx.cleanup().await;

    Ok(())
}

fn create_file(mbs: usize) -> PathBuf {
    use duct::cmd;

    assert!(mbs <= 2048, "don't go too crazy :)");

    let path: io::Result<_> = try {
        let path = std::env::temp_dir().join(format!("zi-test-file-{mbs}mb-lines"));
        if path.exists() {
            return path;
        }

        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        cmd!("dd", "if=/dev/urandom", "ibs=1M", "obs=1M", format!("count={mbs}"))
            .pipe(cmd!("base64"))
            .pipe(cmd!("fold", "-w", "76"))
            .stdout_path(&tmp)
            .run()?;
        std::fs::rename(&tmp, &path)?;

        assert!(path.exists());
        assert!(path.metadata()?.size() > 1024 * 1024 * mbs as u64, "file too small");

        path
    };
    path.unwrap()
}
