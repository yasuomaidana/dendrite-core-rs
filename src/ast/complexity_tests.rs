use super::*;
use rstest::rstest;

fn first(lang: Lang, source: &str) -> FunctionMetrics {
    complexity(lang, source).remove(0)
}

#[rstest]
#[case::empty("func f() {}", 1, 0)]
#[case::if_only("func f(a int) int { if a > 0 { return 1 }; return 0 }", 2, 1)]
#[case::if_else("func f(a bool) { if a { } else { } }", 2, 2)]
#[case::else_if_chain("func f(a, b bool) { if a { } else if b { } else { } }", 3, 3)]
#[case::nested("func f(n int) { for i := 0; i < n; i++ { if i > 1 { } } }", 3, 3)]
#[case::expression_switch("func f(x int) { switch x { case 1: case 2: default: } }", 3, 1)]
#[case::type_switch("func f(v any) { switch v.(type) { case int: case string: } }", 3, 1)]
#[case::select("func f(c chan int) { select { case <-c: default: } }", 2, 1)]
#[case::mixed_operators("func f(a, b, c bool) { if a && b || c { } }", 4, 3)]
#[case::one_run("func f(a, b, c bool) { if a && b && c { } }", 4, 2)]
#[case::closure("func f(x bool) { g := func() { if x { } }; g() }", 2, 2)]
#[case::recursion("func f(n int) int { if n == 0 { return 0 }; return f(n - 1) }", 2, 2)]
#[case::labeled_break("func f() { L: for { for { break L } } }", 3, 4)]
#[case::plain_break("func f() { for { break } }", 2, 1)]
#[case::goto("func f() { goto L; L: return }", 1, 1)]
fn go(#[case] body: &str, #[case] cyclomatic: u32, #[case] cognitive: u32) {
    let m = first(Lang::Go, &format!("package p\n{body}\n"));
    assert_eq!((m.cyclomatic, m.cognitive), (cyclomatic, cognitive));
}

#[rstest]
#[case::match_arms(
    "fn f(x: Option<i32>) -> i32 { match x { Some(v) => v, None => 0 } }",
    3,
    1
)]
#[case::default_arm("fn f(n: i32) -> i32 { match n { 1 => 1, _ => 0 } }", 2, 1)]
#[case::try_operator("fn f() -> Result<(), E> { g()?; h()?; Ok(()) }", 3, 0)]
#[case::else_if_chain("fn f(a: bool, b: bool) { if a { } else if b { } else { } }", 3, 3)]
#[case::nested_loops("fn f() { for _ in 0..3 { while true { loop { break; } } } }", 4, 6)]
#[case::closure("fn f() { let c = |x: bool| if x { 1 } else { 2 }; c(true); }", 2, 3)]
#[case::boolean("fn f(a: bool, b: bool) -> bool { a && b }", 2, 1)]
#[case::labeled_break("fn f() { 'a: loop { loop { break 'a; } } }", 3, 4)]
#[case::recursion("fn f(n: u32) -> u32 { if n == 0 { 0 } else { f(n - 1) } }", 2, 3)]
#[case::method_recursion("impl A { fn f(&self, n: u32) { if n > 0 { self.f(n - 1) } } }", 2, 2)]
fn rust(#[case] source: &str, #[case] cyclomatic: u32, #[case] cognitive: u32) {
    let m = first(Lang::Rust, source);
    assert_eq!((m.cyclomatic, m.cognitive), (cyclomatic, cognitive));
}

#[rstest]
#[case::elif("def f(x):\n    if x:\n        return 1\n    elif y:\n        return 2\n    else:\n        return 3\n", 3, 3)]
#[case::loops(
    "def f(xs):\n    for x in xs:\n        while x:\n            x -= 1\n",
    3,
    3
)]
#[case::boolean("def f(a, b, c):\n    if a and b or c:\n        pass\n", 4, 3)]
#[case::comprehension("def f(xs):\n    return [x for x in xs if x]\n", 3, 0)]
#[case::try_except("def f():\n    try:\n        g()\n    except E:\n        pass\n", 2, 1)]
#[case::ternary("def f(a):\n    return 1 if a else 2\n", 2, 1)]
#[case::match_default("def f(x):\n    match x:\n        case 1:\n            pass\n        case _:\n            pass\n", 2, 1)]
#[case::match_cases("def f(x):\n    match x:\n        case 1:\n            pass\n        case 2:\n            pass\n", 3, 1)]
#[case::recursion("def f(n):\n    return f(n - 1)\n", 1, 1)]
#[case::lambda("def f(a):\n    return lambda x: x if a else 0\n", 2, 2)]
fn python(#[case] source: &str, #[case] cyclomatic: u32, #[case] cognitive: u32) {
    let m = first(Lang::Python, source);
    assert_eq!((m.cyclomatic, m.cognitive), (cyclomatic, cognitive));
}

