//! Hand-built archives for the tests: a ZIP (with an optional ZIP64 end record) and an ASAR.

use serde_json::{json, Value};

fn le(out: &mut Vec<u8>, value: u64, size: usize) {
    out.extend(&value.to_le_bytes()[..size]);
}

/// The local file headers; the offset of each is returned.
fn local_headers(out: &mut Vec<u8>, entries: &[(&str, u64)]) -> Vec<u64> {
    let mut offsets = Vec::new();
    for (name, _) in entries {
        offsets.push(out.len() as u64);
        out.extend(b"PK\x03\x04");
        out.extend([
            20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        le(out, name.len() as u64, 2);
        out.extend([0, 0]);
        out.extend(name.as_bytes());
    }
    offsets
}

/// One central directory record; a `big` size is 0xffffffff with the real one in a ZIP64 extra field.
fn central_record(out: &mut Vec<u8>, (name, size): (&str, u64), offset: u64, zip64: bool) {
    let big = zip64 || size >= 0xffff_ffff;
    out.extend(b"PK\x01\x02");
    out.extend([20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    le(out, if big { 0xffff_ffff } else { size }, 4);
    le(out, name.len() as u64, 2);
    le(out, if big { 12 } else { 0 }, 2);
    out.extend([0; 10]);
    le(out, offset, 4);
    out.extend(name.as_bytes());
    if big {
        le(out, 1, 2);
        le(out, 8, 2);
        le(out, size, 8);
    }
}

/// The ZIP64 end record and its locator.
fn end64(out: &mut Vec<u8>, count: usize, size: usize, directory: usize) {
    let eocd64 = out.len() as u64;
    out.extend(b"PK\x06\x06");
    le(out, 44, 8);
    out.extend([45, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    for value in [count, count, size, directory] {
        le(out, value as u64, 8);
    }
    out.extend(b"PK\x06\x07");
    le(out, 0, 4);
    le(out, eocd64, 8);
    le(out, 1, 4);
}

/// A ZIP archive of empty stored entries that claim the given sizes: local headers, the central directory and the end
/// record. With `zip64` the counts and the offset are in a ZIP64 end record and the sizes in extra fields.
pub fn zip(entries: &[(&str, u64)], zip64: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let offsets = local_headers(&mut out, entries);
    let directory = out.len();
    for (entry, offset) in entries.iter().zip(offsets) {
        central_record(&mut out, *entry, offset, zip64);
    }
    let size = out.len() - directory;
    if zip64 {
        end64(&mut out, entries.len(), size, directory);
    }
    out.extend(b"PK\x05\x06");
    out.extend([0, 0, 0, 0]);
    let (count, offset) = if zip64 {
        (0xffff, 0xffff_ffff)
    } else {
        (entries.len() as u64, directory as u64)
    };
    le(&mut out, count, 2);
    le(&mut out, count, 2);
    le(&mut out, size as u64, 4);
    le(&mut out, offset, 4);
    le(&mut out, 0, 2);
    out
}

/// An ASAR archive whose header is `header` and whose contents are `contents`.
pub fn asar(header: &Value, contents: &[u8]) -> Vec<u8> {
    let json = header.to_string();
    let padded = json.len().div_ceil(4) * 4;
    let mut out = Vec::new();
    for value in [4, 8 + padded, 4 + padded, json.len()] {
        le(&mut out, value as u64, 4);
    }
    out.extend(json.as_bytes());
    out.resize(16 + padded, 0);
    out.extend(contents);
    out
}

/// An ASAR header with a few files: a `package.json` at offset 0 (so `contents` must start with its text), a script
/// with an integrity record, and a nested package.
pub fn asar_header(package: &str) -> Value {
    let integrity = json!({"algorithm": "SHA256", "hash": "00", "blockSize": 4, "blocks": ["00"]});
    json!({"files": {
        "package.json": {"size": package.len(), "offset": "0"},
        "main.js": {"size": 10, "offset": "100", "integrity": integrity},
        "native.node": {"size": 20, "unpacked": true},
        "app": {"files": {
            "node_modules": {"files": {"left-pad": {"files": {"index.js": {"size": 5, "offset": "200"}}},
                "@scope": {"files": {"pkg": {"files": {"a.js": {"size": 1, "offset": "300"}}}}}}},
            "link.js": {"link": "../main.js"}}}
    }})
}
