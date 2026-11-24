//! The dependencies a project declares, read from the manifests at the top of a source tree. Each file is read up to
//! [`MAX_MANIFEST`] bytes and a manifest that does not parse names nothing.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// A manifest is read up to this many bytes.
pub const MAX_MANIFEST: u64 = 1024 * 1024;

pub fn read_text(path: &Path) -> Option<String> {
    let mut text = String::new();
    File::open(path)
        .ok()?
        .take(MAX_MANIFEST)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// `dependencies` and `devDependencies` of a `package.json`.
fn package_json(text: &str) -> Vec<String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    ["dependencies", "devDependencies"]
        .iter()
        .filter_map(|key| json.get(key)?.as_object())
        .flat_map(|table| table.keys().cloned())
        .collect()
}

/// The keys of the dependency tables of a `Cargo.toml`, and the `[dependencies.name]` tables.
fn cargo_toml(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_table = false;
    for line in text.lines().map(str::trim) {
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_end_matches(']');
            let named = header.rsplit_once("dependencies.");
            names.extend(named.map(|(_, name)| name.trim_matches('"').to_string()));
            in_table = named.is_none() && header.ends_with("dependencies");
        } else if in_table && !line.starts_with('#') {
            let key = line.split(['=', '.']).next().unwrap_or("").trim();
            names.extend(Some(key).filter(|k| !k.is_empty()).map(String::from));
        }
    }
    names
}

/// The modules a `go.mod` requires, in one `require` line or in a block.
fn go_mod(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_block = false;
    for line in text.lines().map(str::trim) {
        let mut words = line.split_whitespace();
        match (in_block, words.next(), words.next()) {
            (_, Some("require"), Some("(")) => in_block = true,
            (_, Some(")"), _) => in_block = false,
            (false, Some("require"), Some(module)) => names.push(module.to_string()),
            (true, Some(module), _) if !module.starts_with("//") => names.push(module.to_string()),
            _ => {}
        }
    }
    names
}

/// A manifest format: the names a text of it declares.
type Parser = fn(&str) -> Vec<String>;

/// What the manifests directly in `dir` declare.
pub fn dependencies(dir: &Path) -> Vec<String> {
    let parsers: [(&str, Parser); 3] = [
        ("package.json", package_json),
        ("Cargo.toml", cargo_toml),
        ("go.mod", go_mod),
    ];
    parsers
        .iter()
        .filter_map(|(name, parse)| Some(parse(&read_text(&dir.join(name))?)))
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_json_names_both_dependency_tables() {
        let text = r#"{"name":"x","dependencies":{"lodash":"1"},"devDependencies":{"jest":"2"},"other":{"a":1}}"#;
        assert_eq!(package_json(text), ["lodash", "jest"]);
        assert!(package_json("{ not json").is_empty());
    }

    #[test]
    fn cargo_toml_names_tables_and_named_tables() {
        let text = "[package]\nname = \"x\"\n[dependencies]\nserde = \"1\"\n# a note\nobject.workspace = true\n\n[dev-dependencies]\nrstest = \"0.28\"\n[dependencies.\"tokio\"]\nversion = \"1\"\n[profile.release]\nlto = \"thin\"\n";
        assert_eq!(cargo_toml(text), ["serde", "object", "rstest", "tokio"]);
    }

    #[test]
    fn go_mod_names_single_and_block_requires() {
        let text = "module x\nrequire a.test/one v1\nrequire (\n\t// note\n\tb.test/two v2\n)\nreplace c => d\n";
        assert_eq!(go_mod(text), ["a.test/one", "b.test/two"]);
    }
}
