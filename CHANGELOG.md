# Changelog

All notable changes to `dendrite-git-rs` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Pure zero-context Myers line diff hunk extraction in `hunk.rs` via `imara-diff`.
- Support for `HunkKind` variants: `Modified`, `Added`, `Deleted`, `Binary`, and `Renamed`.
- In-process Git repository inspection with `gitoxide` (`gix`) in `repo.rs` supporting `Mode::Staged` and `Mode::All`.
- Exact-content rename detection for identical blob object IDs.
- Unborn repository HEAD handling against empty trees.
- JSON-RPC 2.0 stdio server supporting `health` and `git.hunks` methods in `rpc.rs` and `main.rs`.
- Comprehensive `rstest` test suite covering hunk extraction, git repo scenarios, and RPC interactions.
- Complexity gate test in `tests/self_complexity.rs` ensuring cyclomatic complexity <= 10 and cognitive complexity <= 15.
- Integration CLI tests in `tests/cli.rs`.
- CI workflow in `.github/workflows/ci.yml`, and `.github/workflows/complexity.yml` cross-checking function complexity with clippy `cognitive_complexity` (blocking) and `lizard` (report only) (DGIT-10).
- `self_complexity` test uses dendrite-core's complexity implementation, so there is a single one in the ecosystem.
