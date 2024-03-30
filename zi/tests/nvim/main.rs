use std::path::Path;

use datatest_stable::harness;
use zi_nvim::Fixture;

harness!(nvim_vs_zi_test, "tests/nvim/testdata", r"^.*/*");

#[tokio::main]
async fn nvim_vs_zi_test(path: &Path) -> datatest_stable::Result<()> {
    let fixture = Fixture::load(path)?;
    Ok(fixture.nvim_vs_zi().await?)
}
