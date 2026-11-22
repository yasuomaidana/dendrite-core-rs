use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use rstest::{fixture, rstest};
use serde_json::json;

use super::*;
use crate::pe::cfg::tests::{assemble, IF_ELSE, LEAF, LOOP_IF};
use crate::pe::fixture::{build, write_temp, Spec};
use crate::tokens::estimate;

/// `call 0x1050; call 0x1060; ret`
const MAIN: &str = "E84B000000 E856000000 C3";
/// `jmp 0x1070`: a tail call.
const TAIL: &str = "E90B000000";

/// The entry point calls the functions at 0x50 and 0x60, which jumps to the one at 0x70; 0x10 is exported as
/// `Branchy` and 0x30 is only known from `.pdata`.
fn code() -> Vec<u8> {
    let mut code = vec![0xCC; 0x80];
    for (at, hex) in [
        (0x00, MAIN),
        (0x10, IF_ELSE),
        (0x30, LOOP_IF),
        (0x50, LEAF),
        (0x60, TAIL),
        (0x70, LEAF),
    ] {
        let bytes = assemble(hex);
        code[at..at + bytes.len()].copy_from_slice(&bytes);
    }
    code
}

fn spec(plus: bool) -> Spec {
    Spec {
        code: code(),
        exports: vec![(Some("Branchy".into()), 0x1010)],
        ..Spec::new(plus)
    }
}

/// `.pdata` knows 0x30 and where it ends, a fragment at 0x48 that belongs to another function, and where 0x60
/// ends, which makes its jump to 0x70 a tail call. Without `.pdata` a forward jump into code that no one has named
/// is taken for a branch inside the function.
#[fixture]
fn pe64() -> Vec<u8> {
    build(&Spec {
        pdata: vec![
            (0x1030, 0x1041, false),
            (0x1048, 0x1050, true),
            (0x1060, 0x1065, false),
        ],
        ..spec(true)
    })
}

