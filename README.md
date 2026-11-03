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

```rust
use std::path::Path;
use dendrite_git::{diff_file, extract, Mode};

// In-process Git extraction without spawning git CLI:
let hunks = extract(Path::new("."), Mode::Staged)?;

// Pure line diffing without Git:
let old = b"line 1\nline 2\n";
let new = b"line 1\nline 2 modified\n";
let hunks = diff_file("test.txt", Some(old), Some(new));
```

## Testing & Quality Gates

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
dendrite allostasis gate
```
