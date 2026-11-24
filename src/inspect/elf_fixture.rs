//! Hand-built ELF images for the tests: a program header table, a dynamic section, dynamic symbols and, when asked
//! for, GNU symbol versions. Class and byte order are parameters, so every code path runs for all four combinations.

/// What the image holds.
#[derive(Clone)]
pub struct Spec {
    pub wide: bool,
    pub big: bool,
    pub e_type: u16,
    pub needed: Vec<&'static str>,
    /// `printf` carries a version requirement from the first needed library.
    pub verneed: bool,
    pub interp: bool,
    pub sections: bool,
}

impl Spec {
    /// A 64-bit little-endian program that needs libc and libm and has an interpreter.
    pub fn program() -> Self {
        Spec {
            wide: true,
            big: false,
            e_type: 3,
            needed: vec!["libc.so.6", "libm.so.6"],
            verneed: true,
            interp: true,
            sections: true,
        }
    }
}

const TEXT_ADDR: u64 = 0x40_1000;
const INTERP: &[u8] = b"/lib64/ld-linux-x86-64.so.2\0";
/// The strings after the needed libraries, in this order.
const STRINGS: [&str; 6] = [
    "$ORIGIN",
    "mylib.so",
    "printf",
    "pow",
    "check_flag",
    "GLIBC_2.2.5",
];

/// A byte buffer that writes numbers in the byte order and word size of the image.
#[derive(Clone, Copy)]
struct Order {
    big: bool,
    wide: bool,
}

struct Out {
    bytes: Vec<u8>,
    order: Order,
}

impl Out {
    fn new(order: Order) -> Self {
        Out {
            bytes: Vec::new(),
            order,
        }
    }

    fn put(&mut self, value: u64, size: usize) {
        let le = value.to_le_bytes();
        let part = &le[..size];
        if self.order.big {
            self.bytes.extend(part.iter().rev());
        } else {
            self.bytes.extend(part);
        }
    }

    fn u16(&mut self, v: u16) {
        self.put(v.into(), 2);
    }

    fn u32(&mut self, v: u32) {
        self.put(v.into(), 4);
    }

    /// An address or size: 8 bytes in a 64-bit image, 4 in a 32-bit one.
    fn word(&mut self, v: u64) {
        self.put(v, if self.order.wide { 8 } else { 4 });
    }

    fn words(&mut self, values: &[u64]) {
        values.iter().for_each(|&v| self.word(v));
    }

    fn u32s(&mut self, values: &[u32]) {
        values.iter().for_each(|&v| self.u32(v));
    }

    fn pad_to(&mut self, multiple: usize) {
        self.bytes
            .resize(self.bytes.len().div_ceil(multiple) * multiple, 0);
    }
}

/// A string table of `names` and the offset of each.
fn strtab(names: &[&str]) -> (Vec<u8>, Vec<u32>) {
    let (mut table, mut at) = (vec![0u8], Vec::new());
    for name in names {
        at.push(table.len() as u32);
        table.extend(name.as_bytes());
        table.push(0);
    }
    (table, at)
}

/// One section: its name, header fields and contents.
struct Part {
    name: &'static str,
    kind: u32,
    flags: u64,
    link: u32,
    info: u32,
    entsize: u64,
    data: Vec<u8>,
}

fn part(
    name: &'static str,
    kind: u32,
    flags: u64,
    (link, info): (u32, u32),
    data: Vec<u8>,
) -> Part {
    Part {
        name,
        kind,
        flags,
        link,
        info,
        entsize: 0,
        data,
    }
}

/// `.dynsym`: nothing, `printf` and `pow` undefined, and `check_flag` defined in `.text` (section 1).
fn dynsym(order: Order, at: &[u32]) -> Vec<u8> {
    let mut out = Out::new(order);
    for (name, info, shndx) in [
        (0, 0, 0),
        (at[2], 0x12, 0),
        (at[3], 0x12, 0),
        (at[4], 0x12, 1),
    ] {
        let value = if shndx == 0 { 0 } else { TEXT_ADDR };
        out.u32(name);
        if order.wide {
            out.bytes.extend([info, 0]);
            out.u16(shndx);
            out.words(&[value, 0]);
        } else {
            out.words(&[value, 0]);
            out.bytes.extend([info, 0]);
            out.u16(shndx);
        }
    }
    out.bytes
}

/// `.dynamic`: a `DT_NEEDED` per library, then `DT_RUNPATH`, `DT_SONAME` and the end.
fn dynamic(order: Order, needed: usize, at: &[u32]) -> Vec<u8> {
    let mut out = Out::new(order);
    let tags =
        (0..needed)
            .map(|i| (1, at[i]))
            .chain([(29, at[needed]), (14, at[needed + 1]), (0, 0)]);
    for (tag, value) in tags {
        out.words(&[tag, value.into()]);
    }
    out.bytes
}

/// `.gnu.version` gives `printf` version index 2; `.gnu.version_r` says index 2 is `GLIBC_2.2.5` of the first library.
fn versions(order: Order, first: u32, glibc: u32) -> [Part; 2] {
    let mut versym = Out::new(order);
    [0u16, 2, 1, 1].iter().for_each(|&v| versym.u16(v));
    let mut need = Out::new(order);
    [1u16, 1].iter().for_each(|&v| need.u16(v));
    need.u32s(&[first, 16, 0, 0]);
    need.u16(0);
    need.u16(2);
    need.u32s(&[glibc, 0]);
    let mut version = part(".gnu.version", 0x6fff_ffff, 2, (2, 0), versym.bytes);
    version.entsize = 2;
    [
        version,
        part(".gnu.version_r", 0x6fff_fffe, 2, (3, 1), need.bytes),
    ]
}

