# Repository Responsibilities: dendrite-core-rs

## 1. Purpose
`dendrite-core-rs` is the high-performance Rust core engine and stdio sidecar for the Dendrite distributed codebase ecosystem. It provides syntax-aware Tree-sitter AST parsing, signature folding, truncation splicing, cyclomatic and cognitive complexity analysis, BLAKE3 content-addressed chunk hashing, and tokenizer-accurate token counting. Operating as an autonomous, high-throughput component, it replaces line-based heuristics in the Dendrite control plane while guaranteeing deterministic equivalence and graceful in-process Go fallback parity.

---

## 2. Owns vs. Does NOT Own

### What `dendrite-core-rs` Owns
- **Multi-Language Tree-sitter AST Parsing**: Native Tree-sitter grammar parsing for Go, Rust, TypeScript, Python, and C/C++.
- **Signature Folding & Context Compaction (`fold_signatures`)**: AST-aware pruning of function and method bodies into `{ /* pruned */ }` placeholders while preserving type declarations, method receivers, interface contracts, and doc comments for prompt token reduction (50–80%).
- **AST Boundary & Truncation Detection (`last_complete_node`)**: Deterministic byte-offset detection of the last complete top-level declaration or statement to anchor token truncation splicing (Dimension B recovery).
- **Code Complexity Metrics (`complexity`)**: Syntax-tree based McCabe cyclomatic complexity ($\le 10$) and cognitive complexity ($\le 15$) calculations across functions, files, and packages.
- **BLAKE3 Content-Addressed Chunk Hashing & Ledger (`blake3`)**: High-throughput BLAKE3 256-bit cryptographic digest calculation for files, AST symbols, and chunks (`blake3(path + symbol + content)`), matching the Go implementation byte-for-byte.
- **Exact Token Counting (`token counting`)**: Exact token counting via native BPE/HuggingFace tokenizers for supported local and open-weight models, paired with verified token estimation algorithms.
- **Intermediate Semantic Representation (ISR) & AST Slicing (`CFG slice`)**: Tree-sitter based extraction of Intermediate Semantic Representations (ISR) capturing type invariants, control flows, and forward/backward reachability slices for intent-driven debloating.
- **Transpilation Verification Helpers**: Differential test harness primitives and AST structural comparator functions supporting polyglot code generation.

### What `dendrite-core-rs` Does NOT Own (Explicit Exclusions)
- **`dendrite-control` (Dendrite Go Repository — `dendrite.dev/control`)**:
  - Owns root CLI dispatching (`cmd/dendrite`), Cobra command trees, and shell completions.
  - Owns LLM provider routing, model lifecycle management (30-minute cooldown, leases, unload), and the 4-tier escalation ladder (`pkg/inference`).
  - Owns test execution, coverage gating ($\ge 85\%$), mock-reachability analysis, and LLM semantic feature assertions (`pkg/assert`, `pkg/quality`).
  - Owns authoritative Plane v2 issue tracking and gate enforcement (`pkg/tracker`).
  - Owns environment orchestration, Docker Compose stacks, and secret management (`pkg/env`).
  - Owns web UI dashboard, embedded HTTP control servers, and WebSocket telemetry streaming (`pkg/server`, `pkg/ui`).
  - Owns in-process Go AST fallback providers (`pkg/ast/goprovider.go`) and sidecar supervision (`sidecar.Client`).
- **`dendrite-rag-rs` (Rust crate `dendrite-rag`)**:
  - Owns hybrid BM25 lexical search and dense vector embeddings (`nomic-embed-text`).
  - Owns Reciprocal Rank Fusion (RRF) search algorithms.
  - Owns Content-Addressable Storage (CAS) library deduplication store and vector indexing.
  - Owns multi-tier memory synchronization engines (`local-only`, `edge`, `push`) and remote pgvector sync.
- **`dendrite-git-rs` (Rust crate `dendrite-git`)**:
  - Owns Git object access and revision graph traversal via `gitoxide` (`gix`).
  - Owns hunk extraction, symbol dependency graph construction, and hunk clustering (`HunkClusterer`).
  - Owns smart commit planning, conventional commit validation, and `--local-review` stacked review branch management.
- **`dendrite-mcp-hub` (Go module `dendrite.dev/mcphub`)**:
  - Owns Model Context Protocol (MCP) server daemon over stdio and SSE.
  - Owns tool registration, dispatching, and rate limiting (`pkg/mcpserver`).
  - Owns JetBrains IDE MCP bridge and upstream connectors for Penpot, Plane, and Engram.
- **`embedag` (Rust crate `crates/embedag`)**:
  - Owns low-level embedding generation kernels, vector similarity math, and native matcher algorithms.

---

