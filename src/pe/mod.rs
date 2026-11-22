//! `pe.parse`: the headers, sections, imports and exports of a Windows PE image (PE32 and PE32+).
//!
//! The image is read with the bounds-checked parser of the `object` crate, so a damaged or hostile file is an
//! error, never a panic. What the answer may hold is capped: an agent reads it, so it has to stay small.

use object::pe::{ImageNtHeaders32, ImageNtHeaders64};
use object::read::pe::{
    ExportTarget, ImageNtHeaders, ImageOptionalHeader, Import, ImportTable, PeFile,
};
use object::{FileKind, LittleEndian as LE};
use serde::Serialize;

use crate::error::CoreError;
use image::{hex, not_pe, read_file};

pub use image::MAX_FILE_BYTES;

/// Imported functions, DLLs and exports share this many names; past it the answer says `truncated`.
pub const MAX_NAMES: usize = 500;
/// The most sections reported (the Windows loader refuses more than 96).
pub const MAX_SECTIONS: usize = 96;
/// A longer name is cut to this many bytes.
pub const MAX_NAME_LEN: usize = 256;
/// An export table with more entries than the 16 bits of an ordinal can number is not read.
const MAX_EXPORT_ENTRIES: usize = 65_536;

const MACHINES: &[(u16, &str)] = &[
    (0x14c, "x86"),
    (0x8664, "x86_64"),
    (0x1c0, "arm"),
    (0x1c4, "arm"),
    (0xaa64, "arm64"),
];
const SUBSYSTEMS: &[(u16, &str)] = &[
    (1, "native"),
    (2, "gui"),
    (3, "console"),
    (7, "posix"),
    (9, "windows_ce"),
    (10, "efi_application"),
    (11, "efi_boot_driver"),
    (12, "efi_runtime_driver"),
    (14, "xbox"),
];
const SECTION_FLAGS: &[(u32, &str)] = &[
    (0x20, "code"),
    (0x40, "initialized_data"),
    (0x80, "uninitialized_data"),
    (0x2000_0000, "execute"),
    (0x4000_0000, "read"),
    (0x8000_0000, "write"),
    (0x1000_0000, "shared"),
    (0x0200_0000, "discardable"),
];

/// One section of the image; addresses are hex strings so 64-bit values survive JSON.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub virtual_address: String,
    pub virtual_size: u32,
    pub raw_size: u32,
    pub flags: Vec<&'static str>,
}

/// The functions imported from one DLL; an import by ordinal is `#<ordinal>`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ImportedDll {
    pub dll: String,
    pub functions: Vec<String>,
}

/// What `pe.parse` answers.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PeInfo {
    pub machine: String,
    pub subsystem: String,
    pub pe32_plus: bool,
    pub entry_point: String,
    pub image_base: String,
    pub sections: Vec<Section>,
    pub imports: Vec<ImportedDll>,
    pub exports: Vec<String>,
    /// True when a cap cut something from the answer.
    pub truncated: bool,
}

/// Counts the names still allowed and remembers whether any cap was hit.
struct Limits {
    names: usize,
    truncated: bool,
}

impl Limits {
    /// Takes one name from the budget; false once it is spent.
    fn take(&mut self) -> bool {
        let allowed = self.names > 0;
        self.names = self.names.saturating_sub(1);
        self.truncated |= !allowed;
        allowed
    }

    /// A name as text, cut to [`MAX_NAME_LEN`] bytes.
    fn text(&mut self, bytes: &[u8]) -> String {
        self.truncated |= bytes.len() > MAX_NAME_LEN;
        String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_NAME_LEN)]).into_owned()
    }
}

