use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, ensure};

mod build;
mod language;
mod name;
mod process;
#[cfg(test)]
mod tests;

pub use self::build::build;
pub use self::language::Language;
use self::language::TEMPLATE_NAME;
pub use self::name::PluginName;
use self::process::{run, tool};

pub const WIT: &str = include_str!("../../zi-wasm/wit/zi.wit");

const WORLD: &str = "plugin";

pub fn scaffold(dir: &Path, name: &PluginName, lang: Language) -> anyhow::Result<()> {
    if dir.exists() {
        ensure!(dir.read_dir()?.next().is_none(), "`{}` is not empty", dir.display());
    }

    for (path, contents) in lang.template() {
        let path = dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents.replace(TEMPLATE_NAME, name.as_str()))
            .with_context(|| format!("failed to write `{}`", path.display()))?;
    }
    Ok(())
}

pub fn install_toolchain(project: &Path) -> anyhow::Result<()> {
    match Language::detect(project)? {
        Language::TypeScript | Language::JavaScript => {
            run(Command::new("npm").arg("install").current_dir(project))
        }
        Language::Rust => Ok(()),
    }
}

pub fn types(project: &Path) -> anyhow::Result<()> {
    match Language::detect(project)? {
        Language::TypeScript | Language::JavaScript => {
            write_wit(&project.join("types"))?;
            run(tool(project, "jco")?
                .args(["guest-types", "types/zi.wit", "--world-name", WORLD, "-o", "types"])
                .current_dir(project))
        }
        Language::Rust => write_wit(&project.join("wit")),
    }
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

fn write_wit(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    Ok(std::fs::write(dir.join("zi.wit"), WIT)?)
}
