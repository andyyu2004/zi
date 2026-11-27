use super::*;

// Utility type that can be referenced from within `Fn` closures.
pub struct ExpectedSequence<T> {
    xs: Vec<T>,
    idx: AtomicUsize,
}

impl<T> ExpectedSequence<T> {
    pub fn new(xs: impl Into<Vec<T>>) -> Self {
        Self { xs: xs.into(), idx: AtomicUsize::new(0) }
    }

    #[track_caller]
    pub fn assert_eq(&self, expected: &T)
    where
        T: fmt::Debug + PartialEq,
    {
        let idx = self.idx.fetch_add(1, atomic::Ordering::Relaxed);
        let actual = self.xs.get(idx).unwrap_or_else(|| {
            panic!("ExpectedSequence: out of bounds access at index {idx}, expected {expected:?}")
        });

        assert_eq!(actual, expected, "ExpectedSequence: mismatch at index {idx}");
    }
}

impl<T> Drop for ExpectedSequence<T> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let idx = self.idx.load(atomic::Ordering::Relaxed);
            assert_eq!(
                idx,
                self.xs.len(),
                "ExpectedSequence: not all expected events were received"
            );
        }
    }
}

macro_rules! lsp_pos {
    ($line:literal:$character:literal) => {
        lsp_types::Position { line: $line, character: $character }
    };
}

macro_rules! lsp_range {
    ($start_line:literal:$start_character:literal..$end_line:literal:$end_character:literal) => {
        lsp_types::Range {
            start: lsp_pos!($start_line:$start_character),
            end: lsp_pos!($end_line:$end_character),
        }
    };
}

macro_rules! lsp_change_event {
    ($start_line:literal:$start_character:literal..$end_line:literal:$end_character:literal =>$text:expr) => {
        lsp_types::TextDocumentContentChangeEvent {
            range: Some(lsp_range!($start_line:$start_character..$end_line:$end_character)),
            text: $text.to_string(),
            range_length: None,
        }
    };
    ($text:expr) => {
        lsp_types::TextDocumentContentChangeEvent {
            range: None,
            text: $text.to_string(),
            range_length: None,
        }
    };
}

pub(crate) use lsp_change_event;
pub(crate) use lsp_pos;
pub(crate) use lsp_range;

pub(crate) fn on_initialized(
    service_id: zi::LanguageServiceId,
) -> tokio::sync::mpsc::UnboundedReceiver<()> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    zi::event::subscribe_with::<zi::event::DidInitializeLanguageService>(move |_editor, event| {
        if event.service_id == service_id {
            let _ = tx.send(());
        }
        zi::event::HandlerResult::Continue
    });
    rx
}

/// Configure gopls with `args` for Go, open the test module's `main.go` and wait for gopls to initialize.
pub(crate) async fn start_gopls(cx: &TestContext, args: &[&str]) -> zi::Result<zi::BufferId> {
    let service_id = zi::LanguageServiceId::from("gopls");
    let args = args.iter().map(Into::into).collect::<Vec<_>>();
    cx.with(move |editor| {
        editor
            .language_config_mut()
            .add_language_service(service_id, zi_lsp::LanguageServerConfig::new("gopls", args))
            .add_language(zi::filetype!(go), zi::LanguageConfig::new([service_id]));
    })
    .await;

    let mut initialized = on_initialized(service_id);
    let buf = cx
        .with(move |editor| {
            editor.open(
                "tests/zi-lsp/testdirs/lsp-test/main.go",
                zi::OpenFlags::SPAWN_LANGUAGE_SERVICES,
            )
        })
        .await?
        .await?;
    if tokio::time::timeout(std::time::Duration::from_secs(40), initialized.recv()).await.is_err() {
        let err = cx.with(|editor| editor.get_error().map(str::to_owned)).await;
        panic!("gopls did not initialize: {err:?}");
    }
    Ok(buf)
}

pub(crate) trait TestContextExt {
    async fn setup_lang_server<St: Send + Sync + Clone + 'static>(
        &self,
        ft: zi::FileType,
        server_id: impl Into<zi::LanguageServiceId>,
        st: St,
        f: impl FnOnce(FakeLanguageServerBuilder<St>) -> FakeLanguageServerBuilder<St>,
    );
}

impl TestContextExt for TestContext {
    async fn setup_lang_server<St: Send + Sync + Clone + 'static>(
        &self,
        ft: zi::FileType,
        server_id: impl Into<zi::LanguageServiceId>,
        st: St,
        f: impl FnOnce(FakeLanguageServerBuilder<St>) -> FakeLanguageServerBuilder<St>,
    ) {
        let server_id = server_id.into();
        // Setup a few default handlers.
        let server = f(FakeLanguageServer::builder()
            .request::<request::Initialize, _>(|_, _| async {
                Ok(lsp_types::InitializeResult::default())
            })
            .notification::<notification::Initialized>(|_st, _params| Ok(()))
            .notification::<notification::DidOpenTextDocument>(|_st, _params| Ok(()))
            .notification::<notification::DidChangeTextDocument>(|_st, _params| Ok(())));

        self.with(move |editor| {
            editor
                .language_config_mut()
                .add_language(ft, zi::LanguageConfig::new([server_id]))
                .add_language_service(server_id, server.finish(st));
        })
        .await;
    }
}