#[rstest]
#[case::if_else("function f(a) { if (a) { return 1 } else { return 2 } }", 2, 2)]
#[case::switch(
    "function f(x) { switch (x) { case 1: break; case 2: break; default: break } }",
    3,
    1
)]
#[case::ternary_and("function f(a, b) { return a && b ? 1 : 2 }", 3, 2)]
#[case::catch("function f() { try { g() } catch (e) { h() } }", 2, 1)]
#[case::arrow("const f = (a) => { if (a) { return 1 } }", 2, 1)]
#[case::method("class A { m(x) { for (const y of x) { if (y) {} } } }", 3, 3)]
#[case::labeled("function f() { a: for (;;) { for (;;) { break a } } }", 3, 4)]
#[case::do_while("function f() { do { g() } while (x) }", 2, 1)]
fn typescript(#[case] source: &str, #[case] cyclomatic: u32, #[case] cognitive: u32) {
    let m = first(Lang::TypeScript, source);
    assert_eq!((m.cyclomatic, m.cognitive), (cyclomatic, cognitive));
}

#[rstest]
#[case::javascript(Lang::JavaScript, "function f() { while (x) {} }", "f", 2, 1)]
#[case::tsx(Lang::Tsx, "const C = () => <div/>;", "C", 1, 0)]
fn other_ecmascript(
    #[case] lang: Lang,
    #[case] source: &str,
    #[case] symbol: &str,
    #[case] cyclomatic: u32,
    #[case] cognitive: u32,
) {
    let m = first(lang, source);
    assert_eq!(
        (m.symbol.as_str(), m.cyclomatic, m.cognitive),
        (symbol, cyclomatic, cognitive)
    );
}

#[rstest]
#[case::else_if(
    "int f(int a) { if (a) { return 1; } else if (a > 1) { return 2; } return 0; }",
    3,
    2
)]
#[case::switch(
    "int f(int x) { switch (x) { case 1: return 1; case 2: return 2; default: return 0; } }",
    3,
    1
)]
#[case::ternary("int f(int a) { return a ? 1 : 2; }", 2, 1)]
#[case::goto("void f(void) { goto end; end: return; }", 1, 1)]
#[case::loops(
    "void f(int n) { for (;;) { while (n) { do { n--; } while (n); } } }",
    4,
    6
)]
fn c(#[case] source: &str, #[case] cyclomatic: u32, #[case] cognitive: u32) {
    let m = first(Lang::C, source);
    assert_eq!((m.cyclomatic, m.cognitive), (cyclomatic, cognitive));
}

#[rstest]
#[case::catch(
    "int A::m(int x) { try { return 1; } catch (...) { return 2; } }",
    2,
    1
)]
#[case::lambda("void f() { auto g = [](int x) { if (x) {} }; }", 2, 2)]
#[case::range_for("void f() { for (auto x : v) {} }", 2, 1)]
fn cpp(#[case] source: &str, #[case] cyclomatic: u32, #[case] cognitive: u32) {
    let m = first(Lang::Cpp, source);
    assert_eq!((m.cyclomatic, m.cognitive), (cyclomatic, cognitive));
}

#[rstest]
#[case::c_pointer(Lang::C, "char *f(void) { return 0; }", "f")]
#[case::cpp_qualified(Lang::Cpp, "int A::m() { return 1; }", "A::m")]
#[case::go_method(Lang::Go, "package p\nfunc (r R) m() {}", "m")]
#[case::rust_fn(Lang::Rust, "fn named() {}", "named")]
fn names(#[case] lang: Lang, #[case] source: &str, #[case] want: &str) {
    assert_eq!(first(lang, source).symbol, want);
}

