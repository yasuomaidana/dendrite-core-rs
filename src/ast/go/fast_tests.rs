use super::*;
use crate::complexity::complexity_tree;
use crate::lang::Lang;
use crate::parse::parse;
use rstest::rstest;

fn fast(src: &str) -> Vec<FunctionMetrics> {
    complexity(src).expect("the scan handles this source")
}

fn tree(src: &str) -> Vec<FunctionMetrics> {
    complexity_tree(Lang::Go, src)
}

const IF_CHAIN: &str = "package p\nfunc f(a, b int) int {\n\tif a > b {\n\t\treturn 1\n\t} else if a < b {\n\t\tif a == 0 {\n\t\t\treturn 2\n\t\t}\n\t} else if b == 0 {\n\t\treturn 3\n\t} else {\n\t\tfor i := 0; i < a; i++ {\n\t\t\tif i > 2 {\n\t\t\t\tbreak\n\t\t\t}\n\t\t}\n\t}\n\treturn 0\n}\n";
const SWITCHES: &str = "package p\nfunc f(x interface{}, ch chan int) {\n\tswitch v := x.(type) {\n\tcase int:\n\t\t_ = v\n\tcase string, []byte:\n\tdefault:\n\t}\n\tswitch {\n\tcase ch == nil:\n\tcase len(ch) > 0 && cap(ch) > 1:\n\t}\n\tselect {\n\tcase <-ch:\n\tcase ch <- 1:\n\tdefault:\n\t}\n}\n";
const BOOL_RUNS: &str = "package p\nfunc f(a, b, c, d bool) bool {\n\tx := a && b && c\n\ty := a || b || c\n\tz := a && b || c && d\n\tw := a || b && c || d\n\tif (a || b) && (c || d) && f(a && b, c || d) {\n\t\treturn x && y\n\t}\n\treturn z || w\n}\n";
const CLOSURES: &str = "package p\nfunc f(xs []int) {\n\tgo func() {\n\t\tfor _, x := range xs {\n\t\t\tif x > 0 {\n\t\t\t\tdefer func() { recover() }()\n\t\t\t}\n\t\t}\n\t}()\n\tfn := func(a int) (int, error) {\n\t\tif a > 1 {\n\t\t\treturn a, nil\n\t\t}\n\t\treturn 0, nil\n\t}\n\t_, _ = fn(1)\n}\n";
const LABELS: &str = "package p\nfunc f() {\nouter:\n\tfor i := 0; i < 3; i++ {\n\t\tfor j := 0; j < 3; j++ {\n\t\t\tif j == 1 {\n\t\t\t\tcontinue outer\n\t\t\t}\n\t\t\tif j == 2 {\n\t\t\t\tbreak outer\n\t\t\t}\n\t\t\tcontinue\n\t\t}\n\t}\n\tgoto done\ndone:\n}\n";
const RECURSION: &str = "package p\ntype T struct{}\nfunc fact(n int) int {\n\tif n < 2 {\n\t\treturn 1\n\t}\n\treturn n * fact(n-1)\n}\nfunc (t *T) walk(n int) {\n\tif n > 0 {\n\t\tt.walk(n - 1)\n\t\tother.walk(n)\n\t}\n}\nfunc gen[T any](x T) T {\n\treturn x\n}\n";
const HEADER_LITERALS: &str = "package p\nfunc f(m map[string]int) {\n\tfor _, x := range []int{1, 2, 3} {\n\t\t_ = x\n\t}\n\tfor k := range map[string][]int{\"a\": {1}} {\n\t\t_ = k\n\t}\n\tif v, ok := m[\"a\"]; ok && v > 0 {\n\t}\n\tfor _, p := range []*pair{{1}, {2}} {\n\t\t_ = p\n\t}\n}\ntype pair struct{ a int }\n";
const TYPES_IN_SIGNATURES: &str = "package p\ntype H func(int) (bool, error)\nfunc f(cb func(int) bool, tbl map[string]func() error) func(int) func() int {\n\tif cb(1) {\n\t\treturn nil\n\t}\n\thandlers := []func(){func() {}, func() { if tbl == nil { return } }}\n\t_ = handlers\n\treturn nil\n}\nfunc ext(x int) int\nfunc g() {}\n";
const ELSE_COMMENT_IF: &str = "package p\nfunc f(a, b bool) int {\n\tif a {\n\t\treturn 1\n\t} else\n\t// a note between else and if\n\tif b {\n\t\tfor {\n\t\t}\n\t} else /* here too */ if a {\n\t\treturn 2\n\t}\n\treturn 0\n}\n";
const HEADER_SHAPES: &str = "package p\nfunc f() {\n\tfor _, p := range []**int{nil} {\n\t\t_ = p\n\t}\n\tfor _, g := range []func() any{func() any { return nil }} {\n\t\t_ = g\n\t}\n\tfor _, c := range []chan int{nil} {\n\t\t_ = c\n\t}\n}\n";
const TABLE_TESTS: &str = "package p\nfunc f() {\n\tfor _, c := range []struct {\n\t\tname string\n\t\tok   bool\n\t}{\n\t\t{\"a\", true},\n\t\t{\"b\", false},\n\t} {\n\t\tif c.ok && c.name != \"\" {\n\t\t}\n\t}\n\tfor _, x := range []interface{}{1, \"a\"} {\n\t\t_ = x\n\t}\n\tfor k, v := range map[string]struct{}{\"a\": {}} {\n\t\t_, _ = k, v\n\t}\n\tif _, ok := any(1).(interface{ M() }); ok {\n\t}\n}\n";
const NOISE: &str = "package p\n// if for switch && || case\n/* if for\n && || */\nfunc f() string {\n\ts := \"if for && || case { }\"\n\tr := `raw if\n for { `\n\tc := '{'\n\t_ = c\n\treturn s + r\n}\n";
const MULTILINE: &str = "package p\nfunc f(a, b, c bool) bool {\n\tif a &&\n\t\tb ||\n\t\tc {\n\t\treturn true\n\t}\n\treturn a &&\n\t\tb\n}\n";
const LOCAL_TYPES: &str = "package p\nfunc f() {\n\ttype cfg struct {\n\t\tdo func()\n\t\tok bool\n\t}\n\ttype iface interface {\n\t\tf(int) bool\n\t}\n\tx := struct{ a int }{1}\n\t_ = x\n\tvar i interface{} = 1\n\tif _, ok := i.(interface{ f() }); ok {\n\t}\n}\n";
const UNICODE_NAMES: &str = "package p\nfunc héllo(größe int) int {\n\tif größe > 0 {\n\t\treturn héllo(größe - 1)\n\t}\n\treturn 0\n}\n";
const INIT_AND_METHODS: &str = "package p\nvar g = func() int { if true { return 1 }; return 0 }()\nfunc init() { if g > 0 { g++ } }\nfunc (s S) a() {}\nfunc (S) b(x int) { for x > 0 { x-- } }\ntype S struct{}\n";

