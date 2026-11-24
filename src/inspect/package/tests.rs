use std::fs;
use std::path::PathBuf;

use rstest::rstest;
use serde_json::json;

use super::fixture::{asar, asar_header, zip};
use super::*;
use crate::inspect::{inspect_bytes, inspect_named, inspect_path, Image};
use crate::pe::fixture::temp_path;
use crate::tokens::estimate;

fn paths(image: &Image) -> Vec<&str> {
    image.files.iter().map(|f| f.path.as_str()).collect()
}

fn library_names(image: &Image) -> Vec<&str> {
    image.libraries.iter().map(|l| l.name.as_str()).collect()
}

/// An empty directory of its own.
fn scratch(tag: &str) -> PathBuf {
    let dir = temp_path(tag);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn put(root: &std::path::Path, path: &str, text: &str) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}

#[rstest]
#[case("AndroidManifest.xml", "manifest")]
#[case("META-INF/MANIFEST.MF", "manifest")]
#[case("a/b/package.json", "manifest")]
#[case("classes.dex", "bytecode")]
#[case("lib/arm64-v8a/libfoo.so", "native")]
#[case("app/main.JS", "script")]
#[case("src/main.rs", "source")]
#[case("res/icon.png", "resource")]
#[case(".gitignore", "resource")]
#[case("Makefile", "resource")]
fn a_path_has_a_kind_from_its_name(#[case] path: &str, #[case] want: &str) {
    assert_eq!(kind_of(path), want);
}

#[rstest]
#[case(&["node_modules/a/index.js"], &["a"])]
#[case(&["x/node_modules/@s/p/lib.js", "node_modules/@s/p/b.js"], &["@s/p"])]
#[case(&["node_modules/a/node_modules/b/x.js"], &["a", "b"])]
#[case(&["node_modules/.bin/tool", "node_modules/", "node_modules/@s", "src/a.js"], &[])]
fn packages_are_named_by_their_node_modules_directory(
    #[case] paths: &[&str],
    #[case] want: &[&str],
) {
    let names = node_modules_names(paths.iter().copied());
    assert_eq!(names.into_iter().collect::<Vec<_>>(), want);
}

#[test]
fn only_so_many_packages_are_named() {
    let many: Vec<String> = (0..500)
        .map(|i| format!("node_modules/p{i}/x.js"))
        .collect();
    let names = node_modules_names(many.iter().map(String::as_str));
    assert!(names.len() <= MAX_DEPENDENCIES + 1);
    let mut budget = Budget::default();
    let libraries = dependencies(&mut budget, (0..500).map(|i| format!("p{i}")));
    assert_eq!(libraries.len(), MAX_DEPENDENCIES);
    assert!(budget.truncated);
}

// A source tree.

