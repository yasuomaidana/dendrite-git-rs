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

fn git_command(dir: &std::path::Path, args: &[&str]) -> Command {
    let home = dir.join(".home");
    fs::create_dir_all(&home).unwrap();
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(dir)
        .env("HOME", &home)
        .env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"))
        .env("GIT_AUTHOR_NAME", "Dendrite Test")
        .env("GIT_AUTHOR_EMAIL", "test@dendrite.dev")
        .env("GIT_COMMITTER_NAME", "Dendrite Test")
        .env("GIT_COMMITTER_EMAIL", "test@dendrite.dev");
    command
}

fn git(dir: &std::path::Path, args: &[&str]) {
    assert!(
        git_command(dir, args).status().unwrap().success(),
        "git {args:?}"
    );
}

fn git_stdout(dir: &std::path::Path, args: &[&str]) -> String {
    let out = git_command(dir, args).output().unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A repository with one commit, then: a.txt edited, b.txt deleted, c.txt staged as new, old.txt staged as a
/// rename, dir/sub/n.txt edited and staged, and an ignored x.log left in the working tree.
fn committed_repo_with_changes() -> TempDir {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();
    git(dir, &["init", "-q"]);
    fs::create_dir_all(dir.join("dir/sub")).unwrap();
    for (name, content) in [
        ("a.txt", "one\ntwo\n"),
        ("b.txt", "bee\n"),
        ("old.txt", "rename me\n"),
        ("dir/sub/n.txt", "nested\n"),
        (".gitignore", "# logs\n\n*.log\nbuild/\nsecret.txt\n"),
    ] {
        fs::write(dir.join(name), content).unwrap();
    }
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "initial"]);
    fs::write(dir.join("a.txt"), "one\nchanged\n").unwrap();
    fs::remove_file(dir.join("b.txt")).unwrap();
    fs::write(dir.join("c.txt"), "new\n").unwrap();
    fs::write(dir.join("x.log"), "ignored\n").unwrap();
    fs::create_dir_all(dir.join("build")).unwrap();
    fs::write(dir.join("build/out.o"), "ignored\n").unwrap();
    fs::write(dir.join("dir/secret.txt"), "ignored\n").unwrap();
    fs::write(dir.join("dir/sub/n.txt"), "nested\nedited\n").unwrap();
    git(dir, &["mv", "old.txt", "renamed.txt"]);
    git(dir, &["add", "c.txt", "dir/sub/n.txt"]);
    temp
}

/// The `kind` of every hunk in an answer: a bare name, or the key of `{"renamed": "<old path>"}` for a rename.
fn hunk_kinds(answer: &serde_json::Value) -> Vec<String> {
    answer["result"]["hunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| match h["kind"].as_str() {
            Some(name) => name.to_string(),
            None => h["kind"]
                .as_object()
                .unwrap()
                .keys()
                .next()
                .unwrap()
                .clone(),
        })
        .collect()
}

fn hunk_files(answer: &serde_json::Value) -> Vec<&str> {
    answer["result"]["hunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["file"].as_str().unwrap())
        .collect()
}

fn hunks_request(repo: &str, mode: &str) -> serde_json::Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 9, "method": "git.hunks",
        "params": {"repo": repo, "mode": mode}
    });
    let (out, _, ok) = run(&[], &format!("{request}\n"));
    assert!(ok);
    serde_json::from_str(out.trim()).unwrap()
}

#[rstest::rstest]
#[case::all("all", &[("a.txt", "modified"), ("b.txt", "deleted"), ("c.txt", "added"), ("dir/sub/n.txt", "modified")])]
#[case::staged("staged", &[("c.txt", "added"), ("dir/sub/n.txt", "modified"), ("renamed.txt", "renamed")])]
fn lists_the_hunks_of_a_committed_repository(#[case] mode: &str, #[case] want: &[(&str, &str)]) {
    let repo = committed_repo_with_changes();
    let answer = hunks_request(&repo.path().display().to_string(), mode);
    let got: Vec<(&str, String)> = hunk_files(&answer)
        .into_iter()
        .zip(hunk_kinds(&answer))
        .collect();
    for (file, kind) in want {
        assert!(got.contains(&(*file, kind.to_string())), "{got:?}");
    }
}

