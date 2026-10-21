# dendrite-core-rs

Rust core: tree-sitter signature folding, last-complete-node, complexity, BLAKE3 hashing and token counting sidecar (Pillar 9/13)

Provides high-performance Tree-sitter AST parsing, signature folding, and truncation splicing for multi-language context budget management.
Computes McCabe cyclomatic and cognitive complexity metrics to enforce deterministic architectural quality gates across repositories.
Implements BLAKE3 content-addressed hashing and chunk state management alongside tokenizer-accurate token counting for LLM inference.
Acts as a decoupled stdio JSON-RPC 2.0 sidecar with guaranteed Go in-process fallback parity for the Dendrite ecosystem.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the technical specification and [docs/RESPONSIBILITIES.md](docs/RESPONSIBILITIES.md) for the ownership boundary and ticket map.
