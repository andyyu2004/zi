#![feature(closure_track_caller, stmt_expr_attributes, try_blocks)]

mod api;
mod keymap;
mod perf;
mod render;

use std::future::IntoFuture;
use std::io;
use std::path::PathBuf;

use anyhow::Result;
use expect_test::{expect, Expect};
use futures_util::future::BoxFuture;
use stdx::bomb::DropBomb;
use tui::backend::{Backend as _, TestBackend};
use tui::Terminal;
use unicode_width::UnicodeWidthStr;

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

// FIXME deduplicate from zi-test
pub struct TestContext {
    size: zi::Size,
    client: zi::Client,
    handle: Option<tokio::task::JoinHandle<()>>,
    bomb: DropBomb,
}

impl TestContext {
    /// Convenience wrapper around `Editor::open`
    pub async fn open(
        &self,
        path: impl Into<PathBuf>,
        flags: zi::OpenFlags,
    ) -> Result<zi::BufferId> {
        let path = path.into();
        self.with(move |editor| editor.open(path, flags)).await?.await
    }

    pub async fn with<R>(&self, f: impl FnOnce(&mut zi::Editor) -> R + Send + 'static) -> R
    where
        R: Send + 'static,
    {
        self.client.with(f).await
    }

    pub async fn snapshot(&self, expect: Expect) {
        let size = self.size;
        // Have to render twice to get updated output for certain events.
        self.render().await;

        self.client
            .with(move |editor| {
                let mut term = Terminal::new(TestBackend::new(size.width, size.height)).unwrap();
                term.draw(|frame| editor.render(frame)).unwrap();
                expect.assert_eq(&render(term.backend_mut()))
            })
            .await;
    }

    pub async fn render(&self) {
        let size = self.size;
        self.client
            .with(move |editor| editor.render(&mut tui::TestFrame::new(size.width, size.height)))
            .await;
    }

    pub fn tempdir(&self) -> io::Result<PathBuf> {
        tempfile::tempdir().map(|dir| dir.into_path())
    }

    pub fn tempfile(&self, content: &str) -> io::Result<PathBuf> {
        let file = tempfile::NamedTempFile::new()?;
        std::fs::write(file.path(), content)?;
        let (_file, path) = file.keep()?;
        assert!(path.exists());
        debug_assert_eq!(std::fs::read_to_string(&path)?, content);
        Ok(path)
    }

    /// Open a temporary file with the given content.
    pub async fn open_tmp(&self, content: &str, flags: zi::OpenFlags) -> zi::Result<zi::BufferId> {
        let path = self.tempfile(content)?;
        Ok(self.open(&path, flags).await?)
    }

    async fn cleanup(mut self) {
        self.bomb.defuse();
        let handle = self.handle.take().unwrap();
        handle.abort();

        if let Err(err) = handle.await {
            if err.is_panic() {
                panic!("editor panicked: {err}");
            }
        }
    }
}

pub fn new(scratch_content: impl Into<String>) -> TestContextBuilder {
    TestContextBuilder { size: zi::Size::new(80, 10), scratch_content: scratch_content.into() }
}

pub struct TestContextBuilder {
    size: zi::Size,
    scratch_content: String,
}

impl TestContextBuilder {
    pub fn with_size(mut self, size: impl Into<zi::Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl IntoFuture for TestContextBuilder {
    type Output = TestContext;
    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let (mut editor, tasks) = zi::Editor::new(zi::DummyBackend, self.size);
            editor.set_mode(zi::Mode::Insert);
            editor.edit(zi::Active, &zi::Deltas::insert_at(0, &self.scratch_content)).unwrap();
            editor.set_cursor(zi::Active, self.scratch_content.len());
            editor.set_mode(zi::Mode::Normal);

            let client = editor.client();
            let handle = tokio::spawn(async move {
                editor.run(futures_util::stream::empty(), tasks, |_editor| Ok(())).await.unwrap()
            });

            TestContext {
                client,
                size: self.size,
                handle: Some(handle),
                bomb: DropBomb::new("call `cleanup().await`"),
            }
        })
    }
}

/// Copied from ratatui's `buffer_view`, but draws the cursor too.
fn render(backend: &mut TestBackend) -> String {
    use std::fmt::Write as _;

    let cursor = backend.get_cursor().unwrap();
    let buffer = backend.buffer();
    let mut view = String::with_capacity(buffer.content.len() + buffer.area.height as usize * 3);
    for (y, cells) in buffer.content.chunks(buffer.area.width as usize).enumerate() {
        let mut overwritten = vec![];
        let mut skip: usize = 0;

        view.push('"');
        for (x, c) in cells.iter().enumerate() {
            if (x as u16, y as u16) == cursor {
                view.push('|');
                continue;
            }

            if skip == 0 {
                view.push_str(c.symbol());
            } else {
                overwritten.push((x, c.symbol()));
            }
            skip = std::cmp::max(skip, c.symbol().width()).saturating_sub(1);
        }
        view.push('"');
        if !overwritten.is_empty() {
            write!(&mut view, " Hidden by multi-width symbols: {overwritten:?}").unwrap();
        }
        view.push('\n');
    }
    view
}