#[rstest::rstest]
#[case::unknown_mode(true, "bogus")]
#[case::not_a_repository(false, "all")]
fn refuses_a_request_it_cannot_serve(#[case] real_repo: bool, #[case] mode: &str) {
    let repo = committed_repo_with_changes();
    let target = if real_repo {
        repo.path().to_path_buf()
    } else {
        std::env::temp_dir().join("dendrite-git-not-a-repo-xyz")
    };
    let answer = hunks_request(&target.display().to_string(), mode);
    assert!(answer["error"]["code"].is_i64(), "{answer}");
}

#[test]
fn leaves_out_what_gitignore_names() {
    let repo = committed_repo_with_changes();
    let answer = hunks_request(&repo.path().display().to_string(), "all");
    let files = hunk_files(&answer);
    for ignored in ["x.log", "build/out.o", "dir/secret.txt"] {
        assert!(!files.contains(&ignored), "{ignored} in {files:?}");
    }
}

/// Starts the binary, feeds it raw bytes and returns (stderr, exit code). With `hang_up_stdout` the read end
/// of its stdout is closed before it answers, so its first write fails.
fn run_raw(stdin: &[u8], hang_up_stdout: bool) -> (String, Option<i32>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dendrite-git"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if hang_up_stdout {
        drop(child.stdout.take());
    }
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    (String::from_utf8(out.stderr).unwrap(), out.status.code())
}

#[rstest::rstest]
#[case::not_json("{nope\n", -32700)]
#[case::no_method("{\"jsonrpc\":\"2.0\",\"id\":3}\n", -32600)]
#[case::unknown_method("{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"nope\"}\n", -32601)]
fn answers_failed_requests_with_their_code(#[case] request: &str, #[case] code: i64) {
    let (out, _, ok) = run(&[], request);
    assert!(ok);
    let answer: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(answer["error"]["code"], code);
}

#[rstest::rstest]
#[case::invalid_utf8(b"\xff\xfe\n".as_slice(), false, "valid UTF-8")]
#[case::stdout_closed(b"{\"id\":1,\"method\":\"health\"}\n".as_slice(), true, "dendrite-git: ")]
fn exits_with_1_when_the_stream_breaks(
    #[case] stdin: &[u8],
    #[case] hang_up_stdout: bool,
    #[case] message: &str,
) {
    let (err, code) = run_raw(stdin, hang_up_stdout);
    assert_eq!(code, Some(1));
    assert!(err.contains(message), "{err}");
}

/// Takes away all access to a path and gives it back when dropped, so the temp dir can be removed.
#[cfg(unix)]
struct Denied(std::path::PathBuf, u32);

#[cfg(unix)]
impl Denied {
    fn new(path: std::path::PathBuf, restore_mode: u32) -> Self {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        Denied(path, restore_mode)
    }
}

#[cfg(unix)]
impl Drop for Denied {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(self.1));
    }
}

#[cfg(unix)]
#[test]
fn reports_a_file_it_cannot_read_as_an_error() {
    let temp = TempDir::new().unwrap();
    gix::init(temp.path()).unwrap();
    let file = temp.path().join("locked.dat");
    fs::write(&file, "secret\n").unwrap();
    let _denied = Denied::new(file, 0o644);
    let answer = hunks_request(&temp.path().display().to_string(), "all");
    assert_eq!(answer["error"]["code"], -32000, "{answer}");
}

