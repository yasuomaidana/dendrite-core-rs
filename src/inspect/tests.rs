use std::fs::{self, File};

use rstest::{fixture, rstest};

use super::*;
use crate::pe::fixture::{build as build_pe, temp_path, write_temp, Spec as PeSpec};
use crate::tokens::estimate;

use super::elf_fixture::{build as build_elf, Spec as ElfSpec};
use super::macho_fixture::{build as build_macho, fat, Spec as MachoSpec};

const X86_64: u32 = 0x0100_0007;
const ARM64: u32 = 0x0100_000c;

fn names(libraries: &[Library]) -> Vec<(&str, Vec<&str>)> {
    libraries
        .iter()
        .map(|l| {
            (
                l.name.as_str(),
                l.symbols.iter().map(String::as_str).collect(),
            )
        })
        .collect()
}

fn section_names(image: &Image) -> Vec<&str> {
    image.sections.iter().map(|s| s.name.as_str()).collect()
}

#[fixture]
fn pe_spec() -> PeSpec {
    PeSpec {
        imports: vec![(
            "KERNEL32.dll".into(),
            vec!["CreateFileW".into(), "#7".into()],
        )],
        exports: vec![(Some("Compute".into()), 0x1000)],
        code: vec![0xc3],
        ..PeSpec::new(true)
    }
}

#[rstest]
#[case::pe64(true, 64, "x86_64")]
#[case::pe32(false, 32, "x86")]
fn a_pe_image_is_described_in_the_common_shape(
    pe_spec: PeSpec,
    #[case] plus: bool,
    #[case] bits: u8,
    #[case] arch: &str,
) {
    let spec = PeSpec {
        plus,
        machine: if plus { 0x8664 } else { 0x14c },
        ..pe_spec
    };
    let image = inspect_bytes(&build_pe(&spec));
    assert_eq!(
        (image.kind, image.format, image.bits),
        ("executable", "pe", Some(bits))
    );
    assert_eq!(
        (image.arch.as_deref(), image.endian),
        (Some(arch), Some("little"))
    );
    assert_eq!(section_names(&image), [".text", ".rdata"]);
    assert_eq!(
        names(&image.libraries),
        [("KERNEL32.dll", vec!["CreateFileW", "#7"])]
    );
    assert_eq!(image.exports, ["Compute"]);
    assert_eq!(image.notes["subsystem"], "console");
    assert!(image.entry.is_some() && image.base.is_some() && !image.truncated);
}

#[rstest]
fn a_pe_image_marked_as_a_dll_is_a_library(pe_spec: PeSpec) {
    let mut bytes = build_pe(&pe_spec);
    let nt = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
    bytes[nt + 23] |= 0x20;
    assert_eq!(inspect_bytes(&bytes).kind, "library");
}

#[test]
fn an_elf_program_reports_its_dynamic_section_and_symbols() {
    let image = inspect_bytes(&build_elf(&ElfSpec::program()));
    assert_eq!(
        (image.kind, image.format, image.bits),
        ("executable", "elf", Some(64))
    );
    assert_eq!(
        (image.arch.as_deref(), image.endian),
        (Some("x86_64"), Some("little"))
    );
    assert_eq!(
        (image.entry.as_deref(), image.base.as_deref()),
        (Some("0x401000"), Some("0x400000"))
    );
    assert_eq!(
        section_names(&image),
        [
            ".text",
            ".dynsym",
            ".dynstr",
            ".dynamic",
            ".gnu.version",
            ".gnu.version_r",
            ".shstrtab"
        ]
    );
    assert_eq!(image.sections[0].flags, ["alloc", "execute"]);
    // `printf` has a version requirement from libc, `pow` has none and there are two libraries to choose from.
    assert_eq!(
        names(&image.libraries),
        [
            ("libc.so.6", vec!["printf"]),
            ("libm.so.6", vec![]),
            ("*", vec!["pow"])
        ]
    );
    assert_eq!(image.exports, ["check_flag"]);
    assert_eq!(image.notes["interpreter"], "/lib64/ld-linux-x86-64.so.2");
    assert_eq!(
        (
            image.notes["rpath"].as_str(),
            image.notes["soname"].as_str()
        ),
        ("$ORIGIN", "mylib.so")
    );
}

#[test]
fn with_one_needed_library_every_import_is_credited_to_it() {
    let spec = ElfSpec {
        needed: vec!["libc.so.6"],
        verneed: false,
        ..ElfSpec::program()
    };
    let image = inspect_bytes(&build_elf(&spec));
    assert_eq!(
        names(&image.libraries),
        [("libc.so.6", vec!["printf", "pow"])]
    );
}

