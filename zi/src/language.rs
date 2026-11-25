use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::bail;
use ustr::{Ustr, ustr};

use crate::{LanguageServiceConfig, Result};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileType(Ustr);

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
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
    pub fsharp: FileType,
    pub go: FileType,
    pub haskell: FileType,
    pub toml: FileType,
    pub json: FileType,
    pub picker: FileType,
    pub explorer: FileType,
    pub zig: FileType,
    pub python: FileType,
    pub yaml: FileType,
    pub nix: FileType,
    pub bash: FileType,
    pub c_sharp: FileType,
    pub cmake: FileType,
    pub cpp: FileType,
    pub css: FileType,
    pub dart: FileType,
    pub diff: FileType,
    pub dockerfile: FileType,
    pub elixir: FileType,
    pub graphql: FileType,
    pub groovy: FileType,
    pub html: FileType,
    pub ini: FileType,
    pub java: FileType,
    pub kotlin: FileType,
    pub lua: FileType,
    pub make: FileType,
    pub markdown: FileType,
    pub objc: FileType,
    pub ocaml: FileType,
    pub perl: FileType,
    pub php: FileType,
    pub powershell: FileType,
    pub proto: FileType,
    pub r: FileType,
    pub ruby: FileType,
    pub scala: FileType,
    pub sql: FileType,
    pub svelte: FileType,
    pub swift: FileType,
    pub tsx: FileType,
    pub vue: FileType,
    pub xml: FileType,
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
            fsharp: ft("fsharp"),
            go: ft("go"),
            haskell: ft("haskell"),
            toml: ft("toml"),
            json: ft("json"),
            zig: ft("zig"),
            picker: ft("picker"),
            explorer: ft("explorer"),
            python: ft("python"),
            yaml: ft("yaml"),
            nix: ft("nix"),
            bash: ft("bash"),
            c_sharp: ft("c_sharp"),
            cmake: ft("cmake"),
            cpp: ft("cpp"),
            css: ft("css"),
            dart: ft("dart"),
            diff: ft("diff"),
            dockerfile: ft("dockerfile"),
            elixir: ft("elixir"),
            graphql: ft("graphql"),
            groovy: ft("groovy"),
            html: ft("html"),
            ini: ft("ini"),
            java: ft("java"),
            kotlin: ft("kotlin"),
            lua: ft("lua"),
            make: ft("make"),
            markdown: ft("markdown"),
            objc: ft("objc"),
            ocaml: ft("ocaml"),
            perl: ft("perl"),
            php: ft("php"),
            powershell: ft("powershell"),
            proto: ft("proto"),
            r: ft("r"),
            ruby: ft("ruby"),
            scala: ft("scala"),
            sql: ft("sql"),
            svelte: ft("svelte"),
            swift: ft("swift"),
            tsx: ft("tsx"),
            vue: ft("vue"),
            xml: ft("xml"),
        })
    }

    pub fn detect(path: &Path) -> Self {
        let filename = path.file_name().and_then(|name| name.to_str());
        let extension = path.extension().and_then(|ext| ext.to_str());
        match (filename, extension) {
            (Some(".bash_profile" | ".bashrc" | ".profile"), _) => filetype!(bash),
            (Some("CMakeLists.txt"), _) => filetype!(cmake),
            (Some("Containerfile" | "Dockerfile"), _) => filetype!(dockerfile),
            (Some("GNUmakefile" | "Makefile" | "makefile"), _) => filetype!(make),
            (Some("Gemfile" | "Rakefile"), _) => filetype!(ruby),
            (Some(name), _)
                if name.starts_with("Dockerfile.") || name.starts_with("Containerfile.") =>
            {
                filetype!(dockerfile)
            }
            (_, Some("bash" | "sh")) => filetype!(bash),
            (_, Some("c" | "h")) => filetype!(c),
            (_, Some("C" | "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx")) => filetype!(cpp),
            (_, Some("cmake")) => filetype!(cmake),
            (_, Some("cs")) => filetype!(c_sharp),
            (_, Some("css")) => filetype!(css),
            (_, Some("dart")) => filetype!(dart),
            (_, Some("diff" | "patch")) => filetype!(diff),
            (_, Some("dockerfile" | "containerfile")) => filetype!(dockerfile),
            (_, Some("ex" | "exs")) => filetype!(elixir),
            (_, Some("fs" | "fsi" | "fsx")) => filetype!(fsharp),
            (_, Some("go")) => filetype!(go),
            (_, Some("gql" | "graphql" | "graphqls")) => filetype!(graphql),
            (_, Some("gradle" | "groovy")) => filetype!(groovy),
            (_, Some("gqlt")) => filetype!(gqlt),
            (_, Some("hs")) => filetype!(haskell),
            (_, Some("htm" | "html")) => filetype!(html),
            (_, Some("ini")) => filetype!(ini),
            (_, Some("java")) => filetype!(java),
            (_, Some("cjs" | "js" | "jsx" | "mjs")) => filetype!(javascript),
            (_, Some("json")) => filetype!(json),
            (_, Some("kt" | "kts")) => filetype!(kotlin),
            (_, Some("lua")) => filetype!(lua),
            (_, Some("mk")) => filetype!(make),
            (_, Some("m")) => filetype!(objc),
            (_, Some("markdown" | "md")) => filetype!(markdown),
            (_, Some("nix")) => filetype!(nix),
            (_, Some("ml")) => filetype!(ocaml),
            (_, Some("pl" | "pm")) => filetype!(perl),
            (_, Some("php")) => filetype!(php),
            (_, Some("proto")) => filetype!(proto),
            (_, Some("ps1" | "psd1" | "psm1")) => filetype!(powershell),
            (_, Some("py" | "pyi")) => filetype!(python),
            (_, Some("R" | "r")) => filetype!(r),
            (_, Some("rb")) => filetype!(ruby),
            (_, Some("rs")) => filetype!(rust),
            (_, Some("scala" | "sc")) => filetype!(scala),
            (_, Some("sql")) => filetype!(sql),
            (_, Some("svelte")) => filetype!(svelte),
            (_, Some("swift")) => filetype!(swift),
            (_, Some("toml")) => filetype!(toml),
            (_, Some("cts" | "mts" | "ts")) => filetype!(typescript),
            (_, Some("tsx")) => filetype!(tsx),
            (_, Some("vue")) => filetype!(vue),
            (_, Some("xml" | "xsd" | "xsl" | "xslt")) => filetype!(xml),
            (_, Some("yaml" | "yml")) => filetype!(yaml),
            (_, Some("zig")) => filetype!(zig),
            _ => filetype!(text),
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
    pub languages: BTreeMap<FileType, LanguageConfig>,
    pub(crate) language_services:
        BTreeMap<LanguageServiceId, Box<dyn LanguageServiceConfig + Send>>,
}

impl Config {
    pub fn new(
        languages: BTreeMap<FileType, LanguageConfig>,
        language_servers: BTreeMap<LanguageServiceId, Box<dyn LanguageServiceConfig + Send>>,
    ) -> Result<Self> {
        for (lang, config) in &languages {
            for server in &*config.language_services {
                if !language_servers.contains_key(server) {
                    bail!("language server `{server}` for language `{lang}` is not defined")
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

    pub fn add_language_service(
        &mut self,
        id: impl Into<LanguageServiceId>,
        config: impl LanguageServiceConfig + Send + 'static,
    ) -> &mut Self {
        self.language_services.insert(id.into(), Box::new(config));
        self
    }
}

#[derive(Debug, Default)]
pub struct LanguageConfig {
    pub language_services: Box<[LanguageServiceId]>,
}

impl LanguageConfig {
    pub fn new(language_servers: impl IntoIterator<Item = LanguageServiceId>) -> Self {
        Self { language_services: language_servers.into_iter().collect() }
    }
}