#[test]
fn nested_functions_count_separately() {
    let found = complexity(
        Lang::Rust,
        "fn outer() {\n    fn inner() {\n        if true {}\n    }\n}\n",
    );
    let got: Vec<_> = found
        .iter()
        .map(|m| (m.symbol.as_str(), m.start_line, m.end_line, m.cyclomatic))
        .collect();
    assert_eq!(got, [("outer", 1, 5, 1), ("inner", 2, 4, 2)]);
}

#[test]
fn source_without_functions_has_no_metrics() {
    assert!(complexity(Lang::Go, "package p\nvar x = 1\n").is_empty());
}

#[test]
fn limits_are_the_documented_ones() {
    assert_eq!((MAX_CYCLOMATIC, MAX_COGNITIVE), (10, 15));
}

#[test]
fn a_node_without_children_scores_as_a_straight_line() {
    let tree = parse(Lang::Go, "package p");
    let mut leaf = tree.root_node();
    while let Some(child) = leaf.child(0) {
        leaf = child;
    }
    let mut scorer = Scorer {
        kinds: table(Lang::Go),
        spec: Lang::Go.spec(),
        source: b"package p",
        name: "",
        cyclomatic: 1,
        cognitive: 0,
    };
    scorer.score(leaf);
    assert_eq!((scorer.cyclomatic, scorer.cognitive), (1, 0));
}

#[test]
fn java_constructors_are_functions_named_after_the_class() {
    let m = first(Lang::Java, "class A { A() { if (true) { } } }");
    assert_eq!((m.symbol.as_str(), m.cyclomatic, m.cognitive), ("A", 2, 1));
}

#[rstest]
#[case::java_empty(Lang::Java, "class A { void f() {} }", 1, 0)]
#[case::java_if_only(
    Lang::Java,
    "class A { int f(int a) { if (a > 0) { return 1; } return 0; } }",
    2,
    1
)]
#[case::java_if_else(
    Lang::Java,
    "class A { void f(boolean a) { if (a) { } else { } } }",
    2,
    2
)]
#[case::java_else_if_chain(
    Lang::Java,
    "class A { void f(boolean a, boolean b) { if (a) { } else if (b) { } else { } } }",
    3,
    3
)]
#[case::java_loops(Lang::Java, "class A { void f(java.util.List<String> l, int a) { for (int i = 0; i < 3; i++) { } for (String s : l) { } while (a > 0) { } do { } while (a > 0); } }", 5, 4)]
#[case::java_nested(
    Lang::Java,
    "class A { void f(int a) { for (int i = 0; i < a; i++) { if (i > 1) { } } } }",
    3,
    3
)]
#[case::java_old_switch(
    Lang::Java,
    "class A { void f(int a) { switch (a) { case 1: break; case 2: default: break; } } }",
    3,
    1
)]
#[case::java_arrow_switch(
    Lang::Java,
    "class A { int f(int a) { return switch (a) { case 1 -> 1; default -> 0; }; } }",
    2,
    1
)]
#[case::java_boolean_runs(
    Lang::Java,
    "class A { boolean f(boolean a, boolean b, boolean c) { return a && b || c; } }",
    3,
    2
)]
#[case::java_one_run(
    Lang::Java,
    "class A { boolean f(boolean a, boolean b, boolean c) { return a && b && c; } }",
    3,
    1
)]
#[case::java_ternary(Lang::Java, "class A { int f(boolean a) { return a ? 1 : 2; } }", 2, 1)]
#[case::java_catch_clause(
    Lang::Java,
    "class A { void f() { try { } catch (Exception e) { } finally { } } }",
    2,
    1
)]
#[case::java_lambda(
    Lang::Java,
    "class A { void f(boolean a) { Runnable q = () -> { if (a) { } }; } }",
    2,
    2
)]
#[case::java_labeled_break(
    Lang::Java,
    "class A { void f() { outer: for (;;) { break outer; } } }",
    2,
    2
)]
#[case::java_plain_break(Lang::Java, "class A { void f() { for (;;) { break; } } }", 2, 1)]
#[case::java_recursion(Lang::Java, "class A { int f(int a) { return f(a); } }", 1, 1)]
#[case::java_method_recursion(Lang::Java, "class A { int f(A o) { return o.f(o); } }", 1, 1)]
#[case::kotlin_empty(Lang::Kotlin, "fun f() {}", 1, 0)]
#[case::kotlin_if_only(
    Lang::Kotlin,
    "fun f(a: Int): Int {\n    if (a > 0) { return 1 }\n    return 0\n}",
    2,
    1
)]
#[case::kotlin_if_else(Lang::Kotlin, "fun f(a: Boolean) {\n    if (a) { } else { }\n}", 2, 2)]
#[case::kotlin_else_if_chain(
    Lang::Kotlin,
    "fun f(a: Boolean, b: Boolean) {\n    if (a) { } else if (b) { } else { }\n}",
    3,
    3
)]
#[case::kotlin_loops(Lang::Kotlin, "fun f(a: Int) {\n    for (i in 0 until a) { }\n    while (a > 0) { }\n    do { } while (a > 0)\n}", 4, 3)]
#[case::kotlin_when_entries(
    Lang::Kotlin,
    "fun f(a: Int) {\n    when (a) { 1 -> {} 2, 3 -> {} else -> {} }\n}",
    3,
    1
)]
#[case::kotlin_elvis(Lang::Kotlin, "fun f(a: Int?): Int {\n    return a ?: 0\n}", 2, 0)]
#[case::kotlin_boolean_runs(
    Lang::Kotlin,
    "fun f(a: Boolean, b: Boolean, c: Boolean): Boolean {\n    return a && b || c\n}",
    3,
    2
)]
#[case::kotlin_catch_block(
    Lang::Kotlin,
    "fun f() {\n    try { } catch (e: Exception) { } finally { }\n}",
    2,
    1
)]
#[case::kotlin_lambda(
    Lang::Kotlin,
    "fun f() {\n    val l = { v: Int -> if (v > 1) 1 else 2 }\n}",
    2,
    3
)]
#[case::kotlin_anonymous_function(
    Lang::Kotlin,
    "fun f() {\n    val g = fun(v: Int): Int { if (v > 1) { return 1 }; return 2 }\n}",
    2,
    2
)]
#[case::kotlin_labeled_break(
    Lang::Kotlin,
    "fun f() {\n    loop@ for (i in 1..3) { break@loop }\n}",
    2,
    2
)]
#[case::kotlin_recursion(Lang::Kotlin, "fun f(a: Int): Int {\n    return f(a)\n}", 1, 1)]
#[case::kotlin_expression_body(Lang::Kotlin, "fun f(a: Int) = if (a > 0) 1 else 2", 2, 2)]
fn metrics_of_java_and_kotlin(
    #[case] lang: Lang,
    #[case] source: &str,
    #[case] cyclomatic: u32,
    #[case] cognitive: u32,
) {
    let m = first(lang, source);
    assert_eq!(
        (m.cyclomatic, m.cognitive),
        (cyclomatic, cognitive),
        "{source}"
    );
}

