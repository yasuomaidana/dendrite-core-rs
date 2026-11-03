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

/// Starts the binary, feeds it raw bytes and returns (stderr, exit code). With `hang_up_stdout` the read end
/// of its stdout is closed before it answers, so its first write fails.
fn run_raw(stdin: &[u8], hang_up_stdout: bool) -> (String, Option<i32>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dendrite-core"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if hang_up_stdout {
        drop(child.stdout.take());
    }
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    (String::from_utf8(out.stderr).unwrap(), out.status.code())
}

#[rstest::rstest]
#[case::not_json("{nope\n", -32700)]
#[case::no_method("{\"jsonrpc\":\"2.0\",\"id\":3}\n", -32600)]
#[case::unknown_method("{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"nope\"}\n", -32601)]
fn answers_failed_requests_with_their_code(#[case] request: &str, #[case] code: i64) {
    let (out, _, ok) = run(&[], request);
    assert!(ok);
    let answer: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(answer["error"]["code"], code);
}

#[rstest::rstest]
#[case::invalid_utf8(b"\xff\xfe\n".as_slice(), false, "valid UTF-8")]
#[case::stdout_closed(b"{\"id\":1,\"method\":\"health\"}\n".as_slice(), true, "dendrite-core: ")]
fn exits_with_1_when_the_stream_breaks(
    #[case] stdin: &[u8],
    #[case] hang_up_stdout: bool,
    #[case] message: &str,
) {
    let (err, code) = run_raw(stdin, hang_up_stdout);
    assert_eq!(code, Some(1));
    assert!(err.contains(message), "{err}");
}
