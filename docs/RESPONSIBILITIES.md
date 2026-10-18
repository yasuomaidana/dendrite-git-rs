# `dendrite-git-rs` — Repository Responsibilities & Boundary Definition

> **Plane Project**: `DGIT` (workspace `dendrite-ws`, `http://localhost:8090`)  
> **Extraction Umbrella**: `DEN-973` (first child: `DGIT-1`)  
> **Target Rust Crate**: `dendrite-git` (crate binary: `dendrite-git`, library: `dendrite_git`, edition `2021`)

---

## 1. Purpose

`dendrite-git-rs` (Rust crate `dendrite-git`, shipping the `dendrite-git` binary) is the dedicated high-performance Git object model, hunk dependency graph, semantic clusterer, and `--local-review` stacked branch engine for the Dendrite ecosystem (Pillar 8 and Pillar 9 of the 2026-10 Quality Audit & Architecture Execution Plan). Powered by `gitoxide` (`gix`), it replaces slow shell-out Git invocations and line heuristics with high-throughput in-process Git revision graph traversal, AST-aware hunk extraction, strongly connected component (SCC) dependency analysis, and verified atomic commit synthesis. It executes as a JSON-RPC 2.0 stdio sidecar consumed by the Go control plane (`dendrite-control`), falling back to in-process Go Git implementations when absent.

---

## 2. Owns vs. Does NOT Own

### Owns (`dendrite-git-rs` / `dendrite-git`)

- **Gitoxide (`gix`) Object Model & Fast Revision Traversal (Planned)**:
  - Native, pure-Rust repository inspection, commit graph traversal, tree-to-tree and worktree diffing, and blob access using `gitoxide` (`gix`), replacing shell-outs to `git diff`, `git log`, `git rev-list`, and `git show`.
  - Fast divergence metrics computation: ahead/behind revision counting against remote tracking branches without spawning external subprocesses.
- **Hunk Extraction & Symbol-Dependency Graph (`DEN-932`, Planned)**:
  - Extraction of unified diff hunks (`git diff -U0`) across staged changes or working trees, annotating each hunk with file paths, line ranges, and enclosing syntax symbols (via symbol definitions from `dendrite-core-rs` or `ASTProvider`).
  - Hunk dependency DAG construction: detecting definition-use relationships (Hunk B uses a symbol defined or modified by Hunk A $\to$ directed edge A $\to$ B), grouping edits within the same enclosing symbol, and computing Strongly Connected Components (SCCs).
- **Semantic Hunk Clustering & Agglomerative Merging (`DEN-933`, Planned)**:
  - Agglomerative clustering of SCCs based on structural dependency edges, semantic affinity (package/directory scope, symbol documentation embeddings), and LLM intent mappings.
  - Production-to-test affinity linking (`foo.go` $\leftrightarrow$ `foo_test.go`, Rust `#[cfg(test)] mod`).
- **Topological Plan Sorting & Per-Commit Build Validation (`DEN-933`, Planned)**:
  - Topological ordering of commit clusters to guarantee every intermediate commit builds and passes linting independently.
  - Temporary isolated worktree mounting (`git worktree add`) to apply cumulative patches and verify `go build ./...` / `cargo check` before committing.
  - Commit subject quality gate: enforcement of Conventional Commits format, rejection of deny-listed/canned messages (`wip`, `update files`, `third pass`), and requirement of meaningful symbol/scope attribution.
- **Speculative Stacked Branches & `--local-review` Engine (`DEN-934`, `FEAT-070`, Planned)**:
  - Automated construction and management of stacked review branches (`review/<ticket>/01-scaffold`, `review/<ticket>/02-logic`, `review/<ticket>/03-tests` or `local/<module-slug>`).
  - Git notes telemetry management: attaching structured JSON execution payloads (`tier`, `model`, `tokens`, `test_exit`, `coverage`, `assertion_verdict`) to `refs/notes/dendrite/telemetry`.
  - Speculative branch queue management (replacing `.dendrite/audit_queue.json` with native git references).
- **Review Landing & Rebase Cascade Pipeline (`DEN-934`, Planned)**:
  - Safe rebase and landing of audited stacks onto target branches (`develop`), verifying `VERIFIED_COMPLETE` assertion status before merge/squash and cascading rebases onto downstream stacked branches.
- **Git Worktree Isolation Harness (`DEN-064`, Planned)**:
  - Low-level Git worktree lifecycle management (`.dendrite/worktrees/<story-id>`), differential branch switching, and RAII cleanup for parallel speculative stories.
