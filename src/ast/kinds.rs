//! Node kinds as numbers. A [`Spec`](crate::lang::Spec) names kinds by string, and asking a node for its kind
//! name costs a C call, a `strlen` and a UTF-8 check; deciding a node's part that way, for every node of a
//! tree, was most of the time `ast.complexity` took. A [`KindTable`] resolves the names once per language into
//! flags indexed by the grammar's numeric kind id, so the question per node is one array read.

use std::sync::OnceLock;

use tree_sitter::Node;

use crate::lang::{Kinds, Lang};

pub(crate) const FUNCTION: u32 = 1 << 0;
pub(crate) const TYPE: u32 = 1 << 1;
pub(crate) const CLOSURE: u32 = 1 << 2;
pub(crate) const BINDER: u32 = 1 << 3;
pub(crate) const FLOW: u32 = 1 << 4;
pub(crate) const ELIF: u32 = 1 << 5;
pub(crate) const SWITCH: u32 = 1 << 6;
pub(crate) const CASE: u32 = 1 << 7;
pub(crate) const IF: u32 = 1 << 8;
pub(crate) const ELSE_EXEMPT: u32 = 1 << 9;
pub(crate) const BOOL_NODE: u32 = 1 << 10;
pub(crate) const BOOL_OP: u32 = 1 << 11;
pub(crate) const JUMP: u32 = 1 << 12;
pub(crate) const LABELED_JUMP: u32 = 1 << 13;
pub(crate) const LABEL: u32 = 1 << 14;
pub(crate) const CALL: u32 = 1 << 15;
/// The `else` keyword and the clause that holds it; every grammar spells them the same, so they are not in a
/// [`Spec`](crate::lang::Spec).
pub(crate) const ELSE: u32 = 1 << 16;
pub(crate) const ELSE_CLAUSE: u32 = 1 << 17;
/// An operator that adds a path without being a boolean operator (Kotlin's `?:`).
pub(crate) const PATH_OP: u32 = 1 << 18;

/// The parts a language's node kinds play, by kind id.
pub struct KindTable {
    flags: Vec<u32>,
}

impl KindTable {
    fn build(lang: Lang) -> KindTable {
        let grammar = lang.grammar();
        let spec = lang.spec();
        let groups: [(u32, Kinds); 17] = [
            (FUNCTION, spec.functions),
            (TYPE, spec.types),
            (CLOSURE, spec.closures),
            (BINDER, spec.binders),
            (FLOW, spec.flow),
            (ELIF, spec.elifs),
            (SWITCH, spec.switches),
            (CASE, spec.cases),
            (IF, spec.ifs),
            (ELSE_EXEMPT, spec.else_exempt),
            (BOOL_NODE, spec.bool_nodes),
            (BOOL_OP, spec.bool_ops),
            (PATH_OP, spec.path_ops),
            (JUMP, spec.jumps),
            (LABELED_JUMP, spec.labeled_jumps),
            (LABEL, spec.labels),
            (CALL, spec.calls),
        ];
        let flags = (0..grammar.node_kind_count())
            .map(|id| {
                let name = u16::try_from(id)
                    .ok()
                    .and_then(|id| grammar.node_kind_for_id(id))
                    .unwrap_or("");
                let from_spec = groups
                    .iter()
                    .filter(|(_, kinds)| kinds.contains(&name))
                    .fold(0, |all, (flag, _)| all | flag);
                from_spec
                    | if name == "else" { ELSE } else { 0 }
                    | if name == "else_clause" {
                        ELSE_CLAUSE
                    } else {
                        0
                    }
            })
            .collect();
        KindTable { flags }
    }

    /// Whether the node's kind plays the part `flag` names.
    pub(crate) fn has(&self, node: Node, flag: u32) -> bool {
        self.has_id(node.kind_id(), flag)
    }

    /// Like [`has`](Self::has), for a kind id.
    pub(crate) fn has_id(&self, kind_id: u16, flag: u32) -> bool {
        self.flags
            .get(usize::from(kind_id))
            .is_some_and(|flags| flags & flag != 0)
    }

    /// A declaration with its own metrics, or an inline function bound to a name (`const f = () => ...`).
    pub(crate) fn is_function(&self, node: Node) -> bool {
        self.has(node, FUNCTION)
            || (self.has(node, CLOSURE) && node.parent().is_some_and(|p| self.has(p, BINDER)))
    }
}

static TABLES: [OnceLock<KindTable>; 11] = [const { OnceLock::new() }; 11];

/// The table of `lang`, built on first use.
pub fn table(lang: Lang) -> &'static KindTable {
    TABLES[lang.index()].get_or_init(|| KindTable::build(lang))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use rstest::rstest;

    /// Every node of `source`, parents before children.
    fn nodes(tree: &tree_sitter::Tree) -> Vec<Node<'_>> {
        let mut out = Vec::new();
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            out.push(node);
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            stack.extend(children.into_iter().rev());
        }
        out
    }

    #[rstest]
    #[case(Lang::Go, "package p\nfunc f() {}\n", "function_declaration", FUNCTION)]
    #[case(Lang::Go, "package p\nfunc f() { if true {} }\n", "if_statement", FLOW | IF)]
    #[case(Lang::Go, "package p\nfunc f() { if a {} else {} }\n", "else", ELSE)]
    #[case(
        Lang::Rust,
        "fn f() { let c = |x| x; }\n",
        "closure_expression",
        CLOSURE
    )]
    #[case(Lang::Python, "def f():\n    pass\n", "function_definition", FUNCTION)]
    #[case(
        Lang::Python,
        "if a:\n    pass\nelse:\n    pass\n",
        "else_clause",
        ELSE_CLAUSE
    )]
    fn flags_follow_the_spec(
        #[case] lang: Lang,
        #[case] source: &str,
        #[case] kind: &str,
        #[case] want: u32,
    ) {
        let tree = parse(lang, source);
        let node = nodes(&tree)
            .into_iter()
            .find(|n| n.kind() == kind)
            .expect("the snippet holds the kind");
        assert!(table(lang).has(node, want), "{kind} lacks {want:#b}");
    }

    #[test]
    fn a_plain_identifier_plays_no_part() {
        let tree = parse(Lang::Go, "package p\nvar x = 1\n");
        let ident = nodes(&tree)
            .into_iter()
            .find(|n| n.kind() == "identifier")
            .unwrap();
        assert!(!table(Lang::Go).has(ident, u32::MAX));
    }

    #[test]
    fn an_unknown_kind_id_has_no_part() {
        assert!(!table(Lang::Go).has_id(u16::MAX, FUNCTION));
    }

    #[test]
    fn every_language_builds_a_table() {
        for lang in Lang::ALL {
            assert!(!table(lang).flags.is_empty());
        }
    }
}
