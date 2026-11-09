# dendrite-core-rs

Rust core: tree-sitter signature folding, last-complete-node, complexity, BLAKE3 hashing and token counting sidecar (Pillar 9/13)

Provides high-performance Tree-sitter AST parsing, signature folding, and truncation splicing for multi-language context budget management.
Computes McCabe cyclomatic and cognitive complexity metrics to enforce deterministic architectural quality gates across repositories.
Implements BLAKE3 content-addressed hashing and chunk state management alongside tokenizer-accurate token counting for LLM inference.
Acts as a decoupled stdio JSON-RPC 2.0 sidecar with guaranteed Go in-process fallback parity for the Dendrite ecosystem.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the technical specification and [docs/RESPONSIBILITIES.md](docs/RESPONSIBILITIES.md) for the ownership boundary and ticket map.

## Use

```bash
cargo build --release                    # target/release/dendrite-core
echo '{"jsonrpc":"2.0","id":1,"method":"ast.complexity","params":{"lang":"go","source":"package p\nfunc f(a bool) { if a {} }"}}' \
  | target/release/dendrite-core
```

As a library: `dendrite_core::complexity::complexity(Lang::Go, source)` returns the cyclomatic and cognitive complexity of every function; `fold::fold_signatures`, `last_node::last_complete_node`, `symbols::changed_symbols`, `hash::{blake3_hex, chunk_id}` and `tokens::count` are the other operations. What is and is not implemented is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#31-status).

## Check

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Tests use the `rstest` crate. `tests/self_complexity.rs` fails when any function of this crate is above the complexity limits (cyclomatic 10, cognitive 15).
