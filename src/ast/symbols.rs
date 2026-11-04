//! Mapping changed lines to the declarations that hold them.

use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use crate::complexity::symbol_name;
use crate::kinds::{table, KindTable, TYPE};
use crate::lang::Lang;
use crate::parse::{parse, text};

/// A changed range of lines, 1-based and inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Hunk {
    pub start_line: usize,
    pub end_line: usize,
}

/// The declaration a hunk changes. A hunk outside every declaration maps to the file scope (`kind` `file`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangedSymbol {
    pub hunk: usize,
    pub symbol: String,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
}

struct Decl {
    symbol: String,
    kind: String,
    start_line: usize,
    end_line: usize,
}

/// For each hunk, the innermost declaration of each of its lines, one entry per run of lines. A run outside
/// every declaration is one `file` entry that spans just those lines.
pub fn changed_symbols(lang: Lang, source: &str, hunks: &[Hunk]) -> Vec<ChangedSymbol> {
    let tree = parse(lang, source);
    let mut decls = Vec::new();
    declarations(tree.root_node(), table(lang), source.as_bytes(), &mut decls);
    let mut out: Vec<ChangedSymbol> = Vec::new();
    for (index, hunk) in hunks.iter().enumerate() {
        let before = out.len();
        for line in hunk.start_line..=hunk.end_line {
            let entry = entry_for(index, line, &decls);
            if !out[before..]
                .last_mut()
                .is_some_and(|last| absorb(last, &entry))
            {
                out.push(entry);
            }
        }
    }
    out
}

/// Folds the next line's entry into `last` when they are the same declaration, or both file scope (the run of
/// lines then grows).
fn absorb(last: &mut ChangedSymbol, next: &ChangedSymbol) -> bool {
    if last.kind == "file" && next.kind == "file" {
        last.end_line = next.end_line;
        return true;
    }
    (&last.symbol, &last.kind, last.start_line) == (&next.symbol, &next.kind, next.start_line)
}

/// Declarations are collected outermost first, so the innermost one covering a line is the last that does.
fn entry_for(index: usize, line: usize, decls: &[Decl]) -> ChangedSymbol {
    match decls
        .iter()
        .rev()
        .find(|d| d.start_line <= line && line <= d.end_line)
    {
        Some(d) => ChangedSymbol {
            hunk: index,
            symbol: d.symbol.clone(),
            kind: d.kind.clone(),
            start_line: d.start_line,
            end_line: d.end_line,
        },
        None => ChangedSymbol {
            hunk: index,
            symbol: String::new(),
            kind: "file".into(),
            start_line: line,
            end_line: line,
        },
    }
}

fn declarations(node: Node, kinds: &KindTable, source: &[u8], out: &mut Vec<Decl>) {
    if kinds.is_function(node) || kinds.has(node, TYPE) {
        out.push(Decl {
            symbol: declared_name(node, source, kinds),
            kind: node.kind().to_string(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
        });
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        declarations(child, kinds, source, out);
    }
}

/// A function's symbol, or a type's `name` (an `impl` block has its `type`).
fn declared_name(node: Node, source: &[u8], kinds: &KindTable) -> String {
    if kinds.is_function(node) {
        return symbol_name(node, source, kinds);
    }
    let named = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("type"));
    named
        .map(|n| text(n, source).to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    const GO: &str = "package p\n\ntype T struct {\n\tx int\n}\n\nfunc (t T) A() {\n\tg := func() {\n\t\tx()\n\t}\n\tg()\n}\n\nfunc B() {}\n";

    fn at(source: &str, lang: Lang, start: usize, end: usize) -> Vec<(String, String, usize)> {
        let got = changed_symbols(
            lang,
            source,
            &[Hunk {
                start_line: start,
                end_line: end,
            }],
        );
        got.into_iter()
            .map(|c| (c.symbol, c.kind, c.start_line))
            .collect()
    }

    #[rstest]
    #[case::method(7, 7, vec![("A", "method_declaration", 7)])]
    #[case::type_field(4, 4, vec![("T", "type_spec", 3)])]
    #[case::function(14, 14, vec![("B", "function_declaration", 14)])]
    #[case::spans_two(7, 14, vec![("A", "method_declaration", 7), ("", "file", 13), ("B", "function_declaration", 14)])]
    #[case::file_scope(1, 1, vec![("", "file", 1)])]
    #[case::file_scope_run(1, 2, vec![("", "file", 1)])]
    fn go(#[case] start: usize, #[case] end: usize, #[case] want: Vec<(&str, &str, usize)>) {
        let want: Vec<_> = want
            .into_iter()
            .map(|(s, k, l)| (s.to_string(), k.to_string(), l))
            .collect();
        assert_eq!(at(GO, Lang::Go, start, end), want);
    }

    #[rstest]
    #[case::rust_impl(Lang::Rust, "struct S;\nimpl S {\n    fn m(&self) {\n        1;\n    }\n}\n", 4, vec![("m", "function_item", 3)])]
    #[case::rust_impl_block(Lang::Rust, "struct S;\nimpl S {\n    const X: u8 = 1;\n}\n", 3, vec![("S", "impl_item", 2)])]
    #[case::python_class(Lang::Python, "class A:\n    x = 1\n", 2, vec![("A", "class_definition", 1)])]
    #[case::ts_interface(Lang::TypeScript, "interface I {\n  x: number;\n}\n", 2, vec![("I", "interface_declaration", 1)])]
    fn other_languages(
        #[case] lang: Lang,
        #[case] source: &str,
        #[case] line: usize,
        #[case] want: Vec<(&str, &str, usize)>,
    ) {
        let want: Vec<_> = want
            .into_iter()
            .map(|(s, k, l)| (s.to_string(), k.to_string(), l))
            .collect();
        assert_eq!(at(source, lang, line, line), want);
    }

    #[test]
    fn an_unnamed_declaration_has_an_empty_symbol() {
        let got = changed_symbols(
            Lang::Cpp,
            "struct { int x; };\n",
            &[Hunk {
                start_line: 1,
                end_line: 1,
            }],
        );
        assert_eq!(got[0].symbol, "");
    }

    #[test]
    fn hunks_keep_their_index_and_empty_input_is_empty() {
        let hunks = [
            Hunk {
                start_line: 14,
                end_line: 14,
            },
            Hunk {
                start_line: 1,
                end_line: 1,
            },
        ];
        let got = changed_symbols(Lang::Go, GO, &hunks);
        assert_eq!(got.iter().map(|c| c.hunk).collect::<Vec<_>>(), [0, 1]);
        assert!(changed_symbols(Lang::Go, GO, &[]).is_empty());
    }
}