## 3. Interfaces

### Exposed Interfaces
1. **JSON-RPC 2.0 Sidecar Protocol over Stdio**:
   - The compiled binary `dendrite-core` runs as a child process of `dendrite-control`, communicating over standard input/output using JSON-RPC 2.0 framing.
   - Schemas are formalized in `proto/*.json` and versioned under strict Semantic Versioning (SemVer).
   - An initial handshake verifies protocol compatibility: a major version mismatch triggers a clean refusal, prompting the consumer to rebuild or fall back.
2. **Core RPC RPC Methods**:
   - `ast.fold_signatures`: Accepts source code, language ID, and kept symbol identifiers; returns compressed source with `{ /* pruned */ }` placeholders.
   - `ast.last_complete_node`: Accepts partial source code buffer and language ID; returns the exact byte offset of the last complete node or `ErrNoCompleteNode`.
   - `ast.changed_symbols`: Accepts unified diff text or base git reference; maps hunk line ranges to enclosing syntax declarations (`SymbolSlice`).
   - `ast.complexity`: Computes McCabe cyclomatic complexity and cognitive complexity per function, returning an array of `FunctionComplexity`.
   - `hash.blake3`: Computes BLAKE3 256-bit digests over binary or text payloads.
   - `tokens.count`: Computes exact token count using local BPE tokenizers or returns bounded estimates.
   - `cfg.slice`: Computes forward/backward reachability slices from specified entry points.
3. **Rust Crate Library API**:
   - Crate `dendrite-core` exports public Rust library traits and structures for direct consumption by sibling Rust crates within a Cargo workspace or pool.

### Consumers
- **`dendrite-control`**: Consumes the JSON-RPC 2.0 stdio sidecar through `sidecar.Client` (in `pkg/ast` / `pkg/sidecar`).
- **Sibling Rust Crates (`dendrite-rag-rs`, `dendrite-git-rs`)**: Consume `dendrite-core` as a path or git dependency for shared AST models and BLAKE3 utilities.
- **Developer Workstations / CI Harnesses**: Invoked via CLI or differential test suites during build, linting, and quality verification.

---

## 4. Code Being Extracted / Replaced & Go Fallback Behaviour

### Exact Dendrite Go Packages & Files Replaced
- **`pkg/ast/provider.go`**:
  - Replaces the generic `ASTProvider` abstraction with native Tree-sitter parsers supporting polyglot source trees.
- **`pkg/ast/goprovider.go`**:
  - Replaces `GoASTProvider` (`FoldSignatures`, `LastCompleteNode`, `ChangedSymbols`) with Rust-native Tree-sitter grammar traversals that execute $\ge 3\times$ faster.
- **`pkg/ast/complexity.go`**:
  - Replaces `CyclomaticComplexity`, `AnalyzeFuncComplexity`, `AnalyzeFileComplexity`, and legacy line heuristics (`AnalyzeSourceComplexity`) with exact AST-based McCabe and cognitive metric calculators.
- **`pkg/ai/tokenopt/compactor.go`**:
  - Replaces character heuristic `EstimateTokens` (~4 chars/token) with exact tokenizer counting, and replaces `CompactGoSource` line-based fallback with polyglot Tree-sitter AST pruning.
- **`pkg/tokens/evaluator.go`**:
  - Complements token window evaluators by supplying exact local token counts.
- **`pkg/store/segmented` & `pkg/ctxbudget`**:
  - Replaces Go BLAKE3 chunk hashing and provides chunk checkpointer primitives for Dimension B (truncation splicing) and Dimension C (chunk ledger).
- **`pkg/ast/isr` & `pkg/query/transpile`** (relocated tickets DEN-020 and DEN-056):
  - Provides the missing Intermediate Semantic Representation (ISR) Tree-sitter parser and differential transpilation test harness.

### Go Fallback Behaviour
The Go control plane maintains runtime resilience through `sidecar.Client`:
1. **Startup Handshake**: During initialization, `dendrite-control` launches `dendrite-core` and executes a health/version handshake.
2. **Transparent Fallback**: If the `dendrite-core` binary is missing, unstartable, terminates unexpectedly, or reports a protocol mismatch:
   - A warning log is emitted (`"dendrite-core sidecar unavailable, falling back to in-process Go provider"`).
   - For Go code, execution falls back immediately to `GoASTProvider` (`pkg/ast/goprovider.go`), which uses Go's standard library `go/parser` and `go/ast`.
   - For token counting, execution falls back to character heuristics (`pkg/ai/tokenopt.EstimateTokens`, ~4 characters/token with 1.15× safety margin).
   - For hashing, execution falls back to Go BLAKE3 (`lukechampine.com/blake3`), ensuring byte-identical digests.
   - For non-Go languages, the system falls back to line-based heuristic scanning until the sidecar is restored.

