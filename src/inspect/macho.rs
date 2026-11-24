//! Mach-O images, thin and fat, as an [`Image`]. A fat file is described through its first slice of a known
//! architecture, and `architectures` lists them all.

use object::macho::{
    LC_CODE_SIGNATURE, MH_BUNDLE, MH_DYLIB, MH_OBJECT, S_ATTR_PURE_INSTRUCTIONS,
    S_ATTR_SOME_INSTRUCTIONS,
};
use object::read::macho::{
    FatArch, MachHeader, MachOFatFile32, MachOFatFile64, MachOFile, Section as _,
};
use object::{Architecture, Endianness, Object, ObjectSection, ObjectSegment};

use super::image::{
    arch_name, endian_name, hex, Budget, Image, Section, MAX_SECTIONS, SECTION_COST,
};

/// The most entitlement keys listed.
const MAX_ENTITLEMENTS: usize = 24;
/// The most slices of a fat file listed.
const MAX_SLICES: usize = 16;
/// `CS_ADHOC` in a code directory's flags.
const CS_ADHOC: u32 = 0x2;

fn be32(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// The big-endian word at `at` as an index.
fn be_usize(data: &[u8], at: usize) -> Option<usize> {
    usize::try_from(be32(data, at)?).ok()
}

/// The blob of `kind` in a code signature's super blob, with its magic checked.
fn blob(sig: &[u8], kind: u32, magic: u32) -> Option<&[u8]> {
    let count = be_usize(sig, 8)?.min(32);
    (0..count).find_map(|i| blob_at(sig, 12 + 8 * i, kind, magic))
}

/// The blob that the index entry at `at` points to, when it is of `kind` and starts with `magic`.
fn blob_at(sig: &[u8], at: usize, kind: u32, magic: u32) -> Option<&[u8]> {
    let offset = be_usize(sig, at + 4)?;
    if be32(sig, at)? != kind || be32(sig, offset)? != magic {
        return None;
    }
    sig.get(offset..offset.checked_add(be_usize(sig, offset + 4)?)?)
}

/// The keys of an entitlements plist, as text.
fn entitlement_keys(plist: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(plist);
    text.split("<key>")
        .skip(1)
        .filter_map(|part| part.split("</key>").next())
        .take(MAX_ENTITLEMENTS)
        .map(str::to_string)
        .collect()
}

/// `(codesign, entitlements)`: `none`, `present` or `adhoc`, and the keys of the entitlements.
fn signature(sig: Option<&[u8]>) -> (&'static str, Vec<String>) {
    let Some(sig) = sig else {
        return ("none", Vec::new());
    };
    let adhoc = blob(sig, 0, 0xfade_0c02)
        .and_then(|directory| be32(directory, 12))
        .is_some_and(|flags| flags & CS_ADHOC != 0);
    let keys = blob(sig, 5, 0xfade_7171).map(|b| entitlement_keys(&b[8..]));
    (
        if adhoc { "adhoc" } else { "present" },
        keys.unwrap_or_default(),
    )
}

fn code_signature<'a, Mach: MachHeader<Endian = Endianness>>(
    file: &MachOFile<'a, Mach>,
    data: &'a [u8],
) -> Option<&'a [u8]> {
    let endian = file.endian();
    let mut commands = file.macho_load_commands().ok()?;
    while let Ok(Some(command)) = commands.next() {
        if command.cmd() == LC_CODE_SIGNATURE {
            let link = command
                .data::<object::macho::LinkeditDataCommand<Endianness>>()
                .ok()?;
            let start = usize::try_from(link.dataoff.get(endian)).ok()?;
            let end = start.checked_add(usize::try_from(link.datasize.get(endian)).ok()?)?;
            return data.get(start..end);
        }
    }
    None
}

/// The address the image is linked at: that of its `__TEXT` segment.
fn base<Mach: MachHeader<Endian = Endianness>>(file: &MachOFile<'_, Mach>) -> Option<u64> {
    let text = |segment: &object::read::macho::MachOSegment<'_, '_, Mach>| {
        segment.name_bytes() == Ok(Some(&b"__TEXT"[..]))
    };
    file.segments().find(text).map(|segment| segment.address())
}

/// `LC_MAIN` gives the entry as an offset into `__TEXT`; a thread command gives the address itself.
fn entry_address(entry: u64, base: Option<u64>) -> u64 {
    match base {
        Some(base) if entry < base => base.wrapping_add(entry),
        _ => entry,
    }
}

fn sections<Mach: MachHeader<Endian = Endianness>>(
    file: &MachOFile<'_, Mach>,
    budget: &mut Budget,
) -> Vec<Section> {
    file.sections()
        .take(MAX_SECTIONS)
        .map_while(|section| {
            let flags = section.macho_section().flags(file.endian());
            let segment = String::from_utf8_lossy(section.segment_name_bytes().ok()??).into_owned();
            let name = format!(
                "{segment}.{}",
                String::from_utf8_lossy(section.name_bytes().ok()?)
            );
            let code = flags & (S_ATTR_PURE_INSTRUCTIONS | S_ATTR_SOME_INSTRUCTIONS) != 0;
            Some(Section {
                name: budget.take_costing(name.as_bytes(), SECTION_COST)?,
                address: hex(section.address()),
                size: section.size(),
                raw_size: 0,
                flags: if code { vec!["code"] } else { Vec::new() },
            })
        })
        .collect()
}

