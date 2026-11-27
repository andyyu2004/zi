use std::fmt;
use std::io::BufRead as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;

use anyhow::{Context as _, bail, ensure};

pub const WIT: &str = include_str!("../../zi-wasm/wit/zi.wit");

const WORLD: &str = "plugin";

const TEMPLATE_NAME: &str = "zi-plugin-template";

const NPM_TEMPLATE: &[(&str, &str)] = &[
    ("package.json", include_str!("../template/common/package.json")),
    ("package-lock.json", include_str!("../template/common/package-lock.json")),
    (".gitignore", include_str!("../template/common/.gitignore")),
];

const WASM_TARGET: &str = "wasm32-unknown-unknown";

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Language {
    #[value(name = "ts", alias = "typescript")]
    TypeScript,
    #[value(name = "js", alias = "javascript")]
    JavaScript,
    #[value(name = "rust", alias = "rs")]
    Rust,
}

impl Language {
    const ALL: [Self; 3] = [Self::TypeScript, Self::JavaScript, Self::Rust];

    fn marker(self) -> &'static str {
        match self {
            Self::TypeScript => "src/plugin.ts",
            Self::JavaScript => "src/plugin.js",
            Self::Rust => "Cargo.toml",
        }
    }

    fn template(self) -> Vec<(&'static str, &'static str)> {
        match self {
            Self::TypeScript => [
                NPM_TEMPLATE,
                &[
                    ("tsconfig.json", include_str!("../template/ts/tsconfig.json")),
                    ("src/plugin.ts", include_str!("../template/ts/src/plugin.ts")),
                ],
            ]
            .concat(),
            Self::JavaScript => [
                NPM_TEMPLATE,
                &[
                    ("jsconfig.json", include_str!("../template/js/jsconfig.json")),
                    ("src/plugin.js", include_str!("../template/js/src/plugin.js")),
                ],
            ]
            .concat(),
            Self::Rust => vec![
                ("Cargo.toml", include_str!("../template/rs/Cargo.toml")),
                ("Cargo.lock", include_str!("../template/rs/Cargo.lock")),
                (".gitignore", include_str!("../template/rs/.gitignore")),
                ("src/lib.rs", include_str!("../template/rs/src/lib.rs")),
            ],
        }
    }

    pub fn detect(project: &Path) -> anyhow::Result<Self> {
        let found = Self::ALL
            .into_iter()
            .filter(|lang| project.join(lang.marker()).is_file())
            .collect::<Vec<_>>();
        match found[..] {
            [lang] => Ok(lang),
            [] => bail!(
                "`{}` is not a plugin project (it has none of {})",
                project.display(),
                Self::ALL.map(Self::marker).join(", ")
            ),
            _ => bail!(
                "`{}` looks like more than one kind of plugin project: {}",
                project.display(),
                found.into_iter().map(Self::marker).collect::<Vec<_>>().join(", ")
            ),
        }
    }
}

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

pub fn scaffold(dir: &Path, name: &PluginName, lang: Language) -> anyhow::Result<()> {
    if dir.exists() {
        ensure!(dir.read_dir()?.next().is_none(), "`{}` is not empty", dir.display());
    }

    for (path, contents) in lang.template() {
        let path = dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents.replace(TEMPLATE_NAME, &name.0))
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

pub fn build(project: &Path) -> anyhow::Result<PathBuf> {
    let lang = Language::detect(project)?;
    let name = plugin_name(project, lang)?;
    types(project)?;

    let output = project.join("dist").join(format!("{name}.wasm"));
    std::fs::create_dir_all(project.join("dist"))?;
    match lang {
        Language::TypeScript => {
            run(tool(project, "tsc")?.args(["-p", "."]).current_dir(project))?;
            componentize(project, "dist/plugin.js", &output)?;
        }
        Language::JavaScript => componentize(project, "src/plugin.js", &output)?,
        Language::Rust => {
            let module = std::fs::read(cargo_build(project)?)?;
            let component = wit_component::ComponentEncoder::default()
                .module(&module)?
                .validate(true)
                .encode()?;
            std::fs::write(&output, component)?;
        }
    }
    Ok(output)
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

fn componentize(project: &Path, js: &str, output: &Path) -> anyhow::Result<()> {
    run(tool(project, "jco")?
        .args(["componentize", js, "--wit", "types/zi.wit", "--world-name", WORLD])
        .args(["--disable", "http", "--disable", "fetch-event"])
        .arg("-o")
        .arg(std::path::absolute(output)?)
        .current_dir(project))
}

fn cargo_build(project: &Path) -> anyhow::Result<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release", "--target", WASM_TARGET])
        .arg("--message-format=json-render-diagnostics")
        .current_dir(project)
        .stdout(Stdio::piped())
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

    let mut wasm = None;
    for line in output.stdout.lines() {
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

fn plugin_name(project: &Path, lang: Language) -> anyhow::Result<PluginName> {
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

    fn source(lang: Language) -> &'static str {
        match lang {
            Language::TypeScript => "src/plugin.ts",
            Language::JavaScript => "src/plugin.js",
            Language::Rust => "src/lib.rs",
        }
    }

    #[test]
    fn detect_language() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Language::detect(dir.path()).unwrap_err().to_string().contains("not a plugin"));

        for lang in Language::ALL {
            let project = dir.path().join(format!("{lang:?}"));
            scaffold(&project, &"p".parse().unwrap(), lang).unwrap();
            assert_eq!(Language::detect(&project).unwrap(), lang);
        }

        let both = dir.path().join("both");
        scaffold(&both, &"p".parse().unwrap(), Language::TypeScript).unwrap();
        std::fs::write(both.join("src/plugin.js"), "").unwrap();
        assert!(Language::detect(&both).unwrap_err().to_string().contains("more than one"));
    }

    #[test]
    fn scaffold_names_the_plugin() {
        let dir = tempfile::tempdir().unwrap();
        for lang in Language::ALL {
            let project = dir.path().join(format!("{lang:?}"));
            scaffold(&project, &"my-plugin".parse().unwrap(), lang).unwrap();
            assert_eq!(plugin_name(&project, lang).unwrap().to_string(), "my-plugin");
            let source = std::fs::read_to_string(project.join(source(lang))).unwrap();
            assert!(source.contains(r#""my-plugin""#), "{source}");
            assert!(!source.contains(TEMPLATE_NAME));
        }
    }

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
