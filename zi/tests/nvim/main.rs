//! Tests against a headless neovim instance

use std::path::Path;
use std::str::FromStr;

use anyhow::{ensure, Context};
use datatest_stable::harness;
use nvim_rs::error::LoopError;
use tokio::process::{ChildStdin, Command};
use zi::input::KeySequence;

harness!(nvim_vs_zi_test, "tests/nvim/testdata", r"^.*/*");

struct Fixture {
    text: String,
    size: zi::Size,
    inputs: KeySequence,
}

impl Fixture {
    fn load(path: &Path) -> zi::Result<Self> {
        let reader = std::fs::read_to_string(path)?;
        let mut text = String::new();
        let mut lines = reader.split_inclusive('\n');
        const SEP: &str = "----";
        for line in lines.by_ref().take_while(|line| !line.starts_with(SEP)) {
            text.push_str(line);
        }

        let line = lines.next().expect("expected input key sequence line after ----");
        let inputs = KeySequence::from_str(line.trim()).expect("could not parse key sequence");

        assert!(lines.next().is_none(), "expected EOF after key sequence");

        Ok(Self { text, size: zi::Size::new(80, 24), inputs })
    }
}

#[tokio::main]
async fn nvim_vs_zi_test(path: &Path) -> datatest_stable::Result<()> {
    let fixture = Fixture::load(path)?;
    Ok(nvim_vs_zi(fixture).await?)
}

async fn nvim_vs_zi(fixture: Fixture) -> zi::Result<()> {
    let size = fixture.size;
    let nvim = Nvim::spawn(size.width, size.height).await?;
    let (mut editor, _tasks) = zi::Editor::new(size);

    nvim.run(&mut editor, &fixture.text, fixture.inputs).await?;

    Ok(())
}

struct Nvim {
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
        initial: &str,
        seq: KeySequence,
    ) -> zi::Result<()> {
        let initial = initial.trim_end();
        editor.active_buffer_mut().edit(&zi::Delta::new(zi::Range::default(), initial));
        self.nvim
            .put(initial.lines().map(|line| line.to_string()).collect::<Vec<_>>(), "c", true, false)
            .await?;
        self.nvim.get_current_win().await?.set_cursor((1, 0)).await?;
        self.assert_eq(editor).await?;

        for (i, key) in seq.clone().into_iter().enumerate() {
            self.nvim.feedkeys(&key.to_string(), "m", true).await?;
            editor.handle_input(key.clone());
            self.assert_eq(editor)
                .await
                .with_context(|| format!("index {i} in key sequence: `{seq}`, key: `{key}`"))?;
        }

        Ok(())
    }

    // Compare the state of the editor with the state of the nvim instance
    async fn assert_eq(&self, editor: &zi::Editor) -> zi::Result<()> {
        let vi_buf = self.nvim.get_current_buf().await?;
        let mut vi_lines = vi_buf.get_lines(0, -1, false).await?.join("\n");
        // zi always adds a newline at the end
        vi_lines.push('\n');
        let vi_win = self.nvim.get_current_win().await?;
        let (line, col) = vi_win.get_cursor().await?;
        let line = line.checked_sub(1).expect("1-indexed lines");

        let zi_buf = editor.active_buffer();
        let zi_lines = zi_buf.text().to_string();
        let zi_cursor = editor.active_cursor();

        let vi_mode = self
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
            .expect("Could not find mode value");

        let mode = match vi_mode.as_ref() {
            "i" => zi::Mode::Insert,
            "n" => zi::Mode::Normal,
            "v" => zi::Mode::Visual,
            // "V" => zi::Mode::VisualLine,
            // "\x16" => zi::Mode::VisualBlock,
            _ => panic!("unknown mode: {vi_mode}"),
        };

        ensure!(mode == editor.mode());
        ensure!(vi_lines == zi_lines);
        ensure!(line as usize == zi_cursor.line().idx());
        ensure!(col as usize == zi_cursor.col().idx());
        Ok(())
    }
}

impl Nvim {
    async fn spawn(width: u16, height: u16) -> zi::Result<Nvim> {
        let (nvim, join_handle, child) = nvim_rs::create::tokio::new_child_cmd(
            Command::new("nvim")
                .arg("--embed")
                .arg("--headless") // otherwise nvim will block until a ui is attached
                .arg("--clean")
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
