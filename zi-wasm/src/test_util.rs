use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;
use zi_plugin::Language;

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

pub fn build_fixture_plugin(lang: Language) -> anyhow::Result<Vec<u8>> {
    let (name, fixture) = match lang {
        Language::TypeScript => ("test-ts", "plugin-ts/plugin.ts"),
        Language::JavaScript => ("test-js", "plugin-js/plugin.js"),
        Language::Rust => ("test-rs", "plugin-rs/lib.rs"),
    };
    build_source_plugin(
        lang,
        name,
        &std::fs::read_to_string(manifest_dir().join("fixtures").join(fixture))?,
    )
}

pub fn build_source_plugin(lang: Language, name: &str, source: &str) -> anyhow::Result<Vec<u8>> {
    let source_path = match lang {
        Language::TypeScript => "src/plugin.ts",
        Language::JavaScript => "src/plugin.js",
        Language::Rust => "src/lib.rs",
    };
    let target_dir = plugin_target_dir()?;
    std::fs::create_dir_all(&target_dir)?;
    // A fresh directory per build, so tests building the same plugin at once don't collide.
    let tempdir = tempfile::Builder::new().prefix(name).tempdir_in(&target_dir)?;
    let project = tempdir.path().join(name);
    zi_plugin::scaffold(&project, &name.parse()?, lang)?;
    std::fs::write(project.join(source_path), source)?;
    match lang {
        Language::TypeScript | Language::JavaScript => {
            let node_modules = std::env::var_os("ZI_PLUGIN_TS_NODE_MODULES")
                .context("ZI_PLUGIN_TS_NODE_MODULES is not set (the nix shells set it)")?;
            std::os::unix::fs::symlink(node_modules, project.join("node_modules"))?;
        }
        Language::Rust => {}
    }
    Ok(std::fs::read(zi_plugin::build(&project)?)?)
}
