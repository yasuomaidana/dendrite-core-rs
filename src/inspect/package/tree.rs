//! A directory as a source tree: its first levels, breadth first, with the dependencies its manifests declare. A
//! symbolic link is listed as a `link` and never followed, so a loop ends where it starts.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use super::{dependencies, files, kind_of, manifest, node_modules_names};
use crate::inspect::image::{Budget, Image};

/// Directories deeper than this many levels below the root are not entered.
const MAX_DEPTH: usize = 4;
/// The most entries visited.
const MAX_ENTRIES: usize = 20_000;
/// Directories that hold generated or foreign files: named, never entered.
const SKIPPED: [&str; 5] = [".git", "target", "node_modules", ".venv", "__pycache__"];
/// The most extensions in the `languages` note.
const MAX_LANGUAGES: usize = 5;
/// The most packages read from one `node_modules`.
const MAX_MODULES: usize = 500;

type Listed = (String, u64, &'static str);

#[derive(Default)]
struct Walk {
    files: Vec<Listed>,
    skipped: BTreeSet<String>,
    modules: Vec<String>,
    visited: usize,
    cut: bool,
}

/// The package names directly in a `node_modules` directory, scoped ones as `@scope/name`.
fn module_names(dir: &Path) -> Vec<String> {
    let children = |path: &Path| -> Vec<String> {
        let Ok(read) = fs::read_dir(path) else {
            return Vec::new();
        };
        let names = read
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned());
        names.take(MAX_MODULES).collect()
    };
    let mut names = Vec::new();
    for name in children(dir) {
        if name.starts_with('@') {
            let scoped = children(&dir.join(&name)).into_iter();
            names.extend(scoped.map(|child| format!("{name}/{child}")));
        } else {
            names.push(name);
        }
    }
    names
}

impl Walk {
    fn add_entry(
        &mut self,
        dir: &Path,
        rel: String,
        entry: &fs::DirEntry,
        queue: &mut VecDeque<(PathBuf, String, usize)>,
        depth: usize,
    ) {
        let Ok(kind) = entry.file_type() else {
            return;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if kind.is_symlink() {
            self.files.push((rel, 0, "link"));
        } else if kind.is_file() {
            let size = entry.metadata().map_or(0, |m| m.len());
            self.files.push((rel.clone(), size, kind_of(&rel)));
        } else if SKIPPED.contains(&name.as_str()) {
            self.skipped.insert(rel);
            if name == "node_modules" {
                self.modules.extend(module_names(&dir.join(&name)));
            }
        } else if depth < MAX_DEPTH {
            queue.push_back((dir.join(&name), rel, depth + 1));
        } else {
            self.cut = true;
        }
    }

    fn visit(
        &mut self,
        dir: &Path,
        prefix: &str,
        depth: usize,
        queue: &mut VecDeque<(PathBuf, String, usize)>,
    ) {
        let Ok(read) = fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<fs::DirEntry> = read.flatten().collect();
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in &entries {
            self.visited += 1;
            if self.visited > MAX_ENTRIES {
                self.cut = true;
                return;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            self.add_entry(dir, rel, entry, queue, depth);
        }
    }
}

/// The files of the tree under `root`, breadth first.
fn walk(root: &Path) -> Walk {
    let mut walk = Walk::default();
    let mut queue = VecDeque::from([(root.to_path_buf(), String::new(), 0)]);
    while let Some((dir, prefix, depth)) = queue.pop_front() {
        walk.visit(&dir, &prefix, depth, &mut queue);
        if walk.visited > MAX_ENTRIES {
            break;
        }
    }
    walk
}

/// The most frequent extensions as `rs:40,toml:2`.
fn languages(files: &[Listed]) -> String {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (path, _, kind) in files {
        let ext = path
            .rsplit('/')
            .next()
            .and_then(|name| name.rsplit_once('.'));
        if let (Some((_, ext)), true) = (ext, *kind != "link") {
            *counts.entry(ext).or_default() += 1;
        }
    }
    let mut ranked: Vec<_> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let top: Vec<String> = ranked
        .iter()
        .take(MAX_LANGUAGES)
        .map(|(ext, n)| format!("{ext}:{n}"))
        .collect();
    top.join(",")
}

/// Describes the directory at `root`; an error only when the directory itself cannot be read.
pub fn inspect(root: &Path) -> Result<Image, String> {
    fs::read_dir(root).map_err(|e| e.to_string())?;
    let mut budget = Budget::default();
    let walk = walk(root);
    let skipped: Vec<&str> = walk.skipped.iter().map(String::as_str).collect();
    let notes = budget.notes([
        ("entries", Some(walk.files.len().to_string())),
        (
            "languages",
            Some(languages(&walk.files)).filter(|l| !l.is_empty()),
        ),
        ("skipped", Some(skipped.join(",")).filter(|s| !s.is_empty())),
    ]);
    let names: Vec<String> = manifest::dependencies(root)
        .into_iter()
        .chain(walk.modules.iter().cloned())
        .chain(node_modules_names(
            walk.files.iter().map(|(p, _, _)| p.as_str()),
        ))
        .collect();
    let files = files(&mut budget, walk.files);
    let libraries = dependencies(&mut budget, names);
    budget.cut(walk.cut);
    Ok(Image {
        kind: "source",
        format: "source-tree",
        files,
        libraries,
        notes,
        truncated: budget.truncated,
        ..Image::default()
    })
}
