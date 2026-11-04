//! Languages, their grammars and the node tables every operation reads.
//!
//! Supporting a language means adding a grammar and a [`Spec`], not new algorithms.

use tree_sitter::Language;

use crate::ast::specs;
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
    Java,
    Kotlin,
    Svelte,
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
    ("java", Lang::Java),
    ("kotlin", Lang::Kotlin),
    ("kt", Lang::Kotlin),
    ("svelte", Lang::Svelte),
];

/// The compiled-in grammar of each language, in the order of the [`Lang`] variants.
const GRAMMARS: [fn() -> Language; 11] = [
    || tree_sitter_go::LANGUAGE.into(),
    || tree_sitter_rust::LANGUAGE.into(),
    || tree_sitter_python::LANGUAGE.into(),
    || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    || tree_sitter_typescript::LANGUAGE_TSX.into(),
    || tree_sitter_javascript::LANGUAGE.into(),
    || tree_sitter_c::LANGUAGE.into(),
    || tree_sitter_cpp::LANGUAGE.into(),
    || tree_sitter_java::LANGUAGE.into(),
    || tree_sitter_kotlin_ng::LANGUAGE.into(),
    || tree_sitter_svelte_ng::LANGUAGE.into(),
];

impl Lang {
    /// Every language, in discriminant order.
    #[cfg(test)]
    pub(crate) const ALL: [Lang; 11] = [
        Lang::Go,
        Lang::Rust,
        Lang::Python,
        Lang::TypeScript,
        Lang::Tsx,
        Lang::JavaScript,
        Lang::C,
        Lang::Cpp,
        Lang::Java,
        Lang::Kotlin,
        Lang::Svelte,
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
        GRAMMARS[self.index()]()
    }

    /// The node tables of this language.
    pub fn spec(self) -> &'static Spec {
        match self {
            Lang::Go => &specs::GO,
            Lang::Rust => &specs::RUST,
            Lang::Python => &specs::PYTHON,
            Lang::TypeScript | Lang::Tsx | Lang::JavaScript => &specs::ECMASCRIPT,
            Lang::C => &specs::C,
            Lang::Cpp => &specs::CPP,
            Lang::Java => &specs::JAVA,
            Lang::Kotlin => &specs::KOTLIN,
            Lang::Svelte => &specs::SVELTE,
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
    /// Parents whose `else` keyword is not an else branch: a Python ternary, a Kotlin `when` entry (`else ->`).
    pub else_exempt: Kinds,
    /// Nodes that hold a boolean operator.
    pub bool_nodes: Kinds,
    /// The operator tokens that are boolean.
    pub bool_ops: Kinds,
    /// Operators of those nodes that add a path (cyclomatic) but are not boolean operators, so no run in the
    /// cognitive score: Kotlin's elvis `?:`.
    pub path_ops: Kinds,
    /// Jumps that always break the flow.
    pub jumps: Kinds,
    /// Jumps that break the flow only when they name a label.
    pub labeled_jumps: Kinds,
    /// The node kinds of a jump's label.
    pub labels: Kinds,
    /// Call expressions, to find recursion.
    pub calls: Kinds,
    /// Where a call names the function it calls.
    pub callee: Callee,
    /// Case arms that are the default one (`default:`, `_ =>`, `else ->`) and add no path of their own.
    pub default_arms: &'static [(&'static str, DefaultArm)],
    /// Kinds of a function's body when the grammar gives it no `body` field (Kotlin's `function_body`).
    pub body_kinds: Kinds,
    /// Regions of other languages inside a file of this one (a Svelte component's `<script>`).
    pub embedded: &'static [Embed],
    /// Declarations whose name node defines a symbol.
    pub definitions: Kinds,
    /// Reference identifier node kinds.
    pub identifiers: Kinds,
}

/// Where a call expression names the function it calls.
#[derive(Debug, Clone, Copy)]
pub enum Callee {
    /// In this field of the call node.
    Field(&'static str),
    /// In its first named child (Kotlin's call expressions have no fields).
    FirstChild,
}

impl Callee {
    /// The node that names the callee of `call`.
    pub(crate) fn of<'t>(self, call: tree_sitter::Node<'t>) -> Option<tree_sitter::Node<'t>> {
        match self {
            Callee::Field(name) => call.child_by_field_name(name),
            Callee::FirstChild => call.named_child(0),
        }
    }
}

