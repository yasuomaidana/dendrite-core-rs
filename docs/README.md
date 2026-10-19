# dendrite-core-rs Documentation

This directory contains the boundary specifications, architectural designs, and imported reference documentation for the `dendrite-core-rs` repository.

## Core Documentation

- [RESPONSIBILITIES.md](RESPONSIBILITIES.md): Authoritative repository responsibilities, capability ownership, JSON-RPC 2.0 interface contracts, Go package extraction and fallback behavior, and relocated Plane v2 tickets.
- [imports.json](imports.json): Machine-readable manifest of upstream Dendrite documentation relevant to this repository.

## Imported from Dendrite

<!-- Generated from imports.json by scripts/sync_subproject_docs.py (control-plane repo). The list is regenerated; descriptions are kept and must describe this repository. -->

Specifications that define `dendrite-core-rs`'s responsibilities, listed in [`imports.json`](imports.json) and copied into `docs/imported/`. The first line of each copy records where it was copied from. The copies are regenerated, so do not edit them here. Each entry below says what the spec defines for this repository.

### Architecture

- [`2026-10-quality-audit-execution-plan`](imported/architecture/2026-10-quality-audit-execution-plan.md) — Boundary of dendrite-core-rs in the five-repository split: the parsing, folding, complexity, hashing and token-counting work it takes over, and its JSON-RPC sidecar contract.
- [`03-context-intelligence-re`](imported/architecture/03-context-intelligence-re.md) — Context-compaction requirements dendrite-core-rs implements: signature folding, `last_complete_node` truncation and AST-aware slicing.

### Feature specifications

- [`FEAT-004-repo-intel-ast`](imported/features/FEAT-004-repo-intel-ast.md) — Tree-sitter parsing and symbol extraction that dendrite-core-rs provides for the AST knowledge graph.
- [`FEAT-010-ai-assisted-token-optimization`](imported/features/FEAT-010-ai-assisted-token-optimization.md) — Token-optimisation behaviour dendrite-core-rs implements: `fold_signatures` pruning and exact pre-flight token counting.
- [`FEAT-011-polyglot-translation-synthesis`](imported/features/FEAT-011-polyglot-translation-synthesis.md) — Polyglot translation support dendrite-core-rs provides: ISR extraction and the differential-test helpers that verify transpiled code.
- [`FEAT-030-universal-query-engine-3tier-library-reuse`](imported/features/FEAT-030-universal-query-engine-3tier-library-reuse.md) — Reachability slicing dendrite-core-rs implements (`cfg.slice`): forward and backward CFG / AST slices over the ISR.
- [`FEAT-031-persistent-store-hybrid-token-scheduler`](imported/features/FEAT-031-persistent-store-hybrid-token-scheduler.md) — BLAKE3 chunk hashes and token counts that dendrite-core-rs supplies to the persistent store and the token scheduler.

### CLI reference

- [`analyze`](imported/cli/analyze.md) — Analysis command whose complexity and token figures come from dendrite-core-rs.
- [`query`](imported/cli/query.md) — Query command whose slicing and transpilation steps are served by dendrite-core-rs.

### MCP tool reference

- [`dendrite_query_slice`](imported/mcp/dendrite_query_slice.md) — Tool contract for intent-driven slicing that dendrite-core-rs's `cfg.slice` RPC must satisfy.
- [`dendrite_query_transpile`](imported/mcp/dendrite_query_transpile.md) — Tool contract for Rust transpilation and differential testing, backed by dendrite-core-rs's verification helpers.

### Quality

- [`scrum-and-complexity-scorecard`](imported/quality/scrum-and-complexity-scorecard.md) — Complexity thresholds (cyclomatic ≤ 10, cognitive ≤ 15) that dendrite-core-rs's `complexity` metric enforces.

### Reports

- [`2026-10-17-done-ticket-audit`](imported/reports/2026-10-17-done-ticket-audit.md) — Audit of the tickets whose implementation moves to dendrite-core-rs, with their verified state at the split.

