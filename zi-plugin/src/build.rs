use std::io::BufRead as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context as _, ensure};

use crate::process::{run, tool};
use crate::{Language, PluginName, WORLD, types};

const WASM_TARGET: &str = "wasm32-unknown-unknown";

pub fn build(project: &Path) -> anyhow::Result<PathBuf> {
    let lang = Language::detect(project)?;
    match lang {
        Language::TypeScript | Language::JavaScript => {
            let name = npm_plugin_name(project)?;
            let output = prepare_output(project, &name)?;
            let source = if lang == Language::TypeScript {
                run(tool(project, "tsc")?.args(["-p", "."]).current_dir(project))?;
                "dist/plugin.js"
            } else {
                "src/plugin.js"
            };
            componentize(project, Path::new(source), &output)?;
            Ok(output)
        }
        Language::Rust => {
            let package = RustPackage::load(project)?;
            let output = prepare_output(project, &package.name)?;
            encode_component(&cargo_build(project, &package)?, &output)?;
            Ok(output)
        }
    }
}

fn prepare_output(project: &Path, name: &PluginName) -> anyhow::Result<PathBuf> {
    types(project)?;
    let dist = project.join("dist");
    std::fs::create_dir_all(&dist)?;
    Ok(dist.join(format!("{name}.wasm")))
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

fn cargo_build(project: &Path, package: &RustPackage) -> anyhow::Result<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release", "--target", WASM_TARGET])
        .arg("--manifest-path")
        .arg(&package.manifest)
        .args(["--package", package.name.as_str(), "--lib"])
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
    find_cdylib_wasm(&output.stdout, &package.id)
}

fn find_cdylib_wasm(messages: &[u8], package_id: &str) -> anyhow::Result<PathBuf> {
    let mut wasm = None;
    for line in messages.lines() {
        let message: serde_json::Value = serde_json::from_str(&line?)?;
        if message["reason"] != "compiler-artifact"
            || message["package_id"] != package_id
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
    wasm.with_context(|| format!("cargo built no .wasm cdylib for {package_id}"))
}

pub(crate) fn npm_plugin_name(project: &Path) -> anyhow::Result<PluginName> {
    let path = project.join("package.json");
    let package: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read `{}`", path.display()))?,
    )?;
    package["name"]
        .as_str()
        .with_context(|| format!("`{}` has no package name", path.display()))?
        .parse()
}

pub(crate) struct RustPackage {
    pub(crate) name: PluginName,
    manifest: PathBuf,
    id: String,
}

impl RustPackage {
    pub(crate) fn load(project: &Path) -> anyhow::Result<Self> {
        let manifest = project.canonicalize()?.join("Cargo.toml");
        let output = Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version", "1", "--manifest-path"])
            .arg(&manifest)
            .current_dir(project)
            .output()?;
        ensure!(
            output.status.success(),
            "cargo metadata failed for `{}`: {}",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr).trim_end()
        );
        let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let package = metadata["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|package| package["manifest_path"].as_str().map(Path::new) == Some(&manifest))
            .with_context(|| {
                format!("cargo metadata found no package for `{}`", manifest.display())
            })?;
        let name = package["name"]
            .as_str()
            .with_context(|| format!("`{}` has no package name", manifest.display()))?
            .parse()?;
        let id = package["id"].as_str().context("cargo metadata package has no ID")?.to_owned();
        Ok(Self { name, manifest, id })
    }
}