/// A library's name; an import that names none (flat namespace) is credited to `*`.
fn library_name(bytes: &[u8]) -> String {
    ["*".to_string(), String::from_utf8_lossy(bytes).into_owned()][usize::from(!bytes.is_empty())]
        .clone()
}

/// The dylibs the image loads, each with the symbols it takes from it.
fn dylibs<Mach: MachHeader<Endian = Endianness>>(
    file: &MachOFile<'_, Mach>,
) -> Vec<(String, Vec<String>)> {
    let endian = file.endian();
    let mut libraries: Vec<(String, Vec<String>)> = Vec::new();
    if let Ok(mut commands) = file.macho_load_commands() {
        while let Ok(Some(command)) = commands.next() {
            let name = command
                .dylib()
                .ok()
                .flatten()
                .and_then(|dylib| command.string(endian, dylib.dylib.name).ok());
            if let Some(name) = name {
                libraries.push((String::from_utf8_lossy(name).into_owned(), Vec::new()));
            }
        }
    }
    for import in file.imports().unwrap_or_default() {
        let library = library_name(import.library());
        let at = libraries.iter().position(|(name, _)| *name == library);
        let at = at.unwrap_or_else(|| {
            libraries.push((library, Vec::new()));
            libraries.len() - 1
        });
        libraries[at]
            .1
            .push(String::from_utf8_lossy(import.name()).into_owned());
    }
    libraries
}

fn kind(filetype: u32) -> &'static str {
    match filetype {
        MH_OBJECT => "object",
        MH_DYLIB | MH_BUNDLE => "library",
        _ => "executable",
    }
}

fn read<Mach: MachHeader<Endian = Endianness>>(data: &[u8], budget: &mut Budget) -> Option<Image> {
    let file = MachOFile::<Mach>::parse(data).ok()?;
    let base = base(&file);
    let sections = sections(&file, budget);
    let libraries = budget.libraries(dylibs(&file));
    let exports: Vec<String> = file
        .exports()
        .unwrap_or_default()
        .iter()
        .map(|e| String::from_utf8_lossy(e.name()).into_owned())
        .collect();
    let exports = budget.take_all(exports.iter().map(String::as_bytes));
    let (codesign, entitlements) = signature(code_signature(&file, data));
    let mut image = Image {
        kind: kind(file.macho_header().filetype(file.endian())),
        format: "macho",
        arch: Some(arch_name(file.architecture())),
        bits: Some(if file.is_64() { 64 } else { 32 }),
        endian: Some(endian_name(file.is_little_endian())),
        entry: Some(hex(entry_address(file.entry(), base))),
        base: base.map(hex),
        sections,
        libraries,
        exports,
        ..Image::default()
    };
    image.notes.insert("codesign", codesign.to_string());
    if let Some(keys) = budget
        .take(entitlements.join(",").as_bytes())
        .filter(|k| !k.is_empty())
    {
        image.notes.insert("entitlements", keys);
    }
    Some(image)
}

/// A thin Mach-O: the magic says whether it is 32 or 64 bit.
pub fn inspect(data: &[u8], budget: &mut Budget) -> Option<Image> {
    if matches!(
        data.get(..4),
        Some([0xcf, 0xfa, 0xed, 0xfe] | [0xfe, 0xed, 0xfa, 0xcf])
    ) {
        read::<object::macho::MachHeader64<Endianness>>(data, budget)
    } else {
        read::<object::macho::MachHeader32<Endianness>>(data, budget)
    }
}

/// The slices of a fat file: their architectures, and the data of each.
fn slices<'a, A: FatArch>(arches: &[A], data: &'a [u8]) -> Vec<(Architecture, &'a [u8])> {
    arches
        .iter()
        .take(MAX_SLICES)
        .filter_map(|arch| Some((arch.architecture(), arch.data(data).ok()?)))
        .collect()
}

/// A fat file: described through its first readable slice, with every architecture listed in the notes.
pub fn inspect_fat(data: &[u8], budget: &mut Budget) -> Option<Image> {
    let slices = if data.get(3) == Some(&0xbf) {
        slices(MachOFatFile64::parse(data).ok()?.arches(), data)
    } else {
        slices(MachOFatFile32::parse(data).ok()?.arches(), data)
    };
    let names: Vec<String> = slices.iter().map(|(arch, _)| arch_name(*arch)).collect();
    let mut image = slices.iter().find_map(|(arch, slice)| {
        (*arch != Architecture::Unknown).then(|| inspect(slice, budget))?
    })?;
    image.format = "fat-macho";
    image.notes.insert("architectures", names.join(","));
    Some(image)
}
