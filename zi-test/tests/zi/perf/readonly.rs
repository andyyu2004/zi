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
    use std::io::Write as _;

    assert!(mbs <= 2048, "don't go too crazy :)");

    let path = (|| -> io::Result<_> {
        let path = std::env::temp_dir().join(format!("zi-test-file-{mbs}mb-lines"));
        if path.exists() {
            return Ok(path);
        }

        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut chunk = Vec::with_capacity(1024 * 1024 + 77);
        while chunk.len() < 1024 * 1024 {
            for _ in 0..76 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                chunk.push(CHARS[(seed % 64) as usize]);
            }
            chunk.push(b'\n');
        }

        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        let mut file = io::BufWriter::new(std::fs::File::create(&tmp)?);
        for _ in 0..=mbs {
            file.write_all(&chunk)?;
        }
        file.flush()?;
        drop(file);
        std::fs::rename(&tmp, &path)?;

        assert!(path.exists());
        assert!(path.metadata()?.size() > 1024 * 1024 * mbs as u64, "file too small");

        Ok(path)
    })();
    path.unwrap()
}
