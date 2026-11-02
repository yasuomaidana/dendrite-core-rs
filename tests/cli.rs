//! The `dendrite-core` binary, started the way a client starts it.

use std::io::Write;
use std::process::{Command, Stdio};

fn run(args: &[&str], stdin: &str) -> (String, String, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dendrite-core"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
        out.status.success(),
    )
}

#[test]
fn prints_its_version() {
    let (_, err, ok) = run(&["--version"], "");
    assert!(ok && err.starts_with("dendrite-core "));
}

#[test]
fn answers_a_request_per_line() {
    let request = r#"{"jsonrpc":"2.0","id":1,"method":"ast.complexity","params":{"lang":"rust","source":"fn f(a: bool) { if a {} }"}}"#;
    let (out, _, ok) = run(&[], &format!("{request}\n"));
    assert!(ok);
    let answer: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(answer["result"][0]["symbol"], "f");
    assert_eq!(answer["result"][0]["cyclomatic"], 2);
}
