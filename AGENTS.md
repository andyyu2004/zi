# AGENTS.md

zi is a vim-like terminal editor in Rust. Cargo workspace, Rust beta toolchain.

## Build and test

- Use the nix dev shell: `nix develop` (or direnv, `.envrc` has `use flake`). It provides Rust beta (selected by `rust-toolchain.toml` and pinned by `flake.lock`), cargo-nextest, cargo-component, nvim, gopls, go and tree-sitter, and sets `ZI_GRAMMAR_DIR` to the grammars built by the flake. It also has just and coverage tools.
- CI runs in the smaller `nix develop .#ci` shell: only what builds, lints and tests need. Everything either shell needs comes from the flake, not the user's profile; check with `nix develop .#ci -i -k HOME -c ...`.
- Run tests with nextest: `cargo nextest run --workspace --exclude 'plugin-*'`. CI uses `--profile ci`. Run doctests separately with `cargo test --workspace --exclude 'plugin-*' --doc`.
- Measure test coverage with `just coverage` (the workspace), or `just coverage -p zi-text --html --open` for some crates. Its summary table counts generic functions once per instantiation, so check `--show-missing-lines` for lines no test runs.
- The nvim tests (`zi-test/tests/nvim`, `zi-test/tests/nvim-prop`) compare zi against nvim with the plugins in `ZI_NVIM_PLUGIN_PATH` loaded, which the nix shells set. zi's word motions follow vim-wordmotion, so that is one of them.
- `cargo test` is not a supported way to run the suite. It runs tests in a shared process; nextest runs each test in its own process.
- Run `just build-grammars` to build and link the grammar bundle at `target/zi-grammars`, then `just install-grammars` to copy it into a custom `ZI_GRAMMAR_DIR` or the data directory. Outside the nix shell, grammars are read from `ZI_GRAMMAR_DIR` if set, otherwise `$XDG_DATA_HOME/zi/grammars` (default `~/.local/share/zi/grammars`, on macOS too).
- Run `just build-plugins` to build the bundled example plugin with Nix and link the output at `target/zi-plugins`. Run `just install-plugins` to copy that output into `ZI_PLUGIN_DIR` or the checkout's `runtime/plugins` for `just run`. Outside the checkout, Zi uses `ZI_PLUGIN_DIR` or `$XDG_DATA_HOME/zi/plugins` (default `~/.local/share/zi/plugins`). Config (`init.zi`) is read from `$XDG_CONFIG_HOME/zi` (default `~/.config/zi`). `ZI_PLUGIN_PATH` overrides discovery with a platform-separated list of directories. The test plugin is available separately as `nix build .#plugin-test` and the smoke test builds its own fixture. `zi-plugin` (`new`, `types`, `build`, `install`) builds TypeScript, JavaScript and Rust plugins; the `zi-wasm` tests build a fixture in each through it, using the npm toolchain the nix shells point `ZI_PLUGIN_TS_NODE_MODULES` at.
- Run `cargo fmt --all` before committing. CI's clippy job fails on unformatted code (`cargo fmt --all --check`).
- Lint as CI does: `cargo clippy --workspace --exclude 'plugin-*' --all-targets -- -D warnings`, and for plugins `cargo clippy -p 'plugin-*' --target wasm32-unknown-unknown --all-targets -- -D warnings`. Lints are configured in the root `Cargo.toml`; silence one locally with `#[expect(clippy::..., reason = "...")]`, not `#[allow]`.
- Logs: `ZI_LOG` takes a tracing filter (`ZI_LOG=zi=debug,zi_lsp=debug`). Tests write logs to their own output, so nextest shows them for a failing test (`--success-output immediate` for passing ones). Language servers' stderr is logged under the `zi_lsp::stderr` target.

## Conventions

- Process-global state (the event registry, statics) is allowed. Test isolation comes from nextest's process per test. Don't add code just so tests can share a process.
- Still treat global-state bugs that would hit a single editor process as bugs.
- Avoid stubs and mocks where possible: test against the real thing (real language servers, nvim for vim behaviour, the real grammars). A test that stubs what it's about only shows the code does what the stub assumes.
- Match on enums exhaustively, without a `_ =>` arm, so a new variant is a compile error wherever it needs a decision.
- Use a `bool` only for something that will always have exactly two states, such as the `readonly` setting. Don't take a `bool` parameter: a bare `true` at the call site is ambiguous, so use a two-variant enum or bitflags (`OpenFlags::READONLY`).
- Prefer a type that says what a string holds (`Url`, `PathBuf`, `FileType`, a newtype) to `String` or `&str`.
- Don't access tuple fields as `.0`, `.1`: destructure the tuple into named bindings (`let (start, end) = ...`, `|(_, right, id)| ...`).
- The plugin API (`zi-wasm/wit/zi.wit`) follows the Rust API for now: when an editor API that plugins can reach changes, change the WIT and the test plugin (`plugins/test`) in the same PR.
- Log through `tracing`, never `println!`/`eprintln!`: what happened as the message, what it's about (ids, paths, counts) as fields.
- Put code needed in more than one crate in a shared crate (`stdx`, `zi-core`) rather than copying it.

## Worktrees

- Start each piece of work in a new git worktree beside this checkout, `git worktree add ../zi-<name> -b <branch> origin/master`, never in this checkout. Once its PR has merged or been closed, remove it: `git worktree remove ../zi-<name> && git branch -D <branch>`.
- `master` moves. Fetch and branch from `origin/master`, and rebase before pushing more work onto an open PR.
- Each git worktree needs its own `CARGO_TARGET_DIR`. Workspace crates get the same artifact names in every worktree, so a shared target dir lets builds overwrite each other and tests run another worktree's code.

## Issues and merging

- Bugs and design work are tracked in the private repo `zi-editor/zi-tracking` (`gh issue list -R zi-editor/zi-tracking`). A PR that finishes one says `Fixes zi-editor/zi-tracking#N` in its description.
- When a bug isn't clear from a sentence (it takes a sequence of keys or calls to hit, or the wrong result is subtle), show it with a worked example: the starting text and cursor, each key or call in order, and what the buffer, cursor or output is at each step, before and after the fix. Put it in the PR description, or in the linked tracking issue with an explicit pointer from the PR ("see the example in zi-editor/zi-tracking#N"). Wrap key names in backticks: GitHub hides a bare `<Tab>` as an HTML tag.
- After pushing to a PR, watch its CI (`gh pr checks <N> --watch`, in the background) and fix what fails. A PR isn't done until CI passes.
- `master` only takes pull requests, through the merge queue, and needs CI's `test` check. Repo settings and the ruleset are managed in terraform (`~/.dotfiles/terraform/github/zi.tf`), not the GitHub UI.
