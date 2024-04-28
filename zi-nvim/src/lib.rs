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

bitflags::bitflags! {
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CompareFlags: u8 {
        /// Allow all whitespace lines to be considered equal i.e. "a\n  \nb" == "a\n\nb"
        const IGNORE_WHITESPACE_LINES = 0b0001;
    }
}

pub async fn spawn(width: u16, height: u16) -> Nvim {
    Nvim::spawn(width, height).await.expect("could not spawn neovim")
}

impl Fixture {
    pub fn new(cases: impl IntoIterator<Item = TestCase>) -> Self {
        Self { size: zi::Size::new(80, 24), cases: cases.into_iter().collect() }
    }

    pub async fn nvim_vs_zi_with(self, nvim: &Nvim, flags: CompareFlags) -> zi::Result<()> {
        let (mut editor, _tasks) = zi::Editor::new(self.size);

        for case in &self.cases[..] {
            nvim.run(&mut editor, case, flags).await?;
        }

        Ok(())
    }

    pub async fn spawn(self) -> zi::Result<Nvim> {
        Nvim::spawn(self.size.width, self.size.height).await
    }

    pub async fn nvim_vs_zi(self, flags: CompareFlags) -> zi::Result<()> {
        let size = self.size;
        let nvim = Nvim::spawn(size.width, size.height).await?;
        self.nvim_vs_zi_with(&nvim, flags).await
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
            lines.next().expect("expected newline after ====");

            for line in lines.by_ref().take_while(|line| !line.starts_with(SEP)) {
                text.push_str(line);
            }

            let line = lines.next().expect("expected input key sequence line after ----");
            let inputs = KeySequence::from_str(line.trim()).expect("could not parse key sequence");

            assert_eq!(
                text.pop(),
                Some('\n'),
                "there should always be a newline after the text section (we don't want this to be part of the test though but remaining newlines are significant)"
            );
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
    pub async fn run(
        &self,
        editor: &mut zi::Editor,
        case: &TestCase,
        flags: CompareFlags,
    ) -> zi::Result<()> {
        // Only remove the final newline. The rest of the newlines are significant.
        let initial = &case.text;
        let inputs = &case.inputs;
        let n = editor.buffer(zi::Active).text().len_bytes();
        editor.edit(zi::Active, &zi::Delta::new(0..n, initial));
        editor.set_cursor(zi::Active, (0, 0));
        editor.set_mode(zi::Mode::Normal);
        editor.clear_undo();

        self.nvim
            .get_current_buf()
            .await?
            .set_lines(0, -1, false, initial.lines().map(ToOwned::to_owned).collect())
            .await?;
        self.nvim.call_function("ClearUndoHistory", vec![]).await?;
        self.nvim.get_current_win().await?.set_cursor((1, 0)).await?;
        self.nvim.input("<ESC><ESC>").await?;

        self.assert_eq(editor, flags)
            .await
            .context("did not reset state properly before test case")?;

        for (i, key) in inputs.clone().into_iter().enumerate() {
            // https://github.com/neovim/neovim/issues/6159
            // Can't use feedkeys as it will cause hangs
            // Not sure if input is guaranteed to work though since I'm not sure what guarantees
            // nvim provides about when the input will be processed.
            self.nvim.input(&key.to_string()).await?;
            editor.handle_input(key.clone());

            self.assert_eq(editor, flags).await.with_context(|| {
                format!("index {i} in key sequence: `{inputs}`, key=`{key}` text={initial:?}")
            })?;
        }

        Ok(())
    }

    // Compare the state of the editor with the state of the nvim instance
    async fn assert_eq(&self, editor: &zi::Editor, flags: CompareFlags) -> zi::Result<()> {
        let (mut vi_mode, mut blocking) = (None, None);
        self.nvim.get_mode().await?.into_iter().for_each(|(key, value)| {
            match key.as_str().unwrap() {
                "mode" => vi_mode = Some(value.as_str().unwrap().to_owned()),
                "blocking" => blocking = Some(value.as_bool().unwrap()),
                _ => unreachable!(),
            }
        });

        let (vi_mode, blocking) = (vi_mode.unwrap(), blocking.unwrap());
        if blocking {
            // nvim is waiting for input, can't do anything else until it's done.
            // It is important to check this upfront otherwise we will probably get stuck.
            // Even if we try and do this concurrently with the other operations below it won't
            // work since `join!` is concurrent but not parallel.
            //
            // The proptests are a bit slower due to making two serial calls though :/
            return Ok(());
        }

        let (vi_lines, (line, col)) = tokio::try_join!(
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
            }
        )?;

        let zi_buf = editor.buffer(zi::Active);
        let zi_lines = zi_buf.text().to_string();
        let zi_cursor = editor.cursor(zi::Active);

        match vi_mode.as_ref() {
            "i" => ensure!(editor.mode() == zi::Mode::Insert),
            "n" => ensure!(matches!(editor.mode(), zi::Mode::Normal)),
            "no" => ensure!(matches!(editor.mode(), zi::Mode::OperatorPending(_))),
            "v" => ensure!(editor.mode() == zi::Mode::Visual),
            // "V" => zi::Mode::VisualLine,
            // "\x16" => zi::Mode::VisualBlock,
            _ => panic!("unknown mode: {vi_mode}"),
        };

        let vi_cursor = zi::Point::new(line as usize, col as usize);
        ensure_eq(vi_cursor, zi_cursor, vi_lines, zi_lines, flags)?;

        Ok(())
    }
}

