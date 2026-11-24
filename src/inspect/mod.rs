//! `bin.inspect`: what any file is, in one shape.
//!
//! The format is recognised from the first bytes, never from the name; PE, ELF, Mach-O (thin and fat) are read with
//! the bounds-checked parsers of the `object` crate and everything else is `data`, which is not an error. A damaged
//! table keeps what was read before it. Nothing in the file is ever executed, and names, symbols and strings share
//! one budget, so the answer stays small enough for an agent to read.

use std::fs;
use std::path::Path;

use crate::error::CoreError;
use crate::pe::MAX_FILE_BYTES;

mod elf;
mod image;
mod macho;
pub(crate) mod package;
mod pe;
mod scan;

#[cfg(test)]
mod elf_fixture;
#[cfg(test)]
mod macho_fixture;
#[cfg(test)]
mod tests;

pub use image::{Budget, FileEntry, Image, Library, Section};
use image::{BUDGET_CHARS, STRINGS_CHARS};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Magic {
    Pe,
    Elf,
    MachO,
    Fat,
    Zip,
    Asar,
    Unknown,
}

/// The format named by the first bytes. A fat Mach-O and a Java class both start `CAFEBABE`; a class file has a
/// version (45 or more) where a fat file has its small count of architectures.
fn sniff(data: &[u8]) -> Magic {
    let be32 = |at: usize| {
        let bytes = data.get(at..at + 4)?;
        Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    let count = be32(4).unwrap_or(0);
    match be32(0) {
        Some(0xcafe_babe | 0xcafe_babf) if (1..45).contains(&count) => Magic::Fat,
        Some(0xfeed_face | 0xfeed_facf | 0xcefa_edfe | 0xcffa_edfe) => Magic::MachO,
        Some(0x7f45_4c46) => Magic::Elf,
        Some(0x0400_0000)
            if data
                .get(16..)
                .is_some_and(|rest| rest.starts_with(b"{\"files\"")) =>
        {
            Magic::Asar
        }
        _ if data.starts_with(b"MZ") => Magic::Pe,
        _ if data.starts_with(b"PK\x03\x04") || data.starts_with(b"PK\x05\x06") => Magic::Zip,
        _ => Magic::Unknown,
    }
}

/// A file of a format that is not known: it still has an entropy and strings.
fn unknown() -> Image {
    Image {
        kind: "data",
        format: "unknown",
        ..Image::default()
    }
}

/// Describes `data`. A file that claims a format but cannot be read as one is described as unknown data.
pub fn inspect_bytes(data: &[u8]) -> Image {
    inspect_named(data, None)
}

/// The strings of an image: a package lists its files instead, a bundle has its quoted literals, anything else
/// its printable runs.
fn strings_of(image: &Image, data: &[u8], budget: &mut Budget) -> Vec<String> {
    match image.format {
        "zip" | "asar" => Vec::new(),
        "js-bundle" => package::js::strings(data, budget),
        _ => scan::strings(data, budget),
    }
}

/// Like [`inspect_bytes`], for a file whose lowercase extension is `ext`: a script is a bundle even without a
/// bundler's mark.
pub fn inspect_named(data: &[u8], ext: Option<&str>) -> Image {
    let mut budget = Budget::default();
    let found = match sniff(data) {
        Magic::Pe => pe::inspect(data, &mut budget),
        Magic::Elf => elf::inspect(data, &mut budget),
        Magic::MachO => macho::inspect(data, &mut budget),
        Magic::Fat => macho::inspect_fat(data, &mut budget),
        Magic::Zip => package::zip::inspect(data, &mut budget),
        Magic::Asar => package::asar::inspect(data, &mut budget),
        Magic::Unknown => package::js::inspect(data, ext, &mut budget),
    };
    let mut image = found.unwrap_or_else(unknown);
    let mut strings = Budget::new(if image.format == "unknown" {
        BUDGET_CHARS
    } else {
        STRINGS_CHARS
    });
    image.strings = strings_of(&image, data, &mut strings);
    image.entropy = scan::entropy(data);
    image.truncated = budget.truncated | strings.truncated;
    image
}

/// Describes the file at `path`: a regular file of at most [`MAX_FILE_BYTES`].
pub fn inspect_path(path: &str) -> Result<Image, CoreError> {
    let unreadable = |why: String| CoreError::Unreadable(format!("`{path}`: {why}"));
    let meta = fs::metadata(path).map_err(|e| unreadable(e.to_string()))?;
    if meta.is_dir() {
        return package::tree::inspect(Path::new(path)).map_err(unreadable);
    }
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err(unreadable(format!(
            "not a regular file of at most {MAX_FILE_BYTES} bytes"
        )));
    }
    let data = fs::read(path).map_err(|e| unreadable(e.to_string()))?;
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
    Ok(inspect_named(&data, ext.as_deref()))
}
