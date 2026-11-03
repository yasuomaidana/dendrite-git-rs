//! The errors every git and rpc operation reports.

use thiserror::Error;

/// Why an operation failed. `code` and `kind` are what the JSON-RPC server sends back.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GitError {
    #[error("not a repository: `{0}`")]
    NotARepository(String),
    #[error("git error: {0}")]
    Git(String),
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("method `{0}` not found")]
    MethodNotFound(String),
}

impl GitError {
    /// JSON-RPC error code: -32601 for an unknown method, -32602 for bad input, -32000 for an operation that failed.
    pub fn code(&self) -> i64 {
        match self {
            GitError::MethodNotFound(_) => -32601,
            GitError::InvalidParams(_) => -32602,
            _ => -32000,
        }
    }

    /// The `data.kind` of an operation error; empty for bad input.
    pub fn kind(&self) -> &'static str {
        match self {
            GitError::NotARepository(_) => "not_a_repository",
            GitError::Git(_) => "git_error",
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(GitError::MethodNotFound("m".into()), -32601, "")]
    #[case(GitError::InvalidParams("p".into()), -32602, "")]
    #[case(GitError::NotARepository("/tmp/none".into()), -32000, "not_a_repository")]
    #[case(GitError::Git("corrupt".into()), -32000, "git_error")]
    fn error_codes_and_kinds(#[case] err: GitError, #[case] code: i64, #[case] kind: &str) {
        assert_eq!((err.code(), err.kind()), (code, kind));
        assert!(!err.to_string().is_empty());
    }
}
