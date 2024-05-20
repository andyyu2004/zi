use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::future::Future;
use std::ops::DerefMut;
use std::path::Path;
use std::pin::Pin;

use anyhow::bail;

use crate::lsp::LanguageClient;
use crate::symbol::Symbol;
use crate::Result;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileType(Symbol);

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
    pub const TEXT: Self = Self(Symbol::const_new("text"));
    pub const GQLT: Self = Self(Symbol::const_new("gqlt"));
    pub const C: Self = Self(Symbol::const_new("c"));
    pub const RUST: Self = Self(Symbol::const_new("rust"));
    pub const GO: Self = Self(Symbol::const_new("go"));
    pub const TOML: Self = Self(Symbol::const_new("toml"));
    pub const JSON: Self = Self(Symbol::const_new("json"));
    pub const PICKER: Self = Self(Symbol::const_new("picker"));
    pub const EXPLORER: Self = Self(Symbol::const_new("explorer"));

    pub fn new(name: &'static str) -> Self {
        Self(Symbol::const_new(name))
    }

    pub fn detect(path: &Path) -> Self {
        match path.extension() {
            Some(ext) => match ext.to_str() {
                Some("c") => Self::C,
                Some("rs") => Self::RUST,
                Some("go") => Self::GO,
                Some("toml") => Self::TOML,
                Some("json") => Self::JSON,
                Some("gqlt") => Self::GQLT,
                Some("txt") | Some("text") => Self::TEXT,
                // Some(ext) => Self(Symbol::const_new(ext)),
                // TODO need some string interning mechanism to get &'static str without repeated allocations
                Some(_ext) => Self::TEXT,
                None => Self::TEXT,
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageServerId(Symbol);

impl LanguageServerId {
    pub fn new(id: impl Into<Symbol>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LanguageServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl LanguageServerId {
    pub const RUST_ANALYZER: Self = Self(Symbol::const_new("rust-analyzer"));
    pub const GOPLS: Self = Self(Symbol::const_new("gopls"));
    pub const GQLT: Self = Self(Symbol::const_new("gqlt"));
    pub const CLANGD: Self = Self(Symbol::const_new("clangd"));
}

impl From<&'static str> for LanguageServerId {
    fn from(id: &'static str) -> Self {
        Self(Symbol::const_new(id))
    }
}

pub trait LanguageServerConfig {
    /// Spawn a new language server instance.
    /// Returns a boxed language server client and a future to spawn to run the server.
    fn spawn(
        &self,
    ) -> zi_lsp::Result<(
        Box<dyn DerefMut<Target = zi_lsp::DynLanguageServer> + Send>,
        Pin<Box<dyn Future<Output = zi_lsp::Result<()>> + Send>>,
    )>;
}

pub struct Config {
    pub(crate) languages: BTreeMap<FileType, LanguageConfig>,
    pub(crate) language_servers: BTreeMap<LanguageServerId, Box<dyn LanguageServerConfig + Send>>,
}

impl Config {
    pub fn new(
        languages: BTreeMap<FileType, LanguageConfig>,
        language_servers: BTreeMap<LanguageServerId, Box<dyn LanguageServerConfig + Send>>,
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

    pub fn add_language(
        &mut self,
        file_type: impl Into<FileType>,
        config: LanguageConfig,
    ) -> &mut Self {
        self.languages.insert(file_type.into(), config);
        self
    }

    pub fn add_language_server(
        &mut self,
        id: impl Into<LanguageServerId>,
        config: impl LanguageServerConfig + Send + 'static,
    ) -> &mut Self {
        self.language_servers.insert(id.into(), Box::new(config));
        self
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

        let language_servers = BTreeMap::from(
            [
                (
                    LanguageServerId::RUST_ANALYZER,
                    // ExecutableLanguageServerConfig {
                    //     command: "rust-analyzer".into(),
                    //     args: Box::new([]),
                    // },
                    ExecutableLanguageServerConfig {
                        command: "ra-multiplex".into(),
                        args: Box::new([]),
                    },
                ),
                (
                    LanguageServerId::GOPLS,
                    ExecutableLanguageServerConfig { command: "gopls".into(), args: Box::new([]) },
                ),
                (
                    LanguageServerId::GQLT,
                    ExecutableLanguageServerConfig { command: "gqlt".into(), args: Box::new([]) },
                ),
                (
                    LanguageServerId::CLANGD,
                    ExecutableLanguageServerConfig { command: "clangd".into(), args: Box::new([]) },
                ),
            ]
            .map(|(k, v)| (k, Box::new(v) as _)),
        );

        Self::new(languages, language_servers).expect("invalid default config")
    }
}

#[derive(Debug, Default)]
pub struct LanguageConfig {
    pub(crate) language_servers: Box<[LanguageServerId]>,
}

impl LanguageConfig {
    pub fn new(language_servers: impl IntoIterator<Item = LanguageServerId>) -> Self {
        Self { language_servers: language_servers.into_iter().collect() }
    }
}

#[derive(Debug)]
pub struct ExecutableLanguageServerConfig {
    pub command: OsString,
    pub args: Box<[OsString]>,
}

impl LanguageServerConfig for ExecutableLanguageServerConfig {
    fn spawn(
        &self,
    ) -> zi_lsp::Result<(
        Box<dyn DerefMut<Target = zi_lsp::DynLanguageServer> + Send>,
        Pin<Box<dyn std::future::Future<Output = zi_lsp::Result<()>> + Send>>,
    )> {
        tracing::debug!(command = ?self.command, args = ?self.args, "spawn language server");
        let (server, fut) =
            zi_lsp::Server::start(LanguageClient, ".", &self.command, &self.args[..])?;
        Ok((Box::new(server), Box::pin(fut)))
    }
}
