//! An Electron ASAR archive listed from its JSON header. File contents are read only for the root `package.json`,
//! and only when it lies inside the file and is small.

use std::collections::VecDeque;

use serde_json::Value;

use super::manifest::MAX_MANIFEST;
use super::{bytes, dependencies, files, kind_of, node_modules_names};
use crate::inspect::image::{Budget, Image};

/// The header is JSON; a longer one is not read.
const MAX_HEADER: usize = 16 * 1024 * 1024;
/// Directories nested deeper than this are not entered.
const MAX_DEPTH: usize = 32;
/// The most entries read.
const MAX_ENTRIES: usize = 100_000;

#[derive(Default)]
struct Tally {
    files: Vec<(String, u64, &'static str)>,
    with_integrity: usize,
    unpacked: usize,
    algorithm: Option<String>,
    cut: bool,
}

impl Tally {
    fn add(&mut self, path: String, node: &Value) {
        let size = node.get("size").and_then(Value::as_u64).unwrap_or(0);
        let kind = if node.get("link").is_some() {
            "link"
        } else {
            kind_of(&path)
        };
        if let Some(integrity) = node.get("integrity") {
            self.with_integrity += 1;
            let algorithm = integrity.get("algorithm").and_then(Value::as_str);
            self.algorithm = self.algorithm.take().or(algorithm.map(str::to_lowercase));
        }
        self.unpacked += usize::from(node.get("unpacked") == Some(&Value::Bool(true)));
        self.files.push((path, size, kind));
    }

    fn integrity(&self) -> &str {
        match (self.with_integrity, self.files.len()) {
            (0, _) => "none",
            (n, total) if n == total => self.algorithm.as_deref().unwrap_or("present"),
            _ => "partial",
        }
    }
}

type Queue<'a> = VecDeque<(String, &'a Value, usize)>;

impl Tally {
    /// Lists the files of `dir` and queues its directories; false once the cap is reached.
    fn read_dir<'a>(
        &mut self,
        (prefix, dir, depth): (String, &'a Value, usize),
        queue: &mut Queue<'a>,
    ) -> bool {
        let Some(children) = dir.get("files").and_then(Value::as_object) else {
            return true;
        };
        for (name, node) in children {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if self.files.len() >= MAX_ENTRIES {
                self.cut = true;
                return false;
            }
            match (node.get("files"), depth < MAX_DEPTH) {
                (None, _) => self.add(path, node),
                (Some(_), true) => queue.push_back((path, node, depth + 1)),
                (Some(_), false) => self.cut = true,
            }
        }
        true
    }
}

/// Every file of the tree under `root`, directories first level by level.
fn walk(root: &Value) -> Tally {
    let mut tally = Tally::default();
    let mut queue: Queue = VecDeque::from([(String::new(), root, 0)]);
    while let Some(dir) = queue.pop_front() {
        if !tally.read_dir(dir, &mut queue) {
            break;
        }
    }
    tally
}

/// The header and where the file contents start, when the file is an ASAR archive.
fn header(data: &[u8]) -> Option<(Value, usize)> {
    let u32_at =
        |at: usize| Some(u32::from_le_bytes(bytes(data, at, 4)?.try_into().ok()?) as usize);
    let (pickle, len) = (u32_at(4)?, u32_at(12)?);
    let json = bytes(data, 16, len.min(MAX_HEADER + 1))?;
    let json = (len <= MAX_HEADER).then_some(json)?;
    Some((
        serde_json::from_slice(json).ok()?,
        8usize.checked_add(pickle)?,
    ))
}

/// The bytes of the root `package.json`, when its record is sound and the text lies inside the file.
fn package_bytes<'a>(root: &Value, data: &'a [u8], contents: usize) -> Option<&'a [u8]> {
    let node = root.pointer("/files/package.json")?;
    let offset = node.get("offset")?.as_str()?.parse::<usize>().ok()?;
    let size = usize::try_from(node.get("size")?.as_u64()?).ok()?;
    if size as u64 > MAX_MANIFEST {
        return None;
    }
    bytes(data, contents.checked_add(offset)?, size)
}

/// The `name` and `main` of the root `package.json`.
fn package_json(root: &Value, data: &[u8], contents: usize) -> Vec<(&'static str, Option<String>)> {
    let json: Option<Value> =
        package_bytes(root, data, contents).and_then(|t| serde_json::from_slice(t).ok());
    let get = |key: &str| Some(json.as_ref()?.get(key)?.as_str()?.to_string());
    vec![("name", get("name")), ("main", get("main"))]
}

pub fn inspect(data: &[u8], budget: &mut Budget) -> Option<Image> {
    let (root, contents) = header(data)?;
    root.get("files")?.as_object()?;
    let tally = walk(&root);
    let mut pairs = package_json(&root, data, contents);
    pairs.push(("integrity", Some(tally.integrity().to_string())));
    pairs.push((
        "unpacked",
        (tally.unpacked > 0).then(|| tally.unpacked.to_string()),
    ));
    pairs.push(("entries", Some(tally.files.len().to_string())));
    let notes = budget.notes(pairs);
    let names = node_modules_names(tally.files.iter().map(|(p, _, _)| p.as_str()));
    budget.cut(tally.cut);
    let files = files(budget, tally.files);
    Some(Image {
        kind: "package",
        format: "asar",
        files,
        libraries: dependencies(budget, names),
        notes,
        ..Image::default()
    })
}
