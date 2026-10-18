<!-- Imported from Dendrite docs/reports/2026-10-17-done-ticket-audit.md @ 69ea048 on 2026-10-18. Source of truth for this repo's scope; keep in sync with the Dendrite copy until the extraction umbrella lands. -->

# Done / To Verify Ticket Audit (2026-10-17)

Audit of every Plane ticket that was marked `Done` or `To Verify` in the Dendrite Core project, against the real source code. Per-ticket verdicts were produced by AGY (Gemini 3.8 Flash) reading the code with measured `go test -cover` evidence, spot-checked by Claude (one false 'not implemented' verdict, DEN-702, was overridden after a code check), and applied to Plane through `dendrite plane update` and `dendrite dev ticket set` (DEN-917).

## Summary

- Tickets audited: **139**
- Implemented: **91** · partial: **14** · not implemented: **34**
- Flagged as under-specified: **91**; descriptions rebuilt in Plane to the ticket schema (context, targets, entry points, acceptance criteria, verification): **73**
- Moved back from `Done`/`To Verify`: **51** (implemented `Done` tickets keep their human sign-off)
- Coverage below the 100% goal does not demote a ticket on its own; it is tracked by the per-package coverage ratchet (DEN-986).

## State corrections

| Ticket | Plane # | Verdict | Previous | Now | What is missing |
|---|---|---|---|---|---|
| DEN-009 | 9 | no | Done | To Do | Target package pkg/rea/client does not exist in the repository; Hopper/Ghidra/Radare2 disassembler protocol client not implemented; REA MCP bridge stdio tools not registered |
| DEN-019 | 19 | no | Done | To Do | Package pkg/ai/import does not exist; Cross-repository capability dissection engine (FEAT-010) is not implemented; Cleanroom code synthesizer is not implemented |
| DEN-020 | 20 | no | Done | To Do | Package pkg/ast/isr does not exist; Intermediate Semantic Representation (ISR) Tree-sitter parser |
| DEN-021 | 21 | partial | Done | In Progress | Target language synthesizer generating code from ISR models; Dual property test parity generator for cross-language verification |
| DEN-026 | 26 | yes | To Verify | In Progress | — |
| DEN-050 | 50 | yes | To Verify | In Progress | Fix failing tests in pkg/agent/triage |
| DEN-065 | 65 | no | To Verify | To Do | pkg/story/drift package implementing story AST drift analysis; dendrite ref sync --check-drift CLI flag and drift diff reporting |
| DEN-066 | 66 | no | To Verify | To Do | Package `pkg/story/resume` is missing; `dendrite story resume` CLI subcommand and context re-hydration logic are not implemented; IDE scope restoration for `.idea/scopes/` XML is not implemented |
| DEN-068 | 68 | no | To Verify | To Do | Package pkg/node/wal does not exist; Node write-ahead logging (WAL) journaling protocol is missing; WAL recovery replay mechanism is missing |
| DEN-069 | 69 | no | To Verify | To Do | pkg/backup/engine package providing snapshot creation and validation; dendrite backup CLI commands for creating and restoring state archives |
| DEN-070 | 70 | no | To Verify | To Do | Package `pkg/config/engine` is missing; `dendrite config edit` / `get` / `set` CLI commands are not implemented; Transactional nvim/vim staging with `:wq` validation loop is not implemented |
| DEN-071 | 71 | no | To Verify | To Do | Missing .goreleaser.yaml configuration for cross-compiling linux/darwin/windows binaries; Missing .github/workflows/ci-release.yml triggering on v* tags; Missing Debian .deb packaging and deploy/systemd/dendrite-daemon.service |
| DEN-072 | 72 | no | To Verify | To Do | Package pkg/agent/runtime does not exist; Model capability auto-negotiation protocol is missing; Direct /api/chat streaming transport is missing |
| DEN-073 | 73 | no | To Verify | To Do | pkg/agent/gitgov package implementing the 5-tier autonomy governance matrix; Dynamic policy interceptor enforcing commit, branch, and push authorization rules |
| DEN-075 | 75 | partial | To Verify | In Progress | Package pkg/stories/next does not exist; Automated git checkout of story/<ticket> branch rooted under parent milestone is not implemented; Injection of JetBrains .idea/scopes/Dendrite_Active_Story.xml and active run configuration is missing |
| DEN-078 | 78 | no | To Verify | To Do | Package `pkg/update/lifecycle` is missing; CLI command `dendrite update` querying GitHub Releases and downloading target OS/arch binary is not implemented; SHA-256 signature verification and atomic in-place binary swapping with `.bak` rollback are not implemented |
| DEN-079 | 79 | no | To Verify | To Do | Package pkg/update/devmode does not exist; CLI command dendrite update --dev is not implemented; docs/DEVELOPMENT.md documentation is missing |
| DEN-081 | 81 | no | To Verify | To Do | pkg/env/patterns package implementing scaffolding for 22 GoF design patterns; CLI scaffolding commands for pattern generation and boilerplate generation |
| DEN-082 | 82 | no | To Verify | To Do | Package `pkg/env/updater` is missing; `--sota` flag on `dendrite env update` querying language idioms and framework advisories is not implemented; Visual diff inspector proposing modifications to `.agents/`, `.ai/`, `.gemini/`, and `AGENTS.md` is not implemented |
| DEN-083 | 83 | no | To Verify | To Do | Package pkg/cache/turso does not exist; Embedded Turso / libSQL local cache at .dendrite/cache/dendrite.db in WAL mode is not implemented; Offline-first read-through caching for Plane/Penpot and [OFFLINE CACHE] banner are missing |
| DEN-084 | 84 | no | To Verify | To Do | Package pkg/cache/outbox does not exist; Transactional outbox pattern for offline tracker mutations is missing; Exponential backoff replay worker is missing |
| DEN-085 | 85 | no | To Verify | To Do | pkg/cache/hygiene package verifying working tree exclusion of temporary DB and cache artifacts; Pre-commit staging inspection hooks blocking binary/cache leakage |
| DEN-086 | 86 | no | To Verify | To Do | Package `pkg/security/secretgate` is missing; Pre-commit entropy scanner analyzing staged git diffs before `git commit` is not implemented; Credential pattern detection (GitHub PATs, Tailscale, Plane, Penpot API keys, private keys, `.env`) is not implemented |
| DEN-087 | 87 | partial | To Verify | In Progress | Package pkg/quality/harness does not exist; Automated pre-PR and pre-merge verification gate intercepting gh pr create and gh pr merge is not implemented; Self-healing feedback loop feeding diagnostics back into agent before human escalation is missing |
| DEN-088 | 88 | no | To Verify | To Do | Package pkg/scheduler/failover does not exist; HTTP 429 Copilot-to-Ollama automatic failover protocol is missing; Story state checkpointing in Turso is missing |
| DEN-089 | 89 | partial | To Verify | In Progress | pkg/agent/worktree package binding agent loops to dedicated ephemeral worktrees; dendrite agent run --worktree CLI flag |
| DEN-090 | 90 | no | To Verify | To Do | Package `pkg/audit/logger` is missing; Immutable libSQL append-only table recording git commands, policy verdicts, LLM calls, and token metrics is not implemented; CLI command `dendrite audit --story <id>` displaying chronological traces is not implemented |
| DEN-091 | 91 | no | To Verify | To Do | Package pkg/mcp/publisher does not exist; PublisherAdapter interface and CratePublisher are not implemented; Cargo workspace topological dependency ordering and index propagation delays are missing |
| DEN-092 | 92 | no | To Verify | To Do | Package pkg/mcp/publisher does not exist; Pre-flight MCP bundle validation checks are missing; Registry publication pipeline with rollback is missing |
| DEN-093 | 93 | no | To Verify | To Do | pkg/mcp/publisher package managing MCP tool publication and registry syncing; dendrite_publish_tool MCP tool implementation in mcpserver |
| DEN-094 | 94 | no | To Verify | To Do | Package `pkg/routines/cicd` / `internal/routines/cicd` is missing; Dynamic activation toggle (`SetEnabled(true, ephemeral)`) with auto-revert to disabled at 0 jobs is not implemented; Ephemeral CI/CD disposable container sandbox with `/tmp/build` tmpfs and read-only host repo bind mount is not implemented |
| DEN-095 | 95 | no | To Verify | To Do | Package pkg/routines/cicd does not exist; Tunnels Docker daemon socket via Tailscale encrypted overlay with zero public ingress is not implemented; Backend github_ephemeral_runner one-time runner registration and de-registration is missing |
| DEN-096 | 96 | no | To Verify | To Do | Package pkg/routines/cicd does not exist; Pre-configured CI/CD pipeline recipes are missing; Live pipeline status and log streaming are missing |
| DEN-101 | 99 | no | To Verify | To Do | Architecture review document evaluating Claude Code, OpenAI Operator, and Cursor is missing from docs/; Adapter definitions and CLI invocation harnesses for Claude Code, OpenAI Operator, and Cursor are missing from pkg/agent |
| DEN-102 | 100 | yes | To Verify | In Progress | — |
| DEN-103 | 101 | partial | To Verify | In Progress | Resolution of failing unit tests in pkg/agent; Expansion of unit and branch tests to reach 100% statement coverage in pkg/agent |
| DEND-101 | 103 | partial | To Verify | In Progress | Tree-sitter AST static analyzer for syntax-tree-aware mock detection across Go, Rust, TS, and Python instead of regex grep |
| DEND-102 | 104 | partial | To Verify | In Progress | StatusToVerify is missing from pkg/tracker.Status in pkg/tracker/types.go; dendrite skill verify subcommand is missing from cmd/dendrite/main.go; Automated verification routine checking git diff status, 100% statement coverage, and live integration before marking Done is incomplete |
| DEND-202 | 107 | no | To Verify | To Do | Leiden community detection algorithm for AST symbol clustering; God-node identification and dependency centrality indexing |
| DEND-203 | 108 | partial | To Verify | In Progress | Vector embedding pipeline using nomic-embed-text is not implemented (current engine uses lexical BM25); Confidence score calculation with 0.72 threshold for paid cloud escalation is missing; Content-Addressable Storage (CAS) library deduplication store integrated with RAG is missing |
| DEND-302 | 110 | no | To Verify | To Do | JetBrains IDE Plugin Kotlin/Gradle project structure is missing; Embedded local AI chat tool window is missing; Graphify AST visual code markers are missing |
| DEND-303 | 111 | no | To Verify | To Do | VSCode extension package (plugins/vscode or extensions/vscode) containing extension manifest and client code; Language client integration bridging VSCode to Dendrite MCP server |
| DEND-304 | 112 | no | To Verify | To Do | Headless Lua RPC client (dendrite.nvim) for Neovim LSP integration; Lua RPC protocol bindings to Dendrite MCP server tools |
| DEND-305 | 113 | no | To Verify | To Do | Rust WASM extension crate dendrite-zed does not exist in the repository; Zed editor MCP extension manifests (extension.toml) and Rust WASM bindings are missing; Integration connecting Zed editor to Dendrite MCP server is missing |
| DEND-403 | 116 | no | To Verify | To Do | CLI command dendrite cluster sync; Cross-node endpoint and API token dynamic propagation engine across cluster mesh |
| DEND-501 | 117 | partial | To Verify | In Progress | macOS dissection of PE binaries is blocked with ErrUntrustedOnHost in pkg/rea/client.go:49; Pause/resume checkpointing across multi-stage reverse engineering sessions is missing; Multi-tier LLM complexity triage (local-first with cloud escalation) is missing from REA client |
| DEN-701-COV-1 | 124 | partial | To Verify | In Progress | Reach 100% statement test coverage in pkg/vcs/guard.go (currently 93.9%) |
| DEN-702-COV-1 | 125 | partial | To Verify | In Progress | Statement coverage for pkg/tracker is 79.7%, failing the 100% statement coverage goal for routing.go |
| DEN-704-COV-1 | 127 | partial | To Verify | In Progress | Additional branch and error condition test suites to elevate pkg/vcs statement coverage from 93.9% to 100% |
| DEN-801 | 129 | partial | To Verify | In Progress | Package pkg/prompt does not exist; Multi-turn state machine transitions (Init -> TopicSelect -> Clarification -> ActiveSession) are not implemented; Intent classification routing inputs to optimal agent skills or commands is missing |
| DEN-816 | 145 | partial | To Verify | In Progress | Hardcoded 'oxillama-rust-qwen7b' fallback remains at triage.go:98; Missing dynamic configuration file lookup before defaulting; Tests in pkg/agent/triage are failing |

