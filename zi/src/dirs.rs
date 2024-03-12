use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static DIRS: OnceLock<Dirs> = OnceLock::new();

struct Dirs {
    grammar_dir: PathBuf,
}

fn dirs() -> &'static Dirs {
    DIRS.get_or_init(|| {
        let dirs = directories_next::BaseDirs::new().expect("couldn't retrieve home directory");
        Dirs { grammar_dir: dirs.data_dir().join("zi/grammars") }
    })
}

pub fn grammar() -> &'static Path {
    &dirs().grammar_dir
}
