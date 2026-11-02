//! Cyclomatic (McCabe) and cognitive (SonarSource) complexity per function.
//!
//! Cyclomatic is 1 plus the decision points: branches, loops, case arms other than the default, `catch`,
//! boolean operators, ternaries and Rust's `?`.
//!
//! Cognitive adds, for each structure that breaks the linear flow, 1 plus its nesting depth; for `else if`
//! and `else`, 1; for each run of one boolean operator, 1; for a labelled jump or a recursive call, 1.

use serde::Serialize;
use tree_sitter::Node;

use crate::lang::{Lang, Spec};
use crate::parse::{parse, text};

/// The limits quality tooling holds a function to.
pub const MAX_CYCLOMATIC: u32 = 10;
pub const MAX_COGNITIVE: u32 = 15;

/// The complexity of one function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FunctionMetrics {
    pub symbol: String,
    pub start_line: usize,
    pub end_line: usize,
    pub cyclomatic: u32,
    pub cognitive: u32,
}

/// Measures every function of `source`, in source order. Nested declarations get their own entry and do not
/// count toward the function around them.
pub fn complexity(lang: Lang, source: &str) -> Vec<FunctionMetrics> {
    let tree = parse(lang, source);
    let mut out = Vec::new();
    collect(tree.root_node(), lang.spec(), source.as_bytes(), &mut out);
    out
}

fn has(kinds: &[&str], kind: &str) -> bool {
    kinds.contains(&kind)
}

pub(crate) fn is_function(node: Node, spec: &Spec) -> bool {
    let kind = node.kind();
    has(spec.functions, kind)
        || (has(spec.closures, kind) && node.parent().is_some_and(|p| has(spec.binders, p.kind())))
}

/// The node that names a function: its `name`, or the innermost `declarator` (C and C++).
fn name_node(node: Node) -> Option<Node> {
    let declarator = node.child_by_field_name("declarator");
    node.child_by_field_name("name")
        .or_else(|| declarator.and_then(declarator_name))
}

/// A declarator names the function when it has none inside; otherwise the name is further in.
fn declarator_name(declarator: Node) -> Option<Node> {
    if declarator.child_by_field_name("declarator").is_some() {
        name_node(declarator)
    } else {
        Some(declarator)
    }
}

pub(crate) fn symbol_name(node: Node, source: &[u8], spec: &Spec) -> String {
    let owner = match node.parent() {
        Some(parent) if has(spec.closures, node.kind()) => parent,
        _ => node,
    };
    name_node(owner)
        .map(|n| text(n, source).to_string())
        .unwrap_or_default()
}

fn collect(node: Node, spec: &Spec, source: &[u8], out: &mut Vec<FunctionMetrics>) {
    if is_function(node, spec) {
        out.push(measure(node, spec, source));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, spec, source, out);
    }
}

fn measure(func: Node, spec: &Spec, source: &[u8]) -> FunctionMetrics {
    let symbol = symbol_name(func, source, spec);
    let mut scorer = Scorer {
        spec,
        source,
        name: &symbol,
        cyclomatic: 1,
        cognitive: 0,
    };
    scorer.walk(func, 0, true);
    FunctionMetrics {
        start_line: func.start_position().row + 1,
        end_line: func.end_position().row + 1,
        cyclomatic: scorer.cyclomatic,
        cognitive: scorer.cognitive,
        symbol,
    }
}

struct Scorer<'a> {
    spec: &'a Spec,
    source: &'a [u8],
    name: &'a str,
    cyclomatic: u32,
    cognitive: u32,
}