#[test]
fn a_directory_is_a_source_tree_listed_breadth_first_with_its_dependencies() {
    let root = scratch("tree");
    put(&root, "Cargo.toml", "[dependencies]\nserde = \"1\"\n");
    put(&root, "package.json", r#"{"dependencies":{"express":"4"}}"#);
    put(&root, "src/main.rs", "fn main() {}");
    put(&root, "src/a/b/lib.rs", "");
    put(&root, "node_modules/left-pad/index.js", "");
    put(&root, "node_modules/@s/pkg/index.js", "");
    put(&root, ".git/HEAD", "ref");
    let image = inspect_path(root.to_str().unwrap()).unwrap();
    assert_eq!((image.kind, image.format), ("source", "source-tree"));
    assert_eq!(
        paths(&image),
        [
            "Cargo.toml",
            "package.json",
            "src/main.rs",
            "src/a/b/lib.rs"
        ]
    );
    assert_eq!(image.files[0].kind, "manifest");
    assert_eq!(image.files[2].kind, "source");
    assert_eq!(image.files[2].size, 12);
    assert_eq!(
        library_names(&image),
        ["@s/pkg", "express", "left-pad", "serde"]
    );
    assert_eq!(image.notes["entries"], "4");
    assert_eq!(image.notes["skipped"], ".git,node_modules");
    assert_eq!(image.notes["languages"], "rs:2,json:1,toml:1");
    assert!(!image.truncated);
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_is_listed_and_never_followed() {
    let root = scratch("loop");
    put(&root, "a/file.txt", "x");
    std::os::unix::fs::symlink(&root, root.join("a/loop")).unwrap();
    std::os::unix::fs::symlink(root.join("missing"), root.join("dangling")).unwrap();
    let image = inspect_path(root.to_str().unwrap()).unwrap();
    let kinds: Vec<(&str, &str)> = image
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.kind.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("dangling", "link"),
            ("a/file.txt", "resource"),
            ("a/loop", "link")
        ]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_tree_deeper_than_the_cap_says_it_was_cut() {
    let root = scratch("deep");
    put(&root, "a/b/c/d/e/f.rs", "");
    put(&root, "a/b/c/d/g.rs", "");
    let image = inspect_path(root.to_str().unwrap()).unwrap();
    assert_eq!(paths(&image), ["a/b/c/d/g.rs"]);
    assert!(image.truncated);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_tree_of_more_entries_than_the_cap_says_it_was_cut() {
    let root = scratch("wide");
    for i in 0..20_001 {
        fs::write(root.join(format!("f{i:05}.txt")), "").unwrap();
    }
    let image = inspect_path(root.to_str().unwrap()).unwrap();
    assert!(image.truncated);
    assert!(estimate(&serde_json::to_string(&image).unwrap()) < 4_000);
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn a_directory_that_cannot_be_read_is_skipped_below_the_root_and_refused_at_it() {
    use std::os::unix::fs::PermissionsExt;
    let root = scratch("locked");
    put(&root, "open/a.rs", "");
    put(&root, "closed/b.rs", "");
    let lock = |dir: &std::path::Path, mode| {
        fs::set_permissions(dir, fs::Permissions::from_mode(mode)).unwrap()
    };
    lock(&root.join("closed"), 0o000);
    let readable = fs::read_dir(root.join("closed")).is_ok();
    let image = inspect_path(root.to_str().unwrap()).unwrap();
    assert!(paths(&image).contains(&"open/a.rs"));
    assert_eq!(paths(&image).contains(&"closed/b.rs"), readable);
    lock(&root.join("closed"), 0o755);
    lock(&root, 0o000);
    let still_readable = fs::read_dir(&root).is_ok();
    let refused = inspect_path(root.to_str().unwrap());
    lock(&root, 0o755);
    assert_eq!(refused.is_err(), !still_readable);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_missing_manifest_or_a_broken_one_names_nothing() {
    let root = scratch("broken");
    put(&root, "package.json", "{ not json");
    put(&root, "go.mod", "module x\nrequire a.test/m v1\n");
    let image = inspect_path(root.to_str().unwrap()).unwrap();
    assert_eq!(library_names(&image), ["a.test/m"]);
    let _ = fs::remove_dir_all(root);
}

// ZIP.

fn zipped(entries: &[(&str, u64)]) -> Image {
    inspect_bytes(&zip(entries, false))
}

#[rstest]
#[case::apk(&["AndroidManifest.xml", "classes.dex", "lib/arm64-v8a/liba.so", "lib/x86_64/liba.so"], "apk")]
#[case::aab(&["BundleConfig.pb", "base/manifest/AndroidManifest.xml"], "aab")]
#[case::aar(&["AndroidManifest.xml", "classes.jar"], "aar")]
#[case::ipa(&["Payload/Demo.app/Info.plist", "Payload/Demo.app/Demo"], "ipa")]
#[case::jar(&["META-INF/MANIFEST.MF", "a/B.class"], "jar")]
#[case::plain(&["readme.txt"], "zip")]
fn an_archive_is_named_for_what_it_holds(#[case] names: &[&str], #[case] package: &str) {
    let entries: Vec<(&str, u64)> = names.iter().map(|n| (*n, 10)).collect();
    let image = zipped(&entries);
    assert_eq!((image.kind, image.format), ("package", "zip"));
    assert_eq!(image.notes["package"], package);
    assert_eq!(image.notes["entries"], names.len().to_string());
    assert_eq!(image.files.len(), names.len());
    assert!(image.strings.is_empty());
}

#[test]
fn a_zip_names_its_manifest_and_its_native_architectures() {
    let image = zipped(&[
        ("res/a.png", 1),
        ("assets/", 0),
        ("AndroidManifest.xml", 512),
        ("lib/arm64-v8a/liba.so", 9),
        ("lib/x86_64/liba.so", 9),
        ("classes.dex", 4096),
    ]);
    assert_eq!(image.notes["manifest"], "AndroidManifest.xml");
    assert_eq!(image.notes["architectures"], "arm64-v8a,x86_64");
    assert_eq!(
        paths(&image),
        [
            "AndroidManifest.xml",
            "lib/arm64-v8a/liba.so",
            "lib/x86_64/liba.so",
            "classes.dex",
            "res/a.png"
        ]
    );
    assert_eq!(image.files[3].kind, "bytecode");
    assert_eq!(image.files[3].size, 4096);
}

#[test]
fn a_zip64_archive_and_an_entry_claiming_gigabytes_are_listed_without_reading_them() {
    let big = 5_000_000_000;
    for zip64 in [false, true] {
        let image = inspect_bytes(&zip(&[("bomb.bin", big), ("a.txt", 3)], zip64));
        assert_eq!(paths(&image), ["bomb.bin", "a.txt"]);
        assert_eq!(image.files[0].size, big);
        assert_eq!(image.files[1].size, 3);
    }
}

#[test]
fn an_empty_archive_is_a_package_with_no_files() {
    let mut data = b"PK\x05\x06".to_vec();
    data.resize(22, 0);
    let image = inspect_bytes(&data);
    assert_eq!((image.kind, image.format), ("package", "zip"));
    assert!(image.files.is_empty());
}

#[test]
fn an_archive_claiming_more_entries_than_it_has_lists_what_is_there() {
    let mut data = zip(&[("a.txt", 1), ("b.txt", 2)], false);
    let end = data.len() - 22;
    data[end + 8..end + 12].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
    data[end + 10..end + 12].copy_from_slice(&[0x10, 0x27]);
    assert_eq!(paths(&inspect_bytes(&data)), ["a.txt", "b.txt"]);
}

#[test]
fn a_directory_that_points_past_the_end_lists_nothing_and_a_missing_one_is_unknown() {
    let mut data = zip(&[("a.txt", 1)], false);
    let end = data.len() - 22;
    data[end + 16..end + 20].copy_from_slice(&[0xf0, 0xff, 0xff, 0x7f]);
    assert!(inspect_bytes(&data).files.is_empty());
    let cut = b"PK\x03\x04 and then nothing that ends the archive";
    assert_eq!(inspect_bytes(cut).format, "unknown");
}

#[test]
fn a_zip64_pointer_that_leads_nowhere_makes_the_file_unknown() {
    let mut data = zip(&[("a.txt", 1)], true);
    let end = data.len() - 22;
    let locator = end - 20;
    data[locator + 8..locator + 16].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(inspect_bytes(&data).format, "unknown");
    let mut no_locator = zip(&[("a.txt", 1)], true);
    no_locator[locator..locator + 4].copy_from_slice(b"XXXX");
    assert_eq!(inspect_bytes(&no_locator).format, "unknown");
    let mut wrong = zip(&[("a.txt", 1)], true);
    let eocd64 = u64::from_le_bytes(wrong[locator + 8..locator + 16].try_into().unwrap()) as usize;
    wrong[eocd64..eocd64 + 4].copy_from_slice(b"XXXX");
    assert_eq!(inspect_bytes(&wrong).format, "unknown");
}

#[test]
fn a_small_budget_keeps_the_manifest_and_native_code_before_resources() {
    let mut entries: Vec<(String, u64)> = (0..600)
        .map(|i| (format!("res/image{i:04}.png"), 9))
        .collect();
    entries.push(("AndroidManifest.xml".into(), 100));
    entries.push(("lib/arm64-v8a/libgame.so".into(), 100));
    let refs: Vec<(&str, u64)> = entries.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    let image = zipped(&refs);
    assert!(image.truncated);
    assert_eq!(
        paths(&image)[..2],
        ["AndroidManifest.xml", "lib/arm64-v8a/libgame.so"]
    );
    assert!(estimate(&serde_json::to_string(&image).unwrap()) < 4_000);
}

// ASAR.

fn package_text() -> String {
    r#"{"name":"demo","main":"main.js"}"#.to_string()
}

fn asar_of(header: &serde_json::Value) -> Vec<u8> {
    asar(header, package_text().as_bytes())
}

#[test]
fn an_asar_lists_its_files_integrity_and_packages() {
    let image = inspect_bytes(&asar_of(&asar_header(&package_text())));
    assert_eq!((image.kind, image.format), ("package", "asar"));
    assert_eq!(
        paths(&image),
        [
            "main.js",
            "native.node",
            "package.json",
            "app/link.js",
            "app/node_modules/left-pad/index.js",
            "app/node_modules/@scope/pkg/a.js"
        ]
    );
    let kinds: Vec<&str> = image.files.iter().map(|f| f.kind.as_str()).collect();
    assert_eq!(
        kinds,
        ["script", "native", "manifest", "link", "script", "script"]
    );
    assert_eq!(image.notes["integrity"], "partial");
    assert_eq!(image.notes["unpacked"], "1");
    assert_eq!(image.notes["entries"], "6");
    assert_eq!(
        (image.notes["name"].as_str(), image.notes["main"].as_str()),
        ("demo", "main.js")
    );
    assert_eq!(library_names(&image), ["@scope/pkg", "left-pad"]);
    assert!(image.strings.is_empty());
}

#[rstest]
#[case::none(json!({"files": {"a.js": {"size": 1, "offset": "0"}}}), "none")]
#[case::all(json!({"files": {"a.js": {"size": 1, "offset": "0", "integrity": {"algorithm": "SHA256"}}}}), "sha256")]
#[case::no_algorithm(json!({"files": {"a.js": {"size": 1, "integrity": {}}}}), "present")]
#[case::empty(json!({"files": {}}), "none")]
fn the_integrity_of_an_asar_is_none_partial_or_the_algorithm(
    #[case] header: serde_json::Value,
    #[case] want: &str,
) {
    let image = inspect_bytes(&asar(&header, b""));
    assert_eq!(image.format, "asar");
    assert_eq!(image.notes["integrity"], want);
    assert!(!image.notes.contains_key("unpacked"));
    assert!(!image.notes.contains_key("name"));
}

#[rstest]
#[case::offset_not_a_number(json!({"files": {"package.json": {"size": 5, "offset": "x"}}}))]
#[case::offset_past_the_end(json!({"files": {"package.json": {"size": 5, "offset": "99999"}}}))]
#[case::offset_overflows(json!({"files": {"package.json": {"size": 5, "offset": "18446744073709551615"}}}))]
#[case::too_big(json!({"files": {"package.json": {"size": 5000000, "offset": "0"}}}))]
#[case::no_size(json!({"files": {"package.json": {"offset": "0"}}}))]
#[case::not_json(json!({"files": {"package.json": {"size": 4, "offset": "0"}}}))]
fn a_package_json_that_cannot_be_read_names_nothing(#[case] header: serde_json::Value) {
    let image = inspect_bytes(&asar(&header, b"{{{{"));
    assert_eq!(image.format, "asar");
    assert!(!image.notes.contains_key("name"));
}

#[test]
fn an_asar_nested_deeper_than_the_cap_or_larger_than_it_says_it_was_cut() {
    let mut deep = json!({"x.js": {"size": 1}});
    for _ in 0..40 {
        deep = json!({"d": {"files": deep}});
    }
    let image = inspect_bytes(&asar(&json!({"files": deep}), b""));
    assert_eq!(image.format, "asar");
    assert!(image.truncated);
    let files: Vec<String> = (0..100_001)
        .map(|i| format!("\"f{i}\":{{\"size\":1}}"))
        .collect();
    let big: serde_json::Value =
        serde_json::from_str(&format!("{{\"files\":{{{}}}}}", files.join(","))).unwrap();
    let image = inspect_bytes(&asar(&big, b""));
    assert_eq!(image.notes["entries"], "100000");
    assert!(image.truncated);
}

fn raw_asar(json_len: u32, json: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for value in [4u32, 8, 4, json_len] {
        out.extend(value.to_le_bytes());
    }
    out.extend(json);
    out
}

#[rstest]
#[case::length_past_the_end(raw_asar(500, br#"{"files":{}}"#))]
#[case::header_over_the_cap(raw_asar(17 * 1024 * 1024, br#"{"files":{}}"#))]
#[case::bad_json(raw_asar(12, br#"{"files":{]}"#))]
#[case::files_is_not_a_tree(raw_asar(12, br#"{"files":5 }"#))]
#[case::too_short(b"\x04\0\0\0\x08\0\0\0{\"files\"".to_vec())]
fn a_damaged_asar_header_is_unknown_data(#[case] data: Vec<u8>) {
    assert_eq!(inspect_bytes(&data).format, "unknown");
}

#[test]
fn json_nested_ten_thousand_deep_is_refused_without_blowing_the_stack() {
    let text = format!(
        "{{\"files\":{}{}}}",
        "{\"files\":".repeat(10_000),
        "}".repeat(10_000)
    );
    let data = raw_asar(text.len() as u32, text.as_bytes());
    assert_eq!(inspect_bytes(&data).format, "unknown");
}

// JS bundles.

fn bundled(text: &str, ext: Option<&str>) -> Image {
    inspect_named(text.as_bytes(), ext)
}

#[test]
fn a_webpack_bundle_names_its_bundler_libraries_strings_and_source_map() {
    let text = "/*! lodash v4.17.21 | MIT */\n/*! For license information please see main.js.LICENSE.txt */\n\
        var a=__webpack_require__(\"./node_modules/react/index.js\");__webpack_require__(\"./node_modules/@s/p/x.js\");\n\
        fetch('https://api.internal.endpoint/v1');var f=\"usage: %s file\";var p=\"/etc/config\";var s='ab';\n\
        //# sourceMappingURL=main.js.map\n";
    let image = bundled(text, None);
    assert_eq!((image.kind, image.format), ("bundle", "js-bundle"));
    assert_eq!(image.notes["bundler"], "webpack");
    assert_eq!(image.notes["sourcemap"], "main.js.map");
    assert!(!image.notes.contains_key("minified"));
    assert_eq!(library_names(&image), ["@s/p", "lodash", "react"]);
    assert_eq!(image.strings[0], "https://api.internal.endpoint/v1");
    assert!(image.strings.contains(&"/etc/config".to_string()));
    assert!(!image.strings.contains(&"ab".to_string()));
    assert!(image.entropy > 0.0);
}

#[rstest]
#[case("var x=__commonJS({});", "esbuild")]
#[case("var x=parcelRequire('a');", "parcel")]
#[case("System.register([], function(){});", "systemjs")]
fn other_bundlers_are_recognised_by_their_runtime(#[case] text: &str, #[case] want: &str) {
    assert_eq!(bundled(text, None).notes["bundler"], want);
}

#[test]
fn a_long_line_marks_a_bundle_as_minified_and_a_script_name_is_enough() {
    let text = format!("var a=\"{}\";", "x".repeat(1_500));
    let image = bundled(&text, Some("mjs"));
    assert_eq!(image.format, "js-bundle");
    assert_eq!(image.notes["minified"], "true");
    assert!(!image.notes.contains_key("bundler"));
    assert!(image.libraries.is_empty());
}

#[rstest]
#[case::plain_text("just some prose here\nwith lines\n", None)]
#[case::unmarked_extension("var a = 1;\n", Some("txt"))]
fn text_without_a_mark_or_a_script_name_stays_unknown(
    #[case] text: &str,
    #[case] ext: Option<&str>,
) {
    assert_eq!(bundled(text, ext).format, "unknown");
}

#[test]
fn binary_data_named_like_a_script_stays_unknown() {
    let data: Vec<u8> = (0..4096).map(|i| (i * 7 % 251) as u8).collect();
    assert_eq!(inspect_named(&data, Some("js")).format, "unknown");
    assert_eq!(inspect_named(b"", Some("js")).format, "unknown");
}

#[test]
fn a_banner_without_a_version_or_a_name_names_no_library() {
    let text = "/*! MIT licensed */ /*! 1.2.3 */ @license MIT\n@license pkg-a v1.0.0\n/*! x\n";
    assert_eq!(library_names(&bundled(text, Some("js"))), ["pkg-a"]);
}

#[test]
fn quoted_text_is_read_to_the_closing_quote_on_one_line() {
    let text = "a='unterminated string\nb=\"closed string\" c=`template text` d='x'";
    let image = bundled(text, Some("js"));
    assert_eq!(image.strings, ["closed string", "template text"]);
    let long = format!("e=\"{}\"", "y".repeat(121));
    assert!(bundled(&long, Some("js")).strings.is_empty());
    assert!(bundled("f='", Some("js")).strings.is_empty());
}

#[test]
fn a_file_larger_than_the_scan_is_searched_in_its_first_part_only() {
    let mut text = b"var a=\"inside the scan\";".to_vec();
    text.resize(crate::inspect::scan::MAX_SCAN + 100, b' ');
    text.extend(b"var b=\"beyond the scan\";");
    let image = inspect_named(&text, Some("js"));
    assert_eq!(image.strings, ["inside the scan"]);
    assert!(image.truncated);
}

// Through a path.

#[test]
fn a_file_is_taken_for_a_bundle_by_its_extension_and_a_zip_by_its_bytes() {
    let js = temp_path("bundle.JS");
    fs::write(&js, "var a = 1;\n").unwrap();
    assert_eq!(
        inspect_path(js.to_str().unwrap()).unwrap().format,
        "js-bundle"
    );
    let plain = temp_path("noext");
    fs::write(&plain, "var a = 1;\n").unwrap();
    assert_eq!(
        inspect_path(plain.to_str().unwrap()).unwrap().format,
        "unknown"
    );
    let archive = temp_path("a.zip");
    fs::write(&archive, zip(&[("a.txt", 1)], false)).unwrap();
    assert_eq!(
        inspect_path(archive.to_str().unwrap()).unwrap().format,
        "zip"
    );
    for path in [js, plain, archive] {
        let _ = fs::remove_file(path);
    }
}