#[rstest]
#[case::empty("package p\nfunc f() {}\n")]
#[case::if_chain(IF_CHAIN)]
#[case::switches(SWITCHES)]
#[case::bool_runs(BOOL_RUNS)]
#[case::closures(CLOSURES)]
#[case::labels(LABELS)]
#[case::recursion(RECURSION)]
#[case::header_literals(HEADER_LITERALS)]
#[case::types_in_signatures(TYPES_IN_SIGNATURES)]
#[case::noise(NOISE)]
#[case::table_tests(TABLE_TESTS)]
#[case::else_comment_if(ELSE_COMMENT_IF)]
#[case::header_literal_shapes(HEADER_SHAPES)]
#[case::multiline(MULTILINE)]
#[case::local_types(LOCAL_TYPES)]
#[case::unicode(UNICODE_NAMES)]
#[case::init_and_methods(INIT_AND_METHODS)]
fn matches_the_parser(#[case] src: &str) {
    assert!(
        !parse(Lang::Go, src).root_node().has_error(),
        "the fixture is valid Go"
    );
    assert_eq!(fast(src), tree(src));
}

#[rstest]
#[case::unterminated_string("package p\nfunc f() { s := \"open }\n")]
#[case::unbalanced("package p\nfunc f() {\n")]
#[case::no_name("package p\nfunc () {}\n")]
#[case::generic_self_call("package p\nfunc gen[T any](x T) T {\n\treturn gen[T](x)\n}\n")]
fn what_it_cannot_be_sure_of_falls_back(#[case] src: &str) {
    assert!(complexity(src).is_none());
}

