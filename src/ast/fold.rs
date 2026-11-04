//! Signature folding: function bodies become a placeholder, everything else stays.

use serde::Serialize;
use tree_sitter::Node;

use crate::complexity::symbol_name;
use crate::kinds::{table, KindTable};
use crate::lang::Lang;
use crate::parse::{parse, text};
use crate::tokens::estimate;

const PLACEHOLDER: &str = "{ /* pruned */ }";

/// The folded source and what it saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Folded {
    pub source: String,
    pub folded: Vec<String>,
    pub tokens_before: usize,
    pub tokens_after: usize,
}

struct Edit {
    start: usize,
    end: usize,
    replacement: String,
}

/// Replaces the body of every function whose symbol is not in `keep` with a placeholder (`...` in Python).
/// Types, fields, contracts, doc comments, imports and constants stay; kept symbols keep their bodies.
pub fn fold_signatures(lang: Lang, source: &str, keep: &[String]) -> Folded {
    let tree = parse(lang, source);
    let mut edits = Vec::new();
    let mut folded = Vec::new();
    let ctx = Ctx {
        lang,
        kinds: table(lang),
        source,
        keep,
    };
    ctx.visit(tree.root_node(), &mut edits, &mut folded);
    let out = apply(source, &edits);
    Folded {
        tokens_before: estimate(source),
        tokens_after: estimate(&out),
        source: out,
        folded,
    }
}

struct Ctx<'a> {
    lang: Lang,
    kinds: &'static KindTable,
    source: &'a str,
    keep: &'a [String],
}

impl Ctx<'_> {
    fn visit(&self, node: Node, edits: &mut Vec<Edit>, folded: &mut Vec<String>) {
        if self.kinds.is_function(node) {
            self.fold(node, edits, folded);
            return;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit(child, edits, folded);
        }
    }

    /// A function is folded or kept whole; nothing inside it is visited either way.
    fn fold(&self, func: Node, edits: &mut Vec<Edit>, folded: &mut Vec<String>) {
        let symbol = symbol_name(func, self.source.as_bytes(), self.kinds);
        let Some(body) = func.child_by_field_name("body") else {
            return;
        };
        if self.keep.contains(&symbol) {
            return;
        }
        edits.push(Edit {
            start: body.start_byte(),
            end: body.end_byte(),
            replacement: self.replacement(body),
        });
        folded.push(symbol);
    }

    fn replacement(&self, body: Node) -> String {
        if self.lang != Lang::Python {
            return PLACEHOLDER.to_string();
        }
        let indent = indent_before(self.source, body.start_byte());
        match docstring(body, self.source) {
            Some(doc) => format!("{doc}\n{indent}..."),
            None => "...".to_string(),
        }
    }
}

/// The docstring a Python body starts with, if any.
fn docstring<'a>(body: Node, source: &'a str) -> Option<&'a str> {
    let first = body
        .named_child(0)
        .filter(|f| f.kind() == "expression_statement");
    let string = first
        .and_then(|f| f.named_child(0))
        .filter(|s| s.kind() == "string");
    string.map(|s| text(s, source.as_bytes()))
}

/// The whitespace in front of `byte` on its line (empty when the line holds code before it).
fn indent_before(source: &str, byte: usize) -> &str {
    let line_start = source[..byte].rfind('\n').map_or(0, |i| i + 1);
    let prefix = &source[line_start..byte];
    if prefix.trim().is_empty() {
        prefix
    } else {
        ""
    }
}

fn apply(source: &str, edits: &[Edit]) -> String {
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for edit in edits {
        out.push_str(&source[at..edit.start]);
        out.push_str(&edit.replacement);
        at = edit.end;
    }
    out.push_str(&source[at..]);
    out
}

#[cfg(test)]
#[path = "fold_tests.rs"]
mod tests;
