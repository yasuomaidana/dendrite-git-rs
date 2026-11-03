//! JSON-RPC 2.0 over lines: one request per line in, one response per line out. Logs go to stderr.

use std::io::{BufRead, Write};
use std::path::Path;
use std::str::FromStr;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::GitError;
use crate::repo::{extract, Mode};

/// The protocol version; a client refuses a different major version.
pub const PROTOCOL: &str = "1.0.0";

type Handler = fn(Value) -> Result<Value, GitError>;

const METHODS: &[(&str, Handler)] = &[("health", health), ("git.hunks", git_hunks)];

#[derive(Deserialize)]
struct GitHunksParams {
    repo: String,
    mode: String,
}

fn call<P: DeserializeOwned, R: Serialize>(
    params: Value,
    f: impl FnOnce(P) -> Result<R, GitError>,
) -> Result<Value, GitError> {
    let parsed =
        serde_json::from_value(params).map_err(|e| GitError::InvalidParams(e.to_string()))?;
    Ok(serde_json::to_value(f(parsed)?).expect("results are plain data"))
}

fn health(_: Value) -> Result<Value, GitError> {
    Ok(json!({
        "name": "dendrite-git",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": PROTOCOL,
    }))
}

fn git_hunks(params: Value) -> Result<Value, GitError> {
    call(params, |p: GitHunksParams| {
        let mode = Mode::from_str(&p.mode)?;
        let hunks = extract(Path::new(&p.repo), mode)?;
        Ok(json!({ "hunks": hunks }))
    })
}

fn dispatch(method: &str, params: Value) -> Result<Value, GitError> {
    let handler = METHODS
        .iter()
        .find(|(name, _)| *name == method)
        .map(|(_, handler)| handler)
        .ok_or_else(|| GitError::MethodNotFound(method.to_string()))?;
    handler(params)
}

fn failure(id: Value, code: i64, message: &str, kind: &str) -> String {
    let mut error = json!({"code": code, "message": message});
    if !kind.is_empty() {
        error["data"] = json!({"kind": kind});
    }
    json!({"jsonrpc": "2.0", "id": id, "error": error}).to_string()
}

/// Answers one request line. A notification (no `id`) gets no answer.
pub fn handle_line(line: &str) -> Option<String> {
    let request: Value = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(e) => return Some(failure(Value::Null, -32700, &e.to_string(), "")),
    };
    let id = request.get("id")?.clone();
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Some(failure(id, -32600, "invalid request: no method", ""));
    };
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    Some(match dispatch(method, params) {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
        Err(e) => failure(id, e.code(), &e.to_string(), e.kind()),
    })
}

/// Serves requests until `input` ends.
pub fn serve(input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if let Some(answer) = handle_line(&line).filter(|_| !line.trim().is_empty()) {
            writeln!(output, "{answer}")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// The program: `--version` prints the version, no argument serves stdin to stdout. Returns the exit code.
pub fn run(
    args: &[String],
    input: impl BufRead,
    output: impl Write,
    mut errors: impl Write,
) -> i32 {
    if args.get(1).is_some_and(|a| a == "--version") {
        let _ = writeln!(errors, "dendrite-git {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    match serve(input, output) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(errors, "dendrite-git: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tempfile::TempDir;

    #[test]
    fn health_returns_expected_metadata() {
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"health","params":{}}"#;
        let response_str = handle_line(line).expect("expected response");
        let val: Value = serde_json::from_str(&response_str).unwrap();
        assert_eq!(val["id"], 1);
        assert_eq!(val["result"]["name"], "dendrite-git");
        assert_eq!(val["result"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(val["result"]["protocol"], "1.0.0");
    }

    #[rstest]
    #[case::unknown_method(r#"{"jsonrpc":"2.0","id":2,"method":"unknown.method"}"#, -32601)]
    #[case::invalid_json("{not valid json}", -32700)]
    fn errors_carry_their_json_rpc_code(#[case] line: &str, #[case] code: i64) {
        let response_str = handle_line(line).expect("expected response");
        let val: Value = serde_json::from_str(&response_str).unwrap();
        assert_eq!(val["error"]["code"], code);
    }

    #[test]
    fn notification_gets_no_answer() {
        let line = r#"{"jsonrpc":"2.0","method":"health"}"#;
        assert_eq!(handle_line(line), None);
    }

    #[test]
    fn git_hunks_unknown_mode_returns_32602() {
        let temp = TempDir::new().unwrap();
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"git.hunks","params":{{"repo":"{}","mode":"invalid"}}}}"#,
            temp.path().display()
        );
        let response_str = handle_line(&line).expect("expected response");
        let val: Value = serde_json::from_str(&response_str).unwrap();
        assert_eq!(val["error"]["code"], -32602);
    }

    #[test]
    fn git_hunks_not_a_repository_returns_32000_with_kind() {
        let temp = TempDir::new().unwrap();
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":4,"method":"git.hunks","params":{{"repo":"{}","mode":"staged"}}}}"#,
            temp.path().display()
        );
        let response_str = handle_line(&line).expect("expected response");
        let val: Value = serde_json::from_str(&response_str).unwrap();
        assert_eq!(val["error"]["code"], -32000);
        assert_eq!(val["error"]["data"]["kind"], "not_a_repository");
    }

    #[test]
    fn serve_streams_responses_line_by_line() {
        let request = r#"{"jsonrpc":"2.0","id":5,"method":"health"}"#;
        let input = format!("{request}\n");
        let mut output = Vec::new();
        serve(input.as_bytes(), &mut output).unwrap();
        let out_str = String::from_utf8(output).unwrap();
        assert!(out_str.contains(r#""name":"dendrite-git""#));
    }
}