/// How to tell the default arm of a `switch`/`match`/`when` from the arms that branch.
#[derive(Debug, Clone, Copy)]
pub enum DefaultArm {
    /// The arm has no child in this field (C's `default:` has no `value`; Kotlin's `else ->` no `condition`).
    NoField(&'static str),
    /// The arm has no named child at all (Java's `default` label).
    NoNamedChild,
    /// The arm's first named child is the wildcard `_` (Rust and Python patterns).
    WildcardPattern,
}

/// Code of another language inside a file: the nodes that hold it, and how to know which language it is.
pub struct Embed {
    /// The node that holds the region together with its tag (`script_element`).
    pub container: &'static str,
    /// The child of the container with the region's text (`raw_text`).
    pub content: &'static str,
    /// The attribute that names the language (`lang`), with its node kinds.
    pub attribute: &'static str,
    /// Node kinds of an attribute, its name and its value (`attribute`, `attribute_name`, `attribute_value`).
    pub attribute_kinds: [&'static str; 3],
    /// The attribute values that name a language; any other value, or none, means `fallback`.
    pub langs: &'static [(&'static str, Lang)],
    pub fallback: Lang,
}

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
    #[case("java", Lang::Java)]
    #[case("Kotlin", Lang::Kotlin)]
    #[case("kt", Lang::Kotlin)]
    #[case("svelte", Lang::Svelte)]
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
    #[case(Lang::Java)]
    #[case(Lang::Kotlin)]
    fn every_language_has_functions_and_a_grammar(#[case] lang: Lang) {
        assert!(!lang.spec().functions.is_empty());
        assert!(!lang.spec().definitions.is_empty());
        assert!(!lang.spec().identifiers.is_empty());
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang.grammar()).is_ok());
    }

    #[test]
    fn svelte_has_no_functions_of_its_own_but_embeds_scripts() {
        let spec = Lang::Svelte.spec();
        assert!(spec.functions.is_empty());
        assert_eq!(spec.embedded.len(), 1);
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&Lang::Svelte.grammar()).is_ok());
    }

    #[rstest]
    #[case::field(Callee::Field("function"), "f(x)", Some("f"))]
    #[case::first_child(Callee::FirstChild, "f(x)", Some("f"))]
    #[case::missing_field(Callee::Field("nope"), "f(x)", None)]
    fn callee_names_the_function_called(
        #[case] callee: Callee,
        #[case] source: &str,
        #[case] want: Option<&str>,
    ) {
        let tree = crate::parse::parse(Lang::JavaScript, source);
        let mut node = tree.root_node();
        while node.kind() != "call_expression" {
            node = node.child(0).expect("a call");
        }
        let got = callee.of(node).map(|n| source[n.byte_range()].to_string());
        assert_eq!(got.as_deref(), want);
    }

    /// `GRAMMARS` is indexed by variant, so a wrong order would give a language another's grammar.
    #[rstest]
    #[case(Lang::Go, "package p\nfunc f() {}\n")]
    #[case(Lang::Rust, "fn f() {}\n")]
    #[case(Lang::Python, "def f():\n    pass\n")]
    #[case(Lang::TypeScript, "function f(a: number): number { return a; }\n")]
    #[case(Lang::Tsx, "const a = <div>{x}</div>;\n")]
    #[case(Lang::JavaScript, "function f(a) { return a; }\n")]
    #[case(Lang::C, "int f(void) { return 0; }\n")]
    #[case(Lang::Cpp, "namespace n { int f() { return 0; } }\n")]
    #[case(Lang::Java, "class A { void f() {} }\n")]
    #[case(Lang::Kotlin, "fun f() {}\n")]
    #[case(Lang::Svelte, "<p>{x}</p>\n")]
    fn each_language_parses_its_own_source(#[case] lang: Lang, #[case] source: &str) {
        let tree = crate::parse::parse(lang, source);
        assert!(!tree.root_node().has_error(), "{lang:?}");
    }
}
