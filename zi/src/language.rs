use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::ops::DerefMut;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::bail;
use futures_core::future::BoxFuture;
use ustr::{ustr, Ustr};

use crate::lsp::LanguageClient;
use crate::Result;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileType(Ustr);

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

#[doc(hidden)]
pub struct KnownFileTypes {
    pub text: FileType,
    pub gqlt: FileType,
    pub javascript: FileType,
    pub typescript: FileType,
    pub c: FileType,
    pub rust: FileType,
    pub go: FileType,
    pub toml: FileType,
    pub json: FileType,
    pub picker: FileType,
    pub explorer: FileType,
}

fn ft(ft: &str) -> FileType {
    FileType(ustr(ft))
}

#[macro_export]
macro_rules! filetype {
    ($ft:ident) => {
        $crate::FileType::known().$ft
    };
}

impl FileType {
    #[doc(hidden)]
    pub fn known() -> &'static KnownFileTypes {
        static KNOWN_FILE_TYPES: OnceLock<KnownFileTypes> = OnceLock::new();
        KNOWN_FILE_TYPES.get_or_init(|| KnownFileTypes {
            text: ft("text"),
            gqlt: ft("gqlt"),
            javascript: ft("javascript"),
            typescript: ft("typescript"),
            c: ft("c"),
            rust: ft("rust"),
            go: ft("go"),
            toml: ft("toml"),
            json: ft("json"),
            picker: ft("picker"),
            explorer: ft("explorer"),
        })
    }

    pub fn detect(path: &Path) -> Self {
        match path.extension() {
            Some(ext) => match ext.to_str() {
                Some("c") => filetype!(c),
                Some("rs") => filetype!(rust),
                Some("go") => filetype!(go),
                Some("toml") => filetype!(toml),
                Some("json") => filetype!(json),
                Some("gqlt") => filetype!(gqlt),
                Some("js") => filetype!(javascript),
                Some("ts") => filetype!(typescript),
                _ => filetype!(text),
            },
            None => filetype!(text),
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
pub struct LanguageServiceId(Ustr);

#[doc(hidden)]
struct KnownLanguageServers {
    pub rust_analyzer: LanguageServiceId,
    pub tsserver: LanguageServiceId,
    pub gopls: LanguageServiceId,
    pub gqlt: LanguageServiceId,
    pub clangd: LanguageServiceId,
}

macro_rules! language_server_id {
    ($id:ident) => {
        $crate::LanguageServiceId::known().$id
    };
}

impl From<&str> for LanguageServiceId {
    fn from(id: &str) -> Self {
        Self(ustr(id))
    }
}

impl LanguageServiceId {
    pub fn new(id: impl Into<Ustr>) -> Self {
        Self(id.into())
    }

    #[doc(hidden)]
    fn known() -> &'static KnownLanguageServers {
        static KNOWN_LANGUAGE_SERVERS: OnceLock<KnownLanguageServers> = OnceLock::new();
        KNOWN_LANGUAGE_SERVERS.get_or_init(|| KnownLanguageServers {
            rust_analyzer: LanguageServiceId::new("rust-analyzer"),
            tsserver: LanguageServiceId::new("tsserver"),
            gopls: LanguageServiceId::new("gopls"),
            gqlt: LanguageServiceId::new("gqlt"),
            clangd: LanguageServiceId::new("clangd"),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LanguageServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait LanguageServerConfig {
    /// Spawn a new language server instance.
    /// Returns a boxed language server client and a future to spawn to run the server.
    #[allow(clippy::type_complexity)]
    fn spawn(
        &self,
        cwd: &Path,
        client: LanguageClient,
    ) -> zi_lsp::Result<(
        Box<dyn DerefMut<Target = zi_lsp::DynLanguageServer> + Send>,
        BoxFuture<'static, zi_lsp::Result<()>>,
    )>;
}

pub struct Config {
    pub(crate) languages: BTreeMap<FileType, LanguageConfig>,
    pub(crate) language_services: BTreeMap<LanguageServiceId, Box<dyn LanguageServerConfig + Send>>,
}

impl Config {
    pub fn new(
        languages: BTreeMap<FileType, LanguageConfig>,
        language_servers: BTreeMap<LanguageServiceId, Box<dyn LanguageServerConfig + Send>>,
    ) -> Result<Self> {
        for (lang, config) in &languages {
            for server in &*config.language_services {
                if !language_servers.contains_key(server) {
                    bail!("language server `{server}` for language `{lang}` is not defined",)
                }
            }
        }

        Ok(Self { languages, language_services: language_servers })
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
        id: impl Into<LanguageServiceId>,
        config: impl LanguageServerConfig + Send + 'static,
    ) -> &mut Self {
        self.language_services.insert(id.into(), Box::new(config));
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        let languages = BTreeMap::from([
            (
                filetype!(rust),
                LanguageConfig {
                    language_services: Box::new([language_server_id!(rust_analyzer)]),
                },
            ),
            (
                filetype!(go),
                LanguageConfig { language_services: Box::new([language_server_id!(gopls)]) },
            ),
            (
                filetype!(gqlt),
                LanguageConfig { language_services: Box::new([language_server_id!(gqlt)]) },
            ),
            (
                filetype!(c),
                LanguageConfig { language_services: Box::new([language_server_id!(clangd)]) },
            ),
            (
                filetype!(javascript),
                LanguageConfig { language_services: Box::new([language_server_id!(tsserver)]) },
            ),
            (
                filetype!(typescript),
                LanguageConfig { language_services: Box::new([language_server_id!(tsserver)]) },
            ),
            (filetype!(text), LanguageConfig { language_services: Box::new([]) }),
            (filetype!(toml), LanguageConfig { language_services: Box::new([]) }),
            (filetype!(json), LanguageConfig { language_services: Box::new([]) }),
        ]);

        let language_servers = BTreeMap::from(
            [
                (
                    language_server_id!(rust_analyzer),
                    ExecutableLanguageServerConfig::new("ra-multiplex", []),
                    // ExecutableLanguageServerConfig::new("rust-analyzer", []),
                ),
                (
                    language_server_id!(tsserver),
                    ExecutableLanguageServerConfig::new(
                        "typescript-language-server",
                        ["--stdio".into()],
                    ),
                ),
                (language_server_id!(gopls), ExecutableLanguageServerConfig::new("gopls", [])),
                (language_server_id!(gqlt), ExecutableLanguageServerConfig::new("gqlt", [])),
                (language_server_id!(clangd), ExecutableLanguageServerConfig::new("clangd", [])),
            ]
            .map(|(k, v)| (k, Box::new(v) as _)),
        );

        Self::new(languages, language_servers).expect("invalid default config")
    }
}

#[derive(Debug, Default)]
pub struct LanguageConfig {
    pub(crate) language_services: Box<[LanguageServiceId]>,
}

impl LanguageConfig {
    pub fn new(language_servers: impl IntoIterator<Item = LanguageServiceId>) -> Self {
        Self { language_services: language_servers.into_iter().collect() }
    }
}

#[derive(Debug)]
pub struct ExecutableLanguageServerConfig {
    pub command: OsString,
    pub args: Box<[OsString]>,
}

impl ExecutableLanguageServerConfig {
    fn new(command: impl Into<OsString>, args: impl IntoIterator<Item = OsString>) -> Self {
        Self { command: command.into(), args: args.into_iter().collect() }
    }
}

impl LanguageServerConfig for ExecutableLanguageServerConfig {
    fn spawn(
        &self,
        cwd: &Path,
        client: LanguageClient,
    ) -> zi_lsp::Result<(
        Box<dyn DerefMut<Target = zi_lsp::DynLanguageServer> + Send>,
        BoxFuture<'static, zi_lsp::Result<()>>,
    )> {
        tracing::debug!(command = ?self.command, args = ?self.args, "spawn language server");
        let (server, fut) = zi_lsp::Server::start(client, cwd, &self.command, &self.args[..])?;
        Ok((Box::new(server), Box::pin(fut)))
    }
}