- **Sliced Repository Commit Drift Analysis (`DEN-065`, Planned)**:
  - Analyzing upstream commit logs for sliced downstream repositories, filtering out changes touching un-retained modules based on AST provenance manifests, and calculating bug-fix port candidates.

### Does NOT Own (Explicit Exclusions & Owning Repositories)

| Capability / Domain | Owning Repository | Module / Crate | Rationale |
| :--- | :--- | :--- | :--- |
| CLI command parsing (`cmd/dendrite`), environment replication (`pkg/env`), LiteLLM token governor & gateway (`pkg/gateway`, `pkg/scheduler`), Plane v2 ticket transitions & `GatedTracker` (`pkg/tracker`), agent runner & supervision (`pkg/agent`), web UI & REST server (`pkg/ui`, `pkg/server`), multi-repo pool orchestration (`pkg/pool`) | **`dendrite-control`** (currently `/Users/yasuomaidana/GoLandProjects/Dendrite`) | `dendrite.dev/control` (currently `module Dendrite`) | Core control plane and orchestration state machine; `dendrite-git-rs` is a compute sidecar that executes Git graph and review tasks on behalf of the control plane. |
| Tree-sitter polyglot syntax parsing, code folding (`fold_signatures`), cut-point detection (`last_complete_node`), cyclomatic/cognitive complexity analysis, BLAKE3 chunk hashing, model tokenizer token counting (`count_tokens`), and CFG slicing | **`dendrite-core-rs`** | `dendrite-core` (binary `dendrite-core`) | CPU-intensive deterministic AST/CFG and token analysis executed via Rust JSON-RPC 2.0 stdio sidecar. |
| Hybrid BM25 + vector search, Content-Addressable Storage (CAS), memory synchronization (`local-only`, `edge`, `push`), WAL journal replay, and sqlite/vector index management | **`dendrite-rag-rs`** | `dendrite-rag` (binary `dendrite-rag`) | High-throughput indexing, vector search, and segmented storage executed via Rust JSON-RPC 2.0 stdio sidecar. |
| MCP JSON-RPC 2.0 server lifecycle, tool schema registration (`dendrite_commit`, `dendrite_vcs_guard`, `dendrite_audit_changes`, `dendrite_remote_sync`), JetBrains IDE reverse bridge (`:63342/api/mcp`), and upstream tool proxying | **`dendrite-mcp-hub`** | `dendrite.dev/mcphub` (Go module) | Dedicated Model Context Protocol server exposing tool surfaces to IDEs and AI coding agents. |
| Foundational vector embeddings generation, graph matching, and text chunking data structures (`features = ["core", "matcher"]`) | **`embedag`** (`../../RustRoverProjects/embedag/crates/embedag`) | `embedag` | Shared Rust library crate consumed by `dendrite-core-rs` and `dendrite-rag-rs`. |

---

## 3. Interfaces

### What `dendrite-git-rs` Exposes

1. **Sidecar Binary JSON-RPC 2.0 over stdio (`dendrite-git`)**:
   - Binary executable: `dendrite-git` (planned)
   - Protocol: JSON-RPC 2.0 over standard input / standard output (`stdio`).
   - Schema: Formal JSON schemas located in `proto/*.json` (e.g., `proto/git_rpc_v1.json`), versioned strictly according to Semantic Versioning (SemVer).
   - Planned RPC methods:
     - `health.check`: Protocol version handshake and engine readiness probe.
     - `git.hunk_graph`: Extracts diff hunks from staged/unstaged changes, associates enclosing symbols, and returns dependency DAG with Strongly Connected Components (SCCs).
     - `git.cluster_hunks`: Clusters hunks into cohesive commit units based on affinity and structural dependencies.
     - `git.validate_plan`: Topologically sorts commit clusters, verifies exact-once hunk coverage, and tests per-commit compilation in temporary worktrees.
     - `git.review_stack.create`: Spawns and configures stacked review branches (`review/<ticket>/<stage>` or `local/<module>`).
     - `git.review_stack.telemetry`: Appends or reads structured execution telemetry to `refs/notes/dendrite/telemetry`.
     - `git.review_stack.land`: Safely rebases or squash-merges verified review stacks onto target branches (`develop`) and cascades downstream rebases.
     - `git.divergence.check`: Computes ahead/behind commit counts and incoming remote commit IDs.
     - `git.drift.analyze`: Analyzes upstream commit logs, filters diffs by retained AST slices, and generates bug-fix port candidates.

### Who Consumes `dendrite-git-rs`

