//! Cyclomatic (McCabe) and cognitive (SonarSource) complexity per function.
//!
//! Cyclomatic is 1 plus the decision points: branches, loops, case arms other than the default, `catch`,
//! boolean operators, ternaries and Rust's `?`.
//!
//! Cognitive adds, for each structure that breaks the linear flow, 1 plus its nesting depth; for `else if`
//! and `else`, 1; for each run of one boolean operator, 1; for a labelled jump or a recursive call, 1.
//!
//! The trees are walked with one cursor each and nodes are classified by kind id ([`KindTable`]), never by
//! kind name: that is what keeps this faster than parsing the source (DCORE-13).

use serde::Serialize;
use tree_sitter::Node;

use crate::embedded::regions;
use crate::kinds::{
    table, KindTable, BOOL_NODE, BOOL_OP, CALL, CASE, CLOSURE, ELIF, ELSE, ELSE_CLAUSE,
    ELSE_EXEMPT, FLOW, IF, JUMP, LABEL, LABELED_JUMP, PATH_OP, SWITCH,
};
use crate::lang::{DefaultArm, Lang, Spec};
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
    if !lang.spec().embedded.is_empty() {
        return embedded_complexity(lang, source);
    }
    if lang == Lang::Go {
        if let Some(found) = crate::ast::go::fast::complexity(source) {
            return found;
        }
    }
    complexity_tree(lang, source)
}

/// The functions of the regions of other languages inside the file, numbered by the lines of the file.
fn embedded_complexity(lang: Lang, source: &str) -> Vec<FunctionMetrics> {
    regions(lang, source)
        .iter()
        .flat_map(|region| {
            let found = complexity(region.lang, &source[region.start..region.end]);
            found.into_iter().map(move |mut m| {
                m.start_line += region.row;
                m.end_line += region.row;
                m
            })
        })
        .collect()
}

/// [`complexity`] through the syntax tree, for every language: the parser path, and the oracle the Go fast path
/// is checked against.
pub fn complexity_tree(lang: Lang, source: &str) -> Vec<FunctionMetrics> {
    let tree = parse(lang, source);
    let kinds = table(lang);
    let mut out = Vec::new();
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        if kinds.is_function(node) {
            out.push(measure(node, kinds, lang.spec(), source.as_bytes()));
        }
        if cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return out;
            }
        }
    }
}

/// The node that names a function: its `name`, or the innermost `declarator` (C and C++).
pub(crate) fn name_node(node: Node) -> Option<Node> {
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

pub(crate) fn symbol_name(node: Node, source: &[u8], kinds: &KindTable) -> String {
    let owner = match node.parent() {
        Some(parent) if kinds.has(node, CLOSURE) => parent,
        _ => node,
    };
    name_node(owner)
        .map(|n| text(n, source).to_string())
        .unwrap_or_default()
}

fn measure(func: Node, kinds: &KindTable, spec: &'static Spec, source: &[u8]) -> FunctionMetrics {
    let symbol = symbol_name(func, source, kinds);
    let mut scorer = Scorer {
        kinds,
        spec,
        source,
        name: &symbol,
        cyclomatic: 1,
        cognitive: 0,
    };
    scorer.score(func);
    FunctionMetrics {
        start_line: func.start_position().row + 1,
        end_line: func.end_position().row + 1,
        cyclomatic: scorer.cyclomatic,
        cognitive: scorer.cognitive,
        symbol,
    }
}

struct Scorer<'a> {
    kinds: &'a KindTable,
    spec: &'static Spec,
    source: &'a [u8],
    name: &'a str,
    cyclomatic: u32,
    cognitive: u32,
}

