//! Symbol definitions and identifier uses for line ranges.

use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use crate::complexity::name_node;
use crate::lang::{Lang, Spec};
use crate::parse::{parse, text};
use crate::symbols::Hunk;

/// The symbols defined and identifiers used in one line range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeSymbols {
    pub range: usize,
    pub defines: Vec<String>,
    pub uses: Vec<String>,
}

struct DeclEntry<'a> {
    name: &'a str,
    line: usize,
}

struct UseEntry<'a> {
    name: &'a str,
    line: usize,
}

const LITERALS: &[&str] = &[
    "true",
    "false",
    "nil",
    "null",
    "None",
    "NULL",
    "nullptr",
    "undefined",
];

fn is_literal(s: &str) -> bool {
    LITERALS.contains(&s)
}

fn is_decl_name(node: Node) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.child_by_field_name("name") == Some(node) {
        return true;
    }
    parent.child_by_field_name("declarator").is_some() && name_node(parent) == Some(node)
}

fn check_decl<'a>(node: Node<'a>, spec: &Spec, source: &'a [u8], decls: &mut Vec<DeclEntry<'a>>) {
    if !spec.definitions.contains(&node.kind()) {
        return;
    }
    if let Some(name) = name_node(node) {
        let s = text(name, source);
        if !s.is_empty() {
            decls.push(DeclEntry {
                name: s,
                line: name.start_position().row + 1,
            });
        }
    }
}

fn check_use<'a>(node: Node<'a>, spec: &Spec, source: &'a [u8], uses: &mut Vec<UseEntry<'a>>) {
    if !spec.identifiers.contains(&node.kind()) {
        return;
    }
    if is_decl_name(node) {
        return;
    }
    let s = text(node, source);
    if !s.is_empty() && !is_literal(s) {
        uses.push(UseEntry {
            name: s,
            line: node.start_position().row + 1,
        });
    }
}

fn collect_nodes<'a>(
    node: Node<'a>,
    spec: &Spec,
    source: &'a [u8],
    decls: &mut Vec<DeclEntry<'a>>,
    uses: &mut Vec<UseEntry<'a>>,
) {
    check_decl(node, spec, source, decls);
    check_use(node, spec, source, uses);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_nodes(child, spec, source, decls, uses);
    }
}

fn in_range(line: usize, hunk: &Hunk) -> bool {
    line >= hunk.start_line && line <= hunk.end_line
}

fn collect_defines(hunk: &Hunk, decls: &[DeclEntry]) -> Vec<String> {
    let mut defines: Vec<String> = decls
        .iter()
        .filter(|d| in_range(d.line, hunk))
        .map(|d| d.name.to_string())
        .collect();
    defines.sort();
    defines.dedup();
    defines
}

fn collect_uses(hunk: &Hunk, uses: &[UseEntry], defines: &[String]) -> Vec<String> {
    let mut result: Vec<String> = uses
        .iter()
        .filter(|u| in_range(u.line, hunk) && !defines.contains(&u.name.to_string()))
        .map(|u| u.name.to_string())
        .collect();
    result.sort();
    result.dedup();
    result
}

fn process_range(
    index: usize,
    hunk: &Hunk,
    decls: &[DeclEntry],
    uses: &[UseEntry],
) -> RangeSymbols {
    let defines = collect_defines(hunk, decls);
    let uses = collect_uses(hunk, uses, &defines);
    RangeSymbols {
        range: index,
        defines,
        uses,
    }
}

