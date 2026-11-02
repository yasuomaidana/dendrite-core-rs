//! The end of the last complete node of a truncated buffer, to splice LLM output that was cut off.

use tree_sitter::Node;

use crate::error::CoreError;
use crate::lang::Lang;
use crate::parse::parse;

/// The end byte of the last top-level or block-level node of `buffer` that holds no `ERROR` or `MISSING`
/// node. The caller keeps `buffer[..offset]` and asks for a continuation.
pub fn last_complete_node(lang: Lang, buffer: &str) -> Result<usize, CoreError> {
    let tree = parse(lang, buffer);
    let root = tree.root_node();
    last_end(root, !root.is_error()).ok_or(CoreError::NoCompleteNode)
}

fn incomplete(node: Node) -> bool {
    node.is_error() || node.is_missing() || node.has_error()
}

/// Node kinds that are whole statements or declarations. Inside an `ERROR` node, header pieces such as names
/// and parameter lists are complete too but are no place to cut.
fn statement_like(node: Node) -> bool {
    [
        "_statement",
        "_declaration",
        "_definition",
        "_item",
        "declaration",
    ]
    .iter()
    .any(|s| node.kind().ends_with(s))
}

/// The furthest end among the complete children of `node` (every one when `take_all`, else the statements) and,
/// inside each incomplete child, among the statements of its body.
fn last_end(node: Node, take_all: bool) -> Option<usize> {
    let mut best = None;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        let found = if incomplete(child) {
            descend(child)
        } else {
            (take_all || statement_like(child)).then(|| child.end_byte())
        };
        best = found.or(best);
    }
    best
}

/// Inside an incomplete node the cut points are the statements of its body, or, when error recovery left no
/// body, the statements it swallowed.
fn descend(node: Node) -> Option<usize> {
    match node.child_by_field_name("body") {
        Some(body) => last_end(body, true),
        None => last_end(node, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn end_of(buffer: &str, needle: &str) -> usize {
        buffer.find(needle).unwrap() + needle.len()
    }

    #[rstest]
    #[case::go(
        Lang::Go,
        "package p\n\nfunc a() {}\n\nfunc b() {\n\tx := 1\n\ty :=",
        "x := 1"
    )]
    #[case::go_whole_declaration(Lang::Go, "package p\n\nfunc a() {}\n\nfunc b(", "func a() {}")]
    #[case::rust(
        Lang::Rust,
        "fn a() {}\nfn b() {\n    let x = 1;\n    let y =",
        "let x = 1;"
    )]
    #[case::python(
        Lang::Python,
        "def a():\n    return 1\n\ndef b():\n    x = 1\n    y = (",
        "x = 1"
    )]
    #[case::typescript(
        Lang::TypeScript,
        "function a() {}\nfunction b() {\n  const x = 1;\n  const y = ",
        "const x = 1;"
    )]
    #[case::c(
        Lang::C,
        "int a(void) { return 1; }\nint b(void) {\n  int x = 1;\n  int y =",
        "int x = 1;"
    )]
    #[case::unclosed_rust_body(
        Lang::Rust,
        "fn b() {\n    let x = 1;\n    let y = 2;\n",
        "let y = 2;"
    )]
    #[case::unclosed_ts_body(
        Lang::TypeScript,
        "function b() {\n  const x = 1;\n  if (x) {\n",
        "const x = 1;"
    )]
    #[case::complete(Lang::Go, "package p\n\nfunc a() {}\n", "func a() {}")]
    fn finds_the_cut(#[case] lang: Lang, #[case] buffer: &str, #[case] last: &str) {
        assert_eq!(last_complete_node(lang, buffer), Ok(end_of(buffer, last)));
    }

    #[rstest]
    #[case::empty("")]
    #[case::blank("  \n")]
    #[case::header_only("func b(")]
    fn reports_when_nothing_is_complete(#[case] buffer: &str) {
        assert_eq!(
            last_complete_node(Lang::Go, buffer),
            Err(CoreError::NoCompleteNode)
        );
    }
}
