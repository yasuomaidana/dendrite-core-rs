//! A JavaScript bundle: a script file that carries a bundler's runtime or is named like one. The libraries come from
//! the `node_modules/` paths and the license banners the bundler left in, the strings from the quoted literals.

use super::{dependencies, node_modules_names};
use crate::inspect::image::{Budget, Image};
use crate::inspect::scan::{ranked, MAX_SCAN};

/// How much of the start of a file decides whether it is text.
const HEAD: usize = 8 * 1024;
/// A line longer than this makes a file minified.
const MINIFIED_LINE: usize = 1_000;
/// Quoted literals shorter or longer than this are not strings worth reading.
const LITERAL: std::ops::RangeInclusive<usize> = 6..=120;
/// The most literals and names looked at.
const MAX_FOUND: usize = 4_000;
/// A name or path after a marker is read up to this many bytes.
const MAX_TOKEN: usize = 120;
const EXTENSIONS: [&str; 3] = ["js", "mjs", "cjs"];
/// A bundler and the text its output carries.
const BUNDLERS: [(&str, &[&str]); 4] = [
    ("webpack", &["__webpack_require__", "webpackChunk"]),
    ("esbuild", &["__commonJS(", "__toESM("]),
    ("parcel", &["parcelRequire"]),
    ("systemjs", &["System.register"]),
];

fn has(data: &[u8], needle: &str) -> bool {
    data.windows(needle.len()).any(|w| w == needle.as_bytes())
}

/// Text: no NUL byte and at least 95% of the first bytes printable (bytes of UTF-8 count).
fn is_text(data: &[u8]) -> bool {
    let head = &data[..data.len().min(HEAD)];
    let plain = head
        .iter()
        .filter(|&&b| matches!(b, b' '..=b'~' | b'\t' | b'\n' | b'\r' | 0x80..))
        .count();
    !head.is_empty() && !head.contains(&0) && plain * 100 >= head.len() * 95
}

fn bundler(data: &[u8]) -> Option<&'static str> {
    BUNDLERS
        .iter()
        .find(|(_, marks)| marks.iter().any(|m| has(data, m)))
        .map(|(name, _)| *name)
}

/// What follows each occurrence of `marker`: up to [`MAX_TOKEN`] bytes made of `allowed` bytes.
fn after<'a>(
    data: &'a [u8],
    marker: &'a str,
    allowed: fn(u8) -> bool,
) -> impl Iterator<Item = &'a [u8]> {
    let hits = data
        .windows(marker.len())
        .enumerate()
        .filter(move |(_, w)| *w == marker.as_bytes())
        .map(move |(at, _)| at + marker.len());
    hits.take(MAX_FOUND).map(move |from| {
        let rest = &data[from..data.len().min(from + MAX_TOKEN)];
        let len = rest.iter().take_while(|&&b| allowed(b)).count();
        &rest[..len]
    })
}

fn path_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'@' | b'/' | b'.' | b'_' | b'-')
}

/// The package a banner line names: `lodash v4.17.21 ...` gives `lodash` (a name and then a version).
fn banner(line: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(line);
    let mut words = text.split_whitespace();
    let (name, version) = (words.next()?, words.next()?);
    let numbered = version
        .trim_start_matches('v')
        .starts_with(|c: char| c.is_ascii_digit());
    let named = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '@');
    (named && numbered).then(|| name.to_string())
}

fn line_byte(b: u8) -> bool {
    b == b' ' || b.is_ascii_graphic()
}

/// Packages named by `node_modules/` paths and by the `/*! name 1.2.3` and `@license name 1.2.3` banners.
fn libraries(data: &[u8]) -> Vec<String> {
    let paths: Vec<String> = after(data, "node_modules/", path_byte)
        .map(|p| format!("node_modules/{}", String::from_utf8_lossy(p)))
        .collect();
    let mut names = node_modules_names(paths.iter().map(String::as_str));
    for marker in ["/*! ", "@license "] {
        names.extend(after(data, marker, line_byte).filter_map(banner));
    }
    names.into_iter().collect()
}

/// The text between a pair of quotes of the same kind on one line, when it is of a length worth reading.
fn literals(data: &[u8]) -> Vec<&[u8]> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < data.len() && found.len() < MAX_FOUND {
        let quote = data[at];
        let end = matches!(quote, b'"' | b'\'' | b'`')
            .then(|| {
                data[at + 1..data.len().min(at + 2 + LITERAL.end())]
                    .iter()
                    .position(|&b| b == quote || b == b'\n')
            })
            .flatten()
            .filter(|&len| data[at + 1 + len] == quote && LITERAL.contains(&len));
        match end {
            Some(len) => {
                found.push(&data[at + 1..at + 1 + len]);
                at += len + 2;
            }
            None => at += 1,
        }
    }
    found
}

/// The strings of a bundle: its quoted literals, URLs and paths first.
pub fn strings(data: &[u8], budget: &mut Budget) -> Vec<String> {
    let scanned = &data[..data.len().min(MAX_SCAN)];
    budget.cut(data.len() > MAX_SCAN);
    ranked(literals(scanned).into_iter(), budget)
}

fn sourcemap(data: &[u8]) -> Option<String> {
    let url = after(data, "sourceMappingURL=", |b| {
        b.is_ascii_graphic() && b != b'*' && b != b'\'' && b != b'"'
    })
    .last()?;
    Some(String::from_utf8_lossy(url).into_owned()).filter(|u| !u.is_empty())
}

/// `data` as a bundle, when it is text and has a bundler's mark or a script's name (`ext` is lowercase).
pub fn inspect(data: &[u8], ext: Option<&str>, budget: &mut Budget) -> Option<Image> {
    if !is_text(data) {
        return None;
    }
    let scanned = &data[..data.len().min(MAX_SCAN)];
    let made_by = bundler(scanned);
    if made_by.is_none() && !ext.is_some_and(|e| EXTENSIONS.contains(&e)) {
        return None;
    }
    let longest = scanned
        .split(|&b| b == b'\n')
        .map(<[u8]>::len)
        .max()
        .unwrap_or(0);
    let notes = budget.notes([
        ("bundler", made_by.map(String::from)),
        (
            "minified",
            (longest > MINIFIED_LINE).then(|| "true".to_string()),
        ),
        ("sourcemap", sourcemap(scanned)),
    ]);
    Some(Image {
        kind: "bundle",
        format: "js-bundle",
        libraries: dependencies(budget, libraries(scanned)),
        notes,
        ..Image::default()
    })
}