## All audited tickets

| Ticket | Plane # | Verdict | State | Entry points | Summary |
|---|---|---|---|---|---|
| DEN-001 | 1 | yes | Done | `dendrite dev ticket list [file]`, `dendrite dev ticket set <id> <status>`, `dendrite_ticket_list` | Development status ledger parser and in-memory tracker fully implemented with markdown synchronization and CLI/MCP entrypoints. |
| DEN-002 | 2 | yes | Done | `dendrite dev ollama probe`, `dendrite_ollama_probe` | Idiomatic Ollama Metal client with health checks, model enumeration, and text generation fully implemented and wired into CLI and MCP tools. |
| DEN-003 | 3 | yes | Done | `dendrite dev`, `dendrite commit`, `dendrite_commit` | Developer productivity tools and CLI scaffolding are implemented in pkg/devtool and cmd/dendrite with passing tests. |
| DEN-004 | 4 | yes | Done | `dendrite plane`, `dendrite_plane_issue_create`, `dendrite_plane_issue_update` | Plane v2 issue tracking integration is fully implemented with comprehensive client methods, CLI commands, and MCP tools. |
| DEN-005 | 5 | yes | Done | `dendrite topology [scenario]`, `dendrite dev topology [scenario]`, `dendrite_gateway_render` | Multi-node topology configuration model and scenario placements fully implemented and validated with CLI inspection commands. |
| DEN-006 | 6 | yes | Done | `dendrite topology` | Terraform cloud-init generation and compose profile orchestration are fully implemented in pkg/orchestrator with 93.9% statement coverage. |
| DEN-007 | 7 | yes | Done | `dendrite gateway`, `dendrite dev gateway`, `dendrite_gateway_render` | LiteLLM configuration template and dynamic renderer are implemented in pkg/gateway and verified with 88.3% statement coverage. |
| DEN-008 | 8 | yes | Done | `dendrite gateway governance` | LiteLLM token gateway governance and quota enforcement are implemented with thread-safe tracking and CLI commands. |
| DEN-009 | 9 | no | To Do | — | The REA MCP bridge and disassembler protocol client are missing from the codebase; pkg/rea only contains showcase verification harnesses. |
| DEN-010 | 10 | yes | Done | `dendrite analyze` | AST symbol knowledge graph and cognitive/cyclomatic complexity analyzer are fully implemented in pkg/ast with 92.2% statement coverage. |
| DEN-011 | 11 | yes | Done | `dendrite agent run --sandbox`, `dendrite_agent_run` | Ephemeral hardened container sandbox with RAII Drop is fully implemented in pkg/sandbox with 98.7% test coverage. |
| DEN-012 | 12 | yes | Done | `dendrite workspace` | Workspace isolation registry is fully implemented with robust file-locking and metadata tracking. |
| DEN-013 | 13 | yes | Done | — | Atlassian Jira REST v3 issue tracker adapter and UnifiedIssueTracker pluggable interface are fully implemented and verified. |
| DEN-014 | 14 | yes | Done | `dendrite ui` | Embedded HTTP control server and WebSocket telemetry hub are fully implemented in pkg/server and exposed via dendrite ui. |
| DEN-016 | 16 | yes | Done | `dendrite topology aws-burst`, `dendrite dev topology aws-burst` | Terraform AWS Spot EC2 module and cloud-init bootstrap are fully implemented in infra/modules/aws-burst. |
| DEN-017 | 17 | yes | Done | `dendrite orchestrator aws` | AWS burst orchestration with persistent EBS attachment and infrastructure validation is implemented. |
| DEN-018 | 18 | yes | Done | `dendrite triage --prompt "..." --file <path>` | AST context compaction and LiteLLM prefix cache harness are implemented under pkg/ai/tokenopt and active in agent triage. |
| DEN-019 | 19 | no | To Do | — | Target package pkg/ai/import does not exist in the repository; capability dissection and cleanroom synthesis under FEAT-010 remain to be built. |
| DEN-020 | 20 | no | To Do | — | Target package pkg/ast/isr does not exist in the repository, and the Tree-sitter Intermediate Semantic Representation parser is completely unimplemented. |
| DEN-021 | 21 | partial | In Progress | `dendrite translate` | Language translation pipeline partially implemented; ISR parser exists, but target code synthesizer and dual property parity test generators are missing. |
| DEN-022 | 22 | yes | Done | `dendrite doc reflect [--reference <dir>] [--source <dir>] [--target <dir>]` | AST Docs-as-Code synthesizer harvesting symbols, templates, and Mermaid diagrams is fully implemented and CLI-wired. |
| DEN-023 | 23 | yes | Done | `dendrite doc reflect`, `dendrite doc docusaurus` | Cross-project documentation reflection engine and Docusaurus sync are fully implemented in pkg/docgen/reflect with 91.3% statement coverage. |
| DEN-025 | 25 | yes | Done | `dendrite agent run --sandbox`, `dendrite_agent_run` | Agent sandbox isolation and LiteLLM gateway budget binding is implemented in pkg/agent/sandboxed_runner.go and pkg/sandbox, but recommended state is In Progress due to failing tests in pkg/agent. |
| DEN-026 | 26 | yes | In Progress | `dendrite agent session` | Agent session lifecycle management is implemented in pkg/agent, but unit tests in pkg/agent are currently failing. |
| DEN-027 | 27 | yes | To Verify | `dendrite plane <status|replicate|push-status|issues>`, `dendrite_plane_issues` | Plane v2 and Linear strategy adapters implementing UnifiedIssueTracker are fully operational with CLI and MCP integrations. |
| DEN-028 | 28 | yes | To Verify | `dendrite jetbrains`, `dendrite_jetbrains_setup` | JetBrains IDE MCP integration and communication bridge are fully implemented in pkg/jetbrains with active CLI and MCP tool wiring. |
| DEN-029 | 29 | yes | To Verify | `dendrite env status`, `dendrite env detect`, `dendrite_env_status` | Environment replication engine and toolchain detection are fully implemented in pkg/env with CLI and MCP endpoints. |
| DEN-030 | 30 | yes | To Verify | `dendrite topology network` | Mesh network overlay coordination with Tailscale and WireGuard providers is implemented and integrated with cloudinit. |
| DEN-031 | 31 | yes | To Verify | `dendrite env plan`, `dendrite env replicate`, `dendrite env status` | Self-hosted Plane v2 and Penpot design token stacks and environment replication auditing are fully implemented. |
| DEN-032 | 32 | yes | To Verify | `dendrite topology advisor`, `dendrite_topology_advisor` | Remote SSH hardware discovery and dynamic microservice placement router are fully implemented with 88.7% statement coverage. |
| DEN-033 | 33 | yes | To Verify | `dendrite telemetry`, `dendrite run --telemetry` | Telemetry exporter for Prometheus and Grafana dashboards is fully implemented with 99.2% statement coverage. |
| DEN-034 | 34 | yes | To Verify | `dendrite orchestrator azure` | Azure burst orchestration with managed disk lifecycle handling is fully implemented and tested. |
| DEN-035 | 35 | yes | To Verify | `dendrite inference`, `dendrite_inference_list`, `dendrite_inference_probe` | Unified multi-provider inference engine gateway with dynamic cascade failover across Ollama, OxiLLaMa, vLLM, and TGI is fully implemented. |
| DEN-036 | 36 | yes | To Verify | `dendrite bench`, `dendrite_bench_run` | k6 performance testing, TTFT/TPS benchmarking, and gateway cascade failover harness are fully implemented in pkg/perf with 90.6% coverage. |
| DEN-039 | 39 | yes | To Verify | `dendrite rea showcase`, `dendrite_rea_showcase` | REA showcase runner harness is fully implemented with CLI entrypoints and MCP tools. |
| DEN-040 | 40 | yes | To Verify | `dendrite mcp serve` | Model Context Protocol (MCP) server is implemented with JSON-RPC over stdio, tool dispatching, and CLI daemon command. |
| DEN-041 | 41 | yes | To Verify | `dendrite dist`, `dendrite_dist_sources`, `dendrite_dist_resolve` | Distributed codebase multi-source ecosystem and capability priority resolver are fully implemented with CLI and MCP integration. |
| DEN-042 | 42 | yes | To Verify | `dendrite dist`, `dendrite_dist_decompose` | REA modular decomposition and ecosystem reuse discovery engine are fully implemented across pkg/distcodebase and pkg/rea. |
| DEN-043 | 43 | yes | To Verify | `dendrite node spawn`, `dendrite workspace sync` | Distributed codebase spawner and node lifecycle management are implemented in pkg/distcodebase and pkg/workspace. |
| DEN-044 | 44 | yes | To Verify | `dendrite codebase interop` | Distributed codebase interoperability layer and peer protocol schemas are implemented with registry support. |
| DEN-045 | 45 | yes | To Verify | `dendrite dist spawn`, `dendrite_dist_spawn` | Scenario 4 generative satellite project synthesis and autonomous Plane project provisioning are fully implemented. |
| DEN-046 | 46 | yes | To Verify | `dendrite jetbrains setup`, `dendrite_jetbrains_setup` | JetBrains IDE toolset integration, the 19 MCP tool suite, and developer setup automation are fully implemented across pkg/jetbrains and pkg/mcpserver. |
| DEN-047 | 47 | yes | To Verify | `dendrite perf synthetic` | Synthetic load generation and benchmark performance profiling tool is fully implemented with CLI support. |
| DEN-048 | 48 | yes | To Verify | `dendrite bench gateway`, `dendrite_bench_gateway` | Automatic zero-latency fallback cascade for unreachable inference upstreams during benchmark scenarios is fully implemented. |
| DEN-049 | 49 | yes | To Verify | `dendrite rea showcase` | REA economic model showcase and multi-agent resource exchange scenario simulation are implemented and verified. |
| DEN-050 | 50 | yes | In Progress | `dendrite triage`, `dendrite_triage_prompt`, `dendrite_jetbrains_setup` | Local agent triage gateway and JetBrains AI Chat multi-MCP orchestrator are implemented, but tests are currently failing in pkg/agent/triage. |
| DEN-051 | 51 | yes | To Verify | `dendrite pool add`, `dendrite pool resolve`, `dendrite_pool_resolve` | Autonomous multi-directory manifest dependency scanner and topological DAG solver are fully implemented in pkg/pool/dag with 92.2% statement coverage. |
| DEN-052 | 52 | yes | To Verify | `dendrite pool switch`, `dendrite_pool_switch` | Transactional workspace pool switcher supporting Published, Branch, Local, and Fork modes is fully implemented in pkg/pool/switcher with rollback support. |
| DEN-053 | 53 | yes | To Verify | `dendrite ui pool` | Pool split-view pane management and interactive modal dialog components are fully implemented. |
| DEN-054 | 54 | yes | To Verify | `dendrite query`, `dendrite_query_slice` | Universal intent slicing and binary/repo debloating engine is fully implemented with CFG/AST reachability analysis. |
| DEN-055 | 55 | yes | To Verify | `dendrite query`, `dendrite_query_swap` | Subsystem trait boundary adapter and graphics engine swapper preserving kinematics parity are fully implemented in pkg/query/swap with 95.5% statement coverage. |
| DEN-056 | 56 | yes | To Verify | `dendrite query transpile`, `dendrite_query_transpile` | Vector query transpiler with zero unsafe code, newtype wrappers, and float epsilon assertions is fully implemented in pkg/query/transpile. |
| DEN-057 | 57 | yes | To Verify | `dendrite query fuse` | Reciprocal Rank Fusion (RRF) engine for hybrid search queries is fully implemented and tested. |
| DEN-058 | 58 | yes | To Verify | `dendrite ref`, `dendrite query`, `dendrite_query_reuse_audit` | Strict 3-tier library reuse hierarchy lookup engine and ADR provenance recorder are fully implemented with store integration. |
| DEN-059 | 59 | yes | To Verify | `dendrite store`, `dendrite_store_status`, `dendrite_store_chunks` | BLAKE3 content-addressed persistent segmented store and SQLite WAL chunk checkpointer are fully implemented in pkg/store/segmented with 81.6% statement coverage. |
| DEN-060 | 60 | yes | To Verify | `dendrite store status`, `dendrite store inspect <chunk-id>` | Store Status Inspector CLI & UI with tabular formatting and safe chunk inspection is fully implemented in pkg/store/status and pkg/ui/store. |
| DEN-061 | 61 | yes | To Verify | `dendrite scheduler token` | Token scheduler with dynamic rate-limiting, priority queues, and leaky-bucket throttling is fully implemented. |
| DEN-062 | 62 | yes | To Verify | `dendrite analyze`, `dendrite_scheduler_pause`, `dendrite_scheduler_resume` | Quota floor safety governor (20%/0% drain override) and continuous analysis pause engine are fully implemented. |
| DEN-063 | 63 | yes | To Verify | `dendrite story`, `dendrite_contract_drift_check` | Quad-IDE cross-language schema and IPC contract drift monitor is fully implemented in pkg/polyglot/contract with 91.0% statement coverage. |
| DEN-064 | 64 | yes | To Verify | `dendrite story fork`, `dendrite_story_fork` | Story Fork Controller providing multi-agent branch isolation, worktree allocation, and quotas is implemented in pkg/story/fork with CLI and MCP endpoints. |
| DEN-065 | 65 | no | To Do | — | Story drift detection package and CLI validation flags are completely missing from the codebase. |
| DEN-066 | 66 | no | To Do | — | Multi-repo context re-hydration and IDE scope restorer are not implemented; pkg/story/resume does not exist. |
| DEN-067 | 67 | yes | To Verify | `dendrite move`, `dendrite node migrate`, `dendrite_node_migrate` | 5-phase atomic service migration engine across Local, Debian, and Azure nodes is fully implemented in pkg/node/migrate with 91.8% statement coverage. |
| DEN-068 | 68 | no | To Do | — | Node WAL journaling protocol and Merkle digest verification are not implemented; pkg/node/wal is missing. |
| DEN-069 | 69 | no | To Do | — | Workspace backup engine and snapshot CLI command are not implemented. |
| DEN-070 | 70 | no | To Do | — | Unified configuration subsystem with nvim transactional staging and taplo schema is not implemented; pkg/config/engine does not exist. |
| DEN-071 | 71 | no | To Do | — | Multi-channel distribution pipeline via GoReleaser, Homebrew tap, Debian .deb, and GHCR is not implemented; configuration files and release workflows are missing. |
| DEN-072 | 72 | no | To Do | — | Agent Direct Runtime with auto-negotiation and direct /api/chat token streaming to telemetry is not implemented; pkg/agent/runtime is missing. |
| DEN-073 | 73 | no | To Do | — | Five-tier autonomy matrix for agent Git governance is not implemented in pkg/agent/gitgov. |
| DEN-074 | 74 | yes | To Verify | `dendrite plane link`, `dendrite plane next`, `dendrite_plane_dag` | Ticket-milestone relational ingestion and dependency DAG solver are fully implemented in pkg/tracker/dag with Plane integration. |
| DEN-075 | 75 | partial | In Progress | `dendrite plane next`, `dendrite story next` | DAG unblock weight calculation and smart selection are implemented in pkg/tracker/dag, but automated story branch checkout and JetBrains scope injection remain unbuilt. |
| DEN-076 | 76 | yes | To Verify | `dendrite penpot sync`, `dendrite_penpot_sync` | Penpot MCP and CLI integration for bidirectional canvas sync and design token extraction is fully implemented in pkg/ui/penpot. |
| DEN-077 | 77 | yes | To Verify | `dendrite ui penpot` | Penpot design integration client and asset synchronization CLI commands are fully implemented. |
| DEN-078 | 78 | no | To Do | — | Self-update lifecycle manager with checksum verification and binary rollback is not implemented; pkg/update/lifecycle does not exist. |
| DEN-079 | 79 | no | To Do | — | Developer mode dual-source build engine is not implemented; pkg/update/devmode, CLI update command, and docs/DEVELOPMENT.md are missing. |
| DEN-080 | 80 | yes | To Verify | `dendrite env prepare`, `dendrite_env_prepare` | Dendrite Env Prepare declarative workspace bootstrap and multi-toolchain provisioning is fully implemented in pkg/env/bootstrap. |
| DEN-081 | 81 | no | To Do | — | Gang of Four (GoF) design pattern generator in pkg/env/patterns is completely missing. |
| DEN-082 | 82 | no | To Do | — | SOTA best-practices web/advisory diff inspector and automated 3-way updater are not implemented; pkg/env/updater does not exist. |
| DEN-083 | 83 | no | To Do | — | Embedded Turso/libSQL local offline cache at .dendrite/cache/dendrite.db is not implemented; pkg/cache/turso is missing from the repository. |
| DEN-084 | 84 | no | To Do | — | Offline-First Outbox Pattern for queueing and replaying tracker mutations is not implemented; pkg/cache/outbox is missing. |
| DEN-085 | 85 | no | To Do | — | Cache and DB hygiene validator in pkg/cache/hygiene is not implemented. |
| DEN-086 | 86 | no | To Do | — | Deterministic pre-commit entropy and credential scanner interceptor is not implemented; pkg/security/secretgate does not exist. |
| DEN-087 | 87 | partial | In Progress | `dendrite quality audit` | Anti-mock and test coverage verification tools exist in pkg/quality, but automated pre-PR/merge interceptors and self-healing feedback loops in pkg/quality/harness remain unbuilt, and pkg/agent tests fail. |
| DEN-088 | 88 | no | To Do | — | Multi-LLM Failover Protocol with Copilot-to-Ollama 429 failover and Turso checkpointing is not implemented; pkg/scheduler/failover is missing. |
| DEN-089 | 89 | partial | In Progress | `dendrite story fork` | Git worktree orchestration exists in pkg/story/fork, but dedicated agent loop integration in pkg/agent/worktree and --worktree CLI execution flags are missing. |
| DEN-090 | 90 | no | To Do | — | Immutable libSQL agent audit logger and `dendrite audit` trace CLI are not implemented; pkg/audit/logger does not exist. |
| DEN-091 | 91 | no | To Do | — | Neither pkg/mcp/publisher nor internal/mcp/publisher exists in the codebase; the pluggable PublisherAdapter and Cargo workspace topological publisher remain unimplemented. |
| DEN-092 | 92 | no | To Do | — | MCP Bundle Self-Publish Pipeline with pre-flight checks and rollback is not implemented; pkg/mcp/publisher is missing. |
| DEN-093 | 93 | no | To Do | — | MCP tool publisher package and dynamic tool publishing capabilities are not implemented. |
| DEN-094 | 94 | no | To Do | — | Ephemeral container sandbox runner and dynamic on-demand CI/CD activation routine are not implemented; pkg/routines/cicd does not exist. |
| DEN-095 | 95 | no | To Do | — | Tailscale endpoint definitions exist in pkg/topology, but the ephemeral CI/CD routine package pkg/routines/cicd, Docker overlay tunneling, and one-time GitHub runner backend are not implemented. |
| DEN-096 | 96 | no | To Do | — | CI/CD Pipeline Routine with recipes, live status streaming, and artifact management is not implemented; pkg/routines/cicd is missing. |
| DEN-097 | 97 | yes | To Verify | `dendrite plane doc-sync`, `dendrite_plane_doc_sync` | Plane documentation synchronization syncing local markdown docs and Plane pages bidirectionally is fully implemented. |
| DEN-098 | 98 | yes | To Verify | `dendrite commit`, `dendrite_commit` | Autonomous git commit engine with local model delegation (Ollama / OxiLLaMa) for conventional commit synthesis and ticket provenance is fully implemented. |
| DEN-101 | 99 | no | To Do | — | Architecture evaluation and adapter integrations for Anthropic Claude Code, OpenAI Operator, and Cursor do not exist in docs/ or pkg/agent. |
| DEN-102 | 100 | yes | In Progress | `dendrite agent run --sandboxed`, `dendrite_agent_run` | Container-based sandboxed agent runner is implemented in pkg/agent/sandboxed_runner.go, but recommended state is In Progress due to test failures in pkg/agent. |
| DEN-103 | 101 | partial | In Progress | `dendrite agent` | Agent execution coverage follow-up remains in progress due to failing tests and statement coverage below the 100% requirement. |
| DEND-101 | 103 | partial | In Progress | `dendrite commit`, `dendrite dev commit` | Pre-commit mock rejection gate is implemented across CLI and devtool commit workflows, but relies on git grep regex patterns rather than Tree-sitter AST parsing. |
| DEND-102 | 104 | partial | In Progress | `dendrite skill` | StateToVerify constant exists in pkg/quality, but pkg/tracker lacks StatusToVerify and dendrite skill verify CLI command is missing from cmd/dendrite/main.go. |
| DEND-103 | 105 | yes | To Verify | `dendrite env prepare`, `dendrite_env_prepare` | Strict Bun and uv toolchain enforcement preventing npm/yarn/pip fallback is fully implemented in pkg/env/bootstrap. |
| DEND-201 | 106 | yes | To Verify | `dendrite query slice`, `dendrite_rag_ask`, `dendrite_query_slice` | AST graph construction, slicing queries, and RAG MCP tools are fully implemented. |
| DEND-202 | 107 | no | To Do | — | AST SymbolGraph exists with blast radius calculation, but Leiden community clustering and god-node centrality indexing are completely unimplemented. |
| DEND-203 | 108 | partial | In Progress | `dendrite ask` | Local RAG Q&A engine with lexical BM25 and paid LLM fallback exists in pkg/rag, but nomic-embed-text vector embeddings, 0.72 confidence thresholding, and CAS library store deduplication remain unimplemented. |
| DEND-302 | 110 | no | To Do | — | JetBrains IDE Plugin with embedded AI chat, Graphify markers, and Plane DAG blocker window is not implemented; only a Go MCP client bridge exists. |
| DEND-303 | 111 | no | To Do | — | VSCode extension integration for Dendrite is completely absent from the repository. |
| DEND-304 | 112 | no | To Do | — | Neovim integration plugin (dendrite.nvim) and headless Lua RPC client do not exist in the repository. |
| DEND-305 | 113 | no | To Do | — | The Rust WASM extension crate dendrite-zed does not exist in the repository; integration between Zed editor and Dendrite MCP tools remains unimplemented. |
| DEND-401 | 114 | yes | Done | `dendrite move`, `dendrite_node_migrate` | Node Migration Engine with zero-downtime shard rebalancing, live replica migration, and Merkle boundary checks is implemented in pkg/node/migrate with dendrite move CLI. |
| DEND-402 | 115 | yes | To Verify | `dendrite env service`, `dendrite env service penpot` | Penpot service stack orchestration, Docker Compose integration, and health monitoring are fully implemented. |
| DEND-403 | 116 | no | To Do | — | Topology package defines static node placements, but CLI dendrite cluster sync and cross-node token/endpoint propagation across the cluster mesh are unimplemented. |
| DEND-501 | 117 | partial | In Progress | — | BinaryFormat PE and REA analysis client exist in pkg/rea, but PE execution on macOS is blocked by ErrUntrustedOnHost guardrail, and pause/resume checkpointing, triage, and remote purge remain unimplemented. |
| DEND-601 | 118 | yes | To Verify | `dendrite plane relation link`, `dendrite_plane_relation_link` | Plane Issue DAG Relationship Link for blocked_by and blocking relations is fully implemented in CLI and MCP tools. |
| DEN-701 | 119 | yes | To Verify | `dendrite vcs guard` | VCS GitGuard enforcing branch protection against direct commits or pushes to main/master is fully implemented. |
| DEN-702 | 120 | yes | To Verify | — | Implemented in pkg/tracker/routing.go (DifficultyTier, InferenceEngine, AuditStatus, EnrichTicketClassification, SelectInferenceEngine, SortForLowBudgetQueue); pkg/tracker coverage 98.6%. (AGY verdict 'no' overridden by Claude after code check.) |
| DEN-703 | 121 | yes | To Verify | `dendrite github-sync` | Fully implemented in pkg/tracker/github and cmd/dendrite/github_sync.go with label provisioning, dependency linking, and CLI integration. |
| DEN-704 | 122 | yes | To Verify | `dendrite vcs lock`, `dendrite vcs merge` | Multi-Repo Lock Manager & Merge Controller for swarm coordination is implemented in pkg/vcs with lock guards and branch reconciliation. |
| DEN-705 | 123 | yes | To Verify | `dendrite vcs audit-changes` | VCS audit and rebase engine for inspecting branch deltas and managing safe rebase sequences is implemented. |
| DEN-701-COV-1 | 124 | partial | In Progress | — | pkg/vcs/guard.go is implemented with 93.9% statement test coverage, but requires tests for error paths and retries to close the gap to 100%. |
| DEN-702-COV-1 | 125 | partial | In Progress | — | Routing models and low-budget sorters are implemented in pkg/tracker/routing.go, but statement coverage is 79.7%, leaving the 100% coverage gap unclosed. |
| DEN-703-COV-1 | 126 | yes | To Verify | `dendrite tracker sync` | Statement coverage of pkg/tracker/github is at 100.0%, fully meeting the goal. |
| DEN-704-COV-1 | 127 | partial | In Progress | `dendrite vcs` | VCS package coverage improvement is in progress; current statement coverage is 93.9%, requiring additional tests to hit 100%. |
| DEN-705-COV-1 | 128 | yes | To Verify | — | pkg/audit/auditor.go is fully implemented with 100.0% statement test coverage without mocks, fully satisfying the acceptance criteria. |
| DEN-801 | 129 | partial | In Progress | `dendrite greeting` | Greeting styles and anti-cliché detection exist in pkg/personalcontext/greeting, but pkg/prompt, multi-turn state transitions, and persistent session recovery remain unimplemented. |
| DEN-802 | 130 | yes | To Verify | `dendrite welcome`, `dendrite_welcome_message` | Personalized Developer Greeting synthesizing project preferences and session history is fully implemented with 100% test coverage in pkg/personalcontext/welcome. |
| DEN-803 | 131 | yes | To Verify | `dendrite context memory` | Personal context memory store is fully implemented with 100% statement test coverage and zero mocks. |
| DEN-804 | 132 | yes | To Verify | `dendrite doctor`, `dendrite env doctor` | Pre-flight system diagnostics auditing host compute resources, Apple Silicon Metal GPU, developer toolchains, and service port bindings are fully implemented with 100.0% coverage in pkg/installer. |
| DEN-805 | 133 | yes | To Verify | — | Fully implemented with 100% test coverage in pkg/installer/models, verifying Ollama model pulling, OxiLLaMa probing, and LiteLLM gateway routing validation. |
| DEN-806 | 134 | yes | To Verify | `dendrite greeting`, `dendrite_get_greeting` | Multi-Style Greeting Engine featuring 10 styles and anti-cliché filters is fully implemented in pkg/personalcontext/greeting with 94.7% coverage. |
| DEN-807 | 135 | yes | To Verify | `dendrite context canvas` | Personal context canvas for visualizing agent memories and conceptual graphs is implemented. |
| DEN-810 | 138 | yes | To Verify | `dendrite ui sync`, `dendrite_ui_penpot_sync` | Penpot service synchronization, component token parsing (Svelte/React), and Plane ticket deep-linking are fully implemented with 100.0% coverage in pkg/ui/penpot. |
| DEN-809 | 139 | yes | To Verify | — | Hardware-aware concurrency limiter and heap-based priority queue are fully implemented in pkg/inference/queue.go with 85.5% test coverage. |
| DEN-811 | 140 | yes | To Verify | `dendrite completion`, `dendrite --help` | CLI Subcommand Router & Dispatcher with flag parsing, help generation, and shell completion is fully implemented in pkg/cli with 99.0% coverage. |
| DEN-813 | 142 | yes | To Verify | `dendrite env prepare` | Environment preparation bootstrap automating toolchain verification and configuration sync is fully implemented. |
| DEN-814 | 143 | yes | To Verify | `dendrite gateway update`, `dendrite gateway schedule`, `dendrite_gateway_update` | RoutingTableUpdater dynamically probes inference engines, ranks models using ModelIntelligenceScore, writes litellm_config.yaml, runs a 24h background daemon, and provides CLI and MCP triggers. |
| DEN-815 | 144 | yes | To Verify | — | Inference endpoints, model lists, and cascade sequence are fully decoupled from hardcoded constants via environment variables with safe defaults in pkg/inference/manager.go. |
| DEN-816 | 145 | partial | In Progress | `dendrite agent triage`, `dendrite_agent_triage` | Hardcoded model fallback in pkg/agent/triage is only partially addressed; oxillama-rust-qwen7b remains in triage.go and package tests fail. |
| DEN-817 | 146 | yes | To Verify | `dendrite devtool commit-msg` | Commit message generation tool using local model HTTP adapter with fallback is fully implemented. |
| DEN-820 | 149 | yes | To Verify | `dendrite dev gateway [scenario]`, `dendrite_gateway_render` | DefaultLiteLLMConfig and MultiEngineLiteLLMConfig dynamically route local-qwen models to healthy engines (OxiLLaMa/Ollama) and rank fallback routes by ModelIntelligenceScore. |
| DEN-822 | 151 | yes | To Verify | `dendrite --help`, `dendrite version`, `dendrite doctor` | Root CLI in cmd/dendrite/main.go has been modernized using declarative pkg/cli.Registry with categorized help, alias resolution, and 99.0% statement coverage. |
