//! In-process Git repository inspection using gitoxide (gix).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use gix::ObjectId;

use crate::error::GitError;
use crate::hunk::{diff_file, number, Hunk, HunkKind};

/// What to diff against HEAD: index (Staged) or worktree (All).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Staged,
    All,
}

impl std::str::FromStr for Mode {
    type Err = GitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "staged" => Ok(Mode::Staged),
            "all" => Ok(Mode::All),
            other => Err(GitError::InvalidParams(format!("unknown mode `{other}`"))),
        }
    }
}

/// Extracts diff hunks from a repository for the specified mode.
pub fn extract(repo_path: &Path, mode: Mode) -> Result<Vec<Hunk>, GitError> {
    let repo = gix::open(repo_path).map_err(|e| GitError::NotARepository(e.to_string()))?;
    let head_files = collect_head_files(&repo)?;

    let mut hunks = match mode {
        Mode::Staged => {
            let index_files = collect_index_files(&repo)?;
            extract_staged(head_files, index_files)
        }
        Mode::All => extract_all_worktree(&repo, repo_path, head_files)?,
    };

    number(&mut hunks);
    Ok(hunks)
}

fn extract_all_worktree(
    repo: &gix::Repository,
    repo_path: &Path,
    head_files: BTreeMap<String, (ObjectId, Vec<u8>)>,
) -> Result<Vec<Hunk>, GitError> {
    let worktree_dir = repo.workdir().unwrap_or(repo_path);
    let ignore_patterns = read_ignore_patterns(worktree_dir);
    let mut worktree_files = BTreeMap::new();
    walk_worktree(
        worktree_dir,
        worktree_dir,
        &ignore_patterns,
        &mut worktree_files,
    )?;
    Ok(extract_all(head_files, worktree_files))
}

fn collect_head_files(
    repo: &gix::Repository,
) -> Result<BTreeMap<String, (ObjectId, Vec<u8>)>, GitError> {
    let mut head_files = BTreeMap::new();
    let head = repo.head().map_err(|e| GitError::Git(e.to_string()))?;
    if !head.is_unborn() {
        if let Ok(commit) = repo.head_commit() {
            if let Ok(tree) = commit.tree() {
                collect_tree_entries(repo, &tree, "", &mut head_files)?;
            }
        }
    }
    Ok(head_files)
}

fn collect_tree_entries(
    repo: &gix::Repository,
    tree: &gix::Tree,
    prefix: &str,
    out: &mut BTreeMap<String, (ObjectId, Vec<u8>)>,
) -> Result<(), GitError> {
    let decoded = tree.decode().map_err(|e| GitError::Git(e.to_string()))?;
    for entry in decoded.entries {
        process_tree_entry(repo, entry, prefix, out)?;
    }
    Ok(())
}

fn process_tree_entry(
    repo: &gix::Repository,
    entry: gix::objs::tree::EntryRef<'_>,
    prefix: &str,
    out: &mut BTreeMap<String, (ObjectId, Vec<u8>)>,
) -> Result<(), GitError> {
    let name = entry.filename.to_string();
    let path = if prefix.is_empty() {
        name
    } else {
        format!("{prefix}/{name}")
    };
    if entry.mode.is_tree() {
        let subtree_obj = repo
            .find_object(entry.oid)
            .map_err(|e| GitError::Git(e.to_string()))?;
        let subtree = subtree_obj.into_tree();
        collect_tree_entries(repo, &subtree, &path, out)?;
    } else if entry.mode.is_blob() {
        let blob_obj = repo
            .find_object(entry.oid)
            .map_err(|e| GitError::Git(e.to_string()))?;
        out.insert(path, (entry.oid.to_owned(), blob_obj.data.to_vec()));
    }
    Ok(())
}