impl Scorer<'_> {
    fn walk(&mut self, node: Node, nesting: u32, root: bool) {
        if !root && is_function(node, self.spec) {
            return;
        }
        self.cyclomatic += self.decisions(node);
        let deeper = if root {
            nesting
        } else {
            self.cognitive(node, nesting)
        };
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk(child, deeper, false);
        }
    }

    /// Decision points this node adds to the cyclomatic count.
    fn decisions(&self, node: Node) -> u32 {
        let kind = node.kind();
        let counts = has(self.spec.flow, kind)
            || has(self.spec.elifs, kind)
            || (has(self.spec.cases, kind) && !self.is_default_arm(node));
        u32::from(counts) + u32::from(self.boolean_operator(node).is_some())
    }

    /// The `_` arm of a `match`/`case`, and C's `default:`; they add no path of their own.
    fn is_default_arm(&self, node: Node) -> bool {
        match node.kind() {
            "case_statement" => node.child_by_field_name("value").is_none(),
            "match_arm" | "case_clause" => node
                .named_child(0)
                .is_some_and(|pattern| text(pattern, self.source).trim() == "_"),
            _ => false,
        }
    }

    fn boolean_operator(&self, node: Node) -> Option<&'static str> {
        if !has(self.spec.bool_nodes, node.kind()) {
            return None;
        }
        let op = node
            .child_by_field_name("operator")
            .map_or("", |n| n.kind());
        self.spec
            .bool_ops
            .iter()
            .copied()
            .find(|candidate| *candidate == op)
    }

    /// Adds the cognitive cost of this node and returns the nesting level its children are at.
    fn cognitive(&mut self, node: Node, nesting: u32) -> u32 {
        let kind = node.kind();
        if self.is_else(node) || has(self.spec.elifs, kind) {
            self.cognitive += 1;
            return nesting;
        }
        if has(self.spec.flow, kind) || has(self.spec.switches, kind) {
            return self.structure(node, nesting);
        }
        if has(self.spec.closures, kind) {
            return nesting + 1;
        }
        self.cognitive += self.flat_cost(node);
        nesting
    }

    /// An `else` keyword; the one of a Python ternary is part of the ternary, which is already paid.
    fn is_else(&self, node: Node) -> bool {
        node.kind() == "else"
            && node
                .parent()
                .is_some_and(|p| !has(self.spec.ternaries, p.kind()))
    }

    /// A structure that nests: an `else if` already paid through its `else`, anything else pays its depth.
    fn structure(&mut self, node: Node, nesting: u32) -> u32 {
        if self.follows_else(node) {
            return nesting;
        }
        self.cognitive += 1 + nesting;
        nesting + 1
    }

    fn follows_else(&self, node: Node) -> bool {
        has(self.spec.ifs, node.kind())
            && (node.prev_sibling().is_some_and(|s| s.kind() == "else")
                || node.parent().is_some_and(|p| p.kind() == "else_clause"))
    }

    /// Costs that do not nest: jumps, recursion and the first operator of each boolean run.
    fn flat_cost(&self, node: Node) -> u32 {
        let kind = node.kind();
        let jump = has(self.spec.jumps, kind)
            || (has(self.spec.labeled_jumps, kind) && self.has_label(node));
        u32::from(jump)
            + u32::from(self.is_recursive_call(node))
            + u32::from(self.starts_bool_run(node))
    }

    fn has_label(&self, node: Node) -> bool {
        let mut cursor = node.walk();
        let found = node
            .named_children(&mut cursor)
            .any(|c| has(self.spec.labels, c.kind()));
        found
    }

    fn starts_bool_run(&self, node: Node) -> bool {
        let Some(op) = self.boolean_operator(node) else {
            return false;
        };
        node.parent().and_then(|p| self.boolean_operator(p)) != Some(op)
    }

    fn is_recursive_call(&self, node: Node) -> bool {
        if self.name.is_empty() || !has(self.spec.calls, node.kind()) {
            return false;
        }
        let callee = node
            .child_by_field_name("function")
            .map(|f| text(f, self.source))
            .unwrap_or("");
        callee.rsplit(['.', ':']).next() == Some(self.name)
    }
}

#[cfg(test)]
#[path = "complexity_tests.rs"]
mod tests;
