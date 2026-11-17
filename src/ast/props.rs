//! The props of a UI component, read from its syntax tree: Svelte 5 (`$props()`) and legacy (`export let`)
//! components, and React function components in TypeScript, TSX and JavaScript.
//!
//! A prop comes from two places that are merged: the destructured parameter (its default, its `...rest`) and the
//! declared type (`interface Props`, `type Props = {...}`, an inline object type: its type and its `?`). Callers used
//! to find these with regular expressions, which break on a nested brace, a renamed or defaulted property and a
//! callback type; a parse tree does not.

use std::collections::HashMap;

use serde::Serialize;
use tree_sitter::Node;

use crate::embedded::regions;
use crate::lang::Lang;
use crate::parse::{parse, text};

/// One prop of a component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Prop {
    /// The prop's name; for a renamed property (`{ a: alias }`) the name callers pass, `a`.
    pub name: String,
    /// The declared type, as written, when there is one.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
    /// True for `a?: T` and for a prop with a default.
    pub optional: bool,
    /// The default value, as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// True for `...rest`, which collects every other prop.
    pub rest: bool,
    /// The 1-based line of the prop in the file.
    pub line: usize,
}

type Declared<'t> = HashMap<String, Node<'t>>;

/// The props of the component in `source`, in the order they appear; none for a language without components.
pub fn component_props(lang: Lang, source: &str) -> Vec<Prop> {
    match lang {
        Lang::Svelte => regions(lang, source)
            .into_iter()
            .flat_map(|r| Script::new(r.lang, &source[r.start..r.end], r.row, true).props())
            .collect(),
        Lang::TypeScript | Lang::Tsx | Lang::JavaScript => {
            Script::new(lang, source, 0, false).props()
        }
        _ => Vec::new(),
    }
}

/// One script (a whole file, or a Svelte `<script>`), parsed.
struct Script<'a> {
    src: &'a [u8],
    tree: tree_sitter::Tree,
    /// The row the script starts on in its file.
    row: usize,
    svelte: bool,
}

/// Where a component's props are read: the destructuring pattern, if any, and the declared type, if any.
struct Site<'t> {
    pattern: Option<Node<'t>>,
    ty: Option<Node<'t>>,
}

impl<'a> Script<'a> {
    fn new(lang: Lang, code: &'a str, row: usize, svelte: bool) -> Self {
        Script {
            src: code.as_bytes(),
            tree: parse(lang, code),
            row,
            svelte,
        }
    }

    fn props(&self) -> Vec<Prop> {
        let root = self.tree.root_node();
        let nodes = descendants(root);
        let declared = self.declared(&nodes);
        let mut out: Vec<Prop> = Vec::new();
        let mut add = |p: Prop| {
            if !out.iter().any(|o| o.name == p.name) {
                out.push(p);
            }
        };
        for site in self.sites(&nodes) {
            let members = site.ty.map_or_else(Vec::new, |t| {
                self.type_members(t, &declared, &mut Vec::new())
            });
            let entries = site.pattern.map_or_else(Vec::new, |p| self.entries(p));
            for e in &entries {
                add(merge(e, members.iter().find(|m| m.name == e.name)));
            }
            for m in members {
                add(m);
            }
        }
        if self.svelte {
            for n in nodes {
                if let Some(p) = self.legacy(n) {
                    add(p);
                }
            }
        }
        out
    }

    fn text(&self, n: Node) -> String {
        text(n, self.src).to_string()
    }

    fn line(&self, n: Node) -> usize {
        self.row + n.start_position().row + 1
    }