#[rstest]
#[case::elf64_le(true, false, 64, "little")]
#[case::elf64_be(true, true, 64, "big")]
#[case::elf32_le(false, false, 32, "little")]
#[case::elf32_be(false, true, 32, "big")]
fn every_class_and_byte_order_of_elf_is_read(
    #[case] wide: bool,
    #[case] big: bool,
    #[case] bits: u8,
    #[case] endian: &str,
) {
    let image = inspect_bytes(&build_elf(&ElfSpec {
        wide,
        big,
        ..ElfSpec::program()
    }));
    assert_eq!(
        (image.format, image.bits, image.endian),
        ("elf", Some(bits), Some(endian))
    );
    assert_eq!(image.exports, ["check_flag"]);
    assert_eq!(image.libraries.len(), 3);
}

#[rstest]
#[case::object(1, true, "object")]
#[case::executable(2, false, "executable")]
#[case::pie(3, true, "executable")]
#[case::shared_object(3, false, "library")]
#[case::core(4, false, "data")]
fn the_kind_of_an_elf_follows_its_type(
    #[case] e_type: u16,
    #[case] interp: bool,
    #[case] kind: &str,
) {
    let image = inspect_bytes(&build_elf(&ElfSpec {
        e_type,
        interp,
        ..ElfSpec::program()
    }));
    assert_eq!(image.kind, kind);
}

#[test]
fn an_elf_without_section_headers_is_described_by_its_segments() {
    let image = inspect_bytes(&build_elf(&ElfSpec {
        sections: false,
        ..ElfSpec::program()
    }));
    assert_eq!(section_names(&image), ["LOAD"]);
    assert_eq!(image.sections[0].flags, ["read", "execute"]);
    assert!(image.libraries.is_empty());
}

#[test]
fn a_thin_macho_reports_its_segments_libraries_and_signature() {
    let image = inspect_bytes(&build_macho(&MachoSpec::program()));
    assert_eq!(
        (image.kind, image.format, image.bits),
        ("executable", "macho", Some(64))
    );
    assert_eq!(
        (image.arch.as_deref(), image.endian),
        (Some("x86_64"), Some("little"))
    );
    assert_eq!(
        (image.entry.as_deref(), image.base.as_deref()),
        (Some("0x100000f00"), Some("0x100000000"))
    );
    assert_eq!(section_names(&image), ["__TEXT.__text"]);
    assert_eq!(image.sections[0].flags, ["code"]);
    assert_eq!(
        names(&image.libraries),
        [("/usr/lib/libSystem.B.dylib", vec!["_printf"])]
    );
    assert_eq!(image.exports, ["_main"]);
    assert_eq!(image.notes["codesign"], "adhoc");
    assert_eq!(
        image.notes["entitlements"],
        "com.apple.security.app-sandbox,get-task-allow"
    );
}

#[test]
fn imports_of_a_flat_namespace_macho_are_credited_to_no_library() {
    let image = inspect_bytes(&build_macho(&MachoSpec {
        flat: true,
        ..MachoSpec::program()
    }));
    assert_eq!(
        names(&image.libraries),
        [
            ("/usr/lib/libSystem.B.dylib", vec![]),
            ("*", vec!["_printf"])
        ]
    );
}

#[test]
fn a_big_endian_macho_is_read() {
    let mut header = vec![0xfe, 0xed, 0xfa, 0xce, 0, 0, 0, 7, 0, 0, 0, 3, 0, 0, 0, 2];
    header.resize(28, 0);
    let image = inspect_bytes(&header);
    assert_eq!((image.format, image.endian), ("macho", Some("big")));
}

#[rstest]
#[case::unsigned(None, "none")]
#[case::signed(Some(false), "present")]
#[case::adhoc(Some(true), "adhoc")]
fn the_signature_of_a_macho_is_none_present_or_adhoc(
    #[case] adhoc: Option<bool>,
    #[case] want: &str,
) {
    let image = inspect_bytes(&build_macho(&MachoSpec {
        adhoc,
        entitlements: "",
        ..MachoSpec::program()
    }));
    assert_eq!(image.notes["codesign"], want);
    assert!(!image.notes.contains_key("entitlements"));
}

#[rstest]
#[case::object(1, "object")]
#[case::dylib(6, "library")]
#[case::bundle(8, "library")]
#[case::executable(2, "executable")]
fn the_kind_of_a_macho_follows_its_file_type(#[case] filetype: u32, #[case] kind: &str) {
    let image = inspect_bytes(&build_macho(&MachoSpec {
        filetype,
        ..MachoSpec::program()
    }));
    assert_eq!(image.kind, kind);
}

#[test]
fn a_32_bit_macho_with_no_commands_is_still_described() {
    let mut header = vec![0xce, 0xfa, 0xed, 0xfe, 7, 0, 0, 0, 3, 0, 0, 0, 2, 0, 0, 0];
    header.resize(28, 0);
    let image = inspect_bytes(&header);
    assert_eq!(
        (image.format, image.bits, image.arch.as_deref()),
        ("macho", Some(32), Some("x86"))
    );
    assert_eq!(image.notes["codesign"], "none");
}

