# AGENTS.md

zi is a vim-like terminal editor in Rust. Cargo workspace, nightly toolchain.

## Build and test

- Use the nix dev shell: `nix develop` (or direnv, `.envrc` has `use flake`). It provides the pinned nightly (from `rust-toolchain.toml`), cargo-nextest, cargo-component, nvim, gopls, go and tree-sitter, and sets `ZI_GRAMMAR_DIR` to the grammars built by the flake.
- Run tests with nextest: `cargo nextest run --workspace --exclude 'plugin-*'`. CI uses `--profile ci`. Run doctests separately with `cargo test --workspace --exclude 'plugin-*' --doc`.
- `cargo test` is not a supported way to run the suite. It runs tests in a shared process; nextest runs each test in its own process.
- Outside the nix shell, grammars are read from `ZI_GRAMMAR_DIR` if set, otherwise the platform data dir (`~/Library/Application Support/zi/grammars` on macOS, `~/.local/share/zi/grammars` on Linux).
- Wasm plugins under `plugins/` don't build with a plain cargo build. The plugin smoke test builds its own; to run zi with plugins, use `just build-plugins`.
- Run `cargo fmt` before committing.
- Logs: `ZI_LOG` takes a tracing filter (`ZI_LOG=zi=debug,zi_lsp=debug`). Tests write logs to their own output, so nextest shows them for a failing test (`--success-output immediate` for passing ones). Language servers' stderr is logged under the `zi_lsp::stderr` target.

## Conventions

- Process-global state (the event registry, statics) is allowed. Test isolation comes from nextest's process per test. Don't add code just so tests can share a process.
- Still treat global-state bugs that would hit a single editor process as bugs.
- Avoid stubs and mocks where possible: test against the real thing (real language servers, nvim for vim behaviour, the real grammars). A test that stubs what it's about only shows the code does what the stub assumes.
- Match on enums exhaustively, without a `_ =>` arm, so a new variant is a compile error wherever it needs a decision.
- Log through `tracing`, never `println!`/`eprintln!`: what happened as the message, what it's about (ids, paths, counts) as fields.
- Put code needed in more than one crate in a shared crate (`stdx`, `zi-core`) rather than copying it.

## Worktrees

- Start each piece of work in a new git worktree beside this checkout, `git worktree add ../zi-<name> -b <branch> origin/master`, never in this checkout. Once its PR has merged or been closed, remove it: `git worktree remove ../zi-<name> && git branch -D <branch>`.
- `master` moves. Fetch and branch from `origin/master`, and rebase before pushing more work onto an open PR.
- Each git worktree needs its own `CARGO_TARGET_DIR`. Workspace crates get the same artifact names in every worktree, so a shared target dir lets builds overwrite each other and tests run another worktree's code.

## Issues and merging

- Bugs and design work are tracked in the private repo `zi-editor/zi-tracking` (`gh issue list -R zi-editor/zi-tracking`). A PR that finishes one says `Fixes zi-editor/zi-tracking#N` in its description.
- After pushing to a PR, watch its CI (`gh pr checks <N> --watch`, in the background) and fix what fails. A PR isn't done until CI passes.
- `master` only takes pull requests, through the merge queue, and needs CI's `test` check. Repo settings and the ruleset are managed in terraform (`~/.dotfiles/terraform/github/zi.tf`), not the GitHub UI.
