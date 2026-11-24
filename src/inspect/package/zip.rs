//! A ZIP archive (APK, AAB, JAR, AAR, IPA, plain ZIP) listed from its central directory. No entry is decompressed, so
//! an archive that claims gigabytes costs nothing; every offset and count is checked against the file.

use std::collections::BTreeSet;

use super::{bytes, dependencies, files, kind_of, node_modules_names};
use crate::inspect::image::{Budget, Image};

/// The end-of-central-directory record is within this many bytes of the end.
const EOCD_WINDOW: usize = 65_557;
/// The most central directory records read.
const MAX_ENTRIES: usize = 100_000;
const EOCD: &[u8] = b"PK\x05\x06";
const LOCATOR64: &[u8] = b"PK\x06\x07";
const EOCD64: &[u8] = b"PK\x06\x06";
const RECORD: &[u8] = b"PK\x01\x02";
/// A 16 or 32 bit field holding this value says the real one is in the ZIP64 record.
const MAX_16: u64 = 0xffff;
const MAX_32: u64 = 0xffff_ffff;

/// One file of the archive.
struct Entry {
    path: String,
    size: u64,
}

fn le(data: &[u8], at: usize, len: usize) -> Option<u64> {
    let bytes = bytes(data, at, len)?;
    Some(bytes.iter().rev().fold(0, |n, &b| (n << 8) | u64::from(b)))
}

/// The count and offset of the ZIP64 end record that the locator before `eocd` points to.
fn directory64(data: &[u8], eocd: usize) -> Option<(u64, u64)> {
    let locator = eocd
        .checked_sub(20)
        .filter(|&at| bytes(data, at, 4) == Some(LOCATOR64))?;
    let at = usize::try_from(le(data, locator + 8, 8)?).ok()?;
    if bytes(data, at, 4) != Some(EOCD64) {
        return None;
    }
    Some((le(data, at + 32, 8)?, le(data, at + 48, 8)?))
}

/// Where the central directory is and how many records it holds: from the ZIP64 record when the EOCD points to one.
fn directory(data: &[u8]) -> Option<(usize, usize)> {
    let start = data.len().saturating_sub(EOCD_WINDOW);
    let eocd = start + data[start..].windows(4).rposition(|w| w == EOCD)?;
    let (count, offset) = (le(data, eocd + 10, 2)?, le(data, eocd + 16, 4)?);
    let (count, offset) = if count == MAX_16 || offset == MAX_32 {
        directory64(data, eocd)?
    } else {
        (count, offset)
    };
    Some((usize::try_from(offset).ok()?, usize::try_from(count).ok()?))
}

/// The size from a ZIP64 extra field, when the 32 bit one says to look there.
fn size64(extra: &[u8]) -> Option<u64> {
    let mut at = 0;
    while at + 4 <= extra.len() {
        let (id, len) = (
            le(extra, at, 2)?,
            usize::try_from(le(extra, at + 2, 2)?).ok()?,
        );
        if id == 1 {
            return le(extra, at + 4, 8);
        }
        at += 4 + len;
    }
    None
}

/// The record at `at` and where the next one starts.
fn record(data: &[u8], at: usize) -> Option<(Entry, usize)> {
    if bytes(data, at, 4) != Some(RECORD) {
        return None;
    }
    let size = le(data, at + 24, 4)?;
    let (name, extra, comment) = (
        le(data, at + 28, 2)?,
        le(data, at + 30, 2)?,
        le(data, at + 32, 2)?,
    );
    let (name, extra, comment) = (name as usize, extra as usize, comment as usize);
    let path = String::from_utf8_lossy(bytes(data, at + 46, name)?).into_owned();
    let extra = bytes(data, at + 46 + name, extra)?;
    let size = if size == MAX_32 {
        size64(extra).unwrap_or(size)
    } else {
        size
    };
    Some((Entry { path, size }, at + 46 + name + extra.len() + comment))
}

/// The files of the archive, in the order of the central directory; directories are left out.
fn entries(data: &[u8], budget: &mut Budget) -> Option<Vec<Entry>> {
    let (mut at, count) = directory(data)?;
    budget.cut(count > MAX_ENTRIES);
    let mut list = Vec::new();
    for _ in 0..count.min(MAX_ENTRIES) {
        let Some((entry, next)) = record(data, at) else {
            break;
        };
        at = next;
        if !entry.path.ends_with('/') {
            list.push(entry);
        }
    }
    Some(list)
}

/// APK, AAB, AAR, IPA, JAR or plain ZIP, from what the entries are.
fn package(paths: &[&str]) -> &'static str {
    let has = |path: &str| paths.contains(&path);
    if paths
        .iter()
        .any(|p| p.starts_with("Payload/") && p.contains(".app/"))
    {
        "ipa"
    } else if has("BundleConfig.pb") {
        "aab"
    } else if has("AndroidManifest.xml") && has("classes.jar") {
        "aar"
    } else if has("AndroidManifest.xml") {
        "apk"
    } else if has("META-INF/MANIFEST.MF") {
        "jar"
    } else {
        "zip"
    }
}

/// The ABIs of the native libraries of an APK: `lib/<abi>/...`.
fn abis(paths: &[&str]) -> String {
    let abis: BTreeSet<&str> = paths
        .iter()
        .filter_map(|p| p.strip_prefix("lib/")?.split('/').next())
        .collect();
    abis.into_iter().collect::<Vec<_>>().join(",")
}

/// Manifests first, then native code, bytecode, scripts, sources and the rest, so a small budget keeps the files an
/// analyst opens first.
fn priority(kind: &str) -> usize {
    ["manifest", "native", "bytecode", "script", "source"]
        .iter()
        .position(|k| *k == kind)
        .unwrap_or(5)
}

pub fn inspect(data: &[u8], budget: &mut Budget) -> Option<Image> {
    let list = entries(data, budget)?;
    let paths: Vec<&str> = list.iter().map(|e| e.path.as_str()).collect();
    let manifest = paths
        .iter()
        .filter(|p| kind_of(p) == "manifest")
        .min_by_key(|p| p.matches('/').count())
        .map(|p| (*p).to_string());
    let notes = budget.notes([
        ("package", Some(package(&paths).to_string())),
        ("manifest", manifest),
        ("entries", Some(list.len().to_string())),
        (
            "architectures",
            Some(abis(&paths)).filter(|a| !a.is_empty()),
        ),
    ]);
    let names = node_modules_names(paths.iter().copied());
    let mut listed: Vec<(String, u64, &'static str)> = list
        .iter()
        .map(|e| (e.path.clone(), e.size, kind_of(&e.path)))
        .collect();
    listed.sort_by_key(|(_, _, kind)| priority(kind));
    Some(Image {
        kind: "package",
        format: "zip",
        files: files(budget, listed),
        libraries: dependencies(budget, names),
        notes,
        ..Image::default()
    })
}