---

## 5. Dependencies on Sibling Repositories

### Direct Crate Dependencies
- **`embedag`**:
  - Path: `../../RustRoverProjects/embedag/crates/embedag`
  - Cargo configuration: `default-features = false, features = ["core", "matcher"]`
  - Used for embedding abstraction interfaces and similarity matching operations.

### Multi-Directory Workspace & Pool Synchronization (`dendrite pool`)
- Sibling directories are coordinated via `dendrite pool`:
  ```
  ~/GoLandProjects/
  ├── dendrite-control/     Go  module dendrite.dev/control
  ├── dendrite-mcp-hub/     Go  module dendrite.dev/mcphub
  ├── dendrite-core-rs/     Rust crate dendrite-core
  ├── dendrite-rag-rs/      Rust crate dendrite-rag
  └── dendrite-git-rs/      Rust crate dendrite-git
  ```
- **`dendrite pool resolve`**:
  - Automatically injects Cargo patch overrides into consuming Rust crates:
    `[patch."https://github.com/yasuomaidana/dendrite-core-rs"] dendrite-core = { path = "../dendrite-core-rs" }`
  - Generates cross-project IDE module configurations (`.idea/modules.xml`) for GoLand and RustRover.
  - Ensures changes to `dendrite-core-rs` RPC interfaces or data structures immediately surface in `dendrite-control` build tests.

---

## 6. Ticket Map

Authoritative tracking is governed in Plane project **DCORE** (workspace `dendrite-ws`, `http://localhost:8090`).
The extraction umbrella in Dendrite is **DEN-972**, and the first child ticket in DCORE is **DCORE-1**.

| Ticket ID | Title | Capability Group | Original State | Relocated State | Implementation Notes |
|:---|:---|:---|:---|:---|:---|
| **DEN-010** | Tree-sitter AST & Complexity Analyzer | AST & Complexity Analysis | Done (Go heuristic) | Move to DCORE | Fully implemented in Go `pkg/ast`; Rust Tree-sitter replacement planned. |
| **DEN-972** | `dendrite-core-rs`: tree-sitter fold / last-node / complexity / blake3 / tokenize | Core Rust Extraction Umbrella | Milestone M7 | Planned (Backlog) | Extraction umbrella ticket covering fold parity, byte-identical BLAKE3, and $\ge 3\times$ speedup. |
| **DEN-018** | AST Context Compaction & LiteLLM Prefix Cache Harness | Token Optimization & Compaction | Done (Go) | Split to DCORE | Go compactor in `pkg/ai/tokenopt` done; polyglot AST compaction moving to Rust. |
| **DEN-942** | Truncation splicer (Go via `go/ast`; other languages after DEN-972) | Token-Limit Recovery | Backlog | Split to DCORE | Splicer orchestration in Go `pkg/ctxbudget`; polyglot last-complete-node detection in Rust. |
| **DEN-943** | BLAKE3 chunk ledger + WAL + resume | Chunk Ledger & Storage | Backlog | Move to DCORE | High-throughput BLAKE3 hashing and ledger state recovery. |
| **DEN-020** | Intermediate Semantic Representation ISR Tree-sitter Parser | Polyglot Translation & ISR | To Do | Move to DCORE | Target package `pkg/ast/isr` does not exist in Dendrite; fully assigned to Rust core. |
| **DEN-056** | Idiomatic Binary/Repo-to-Rust Transpiler & Differential Test Harness | Transpilation & Verification | To Verify | Split to DCORE | Safe Rust transpiler and differential test harness (`pkg/query/transpile`, `tests/differential`). |

### Plane keys (synced with Plane on 2026-10-18)

Each relocated ticket has its own key in `DCORE`. The DEN original either is archived with the `Relocated` state (**move**) or stays open in DEN as the Go-side umbrella with the `relocated-split` label (**split**).

