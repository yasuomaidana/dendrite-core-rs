# Changelog

All notable changes are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Java, Kotlin and Svelte (`java`, `kotlin`/`kt`, `svelte`) for `ast.complexity`, `ast.fold_signatures`, `ast.changed_symbols` and `ast.defs_uses`, so Rust, TypeScript, Svelte, Python, Kotlin, Java, C and C++ are all covered. A Svelte component has no functions of its own: those of its `<script>` blocks are analysed as TypeScript (`lang="ts"`) or JavaScript, numbered by the lines of the file (DCORE-20).
- The node tables (`ast::specs`) gained what these languages need, and no language is special-cased in code: where a call names its callee (`callee`), which arm is the default one (`default_arms`), where a function's body is when it is not a `body` field (`body_kinds`), operators that add a path without being boolean operators (`path_ops`, Kotlin's `?:`), and `embedded` regions.
- `ast.defs_uses`: for line ranges, the symbols defined and the identifiers used; dendrite-git-rs builds its hunk dependency graph from it (DCORE-10).
- CI: `.github/workflows/complexity.yml` cross-checks function complexity with clippy `cognitive_complexity` (blocking) and `lizard` (report only until compared with the crate's own measure) (DCORE-14).

- `ast.complexity` for Go no longer builds a syntax tree: a lexer and a structural scan (`ast::go`) give the same metrics 35 times faster (338 MB/s; 8.8 ms against 302 ms for 3 MB), which is 7 times faster than the Go analyzer it replaced. It falls back to tree-sitter for what it cannot decide from tokens, and is checked against it over the Go standard library and a module cache (`DENDRITE_GO_CORPUS`). Where tree-sitter-go misparses valid Go the answer now follows Go (DCORE-13).
- Measured against the Go analyzers that Dendrite removed, on the same 489 Go files (2.9 MB): the cyclomatic values of all 2,955 functions are identical, and the sidecar takes about 19 ms (489 JSON-RPC requests, one process) against about 99 ms for the Go analyzer, about 5 times faster, so the 3x target is met for Go complexity (DCORE-13). The method and caveats are in Dendrite's `docs/quality/core-parity.md`.
- `ast.complexity_batch`: many files in one request, measured on all cores, answered in request order; one bad file reports its own error and does not fail the batch. Protocol 1.1.0 (DCORE-13).
- `ast.complexity_batch` keeps a bounded cache (16,384 files, first in first out) of results keyed by the BLAKE3 digest of the language and the source, so a file the sidecar has seen before is answered without parsing, for every language alike. An unchanged re-run of 400 Python files drops from 134 ms to 1.3 ms and of 300 TypeScript files from 660 ms to 1.2 ms (8 threads, `examples/lang_bench`). The cache lives as long as the sidecar process; it is never written to disk (DCORE-20).

### Changed
- `ast.defs_uses`: a `name` field on a call node is the function being called, a use, not a declared name (Java's `method_invocation`).
- The node table field `ternaries` is now `else_exempt`: parents whose `else` keyword is not an else branch (a Python ternary, a Kotlin `when` entry).
- `ast.complexity` classifies nodes by numeric kind id (`ast::kinds::KindTable`, built once per language) and walks each tree with one cursor, instead of comparing kind names as strings per node. Same output on a 503-file Go corpus (DCORE-13).
- The syntax-analysis modules moved to `src/ast/` so the source directory stays within the structure limit; every public path is re-exported unchanged (DCORE-10).

## [0.1.0]

First release. The crate was a stub before; it now implements the operations of `docs/ARCHITECTURE.md` section 2 that this version covers (DCORE-1).

### Added
- Cyclomatic (McCabe) and cognitive (SonarSource) complexity per function for Go, Rust, Python, TypeScript, TSX, JavaScript, C and C++, driven by per-language node tables (`ast.complexity`). Nested declarations are separate entries, closures count with their parent, the default arm of a `match` or `switch` adds no path.
- Signature folding that replaces function bodies with a placeholder and keeps types, contracts, doc comments and listed symbols (`ast.fold_signatures`).
- Last complete node of a truncated buffer, to splice cut-off LLM output (`ast.last_complete_node`).
- Mapping of changed line ranges to the innermost declaration (`ast.changed_symbols`).
- BLAKE3 digests and chunk identity `blake3(path 0x00 symbol 0x00 content)` with shared test vectors (`hash.blake3`, `hash.chunk_id`).
- Token estimates `ceil(chars / 4 * 1.15)`, always marked `exact: false` (`tokens.count`).
- The `dendrite-core` binary: JSON-RPC 2.0 over stdio, one message per line, protocol 1.0.0, `--version`.
- The `dendrite_core` library with the same operations as functions.
- `tests/self_complexity.rs`: the crate holds its own functions to cyclomatic 10 and cognitive 15.
- CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`; quality workflow for file length and duplication.

### Changed
- `complexity`, `fold_signatures` and `changed_symbols` return their result directly because they cannot fail; syntax errors are reported in the tree and never fail an operation.

### Not in this release
- Exact token counts: no tokenizer files are bundled, so every count is an estimate.
- `cfg.slice` and `isr.extract` answer `-32601`.
