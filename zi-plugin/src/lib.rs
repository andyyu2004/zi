use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use anyhow::{Context as _, bail, ensure};

pub const WIT: &str = include_str!("../../zi-wasm/wit/zi.wit");

const WORLD: &str = "plugin";

const TEMPLATE_NAME: &str = "zi-plugin-template";

const TEMPLATE: &[(&str, &str)] = &[
    ("package.json", include_str!("../template/package.json")),
    ("package-lock.json", include_str!("../template/package-lock.json")),
    ("tsconfig.json", include_str!("../template/tsconfig.json")),
    (".gitignore", include_str!("../template/.gitignore")),
    ("src/plugin.ts", include_str!("../template/src/plugin.ts")),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginName(String);

impl FromStr for PluginName {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        ensure!(
            !name.is_empty()
                && name.starts_with(|c: char| c.is_ascii_lowercase())
                && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "invalid plugin name `{name}`: use lowercase letters, digits and `-`, starting with a letter"
        );
        Ok(Self(name.to_owned()))
    }
}

impl fmt::Display for PluginName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn scaffold(dir: &Path, name: &PluginName) -> anyhow::Result<()> {
    if dir.exists() {
        ensure!(dir.read_dir()?.next().is_none(), "`{}` is not empty", dir.display());
    }

    for (path, contents) in TEMPLATE {
        let path = dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents.replace(TEMPLATE_NAME, &name.0))
            .with_context(|| format!("failed to write `{}`", path.display()))?;
    }
    Ok(())
}

pub fn npm_install(project: &Path) -> anyhow::Result<()> {
    run(Command::new("npm").arg("install").current_dir(project))
}

pub fn types(project: &Path) -> anyhow::Result<()> {
    let types = project.join("types");
    std::fs::create_dir_all(&types)?;
    std::fs::write(types.join("zi.wit"), WIT)?;
    run(tool(project, "jco")?
        .args(["guest-types", "types/zi.wit", "--world-name", WORLD, "-o", "types"])
        .current_dir(project))
}

pub fn build(project: &Path) -> anyhow::Result<PathBuf> {
    let name = package_name(project)?;
    types(project)?;
    run(tool(project, "tsc")?.args(["-p", "."]).current_dir(project))?;

    let output = PathBuf::from("dist").join(format!("{name}.wasm"));
    run(tool(project, "jco")?
        .args(["componentize", "dist/plugin.js", "--wit", "types/zi.wit", "--world-name", WORLD])
        .args(["--disable", "http", "--disable", "fetch-event"])
        .arg("-o")
        .arg(&output)
        .current_dir(project))?;
    Ok(project.join(output))
}

pub fn install(project: &Path) -> anyhow::Result<PathBuf> {
    let component = build(project)?;
    let plugin_dir = zi_core::dirs::plugin().next().context("no plugin directory")?;
    let file_name = component.file_name().context("component has no file name")?;
    let dest = plugin_dir.join(file_name);
    std::fs::copy(&component, &dest).with_context(|| {
        format!("failed to copy `{}` to `{}`", component.display(), dest.display())
    })?;
    Ok(dest)
}

fn package_name(project: &Path) -> anyhow::Result<PluginName> {
    let path = project.join("package.json");
    let package: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read `{}`", path.display()))?,
    )?;
    match package.get("name").and_then(|name| name.as_str()) {
        Some(name) => name.parse(),
        None => bail!("`{}` has no `name`", path.display()),
    }
}

fn tool(project: &Path, name: &str) -> anyhow::Result<Command> {
    let path = std::path::absolute(project)?.join("node_modules/.bin").join(name);
    ensure!(
        path.exists(),
        "`{}` not found: run `npm install` in `{}`",
        path.display(),
        project.display()
    );
    Ok(Command::new(path))
}

fn run(cmd: &mut Command) -> anyhow::Result<()> {
    tracing::info!(command = ?cmd, "running");
    let status = cmd.status().with_context(|| format!("failed to run {cmd:?}"))?;
    ensure!(status.success(), "{cmd:?} failed with {status}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_resolve_from_a_relative_project() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("project/node_modules/.bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("jco"), "").unwrap();
        std::env::set_current_dir(root.path()).unwrap();

        let cmd = tool(Path::new("project"), "jco").unwrap();
        assert_eq!(Path::new(cmd.get_program()), bin.canonicalize().unwrap().join("jco"));
    }

    #[test]
    fn plugin_names() {
        for name in ["zi", "my-plugin", "a1"] {
            assert_eq!(name.parse::<PluginName>().unwrap().to_string(), name);
        }
        for name in ["", "My-Plugin", "1a", "-a", "a_b", "a/b", "@scope/a"] {
            assert!(name.parse::<PluginName>().is_err(), "{name}");
        }
    }
}