    /// The body of every `interface` and `type X = {...}` of the script, by name.
    fn declared<'t>(&self, nodes: &[Node<'t>]) -> Declared<'t> {
        nodes
            .iter()
            .filter_map(|n| {
                let body = match n.kind() {
                    "interface_declaration" => n.child_by_field_name("body"),
                    "type_alias_declaration" => n.child_by_field_name("value"),
                    _ => None,
                };
                Some((self.text(n.child_by_field_name("name")?), body?))
            })
            .collect()
    }

    /// The members a type declares: an object type, an interface body, a declared type (generic or not), or an
    /// intersection of them. `seen` holds the names being resolved, so a cycle ends.
    fn type_members(&self, n: Node, declared: &Declared, seen: &mut Vec<String>) -> Vec<Prop> {
        match n.kind() {
            "type_identifier" => self.referenced(&self.text(n), declared, seen),
            "generic_type" => n
                .child_by_field_name("name")
                .map_or_else(Vec::new, |m| self.type_members(m, declared, seen)),
            "object_type" | "interface_body" => self.signatures(n),
            "intersection_type" | "parenthesized_type" | "type_annotation" => {
                let mut cursor = n.walk();
                n.named_children(&mut cursor)
                    .flat_map(|c| self.type_members(c, declared, seen))
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn referenced(&self, name: &str, declared: &Declared, seen: &mut Vec<String>) -> Vec<Prop> {
        let Some(body) = declared
            .get(name)
            .filter(|_| !seen.iter().any(|s| s == name))
        else {
            return Vec::new();
        };
        seen.push(name.to_string());
        let members = self.type_members(*body, declared, seen);
        seen.pop();
        members
    }

    fn signatures(&self, body: Node) -> Vec<Prop> {
        let mut cursor = body.walk();
        body.named_children(&mut cursor)
            .filter(|c| matches!(c.kind(), "property_signature" | "method_signature"))
            .filter_map(|c| self.signature(c))
            .collect()
    }

    fn signature(&self, n: Node) -> Option<Prop> {
        let name = n.child_by_field_name("name")?;
        let mut cursor = n.walk();
        let optional = n.children(&mut cursor).any(|c| c.kind() == "?");
        let ty = n.child_by_field_name("type").map(|t| self.annotation(t));
        Some(Prop {
            optional,
            ty,
            ..self.named(name, n)
        })
    }

    /// The type written in a `: T` annotation, without the colon.
    fn annotation(&self, n: Node) -> String {
        self.text(n).trim_start_matches(':').trim().to_string()
    }

    /// A prop named by `name_node`, at the line of `at`, with nothing else known.
    fn named(&self, name_node: Node, at: Node) -> Prop {
        Prop {
            name: self.text(name_node).trim_matches(['"', '\'']).to_string(),
            ty: None,
            optional: false,
            default: None,
            rest: false,
            line: self.line(at),
        }
    }

    /// The places a component's props are read from.
    fn sites<'t>(&self, nodes: &[Node<'t>]) -> Vec<Site<'t>> {
        nodes
            .iter()
            .filter_map(|n| {
                if self.svelte {
                    self.props_rune(*n)
                } else {
                    self.react_component(*n)
                }
            })
            .collect()
    }

    /// `let { a, b = 1, ...rest }: Props = $props();`
    fn props_rune<'t>(&self, n: Node<'t>) -> Option<Site<'t>> {
        if n.kind() != "variable_declarator" {
            return None;
        }
        let call = n
            .child_by_field_name("value")
            .filter(|v| v.kind() == "call_expression")?;
        let callee = call.child_by_field_name("function")?;
        (self.text(callee) == "$props").then(|| Site {
            pattern: n.child_by_field_name("name"),
            ty: n.child_by_field_name("type"),
        })
    }

    /// A function component: a function named with a capital whose first parameter is its props.
    fn react_component<'t>(&self, n: Node<'t>) -> Option<Site<'t>> {
        let name = match n.kind() {
            "function_declaration" => n.child_by_field_name("name"),
            "arrow_function" | "function_expression" => n
                .parent()
                .filter(|p| p.kind() == "variable_declarator")
                .and_then(|p| p.child_by_field_name("name")),
            _ => None,
        }?;
        if !self
            .text(name)
            .starts_with(|c: char| c.is_ascii_uppercase())
        {
            return None;
        }
        let first = n.child_by_field_name("parameters")?.named_child(0)?;
        let (pattern, ty) = match first.kind() {
            "required_parameter" | "optional_parameter" => (
                first.child_by_field_name("pattern"),
                first.child_by_field_name("type"),
            ),
            "assignment_pattern" => (first.child_by_field_name("left"), None),
            _ => (Some(first), None),
        };
        Some(Site {
            pattern,
            ty: ty.or_else(|| fc_props(n)),
        })
    }

    /// The entries of a destructuring pattern: shorthand, defaulted, renamed and rest properties.
    fn entries(&self, pattern: Node) -> Vec<Prop> {
        if pattern.kind() != "object_pattern" {
            return Vec::new();
        }
        let mut cursor = pattern.walk();
        pattern
            .named_children(&mut cursor)
            .filter_map(|c| self.entry(c))
            .collect()
    }

    fn entry(&self, n: Node) -> Option<Prop> {
        match n.kind() {
            "shorthand_property_identifier_pattern" => Some(self.named(n, n)),
            "object_assignment_pattern" => Some(Prop {
                default: n.child_by_field_name("right").map(|r| self.text(r)),
                ..self.named(n.child_by_field_name("left")?, n)
            }),
            "pair_pattern" => {
                let value = n.child_by_field_name("value")?;
                let default = (value.kind() == "assignment_pattern")
                    .then(|| value.child_by_field_name("right").map(|r| self.text(r)))
                    .flatten();
                let key = n
                    .child_by_field_name("key")
                    .filter(|k| k.kind() != "computed_property_name")?;
                Some(Prop {
                    default,
                    ..self.named(key, n)
                })
            }
            "rest_pattern" => Some(Prop {
                rest: true,
                ..self.named(n.named_child(0)?, n)
            }),
            _ => None,
        }
    }

    /// Svelte 4 syntax: `export let a = 1;` or `export let a: string;`.
    fn legacy(&self, n: Node) -> Option<Prop> {
        if n.kind() != "export_statement" {
            return None;
        }
        let decl = n.child_by_field_name("declaration")?;
        let mut cursor = decl.walk();
        let var = decl
            .named_children(&mut cursor)
            .find(|c| c.kind() == "variable_declarator")?;
        let name = var
            .child_by_field_name("name")
            .filter(|n| n.kind() == "identifier")?;
        let default = var.child_by_field_name("value").map(|v| self.text(v));
        let ty = var.child_by_field_name("type").map(|t| self.annotation(t));
        Some(Prop {
            optional: default.is_some(),
            default,
            ty,
            ..self.named(name, var)
        })
    }
}

