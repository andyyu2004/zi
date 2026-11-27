use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;

fn plugin_target_dir() -> anyhow::Result<PathBuf> {
    Ok(std::env::current_exe()?
        .ancestors()
        .nth(3)
        .context("unexpected test executable path")?
        .join("plugins"))
}

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

pub fn build_plugin(name: &str) -> anyhow::Result<Vec<u8>> {
    let target_dir = plugin_target_dir()?;
    let target = "wasm32-unknown-unknown";
    let status = Command::new(env!("CARGO"))
        .arg("build")
        .arg("--release")
        .args(["--target", target])
        .arg("--manifest-path")
        .arg(manifest_dir().join(format!("../plugins/{name}/Cargo.toml")))
        .arg("--target-dir")
        .arg(&target_dir)
        // The test build's flags and rustc wrapper (cargo llvm-cov instruments through one)
        // are for the host, not wasm.
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .status()
        .context("failed to run cargo")?;
    anyhow::ensure!(
        status.success(),
        "failed to build plugin `{name}` (is the {target} target installed?)"
    );

    let module =
        std::fs::read(target_dir.join(target).join("release").join(format!("plugin_{name}.wasm")))?;
    wit_component::ComponentEncoder::default().module(&module)?.validate(true).encode()
}

pub fn build_ts_plugin() -> anyhow::Result<Vec<u8>> {
    let node_modules = std::env::var_os("ZI_PLUGIN_TS_NODE_MODULES")
        .map(PathBuf::from)
        .context("ZI_PLUGIN_TS_NODE_MODULES is not set (the nix shells set it)")?;
    let project = plugin_target_dir()?.join("test-ts");
    if project.exists() {
        std::fs::remove_dir_all(&project)?;
    }
    zi_plugin::scaffold(&project, &"test-ts".parse()?)?;
    std::fs::copy(
        manifest_dir().join("fixtures/plugin-ts/plugin.ts"),
        project.join("src/plugin.ts"),
    )?;
    std::os::unix::fs::symlink(node_modules, project.join("node_modules"))?;
    Ok(std::fs::read(zi_plugin::build(&project)?)?)
}
