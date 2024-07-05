use std::path::Path;

use datatest_stable::harness;
use zi_nvim::{CompareFlags, Fixture};

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

harness!(nvim_vs_zi_test, "tests/nvim/testdata", r"^.*/*");

// TODO consider caching the neovim instance similar to the proptests once this is slow
#[tokio::main]
async fn nvim_vs_zi_test(path: &Path) -> datatest_stable::Result<()> {
    let fixture = Fixture::load(path)?;
    Ok(fixture.nvim_vs_zi(CompareFlags::empty()).await?)
}
