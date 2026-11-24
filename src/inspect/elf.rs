//! An ELF image (32 or 64 bit, either byte order) as an [`Image`].
//!
//! ELF does not record which library a symbol comes from. A symbol with a GNU version requirement is credited to the
//! library that version names; the others go to the only `DT_NEEDED` library when there is one, and otherwise to a
//! last entry named `*`.

use object::elf::{
    DT_NEEDED, DT_RPATH, DT_RUNPATH, DT_SONAME, ET_DYN, ET_EXEC, ET_REL, PF_R, PF_W, PF_X,
    PT_INTERP, PT_LOAD, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE,
};
use object::read::elf::{Dyn, ElfFile, FileHeader, ProgramHeader, SectionHeader};
use object::{Endianness, Object, ObjectSection, ObjectSymbol, SymbolIndex};

use super::image::{
    arch_name, endian_name, hex, Budget, Image, Section, MAX_SECTIONS, SECTION_COST,
};

/// The most dynamic symbols looked at, so a hostile table cannot keep the reader busy.
const MAX_SYMBOLS: usize = 200_000;
/// The most dynamic entries looked at.
const MAX_DYNAMIC: usize = 4_096;

pub fn inspect(data: &[u8], budget: &mut Budget) -> Option<Image> {
    if data.get(4) == Some(&2) {
        read::<object::elf::FileHeader64<Endianness>>(data, budget)
    } else {
        read::<object::elf::FileHeader32<Endianness>>(data, budget)
    }
}

