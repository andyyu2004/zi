//! Tests against a headless neovim instance

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use anyhow::{ensure, Context};
use nvim_rs::error::LoopError;
use tokio::process::{ChildStdin, Command};
use zi::input::KeySequence;

/// A fixture is a list of test cases. See [`TestCase`] for more information.
pub struct Fixture {
    size: zi::Size,
    cases: Box<[TestCase]>,
}

/// A test case is a text buffer and a sequence of key inputs.
/// Text format:
/// ```
/// ==== any text here until new line
/// some text
/// that can span multiple lines
///
/// with empty lines
/// ----
/// input key sequence
///
/// (optional empty line)
///
/// ==== next test case
/// ```
pub struct TestCase {
    text: String,
    inputs: KeySequence,
}

impl TestCase {
    pub fn new(
        text: impl Into<String>,
        inputs: impl TryInto<KeySequence, Error: fmt::Debug>,
    ) -> Self {
        Self {
            text: text.into(),
            inputs: inputs.try_into().expect("could not convert into KeySequence"),
        }
    }
}

pub async fn spawn(width: u16, height: u16) -> Nvim {
    Nvim::spawn(width, height).await.expect("could not spawn neovim")
}

impl Fixture {
    pub fn new(cases: impl IntoIterator<Item = TestCase>) -> Self {
        Self { size: zi::Size::new(80, 24), cases: cases.into_iter().collect() }
    }

    pub async fn nvim_vs_zi_with(self, nvim: &Nvim) -> zi::Result<()> {
        let (mut editor, _tasks) = zi::Editor::new(self.size);

        for case in &self.cases[..] {
            nvim.run(&mut editor, case).await?;
        }

        Ok(())
    }

    pub async fn spawn(self) -> zi::Result<Nvim> {
        Nvim::spawn(self.size.width, self.size.height).await
    }

    pub async fn nvim_vs_zi(self) -> zi::Result<()> {
        let size = self.size;
        let nvim = Nvim::spawn(size.width, size.height).await?;
        self.nvim_vs_zi_with(&nvim).await
    }

    pub fn load(path: &Path) -> zi::Result<Self> {
        let mut cases = vec![];

        const TEST_CASE_HEADER: &str = "====";
        const SEP: &str = "----";

        let file = std::fs::read_to_string(path)?;
        let mut sections = file.split(TEST_CASE_HEADER).peekable();
        if let Some(first) = sections.peek() {
            assert!(first.is_empty(), "missing initial test case header ====");
            sections.next().unwrap();
        }

        for section in sections {
            let mut text = String::new();
            let mut lines = section.split_inclusive('\n').filter(|line| !line.starts_with('#'));
            lines.next().expect("expected newline after after ====");

            for line in lines.by_ref().take_while(|line| !line.starts_with(SEP)) {
                text.push_str(line);
            }

            let line = lines.next().expect("expected input key sequence line after ----");
            let inputs = KeySequence::from_str(line.trim()).expect("could not parse key sequence");
            cases.push(TestCase { text, inputs });

            for line in lines {
                assert!(
                    line.trim_end().is_empty(),
                    "unexpected non-empty line after input key sequence before next test case: `{line}`"
                );
            }
        }

        Ok(Self::new(cases))
    }

    pub fn size(&self) -> &zi::Size {
        &self.size
    }
}

pub struct Nvim {
    nvim: nvim_rs::Neovim<nvim_rs::compat::tokio::Compat<ChildStdin>>,
    #[allow(unused)]
    join_handle: tokio::task::JoinHandle<Result<(), Box<LoopError>>>,
    #[allow(unused)]
    child: tokio::process::Child,
}

