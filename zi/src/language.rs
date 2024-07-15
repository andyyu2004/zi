use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::bail;
use ustr::{ustr, Ustr};
use zi_language_service::LanguageServiceConfig;

use crate::{Editor, Result};

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

impl From<&str> for LanguageServiceId {
    fn from(id: &str) -> Self {
        Self(ustr(id))
    }
}

impl LanguageServiceId {
    pub fn new(id: impl Into<Ustr>) -> Self {
        Self(id.into())
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

#[derive(Default)]
pub struct Config {
    pub(crate) languages: BTreeMap<FileType, LanguageConfig>,
    pub(crate) language_services:
        BTreeMap<LanguageServiceId, Box<dyn LanguageServiceConfig<Editor> + Send>>,
}

impl Config {
    pub fn new(
        languages: BTreeMap<FileType, LanguageConfig>,
        language_servers: BTreeMap<
            LanguageServiceId,
            Box<dyn LanguageServiceConfig<Editor> + Send>,
        >,
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
        config: impl LanguageServiceConfig<Editor> + Send + 'static,
    ) -> &mut Self {
        self.language_services.insert(id.into(), Box::new(config));
        self
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
