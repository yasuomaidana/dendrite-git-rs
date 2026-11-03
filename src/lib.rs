//! Pure-Rust Git hunk extraction and dependency graph engine for Dendrite.

pub mod error;
pub mod hunk;
pub mod repo;
pub mod rpc;

pub use error::GitError;
pub use hunk::{diff_file, is_binary, number, split_lines, Hunk, HunkKind};
pub use repo::{extract, Mode};

/// Compiles the Rust examples of the README as doctests, so they cannot drift from the code.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