/// The name of a known code, or the code in hex.
fn named(table: &[(u16, &'static str)], code: u16) -> String {
    table
        .iter()
        .find(|(known, _)| *known == code)
        .map_or_else(|| hex(code.into()), |(_, name)| (*name).to_string())
}

fn sections<H: ImageNtHeaders>(file: &PeFile<H>, limits: &mut Limits) -> Vec<Section> {
    let table = file.section_table();
    limits.truncated |= table.len() > MAX_SECTIONS;
    table
        .iter()
        .take(MAX_SECTIONS)
        .map(|section| {
            let flags = section.characteristics.get(LE);
            let name_len = section.name.iter().take_while(|&&byte| byte != 0).count();
            Section {
                name: String::from_utf8_lossy(&section.name[..name_len]).into_owned(),
                virtual_address: hex(section.virtual_address.get(LE).into()),
                virtual_size: section.virtual_size.get(LE),
                raw_size: section.size_of_raw_data.get(LE),
                flags: SECTION_FLAGS
                    .iter()
                    .filter(|(bit, _)| flags & bit != 0)
                    .map(|(_, name)| *name)
                    .collect(),
            }
        })
        .collect()
}

/// The functions of one DLL, read from its table of thunks at `rva`; the cap may cut the list short.
fn read_functions<H: ImageNtHeaders>(
    table: &ImportTable,
    rva: u32,
    limits: &mut Limits,
) -> object::Result<Vec<String>> {
    let (mut thunks, mut functions) = (table.thunks(rva)?, Vec::new());
    while let Some(thunk) = thunks.next::<H>()? {
        if !limits.take() {
            break;
        }
        functions.push(match table.import::<H>(thunk)? {
            Import::Ordinal(ordinal) => format!("#{ordinal}"),
            Import::Name(_, name) => limits.text(name),
        });
    }
    Ok(functions)
}

/// Appends the imports to `out`; on a damaged table, what was read before the damage stays there.
fn read_imports<H: ImageNtHeaders>(
    file: &PeFile<H>,
    limits: &mut Limits,
    out: &mut Vec<ImportedDll>,
) -> object::Result<()> {
    let Some(table) = file.import_table()? else {
        return Ok(());
    };
    let mut descriptors = table.descriptors()?;
    while let Some(descriptor) = descriptors.next()? {
        if !limits.take() {
            break;
        }
        let dll = limits.text(table.name(descriptor.name.get(LE))?);
        // The lookup table; images without one (a zero) are read through the address table.
        let (lookup, address) = (
            descriptor.original_first_thunk.get(LE),
            descriptor.first_thunk.get(LE),
        );
        let functions =
            read_functions::<H>(&table, [lookup, address][usize::from(lookup == 0)], limits)?;
        out.push(ImportedDll { dll, functions });
    }
    Ok(())
}

/// Appends the exports to `out`: names, or `#<ordinal>` for an export without one; unused ordinal slots are skipped.
fn read_exports<H: ImageNtHeaders>(
    file: &PeFile<H>,
    limits: &mut Limits,
    out: &mut Vec<String>,
) -> object::Result<()> {
    let Some(table) = file.export_table()? else {
        return Ok(());
    };
    if table.addresses().len() > MAX_EXPORT_ENTRIES {
        limits.truncated = true;
        return Ok(());
    }
    let used = |export: &object::read::pe::Export| {
        export.name.is_some() || !matches!(export.target, ExportTarget::Address(0))
    };
    for export in table.exports()?.iter().filter(|export| used(export)) {
        if !limits.take() {
            break;
        }
        out.push(match export.name {
            Some(name) => limits.text(name),
            None => format!("#{}", export.ordinal),
        });
    }
    Ok(())
}

fn read<H: ImageNtHeaders>(data: &[u8]) -> Result<PeInfo, CoreError> {
    let file = PeFile::<H>::parse(data).map_err(not_pe)?;
    let (header, optional) = (
        file.nt_headers().file_header(),
        file.nt_headers().optional_header(),
    );
    let mut limits = Limits {
        names: MAX_NAMES,
        truncated: false,
    };
    let sections = sections(&file, &mut limits);
    let (mut imports, mut exports) = (Vec::new(), Vec::new());
    // A damaged import or export table keeps what was read before the damage; the image itself is still described.
    let _ = read_imports(&file, &mut limits, &mut imports);
    let _ = read_exports(&file, &mut limits, &mut exports);
    let image_base = optional.image_base();
    Ok(PeInfo {
        machine: named(MACHINES, header.machine.get(LE)),
        subsystem: named(SUBSYSTEMS, optional.subsystem()),
        pe32_plus: file.nt_headers().is_type_64(),
        entry_point: hex(image_base.wrapping_add(optional.address_of_entry_point().into())),
        image_base: hex(image_base),
        sections,
        imports,
        exports,
        truncated: limits.truncated,
    })
}

/// Describes the PE image in `data`; anything else is [`CoreError::NotPe`].
pub fn parse_bytes(data: &[u8]) -> Result<PeInfo, CoreError> {
    match FileKind::parse(data) {
        Ok(FileKind::Pe32) => read::<ImageNtHeaders32>(data),
        Ok(FileKind::Pe64) => read::<ImageNtHeaders64>(data),
        _ => Err(not_pe(
            "no MZ and PE signatures, or the headers point outside the file",
        )),
    }
}

/// Describes the PE image at `path`. Only a regular file of at most [`MAX_FILE_BYTES`] is read.
pub fn parse_file(path: &str) -> Result<PeInfo, CoreError> {
    parse_bytes(&read_file(path)?)
}

pub mod cfg;
pub mod functions;
mod image;

#[cfg(test)]
pub(crate) mod fixture;

#[cfg(test)]
mod tests;