- **`dendrite-control` Go Control Plane**:
  - Consumed by `dendrite-control` via `sidecar.Client` (in `pkg/commit` and `pkg/vcs`) over stdio JSON-RPC 2.0.
  - Health Handshake: Upon startup, `sidecar.Client` performs a `health.check` and protocol version verification.
  - **Go Fallback Behavior**: If the `dendrite-git` binary is missing from `$PATH` or `.dendrite/bin/`, or fails the version negotiation, `dendrite-control` logs a warning and falls back to the in-process Go implementations (`exec.Command("git", ...)`, `pkg/devtool/commit.go`, `pkg/vcs/guard.go`, `pkg/vcs/remote_sync.go`, `pkg/vcs/orchestrator.go`).

---

## 4. Code Being Extracted / Replaced & Go Fallback Behavior

### Verified Existing Go Packages and Files in `Dendrite`

Every file listed below was verified by inspecting `/Users/yasuomaidana/GoLandProjects/Dendrite`:

1. **`pkg/devtool/commit.go` (909 LOC) & `pkg/devtool/commit_test.go` (1877 LOC)**:
   - Existing: Staged/unstaged file resolution (`git add -A`), single-message synthesis via Ollama/OxiLLaMa/LiteLLM, regex-based ticket formatting, and `git commit` via `os/exec`. Lacks hunk clustering, multi-commit planning, and review branches.
   - Replaced by: Rust-based `gix` hunk extractor, symbol-dependency DAG builder, and smart commit planner (planned in `dendrite-git-rs`), while Go's `pkg/commit` handles LLM prompt synthesis and sidecar communication.
2. **`pkg/vcs/guard.go` (238 LOC)**:
   - Existing: Branch policy enforcement (`PreFlightBranchCheck`, `EnsureAndCheckoutDevelop`, evacuation of `main`/`master` to `develop`, atomic `CommitAndPush` with retry on divergence), shelling out to `git`.
   - Replaced by: Pure-Rust `gix` branch inspection, revision validation, and atomic commit-push operations in `dendrite-git-rs`.
3. **`pkg/vcs/orchestrator.go` (272 LOC)**:
   - Existing: `StackedBranchOrchestrator`, `AuditQueueItem`, managing `local/<module>` branches and recording state in `.dendrite/audit_queue.json`.
   - Replaced by: `--local-review` stacked branch engine (`review/<ticket>/01..03`) and `git notes` telemetry in `dendrite-git-rs`.
4. **`pkg/vcs/remote_sync.go` (144 LOC)**:
   - Existing: `RemoteSyncWatcher`, `FetchAndDetect`, shelling out to `git fetch`, `git rev-list --left-right --count`, parsing ticket IDs from commit logs.
   - Replaced by: Native `gix` commit graph traversal and revision counting in `dendrite-git-rs`.
5. **`cmd/dendrite/commit.go` (173 LOC) & `cmd/dendrite/vcs.go` (71 LOC)**:
   - Existing: CLI entrypoints in Go that parse flags and invoke `devtool.Commit` and `vcs.*`.
   - Migration: Re-architected in `dendrite-control` as Cobra commands (`cmd/dendrite/cmd/{commit,vcs}.go` per P10), delegating underlying Git graph and review operations to `dendrite-git-rs`.

### Fallback Behavior in `dendrite-control`

The Go control plane implements a resilient sidecar client (`sidecar.Client` per `DEN-970`):
- When `dendrite-git` is available and passes the health handshake, high-throughput operations (diff hunk analysis, dependency graph construction, commit planning, and stacked review landing) run in Rust.
- If the binary is absent or returns an error, Dendrite emits a warning and routes operations through legacy in-process Go implementations using `os/exec.Command("git", ...)` and heuristic staging.

---

## 5. Dependencies on Sibling Repositories

- **`dendrite-control` (`~/GoLandProjects/dendrite-control`, Go module `dendrite.dev/control`)**: Consumes the `dendrite-git` binary as a stdio JSON-RPC 2.0 sidecar.
- **`dendrite-core-rs` (`~/GoLandProjects/dendrite-core-rs`, Rust crate `dendrite-core`)**: Sibling crate providing Tree-sitter symbol boundaries, node ranges, and BLAKE3 digests. Configured via Cargo path dependency (`dendrite-core = { path = "../dendrite-core-rs" }`) or synchronized via `dendrite pool resolve` `[patch]` overrides.
- **`embedag` (`../../RustRoverProjects/embedag/crates/embedag`)**: Consumed via path dependency (`default-features = false, features = ["core", "matcher"]`) for graph algorithms and embedding distance calculations during hunk clustering.
- **Multi-Repo Synchronization**: Synchronized by `dendrite pool` (`pool add`, `pool resolve`, `pool switch`, `pool split`), generating `[patch]` sections and JetBrains `.idea/modules.xml` scopes for GoLand and RustRover.

