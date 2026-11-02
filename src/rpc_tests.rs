use super::*;
use rstest::rstest;

fn ask(line: &str) -> Value {
    serde_json::from_str(&handle_line(line).expect("an answer")).unwrap()
}

fn request(method: &str, params: Value) -> Value {
    ask(&json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params}).to_string())
}

#[test]
fn health_reports_name_and_protocol() {
    let r = request("health", Value::Null);
    assert_eq!(r["id"], 7);
    assert_eq!(r["result"]["name"], "dendrite-core");
    assert_eq!(r["result"]["protocol"], PROTOCOL);
}

#[rstest]
#[case::complexity("ast.complexity", json!({"lang": "go", "source": "package p\nfunc f() {}"}), "/result/0/cyclomatic", json!(1))]
#[case::fold("ast.fold_signatures", json!({"lang": "go", "source": "package p\nfunc f() { x() }\n"}), "/result/folded/0", json!("f"))]
#[case::fold_keep("ast.fold_signatures", json!({"lang": "go", "source": "package p\nfunc f() { x() }\n", "keep_symbols": ["f"]}), "/result/folded", json!([]))]
#[case::last_node("ast.last_complete_node", json!({"lang": "go", "buffer": "package p\nfunc f() {}\nfunc g("}), "/result/offset", json!(21))]
#[case::changed("ast.changed_symbols", json!({"lang": "go", "source": "package p\nfunc f() {\n}\n", "hunks": [{"start_line": 2, "end_line": 2}]}), "/result/0/symbol", json!("f"))]
#[case::defs_uses("ast.defs_uses", json!({"lang": "go", "source": "package p\nfunc f() {\n\tg()\n}\n", "ranges": [{"start_line": 2, "end_line": 4}]}), "/result/0/defines/0", json!("f"))]
#[case::defs_uses_uses("ast.defs_uses", json!({"lang": "go", "source": "package p\nfunc f() {\n\tg()\n}\n", "ranges": [{"start_line": 2, "end_line": 4}]}), "/result/0/uses/0", json!("g"))]
#[case::blake3_text("hash.blake3", json!({"data": "abc"}), "/result/hex", json!("6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"))]
#[case::blake3_b64("hash.blake3", json!({"data_b64": "YWJj"}), "/result/hex", json!("6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"))]
#[case::chunk("hash.chunk_id", json!({"path": "a", "symbol": "b", "content": "c"}), "/result/hex", json!(chunk_id("a", "b", "c")))]
#[case::tokens("tokens.count", json!({"model": "qwen", "text": "abcd"}), "/result", json!({"count": 2, "exact": false}))]
fn answers(
    #[case] method: &str,
    #[case] params: Value,
    #[case] pointer: &str,
    #[case] want: Value,
) {
    assert_eq!(request(method, params).pointer(pointer), Some(&want));
}

#[rstest]
#[case::unknown_method("nope", json!({}), -32601, None)]
#[case::unknown_language("ast.complexity", json!({"lang": "cobol", "source": ""}), -32602, None)]
#[case::unknown_language_fold("ast.fold_signatures", json!({"lang": "cobol", "source": ""}), -32602, None)]
#[case::unknown_language_last("ast.last_complete_node", json!({"lang": "cobol", "buffer": ""}), -32602, None)]
#[case::unknown_language_changed("ast.changed_symbols", json!({"lang": "cobol", "source": "", "hunks": []}), -32602, None)]
#[case::unknown_language_defs_uses("ast.defs_uses", json!({"lang": "cobol", "source": "", "ranges": []}), -32602, None)]
#[case::missing_field_defs_uses("ast.defs_uses", json!({"lang": "go", "source": ""}), -32602, None)]
#[case::missing_field("ast.complexity", json!({"lang": "go"}), -32602, None)]
#[case::both_hash_inputs("hash.blake3", json!({"data": "a", "data_b64": "YQ=="}), -32602, None)]
#[case::no_hash_input("hash.blake3", json!({}), -32602, None)]
#[case::bad_base64("hash.blake3", json!({"data_b64": "!!"}), -32602, None)]
#[case::nothing_complete("ast.last_complete_node", json!({"lang": "go", "buffer": ""}), -32000, Some("no_complete_node"))]
fn fails(
    #[case] method: &str,
    #[case] params: Value,
    #[case] code: i64,
    #[case] kind: Option<&str>,
) {
    let r = request(method, params);
    assert_eq!(r["error"]["code"], code);
    assert_eq!(r["error"]["data"]["kind"].as_str(), kind);
}

#[rstest]
#[case::not_json("{nope", Value::Null, -32700)]
#[case::no_method(r#"{"jsonrpc":"2.0","id":3}"#, json!(3), -32600)]
fn malformed_requests(#[case] line: &str, #[case] id: Value, #[case] code: i64) {
    let r = ask(line);
    assert_eq!(
        (r["id"].clone(), r["error"]["code"].clone()),
        (id, json!(code))
    );
}

#[test]
fn notifications_get_no_answer() {
    assert_eq!(handle_line(r#"{"jsonrpc":"2.0","method":"health"}"#), None);
}

#[test]
fn serve_answers_each_line_and_skips_blanks() {
    let input = "{\"id\":1,\"method\":\"health\"}\n\n   \n{\"id\":2,\"method\":\"nope\"}\n";
    let mut out = Vec::new();
    serve(input.as_bytes(), &mut out).unwrap();
    let lines: Vec<_> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(String::from)
        .collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"result\"") && lines[1].contains("-32601"));
}

/// An output that refuses writes (`write_ok` false) or accepts them and then fails to flush.
struct Failing {
    write_ok: bool,
}

impl Write for Failing {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.write_ok {
            Ok(buf.len())
        } else {
            Err(std::io::Error::other("closed"))
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("flush failed"))
    }
}

#[rstest]
#[case::version(&["dendrite-core", "--version"], "", 0, "dendrite-core 0.1.0")]
#[case::serves(&["dendrite-core"], "{\"id\":1,\"method\":\"health\"}\n", 0, "")]
fn run_modes(#[case] args: &[&str], #[case] input: &str, #[case] code: i32, #[case] stderr: &str) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run(&args, input.as_bytes(), &mut out, &mut err), code);
    assert_eq!(String::from_utf8(err).unwrap().trim(), stderr);
}

#[rstest]
#[case::unreadable_input(&[0xff, b'\n'][..], true, "stream did not contain valid UTF-8")]
#[case::failing_write(&b"{\"id\":1,\"method\":\"health\"}\n"[..], false, "closed")]
#[case::failing_flush(&b"{\"id\":1,\"method\":\"health\"}\n"[..], true, "flush failed")]
fn run_reports_io_failures(#[case] input: &[u8], #[case] write_ok: bool, #[case] message: &str) {
    let args = vec!["dendrite-core".to_string()];
    let mut err = Vec::new();
    assert_eq!(run(&args, input, Failing { write_ok }, &mut err), 1);
    assert!(String::from_utf8(err).unwrap().contains(message));
}
