//! Languages, their grammars and the node tables every operation reads.
//!
//! Supporting a language means adding a grammar and a [`Spec`], not new algorithms.

use tree_sitter::Language;

use crate::error::CoreError;

/// A language with a compiled-in grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Go,
    Rust,
    Python,
    TypeScript,
    Tsx,
    JavaScript,
    C,
    Cpp,
}

const IDS: &[(&str, Lang)] = &[
    ("go", Lang::Go),
    ("golang", Lang::Go),
    ("rust", Lang::Rust),
    ("rs", Lang::Rust),
    ("python", Lang::Python),
    ("py", Lang::Python),
    ("typescript", Lang::TypeScript),
    ("ts", Lang::TypeScript),
    ("tsx", Lang::Tsx),
    ("javascript", Lang::JavaScript),
    ("js", Lang::JavaScript),
    ("c", Lang::C),
    ("cpp", Lang::Cpp),
    ("c++", Lang::Cpp),
];

impl Lang {
    /// Every language, in discriminant order.
    #[cfg(test)]
    pub(crate) const ALL: [Lang; 8] = [
        Lang::Go,
        Lang::Rust,
        Lang::Python,
        Lang::TypeScript,
        Lang::Tsx,
        Lang::JavaScript,
        Lang::C,
        Lang::Cpp,
    ];

    /// The language's position in [`Lang::ALL`], for tables that hold one entry per language.
    pub(crate) fn index(self) -> usize {
        self as usize
    }

    /// Looks a language up by its id (`go`, `rust`, `ts`, ...); callers always name the language.
    pub fn from_id(id: &str) -> Result<Lang, CoreError> {
        let wanted = id.trim().to_ascii_lowercase();
        IDS.iter()
            .find(|(name, _)| *name == wanted)
            .map(|(_, lang)| *lang)
            .ok_or_else(|| CoreError::UnknownLanguage(id.to_string()))
    }

    pub(crate) fn grammar(self) -> Language {
        match self {
            Lang::Go => tree_sitter_go::LANGUAGE.into(),
            Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
            Lang::Python => tree_sitter_python::LANGUAGE.into(),
            Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Lang::C => tree_sitter_c::LANGUAGE.into(),
            Lang::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        }
    }

    /// The node tables of this language.
    pub fn spec(self) -> &'static Spec {
        match self {
            Lang::Go => &GO,
            Lang::Rust => &RUST,
            Lang::Python => &PYTHON,
            Lang::TypeScript | Lang::Tsx | Lang::JavaScript => &ECMASCRIPT,
            Lang::C => &C,
            Lang::Cpp => &CPP,
        }
    }
}

pub(crate) type Kinds = &'static [&'static str];

/// Which grammar node kinds play which part. Kinds are matched by name.
pub struct Spec {
    /// Declarations that get their own metrics (and that folding empties).
    pub functions: Kinds,
    /// Type declarations, which a changed line can also belong to.
    pub types: Kinds,
    /// Inline functions: scored with the code around them, one nesting level deeper.
    pub closures: Kinds,
    /// Parents that turn a closure into a named function (`const f = () => ...`).
    pub binders: Kinds,
    /// Branches and loops: +1 cyclomatic, +1 and the nesting depth cognitive.
    pub flow: Kinds,
    /// `else if` clauses that are nodes of their own (Python `elif`).
    pub elifs: Kinds,
    /// `switch`/`match` containers: cognitive only, once for all their cases.
    pub switches: Kinds,
    /// Case arms and other single decision points: cyclomatic only.
    pub cases: Kinds,
    /// Kinds of the `if` branch, to tell an `else if` from an `if`.
    pub ifs: Kinds,
    /// Ternaries that spell `else`, so their keyword is not an `else` branch.
    pub ternaries: Kinds,
    /// Nodes that hold a boolean operator.
    pub bool_nodes: Kinds,
    /// The operator tokens that are boolean.
    pub bool_ops: Kinds,
    /// Jumps that always break the flow.
    pub jumps: Kinds,
    /// Jumps that break the flow only when they name a label.
    pub labeled_jumps: Kinds,
    /// The node kinds of a jump's label.
    pub labels: Kinds,
    /// Call expressions, to find recursion.
    pub calls: Kinds,
    /// Declarations whose name node defines a symbol.
    pub definitions: Kinds,
    /// Reference identifier node kinds.
    pub identifiers: Kinds,
}

