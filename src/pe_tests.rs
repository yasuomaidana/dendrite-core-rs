use std::fs::{self, File};

use rstest::rstest;
use serde_json::{json, Value};

use super::fixture::{build, directory_offset, temp_path, write_temp, Spec};
use super::*;

fn dll(name: &str, functions: &[&str]) -> (String, Vec<String>) {
    (
        name.to_string(),
        functions.iter().map(ToString::to_string).collect(),
    )
}

fn full_spec(plus: bool) -> Spec {
    let mut spec = Spec::new(plus);
    spec.imports = vec![dll("KERNEL32.dll", &["CreateFileW", "ReadFile", "#7"])];
    spec.exports = vec![
        (Some("ComputeChecksum".into()), 0x1000),
        (None, 0x1000),
        (None, 0),
    ];
    spec
}

fn image() -> Vec<u8> {
    build(&full_spec(true))
}

fn cut(len: usize) -> Vec<u8> {
    image()[..len].to_vec()
}

fn patched(at: usize, bytes: &[u8]) -> Vec<u8> {
    let mut data = image();
    data[at..at + bytes.len()].copy_from_slice(bytes);
    data
}

fn to_json(spec: &Spec) -> Value {
    serde_json::to_value(parse_bytes(&build(spec)).unwrap()).unwrap()
}

#[rstest]
#[case::pe64(true, "x86_64", "0x140000000", "0x140001000")]
#[case::pe32(false, "x86", "0x400000", "0x401000")]
fn an_image_is_described(
    #[case] plus: bool,
    #[case] machine: &str,
    #[case] image_base: &str,
    #[case] entry_point: &str,
) {
    let info = to_json(&full_spec(plus));
    assert_eq!(info["machine"], machine);
    assert_eq!(info["subsystem"], "console");
    assert_eq!(info["pe32_plus"], plus);
    assert_eq!(info["image_base"], image_base);
    assert_eq!(info["entry_point"], entry_point);
    assert_eq!(info["truncated"], false);
    assert_eq!(
        info["sections"][0],
        json!({"name": ".text", "virtual_address": "0x1000", "virtual_size": 512, "raw_size": 512, "flags": ["code", "execute", "read"]})
    );
    assert_eq!(info["sections"][1]["name"], ".rdata");
    assert_eq!(
        info["sections"][1]["flags"],
        json!(["initialized_data", "read"])
    );
    assert_eq!(
        info["imports"],
        json!([{"dll": "KERNEL32.dll", "functions": ["CreateFileW", "ReadFile", "#7"]}])
    );
    assert_eq!(info["exports"], json!(["ComputeChecksum", "#2"]));
}

#[rstest]
#[case::i386(0x14c, 2, "x86", "gui")]
#[case::arm(0x1c0, 1, "arm", "native")]
#[case::arm64(0xaa64, 10, "arm64", "efi_application")]
#[case::unknown(0x1234, 99, "0x1234", "0x63")]
fn machines_and_subsystems_are_named_or_given_in_hex(
    #[case] machine: u16,
    #[case] subsystem: u16,
    #[case] want_machine: &str,
    #[case] want_subsystem: &str,
) {
    let mut spec = Spec::new(true);
    (spec.machine, spec.subsystem) = (machine, subsystem);
    let info = to_json(&spec);
    assert_eq!(
        (&info["machine"], &info["subsystem"]),
        (&json!(want_machine), &json!(want_subsystem))
    );
}

#[test]
fn an_image_without_imports_or_exports_has_empty_lists() {
    let info = to_json(&Spec::new(true));
    assert_eq!(
        (&info["imports"], &info["exports"], &info["truncated"]),
        (&json!([]), &json!([]), &json!(false))
    );
    assert_eq!(info["sections"].as_array().unwrap().len(), 1);
}

fn spec_with(extra_sections: usize, imports: Vec<(String, Vec<String>)>, exports: usize) -> Spec {
    let mut spec = Spec::new(true);
    spec.extra_sections = extra_sections;
    spec.imports = imports;
    spec.exports = (0..exports)
        .map(|i| (Some(format!("f{i}")), 0x1000))
        .collect();
    spec
}

