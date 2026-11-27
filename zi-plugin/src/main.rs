use std::path::PathBuf;

use anyhow::Context as _;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;
use zi_plugin::{Language, PluginName};

#[derive(Parser)]
#[command(about = "Create, build and install zi plugins written in TypeScript, JavaScript or Rust")]
struct Opts {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a plugin project, install its npm toolchain (TypeScript, JavaScript) and generate its bindings.
    New {
        dir: PathBuf,
        /// Defaults to the directory's name.
        #[arg(long)]
        name: Option<PluginName>,
        #[arg(long, value_enum, default_value_t = Language::TypeScript)]
        lang: Language,
    },
    /// Regenerate zi's API bindings: `types/` (TypeScript, JavaScript) or `wit/` (Rust).
    Types {
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    /// Build the plugin into `dist/<name>.wasm`.
    Build {
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    /// Build the plugin and copy it into zi's plugin directory.
    Install {
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_env("ZI_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    match Opts::parse().command {
        Cmd::New { dir, name, lang } => {
            let name = match name {
                Some(name) => name,
                None => dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .context("can't take a plugin name from the directory, pass --name")?
                    .parse()?,
            };
            zi_plugin::scaffold(&dir, &name, lang)?;
            zi_plugin::install_toolchain(&dir)?;
            zi_plugin::types(&dir)?;
            tracing::info!(%name, dir = %dir.display(), "created plugin");
        }
        Cmd::Types { dir } => zi_plugin::types(&dir)?,
        Cmd::Build { dir } => {
            let component = zi_plugin::build(&dir)?;
            tracing::info!(path = %component.display(), "built plugin");
        }
        Cmd::Install { dir } => {
            let installed = zi_plugin::install(&dir)?;
            tracing::info!(path = %installed.display(), "installed plugin");
        }
    }
    Ok(())
}