type Summary = (u32, u32, Option<&'static str>, u32);

fn summary(analysis: &Analysis) -> Vec<(u32, u32, Option<String>, u32)> {
    let owned = |found: &Found| (found.start, found.end, found.name.clone(), found.cyclomatic);
    analysis.functions.iter().map(owned).collect()
}

fn expected(rows: &[Summary]) -> Vec<(u32, u32, Option<String>, u32)> {
    let owned = |(start, end, name, cc): &Summary| (*start, *end, name.map(String::from), *cc);
    rows.iter().map(owned).collect()
}

#[rstest]
fn a_pe64_image_lists_its_functions_in_rva_order_with_their_complexity(pe64: Vec<u8>) {
    let analysis = analyse(&pe64, &Budget::default()).unwrap();
    assert_eq!(
        summary(&analysis),
        expected(&[
            (0x1000, 0x100B, None, 1),
            (0x1010, 0x1021, Some("Branchy"), 2),
            (0x1030, 0x1041, None, 3),
            (0x1050, 0x1053, None, 1),
            (0x1060, 0x1065, None, 1),
            (0x1070, 0x1073, None, 1),
        ])
    );
    assert_eq!(
        (analysis.arch, analysis.image_base),
        ("x86_64", 0x1_4000_0000)
    );
    assert!(!analysis.truncated);
}

#[test]
fn a_pe32_image_without_pdata_finds_what_is_exported_or_called() {
    let analysis = analyse(&build(&spec(false)), &Budget::default()).unwrap();
    let starts: Vec<u32> = analysis.functions.iter().map(|found| found.start).collect();
    assert_eq!(starts, [0x1000, 0x1010, 0x1050, 0x1060]);
    assert_eq!((analysis.arch, analysis.image_base), ("x86", 0x40_0000));
}

#[test]
fn a_start_outside_the_code_is_ignored() {
    let image = build(&Spec {
        code: assemble(LEAF),
        pdata: vec![(0x5000, 0x5010, false), (0x1000, 0x1003, false)],
        ..Spec::new(true)
    });
    let analysis = analyse(&image, &Budget::default()).unwrap();
    assert_eq!(summary(&analysis), expected(&[(0x1000, 0x1003, None, 1)]));
}

/// The export table is the only table, so it starts at 0x2000: a 40-byte header, two addresses, two name pointers
/// and two ordinals, then the strings, `Branchy` (8 bytes with its NUL) at 0x203C and the forwarder at 0x2044. An
/// address that points into the table is a forward to another DLL, which is not code of this image.
#[test]
fn a_forwarded_export_is_not_a_function() {
    let image = build(&Spec {
        code: assemble(LEAF),
        exports: vec![
            (Some("Branchy".into()), 0x1000),
            (Some("KERNEL32.Sleep".into()), 0x2044),
        ],
        ..Spec::new(true)
    });
    let analysis = analyse(&image, &Budget::default()).unwrap();
    assert_eq!(
        summary(&analysis),
        expected(&[(0x1000, 0x1003, Some("Branchy"), 1)])
    );
}

#[rstest]
fn the_page_carries_both_addresses_and_the_sizes(pe64: Vec<u8>) {
    let page = page(&analyse(&pe64, &Budget::default()).unwrap(), 0, 1);
    assert_eq!((page.format, page.arch, page.total), ("pe", "x86_64", 6));
    assert_eq!(page.next_offset, None);
    assert_eq!(
        serde_json::to_value(&page.functions[1]).unwrap(),
        json!({"start": "0x1010", "end": "0x1021", "size": 17, "name": "Branchy", "cyclomatic": 2,
               "address": "0x140001010", "rva": "0x1010", "blocks": 4, "instructions": 6, "partial": false})
    );
}

#[rstest]
#[case::inside_the_dos_header(0x20)]
#[case::inside_the_optional_header(0x90)]
#[case::inside_the_code(0x280)]
fn a_truncated_image_is_not_a_pe_image(pe64: Vec<u8>, #[case] keep: usize) {
    assert!(matches!(
        analyse(&pe64[..keep], &Budget::default()),
        Err(CoreError::NotPe(_))
    ));
}

#[test]
fn other_files_are_refused_with_the_kind_that_says_why() {
    let budget = Budget::default();
    let arm = build(&Spec {
        machine: 0xaa64,
        ..spec(true)
    });
    assert!(matches!(
        analyse(b"hello", &budget),
        Err(CoreError::NotPe(_))
    ));
    assert!(matches!(
        analyse(&arm, &budget),
        Err(CoreError::UnsupportedMachine(_))
    ));
}

#[rstest]
#[case::elf(b"\x7fELF\x02\x01\x01\0\0\0\0\0")]
#[case::macho_64(b"\xCF\xFA\xED\xFE\x07\0\0\x01")]
#[case::macho_32_big_endian(b"\xFE\xED\xFA\xCE\0\0\0\x07")]
fn an_elf_or_macho_image_is_an_unsupported_format(#[case] bytes: &[u8]) {
    assert!(matches!(
        analyse(bytes, &Budget::default()),
        Err(CoreError::UnsupportedFormat(_))
    ));
}

#[rstest]
#[case::per_function(Budget { function: 1, ..Budget::default() })]
#[case::per_image(Budget { image: 4, ..Budget::default() })]
#[case::functions(Budget { functions: 2, ..Budget::default() })]
fn a_spent_budget_cuts_the_analysis_and_says_so(pe64: Vec<u8>, #[case] budget: Budget) {
    let analysis = analyse(&pe64, &budget).unwrap();
    assert!(analysis.truncated);
    assert!(analysis.functions.len() < 6 || budget.function == 1);
}

#[rstest]
fn a_function_cut_by_its_budget_is_partial(pe64: Vec<u8>) {
    let budget = Budget {
        function: 1,
        ..Budget::default()
    };
    let analysis = analyse(&pe64, &budget).unwrap();
    let cut: Vec<u32> = analysis
        .functions
        .iter()
        .filter(|found| found.partial)
        .map(|found| found.start)
        .collect();
    // The function at 0x1060 is one `jmp`, which the budget of one instruction covers.
    assert_eq!(cut, [0x1000, 0x1010, 0x1030, 0x1050, 0x1070]);
    let whole = analyse(&pe64, &Budget::default()).unwrap();
    assert!(whole.functions.iter().all(|found| !found.partial));
}

fn many(count: usize, name: Option<String>) -> Analysis {
    let found = |i: usize| Found {
        start: 0x1000 + 0x10 * i as u32,
        end: 0x1008 + 0x10 * i as u32,
        name: name.clone(),
        cyclomatic: 1 + (i % 3) as u32,
        blocks: 1,
        instructions: 2,
        partial: false,
    };
    Analysis {
        arch: "x86_64",
        image_base: 0x1_4000_0000,
        functions: (0..count).map(found).collect(),
        truncated: false,
    }
}

#[rstest]
#[case::first(0, 1, MAX_PAGE, Some(MAX_PAGE))]
#[case::middle(MAX_PAGE, 1, MAX_PAGE, Some(2 * MAX_PAGE))]
#[case::last(2 * MAX_PAGE, 1, 4, None)]
#[case::past_the_end(1_000, 1, 0, None)]
#[case::filtered(0, 3, 33, None)]
fn pages_walk_the_whole_list(
    #[case] offset: usize,
    #[case] min: u32,
    #[case] len: usize,
    #[case] next: Option<usize>,
) {
    let page = page(&many(2 * MAX_PAGE + 4, None), offset, min);
    assert_eq!((page.functions.len(), page.next_offset), (len, next));
    assert_eq!(page.total, if min == 1 { 2 * MAX_PAGE + 4 } else { 33 });
}

#[test]
fn a_full_page_of_the_longest_names_stays_under_4000_tokens() {
    let name = "n".repeat(MAX_NAME_LEN);
    let page = page(&many(MAX_PAGE, Some(name)), 0, 1);
    let tokens = estimate(&serde_json::to_string(&page).unwrap());
    assert_eq!(page.functions.len(), MAX_PAGE);
    assert!(tokens < 4_000, "{tokens} tokens");
    assert_eq!(text(&[b'x'; 200]).len(), MAX_NAME_LEN);
}

#[test]
fn the_memo_loads_an_image_once_and_loads_again_when_the_file_changes() {
    let (memo, loads) = (Memo::new(), AtomicUsize::new(0));
    let key = |len: u64| ("a.exe".to_string(), len, None);
    let load = || {
        loads.fetch_add(1, Ordering::SeqCst);
        Ok(many(1, None))
    };
    let first = memo.fetch(key(1), load).unwrap();
    let again = memo.fetch(key(1), load).unwrap();
    assert!(Arc::ptr_eq(&first, &again));
    memo.fetch(key(2), load).unwrap();
    assert_eq!(loads.load(Ordering::SeqCst), 2);
    let failed = memo.fetch(key(3), || Err(CoreError::NotPe("x".into())));
    assert!(failed.is_err());
    assert!(Memo::default().fetch(key(1), load).is_ok());
}

#[rstest]
fn functions_are_read_from_a_file_and_served_page_by_page(pe64: Vec<u8>) {
    let path = write_temp("functions", &pe64);
    let path_text = path.to_str().unwrap();
    let (first, second) = (
        functions_file(path_text, 0, 1).unwrap(),
        functions_file(path_text, 0, 1).unwrap(),
    );
    assert_eq!((first.total, first.functions.len()), (6, 6));
    assert_eq!(first, second);
    assert_eq!(functions_file(path_text, 0, 3).unwrap().total, 1);
    fs::remove_file(&path).unwrap();
    assert!(matches!(
        functions_file(path_text, 0, 1),
        Err(CoreError::NotPe(_))
    ));
}