fn parts(spec: &Spec, order: Order) -> Vec<Part> {
    let mut names = spec.needed.clone();
    names.extend(STRINGS);
    let (dynstr, at) = strtab(&names);
    let n = spec.needed.len();
    let entry = if order.wide { 24 } else { 16 };
    let mut dynsym_part = part(".dynsym", 11, 2, (3, 1), dynsym(order, &at[n..]));
    dynsym_part.entsize = entry;
    let mut dynamic_part = part(".dynamic", 6, 3, (3, 0), dynamic(order, n, &at));
    dynamic_part.entsize = if order.wide { 16 } else { 8 };
    let mut parts = vec![
        part(".text", 1, 6, (0, 0), vec![0xc3; 16]),
        dynsym_part,
        part(".dynstr", 3, 2, (0, 0), dynstr),
        dynamic_part,
    ];
    if spec.verneed {
        parts.extend(versions(order, at[0], at[n + 5]));
    }
    parts
}

/// A `PT_LOAD`, a `PT_DYNAMIC` and, when there is one, a `PT_INTERP`.
fn program_headers(out: &mut Out, interp_at: Option<u64>) {
    let mut headers = vec![
        (1u32, 5u32, 0u64, TEXT_ADDR - 0x1000, 0x2000u64),
        (2, 3, 0, 0, 0),
    ];
    if let Some(at) = interp_at {
        headers.push((3, 4, at, 0, INTERP.len() as u64));
    }
    for (kind, flags, offset, vaddr, size) in headers {
        out.u32(kind);
        if out.order.wide {
            out.u32(flags);
        }
        out.words(&[offset, vaddr, vaddr, size, size]);
        if !out.order.wide {
            out.u32(flags);
        }
        out.word(0x1000);
    }
}

fn section_headers(out: &mut Out, body: &[Part], names_at: &[u32], offsets: &[u64]) {
    out.bytes
        .resize(out.bytes.len() + if out.order.wide { 64 } else { 40 }, 0);
    for (i, part) in body.iter().enumerate() {
        let addr = if part.name == ".text" { TEXT_ADDR } else { 0 };
        out.u32s(&[names_at[i], part.kind]);
        out.words(&[part.flags, addr, offsets[i], part.data.len() as u64]);
        out.u32s(&[part.link, part.info]);
        out.words(&[8, part.entsize]);
    }
}

/// The ELF header; the section header offset is written later, at the returned position.
fn header(out: &mut Out, spec: &Spec, sizes: (u16, u16), counts: (u16, u16, u16)) -> usize {
    let (ehsize, phsize) = sizes;
    let class = if spec.wide { 2 } else { 1 };
    out.bytes.extend([
        0x7f,
        b'E',
        b'L',
        b'F',
        class,
        if spec.big { 2 } else { 1 },
        1,
        0,
    ]);
    out.bytes.resize(16, 0);
    out.u16(spec.e_type);
    out.u16(if spec.wide { 62 } else { 3 });
    out.u32(1);
    out.words(&[TEXT_ADDR, ehsize.into()]);
    let shoff_at = out.bytes.len();
    out.word(0);
    out.u32(0);
    let (phnum, shnum, shstrndx) = counts;
    let shsize = if spec.wide { 64 } else { 40 };
    [ehsize, phsize, phnum, shsize, shnum, shstrndx]
        .iter()
        .for_each(|&v| out.u16(v));
    shoff_at
}

/// The image described by `spec`.
pub fn build(spec: &Spec) -> Vec<u8> {
    let order = Order {
        big: spec.big,
        wide: spec.wide,
    };
    let mut out = Out::new(order);
    let (ehsize, phsize) = if spec.wide { (64u16, 56u16) } else { (52, 32) };
    let phnum = 2 + u16::from(spec.interp);
    let mut body = parts(spec, order);
    let (shstr, names_at) = strtab(
        &body
            .iter()
            .map(|p| p.name)
            .chain([".shstrtab"])
            .collect::<Vec<_>>(),
    );
    body.push(part(".shstrtab", 3, 0, (0, 0), shstr));
    let count = if spec.sections { body.len() as u16 } else { 0 };
    let shoff_at = header(
        &mut out,
        spec,
        (ehsize, phsize),
        (phnum, count + u16::from(count > 0), count),
    );
    let interp_at = (ehsize + phsize * phnum) as u64;
    program_headers(&mut out, spec.interp.then_some(interp_at));
    if spec.interp {
        out.bytes.extend(INTERP);
    }
    let mut offsets = Vec::new();
    for part in &body {
        out.pad_to(8);
        offsets.push(out.bytes.len() as u64);
        out.bytes.extend(&part.data);
    }
    out.pad_to(8);
    if spec.sections {
        let shoff = out.bytes.len() as u64;
        section_headers(&mut out, &body, &names_at, &offsets);
        let mut at = Out::new(order);
        at.word(shoff);
        out.bytes[shoff_at..shoff_at + at.bytes.len()].copy_from_slice(&at.bytes);
    }
    out.bytes
}
