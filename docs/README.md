# `dendrite-git-rs` Documentation Index

This directory contains architectural boundary documentation and imported specifications for **`dendrite-git-rs`** (Rust crate `dendrite-git`), the pure-Rust Git object model, hunk dependency graph, semantic clusterer, and `--local-review` stacked branch engine in the Dendrite 5-repository decomposition (Pillar 8 / Pillar 9).

## Repository Documentation

- [Repository Responsibilities & Boundary Definition](RESPONSIBILITIES.md) (`docs/RESPONSIBILITIES.md`) — Authoritative scope, ownership boundaries, exposed JSON-RPC 2.0 interfaces, extracted Go packages, sibling repository dependencies, Plane ticket map (`DGIT`), and open architectural questions.
- [Imported Documentation Manifest](imports.json) (`docs/imports.json`) — Machine-readable list of feature specifications, CLI references, and MCP tool definitions imported from the control-plane repository (`Dendrite`).

## Imported from Dendrite

The following specifications and guides are imported from `/Users/yasuomaidana/GoLandProjects/Dendrite` as listed in [`docs/imports.json`](imports.json):

### Architecture
- [docs/imported/architecture/2026-10-quality-audit-execution-plan.md](docs/imported/architecture/2026-10-quality-audit-execution-plan.md) — Pillar 8 (Smart Commit & `--local-review`) and Pillar 9 (Five Sub-Project Decomposition & Multi-Directory Sync Proof).

### Feature Specifications
- [docs/imported/features/FEAT-098-autonomous-git-commit-engine.md](docs/imported/features/FEAT-098-autonomous-git-commit-engine.md) — Autonomous Git commit engine with local model message delegation and zero-cost synthesis (`DEN-098`).
- [docs/imported/features/FEAT-070-speculative-stacked-branches-and-audit-pipeline.md](docs/imported/features/FEAT-070-speculative-stacked-branches-and-audit-pipeline.md) — Speculative stacked branches (`local/*`), Git guardrails, token-aware degraded mode, and vendor LLM audit/rebase pipeline.
- [docs/imported/features/FEAT-032-polyglot-speculative-drift-workflows.md](docs/imported/features/FEAT-032-polyglot-speculative-drift-workflows.md) — Speculative story branching in isolated Git worktrees and sliced upstream commit drift reconciliation.
- [docs/imported/features/FEAT-035-multiprovider-agent-governance.md](docs/imported/features/FEAT-035-multiprovider-agent-governance.md) — Multi-provider AI runtime and 5-tier semantic hierarchical Git version governance.
- [docs/imported/features/FEAT-041-agent-security-quality-resilience-guardrails.md](docs/imported/features/FEAT-041-agent-security-quality-resilience-guardrails.md) — Autonomous agent Git commit security guardrails, pre-commit secret detection, and worktree isolation/rollback.

### CLI References
- [docs/imported/cli/commit.md](docs/imported/cli/commit.md) — CLI reference for `dendrite commit` (`--ticket`, `--dry-run`, `--no-push`).
- [docs/imported/cli/vcs.md](docs/imported/cli/vcs.md) — CLI reference for `dendrite vcs` (`check`, `guard`, `status`, `sync-remote`, `evacuate`).
- [docs/imported/cli/audit-changes.md](docs/imported/cli/audit-changes.md) — CLI reference for `dendrite audit-changes` (topological vendor audit and rebase engine for stacked branches).
- [docs/imported/cli/story.md](docs/imported/cli/story.md) — CLI reference for `dendrite story` (`fork`, `resolve`) utilizing speculative Git worktrees.
- [docs/imported/cli/sync-remote.md](docs/imported/cli/sync-remote.md) — CLI reference for `dendrite sync-remote` (remote commit detection and ticket reconciliation).

### MCP Tool References
- [docs/imported/mcp/dendrite_commit.md](docs/imported/mcp/dendrite_commit.md) — Native MCP tool specification for autonomous Git commits and ticket provenance.
- [docs/imported/mcp/dendrite_vcs_guard.md](docs/imported/mcp/dendrite_vcs_guard.md) — Native MCP tool specification for Git branch guardrails and policy enforcement.
- [docs/imported/mcp/dendrite_audit_changes.md](docs/imported/mcp/dendrite_audit_changes.md) — Native MCP tool specification for topological vendor LLM audits and branch rebasing.
- [docs/imported/mcp/dendrite_remote_sync.md](docs/imported/mcp/dendrite_remote_sync.md) — Native MCP tool specification for remote Git synchronization and ticket reconciliation.