---

## 6. Ticket Map

All tickets relocated to Plane project **DGIT** (workspace `dendrite-ws`, `http://localhost:8090`) on 2026-10-18, under extraction umbrella **DEN-973** (first child: **DGIT-1**):

### A. Smart Commit & Hunk Decomposition
| Ticket | Title & Target Domain | Action | Status |
| :--- | :--- | :--- | :--- |
| **DEN-932** | Hunk extractor + symbol-dependency graph (`dendrite-git-rs`) | move | Backlog |
| **DEN-933** | Clusterer + LLM plan + validators (exactly-once, topo order, per-commit build) (`dendrite-git-rs`) | move | Backlog |
| **DEN-098** | Autonomous Git Commit Engine with Local Model Message Delegation & Zero-Cost Synthesis | split | To Verify |

### B. Review Branches & Telemetry
| Ticket | Title & Target Domain | Action | Status |
| :--- | :--- | :--- | :--- |
| **DEN-934** | `--local-review` stacked branches + git notes telemetry + review audit/land (`dendrite-git-rs`) | split | Backlog |

### C. Speculative Worktrees & Upstream Drift
| Ticket | Title & Target Domain | Action | Status |
| :--- | :--- | :--- | :--- |
| **DEN-064** | Speculative Git Worktree Branching & Differential Architecture Benchmark Harness (`pkg/story/fork`, `pkg/perf`) | split | To Verify |
| **DEN-065** | Sliced Upstream Commit Drift Analyzer & Bug Fix Porting Generator (`pkg/story/drift`, `pkg/query`) | split | To Do |

### D. Extraction Umbrella & Dogfood
| Ticket | Title & Target Domain | Action | Status |
| :--- | :--- | :--- | :--- |
| **DEN-973** | `dendrite-git-rs`: gix hunk graph + clusterer (Milestone M7 / Pillar 9) | umbrella (stays in DEN) | Backlog |
| **DGIT-1** | Initial crate setup, stdio JSON-RPC 2.0 server skeleton, and `proto/git_rpc_v1.json` schema | new | Backlog |

### Plane keys (synced with Plane on 2026-10-18)

Each relocated ticket has its own key in `DGIT`. The DEN original either is archived with the `Relocated` state (**move**) or stays open in DEN as the Go-side umbrella with the `relocated-split` label (**split**).