fn collect_index_files(
    repo: &gix::Repository,
) -> Result<BTreeMap<String, (ObjectId, Vec<u8>)>, GitError> {
    let index = repo
        .index_or_empty()
        .map_err(|e| GitError::Git(e.to_string()))?;
    let mut index_files = BTreeMap::new();
    for entry in index.entries() {
        let path = entry.path(&index).to_string();
        let oid = entry.id;
        let blob_data = match repo.find_object(oid) {
            Ok(obj) => obj.data.to_vec(),
            Err(_) => Vec::new(),
        };
        index_files.insert(path, (oid, blob_data));
    }
    Ok(index_files)
}

/// The git blob id of `data`. Hashing fails only when SHA-1 collision detection fires on crafted input; the
/// null id then stands in, so such a file never matches another as a rename.
fn hash_blob(data: &[u8]) -> ObjectId {
    let kind = gix::hash::Kind::Sha1;
    gix::objs::compute_hash(kind, gix::objs::Kind::Blob, data)
        .unwrap_or_else(|_| ObjectId::null(kind))
}

fn extract_staged(
    head_files: BTreeMap<String, (ObjectId, Vec<u8>)>,
    index_files: BTreeMap<String, (ObjectId, Vec<u8>)>,
) -> Vec<Hunk> {
    let mut hunks = Vec::new();
    let mut deleted = BTreeMap::new();
    let mut added = BTreeMap::new();

    for (path, (head_oid, head_data)) in &head_files {
        match index_files.get(path) {
            Some((index_oid, index_data)) => {
                if head_oid != index_oid {
                    hunks.extend(diff_file(path, Some(head_data), Some(index_data)));
                }
            }
            None => {
                deleted.insert(path.clone(), (*head_oid, head_data.clone()));
            }
        }
    }

    for (path, (index_oid, index_data)) in &index_files {
        if !head_files.contains_key(path) {
            added.insert(path.clone(), (*index_oid, index_data.clone()));
        }
    }

    detect_renames_and_diff(deleted, added, &mut hunks);
    hunks
}

fn extract_all(
    head_files: BTreeMap<String, (ObjectId, Vec<u8>)>,
    worktree_files: BTreeMap<String, Vec<u8>>,
) -> Vec<Hunk> {
    let mut hunks = Vec::new();
    let mut deleted = BTreeMap::new();
    let mut added = BTreeMap::new();

    for (path, (head_oid, head_data)) in &head_files {
        match worktree_files.get(path) {
            Some(disk_data) => {
                if disk_data != head_data {
                    hunks.extend(diff_file(path, Some(head_data), Some(disk_data)));
                }
            }
            None => {
                deleted.insert(path.clone(), (*head_oid, head_data.clone()));
            }
        }
    }

    for (path, disk_data) in &worktree_files {
        if !head_files.contains_key(path) {
            let disk_oid = hash_blob(disk_data);
            added.insert(path.clone(), (disk_oid, disk_data.clone()));
        }
    }

    detect_renames_and_diff(deleted, added, &mut hunks);
    hunks
}

fn group_by_oid(files: &BTreeMap<String, (ObjectId, Vec<u8>)>) -> BTreeMap<ObjectId, Vec<String>> {
    let mut by_oid: BTreeMap<ObjectId, Vec<String>> = BTreeMap::new();
    for (path, (oid, _)) in files {
        by_oid.entry(*oid).or_default().push(path.clone());
    }
    by_oid
}

fn find_exact_renames(
    deleted: &BTreeMap<String, (ObjectId, Vec<u8>)>,
    added: &BTreeMap<String, (ObjectId, Vec<u8>)>,
) -> Vec<(String, String)> {
    let del_by_oid = group_by_oid(deleted);
    let add_by_oid = group_by_oid(added);
    let mut renames = Vec::new();
    for (oid, del_paths) in &del_by_oid {
        if del_paths.len() == 1 {
            if let Some(add_paths) = add_by_oid.get(oid) {
                if add_paths.len() == 1 {
                    renames.push((del_paths[0].clone(), add_paths[0].clone()));
                }
            }
        }
    }
    renames
}