#[test]
fn kotlin_methods_are_functions_of_their_own() {
    let found = complexity(
        Lang::Kotlin,
        "class C {\n    fun m() { if (true) { } }\n    fun n() { }\n}\n",
    );
    let seen: Vec<_> = found
        .iter()
        .map(|m| (m.symbol.as_str(), m.cyclomatic))
        .collect();
    assert_eq!(seen, vec![("m", 2), ("n", 1)]);
}

const SVELTE: &str = "<script lang=\"ts\">\n  function f(a: number) { if (a > 0) { return 1 } return 0 }\n  const g = (x: number) => x ? 1 : 2;\n</script>\n\n{#if show}<p>hi</p>{:else if other}<p>no</p>{/if}\n\n<script context=\"module\">\n  export function h(a) {\n    for (const x of a) { if (x) { return x } }\n  }\n</script>\n";

#[test]
fn a_svelte_component_measures_the_functions_of_its_scripts_by_the_lines_of_the_file() {
    let found = complexity(Lang::Svelte, SVELTE);
    let seen: Vec<_> = found
        .iter()
        .map(|m| {
            (
                m.symbol.as_str(),
                m.start_line,
                m.end_line,
                m.cyclomatic,
                m.cognitive,
            )
        })
        .collect();
    assert_eq!(
        seen,
        vec![("f", 2, 2, 2, 1), ("g", 3, 3, 2, 1), ("h", 9, 11, 3, 3)]
    );
}

#[test]
fn a_svelte_component_without_a_script_has_no_functions() {
    assert!(complexity(Lang::Svelte, "<p>hi</p>\n{#if x}<b>yes</b>{/if}\n").is_empty());
}