impl Scorer<'_> {
    /// Scores `func` and everything in it that is not a function of its own. One cursor walks the subtree;
    /// `nesting` holds, for each open node, the level its children are at.
    fn score(&mut self, func: Node) {
        self.cyclomatic += self.decisions(func);
        let mut cursor = func.walk();
        let mut nesting = vec![0u32];
        if !cursor.goto_first_child() {
            return;
        }
        loop {
            let node = cursor.node();
            if !self.kinds.is_function(node) {
                self.cyclomatic += self.decisions(node);
                let deeper = self.cognitive(node, nesting[nesting.len() - 1]);
                nesting.push(deeper);
                if cursor.goto_first_child() {
                    continue;
                }
                nesting.pop();
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() || cursor.node() == func {
                    return;
                }
                nesting.pop();
            }
        }
    }

    /// Decision points this node adds to the cyclomatic count.
    fn decisions(&self, node: Node) -> u32 {
        let counts = self.kinds.has(node, FLOW | ELIF)
            || (self.kinds.has(node, CASE) && !self.is_default_arm(node));
        u32::from(counts)
            + u32::from(self.boolean_operator(node).is_some())
            + u32::from(self.path_operator(node))
    }

    /// An operator that adds a path but is not a boolean operator (Kotlin's `?:`): cyclomatic only.
    fn path_operator(&self, node: Node) -> bool {
        self.kinds.has(node, BOOL_NODE)
            && node
                .child_by_field_name("operator")
                .is_some_and(|op| self.kinds.has(op, PATH_OP))
    }

    /// The default arm of a `match`/`switch`/`when`: it adds no path of its own.
    fn is_default_arm(&self, node: Node) -> bool {
        let kind = node.kind();
        let rule = self.spec.default_arms.iter().find(|(k, _)| *k == kind);
        rule.is_some_and(|(_, rule)| match rule {
            DefaultArm::NoField(field) => node.child_by_field_name(field).is_none(),
            DefaultArm::NoNamedChild => node.named_child_count() == 0,
            DefaultArm::WildcardPattern => node
                .named_child(0)
                .is_some_and(|pattern| text(pattern, self.source).trim() == "_"),
        })
    }

    /// The kind id of the boolean operator this node applies, if it applies one.
    fn boolean_operator(&self, node: Node) -> Option<u16> {
        if !self.kinds.has(node, BOOL_NODE) {
            return None;
        }
        let op = node.child_by_field_name("operator")?.kind_id();
        self.kinds.has_id(op, BOOL_OP).then_some(op)
    }

    /// Adds the cognitive cost of this node and returns the nesting level its children are at.
    fn cognitive(&mut self, node: Node, nesting: u32) -> u32 {
        if self.is_else(node) || self.kinds.has(node, ELIF) {
            self.cognitive += 1;
            return nesting;
        }
        if self.kinds.has(node, FLOW | SWITCH) {
            return self.structure(node, nesting);
        }
        if self.kinds.has(node, CLOSURE) {
            return nesting + 1;
        }
        self.cognitive += self.flat_cost(node);
        nesting
    }

    /// An `else` keyword; the one of a Python ternary is part of the ternary, which is already paid.
    fn is_else(&self, node: Node) -> bool {
        self.kinds.has(node, ELSE)
            && node
                .parent()
                .is_some_and(|p| !self.kinds.has(p, ELSE_EXEMPT))
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
        self.kinds.has(node, IF)
            && (node.prev_sibling().is_some_and(|s| self.kinds.has(s, ELSE))
                || node
                    .parent()
                    .is_some_and(|p| self.kinds.has(p, ELSE_CLAUSE)))
    }

    /// Costs that do not nest: jumps, recursion and the first operator of each boolean run.
    fn flat_cost(&self, node: Node) -> u32 {
        let jump = self.kinds.has(node, JUMP)
            || (self.kinds.has(node, LABELED_JUMP) && self.has_label(node));
        u32::from(jump)
            + u32::from(self.is_recursive_call(node))
            + u32::from(self.starts_bool_run(node))
    }

    fn has_label(&self, node: Node) -> bool {
        let mut cursor = node.walk();
        let found = node
            .named_children(&mut cursor)
            .any(|c| self.kinds.has(c, LABEL));
        found
    }

    fn starts_bool_run(&self, node: Node) -> bool {
        let Some(op) = self.boolean_operator(node) else {
            return false;
        };
        node.parent().and_then(|p| self.boolean_operator(p)) != Some(op)
    }

    fn is_recursive_call(&self, node: Node) -> bool {
        if self.name.is_empty() || !self.kinds.has(node, CALL) {
            return false;
        }
        let callee = self
            .spec
            .callee
            .of(node)
            .map(|f| text(f, self.source))
            .unwrap_or("");
        callee.rsplit(['.', ':']).next() == Some(self.name)
    }
}

#[cfg(test)]
#[path = "complexity_tests.rs"]
mod tests;