const BOOL: Kinds = &["&&", "||"];

static GO: Spec = Spec {
    functions: &["function_declaration", "method_declaration"],
    types: &["type_spec"],
    closures: &["func_literal"],
    binders: &[],
    flow: &["if_statement", "for_statement"],
    elifs: &[],
    switches: &[
        "expression_switch_statement",
        "type_switch_statement",
        "select_statement",
    ],
    cases: &["expression_case", "type_case", "communication_case"],
    ifs: &["if_statement"],
    ternaries: &[],
    bool_nodes: &["binary_expression"],
    bool_ops: BOOL,
    jumps: &["goto_statement"],
    labeled_jumps: &["break_statement", "continue_statement"],
    labels: &["label_name"],
    calls: &["call_expression"],
    definitions: &[
        "function_declaration",
        "method_declaration",
        "type_spec",
        "const_spec",
        "var_spec",
    ],
    identifiers: &["identifier", "type_identifier", "field_identifier"],
};

static RUST: Spec = Spec {
    functions: &["function_item"],
    types: &[
        "struct_item",
        "enum_item",
        "trait_item",
        "impl_item",
        "mod_item",
    ],
    closures: &["closure_expression"],
    binders: &[],
    flow: &[
        "if_expression",
        "for_expression",
        "while_expression",
        "loop_expression",
    ],
    elifs: &[],
    switches: &["match_expression"],
    cases: &["match_arm", "try_expression"],
    ifs: &["if_expression"],
    ternaries: &[],
    bool_nodes: &["binary_expression"],
    bool_ops: BOOL,
    jumps: &[],
    labeled_jumps: &["break_expression", "continue_expression"],
    labels: &["label"],
    calls: &["call_expression"],
    definitions: &[
        "function_item",
        "struct_item",
        "enum_item",
        "union_item",
        "trait_item",
        "type_item",
        "const_item",
        "static_item",
        "mod_item",
        "macro_definition",
    ],
    identifiers: &[
        "identifier",
        "type_identifier",
        "field_identifier",
        "shorthand_field_identifier",
    ],
};

static PYTHON: Spec = Spec {
    functions: &["function_definition"],
    types: &["class_definition"],
    closures: &["lambda"],
    binders: &[],
    flow: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "except_clause",
        "conditional_expression",
    ],
    elifs: &["elif_clause"],
    switches: &["match_statement"],
    cases: &["case_clause", "for_in_clause", "if_clause"],
    ifs: &["if_statement"],
    ternaries: &["conditional_expression"],
    bool_nodes: &["boolean_operator"],
    bool_ops: &["and", "or"],
    jumps: &[],
    labeled_jumps: &[],
    labels: &[],
    calls: &["call"],
    definitions: &["function_definition", "class_definition"],
    identifiers: &["identifier"],
};