impl Nvim {
    pub async fn run(&self, editor: &mut zi::Editor, case: &TestCase) -> zi::Result<()> {
        let initial = case.text.trim_end();
        let inputs = &case.inputs;
        let n = editor.active_buffer().text().len_bytes();
        editor.active_buffer_mut().edit(&zi::Delta::new(0..n, initial));
        editor.set_active_cursor((0, 0));

        self.nvim
            .get_current_buf()
            .await?
            .set_lines(0, -1, false, initial.lines().map(ToOwned::to_owned).collect())
            .await?;
        self.nvim.get_current_win().await?.set_cursor((1, 0)).await?;
        self.assert_eq(editor).await.context("did not reset state properly after test case")?;

        for (i, key) in inputs.clone().into_iter().enumerate() {
            self.nvim.feedkeys(&key.to_string(), "m", true).await?;
            editor.handle_input(key.clone());
            self.assert_eq(editor)
                .await
                .with_context(|| format!("index {i} in key sequence: `{inputs}`, key: `{key}`"))?;
        }

        Ok(())
    }

    // Compare the state of the editor with the state of the nvim instance
    async fn assert_eq(&self, editor: &zi::Editor) -> zi::Result<()> {
        let (vi_lines, (line, col), vi_mode) = tokio::try_join!(
            async {
                let buf = self.nvim.get_current_buf().await?;
                let lines = buf.get_lines(0, -1, false).await?.join("\n");
                Ok::<_, zi::Error>(lines)
            },
            async {
                let vi_win = self.nvim.get_current_win().await?;
                let (line, col) = vi_win.get_cursor().await?;
                let line = line.checked_sub(1).expect("1-indexed lines");
                Ok((line, col))
            },
            async {
                Ok(self
                    .nvim
                    .get_mode()
                    .await?
                    .into_iter()
                    .find_map(|(key, value)| {
                        if key.as_str() == Some("mode") {
                            Some(value.as_str().unwrap().to_owned())
                        } else {
                            None
                        }
                    })
                    .expect("Could not find mode value"))
            }
        )?;

        let zi_buf = editor.active_buffer();
        let zi_lines = zi_buf.text().to_string();
        let zi_cursor = editor.active_cursor();

        let mode = match vi_mode.as_ref() {
            "i" => zi::Mode::Insert,
            "n" => zi::Mode::Normal,
            "v" => zi::Mode::Visual,
            // "V" => zi::Mode::VisualLine,
            // "\x16" => zi::Mode::VisualBlock,
            _ => panic!("unknown mode: {vi_mode}"),
        };

        ensure!(mode == editor.mode());
        ensure!(vi_lines == zi_lines, "{vi_lines:?}\n{zi_lines:?}");
        ensure!(line as usize == zi_cursor.line().idx());
        ensure!(col as usize == zi_cursor.col().idx());
        Ok(())
    }
}

const DIR: &str = env!("CARGO_MANIFEST_DIR");

impl Nvim {
    async fn spawn(width: u16, height: u16) -> zi::Result<Nvim> {
        let (nvim, join_handle, child) = nvim_rs::create::tokio::new_child_cmd(
            Command::new("nvim")
                .arg("--embed")
                .arg("--headless") // otherwise nvim will block until a ui is attached
                .args(["--cmd", &format!("set rtp+={DIR}/runtime/vim-wordmotion")]) // must use --cmd not -c or + as this has to run early
                .args(["-u", &format!("{DIR}/init.vim")])
                .arg("-n") // disable swap
                .arg("-m"), // disable writing files to disk to avoid potential mayhem
            Handler,
        )
        .await?;

        let buf = nvim.create_buf(false, true).await?;
        nvim.set_current_buf(&buf).await?;
        let win = nvim.get_current_win().await?;
        win.set_width(width as i64).await?;
        win.set_height(height as i64).await?;

        Ok(Nvim { nvim, join_handle, child })
    }
}

#[derive(Clone)]
struct Handler;

#[async_trait::async_trait]
impl nvim_rs::Handler for Handler {
    type Writer = nvim_rs::compat::tokio::Compat<ChildStdin>;
}
