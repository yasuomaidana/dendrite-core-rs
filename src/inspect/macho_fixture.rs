//! Hand-built 64-bit little-endian Mach-O images for the tests, thin and fat.

const BASE: u64 = 0x1_0000_0000;
const TEXT: u64 = BASE + 0xf00;

#[derive(Clone)]
pub struct Spec {
    /// `0x0100_0007` (x86-64) or `0x0100_000c` (arm64).
    pub cputype: u32,
    pub filetype: u32,
    /// `None`: unsigned; `Some(true)`: ad hoc signature; `Some(false)`: a signature that is not ad hoc.
    pub adhoc: Option<bool>,
    pub entitlements: &'static str,
    /// Flat namespace: the image's imports name no library.
    pub flat: bool,
}

impl Spec {
    pub fn program() -> Self {
        Spec {
            cputype: 0x0100_0007,
            filetype: 2,
            flat: false,
            adhoc: Some(true),
            entitlements: "<plist><dict><key>com.apple.security.app-sandbox</key><true/><key>get-task-allow</key><true/></dict></plist>",
        }
    }
}

fn put(out: &mut Vec<u8>, values: &[u64], size: usize) {
    for value in values {
        out.extend(&value.to_le_bytes()[..size]);
    }
}

fn name16(name: &str) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[..name.len()].copy_from_slice(name.as_bytes());
    bytes
}

fn be(out: &mut Vec<u8>, values: &[u32]) {
    for value in values {
        out.extend(value.to_be_bytes());
    }
}

/// A code signature super blob: a code directory and an entitlements blob.
fn signature(adhoc: bool, entitlements: &str) -> Vec<u8> {
    let mut directory = Vec::new();
    be(
        &mut directory,
        &[0xfade_0c02, 44, 0x2_0400, if adhoc { 2 } else { 0 }],
    );
    directory.resize(44, 0);
    let mut plist = Vec::new();
    be(&mut plist, &[0xfade_7171, 8 + entitlements.len() as u32]);
    plist.extend(entitlements.as_bytes());
    let mut sig = Vec::new();
    let first = 12 + 16;
    be(
        &mut sig,
        &[
            0xfade_0cc0,
            (first + directory.len() + plist.len()) as u32,
            2,
        ],
    );
    be(
        &mut sig,
        &[0, first as u32, 5, (first + directory.len()) as u32],
    );
    sig.extend(directory);
    sig.extend(plist);
    sig
}

/// `__TEXT` with `__text`, a dylib, `LC_MAIN`, a symbol table (one import from libSystem, one export), a dynamic
/// symbol table and, when `spec.adhoc` says so, a code signature.
pub fn build(spec: &Spec) -> Vec<u8> {
    let strings = b"\0_printf\0_main\0";
    let libsystem = b"/usr/lib/libSystem.B.dylib\0\0\0\0\0\0";
    let signed = spec.adhoc.map(|adhoc| signature(adhoc, spec.entitlements));
    let ncmds = 5 + u32::from(signed.is_some());
    let cmds_size: u32 =
        152 + (24 + libsystem.len() as u32) + 24 + 24 + 80 + if signed.is_some() { 16 } else { 0 };
    let data_at = 32 + cmds_size as usize;
    let (symoff, stroff) = (data_at, data_at + 32);
    let mut out = Vec::new();
    // Header: magic, cpu type and subtype, file type, load commands, their size, flags (two-level), reserved.
    put(
        &mut out,
        &[
            0xfeed_facf,
            spec.cputype.into(),
            3,
            spec.filetype.into(),
            ncmds.into(),
            cmds_size.into(),
            0x80,
            0,
        ],
        4,
    );
    // LC_SEGMENT_64 `__TEXT`, one section.
    put(&mut out, &[0x19, 152], 4);
    out.extend(name16("__TEXT"));
    put(&mut out, &[BASE, 0x1000, 0, 0x1000], 8);
    put(&mut out, &[5, 5, 1, 0], 4);
    out.extend(name16("__text"));
    out.extend(name16("__TEXT"));
    put(&mut out, &[TEXT, 16], 8);
    put(&mut out, &[0xf00, 4, 0, 0, 0x8000_0400, 0, 0, 0], 4);
    // LC_LOAD_DYLIB.
    put(
        &mut out,
        &[0xc, 24 + libsystem.len() as u64, 24, 2, 0, 0],
        4,
    );
    out.extend(libsystem);
    // LC_MAIN: entry offset, stack size.
    put(&mut out, &[0x8000_0028, 24], 4);
    put(&mut out, &[0xf00, 0], 8);
    // LC_SYMTAB: two symbols.
    put(
        &mut out,
        &[2, 24, symoff as u64, 2, stroff as u64, strings.len() as u64],
        4,
    );
    // LC_DYSYMTAB: no locals, one defined external at 1 and one undefined at 0; the rest zero.
    let mut dysym = vec![0xb, 80, 0, 0, 1, 1, 0, 1];
    dysym.resize(20, 0);
    put(&mut out, &dysym, 4);
    if let Some(sig) = &signed {
        put(
            &mut out,
            &[0x1d, 16, (stroff + strings.len()) as u64, sig.len() as u64],
            4,
        );
    }
    // Symbols: `_printf` undefined, external, from library 1; `_main` defined in section 1.
    out.extend(1u32.to_le_bytes());
    out.extend([0x01, 0]);
    out.extend((if spec.flat { 0u16 } else { 0x0100 }).to_le_bytes());
    out.extend(0u64.to_le_bytes());
    out.extend(9u32.to_le_bytes());
    out.extend([0x0f, 1]);
    out.extend(0u16.to_le_bytes());
    out.extend(TEXT.to_le_bytes());
    out.extend(strings);
    out.extend(signed.unwrap_or_default());
    out
}

/// A universal binary whose slices are `slices`, in order.
pub fn fat(slices: &[Vec<u8>], cputypes: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    be(&mut out, &[0xcafe_babe, slices.len() as u32]);
    let mut at = 0x1000u32;
    for (slice, cputype) in slices.iter().zip(cputypes) {
        be(&mut out, &[*cputype, 3, at, slice.len() as u32, 12]);
        at += (slice.len() as u32).div_ceil(0x1000) * 0x1000;
    }
    for slice in slices {
        out.resize(out.len().div_ceil(0x1000) * 0x1000, 0);
        out.extend(slice);
    }
    out
}