static ECMASCRIPT: Spec = Spec {
    functions: &[
        "function_declaration",
        "generator_function_declaration",
        "method_definition",
    ],
    types: &[
        "class_declaration",
        "interface_declaration",
        "type_alias_declaration",
        "enum_declaration",
    ],
    closures: &["arrow_function", "function_expression", "function"],
    binders: &["variable_declarator"],
    flow: &[
        "if_statement",
        "for_statement",
        "for_in_statement",
        "while_statement",
        "do_statement",
        "catch_clause",
        "ternary_expression",
    ],
    elifs: &[],
    switches: &["switch_statement"],
    cases: &["switch_case"],
    ifs: &["if_statement"],
    ternaries: &[],
    bool_nodes: &["binary_expression"],
    bool_ops: BOOL,
    jumps: &[],
    labeled_jumps: &["break_statement", "continue_statement"],
    labels: &["statement_identifier"],
    calls: &["call_expression"],
    definitions: &[
        "function_declaration",
        "generator_function_declaration",
        "class_declaration",
        "method_definition",
        "interface_declaration",
        "type_alias_declaration",
        "enum_declaration",
    ],
    identifiers: &[
        "identifier",
        "type_identifier",
        "property_identifier",
        "shorthand_property_identifier",
    ],
};

static C: Spec = Spec {
    functions: &["function_definition"],
    types: &["struct_specifier"],
    closures: &[],
    binders: &[],
    flow: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "do_statement",
        "conditional_expression",
    ],
    elifs: &[],
    switches: &["switch_statement"],
    cases: &["case_statement"],
    ifs: &["if_statement"],
    ternaries: &[],
    bool_nodes: &["binary_expression"],
    bool_ops: BOOL,
    jumps: &["goto_statement"],
    labeled_jumps: &[],
    labels: &[],
    calls: &["call_expression"],
    definitions: &[
        "function_definition",
        "struct_specifier",
        "union_specifier",
        "enum_specifier",
        "type_definition",
    ],
    identifiers: &["identifier", "type_identifier", "field_identifier"],
};

static CPP: Spec = Spec {
    functions: &["function_definition"],
    types: &[
        "class_specifier",
        "struct_specifier",
        "namespace_definition",
    ],
    closures: &["lambda_expression"],
    binders: &[],
    flow: &[
        "if_statement",
        "for_statement",
        "for_range_loop",
        "while_statement",
        "do_statement",
        "catch_clause",
        "conditional_expression",
    ],
    elifs: &[],
    switches: &["switch_statement"],
    cases: &["case_statement"],
    ifs: &["if_statement"],
    ternaries: &[],
    bool_nodes: &["binary_expression"],
    bool_ops: BOOL,
    jumps: &["goto_statement"],
    labeled_jumps: &[],
    labels: &[],
    calls: &["call_expression"],
    definitions: &[
        "function_definition",
        "class_specifier",
        "struct_specifier",
        "union_specifier",
        "enum_specifier",
        "type_definition",
        "alias_declaration",
        "namespace_definition",
        "concept_definition",
    ],
    identifiers: &[
        "identifier",
        "type_identifier",
        "field_identifier",
        "namespace_identifier",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("go", Lang::Go)]
    #[case("  Rust ", Lang::Rust)]
    #[case("PY", Lang::Python)]
    #[case("ts", Lang::TypeScript)]
    #[case("tsx", Lang::Tsx)]
    #[case("js", Lang::JavaScript)]
    #[case("c", Lang::C)]
    #[case("c++", Lang::Cpp)]
    fn finds_ids(#[case] id: &str, #[case] want: Lang) {
        assert_eq!(Lang::from_id(id), Ok(want));
    }

    #[test]
    fn rejects_unknown_id() {
        assert_eq!(
            Lang::from_id("cobol"),
            Err(CoreError::UnknownLanguage("cobol".into()))
        );
    }

    #[rstest]
    #[case(Lang::Go)]
    #[case(Lang::Rust)]
    #[case(Lang::Python)]
    #[case(Lang::TypeScript)]
    #[case(Lang::Tsx)]
    #[case(Lang::JavaScript)]
    #[case(Lang::C)]
    #[case(Lang::Cpp)]
    fn every_language_has_functions_and_a_grammar(#[case] lang: Lang) {
        assert!(!lang.spec().functions.is_empty());
        assert!(!lang.spec().definitions.is_empty());
        assert!(!lang.spec().identifiers.is_empty());
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang.grammar()).is_ok());
    }
}