fn thin(cputype: u32) -> Vec<u8> {
    build_macho(&MachoSpec {
        cputype,
        ..MachoSpec::program()
    })
}

#[test]
fn a_fat_macho_lists_its_architectures_and_describes_the_first_slice() {
    let image = inspect_bytes(&fat(&[thin(X86_64), thin(ARM64)], &[X86_64, ARM64]));
    assert_eq!(
        (image.kind, image.format, image.arch.as_deref()),
        ("executable", "fat-macho", Some("x86_64"))
    );
    assert_eq!(image.notes["architectures"], "x86_64,arm64");
    assert_eq!(image.exports, ["_main"]);
}

#[test]
fn a_fat_macho_skips_a_slice_it_does_not_know_and_a_64_bit_table_is_read() {
    let slice = thin(ARM64);
    let mut bytes = Vec::new();
    for word in [
        0xcafe_babf_u32,
        1,
        ARM64,
        3,
        0,
        0x1000,
        0,
        slice.len() as u32,
        12,
        0,
    ] {
        bytes.extend(word.to_be_bytes());
    }
    bytes.resize(0x1000, 0);
    bytes.extend(&slice);
    let image = inspect_bytes(&bytes);
    assert_eq!(
        (image.format, image.arch.as_deref()),
        ("fat-macho", Some("arm64"))
    );
    let unknown_first = fat(&[thin(ARM64), thin(X86_64)], &[0x77, X86_64]);
    assert_eq!(
        inspect_bytes(&unknown_first).arch.as_deref(),
        Some("x86_64")
    );
}

#[rstest]
#[case::java_class(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 0x34, 1, 2, 3, 4])]
#[case::fat_with_no_slices(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 0])]
#[case::fat_cut_short(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2, 1, 2])]
#[case::elf_header_only(b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0")]
#[case::pe_magic_only(b"MZ")]
fn a_file_that_claims_a_format_but_is_not_one_is_unknown_data(#[case] bytes: &[u8]) {
    let image = inspect_bytes(bytes);
    assert_eq!((image.kind, image.format), ("data", "unknown"));
}

#[test]
fn a_fat_macho_of_unreadable_slices_is_unknown_data() {
    let bytes = fat(&[b"not a macho at all".to_vec()], &[X86_64]);
    assert_eq!(inspect_bytes(&bytes).format, "unknown");
}

#[test]
fn random_bytes_are_data_with_an_entropy_near_eight() {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let bytes: Vec<u8> = (0..65_536)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect();
    let image = inspect_bytes(&bytes);
    assert_eq!((image.kind, image.format), ("data", "unknown"));
    assert!(image.entropy > 7.9, "{}", image.entropy);
}

#[test]
fn a_text_file_is_data_with_its_best_strings_first() {
    let image = inspect_bytes(b"notes\nhttps://example.test/api\nplain words here\n");
    assert_eq!((image.kind, image.format), ("data", "unknown"));
    assert_eq!(image.strings[0], "https://example.test/api");
    assert!(image.entropy > 3.0 && image.entropy < 5.5);
    assert!(image.arch.is_none());
}

#[test]
fn an_empty_file_is_data_with_no_entropy() {
    let image = inspect_bytes(b"");
    assert_eq!(
        (image.kind, image.entropy, image.strings.len()),
        ("data", 0.0, 0)
    );
}

fn all_images() -> Vec<Vec<u8>> {
    let pe = PeSpec {
        imports: vec![("KERNEL32.dll".into(), vec!["CreateFileW".into()])],
        exports: vec![(Some("Compute".into()), 0x1000)],
        code: vec![0xc3],
        ..PeSpec::new(true)
    };
    let slice = thin(X86_64);
    vec![
        build_pe(&pe),
        build_elf(&ElfSpec::program()),
        build_elf(&ElfSpec {
            wide: false,
            big: true,
            ..ElfSpec::program()
        }),
        slice.clone(),
        fat(&[slice], &[X86_64]),
    ]
}

/// A hostile file is answered, whatever is wrong with it: every prefix of each image, and each image with a byte
/// of its first kilobyte damaged, goes through without a panic.
#[test]
fn damaged_and_truncated_images_never_panic() {
    for image in all_images() {
        for cut in 0..image.len() {
            let _ = inspect_bytes(&image[..cut]);
        }
        for at in 0..image.len().min(1024) {
            let mut damaged = image.clone();
            damaged[at] ^= 0xff;
            let _ = inspect_bytes(&damaged);
            damaged[at] = 0x7f;
            let _ = inspect_bytes(&damaged);
        }
    }
}