fn flag_names(table: &[(u64, &'static str)], bits: u64) -> Vec<&'static str> {
    table
        .iter()
        .filter(|(bit, _)| bits & bit != 0)
        .map(|(_, name)| *name)
        .collect()
}

const SECTION_FLAGS: &[(u64, &str)] = &[
    (SHF_ALLOC as u64, "alloc"),
    (SHF_EXECINSTR as u64, "execute"),
    (SHF_WRITE as u64, "write"),
];
const SEGMENT_FLAGS: &[(u64, &str)] = &[
    (PF_R as u64, "read"),
    (PF_W as u64, "write"),
    (PF_X as u64, "execute"),
];

fn sections<Elf: FileHeader<Endian = Endianness>>(
    file: &ElfFile<'_, Elf>,
    budget: &mut Budget,
) -> Vec<Section> {
    let named = file
        .sections()
        .filter(|s| !s.name_bytes().unwrap_or_default().is_empty());
    let found: Vec<Section> = named
        .take(MAX_SECTIONS)
        .map_while(|section| {
            let sh_flags: u64 = section.elf_section_header().sh_flags(file.endian()).into();
            Some(Section {
                name: budget
                    .take_costing(section.name_bytes().unwrap_or_default(), SECTION_COST)?,
                address: hex(section.address()),
                size: section.size(),
                raw_size: 0,
                flags: flag_names(SECTION_FLAGS, sh_flags),
            })
        })
        .collect();
    if found.is_empty() {
        return segments(file, budget);
    }
    found
}

/// The loadable segments, for an image whose section headers were stripped.
fn segments<Elf: FileHeader<Endian = Endianness>>(
    file: &ElfFile<'_, Elf>,
    budget: &mut Budget,
) -> Vec<Section> {
    let endian = file.endian();
    file.elf_program_headers()
        .iter()
        .filter(|p| p.p_type(endian) == PT_LOAD)
        .take(MAX_SECTIONS)
        .map_while(|p| {
            Some(Section {
                name: budget.take_costing(b"LOAD", SECTION_COST)?,
                address: hex(p.p_vaddr(endian).into()),
                size: p.p_memsz(endian).into(),
                raw_size: p.p_filesz(endian).into(),
                flags: flag_names(SEGMENT_FLAGS, p.p_flags(endian).into()),
            })
        })
        .collect()
}

fn interpreter<Elf: FileHeader<Endian = Endianness>>(
    file: &ElfFile<'_, Elf>,
    data: &[u8],
) -> Option<String> {
    let endian = file.endian();
    let header = file
        .elf_program_headers()
        .iter()
        .find(|p| p.p_type(endian) == PT_INTERP)?;
    let bytes = header.data(endian, data).ok()?;
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    Some(String::from_utf8_lossy(&bytes[..end.min(256)]).into_owned())
}

/// What the dynamic section names: libraries needed, and the soname and search path.
#[derive(Default)]
struct Dynamic {
    needed: Vec<String>,
    soname: Option<String>,
    rpath: Option<String>,
}

fn dynamic<Elf: FileHeader<Endian = Endianness>>(file: &ElfFile<'_, Elf>, data: &[u8]) -> Dynamic {
    let endian = file.endian();
    let mut found = Dynamic::default();
    let Ok(Some((entries, strings))) = file.elf_section_table().dynamic(endian, data) else {
        return found;
    };
    let Ok(strings) = file.elf_section_table().strings(endian, data, strings) else {
        return found;
    };
    for entry in entries.iter().take(MAX_DYNAMIC) {
        let Ok(text) = entry.string(endian, strings) else {
            continue;
        };
        let text = String::from_utf8_lossy(text).into_owned();
        match entry.tag(endian) {
            DT_NEEDED => found.needed.push(text),
            DT_SONAME => found.soname = Some(text),
            DT_RPATH | DT_RUNPATH => found.rpath = Some(text),
            _ => {}
        }
    }
    found
}

/// The library a symbol's version requirement names, when it has one.
fn needed_from<'a, Elf: FileHeader<Endian = Endianness>>(
    versions: &Option<object::read::elf::VersionTable<'a, Elf>>,
    endian: Endianness,
    index: SymbolIndex,
) -> Option<String> {
    let table = versions.as_ref()?;
    let version = table.version(table.version_index(endian, index)).ok()??;
    version
        .file()
        .map(|file| String::from_utf8_lossy(file).into_owned())
}

/// Imported symbols grouped by library, and the symbols the image defines.
fn symbols<Elf: FileHeader<Endian = Endianness>>(
    file: &ElfFile<'_, Elf>,
    data: &[u8],
    needed: &[String],
) -> (Vec<(String, Vec<String>)>, Vec<String>) {
    let endian = file.endian();
    let versions = file
        .elf_section_table()
        .versions(endian, data)
        .ok()
        .flatten();
    let mut libraries: Vec<(String, Vec<String>)> =
        needed.iter().map(|n| (n.clone(), Vec::new())).collect();
    let mut unattributed = Vec::new();
    let mut exports = Vec::new();
    for symbol in file.dynamic_symbols().take(MAX_SYMBOLS) {
        let Ok(name) = symbol.name() else { continue };
        if name.is_empty() {
            continue;
        }
        if !symbol.is_undefined() {
            exports.push(name.to_string());
            continue;
        }
        let owner = needed_from(&versions, endian, symbol.index())
            .and_then(|file| libraries.iter().position(|(n, _)| *n == file));
        match (owner, needed.len()) {
            (Some(at), _) => libraries[at].1.push(name.to_string()),
            (None, 1) => libraries[0].1.push(name.to_string()),
            (None, _) => unattributed.push(name.to_string()),
        }
    }
    if !unattributed.is_empty() {
        libraries.push(("*".to_string(), unattributed));
    }
    (libraries, exports)
}

fn kind(e_type: u16, has_interpreter: bool) -> &'static str {
    match e_type {
        ET_REL => "object",
        ET_DYN if !has_interpreter => "library",
        ET_EXEC | ET_DYN => "executable",
        _ => "data",
    }
}

fn read<Elf: FileHeader<Endian = Endianness>>(data: &[u8], budget: &mut Budget) -> Option<Image> {
    let file = ElfFile::<Elf>::parse(data).ok()?;
    let endian = file.endian();
    let interpreter = interpreter(&file, data);
    let dynamic = dynamic(&file, data);
    let base = file
        .elf_program_headers()
        .iter()
        .filter(|p| p.p_type(endian) == PT_LOAD)
        .map(|p| p.p_vaddr(endian).into())
        .min();
    // The notes are few and say the most, so they are paid for before the long lists.
    let mut notes = std::collections::BTreeMap::new();
    for (key, value) in [
        ("interpreter", interpreter.clone()),
        ("rpath", dynamic.rpath.clone()),
        ("soname", dynamic.soname.clone()),
    ] {
        if let Some(text) = value.and_then(|v| budget.take(v.as_bytes())) {
            notes.insert(key, text);
        }
    }
    let sections = sections(&file, budget);
    let (raw, defined) = symbols(&file, data, &dynamic.needed);
    let libraries = budget.libraries(raw);
    let exports = budget.take_all(defined.iter().map(String::as_bytes));
    let image = Image {
        kind: kind(file.elf_header().e_type(endian), interpreter.is_some()),
        format: "elf",
        arch: Some(arch_name(file.architecture())),
        bits: Some(if file.is_64() { 64 } else { 32 }),
        endian: Some(endian_name(file.is_little_endian())),
        entry: Some(hex(file.entry())),
        base: base.map(hex),
        sections,
        libraries,
        exports,
        notes,
        ..Image::default()
    };
    Some(image)
}