/// Identifies symbols defined and identifiers used within the given line ranges.
pub fn defs_uses(lang: Lang, source: &str, ranges: &[Hunk]) -> Vec<RangeSymbols> {
    let tree = parse(lang, source);
    let source_bytes = source.as_bytes();
    let spec = lang.spec();
    let mut decls = Vec::new();
    let mut uses = Vec::new();
    collect_nodes(tree.root_node(), spec, source_bytes, &mut decls, &mut uses);
    ranges
        .iter()
        .enumerate()
        .map(|(index, hunk)| process_range(index, hunk, &decls, &uses))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::go(Lang::Go, "package p\n\nfunc f() {\n\tg()\n}\n", 3, 5, "f", "g")]
    #[case::rust(Lang::Rust, "fn f() {\n    g();\n}\n", 1, 3, "f", "g")]
    #[case::python(Lang::Python, "def f():\n    g()\n", 1, 2, "f", "g")]
    #[case::typescript(Lang::TypeScript, "function f() {\n  g();\n}\n", 1, 3, "f", "g")]
    #[case::tsx(Lang::Tsx, "function f() {\n  g();\n}\n", 1, 3, "f", "g")]
    #[case::javascript(Lang::JavaScript, "function f() {\n  g();\n}\n", 1, 3, "f", "g")]
    #[case::c(Lang::C, "void f() {\n    g();\n}\n", 1, 3, "f", "g")]
    #[case::cpp(Lang::Cpp, "void f() {\n    g();\n}\n", 1, 3, "f", "g")]
    fn function_defining_one_symbol_and_calling_another(
        #[case] lang: Lang,
        #[case] source: &str,
        #[case] start: usize,
        #[case] end: usize,
        #[case] def: &str,
        #[case] call: &str,
    ) {
        let hunk = Hunk {
            start_line: start,
            end_line: end,
        };
        let res = defs_uses(lang, source, &[hunk]);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].range, 0);
        assert_eq!(res[0].defines, vec![def.to_string()]);
        assert_eq!(res[0].uses, vec![call.to_string()]);
    }

    #[rstest]
    #[case::go(Lang::Go, "package p\n\ntype S struct {}\n", 3, 3, "S")]
    #[case::rust(Lang::Rust, "struct S;\n", 1, 1, "S")]
    #[case::python(Lang::Python, "class S:\n    pass\n", 1, 2, "S")]
    #[case::typescript(Lang::TypeScript, "interface S {}\n", 1, 1, "S")]
    #[case::tsx(Lang::Tsx, "interface S {}\n", 1, 1, "S")]
    #[case::javascript(Lang::JavaScript, "class S {}\n", 1, 1, "S")]
    #[case::c(Lang::C, "struct S {};\n", 1, 1, "S")]
    #[case::cpp(Lang::Cpp, "struct S {};\n", 1, 1, "S")]
    fn struct_or_type(
        #[case] lang: Lang,
        #[case] source: &str,
        #[case] start: usize,
        #[case] end: usize,
        #[case] symbol: &str,
    ) {
        let hunk = Hunk {
            start_line: start,
            end_line: end,
        };
        let res = defs_uses(lang, source, &[hunk]);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].range, 0);
        assert_eq!(res[0].defines, vec![symbol.to_string()]);
        assert!(res[0].uses.is_empty());
    }

    #[rstest]
    #[case::go(Lang::Go, "package p\n\nfunc f() {\n\tg()\n}\n", 4, 4, "g")]
    #[case::rust(Lang::Rust, "fn f() {\n    g();\n}\n", 2, 2, "g")]
    #[case::python(Lang::Python, "def f():\n    g()\n", 2, 2, "g")]
    #[case::typescript(Lang::TypeScript, "function f() {\n  g();\n}\n", 2, 2, "g")]
    #[case::tsx(Lang::Tsx, "function f() {\n  g();\n}\n", 2, 2, "g")]
    #[case::javascript(Lang::JavaScript, "function f() {\n  g();\n}\n", 2, 2, "g")]
    #[case::c(Lang::C, "void f() {\n    g();\n}\n", 2, 2, "g")]
    #[case::cpp(Lang::Cpp, "void f() {\n    g();\n}\n", 2, 2, "g")]
    fn range_inside_body_defines_empty_uses_callee(
        #[case] lang: Lang,
        #[case] source: &str,
        #[case] start: usize,
        #[case] end: usize,
        #[case] callee: &str,
    ) {
        let hunk = Hunk {
            start_line: start,
            end_line: end,
        };
        let res = defs_uses(lang, source, &[hunk]);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].range, 0);
        assert!(res[0].defines.is_empty());
        assert_eq!(res[0].uses, vec![callee.to_string()]);
    }

    #[rstest]
    #[case::go(Lang::Go, "package p\n\n// comment\nfunc f() {}\n", 2, 3)]
    #[case::rust(Lang::Rust, "\n// comment\nfn f() {}\n", 1, 2)]
    #[case::python(Lang::Python, "\n# comment\ndef f(): pass\n", 1, 2)]
    #[case::typescript(Lang::TypeScript, "\n// comment\nfunction f() {}\n", 1, 2)]
    #[case::tsx(Lang::Tsx, "\n// comment\nfunction f() {}\n", 1, 2)]
    #[case::javascript(Lang::JavaScript, "\n// comment\nfunction f() {}\n", 1, 2)]
    #[case::c(Lang::C, "\n// comment\nvoid f() {}\n", 1, 2)]
    #[case::cpp(Lang::Cpp, "\n// comment\nvoid f() {}\n", 1, 2)]
    fn range_with_no_code(
        #[case] lang: Lang,
        #[case] source: &str,
        #[case] start: usize,
        #[case] end: usize,
    ) {
        let hunk = Hunk {
            start_line: start,
            end_line: end,
        };
        let res = defs_uses(lang, source, &[hunk]);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].range, 0);
        assert!(res[0].defines.is_empty());
        assert!(res[0].uses.is_empty());
    }

    #[rstest]
    #[case::go(
        Lang::Go,
        "package p\n\nfunc f() {\n\tif 1 == 1 {\n\t\treturn\n\t}\n}\n",
        4,
        6
    )]
    #[case::rust(
        Lang::Rust,
        "fn f() {\n    if true {\n        return;\n    }\n}\n",
        2,
        4
    )]
    #[case::python(Lang::Python, "def f():\n    if True:\n        return None\n", 2, 3)]
    #[case::typescript(
        Lang::TypeScript,
        "function f() {\n  if (true) {\n    return;\n  }\n}\n",
        2,
        4
    )]
    #[case::tsx(
        Lang::Tsx,
        "function f() {\n  if (true) {\n    return;\n  }\n}\n",
        2,
        4
    )]
    #[case::javascript(
        Lang::JavaScript,
        "function f() {\n  if (true) {\n    return;\n  }\n}\n",
        2,
        4
    )]
    #[case::c(Lang::C, "void f() {\n    if (1) {\n        return;\n    }\n}\n", 2, 4)]
    #[case::cpp(
        Lang::Cpp,
        "void f() {\n    if (true) {\n        return;\n    }\n}\n",
        2,
        4
    )]
    fn keywords_and_literals_not_reported_as_uses(
        #[case] lang: Lang,
        #[case] source: &str,
        #[case] start: usize,
        #[case] end: usize,
    ) {
        let hunk = Hunk {
            start_line: start,
            end_line: end,
        };
        let res = defs_uses(lang, source, &[hunk]);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].range, 0);
        assert!(res[0].uses.is_empty());
    }

    #[test]
    fn multiple_ranges_preserve_indices_and_order() {
        let source = "fn f() {\n    g();\n}\nfn h() {\n    i();\n}\n";
        let ranges = [
            Hunk {
                start_line: 4,
                end_line: 6,
            },
            Hunk {
                start_line: 1,
                end_line: 3,
            },
        ];
        let res = defs_uses(Lang::Rust, source, &ranges);
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].range, 0);
        assert_eq!(res[0].defines, vec!["h".to_string()]);
        assert_eq!(res[0].uses, vec!["i".to_string()]);
        assert_eq!(res[1].range, 1);
        assert_eq!(res[1].defines, vec!["f".to_string()]);
        assert_eq!(res[1].uses, vec!["g".to_string()]);
    }

    #[test]
    fn empty_ranges_yields_empty() {
        assert!(defs_uses(Lang::Rust, "fn f() {}", &[]).is_empty());
    }

    #[test]
    fn deduplicates_and_sorts_symbols() {
        let source = "fn f() {\n    b();\n    a();\n    b();\n}\n";
        let res = defs_uses(
            Lang::Rust,
            source,
            &[Hunk {
                start_line: 1,
                end_line: 5,
            }],
        );
        assert_eq!(res[0].defines, vec!["f".to_string()]);
        assert_eq!(res[0].uses, vec!["a".to_string(), "b".to_string()]);
    }
}
