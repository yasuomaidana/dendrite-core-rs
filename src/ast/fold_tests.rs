use super::*;
use rstest::rstest;

fn fold(lang: Lang, source: &str, keep: &[&str]) -> Folded {
    let keep: Vec<String> = keep.iter().map(|k| k.to_string()).collect();
    fold_signatures(lang, source, &keep)
}

#[rstest]
#[case::go(
    Lang::Go,
    "package p\n\n// Add adds.\nfunc Add(a, b int) int {\n\treturn a + b\n}\n\nfunc Keep() int {\n\treturn 1\n}\n",
    &["Keep"],
    "package p\n\n// Add adds.\nfunc Add(a, b int) int { /* pruned */ }\n\nfunc Keep() int {\n\treturn 1\n}\n"
)]
#[case::rust(
    Lang::Rust,
    "struct S { x: i32 }\nimpl S {\n    fn get(&self) -> i32 { self.x }\n}\ntrait T { fn go(&self); }\n",
    &[],
    "struct S { x: i32 }\nimpl S {\n    fn get(&self) -> i32 { /* pruned */ }\n}\ntrait T { fn go(&self); }\n"
)]
#[case::python(
    Lang::Python,
    "def f(x):\n    \"\"\"Doc.\"\"\"\n    return x\n\nclass A:\n    def m(self):\n        return 1\n\ndef one(): return 2\n",
    &[],
    "def f(x):\n    \"\"\"Doc.\"\"\"\n    ...\n\nclass A:\n    def m(self):\n        ...\n\ndef one(): ...\n"
)]
#[case::typescript(
    Lang::TypeScript,
    "const f = (a: number) => a + 1;\nclass A { m(): number { return 1; } }\ninterface I { x: number }\n",
    &[],
    "const f = (a: number) => { /* pruned */ };\nclass A { m(): number { /* pruned */ } }\ninterface I { x: number }\n"
)]
#[case::c(
    Lang::C,
    "struct s { int x; };\nint f(int a) { return a; }\n",
    &[],
    "struct s { int x; };\nint f(int a) { /* pruned */ }\n"
)]
fn folds_all_but_the_kept_functions(
    #[case] lang: Lang,
    #[case] source: &str,
    #[case] keep: &[&str],
    #[case] want: &str,
) {
    let out = fold(lang, source, keep);
    assert_eq!(out.source, want);
    assert!(
        !parse(lang, &out.source).root_node().has_error(),
        "output must still parse"
    );
}

#[test]
fn keeps_the_bodies_of_kept_symbols() {
    let source = "package p\nfunc A() { x() }\nfunc B() { y() }\n";
    let out = fold(Lang::Go, source, &["B"]);
    assert_eq!(
        out.source,
        "package p\nfunc A() { /* pruned */ }\nfunc B() { y() }\n"
    );
    assert_eq!(out.folded, ["A"]);
}

#[test]
fn does_not_look_inside_a_folded_or_kept_body() {
    let source = "fn outer() { fn inner() { 1 } }\nfn kept() { fn inner2() { 2 } }\n";
    let out = fold(Lang::Rust, source, &["kept"]);
    assert_eq!(out.folded, ["outer"]);
    assert!(out.source.contains("fn inner2() { 2 }"));
}

#[test]
fn reports_the_token_saving() {
    let source = format!("package p\nfunc A() {{ {} }}\n", "work()\n".repeat(50));
    let out = fold(Lang::Go, &source, &[]);
    assert!(out.tokens_after < out.tokens_before);
    assert_eq!(out.tokens_before, estimate(&source));
}

#[test]
fn leaves_declarations_without_a_body_alone() {
    let out = fold(Lang::C, "int f(int a);\n", &[]);
    assert!(out.folded.is_empty());
    assert_eq!(out.source, "int f(int a);\n");
}

#[test]
fn indent_ignores_a_line_with_code_before_the_body() {
    assert_eq!(indent_before("def f(): x", 9), "");
    assert_eq!(indent_before("a\n    x", 6), "    ");
}

#[test]
fn a_body_without_a_docstring_has_none() {
    let tree = parse(Lang::Python, "def f():\n    pass\n");
    let body = tree
        .root_node()
        .named_child(0)
        .unwrap()
        .child_by_field_name("body")
        .unwrap();
    assert_eq!(docstring(body, "def f():\n    pass\n"), None);
    let empty = parse(Lang::Python, "x = 1\n");
    assert_eq!(
        docstring(
            empty
                .root_node()
                .named_child(0)
                .unwrap()
                .named_child(0)
                .unwrap(),
            "x = 1\n"
        ),
        None
    );
}

#[test]
fn a_go_declaration_without_a_body_is_left_alone() {
    let out = fold(Lang::Go, "package p\nfunc external(a int) int\n", &[]);
    assert!(out.folded.is_empty());
}
