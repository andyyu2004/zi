use std::io::BufRead as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context as _, bail, ensure};

use crate::process::{run, tool};
use crate::{Language, PluginName, WORLD, types};

const WASM_TARGET: &str = "wasm32-unknown-unknown";

pub fn build(project: &Path) -> anyhow::Result<PathBuf> {
    let lang = Language::detect(project)?;
    let name = plugin_name(project, lang)?;
    types(project)?;

    let output = project.join("dist").join(format!("{name}.wasm"));
    std::fs::create_dir_all(project.join("dist"))?;
    match lang {
        Language::TypeScript => {
            run(tool(project, "tsc")?.args(["-p", "."]).current_dir(project))?;
            componentize(project, Path::new("dist/plugin.js"), &output)?;
        }
        Language::JavaScript => componentize(project, Path::new("src/plugin.js"), &output)?,
        Language::Rust => encode_component(&cargo_build(project)?, &output)?,
    }
    Ok(output)
}

fn componentize(project: &Path, js: &Path, output: &Path) -> anyhow::Result<()> {
    run(tool(project, "jco")?
        .arg("componentize")
        .arg(js)
        .args(["--wit", "types/zi.wit", "--world-name", WORLD])
        .args(["--disable", "http", "--disable", "fetch-event"])
        .arg("-o")
        .arg(std::path::absolute(output)?)
        .current_dir(project))
}

fn encode_component(module: &Path, output: &Path) -> anyhow::Result<()> {
    let module = std::fs::read(module)?;
    let component =
        wit_component::ComponentEncoder::default().module(&module)?.validate(true).encode()?;
    Ok(std::fs::write(output, component)?)
}

fn cargo_build(project: &Path) -> anyhow::Result<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release", "--target", WASM_TARGET])
        .arg("--message-format=json-render-diagnostics")
        .current_dir(project)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");
    tracing::info!(command = ?cmd, "running");
    let output = cmd.output().with_context(|| format!("failed to run {cmd:?}"))?;
    ensure!(
        output.status.success(),
        "{cmd:?} failed with {} (is the {WASM_TARGET} target installed?)",
        output.status
    );
    find_cdylib_wasm(&output.stdout)
}

fn find_cdylib_wasm(messages: &[u8]) -> anyhow::Result<PathBuf> {
    let mut wasm = None;
    for line in messages.lines() {
        let message: serde_json::Value = serde_json::from_str(&line?)?;
        if message["reason"] != "compiler-artifact"
            || !message["target"]["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.contains(&"cdylib".into()))
        {
            continue;
        }
        let files = message["filenames"].as_array().into_iter().flatten();
        wasm = files
            .filter_map(|file| file.as_str())
            .map(PathBuf::from)
            .find(|file| file.extension().is_some_and(|ext| ext == "wasm"));
    }
    wasm.context("cargo built no .wasm cdylib")
}

pub(crate) fn plugin_name(project: &Path, lang: Language) -> anyhow::Result<PluginName> {
    let (path, name) = match lang {
        Language::TypeScript | Language::JavaScript => {
            let path = project.join("package.json");
            let package: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&path)
                    .with_context(|| format!("failed to read `{}`", path.display()))?,
            )?;
            (path, package["name"].as_str().map(str::to_owned))
        }
        Language::Rust => {
            let output = Command::new("cargo")
                .args(["metadata", "--no-deps", "--format-version", "1"])
                .current_dir(project)
                .output()?;
            ensure!(output.status.success(), "cargo metadata failed in `{}`", project.display());
            let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            (
                project.join("Cargo.toml"),
                metadata["packages"][0]["name"].as_str().map(str::to_owned),
            )
        }
    };
    match name {
        Some(name) => name.parse(),
        None => bail!("`{}` has no package name", path.display()),
    }
}