| DGIT key | Origin in DEN | Action | DGIT state |
| :--- | :--- | :--- | :--- |
| **DGIT-1** | umbrella DEN-973 | first child | Backlog |
| **DGIT-2** | DEN-934 (DEN #171) | split | Backlog |
| **DGIT-3** | DEN-933 (DEN #170) | move | Backlog |
| **DGIT-4** | DEN-932 (DEN #169) | move | Backlog |
| **DGIT-5** | DEN-098 (DEN #98) | split | To Verify |
| **DGIT-6** | DEN-065 (DEN #65) | split | To Do |
| **DGIT-7** | DEN-064 (DEN #64) | split | To Verify |

---

## 7. Imported Docs

The following 15 documents in [`docs/imports.json`](imports.json) were verified in `/Users/yasuomaidana/GoLandProjects/Dendrite` and selected for import:

1. `docs/architecture/2026-10-quality-audit-execution-plan.md` — Authoritative specification for Pillar 8 (Smart Commit & `--local-review`), Pillar 9 (5-repo decomposition & `dendrite pool`), and extraction milestones.
2. `docs/features/FEAT-098-autonomous-git-commit-engine.md` — Specification for autonomous commit engine, staging management, local model delegation, and conventional commit formatting.
3. `docs/features/FEAT-070-speculative-stacked-branches-and-audit-pipeline.md` — Specification for speculative stacked branches (`local/*`), git guardrails, degraded budget mode, and vendor LLM audit/rebase pipeline.
4. `docs/features/FEAT-032-polyglot-speculative-drift-workflows.md` — Specification for speculative story branching in isolated git worktrees and sliced upstream commit drift reconciliation.
5. `docs/features/FEAT-035-multiprovider-agent-governance.md` — Specification for 5-tier hierarchical git governance and branch interceptor policy matrix.
6. `docs/features/FEAT-041-agent-security-quality-resilience-guardrails.md` — Specification for git commit security guardrails, pre-commit secret detection, and worktree isolation/rollback.
7. `docs/cli/commit.md` — CLI reference documentation for `dendrite commit`, including `--ticket`, `--dry-run`, and `--no-push` flags.
8. `docs/cli/vcs.md` — CLI reference documentation for `dendrite vcs`, covering branch policy enforcement, evacuation, and remote synchronization.
9. `docs/cli/audit-changes.md` — CLI reference documentation for `dendrite audit-changes`, topological review, and rebase engine for stacked branches.
10. `docs/cli/story.md` — CLI reference documentation for `dendrite story` subcommands (`fork`, `resolve`) using speculative git worktrees.
11. `docs/cli/sync-remote.md` — CLI reference documentation for `dendrite sync-remote`, divergence detection, and commit reconciliation.
12. `docs/mcp/dendrite_commit.md` — Native MCP tool specification and JSON-RPC schema for agent-invoked git commits.
13. `docs/mcp/dendrite_vcs_guard.md` — Native MCP tool specification for Git branch guardrails and execution mode enforcement.
14. `docs/mcp/dendrite_audit_changes.md` — Native MCP tool specification for topological vendor LLM audits and branch rebasing.
15. `docs/mcp/dendrite_remote_sync.md` — Native MCP tool specification for remote git commit synchronization and ticket reconciliation.

---

## 8. Open Questions & Boundary Conflicts Noticed

1. **AST Symbol Boundary Ingestion (Direct Crate Dependency vs. RPC Coupling)**:
   - Does `dendrite-git-rs` depend on `dendrite-core-rs` directly in Cargo (`dendrite-core = { path = "../dendrite-core-rs" }`) to resolve AST symbol boundaries for diff hunks, or does `dendrite-control` invoke `dendrite-core-rs` first and pass the symbol slice spans into `dendrite-git-rs` via the `git.hunk_graph` RPC request? A direct Cargo dependency makes `dendrite-git-rs` faster and self-contained for symbol analysis, but couples the two Rust crates.
2. **Semantic Hunk Affinity Embeddings Generation**:
   - Pillar 8 §2.3 specifies that hunk clustering affinity combines package scope, LLM intent labeling, and embedding similarity of hunk text + enclosing symbol docs (using local TEI/`nomic`). Does `dendrite-git-rs` call `embedag` / local TEI directly in Rust, or does the Go control plane compute embeddings and pass an affinity matrix to `dendrite-git-rs`?
3. **Stacked Branch Naming & Metadata Migration (`local/*` vs. `review/*`)**:
   - FEAT-070 implemented speculative stacked branches under `local/<module-slug>` with status tracked in `.dendrite/audit_queue.json`. Pillar 8 (§2.3 / `DEN-934`) introduces `review/<ticket>/01-scaffold`, `02-logic`, `03-tests` with metadata stored in `git notes --ref=dendrite/telemetry`. The extraction must define whether `dendrite-git-rs` supports both naming schemes or transitions all stacked branch workflows to `review/<ticket>/<stage>` with `git notes`.
4. **Gitoxide (`gix`) Feature Completeness & Shell-out Boundary**:
   - While `gitoxide` (`gix`) provides fast object parsing, revision walking, and tree diffing, certain operations (e.g. authenticated network push/fetch with credential helpers, interactive rebase, and worktree manipulation) may have limited or evolving API stability in pure Rust. The boundary must clearly define whether `dendrite-git-rs` falls back to `std::process::Command` for specific transport/rebase operations or delegates them back to `dendrite-control`.
5. **Worktree Directory Lifecycle Ownership**:
   - Speculative story branching (`dendrite story fork` in FEAT-032 / `DEN-064`) isolates variants in `.dendrite/worktrees/`. Does `dendrite-git-rs` own the full directory lifecycle, locks, and RAII cleanup, or does `dendrite-control` manage the directories while `dendrite-git-rs` only handles git worktree add/prune invocations and diff analysis?

### Boundary note: graph algorithms come from embedag (added 2026-10-18)
The control plane rule is that graph and DAG algorithms live in `embedag`, not in sub-project crates.
- `dendrite-git-rs` owns the **domain**: hunk extraction, the definition–use edge semantics, the clustering policy, LLM planning, and per-commit validation.
- The **algorithms** come from `embedag`:
  - topological ordering of clusters (`embedag::dag::toposort` / `levels`)
  - SCC detection and condensation
- Hunk graphs can be cyclic, but `MemoryDag` is acyclic-only. `embedag` therefore needs a public SCC-condensation API (cyclic graph in, condensed DAG plus component membership out). This is tracked as embedag `DEV_STATUS.md` ticket 9.5.1.
- Until that API exists, do not implement a local Tarjan/Kosaraju pass here.