#[cfg(unix)]
#[test]
fn skips_a_directory_it_cannot_list() {
    let temp = TempDir::new().unwrap();
    gix::init(temp.path()).unwrap();
    let locked = temp.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("inside.txt"), "x\n").unwrap();
    fs::write(temp.path().join("seen.txt"), "x\n").unwrap();
    let _denied = Denied::new(locked, 0o755);
    let answer = hunks_request(&temp.path().display().to_string(), "all");
    let files = hunk_files(&answer);
    assert!(
        files.contains(&"seen.txt") && !files.contains(&"locked/inside.txt"),
        "{files:?}"
    );
}

#[test]
fn tolerates_a_staged_entry_whose_object_is_missing() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();
    git(dir, &["init", "-q"]);
    let ghost = "1111111111111111111111111111111111111111";
    git(
        dir,
        &[
            "update-index",
            "--add",
            "--info-only",
            "--cacheinfo",
            &format!("100644,{ghost},ghost.txt"),
        ],
    );
    let answer = hunks_request(&dir.display().to_string(), "staged");
    assert!(answer["result"]["hunks"].is_array(), "{answer}");
}

#[rstest::fixture]
fn committed_file_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    git(temp.path(), &["init", "-q"]);
    for name in ["s.txt", "t.txt"] {
        fs::write(temp.path().join(name), "payload\n").unwrap();
    }
    git(temp.path(), &["add", "."]);
    git(temp.path(), &["commit", "-q", "-m", "initial"]);
    temp
}

/// Deletes the loose object `rev` names (git keeps objects read-only).
fn destroy_object(dir: &std::path::Path, rev: &str) {
    let sha = git_stdout(dir, &["rev-parse", rev]);
    let path = dir.join(".git/objects").join(&sha[..2]).join(&sha[2..]);
    fs::remove_file(path).unwrap();
}

fn assert_lists_hunks(dir: &std::path::Path, mode: &str) {
    let answer = hunks_request(&dir.display().to_string(), mode);
    assert!(answer["result"]["hunks"].is_array(), "{answer}");
}

#[rstest::rstest]
#[case::head_commit_missing("HEAD")]
#[case::root_tree_missing("HEAD^{tree}")]
fn copes_with_a_head_whose_objects_are_gone(committed_file_repo: TempDir, #[case] victim: &str) {
    destroy_object(committed_file_repo.path(), victim);
    assert_lists_hunks(committed_file_repo.path(), "all");
}

#[test]
fn copes_with_a_submodule_entry_in_head() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();
    git(dir, &["init", "-q"]);
    let gitlink = "160000,1111111111111111111111111111111111111111,vendor";
    git(dir, &["update-index", "--add", "--cacheinfo", gitlink]);
    git(dir, &["commit", "-q", "-m", "with a submodule"]);
    assert_lists_hunks(dir, "all");
}

#[rstest::rstest]
#[case::one_deleted_two_copies(&["s.txt"], &["dup1.txt", "dup2.txt"])]
#[case::two_deleted_one_copy(&["s.txt", "t.txt"], &["u.txt"])]
fn does_not_pair_ambiguous_copies_as_renames(
    committed_file_repo: TempDir,
    #[case] deleted: &[&str],
    #[case] added: &[&str],
) {
    let dir = committed_file_repo.path();
    for name in deleted {
        git(dir, &["rm", "-q", name]);
    }
    for name in added {
        fs::write(dir.join(name), "payload\n").unwrap();
    }
    git(dir, &["add", "-A"]);
    let kinds = hunk_kinds(&hunks_request(&dir.display().to_string(), "staged"));
    assert_eq!(kinds.len(), deleted.len() + added.len(), "{kinds:?}");
    assert!(!kinds.contains(&"renamed".to_string()), "{kinds:?}");
}

#[cfg(unix)]
#[rstest::rstest]
fn ignores_a_symlink_that_points_nowhere(committed_file_repo: TempDir) {
    std::os::unix::fs::symlink("nowhere", committed_file_repo.path().join("dangling")).unwrap();
    assert_lists_hunks(committed_file_repo.path(), "all");
}
