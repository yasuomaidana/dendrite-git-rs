//! The `dendrite-git` binary, started the way a client starts it.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn run(args: &[&str], stdin: &str) -> (String, String, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dendrite-git"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
        out.status.success(),
    )
}

#[test]
fn prints_its_version() {
    let (_, err, ok) = run(&["--version"], "");
    assert!(ok && err.starts_with("dendrite-git "));
}

#[test]
fn answers_health_request() {
    let request = r#"{"jsonrpc":"2.0","id":1,"method":"health","params":{}}"#;
    let (out, _, ok) = run(&[], &format!("{request}\n"));
    assert!(ok);
    let answer: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(answer["id"], 1);
    assert_eq!(answer["result"]["name"], "dendrite-git");
    assert_eq!(answer["result"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(answer["result"]["protocol"], "1.0.0");
}

#[test]
fn answers_git_hunks_request() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();
    gix::init(dir).unwrap();
    fs::write(dir.join("sample.txt"), "hello world\n").unwrap();

    let request = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"git.hunks","params":{{"repo":"{}","mode":"all"}}}}"#,
        dir.display()
    );
    let (out, _, ok) = run(&[], &format!("{request}\n"));
    assert!(ok);
    let answer: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(answer["id"], 2);
    let hunks = answer["result"]["hunks"].as_array().unwrap();
    assert_eq!(hunks.len(), 1);
    assert_eq!(hunks[0]["file"], "sample.txt");
    assert_eq!(hunks[0]["kind"], "added");
}