fn detect_renames_and_diff(
    mut deleted: BTreeMap<String, (ObjectId, Vec<u8>)>,
    mut added: BTreeMap<String, (ObjectId, Vec<u8>)>,
    out: &mut Vec<Hunk>,
) {
    let renames = find_exact_renames(&deleted, &added);
    for (old_path, new_path) in renames {
        deleted.remove(&old_path);
        added.remove(&new_path);
        out.push(Hunk {
            id: 0,
            file: new_path,
            old_start: 0,
            old_lines: 0,
            new_start: 0,
            new_lines: 0,
            added: Vec::new(),
            removed: Vec::new(),
            kind: HunkKind::Renamed(old_path),
        });
    }

    for (path, (_, data)) in deleted {
        out.extend(diff_file(&path, Some(&data), None));
    }
    for (path, (_, data)) in added {
        out.extend(diff_file(&path, None, Some(&data)));
    }
}

fn read_ignore_patterns(root: &Path) -> Vec<String> {
    let mut patterns = Vec::new();
    let gitignore_path = root.join(".gitignore");
    if let Ok(content) = fs::read_to_string(gitignore_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                patterns.push(trimmed.to_string());
            }
        }
    }
    patterns
}

fn pattern_matches(rel_path: &str, pat: &str) -> bool {
    if pat.ends_with('/') {
        let dir_pat = pat.trim_end_matches('/');
        rel_path == dir_pat
            || rel_path.starts_with(&format!("{dir_pat}/"))
            || rel_path.split('/').any(|segment| segment == dir_pat)
    } else if let Some(ext) = pat.strip_prefix("*.") {
        rel_path.ends_with(&format!(".{ext}"))
    } else {
        rel_path == pat || rel_path.ends_with(&format!("/{pat}"))
    }
}

fn is_path_ignored(rel_path: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pat| pattern_matches(rel_path, pat))
}

fn walk_worktree(
    root: &Path,
    dir: &Path,
    patterns: &[String],
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), GitError> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.map_err(|e| GitError::Git(e.to_string()))?;
        process_worktree_entry(root, entry.path(), patterns, out)?;
    }
    Ok(())
}

