use std::path::Path;

use crate::build::plugin_name;
use crate::language::TEMPLATE_NAME;
use crate::process::tool;
use crate::{Language, PluginName, scaffold};

fn source(lang: Language) -> &'static Path {
    Path::new(match lang {
        Language::TypeScript => "src/plugin.ts",
        Language::JavaScript => "src/plugin.js",
        Language::Rust => "src/lib.rs",
    })
}

#[test]
fn detect_language() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Language::detect(dir.path()).unwrap_err().to_string().contains("not a plugin"));

    for lang in Language::ALL {
        let project = dir.path().join(format!("{lang:?}"));
        scaffold(&project, &"p".parse().unwrap(), lang).unwrap();
        assert_eq!(Language::detect(&project).unwrap(), lang);
    }

    let both = dir.path().join("both");
    scaffold(&both, &"p".parse().unwrap(), Language::TypeScript).unwrap();
    std::fs::write(both.join("src/plugin.js"), "").unwrap();
    assert!(Language::detect(&both).unwrap_err().to_string().contains("more than one"));
}

#[test]
fn scaffold_names_the_plugin() {
    let dir = tempfile::tempdir().unwrap();
    for lang in Language::ALL {
        let project = dir.path().join(format!("{lang:?}"));
        scaffold(&project, &"my-plugin".parse().unwrap(), lang).unwrap();
        assert_eq!(plugin_name(&project, lang).unwrap().to_string(), "my-plugin");
        let source = std::fs::read_to_string(project.join(source(lang))).unwrap();
        assert!(source.contains(r#""my-plugin""#), "{source}");
        assert!(!source.contains(TEMPLATE_NAME));
    }
}

#[test]
fn tools_resolve_from_a_relative_project() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("project/node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("jco"), "").unwrap();
    std::env::set_current_dir(root.path()).unwrap();

    let cmd = tool(Path::new("project"), "jco").unwrap();
    assert_eq!(Path::new(cmd.get_program()), bin.canonicalize().unwrap().join("jco"));
}

#[test]
fn plugin_names() {
    for name in ["zi", "my-plugin", "a1"] {
        assert_eq!(name.parse::<PluginName>().unwrap().to_string(), name);
    }
    for name in ["", "My-Plugin", "1a", "-a", "a_b", "a/b", "@scope/a"] {
        assert!(name.parse::<PluginName>().is_err(), "{name}");
    }
}
