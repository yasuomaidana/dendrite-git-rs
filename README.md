# dendrite-git-rs

Rust gitoxide hunk dependency graph, semantic clusterer and --local-review branch engine (Pillar 8/9).

Provides pure-Rust Git object and revision graph traversal via `gitoxide` (`gix`), AST-aware hunk extraction, and strongly connected component (SCC) dependency analysis. Clusters diff hunks into topologically sorted, verified atomic commits and manages speculative stacked review branches with Git notes telemetry. Operates as a JSON-RPC 2.0 stdio sidecar for the Dendrite Go control plane (`dendrite-control`), falling back to in-process Go implementations when absent.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the technical specification and [docs/RESPONSIBILITIES.md](docs/RESPONSIBILITIES.md) for the ownership boundary and ticket map.

## CLI Usage

The `dendrite-git` binary communicates over stdio using line-delimited JSON-RPC 2.0.

```bash
# Print version
dendrite-git --version

# Run JSON-RPC sidecar over stdin/stdout
dendrite-git
```

## JSON-RPC 2.0 Methods

### `health`
Probes engine readiness and protocol version.

Request:
```json
{"jsonrpc": "2.0", "id": 1, "method": "health", "params": {}}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "name": "dendrite-git",
    "version": "0.1.0",
    "protocol": "1.0.0"
  }
}
```

### `git.hunks`
Extracts diff hunks across staged changes (`mode: "staged"`) or working tree (`mode: "all"`).

Request:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "git.hunks",
  "params": {
    "repo": "/path/to/git/repository",
    "mode": "staged"
  }
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "hunks": [
      {
        "id": 1,
        "file": "src/lib.rs",
        "old_start": 1,
        "old_lines": 1,
        "new_start": 1,
        "new_lines": 2,
        "added": ["pub mod hunk;\n", "pub mod repo;\n"],
        "removed": ["// stub\n"],
        "kind": "modified"
      }
    ]
  }
}
```

Error Responses:
- Bad parameters / unknown mode: code `-32602`.
- Not a repository: code `-32000`, `data.kind: "not_a_repository"`.
- Git error: code `-32000`, `data.kind: "git_error"`.

## Rust Library (`dendrite_git`)

```rust,no_run
use std::path::Path;
use dendrite_git::{extract, GitError, Mode};

fn main() -> Result<(), GitError> {
    // In-process Git extraction without spawning the git CLI:
    let staged = extract(Path::new("."), Mode::Staged)?;
    println!("{} staged hunks", staged.len());
    Ok(())
}
```

```rust
use dendrite_git::diff_file;

// Pure line diffing without Git:
let old = b"line 1\nline 2\n";
let new = b"line 1\nline 2 modified\n";
let hunks = diff_file("test.txt", Some(old), Some(new));
assert_eq!(hunks.len(), 1);
```

## Testing & Quality Gates

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
dendrite allostasis gate
```

## Database access

Every database in this project goes through [Diesel](https://diesel.rs); this repository has none today.
`scripts/check-db-access.sh` (job `db-access (diesel only)`, on every pull request and on pushes to develop and
main) fails when a `Cargo.toml` declares `sqlx`, `rusqlite`, `sea-orm`, `mysql`, `mysql_async`, `postgres`,
`tokio-postgres` or `libsql` without also declaring a `diesel` crate (`diesel-async` and `diesel-libsql` use those
drivers as backends), when a source file uses `rusqlite::`, `sqlx::` or `sea_orm::`, or when `sql_query` is called
outside a module named `raw`. An exception goes into `.github/db-access-allow.txt` as
`manifest:<path/to/Cargo.toml> <reason>` or `source:<path prefix> <reason>`; an entry without a reason is an error.
`scripts/tests/check-db-access.test.sh` tests the guard with passing and failing fixtures.

## Release references

A final version refers to its sibling projects by release tag, or by their `main` branch, never by `develop`, a
`rev` or a bare git dependency. `scripts/check-release-refs.sh` (job `release refs`, on every pull request and on
pushes to main) fails when a `Cargo.toml` git dependency has no `tag = "vX.Y.Z"` and is not on `branch = "main"`,
when it names a `rev` or another branch, when a workflow checks out a `develop` ref, or when a Markdown file links
to a `develop` branch. `scripts/tests/check-release-refs.test.sh` tests the guard with passing and failing fixtures.
