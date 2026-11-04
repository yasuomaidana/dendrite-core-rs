//! Prints the fast scan's and the parser's metrics for Go source on stdin, to see where they differ.
use dendrite_core::ast::go::fast as go_fast;
use dendrite_core::{complexity::complexity_tree, Lang};
use std::io::Read;

fn main() {
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).unwrap();
    let fast = go_fast::complexity(&src);
    let tree = complexity_tree(Lang::Go, &src);
    for (i, t) in tree.iter().enumerate() {
        let f = fast.as_ref().and_then(|f| f.get(i));
        println!(
            "{:30} tree cyc {:3} cog {:3} | scan {}",
            t.symbol,
            t.cyclomatic,
            t.cognitive,
            f.map_or("none".into(), |f| format!(
                "cyc {:3} cog {:3}",
                f.cyclomatic, f.cognitive
            ))
        );
    }
}
