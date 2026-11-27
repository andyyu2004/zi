use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static DIRS: OnceLock<Dirs> = OnceLock::new();

struct Dirs {
    grammar_dir: PathBuf,
    plugin_dirs: Vec<PathBuf>,
    config_dir: PathBuf,
}

fn dirs() -> &'static Dirs {
    DIRS.get_or_init(|| {
        let data = xdg_dir("XDG_DATA_HOME", ".local/share").join("zi");

        let grammar_dir = match std::env::var_os("ZI_GRAMMAR_DIR") {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => data.join("grammars"),
        };
        let config_dir = xdg_dir("XDG_CONFIG_HOME", ".config").join("zi");

        if !grammar_dir.exists() {
            std::fs::create_dir_all(&grammar_dir).expect("couldn't create grammar directory");
        }

        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir).expect("couldn't create config directory");
        }

        let mut plugin_dirs: Vec<_> = std::env::var_os("ZI_PLUGIN_PATH")
            .iter()
            .flat_map(std::env::split_paths)
            .filter(|path| !path.as_os_str().is_empty())
            .collect();
        if plugin_dirs.is_empty() {
            let plugin_dir = std::env::var_os("ZI_PLUGIN_DIR")
                .filter(|dir| !dir.is_empty())
                .map_or_else(|| data.join("plugins"), PathBuf::from);
            let plugin_dir =
                std::path::absolute(plugin_dir).expect("couldn't resolve plugin directory");
            if !plugin_dir.is_dir() {
                std::fs::create_dir_all(&plugin_dir).expect("couldn't create plugin directory");
            }
            plugin_dirs.push(plugin_dir);
        }

        Dirs { grammar_dir, plugin_dirs, config_dir }
    })
}

fn xdg_dir(var: &str, default: &str) -> PathBuf {
    xdg_dir_from(std::env::var_os(var).map(PathBuf::from), std::env::home_dir(), default)
}

fn xdg_dir_from(var: Option<PathBuf>, home: Option<PathBuf>, default: &str) -> PathBuf {
    match var {
        Some(dir) if dir.is_absolute() => dir,
        _ => home.expect("couldn't find the home directory").join(default),
    }
}

pub fn grammar() -> &'static Path {
    &dirs().grammar_dir
}

pub fn plugin() -> impl Iterator<Item = &'static Path> {
    dirs().plugin_dirs.iter().map(std::path::PathBuf::as_path)
}

pub fn config() -> &'static Path {
    &dirs().config_dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_dirs() {
        let home = Some(PathBuf::from("/home/u"));
        assert_eq!(
            xdg_dir_from(Some("/xdg/data".into()), home.clone(), ".local/share"),
            Path::new("/xdg/data")
        );
        assert_eq!(
            xdg_dir_from(None, home.clone(), ".local/share"),
            Path::new("/home/u/.local/share")
        );
        assert_eq!(
            xdg_dir_from(Some("relative".into()), home.clone(), ".config"),
            Path::new("/home/u/.config")
        );
        assert_eq!(xdg_dir_from(Some("".into()), home, ".config"), Path::new("/home/u/.config"));
    }
}
