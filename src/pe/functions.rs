//! `bin.functions` for PE32 and PE32+ images: where each function starts and ends, its name when the image gives
//! one, and its cyclomatic complexity.
//!
//! Starts come from the `.pdata` table of a PE32+ image (which also gives the exact end), the entry point, the
//! exports, and from the direct calls and tail jumps found while walking. Without `.pdata` a function ends where
//! its walk ends, so its boundaries are a heuristic. The walk itself is in [`super::cfg`].
//!
//! The answer is a page of at most [`MAX_PAGE`] functions in RVA order, so it stays small however large the image
//! is; `next_offset` leads to the next page. Pages of one image are served from one analysis.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::Metadata;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use object::pe::{ImageNtHeaders32, ImageNtHeaders64};
use object::read::pe::{ExportTarget, ImageNtHeaders, ImageOptionalHeader, PeFile};
use object::{FileKind, LittleEndian as LE};
use serde::Serialize;

use super::cfg::{walk, Code, MAX_FUNCTION_INSTRUCTIONS};
use super::image::{hex, not_pe, read_file, stat};
use crate::error::CoreError;

/// The most functions in one answer.
pub const MAX_PAGE: usize = 48;
/// A longer name is cut to this many bytes.
pub const MAX_NAME_LEN: usize = 64;
/// The most functions found in one image.
pub const MAX_FUNCTIONS: usize = 200_000;
/// The most instructions walked in one image.
pub const MAX_IMAGE_INSTRUCTIONS: usize = 2_000_000;
/// An export table with more entries than the 16 bits of an ordinal can number is cut here.
const MAX_EXPORTS: usize = 65_536;
/// `IMAGE_DIRECTORY_ENTRY_EXCEPTION`: the `.pdata` table.
const EXCEPTION_DIRECTORY: usize = 3;
/// `IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE`.
const CODE_FLAGS: u32 = 0x20 | 0x2000_0000;
/// `UNW_FLAG_CHAININFO`: the entry is a fragment of the function that another entry describes.
const UNWIND_CHAINED: u8 = 0x4;

/// What stops the analysis of a hostile or huge image.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub function: usize,
    pub image: usize,
    pub functions: usize,
}

impl Default for Budget {
    fn default() -> Budget {
        Budget {
            function: MAX_FUNCTION_INSTRUCTIONS,
            image: MAX_IMAGE_INSTRUCTIONS,
            functions: MAX_FUNCTIONS,
        }
    }
}

/// What is known of a function before it is walked.
#[derive(Default)]
struct Seed {
    name: Option<String>,
    /// Where the image says it ends (`.pdata`).
    end: Option<u32>,
}

/// One function found in the image.
#[derive(Debug, PartialEq, Eq)]
pub struct Found {
    pub start: u32,
    pub end: u32,
    pub name: Option<String>,
    pub cyclomatic: u32,
    pub blocks: usize,
    pub instructions: usize,
}

/// Every function of one image, by RVA.
#[derive(Debug, PartialEq, Eq)]
pub struct Analysis {
    pub arch: &'static str,
    pub image_base: u64,
    pub functions: Vec<Found>,
    /// True when a budget cut the analysis short.
    pub truncated: bool,
}

/// A function as the answer lists it; addresses are hex strings so 64-bit values survive JSON.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Function {
    pub start: String,
    pub end: String,
    pub size: u32,
    pub name: Option<String>,
    pub cyclomatic: u32,
    /// The virtual address: the image base plus `rva`.
    pub address: String,
    pub rva: String,
    pub blocks: usize,
    pub instructions: usize,
}

/// What `bin.functions` answers.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Page {
    pub format: &'static str,
    pub arch: &'static str,
    /// The functions that pass the filter, over all pages.
    pub total: usize,
    pub offset: usize,
    /// Where the next page starts; `None` on the last page.
    pub next_offset: Option<usize>,
    pub truncated: bool,
    pub functions: Vec<Function>,
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_NAME_LEN)]).into_owned()
}

fn arch_of(machine: u16) -> Result<&'static str, CoreError> {
    match machine {
        0x14c => Ok("x86"),
        0x8664 => Ok("x86_64"),
        other => Err(CoreError::UnsupportedMachine(format!(
            "machine {other:#x}; only x86 and x86_64 are decoded"
        ))),
    }
}

fn is_code(sections: &[Code], rva: u32) -> bool {
    sections.iter().any(|section| section.contains(rva))
}

