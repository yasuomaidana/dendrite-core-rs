use super::*;
use rstest::rstest;

fn first(lang: Lang, source: &str) -> FunctionMetrics {
    complexity(lang, source).unwrap().remove(0)
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
    )
    .unwrap();
    let got: Vec<_> = found
        .iter()
        .map(|m| (m.symbol.as_str(), m.start_line, m.end_line, m.cyclomatic))
        .collect();
    assert_eq!(got, [("outer", 1, 5, 1), ("inner", 2, 4, 2)]);
}

#[test]
fn source_without_functions_has_no_metrics() {
    assert!(complexity(Lang::Go, "package p\nvar x = 1\n")
        .unwrap()
        .is_empty());
}

#[test]
fn limits_are_the_documented_ones() {
    assert_eq!((MAX_CYCLOMATIC, MAX_COGNITIVE), (10, 15));
}
