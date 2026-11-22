//! Builds small PE images in memory for the tests of `pe.parse`, so no binary files live in the repository.
//!
//! Layout: DOS header, `PE\0\0`, COFF header, optional header with 16 data directories, the section table,
//! `.text` (RVA 0x1000), and `.rdata` (RVA 0x2000) holding the import tables and then the export table.

use std::path::PathBuf;

const TEXT_RVA: u32 = 0x1000;
const DATA_RVA: u32 = 0x2000;
const TEXT_FLAGS: u32 = 0x6000_0020;
const DATA_FLAGS: u32 = 0x4000_0040;

/// What to put in an image.
pub struct Spec {
    pub plus: bool,
    pub machine: u16,
    pub subsystem: u16,
    /// Empty sections after `.text` and `.rdata`, to fill the section table.
    pub extra_sections: usize,
    /// DLLs with the functions imported from each; `#7` is imported by ordinal 7.
    pub imports: Vec<(String, Vec<String>)>,
    /// One entry per ordinal slot (the first is ordinal 1): its name and its address (0 marks an unused slot).
    pub exports: Vec<(Option<String>, u32)>,
}

impl Default for Spec {
    fn default() -> Spec {
        Spec::new(true)
    }
}

impl Spec {
    pub fn new(plus: bool) -> Spec {
        let machine = if plus { 0x8664 } else { 0x14c };
        Spec {
            plus,
            machine,
            subsystem: 3,
            extra_sections: 0,
            imports: vec![],
            exports: vec![],
        }
    }
}

fn set(buf: &mut [u8], at: usize, bytes: &[u8]) {
    buf[at..at + bytes.len()].copy_from_slice(bytes);
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend(value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_le_bytes());
}

/// An import thunk: the RVA of a hint and name, or an ordinal when the top bit is set.
fn put_thunk(out: &mut Vec<u8>, plus: bool, value: u32, by_ordinal: bool) {
    if plus {
        out.extend((u64::from(value) | u64::from(by_ordinal) << 63).to_le_bytes());
    } else {
        put32(out, value | u32::from(by_ordinal) << 31);
    }
}

fn import_blob(spec: &Spec, rva: u32) -> Vec<u8> {
    if spec.imports.is_empty() {
        return Vec::new();
    }
    let mut blob = vec![0u8; (spec.imports.len() + 1) * 20];
    for (i, (dll, functions)) in spec.imports.iter().enumerate() {
        let mut thunks = Vec::new();
        for function in functions {
            if let Some(ordinal) = function.strip_prefix('#') {
                put_thunk(&mut thunks, spec.plus, ordinal.parse().unwrap(), true);
            } else {
                put_thunk(&mut thunks, spec.plus, rva + blob.len() as u32, false);
                put16(&mut blob, 0);
                blob.extend(function.bytes().chain([0]));
                blob.resize(blob.len().next_multiple_of(2), 0);
            }
        }
        put_thunk(&mut thunks, spec.plus, 0, false);
        let thunk_rva = rva + blob.len() as u32;
        blob.extend(thunks);
        let name_rva = rva + blob.len() as u32;
        blob.extend(dll.bytes().chain([0]));
        set(&mut blob, i * 20, &thunk_rva.to_le_bytes());
        set(&mut blob, i * 20 + 12, &name_rva.to_le_bytes());
        set(&mut blob, i * 20 + 16, &thunk_rva.to_le_bytes());
    }
    blob
}

fn export_blob(spec: &Spec, rva: u32) -> Vec<u8> {
    if spec.exports.is_empty() {
        return Vec::new();
    }
    let named: Vec<(usize, &String)> = spec
        .exports
        .iter()
        .enumerate()
        .filter_map(|(i, (name, _))| name.as_ref().map(|name| (i, name)))
        .collect();
    let (count, names) = (spec.exports.len() as u32, named.len() as u32);
    let (functions_rva, names_rva) = (rva + 40, rva + 40 + 4 * count);
    let (ordinals_rva, strings_rva) = (names_rva + 4 * names, names_rva + 6 * names);
    let mut out = vec![0u8; 40];
    for (at, value) in [
        (16, 1),
        (20, count),
        (24, names),
        (28, functions_rva),
        (32, names_rva),
        (36, ordinals_rva),
    ] {
        set(&mut out, at, &u32::to_le_bytes(value));
    }
    for (_, address) in &spec.exports {
        put32(&mut out, *address);
    }
    let mut strings = Vec::new();
    for (_, name) in &named {
        put32(&mut out, strings_rva + strings.len() as u32);
        strings.extend(name.bytes().chain([0]));
    }
    for (index, _) in &named {
        put16(&mut out, *index as u16);
    }
    out.extend(strings);
    out
}

