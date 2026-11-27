use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

pub fn find_external(arg: &OsStr, path_var: Option<&OsStr>) -> Option<PathBuf> {
    let name = arg.to_str()?;
    let is_word = name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !is_word || Path::new(name).exists() {
        return None;
    }

    std::env::split_paths(path_var?)
        .map(|dir| dir.join(format!("zi-{name}")))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    path.metadata().is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;

    use super::*;

    fn bin_dir(names: &[&str]) -> (tempfile::TempDir, OsString) {
        let dir = tempfile::tempdir().unwrap();
        for name in names {
            let path = dir.path().join(name);
            fs::write(&path, "#!/bin/sh\n").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path_var = std::env::join_paths([dir.path()]).unwrap();
        (dir, path_var)
    }

    #[test]
    fn finds_zi_prefixed_executable_on_path() {
        let (dir, path_var) = bin_dir(&["zi-plugin"]);
        assert_eq!(
            find_external(OsStr::new("plugin"), Some(&path_var)),
            Some(dir.path().join("zi-plugin"))
        );
        assert_eq!(find_external(OsStr::new("pkg"), Some(&path_var)), None);
        assert_eq!(find_external(OsStr::new("plugin"), None), None);
    }

    #[test]
    fn non_executable_is_skipped() {
        let (dir, path_var) = bin_dir(&[]);
        fs::write(dir.path().join("zi-plugin"), "").unwrap();
        assert_eq!(find_external(OsStr::new("plugin"), Some(&path_var)), None);
    }

    #[test]
    fn paths_and_flags_are_not_subcommands() {
        let (_dir, path_var) = bin_dir(&["zi-src", "zi-Plugin"]);
        for arg in ["src", "./plugin", "--log", "foo.rs", "Plugin"] {
            assert_eq!(find_external(OsStr::new(arg), Some(&path_var)), None, "{arg}");
        }
    }
}
