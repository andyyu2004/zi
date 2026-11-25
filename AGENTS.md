# AGENTS.md

zi is a vim-like terminal editor in Rust. Cargo workspace, nightly toolchain.

## Build and test

- Use the nix dev shell: `nix develop`. It provides the pinned nightly (from `rust-toolchain.toml`), cargo-nextest, nvim, gopls and tree-sitter.
- Run tests with nextest: `cargo nextest run --workspace --exclude 'plugin-*'`. Run doctests separately with `cargo test --doc`.
- `cargo test` is not a supported way to run the suite. It runs tests in a shared process; nextest runs each test in its own process.
- Wasm plugins under `plugins/` don't build with a plain cargo build; use `just build-plugins`.

## Conventions

- Process-global state (the event registry, statics) is allowed. Test isolation comes from nextest's process per test. Don't add code just so tests can share a process.
- Still treat global-state bugs that would hit a single editor process as bugs.

## Worktrees

- Each git worktree needs its own `CARGO_TARGET_DIR`. Workspace crates get the same artifact names in every worktree, so a shared target dir lets builds overwrite each other and tests run another worktree's code.

## Issues

- Bugs and design work are tracked in the private repo `andyyu2004/zi-tracking`. Reference it from PRs as `andyyu2004/zi-tracking#N`.
