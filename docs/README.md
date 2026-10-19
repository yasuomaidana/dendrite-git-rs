# `dendrite-git-rs` Documentation Index

This directory contains architectural boundary documentation and imported specifications for **`dendrite-git-rs`** (Rust crate `dendrite-git`), the pure-Rust Git object model, hunk dependency graph, semantic clusterer, and `--local-review` stacked branch engine in the Dendrite 5-repository decomposition (Pillar 8 / Pillar 9).

## Repository Documentation

- [Repository Responsibilities & Boundary Definition](RESPONSIBILITIES.md) (`docs/RESPONSIBILITIES.md`) — Authoritative scope, ownership boundaries, exposed JSON-RPC 2.0 interfaces, extracted Go packages, sibling repository dependencies, Plane ticket map (`DGIT`), and open architectural questions.
- [Imported Documentation Manifest](imports.json) (`docs/imports.json`) — Machine-readable list of feature specifications, CLI references, and MCP tool definitions imported from the control-plane repository (`Dendrite`).

## Imported from Dendrite

<!-- Generated from imports.json by scripts/sync_subproject_docs.py (control-plane repo). The list is regenerated; descriptions are kept and must describe this repository. -->

Specifications that define `dendrite-git-rs`'s responsibilities, listed in [`imports.json`](imports.json) and copied into `docs/imported/`. The first line of each copy records where it was copied from. The copies are regenerated, so do not edit them here. Each entry below says what the spec defines for this repository.

### Architecture

- [`2026-10-quality-audit-execution-plan`](imported/architecture/2026-10-quality-audit-execution-plan.md) — Boundary of dendrite-git-rs in the five-repository split: the gitoxide hunk graph, semantic commit clustering and `--local-review` engine it owns.

### Feature specifications

- [`FEAT-098-autonomous-git-commit-engine`](imported/features/FEAT-098-autonomous-git-commit-engine.md) — Commit-engine behaviour dendrite-git-rs implements: grouping staged hunks into semantic commits and ordering them.
- [`FEAT-070-speculative-stacked-branches-and-audit-pipeline`](imported/features/FEAT-070-speculative-stacked-branches-and-audit-pipeline.md) — Speculative stacked branches (`local/*`) and the rebase cascade that dendrite-git-rs's `--local-review` engine performs.
- [`FEAT-032-polyglot-speculative-drift-workflows`](imported/features/FEAT-032-polyglot-speculative-drift-workflows.md) — Story-variant worktrees and upstream drift reconciliation provided by dendrite-git-rs's worktree harness.
- [`FEAT-035-multiprovider-agent-governance`](imported/features/FEAT-035-multiprovider-agent-governance.md) — Hierarchical commit governance rules dendrite-git-rs applies when it clusters and validates commits.
- [`FEAT-041-agent-security-quality-resilience-guardrails`](imported/features/FEAT-041-agent-security-quality-resilience-guardrails.md) — Git safety requirements dendrite-git-rs must uphold: secret checks before commit, worktree isolation and rollback.

### CLI reference

- [`commit`](imported/cli/commit.md) — Commit command whose hunk clustering and commit plan come from dendrite-git-rs.
- [`vcs`](imported/cli/vcs.md) — Branch guard and status checks backed by dendrite-git-rs's revision traversal.
- [`audit-changes`](imported/cli/audit-changes.md) — Stacked-branch audit and rebase flow executed by dendrite-git-rs's rebase cascade.
- [`story`](imported/cli/story.md) — Story fork and resolve commands built on dendrite-git-rs's worktree isolation harness.
- [`sync-remote`](imported/cli/sync-remote.md) — Remote commit detection performed by dendrite-git-rs's gitoxide traversal.

### MCP tool reference

- [`dendrite_commit`](imported/mcp/dendrite_commit.md) — Tool contract for autonomous commits served by dendrite-git-rs's commit planner.
- [`dendrite_vcs_guard`](imported/mcp/dendrite_vcs_guard.md) — Tool contract for branch guardrails backed by dendrite-git-rs.
- [`dendrite_audit_changes`](imported/mcp/dendrite_audit_changes.md) — Tool contract for the topological audit and rebase of stacked branches in dendrite-git-rs.
- [`dendrite_remote_sync`](imported/mcp/dendrite_remote_sync.md) — Tool contract for remote synchronisation using dendrite-git-rs's revision walks.
- [`dendrite_story_abort`](imported/mcp/dendrite_story_abort.md) — Tool contract for aborting a story: dendrite-git-rs removes the variant worktrees and branches.
- [`dendrite_story_fork`](imported/mcp/dendrite_story_fork.md) — Tool contract for forking a story into isolated worktrees created by dendrite-git-rs.
- [`dendrite_story_resolve`](imported/mcp/dendrite_story_resolve.md) — Tool contract for landing the winning variant and purging the others, done by dendrite-git-rs.

