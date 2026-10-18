# dendrite-git-rs

Rust gitoxide hunk dependency graph, semantic clusterer and --local-review branch engine (Pillar 8/9)

Provides pure-Rust Git object and revision graph traversal via `gitoxide` (`gix`), AST-aware hunk extraction, and strongly connected component (SCC) dependency analysis. Clusters diff hunks into topologically sorted, verified atomic commits and manages speculative stacked review branches with Git notes telemetry. Operates as a JSON-RPC 2.0 stdio sidecar for the Dendrite Go control plane (`dendrite-control`), falling back to in-process Go implementations when absent.

See [docs/RESPONSIBILITIES.md](docs/RESPONSIBILITIES.md) for detailed architectural boundaries, interfaces, ticket mappings, and extracted packages.
