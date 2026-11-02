//! The crate holds itself to the limits it enforces: no function of its own sources is above them.

use std::fs;
use std::path::{Path, PathBuf};

use dendrite_core::complexity::{complexity, MAX_COGNITIVE, MAX_CYCLOMATIC};
use dendrite_core::Lang;

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn every_function_is_within_the_limits() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "tests"] {
        rust_files(&root.join(dir), &mut files);
    }
    let mut measured = 0;
    let mut over = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).unwrap();
        for m in complexity(Lang::Rust, &source) {
            measured += 1;
            if m.cyclomatic > MAX_CYCLOMATIC || m.cognitive > MAX_COGNITIVE {
                over.push(format!(
                    "{}:{} {} cyclomatic {} cognitive {}",
                    file.display(),
                    m.start_line,
                    m.symbol,
                    m.cyclomatic,
                    m.cognitive
                ));
            }
        }
    }
    assert!(measured > 100, "measured only {measured} functions");
    assert!(
        over.is_empty(),
        "functions over the limits:\n{}",
        over.join("\n")
    );
}
