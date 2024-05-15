use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::ops::DerefMut;
use std::path::Path;

use anyhow::bail;

use crate::lsp::LanguageClient;
use crate::Result;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileType(Cow<'static, str>);

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl FileType {
    pub const TEXT: Self = Self(Cow::Borrowed("text"));
    pub const GQLT: Self = Self(Cow::Borrowed("gqlt"));
    pub const C: Self = Self(Cow::Borrowed("c"));
    pub const RUST: Self = Self(Cow::Borrowed("rust"));
    pub const GO: Self = Self(Cow::Borrowed("go"));
    pub const TOML: Self = Self(Cow::Borrowed("toml"));
    pub const JSON: Self = Self(Cow::Borrowed("json"));
    pub const PICKER: Self = Self(Cow::Borrowed("picker"));
    pub const EXPLORER: Self = Self(Cow::Borrowed("explorer"));

    pub fn detect(path: &Path) -> Self {
        match path.extension() {
            Some(ext) => match ext {
                x if x == "c" => Self::C,
                x if x == "rs" => Self::RUST,
                x if x == "go" => Self::GO,
                x if x == "toml" => Self::TOML,
                x if x == "json" => Self::JSON,
                x if x == "gqlt" => Self::GQLT,
                _ => Self::TEXT,
            },
            None => Self::TEXT,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<Path> for FileType {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_str().as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageServerId(Cow<'static, str>);

impl fmt::Display for LanguageServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl LanguageServerId {
    pub const RUST_ANALYZER: Self = Self(Cow::Borrowed("rust-analyzer"));
    pub const GOPLS: Self = Self(Cow::Borrowed("gopls"));
    pub const GQLT: Self = Self(Cow::Borrowed("gqlt"));
    pub const CLANGD: Self = Self(Cow::Borrowed("clangd"));
}

#[derive(Debug)]
pub struct Config {
    pub languages: BTreeMap<FileType, LanguageConfig>,
    pub language_servers: BTreeMap<LanguageServerId, LanguageServerConfig>,
}

impl Config {
    pub fn new(
        languages: BTreeMap<FileType, LanguageConfig>,
        language_servers: BTreeMap<LanguageServerId, LanguageServerConfig>,
    ) -> Result<Self> {
        for (lang, config) in &languages {
            for server in &*config.language_servers {
                if !language_servers.contains_key(server) {
                    bail!("language server `{server}` for language `{lang}` is not defined",)
                }
            }
        }

        Ok(Self { languages, language_servers })
    }
}

impl Default for Config {
    fn default() -> Self {
        let languages = BTreeMap::from([
            (
                FileType::RUST,
                LanguageConfig { language_servers: Box::new([LanguageServerId::RUST_ANALYZER]) },
            ),
            (
                FileType::GO,
                LanguageConfig { language_servers: Box::new([LanguageServerId::GOPLS]) },
            ),
            (
                FileType::GQLT,
                LanguageConfig { language_servers: Box::new([LanguageServerId::GQLT]) },
            ),
            (
                FileType::C,
                LanguageConfig { language_servers: Box::new([LanguageServerId::CLANGD]) },
            ),
            (FileType::TEXT, LanguageConfig { language_servers: Box::new([]) }),
            (FileType::TOML, LanguageConfig { language_servers: Box::new([]) }),
            (FileType::JSON, LanguageConfig { language_servers: Box::new([]) }),
        ]);

        let language_servers = BTreeMap::from([
            (
                LanguageServerId::RUST_ANALYZER,
                // LanguageServerConfig { command: "rust-analyzer".into(), args: Box::new([]) },
                LanguageServerConfig { command: "ra-multiplex".into(), args: Box::new([]) },
            ),
            (
                LanguageServerId::GOPLS,
                LanguageServerConfig { command: "gopls".into(), args: Box::new([]) },
            ),
            (
                LanguageServerId::GQLT,
                LanguageServerConfig { command: "gqlt".into(), args: Box::new([]) },
            ),
            (
                LanguageServerId::CLANGD,
                LanguageServerConfig { command: "clangd".into(), args: Box::new([]) },
            ),
        ]);

        Self::new(languages, language_servers).expect("invalid default config")
    }
}

#[derive(Debug, Default)]
pub struct LanguageConfig {
    pub language_servers: Box<[LanguageServerId]>,
}

#[derive(Debug)]
pub struct LanguageServerConfig {
    pub command: OsString,
    pub args: Box<[OsString]>,
}

impl LanguageServerConfig {
    pub(crate) fn spawn(
        &self,
    ) -> zi_lsp::Result<Box<impl DerefMut<Target = zi_lsp::DynLanguageServer>>> {
        tracing::debug!(command = ?self.command, args = ?self.args, "spawn language server");
        zi_lsp::Server::start(LanguageClient, ".", &self.command, &self.args[..]).map(Box::new)
    }
}