| DCORE key | Origin in DEN | Action | DCORE state |
| :--- | :--- | :--- | :--- |
| **DCORE-1** | umbrella DEN-972 | first child | Backlog |
| **DCORE-2** | DEN-943 (DEN #175) | move | Backlog |
| **DCORE-3** | DEN-942 (DEN #174) | split | Backlog |
| **DCORE-4** | DEN-056 (DEN #56) | split | To Verify |
| **DCORE-5** | DEN-020 (DEN #20) | move | To Do |
| **DCORE-6** | DEN-018 (DEN #18) | split | Done |
| **DCORE-7** | DEN-010 (DEN #10) | move | Done |

---

## 7. Imported Docs
The following documentation files from the Dendrite control plane (`Dendrite/docs/...`) are mirrored and indexed in `docs/imports.json`:

1. `docs/architecture/2026-10-quality-audit-execution-plan.md` — Authoritative blueprint defining the 5 sub-project decomposition (Pillar 9 §5), extraction order, sidecar JSON-RPC contract, and Plane v2 milestones (DEN-972).
2. `docs/architecture/03-context-intelligence-re.md` — Architectural foundation for Tree-sitter AST parsing, precision context envelopes, and 50–80% token reduction via structural pruning.
3. `docs/features/FEAT-004-repo-intel-ast.md` — Specifications for AST knowledge graph extraction, symbol call-graphs, and offline cyclomatic/cognitive complexity computation ($\le 10$).
4. `docs/features/FEAT-010-ai-assisted-token-optimization.md` — Specifications for Tree-sitter semantic context compaction, AST signature folding, and cleanroom capability import.
5. `docs/features/FEAT-011-polyglot-translation-synthesis.md` — Specifications for Tree-sitter Intermediate Semantic Representation (ISR), cross-language translation, and dual property test verification.
6. `docs/features/FEAT-030-universal-query-engine-3tier-library-reuse.md` — Architecture for intent-driven forward/backward CFG/AST reachability slicing, safe Rust transpilation, and BLAKE3 chunk pre-checks.
7. `docs/features/FEAT-031-persistent-store-hybrid-token-scheduler.md` — Specifications for BLAKE3 content-addressed persistent chunk storage, AST zero-token pass, and segmented analysis caching.
8. `docs/cli/analyze.md` — CLI specifications for `dendrite analyze repo` and `complexity` subcommands computing function-level cyclomatic and cognitive metrics.
9. `docs/cli/query.md` — CLI specifications for `dendrite query slice`, `swap`, and `transpile` driven by AST/CFG reachability.
10. `docs/quality/scrum-and-complexity-scorecard.md` — Quality gates and engineering standards, requiring cyclomatic complexity $\le 10$, test coverage $\ge 85\%$, zero panics, and Testcontainers validation.
11. `docs/reports/2026-10-17-done-ticket-audit.md` — Baseline audit cataloging the status of tickets DEN-010, DEN-018, DEN-020, DEN-056, and noting missing implementations in `pkg/ast/isr`.

---

## 8. Open Questions & Boundary Conflicts

1. **Tree-sitter Grammar Distribution and Cross-Compilation**:
   Compiling native C Tree-sitter grammars (Go, Rust, TypeScript, Python, C/C++) directly into the Rust crate impacts build times and cross-compilation complexity across target platforms (`darwin-arm64`, `linux-amd64`, `linux-arm64`). The packaging strategy (vendored C sources vs. dynamically loaded libraries vs. pre-built grammar crates) must be evaluated to ensure seamless `cargo build` in CI and local environments.
2. **ISR Parsing vs. Target Code Synthesis Responsibility**:
   Ticket DEN-020 (Tree-sitter ISR parsing) is relocated to `dendrite-core-rs`, whereas ticket DEN-021 (target code synthesis and property test generation) remains in `dendrite-control`. A clean boundary must be enforced: `dendrite-core-rs` is strictly responsible for deterministic AST analysis, type invariant extraction, and ISR JSON serialization; LLM-assisted code generation, prompt synthesis, and file writing belong exclusively to `dendrite-control`.
3. **BLAKE3 Chunk Ledger vs. WAL Journal Storage**:
   Ticket DEN-943 encompasses both BLAKE3 chunk hashing and write-ahead log (WAL) persistence (`.dendrite/wal.journal`). While BLAKE3 digest calculation is CPU-bound and belongs in `dendrite-core-rs`, file I/O, cursor fsync, and ticket state transitions are tightly bound to the Go control plane (`pkg/ctxbudget`). The exact division between Rust computational hashing and Go journal persistence must be codified in `proto/*.json`.
4. **Token Counting Fidelity Across Model Families**:
   Exact token counting in Rust is straightforward for open-weight models using HuggingFace BPE/tokenizers (e.g., Qwen, Llama), but frontier models (Claude 3.5, Gemini 2.5) use proprietary tokenizers or remote endpoints (`count_tokens`). `dendrite-core-rs` must explicitly return an `exact: bool` flag so `dendrite-control` knows when to apply the 1.15× heuristic safety factor.
5. **Source-Level Slicing vs. Decompiled Binary Slicing**:
   FEAT-030 specifies forward and backward CFG reachability slicing for both source code and disassembled binaries (Hopper/Ghidra REA evidence). `dendrite-core-rs` should constrain its scope to source-level AST/CFG slicing, leaving disassembled binary graph synthesis and decompilation evidence handling to the REA worker stack.