/// `const Button: FC<Props> = ({ a }) => ...`: the `Props` of the annotation, when the parameter has no type.
fn fc_props(function: Node) -> Option<Node> {
    let declarator = function
        .parent()
        .filter(|p| p.kind() == "variable_declarator")?;
    let annotated = declarator.child_by_field_name("type")?.named_child(0)?;
    annotated
        .child_by_field_name("type_arguments")?
        .named_child(0)
}

/// A destructured entry completed with what its declared member says.
fn merge(entry: &Prop, member: Option<&Prop>) -> Prop {
    Prop {
        ty: member.and_then(|m| m.ty.clone()),
        optional: entry.default.is_some() || member.is_some_and(|m| m.optional),
        ..entry.clone()
    }
}

/// Every node under `root`, in source order.
fn descendants(root: Node) -> Vec<Node> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        out.push(n);
        let mut cursor = n.walk();
        let children: Vec<_> = n.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn names(props: &[Prop]) -> Vec<&str> {
        props.iter().map(|p| p.name.as_str()).collect()
    }

    fn find<'p>(props: &'p [Prop], name: &str) -> &'p Prop {
        props.iter().find(|p| p.name == name).expect(name)
    }

    #[rstest]
    #[case::shorthand("let { a, b } = $props();", &["a", "b"])]
    #[case::nested_brace_default("let { a = { x: { y: 1 } }, b } = $props();", &["a", "b"])]
    #[case::rename("let { a: alias, b } = $props();", &["a", "b"])]
    #[case::rename_with_default("let { a: alias = 3, b } = $props();", &["a", "b"])]
    #[case::rest("let { a, ...rest } = $props();", &["a", "rest"])]
    #[case::typed_destructuring("let { a }: { a: string; b: number } = $props();", &["a", "b"])]
    #[case::local_interface("interface Props { a: string; onclick: (e: { x: number }) => void }\nlet { a }: Props = $props();", &["a", "onclick"])]
    #[case::type_alias("type Props = { a: string }; let { a } : Props = $props();", &["a"])]
    #[case::intersection("type Base = { a: string };\ntype Props = Base & { b: number };\nlet { a }: Props = $props();", &["a", "b"])]
    #[case::array_default("let { items = [1, [2]], b } = $props();", &["items", "b"])]
    #[case::generic_props("interface P<T> { a: T }\nlet { a }: P<string> = $props();", &["a"])]
    #[case::primitive_type("let { a }: string = $props();", &["a"])]
    #[case::comment_in_pattern("let { a, /* note */ b } = $props();", &["a", "b"])]
    #[case::cyclic_alias("type A = B;\ntype B = A;\nlet { x }: A = $props();", &["x"])]
    #[case::whole_props_object("interface Props { a: string; b?: number }\nlet props: Props = $props();", &["a", "b"])]
    #[case::legacy("export let a = 1;\nexport let b: string;\nlet notAProp = 2;", &["a", "b"])]
    #[case::none("let x = 1;", &[])]
    fn svelte_script_props(#[case] script: &str, #[case] want: &[&str]) {
        let source = format!("<script lang=\"ts\">\n{script}\n</script>\n<p>hi</p>\n");
        assert_eq!(names(&component_props(Lang::Svelte, &source)), want);
    }

    #[test]
    fn a_default_makes_a_prop_optional_and_keeps_its_text() {
        let props = component_props(
            Lang::Svelte,
            "<script lang=\"ts\">\ninterface Props { a: string; b: number; c?: boolean }\nlet { a, b = { n: 1 }, c }: Props = $props();\n</script>\n",
        );
        let (a, b, c) = (find(&props, "a"), find(&props, "b"), find(&props, "c"));
        assert_eq!(
            (a.ty.as_deref(), a.optional, a.default.as_deref()),
            (Some("string"), false, None)
        );
        assert_eq!(
            (b.ty.as_deref(), b.optional, b.default.as_deref()),
            (Some("number"), true, Some("{ n: 1 }"))
        );
        assert!(c.optional && c.default.is_none());
    }

    #[test]
    fn rest_is_flagged_and_lines_are_lines_of_the_file() {
        let props = component_props(
            Lang::Svelte,
            "<h1>title</h1>\n<script>\n  let {\n    a,\n    ...rest\n  } = $props();\n</script>\n",
        );
        assert_eq!((find(&props, "a").line, find(&props, "a").rest), (4, false));
        assert_eq!(
            (find(&props, "rest").line, find(&props, "rest").rest),
            (5, true)
        );
    }

    #[test]
    fn an_intersection_keeps_the_types_of_its_declared_parts() {
        let props = component_props(
            Lang::Svelte,
            "<script lang=\"ts\">\ntype Base = { a: string };\ntype Props = Base & { b: number };\nlet { a }: Props = $props();\n</script>\n",
        );
        assert_eq!(find(&props, "a").ty.as_deref(), Some("string"));
        assert_eq!(find(&props, "b").ty.as_deref(), Some("number"));
    }

    #[test]
    fn an_fc_annotation_types_an_untyped_parameter() {
        let src = "interface P { a: string }\nconst Button: React.FC<P> = ({ a }) => null;";
        let props = component_props(Lang::Tsx, src);
        assert_eq!(find(&props, "a").ty.as_deref(), Some("string"));
        let plain = component_props(Lang::Tsx, "const Button: Foo = ({ a }) => null;");
        assert_eq!(names(&plain), ["a"]);
    }

    #[test]
    fn a_jsx_component_with_a_default_parameter_object() {
        let props = component_props(
            Lang::JavaScript,
            "function Card({ a, b } = {}) { return <p/>; }",
        );
        assert_eq!(names(&props), ["a", "b"]);
        let plain = component_props(Lang::JavaScript, "function Card(props) { return null; }");
        assert!(plain.is_empty());
    }

    #[test]
    fn an_untyped_script_has_no_types() {
        let props = component_props(
            Lang::Svelte,
            "<script>\nlet { a = 1 } = $props();\n</script>\n",
        );
        assert_eq!(props.len(), 1);
        assert!(props[0].ty.is_none());
    }

    #[rstest]
    #[case::declaration("interface P { a: string; b?: () => void }\nexport function Button({ a, b }: P) { return null; }", &["a", "b"])]
    #[case::arrow("type P = { a: string };\nexport const Button = ({ a, ...rest }: P) => null;", &["a", "rest"])]
    #[case::function_expression("interface P { a: string }\nconst Button = function ({ a }: P) { return null; };", &["a"])]
    #[case::inline_type("export function Button({ a }: { a: string; cb: (x: { y: number }) => void }) { return null; }", &["a", "cb"])]
    #[case::props_object("interface P { a: string; b: number }\nfunction Button(props: P) { return null; }", &["a", "b"])]
    #[case::default_and_rename("function Button({ a = {}, b: alias }) { return null; }", &["a", "b"])]
    #[case::not_a_component("interface P { a: string }\nfunction helper({ a }: P) { return a; }", &[])]
    #[case::no_parameter("export function Button() { return null; }", &[])]
    fn react_component_props(#[case] source: &str, #[case] want: &[&str]) {
        for lang in [Lang::Tsx, Lang::TypeScript] {
            assert_eq!(names(&component_props(lang, source)), want, "{lang:?}");
        }
    }

    #[test]
    fn javascript_components_have_props_without_types() {
        let props = component_props(
            Lang::JavaScript,
            "export function Card({ title, onClick = () => {} }) {}",
        );
        assert_eq!(names(&props), ["title", "onClick"]);
        assert!(props.iter().all(|p| p.ty.is_none()));
    }

    #[test]
    fn members_with_quoted_names_and_methods_are_props() {
        let props = component_props(
            Lang::Tsx,
            "interface P { 'data-x': string; render(): void }\nfunction Box(p: P) { return null; }",
        );
        assert_eq!(names(&props), ["data-x", "render"]);
    }

    #[test]
    fn a_prop_is_listed_once_across_sites_and_members() {
        let props = component_props(
            Lang::Tsx,
            "interface P { a: string }\nfunction One({ a }: P) {}\nfunction Two({ a }: P) {}\n",
        );
        assert_eq!(names(&props), ["a"]);
    }

    #[test]
    fn other_languages_have_no_components() {
        assert!(component_props(Lang::Go, "package p\nfunc F() {}\n").is_empty());
    }

    #[test]
    fn a_property_without_a_name_is_skipped_and_odd_patterns_are_ignored() {
        let props = component_props(
            Lang::Tsx,
            "function Box({ [k]: v, a }: { a: string }) { return null; }",
        );
        assert_eq!(names(&props), ["a"]);
    }

    #[test]
    fn serializes_the_wire_shape() {
        let props = component_props(
            Lang::Tsx,
            "function Box({ a = 1 }: { a?: number }) { return null; }",
        );
        let json = serde_json::to_value(&props).unwrap();
        assert_eq!(
            json,
            serde_json::json!([{"name": "a", "type": "number", "optional": true, "default": "1", "rest": false, "line": 1}])
        );
    }
}
