//! Lists the valid Go files under a directory that the fast scan hands back to the parser.
use dendrite_core::ast::go::fast as go_fast;
use dendrite_core::{parse::parse, Lang};

fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out)
        } else if p.extension().is_some_and(|x| x == "go") {
            out.push(p)
        }
    }
}

fn main() {
    let mut files = Vec::new();
    walk(
        std::path::Path::new(&std::env::args().nth(1).unwrap()),
        &mut files,
    );
    let (mut bytes_all, mut bytes_fb) = (0usize, 0usize);
    for p in files {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        if parse(Lang::Go, &src).root_node().has_error() {
            continue;
        }
        bytes_all += src.len();
        if go_fast::complexity(&src).is_none() {
            bytes_fb += src.len();
            println!("{}", p.display())
        }
    }
    eprintln!(
        "fallback files hold {:.1}% of the bytes",
        100.0 * bytes_fb as f64 / bytes_all as f64
    );
}
