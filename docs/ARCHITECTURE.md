# dendrite-core-rs Architecture

`dendrite-core-rs` provides deterministic, syntax-aware source analysis:

- signature folding
- last-complete-node detection for truncation splicing
- mapping diff hunks to the symbols they change
- cyclomatic and cognitive complexity
- BLAKE3 chunk hashing
- token counting
- reachability slicing and an intermediate semantic representation (ISR)

It ships the `dendrite-core` binary (JSON-RPC 2.0 over stdio) and the `dendrite_core` library. It does no LLM calls, file writing or orchestration. Every operation is a pure function of its inputs.

## 1. Parsing layer

- tree-sitter with compiled-in grammars: Go, Rust, TypeScript/TSX, JavaScript, Python, C, C++. Each operation receives the language ID explicitly, so it never guesses from content.
- One `Parser` per language per thread (tree-sitter parsers are `!Sync`), pooled per thread.
- Per-language node tables map grammar node kinds to the concepts every operation uses: *declaration*, *function body*, *branch*, *loop*, *boolean operator*, *doc comment*, *type declaration*. Supporting a new language means adding a grammar and a table, not new algorithms.

## 2. Operations

### 2.1 `ast.fold_signatures`
Input: `{lang, source, keep_symbols: [name]}`. Output: `{source, folded: [name], tokens_before, tokens_after}`.

Every function or method body whose symbol is not in `keep_symbols` is replaced with `{ /* pruned */ }`, or the language's equivalent (`...` for Python, with indentation preserved). Kept unchanged:

- type declarations, struct fields, interface and trait contracts,
- method receivers and generic parameters,
- doc comments, imports, constants.

Kept symbols keep their bodies verbatim. Typical prompt reduction is 50–80%. The output must parse without `ERROR` nodes.

### 2.2 `ast.last_complete_node`
Input: `{lang, buffer}`. Output: `{offset}` or the error `no_complete_node`.

Used to splice truncated LLM output. It parses the partial buffer and returns the end byte of the last top-level or block-level node that contains no `ERROR` or `MISSING` descendant. The caller keeps `buffer[..offset]`, discards the dangling tail, and asks for a continuation anchored on the preceding lines.

### 2.3 `ast.changed_symbols`
Input: `{lang, source, hunks: [{start_line, end_line}]}`. Output: `[{hunk, symbol, kind, start_line, end_line}]`.

Maps each hunk's line range to the innermost enclosing declaration. Hunks outside any declaration map to the file scope.

### 2.4 `ast.complexity`
Input: `{lang, source}`. Output: `[{symbol, start_line, end_line, cyclomatic, cognitive}]`.

- **Cyclomatic (McCabe):** 1 + the number of decision points: `if`, `else if`, each `case`/match arm except the default, `for`/`while`/`loop`, `catch`, `&&`, `||`, ternary, `?` early returns (Rust).
- **Cognitive (SonarSource rules):**
  - +1 for each break in linear flow (`if`, `else if`, `else`, `switch`, loops, `catch`, `goto`/labelled break, recursion).
  - +nesting depth for nested structures.
  - +1 for each run of like boolean operators.

Gate thresholds used by quality tooling: cyclomatic ≤ 10, cognitive ≤ 15.

### 2.5 `hash.blake3`
Input: `{data}` (UTF-8) or `{data_b64}`. Output: `{hex}`.

Chunk identity (`hash.chunk_id`, input `{path, symbol, content}`) is `blake3(path 0x00 symbol 0x00 content)`: the NUL bytes keep `("a","bc")` and `("ab","c")` apart. Every implementation must produce byte-identical digests. Parity is enforced by shared test vectors in `tests/vectors/blake3.json`.

### 2.6 `tokens.count`
Input: `{model, text}`. Output: `{count, exact}`.

- `exact = true` when a local tokenizer for the model family is available (HuggingFace `tokenizers` files for open-weight models: Qwen, Llama, Mistral, Gemma). **Not implemented yet**: no tokenizer files are bundled, so every count is currently an estimate.
- Otherwise `exact = false`, with `ceil(chars / 4 × 1.15)`. Callers then keep the safety margin or ask the provider's own counting endpoint.

### 2.7 `cfg.slice`
Input: `{lang, sources: [{path, source}], entries: [symbol], direction: "forward"|"backward", depth?}`. Output: `{symbols: [{path, symbol, start_line, end_line}], edges}`.

Builds an intra-project call graph from tree-sitter definitions and call sites (name-resolved within imports and the same package or module). Then it walks forward (callees) or backward (callers) from the entry symbols. The result is the minimal source slice that answers "what does this feature touch". Only source-level slicing is in scope.

