//! Times the tree-sitter path of `ast.complexity` for any language over a list of files, one thread against
//! the batch on every worker count. `cargo run --release --example lang_bench -- <lang> files.txt`
use dendrite_core::batch::{complexity_batch, BatchFile};
use std::time::{Duration, Instant};

fn best(runs: usize, mut f: impl FnMut()) -> Duration {
    (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .min()
        .unwrap()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let lang = args.next().expect("a language id");
    let list = std::fs::read_to_string(args.next().expect("a file list")).unwrap();
    let files: Vec<BatchFile> = list
        .lines()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|source| BatchFile {
            lang: lang.clone(),
            source,
        })
        .collect();
    let bytes: usize = files.iter().map(|f| f.source.len()).sum();
    println!(
        "{lang}: {} files, {:.1} MB",
        files.len(),
        bytes as f64 / 1e6
    );
    for workers in [1, 2, 4, 8] {
        let d = best(3, || {
            std::hint::black_box(complexity_batch(&files, workers));
        });
        println!(
            "  {workers} threads: {d:?}  ({:.1} MB/s)",
            bytes as f64 / 1e6 / d.as_secs_f64()
        );
    }
}