/// The executable sections, with their bytes. Raw data that lies outside the file is a truncated image.
fn code_sections<'a, H: ImageNtHeaders>(
    file: &PeFile<H>,
    data: &'a [u8],
    bitness: u32,
) -> Result<Vec<Code<'a>>, CoreError> {
    let mut sections = Vec::new();
    for header in file.section_table().iter() {
        if header.characteristics.get(LE) & CODE_FLAGS == 0 {
            continue;
        }
        let raw = header.pe_data(data).map_err(not_pe)?;
        let size = header.virtual_size.get(LE) as usize;
        sections.push(Code {
            rva: header.virtual_address.get(LE),
            bytes: &raw[..raw.len().min(if size == 0 { raw.len() } else { size })],
            bitness,
        });
    }
    Ok(sections)
}

/// Whether the unwind info at `rva` only continues another entry.
fn is_chained<H: ImageNtHeaders>(file: &PeFile<H>, data: &[u8], unwind: u32) -> bool {
    let at = file.section_table().pe_data_at(data, unwind & !1);
    at.and_then(|bytes| bytes.first())
        .is_some_and(|byte| (byte >> 3) & UNWIND_CHAINED != 0)
}

fn word(entry: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([entry[at], entry[at + 1], entry[at + 2], entry[at + 3]])
}

/// The functions the `.pdata` table of a PE32+ image lists, with their ends.
fn pdata_seeds<H: ImageNtHeaders>(file: &PeFile<H>, data: &[u8], seeds: &mut BTreeMap<u32, Seed>) {
    let table = file
        .data_directories()
        .get(EXCEPTION_DIRECTORY)
        .and_then(|directory| directory.data(data, &file.section_table()).ok());
    for entry in table.unwrap_or_default().as_chunks::<12>().0 {
        let (begin, end) = (word(entry, 0), word(entry, 4));
        if end > begin && !is_chained(file, data, word(entry, 8)) {
            seeds.entry(begin).or_default().end = Some(end);
        }
    }
}

/// The exported functions, named; a forwarder or an export of data is not one.
fn export_seeds<H: ImageNtHeaders>(
    file: &PeFile<H>,
    sections: &[Code],
    seeds: &mut BTreeMap<u32, Seed>,
) {
    let Ok(Some(table)) = file.export_table() else {
        return;
    };
    for export in table.exports().unwrap_or_default().iter().take(MAX_EXPORTS) {
        let ExportTarget::Address(rva) = export.target else {
            continue;
        };
        if is_code(sections, rva) {
            let seed = seeds.entry(rva).or_default();
            seed.name = seed.name.take().or(export.name.map(text));
        }
    }
}

/// Where the walk of the function at `start` must stop: its known end, or the next start, or the section's end.
fn limit_of(code: &Code, seeds: &BTreeMap<u32, Seed>, start: u32) -> u32 {
    let next = seeds
        .range(start.saturating_add(1)..)
        .next()
        .map_or(u32::MAX, |(at, _)| *at);
    seeds[&start].end.unwrap_or(next).min(code.end())
}

/// Walks every start and every start it finds, in RVA order, until done or a budget is spent.
fn discover(
    sections: &[Code],
    mut seeds: BTreeMap<u32, Seed>,
    budget: &Budget,
) -> (Vec<Found>, bool) {
    let mut todo: BTreeSet<u32> = seeds.keys().copied().collect();
    let (mut found, mut spent, mut truncated) = (BTreeMap::new(), 0, false);
    while let Some(start) = todo.pop_first() {
        if found.len() >= budget.functions || spent >= budget.image {
            truncated = true;
            break;
        }
        let Some(code) = sections.iter().find(|code| code.contains(start)) else {
            continue;
        };
        let limit = limit_of(code, &seeds, start);
        let walked = walk(
            code,
            start,
            limit,
            budget.function.min(budget.image - spent),
        );
        spent += walked.instructions;
        truncated |= walked.truncated;
        for target in walked.calls.iter().chain(&walked.tails) {
            if is_code(sections, *target) && !seeds.contains_key(target) {
                seeds.insert(*target, Seed::default());
                todo.insert(*target);
            }
        }
        let seed = &seeds[&start];
        found.insert(
            start,
            Found {
                start,
                end: seed.end.unwrap_or(walked.end),
                name: seed.name.clone(),
                cyclomatic: walked.cyclomatic,
                blocks: walked.blocks,
                instructions: walked.instructions,
            },
        );
    }
    (found.into_values().collect(), truncated)
}

