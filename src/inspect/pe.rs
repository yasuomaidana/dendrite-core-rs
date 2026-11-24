//! A PE image as an [`Image`]: what `pe.parse` reads, in the shape every format shares.

use crate::pe::parse_bytes;

use super::image::{Budget, Image, Section, SECTION_COST};

/// `IMAGE_FILE_DLL` in the file header's characteristics.
const DLL: u16 = 0x2000;

/// Whether the file header says the image is a DLL; the header was already read once, so a short read is `false`.
fn is_dll(data: &[u8]) -> bool {
    let at = |from: usize| {
        data.get(from..from + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let characteristics = at(0x3c).and_then(|nt| {
        let from = usize::try_from(nt).ok()?.checked_add(22)?;
        data.get(from..from + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    });
    characteristics.is_some_and(|bits| bits & DLL != 0)
}

pub fn inspect(data: &[u8], budget: &mut Budget) -> Option<Image> {
    let info = parse_bytes(data).ok()?;
    let sections = info
        .sections
        .into_iter()
        .map_while(|section| {
            Some(Section {
                name: budget.take_costing(section.name.as_bytes(), SECTION_COST)?,
                address: section.virtual_address,
                size: section.virtual_size.into(),
                raw_size: section.raw_size.into(),
                flags: section.flags,
            })
        })
        .collect();
    let imports = info.imports.into_iter().map(|dll| (dll.dll, dll.functions));
    let libraries = budget.libraries(imports.collect());
    let exports = budget.take_all(info.exports.iter().map(String::as_bytes));
    budget.cut(info.truncated);
    let mut image = Image {
        kind: if is_dll(data) {
            "library"
        } else {
            "executable"
        },
        format: "pe",
        arch: Some(info.machine),
        bits: Some(if info.pe32_plus { 64 } else { 32 }),
        endian: Some("little"),
        entry: Some(info.entry_point),
        base: Some(info.image_base),
        sections,
        libraries,
        exports,
        ..Image::default()
    };
    image.notes.insert("subsystem", info.subsystem);
    Some(image)
}
