use std::path::Path;

use anyhow::bail;

pub(crate) const TEMPLATE_NAME: &str = "zi-plugin-template";

const NPM_TEMPLATE: &[(&str, &str)] = &[
    ("package.json", include_str!("../template/common/package.json")),
    ("package-lock.json", include_str!("../template/common/package-lock.json")),
    (".gitignore", include_str!("../template/common/.gitignore")),
];

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
    pub(crate) const ALL: [Self; 3] = [Self::TypeScript, Self::JavaScript, Self::Rust];

    fn marker(self) -> &'static Path {
        Path::new(match self {
            Self::TypeScript => "src/plugin.ts",
            Self::JavaScript => "src/plugin.js",
            Self::Rust => "Cargo.toml",
        })
    }

    pub(crate) fn template(self) -> Vec<(&'static str, &'static str)> {
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

    fn markers(langs: impl IntoIterator<Item = Self>) -> String {
        langs
            .into_iter()
            .map(|lang| lang.marker().display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
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
                Self::markers(Self::ALL)
            ),
            _ => bail!(
                "`{}` looks like more than one kind of plugin project: {}",
                project.display(),
                Self::markers(found)
            ),
        }
    }
}
