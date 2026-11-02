//! JSON-RPC 2.0 over lines: one request per line in, one response per line out. Logs go to stderr.

use std::io::{BufRead, Write};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::complexity::complexity;
use crate::error::CoreError;
use crate::fold::fold_signatures;
use crate::hash::{blake3_hex, chunk_id};
use crate::lang::Lang;
use crate::last_node::last_complete_node;
use crate::symbols::{changed_symbols, Hunk};
use crate::tokens;

/// The protocol version; a client refuses a different major version.
pub const PROTOCOL: &str = "1.0.0";

type Handler = fn(Value) -> Result<Value, CoreError>;

const METHODS: &[(&str, Handler)] = &[
    ("health", health),
    ("ast.complexity", ast_complexity),
    ("ast.fold_signatures", ast_fold),
    ("ast.last_complete_node", ast_last_node),
    ("ast.changed_symbols", ast_changed),
    ("hash.blake3", hash_blake3),
    ("hash.chunk_id", hash_chunk_id),
    ("tokens.count", tokens_count),
];

/// Every `ast.*` method names the language; the rest of its params are `P`.
#[derive(Deserialize)]
struct WithLang<P> {
    lang: String,
    #[serde(flatten)]
    rest: P,
}

#[derive(Deserialize)]
struct SourceParams {
    source: String,
}

#[derive(Deserialize)]
struct FoldParams {
    source: String,
    #[serde(default)]
    keep_symbols: Vec<String>,
}

#[derive(Deserialize)]
struct BufferParams {
    buffer: String,
}

#[derive(Deserialize)]
struct ChangedParams {
    source: String,
    hunks: Vec<Hunk>,
}

#[derive(Deserialize)]
struct HashParams {
    data: Option<String>,
    data_b64: Option<String>,
}

#[derive(Deserialize)]
struct ChunkParams {
    path: String,
    symbol: String,
    content: String,
}

#[derive(Deserialize)]
struct TokensParams {
    model: String,
    text: String,
}

/// Decodes the params, runs `f` and encodes its result.
fn call<P: DeserializeOwned, R: Serialize>(
    params: Value,
    f: impl FnOnce(P) -> Result<R, CoreError>,
) -> Result<Value, CoreError> {
    let parsed =
        serde_json::from_value(params).map_err(|e| CoreError::InvalidParams(e.to_string()))?;
    Ok(serde_json::to_value(f(parsed)?).expect("results are plain data"))
}

fn health(_: Value) -> Result<Value, CoreError> {
    Ok(json!({"name": "dendrite-core", "version": env!("CARGO_PKG_VERSION"), "protocol": PROTOCOL}))
}

/// Like [`call`], for a method that works on one language.
fn call_lang<P: DeserializeOwned, R: Serialize>(
    params: Value,
    f: impl FnOnce(Lang, P) -> Result<R, CoreError>,
) -> Result<Value, CoreError> {
    call(params, |p: WithLang<P>| f(Lang::from_id(&p.lang)?, p.rest))
}

fn ast_complexity(params: Value) -> Result<Value, CoreError> {
    call_lang(params, |lang, p: SourceParams| complexity(lang, &p.source))
}

fn ast_fold(params: Value) -> Result<Value, CoreError> {
    call_lang(params, |lang, p: FoldParams| {
        fold_signatures(lang, &p.source, &p.keep_symbols)
    })
}

fn ast_last_node(params: Value) -> Result<Value, CoreError> {
    call_lang(params, |lang, p: BufferParams| {
        last_complete_node(lang, &p.buffer).map(|offset| json!({"offset": offset}))
    })
}

fn ast_changed(params: Value) -> Result<Value, CoreError> {
    call_lang(params, |lang, p: ChangedParams| {
        changed_symbols(lang, &p.source, &p.hunks)
    })
}

fn hash_blake3(params: Value) -> Result<Value, CoreError> {
    call(params, |p: HashParams| {
        let bytes = match (p.data, p.data_b64) {
            (Some(data), None) => data.into_bytes(),
            (None, Some(b64)) => STANDARD
                .decode(b64)
                .map_err(|e| CoreError::InvalidParams(e.to_string()))?,
            _ => {
                return Err(CoreError::InvalidParams(
                    "give exactly one of data and data_b64".into(),
                ))
            }
        };
        Ok(json!({"hex": blake3_hex(&bytes)}))
    })
}

fn hash_chunk_id(params: Value) -> Result<Value, CoreError> {
    call(params, |p: ChunkParams| {
        Ok(json!({"hex": chunk_id(&p.path, &p.symbol, &p.content)}))
    })
}

fn tokens_count(params: Value) -> Result<Value, CoreError> {
    call(params, |p: TokensParams| {
        Ok(tokens::count(&p.model, &p.text))
    })
}

fn dispatch(method: &str, params: Value) -> Result<Value, CoreError> {
    let handler = METHODS
        .iter()
        .find(|(name, _)| *name == method)
        .map(|(_, handler)| handler)
        .ok_or_else(|| CoreError::MethodNotFound(method.to_string()))?;
    handler(params)
}

fn failure(id: Value, code: i64, message: &str, kind: &str) -> String {
    let mut error = json!({"code": code, "message": message});
    if !kind.is_empty() {
        error["data"] = json!({"kind": kind});
    }
    json!({"jsonrpc": "2.0", "id": id, "error": error}).to_string()
}

/// Answers one request line. A notification (no `id`) gets no answer.
pub fn handle_line(line: &str) -> Option<String> {
    let request: Value = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(e) => return Some(failure(Value::Null, -32700, &e.to_string(), "")),
    };
    let id = request.get("id")?.clone();
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Some(failure(id, -32600, "invalid request: no method", ""));
    };
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    Some(match dispatch(method, params) {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
        Err(e) => failure(id, e.code(), &e.to_string(), e.kind()),
    })
}

/// Serves requests until `input` ends.
pub fn serve(input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if let Some(answer) = handle_line(&line).filter(|_| !line.trim().is_empty()) {
            writeln!(output, "{answer}")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// The program: `--version` prints the version, no argument serves stdin to stdout. Returns the exit code.
pub fn run(
    args: &[String],
    input: impl BufRead,
    output: impl Write,
    mut errors: impl Write,
) -> i32 {
    if args.get(1).is_some_and(|a| a == "--version") {
        let _ = writeln!(errors, "dendrite-core {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    match serve(input, output) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(errors, "dendrite-core: {e}");
            1
        }
    }
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