fn analyse_with<H: ImageNtHeaders>(data: &[u8], budget: &Budget) -> Result<Analysis, CoreError> {
    let file = PeFile::<H>::parse(data).map_err(not_pe)?;
    let arch = arch_of(file.nt_headers().file_header().machine.get(LE))?;
    let plus = file.nt_headers().is_type_64();
    let sections = code_sections(&file, data, if plus { 64 } else { 32 })?;
    let optional = file.nt_headers().optional_header();
    let mut seeds = BTreeMap::new();
    if plus {
        pdata_seeds(&file, data, &mut seeds);
    }
    export_seeds(&file, &sections, &mut seeds);
    let entry = optional.address_of_entry_point();
    if is_code(&sections, entry) {
        seeds.entry(entry).or_default();
    }
    let (functions, truncated) = discover(&sections, seeds, budget);
    Ok(Analysis {
        arch,
        image_base: optional.image_base(),
        functions,
        truncated,
    })
}

/// The executable format of `data` when it is one this analysis does not read, from its magic number.
fn foreign_format(data: &[u8]) -> Option<&'static str> {
    match data.get(..4)? {
        b"\x7fELF" => Some("ELF"),
        [0xFE, 0xED, 0xFA, 0xCE | 0xCF] | [0xCE | 0xCF, 0xFA, 0xED, 0xFE] => Some("Mach-O"),
        _ => None,
    }
}

/// Finds the functions of the PE image in `data`. Another kind of file is [`CoreError::NotPe`], or
/// [`CoreError::UnsupportedFormat`] when it is an ELF or Mach-O image.
pub fn analyse(data: &[u8], budget: &Budget) -> Result<Analysis, CoreError> {
    match FileKind::parse(data) {
        Ok(FileKind::Pe32) => analyse_with::<ImageNtHeaders32>(data, budget),
        Ok(FileKind::Pe64) => analyse_with::<ImageNtHeaders64>(data, budget),
        _ => Err(foreign_format(data).map_or_else(
            || not_pe("no MZ and PE signatures, or the headers point outside the file"),
            |kind| CoreError::UnsupportedFormat(format!("{kind} images are not analysed yet")),
        )),
    }
}

fn listed(found: &Found, image_base: u64) -> Function {
    Function {
        start: hex(found.start.into()),
        end: hex(found.end.into()),
        size: found.end.saturating_sub(found.start),
        name: found.name.clone(),
        cyclomatic: found.cyclomatic,
        address: hex(image_base.wrapping_add(found.start.into())),
        rva: hex(found.start.into()),
        blocks: found.blocks,
        instructions: found.instructions,
    }
}

/// The page of `analysis` that starts at `offset`, over the functions of at least `min_cyclomatic`.
pub fn page(analysis: &Analysis, offset: usize, min_cyclomatic: u32) -> Page {
    let kept: Vec<&Found> = analysis
        .functions
        .iter()
        .filter(|found| found.cyclomatic >= min_cyclomatic)
        .collect();
    let functions: Vec<Function> = kept
        .iter()
        .skip(offset)
        .take(MAX_PAGE)
        .map(|found| listed(found, analysis.image_base))
        .collect();
    let next = offset.saturating_add(functions.len());
    Page {
        format: "pe",
        arch: analysis.arch,
        total: kept.len(),
        offset,
        next_offset: (next < kept.len()).then_some(next),
        truncated: analysis.truncated,
        functions,
    }
}

/// What identifies a file's content for the memo: its path, size and modification time.
type Key = (String, u64, Option<SystemTime>);

/// Remembers the analysis of the last image, so the pages of one image cost one analysis.
pub struct Memo {
    slot: Mutex<Option<(Key, Arc<Analysis>)>>,
}

impl Memo {
    pub const fn new() -> Memo {
        Memo {
            slot: Mutex::new(None),
        }
    }

    /// The analysis remembered for `key`, or the one `load` makes (and remembers).
    pub fn fetch(
        &self,
        key: Key,
        load: impl FnOnce() -> Result<Analysis, CoreError>,
    ) -> Result<Arc<Analysis>, CoreError> {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((known, analysis)) = slot.as_ref() {
            if *known == key {
                return Ok(Arc::clone(analysis));
            }
        }
        let analysis = Arc::new(load()?);
        *slot = Some((key, Arc::clone(&analysis)));
        Ok(analysis)
    }
}

impl Default for Memo {
    fn default() -> Memo {
        Memo::new()
    }
}

static MEMO: Memo = Memo::new();

fn key_of(path: &str, meta: &Metadata) -> Key {
    (path.to_string(), meta.len(), meta.modified().ok())
}

/// The page of the functions of the PE image at `path` that starts at `offset`.
pub fn functions_file(path: &str, offset: usize, min_cyclomatic: u32) -> Result<Page, CoreError> {
    let key = key_of(path, &stat(path)?);
    let analysis = MEMO.fetch(key, || analyse(&read_file(path)?, &Budget::default()))?;
    Ok(page(&analysis, offset, min_cyclomatic))
}

#[cfg(test)]
#[path = "functions_tests.rs"]
mod tests;
