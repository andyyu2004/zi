use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static DIRS: OnceLock<Dirs> = OnceLock::new();

struct Dirs {
    grammar_dir: PathBuf,
    plugin_dirs: &'static [PathBuf],
    config_dir: PathBuf,
}

fn dirs() -> &'static Dirs {
    DIRS.get_or_init(|| {
        let dirs = directories_next::BaseDirs::new().expect("couldn't retrieve home directory");
        let data = dirs.data_dir().join("zi");

        let grammar_dir = data.join("grammars");
        let plugin_dir = data.join("plugins");
        let config_dir = dirs.config_dir().join("zi");

        if !grammar_dir.exists() {
            std::fs::create_dir_all(&grammar_dir).expect("couldn't create grammar directory");
        }

        if !plugin_dir.exists() {
            std::fs::create_dir_all(&plugin_dir).expect("couldn't create plugin directory");
        }

        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir).expect("couldn't create config directory");
        }

        let plugin_path = std::env::var("ZI_PLUGIN_PATH").ok().unwrap_or_else(String::new);
        let plugin_dirs = Box::leak(plugin_path.split(':').map(PathBuf::from).collect::<Box<_>>());

        Dirs { grammar_dir, plugin_dirs, config_dir }
    })
}

pub fn grammar() -> &'static Path {
    &dirs().grammar_dir
}

pub fn plugin() -> impl Iterator<Item = &'static Path> {
    dirs().plugin_dirs.iter().map(|path| path.as_path())
}

pub fn config() -> &'static Path {
    &dirs().config_dir
}
