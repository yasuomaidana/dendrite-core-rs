//! Times `ast.complexity` for Go over a list of files (one path per line): the parser path, the fast scan, and
//! the batch on every core. `cargo run --release --example go_bench -- files.txt`
use dendrite_core::ast::go::fast as go_fast;
use dendrite_core::batch::{complexity_batch, BatchFile};
use dendrite_core::{complexity::complexity_tree, Lang};
#[path = "common/bench.rs"]
mod bench;
use bench::best;

fn main() {
    let list = std::fs::read_to_string(std::env::args().nth(1).expect("a file list")).unwrap();
    let sources: Vec<String> = list
        .lines()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .collect();
    let bytes: usize = sources.iter().map(String::len).sum();
    println!("{} files, {:.1} MB", sources.len(), bytes as f64 / 1e6);
    let files: Vec<BatchFile> = sources
        .iter()
        .map(|s| BatchFile {
            lang: "go".into(),
            source: s.clone(),
        })
        .collect();

    let tree = best(5, || {
        for s in &sources {
            std::hint::black_box(complexity_tree(Lang::Go, s));
        }
    });
    let fast = best(9, || {
        for s in &sources {
            std::hint::black_box(go_fast::complexity(s));
        }
    });
    println!("tree-sitter path, 1 thread: {tree:?}");
    println!(
        "fast scan,        1 thread: {fast:?}   ({:.1}x the tree path, {:.0} MB/s)",
        tree.as_secs_f64() / fast.as_secs_f64(),
        bytes as f64 / 1e6 / fast.as_secs_f64()
    );
    for workers in [2, 4, 8] {
        let d = best(9, || {
            std::hint::black_box(complexity_batch(&files, workers));
        });
        println!("batch (fast scan), {workers} threads: {d:?}");
    }
}