### 2.8 `isr.extract`
Input: `{lang, source}`. Output: an ISR document listing types (fields, variants, generics), functions (parameters, return and error types, side effects), control-flow summaries and concurrency primitives.

The ISR is language-neutral JSON used for translation verification and structural comparison. Code generation from the ISR is not part of this repository.

### 2.9 `ast.defs_uses`
Input: `{lang, source, ranges: [{start_line, end_line}]}`. Output: `[{range, defines: [name], uses: [name]}]`.

For each line range of a source file, reports which symbols are defined in the range and which identifiers are used:
- **defines:** names of declarations (functions, methods, types, structs, enums, traits, consts, statics, package-level vars) whose name node lies inside the range.
- **uses:** identifier, type-identifier, and field-identifier references inside the range that are not the name of a declaration in that same range, excluding language keywords and literals.

Both lists are de-duplicated and sorted. Ranges are 1-based and inclusive. Output entries correspond 1:1 to the input ranges in the same order.

`uses` is an over-approximation on purpose: names that are local to the range (parameters, variables declared inside it) and built-in type names are included, because telling them apart needs scope resolution. A caller that builds a dependency graph from it (dendrite-git-rs) gets, at worst, an extra edge that keeps two hunks together; it never misses a real reference.

### Source layout

The syntax analysis lives in `src/ast/` (languages and their node tables, parsing, complexity, folding, last complete node, changed symbols, defs and uses), re-exported at the crate root so `dendrite_core::complexity` and the other paths do not change. `src/rpc.rs` is the JSON-RPC server, `src/hash.rs` and `src/tokens.rs` the rest of the operations.

## 3. Protocol

JSON-RPC 2.0, one message per line on stdin/stdout, logs on stderr. `health` returns `{name: "dendrite-core", version, protocol}`, where `protocol` is a semver string; clients refuse a different major version. The schemas for every method live in `proto/*.json`. Protocol 1.1.0 added `ast.complexity_batch`: `{files: [{lang, source}]}` returns `{results: [{functions} | {error}]}` in request order, measured on up to one thread per core.

Error codes: `-32700` not JSON, `-32600` no method, `-32601` unknown method, `-32602` invalid params (unknown language, bad base64, a missing field), `-32000` operation error with `data.kind` (`no_complete_node`). Syntax errors never fail an operation: they appear in the tree and each operation works around them. A request without an `id` is a notification and gets no answer.

### 3.1 Status

| Method | State |
|---|---|
| `health`, `ast.complexity`, `ast.fold_signatures`, `ast.last_complete_node`, `ast.changed_symbols`, `ast.defs_uses`, `hash.blake3`, `hash.chunk_id`, `tokens.count` | implemented |
| `cfg.slice`, `isr.extract` | not implemented (DCORE-5 and later); the server answers `-32601` |
| exact `tokens.count` | needs bundled tokenizer files |

### 3.2 Complexity details

Each function of a file is measured once; a nested declaration (Rust `fn` in `fn`, Python `def` in `def`) is its own entry and does not count toward the one around it. Closures are scored with the code around them, one nesting level deeper; a closure bound to a name (`const f = () => ...`) is a function. The default arm of a `match`/`switch` adds no path. `else if` costs one, through its `else`. The crate holds itself to the limits: `tests/self_complexity.rs` fails when any of its own functions is above them.

## 4. Performance and parity targets

- At least 3× faster than the in-process Go providers on the same inputs. **Measured (DCORE-13, 503 Go files, 3.0 MB, 8-core Apple silicon, release build):** the Go analyzer took 0.10 s in one process. `ast.complexity` one file per request took 0.40 s at first (4× slower); classifying nodes by kind id and walking each tree with one cursor brought it to 0.31 s, and the profile then shows tree-sitter parsing as about two thirds of what is left. `ast.complexity_batch` (all files in one request, parsed on all cores) takes 0.07 s: 1.4× faster than the single-threaded Go analyzer, with identical results. The 3× target is not met on this corpus: it would need 0.033 s, below what parsing 3 MB costs on this machine. For one file per request there is no concurrency to use, so callers that measure many files should send a batch.
- Folding and complexity results identical to the Go provider for Go sources (golden tests).
- BLAKE3 digests byte-identical across implementations.

## 5. Consumers

- **dendrite-control** starts `dendrite-core` through its sidecar client and falls back to its in-process Go providers when the binary is missing or incompatible.
- **dendrite-git-rs** uses `ast.changed_symbols` semantics to attach hunks to symbols.
- **dendrite-rag-rs** may adopt the shared parsing layer for retrieval chunking.

See [RESPONSIBILITIES.md](RESPONSIBILITIES.md) for the ownership boundary and ticket map.