#[test]
fn the_public_entry_point_falls_back_to_the_parser() {
    let src = "package p\nfunc gen[T any](x T) T {\n\tif x != nil {\n\t}\n\treturn gen[T](x)\n}\n";
    assert!(complexity(src).is_none());
    assert_eq!(crate::complexity::complexity(Lang::Go, src), tree(src));
}

#[test]
fn a_declaration_without_a_body_is_a_function_of_complexity_one() {
    let got = fast("package p\nfunc ext(x int) int\n");
    assert_eq!(
        (
            got[0].cyclomatic,
            got[0].cognitive,
            got[0].start_line,
            got[0].end_line
        ),
        (1, 0, 2, 2)
    );
    assert_eq!(got, tree("package p\nfunc ext(x int) int\n"));
}

fn go_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            go_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "go") {
            out.push(path);
        }
    }
}

/// Compares the scan with the parser over every valid Go file under `$DENDRITE_GO_CORPUS`.
/// `GOROOT/src` is the corpus that matters: run `DENDRITE_GO_CORPUS=$(go env GOROOT)/src cargo test --release -- --ignored`.
#[test]
#[ignore = "needs DENDRITE_GO_CORPUS: a directory of Go source"]
fn the_corpus_matches_the_parser() {
    let dir = std::env::var("DENDRITE_GO_CORPUS").expect("DENDRITE_GO_CORPUS names a directory");
    let mut files = Vec::new();
    go_files(std::path::Path::new(&dir), &mut files);
    let (mut handled, mut fell_back, mut skipped) = (0, 0, 0);
    let mut mismatches = Vec::new();
    for path in &files {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        if parse(Lang::Go, &src).root_node().has_error() {
            skipped += 1;
            continue;
        }
        match complexity(&src) {
            None => fell_back += 1,
            Some(found) => {
                handled += 1;
                let want = tree(&src);
                if found != want {
                    let first = found.iter().zip(&want).find(|(a, b)| a != b);
                    mismatches.push(format!("{}: {:?}", path.display(), first));
                }
            }
        }
    }
    println!(
        "{} files: {handled} by the scan, {fell_back} fell back, {skipped} with syntax errors skipped, {} differ",
        files.len(),
        mismatches.len()
    );
    for m in mismatches.iter().take(15) {
        println!("  {m}");
    }
    assert!(mismatches.is_empty(), "{} files differ", mismatches.len());
}

/// tree-sitter-go reads `if !a.b { g(x) }` followed by a bare block as a composite literal `a.b{ g(x) }` whose
/// `if` body is the bare block, which nests the block one level deeper. Go's own parser (and this scan) take the
/// first brace as the body. The two differ on such code, and the scan is the one that is right.
#[test]
fn a_qualified_condition_before_a_bare_block_follows_go_not_the_parser() {
    let src = "package p\nfunc f() {\n\tif !a.b {\n\t\tg(x)\n\t}\n\t{\n\t\tif c {\n\t\t}\n\t\tif d {\n\t\t}\n\t}\n}\n";
    let got = fast(src);
    // one `if` at level 0 (1) and two in the bare block, which does not nest (1 + 1)
    assert_eq!((got[0].cyclomatic, got[0].cognitive), (4, 3));
    let plain = src.replace("!a.b", "!b");
    assert_eq!(fast(&plain), tree(&plain));
}

#[test]
fn a_call_on_the_next_line_is_another_statement() {
    let src = "package p\nfunc ru() int {\n\tv := x.ru\n\t(y).z = 1\n\treturn v\n}\n";
    assert_eq!(fast(src)[0].cognitive, 0);
}