fn ensure_eq(
    vi_cursor: zi::Point,
    zi_cursor: zi::Point,
    vi_lines: String,
    zi_lines: String,
    flags: CompareFlags,
) -> Result<(), anyhow::Error> {
    ensure!(vi_cursor == zi_cursor, "vi: {vi_cursor:?}\nzi: {zi_cursor:?}");

    let res = ensure_lines_eq(&vi_lines, &zi_lines);
    if let Ok(()) = res {
        return Ok(());
    }

    if flags.contains(CompareFlags::IGNORE_WHITESPACE_LINES) {
        let mut zi_lines = zi_lines.lines();

        for (i, (vi_line, zi_line)) in vi_lines.lines().zip(&mut zi_lines).enumerate() {
            if vi_line.chars().all(char::is_whitespace) && zi_line.chars().all(char::is_whitespace)
            {
                continue;
            }

            ensure!(vi_line == zi_line, "on line {i}\nvi: {vi_line:?}\nzi: {zi_line:?}");
        }

        if let Some(zi_line) = zi_lines.next() {
            ensure!(
                zi_line.chars().all(char::is_whitespace),
                "unexpected trailing line in zi: {zi_line:?}"
            );
        }

        ensure!(zi_lines.next().is_none(), "zi has more lines than vi");
    }

    res
}

fn ensure_lines_eq(vi_lines: &str, zi_lines: &str) -> Result<(), anyhow::Error> {
    // We allow `zi` to have a single additional newline.
    if vi_lines != &zi_lines[..zi_lines.len().saturating_sub(1)] {
        ensure!(vi_lines == zi_lines, "vi: {vi_lines:?}\nzi: {zi_lines:?}");
    }

    Ok(())
}

const DIR: &str = env!("CARGO_MANIFEST_DIR");

impl Nvim {
    async fn spawn(width: u16, height: u16) -> zi::Result<Nvim> {
        let (nvim, join_handle, child) = nvim_rs::create::tokio::new_child_cmd(
            Command::new("nvim")
                // Command::new("/home/andy/dev/neovim/build/bin/nvim")
                .arg("--embed")
                .arg("--headless") // otherwise nvim will block until a ui is attached
                // .args(["--cmd", &format!("set rtp+={DIR}/runtime/vim-wordmotion")]) // must use --cmd not -c or + as this has to run early
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
