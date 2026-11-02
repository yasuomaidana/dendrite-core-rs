# Changelog

All notable changes are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
- Measured parity with the Go analyzers and the 3x speed target are specified but not measured.
