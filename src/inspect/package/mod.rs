//! Packages, bundles and source trees: targets that hold files instead of code sections. Nothing is decompressed or
//! executed and no symbolic link is followed; what is listed is what the headers and the directory entries say.

use std::collections::BTreeSet;

use super::image::{Budget, FileEntry, Library};

pub mod asar;
pub mod js;
mod manifest;
pub mod tree;
pub mod zip;

#[cfg(test)]
pub mod fixture;
#[cfg(test)]
mod tests;

/// What a listed file costs besides its path: its size, kind and the JSON around them.
const FILE_COST: usize = 40;
/// The most dependencies named.
const MAX_DEPENDENCIES: usize = 200;
/// Files with these names are manifests, wherever they are.
const MANIFESTS: [&str; 9] = [
    "AndroidManifest.xml",
    "MANIFEST.MF",
    "Info.plist",
    "package.json",
    "Cargo.toml",
    "go.mod",
    "pom.xml",
    "pyproject.toml",
    "build.gradle",
];
/// Extensions by kind; anything else is a resource.
const KINDS: [(&str, &[&str]); 4] = [
    ("bytecode", &["dex", "class", "pyc", "wasm", "jar"]),
    ("native", &["so", "dylib", "dll", "exe", "node", "a", "o"]),
    ("script", &["js", "mjs", "cjs", "sh", "ps1", "bat"]),
    (
        "source",
        &[
            "rs", "go", "c", "h", "cc", "cpp", "hpp", "java", "kt", "swift", "ts", "tsx", "jsx",
            "py", "rb", "cs", "php", "m", "lua", "scala",
        ],
    ),
];

/// `len` bytes of `data` from `at`, or nothing when they are not all there; offsets read from a file may be anything.
pub fn bytes(data: &[u8], at: usize, len: usize) -> Option<&[u8]> {
    data.get(at..at.checked_add(len)?)
}

/// The kind of a file from its path: `manifest`, `bytecode`, `native`, `script`, `source` or `resource`.
pub fn kind_of(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    if MANIFESTS.contains(&name) {
        return "manifest";
    }
    let ext = name
        .rsplit_once('.')
        .map_or(String::new(), |(_, e)| e.to_ascii_lowercase());
    KINDS
        .iter()
        .find(|(_, exts)| exts.contains(&ext.as_str()))
        .map_or("resource", |(kind, _)| kind)
}

/// Files with their sizes and kinds, as many as the budget pays for.
pub fn files(
    budget: &mut Budget,
    entries: impl IntoIterator<Item = (String, u64, &'static str)>,
) -> Vec<FileEntry> {
    entries
        .into_iter()
        .map_while(|(path, size, kind)| {
            Some(FileEntry {
                path: budget.take_costing(path.as_bytes(), FILE_COST + kind.len())?,
                size,
                kind: kind.to_string(),
            })
        })
        .collect()
}

/// Dependencies as libraries: sorted, without repeats, at most [`MAX_DEPENDENCIES`], as the budget allows.
pub fn dependencies(budget: &mut Budget, names: impl IntoIterator<Item = String>) -> Vec<Library> {
    let unique: BTreeSet<String> = names.into_iter().collect();
    budget.cut(unique.len() > MAX_DEPENDENCIES);
    let raw = unique.into_iter().take(MAX_DEPENDENCIES);
    budget.libraries(raw.map(|name| (name, Vec::new())).collect())
}

/// The package name that follows a `node_modules` directory in `rest`: `name`, or `@scope/name`.
fn package_name(rest: &[&str]) -> Option<String> {
    let first = rest
        .first()
        .filter(|name| !name.is_empty() && !name.starts_with('.'))?;
    if first.starts_with('@') {
        Some(format!("{first}/{}", rest.get(1)?))
    } else {
        Some((*first).to_string())
    }
}

/// The packages a list of paths keeps under `node_modules/`, nested ones included.
pub fn node_modules_names<'a>(paths: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for path in paths {
        let parts: Vec<&str> = path.split('/').collect();
        for (at, part) in parts.iter().enumerate() {
            if *part == "node_modules" && names.len() <= MAX_DEPENDENCIES {
                names.extend(package_name(&parts[at + 1..]));
            }
        }
    }
    names
}