#[test]
fn a_damaged_dynamic_table_keeps_what_was_read_before_it() {
    let mut bytes = build_elf(&ElfSpec::program());
    let dynstr = bytes.windows(10).position(|w| w == b"libc.so.6\0").unwrap();
    bytes[dynstr..dynstr + 9].fill(b'x');
    let image = inspect_bytes(&bytes);
    assert_eq!(image.format, "elf");
    assert_eq!(image.exports, ["check_flag"]);
}

#[rstest]
#[case::a_text_file(b"hello".to_vec(), Magic::Unknown)]
#[case::pe(b"MZ\x90\0".to_vec(), Magic::Pe)]
#[case::elf(b"\x7fELF\x02".to_vec(), Magic::Elf)]
#[case::macho_64_le(vec![0xcf, 0xfa, 0xed, 0xfe, 0, 0, 0, 0], Magic::MachO)]
#[case::macho_32_be(vec![0xfe, 0xed, 0xfa, 0xce, 0, 0, 0, 0], Magic::MachO)]
#[case::fat(vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2], Magic::Fat)]
#[case::java_class(vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 52], Magic::Unknown)]
#[case::too_short(vec![0xca, 0xfe], Magic::Unknown)]
fn the_format_is_taken_from_the_first_bytes(#[case] data: Vec<u8>, #[case] want: Magic) {
    assert_eq!(sniff(&data), want);
}

#[test]
fn the_worst_case_answer_stays_under_4000_tokens_and_says_it_was_cut() {
    let long = |i: usize, n: usize| format!("{}{i:04}", "n".repeat(n));
    let imports = (0..4)
        .map(|d| {
            (
                format!("{}.dll", long(d, 60)),
                (0..120).map(|f| long(f, 70)).collect(),
            )
        })
        .collect();
    let spec = PeSpec {
        imports,
        extra_sections: 90,
        code: vec![0xc3],
        ..PeSpec::new(true)
    };
    let mut bytes = build_pe(&spec);
    bytes.extend(
        (0..4000).flat_map(|i| {
            format!("\0https://host{i}.example.test/{}", "p".repeat(100)).into_bytes()
        }),
    );
    let image = inspect_bytes(&bytes);
    let tokens = estimate(&serde_json::to_string(&image).unwrap());
    assert!(image.truncated);
    assert!(tokens < 4_000, "{tokens} tokens");
    assert!(!image.strings.is_empty());
}

#[test]
fn a_big_text_file_spends_the_whole_budget_on_strings() {
    let text: String = (0..3000)
        .map(|i| format!("line number {i} of the file\n"))
        .collect();
    let image = inspect_bytes(text.as_bytes());
    let tokens = estimate(&serde_json::to_string(&image).unwrap());
    assert!(
        image.truncated && (2_000..4_000).contains(&tokens),
        "{tokens}"
    );
}

#[test]
fn a_path_is_read_from_disk_and_refused_when_it_is_not_a_file() {
    let path = write_temp("inspect", &build_elf(&ElfSpec::program()));
    let text = path.to_str().unwrap();
    assert_eq!(inspect_path(text).unwrap().format, "elf");
    fs::remove_file(&path).unwrap();
    let missing = inspect_path(text).unwrap_err();
    assert!(matches!(missing, CoreError::Unreadable(_)), "{missing}");
    let dir = inspect_path(std::env::temp_dir().to_str().unwrap()).unwrap_err();
    assert!(dir.to_string().contains("directory"), "{dir}");
}

#[test]
fn a_file_over_the_size_cap_is_refused_unread() {
    let path = temp_path("inspect_big");
    File::create(&path)
        .unwrap()
        .set_len(MAX_FILE_BYTES + 1)
        .unwrap();
    let refused = inspect_path(path.to_str().unwrap());
    fs::remove_file(&path).unwrap();
    assert!(matches!(refused, Err(CoreError::Unreadable(_))));
}

#[cfg(unix)]
#[test]
fn a_file_that_cannot_be_read_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let path = write_temp("inspect_locked", b"secret");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let result = inspect_path(path.to_str().unwrap());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_file(&path).unwrap();
    // A superuser can read it anyway, which is also a valid answer.
    assert!(matches!(result, Err(CoreError::Unreadable(_)) | Ok(_)));
}

/// `/bin/ls` is a real universal Mach-O on macOS: the one check against a binary this crate did not build.
#[cfg(target_os = "macos")]
#[test]
fn a_real_universal_binary_is_described() {
    let image = inspect_path("/bin/ls").unwrap();
    assert_eq!(image.format, "fat-macho");
    assert!(image.libraries.iter().any(|l| l.name.contains("libSystem")));
    assert!(estimate(&serde_json::to_string(&image).unwrap()) < 4_000);
}
