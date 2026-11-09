//! Parsing with one tree-sitter `Parser` per language per thread (parsers are `!Sync`).

use std::cell::RefCell;
use std::collections::HashMap;

use tree_sitter::{Parser, Tree};

use crate::lang::Lang;

thread_local! {
    static PARSERS: RefCell<HashMap<Lang, Parser>> = RefCell::new(HashMap::new());
}

/// Parses `source`. Syntax errors do not fail the parse: they show up as `ERROR` or `MISSING` nodes. The parser
/// has no timeout or cancellation flag, so it always produces a tree.
pub fn parse(lang: Lang, source: &str) -> Tree {
    PARSERS.with(|pool| {
        let mut pool = pool.borrow_mut();
        let parser = pool.entry(lang).or_insert_with(|| {
            let mut parser = Parser::new();
            parser
                .set_language(&lang.grammar())
                .expect("compiled-in grammar matches the library ABI");
            parser
        });
        parser
            .parse(source, None)
            .expect("a parser without timeout or cancellation always yields a tree")
    })
}

/// The text of a node.
pub(crate) fn text<'a>(node: tree_sitter::Node<'_>, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.byte_range()]).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_reuses_the_parser() {
        for _ in 0..2 {
            let tree = parse(Lang::Go, "package p\nfunc f() {}\n");
            assert!(!tree.root_node().has_error());
        }
    }

    #[test]
    fn keeps_syntax_errors_in_the_tree() {
        assert!(parse(Lang::Go, "func (").root_node().has_error());
    }

    #[test]
    fn text_of_invalid_utf8_is_empty() {
        let tree = parse(Lang::Go, "package p");
        assert_eq!(text(tree.root_node(), b"package p"), "package p");
        assert_eq!(text(tree.root_node(), &[0xff; 9]), "");
    }
}