/// (DLLs, functions of the first DLL, exports, sections) of what was answered.
fn counts(info: &PeInfo) -> (usize, usize, usize, usize) {
    let first = info.imports.first().map_or(0, |dll| dll.functions.len());
    (
        info.imports.len(),
        first,
        info.exports.len(),
        info.sections.len(),
    )
}

fn many_functions() -> Vec<(String, Vec<String>)> {
    vec![("a.dll".into(), (0..600).map(|i| format!("f{i}")).collect())]
}

#[rstest]
#[case::functions(spec_with(0, many_functions(), 0), (1, 499, 0, 2))]
#[case::dlls(spec_with(0, (0..600).map(|i| (format!("d{i}.dll"), vec![])).collect(), 0), (500, 0, 0, 2))]
#[case::exports(spec_with(0, vec![], 600), (0, 0, 500, 2))]
#[case::sections(spec_with(100, vec![], 0), (0, 0, 0, 96))]
#[case::long_name(spec_with(0, vec![("a.dll".into(), vec!["x".repeat(300)])], 0), (1, 1, 0, 2))]
#[case::export_entries(spec_with(0, vec![], 65_537), (0, 0, 0, 2))]
fn the_answer_is_capped_and_says_so(
    #[case] spec: Spec,
    #[case] want: (usize, usize, usize, usize),
) {
    let info = parse_bytes(&build(&spec)).unwrap();
    assert_eq!(counts(&info), want);
    assert!(info.truncated);
}

#[test]
fn a_name_is_cut_to_the_cap() {
    let spec = spec_with(0, vec![("a.dll".into(), vec!["x".repeat(300)])], 0);
    assert_eq!(
        parse_bytes(&build(&spec)).unwrap().imports[0].functions[0].len(),
        MAX_NAME_LEN
    );
}

#[rstest]
#[case::imports(1)]
#[case::exports(0)]
fn a_damaged_table_keeps_the_rest_of_the_answer(#[case] directory: usize) {
    let at = directory_offset(true, directory);
    let info = parse_bytes(&patched(at, &0x9000_0000u32.to_le_bytes())).unwrap();
    assert_eq!(
        (info.imports.is_empty(), info.exports.is_empty()),
        (directory == 1, directory == 0)
    );
    assert_eq!(info.sections.len(), 2);
}

#[rstest]
#[case::empty(vec![])]
#[case::shorter_than_a_dos_header(b"MZ".to_vec())]
#[case::not_a_pe(b"just some text, not an executable at all".to_vec())]
#[case::no_pe_signature(patched(0x40, b"XE"))]
#[case::cut_in_the_optional_header(cut(0x40 + 24 + 30))]
#[case::cut_before_the_section_table(cut(0x40 + 24 + 240))]
#[case::cut_in_the_section_table(cut(0x40 + 24 + 240 + 50))]
#[case::section_table_past_the_end(patched(0x40 + 6, &u16::MAX.to_le_bytes()))]
fn what_is_not_a_pe_image_is_refused(#[case] data: Vec<u8>) {
    assert!(matches!(parse_bytes(&data), Err(CoreError::NotPe(_))));
}

#[test]
fn a_file_is_read_by_its_path() {
    let path = write_temp("ok", &image());
    let info = parse_file(path.to_str().unwrap());
    fs::remove_file(&path).unwrap();
    assert_eq!(info.unwrap().machine, "x86_64");
}

#[rstest]
#[case::missing("missing")]
#[case::directory("directory")]
#[case::too_large("large")]
fn a_path_that_cannot_be_read_is_refused(#[case] kind: &str) {
    let path = temp_path(kind);
    match kind {
        "directory" => fs::create_dir(&path).unwrap(),
        "large" => File::create(&path)
            .unwrap()
            .set_len(MAX_FILE_BYTES + 1)
            .unwrap(),
        _ => {}
    }
    let result = parse_file(path.to_str().unwrap());
    let _ = fs::remove_file(&path).or_else(|_| fs::remove_dir(&path));
    assert!(matches!(result, Err(CoreError::NotPe(_))));
}