fn process_worktree_entry(
    root: &Path,
    path: PathBuf,
    patterns: &[String],
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), GitError> {
    if path.file_name().is_some_and(|n| n == ".git") {
        return Ok(());
    }
    let rel_path = path
        .strip_prefix(root)
        .map_err(|e| GitError::Git(e.to_string()))?
        .to_string_lossy()
        .replace('\\', "/");
    if is_path_ignored(&rel_path, patterns) {
        return Ok(());
    }
    if path.is_dir() {
        walk_worktree(root, &path, patterns, out)?;
    } else if path.is_file() {
        let data = fs::read(&path).map_err(|e| GitError::Git(e.to_string()))?;
        out.insert(rel_path, data);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn run_git(dir: &Path, args: &[&str]) {
        let temp_home = dir.join(".home");
        let _ = fs::create_dir_all(&temp_home);
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("HOME", &temp_home)
            .env("GIT_CONFIG_GLOBAL", temp_home.join(".gitconfig"))
            .env("GIT_AUTHOR_NAME", "Dendrite Test")
            .env("GIT_AUTHOR_EMAIL", "test@dendrite.dev")
            .env("GIT_COMMITTER_NAME", "Dendrite Test")
            .env("GIT_COMMITTER_EMAIL", "test@dendrite.dev")
            .output()
            .expect("git CLI command failed to spawn");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn staged_vs_all() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        run_git(dir, &["init"]);
        run_git(dir, &["config", "user.name", "Dendrite Test"]);
        run_git(dir, &["config", "user.email", "test@dendrite.dev"]);

        let file = dir.join("tracked.txt");
        fs::write(&file, "line 1\n").unwrap();
        run_git(dir, &["add", "tracked.txt"]);
        run_git(dir, &["commit", "-m", "initial commit"]);

        fs::write(&file, "line 1\nline 2 (staged)\n").unwrap();
        run_git(dir, &["add", "tracked.txt"]);

        fs::write(&file, "line 1\nline 2 (staged)\nline 3 (unstaged)\n").unwrap();

        let staged_hunks = extract(dir, Mode::Staged).unwrap();
        assert_eq!(staged_hunks.len(), 1);
        assert_eq!(staged_hunks[0].added, vec!["line 2 (staged)\n"]);

        let all_hunks = extract(dir, Mode::All).unwrap();
        assert_eq!(all_hunks.len(), 1);
        assert_eq!(
            all_hunks[0].added,
            vec!["line 2 (staged)\n", "line 3 (unstaged)\n"]
        );
    }

    #[test]
    fn untracked_file() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        run_git(dir, &["init"]);
        run_git(dir, &["config", "user.name", "Dendrite Test"]);
        run_git(dir, &["config", "user.email", "test@dendrite.dev"]);

        let base = dir.join("base.txt");
        fs::write(&base, "base\n").unwrap();
        run_git(dir, &["add", "base.txt"]);
        run_git(dir, &["commit", "-m", "base commit"]);

        let untracked = dir.join("untracked.txt");
        fs::write(&untracked, "new file content\n").unwrap();

        let staged = extract(dir, Mode::Staged).unwrap();
        assert_eq!(staged.len(), 0);

        let all = extract(dir, Mode::All).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].file, "untracked.txt");
        assert_eq!(all[0].kind, HunkKind::Added);
    }

    #[test]
    fn deleted_file() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        run_git(dir, &["init"]);
        run_git(dir, &["config", "user.name", "Dendrite Test"]);
        run_git(dir, &["config", "user.email", "test@dendrite.dev"]);

        let del_file = dir.join("to_delete.txt");
        fs::write(&del_file, "goodbye\n").unwrap();
        run_git(dir, &["add", "to_delete.txt"]);
        run_git(dir, &["commit", "-m", "to delete"]);

        fs::remove_file(&del_file).unwrap();

        let all = extract(dir, Mode::All).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].file, "to_delete.txt");
        assert_eq!(all[0].kind, HunkKind::Deleted);
    }

    #[test]
    fn exact_content_rename() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        run_git(dir, &["init"]);
        run_git(dir, &["config", "user.name", "Dendrite Test"]);
        run_git(dir, &["config", "user.email", "test@dendrite.dev"]);

        let old = dir.join("old.txt");
        fs::write(&old, "exact content 42\n").unwrap();
        run_git(dir, &["add", "old.txt"]);
        run_git(dir, &["commit", "-m", "add old"]);

        run_git(dir, &["mv", "old.txt", "new.txt"]);

        let staged = extract(dir, Mode::Staged).unwrap();
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].file, "new.txt");
        assert_eq!(staged[0].kind, HunkKind::Renamed("old.txt".into()));

        let all = extract(dir, Mode::All).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].file, "new.txt");
        assert_eq!(all[0].kind, HunkKind::Renamed("old.txt".into()));
    }

    #[test]
    fn unborn_head() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        run_git(dir, &["init"]);

        let first = dir.join("first.txt");
        fs::write(&first, "hello unborn\n").unwrap();
        run_git(dir, &["add", "first.txt"]);

        let staged = extract(dir, Mode::Staged).unwrap();
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].file, "first.txt");
        assert_eq!(staged[0].kind, HunkKind::Added);

        let all = extract(dir, Mode::All).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].file, "first.txt");
        assert_eq!(all[0].kind, HunkKind::Added);
    }

    #[test]
    fn not_a_repository() {
        let temp = TempDir::new().unwrap();
        let err = extract(temp.path(), Mode::All).unwrap_err();
        assert_eq!(err.code(), -32000);
        assert_eq!(err.kind(), "not_a_repository");
    }
}