fn section(out: &mut Vec<u8>, name: &str, address: u32, size: u32, offset: u32, flags: u32) {
    let mut header = [0u8; 40];
    set(&mut header, 0, name.as_bytes());
    for (at, value) in [
        (8, size),
        (12, address),
        (16, size),
        (20, offset),
        (36, flags),
    ] {
        set(&mut header, at, &value.to_le_bytes());
    }
    out.extend(header);
}

/// The optional header: PE32 is 224 bytes, PE32+ 240; the data directories are its last 128.
fn optional_header(spec: &Spec, headers: usize, imports: &[u8], exports: &[u8]) -> Vec<u8> {
    let len = if spec.plus { 240 } else { 224 };
    let mut opt = vec![0u8; len];
    set(
        &mut opt,
        0,
        &(if spec.plus { 0x20bu16 } else { 0x10b }).to_le_bytes(),
    );
    set(&mut opt, 16, &TEXT_RVA.to_le_bytes());
    set(&mut opt, 20, &TEXT_RVA.to_le_bytes());
    if spec.plus {
        set(&mut opt, 24, &0x1_4000_0000u64.to_le_bytes());
    } else {
        set(&mut opt, 28, &0x40_0000u32.to_le_bytes());
    }
    set(&mut opt, 32, &0x1000u32.to_le_bytes());
    set(&mut opt, 36, &0x200u32.to_le_bytes());
    set(&mut opt, 60, &(headers as u32).to_le_bytes());
    set(&mut opt, 68, &spec.subsystem.to_le_bytes());
    let directories = len - 128;
    set(&mut opt, directories - 4, &16u32.to_le_bytes());
    for (index, rva, size) in [
        (0, DATA_RVA + imports.len() as u32, exports.len()),
        (1, DATA_RVA, imports.len()),
    ] {
        // An absent table has RVA 0, as in real images.
        set(
            &mut opt,
            directories + 8 * index,
            &(rva * u32::from(size > 0)).to_le_bytes(),
        );
        set(
            &mut opt,
            directories + 8 * index + 4,
            &(size as u32).to_le_bytes(),
        );
    }
    opt
}

/// The file offset of the RVA of data directory `index` (0 is exports, 1 is imports).
pub fn directory_offset(plus: bool, index: usize) -> usize {
    0x40 + 24 + if plus { 112 } else { 96 } + 8 * index
}

/// A well-formed image holding what `spec` asks for.
pub fn build(spec: &Spec) -> Vec<u8> {
    let imports = import_blob(spec, DATA_RVA);
    let exports = export_blob(spec, DATA_RVA + imports.len() as u32);
    let blob = [imports.as_slice(), exports.as_slice()].concat();
    let opt = optional_header(spec, 0, &imports, &exports);
    let sections = 1 + usize::from(!blob.is_empty()) + spec.extra_sections;
    let headers = (0x40 + 24 + opt.len() + 40 * sections).next_multiple_of(0x200);
    let opt = optional_header(spec, headers, &imports, &exports);
    let mut out = b"MZ".to_vec();
    out.resize(0x3c, 0);
    put32(&mut out, 0x40);
    out.extend(b"PE\0\0");
    put16(&mut out, spec.machine);
    put16(&mut out, sections as u16);
    out.extend([0u8; 12]);
    put16(&mut out, opt.len() as u16);
    put16(&mut out, 0x22);
    out.extend(opt);
    section(
        &mut out,
        ".text",
        TEXT_RVA,
        0x200,
        headers as u32,
        TEXT_FLAGS,
    );
    if !blob.is_empty() {
        section(
            &mut out,
            ".rdata",
            DATA_RVA,
            blob.len() as u32,
            headers as u32 + 0x200,
            DATA_FLAGS,
        );
    }
    for i in 0..spec.extra_sections {
        section(&mut out, &format!(".x{i}"), 0x10_0000, 0, 0, 0);
    }
    out.resize(headers + 0x200, 0);
    out.extend(blob);
    out
}

/// A path in the temp directory that no other test and no other run uses.
pub fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("dendrite-core-pe-{}-{tag}", std::process::id()))
}

/// Writes `bytes` to a file of its own and returns the path.
pub fn write_temp(tag: &str, bytes: &[u8]) -> PathBuf {
    let path = temp_path(tag);
    std::fs::write(&path, bytes).unwrap();
    path
}
