//! In-process Git repository inspection using gitoxide (gix).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

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
    let index = repo
        .index_or_empty()
        .map_err(|e| GitError::Git(e.to_string()))?;
    let excludes = repo
        .excludes(
            &index,
            None,
            gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
        )
        .map_err(|e| GitError::Git(e.to_string()))?;
    // Ignore rules never apply to a path Git already tracks, in HEAD or in the index.
    let tracked: BTreeSet<String> = head_files
        .keys()
        .cloned()
        .chain(index.entries().iter().map(|e| e.path(&index).to_string()))
        .collect();
    let mut worktree_files = BTreeMap::new();
    Scanner {
        root: worktree_dir,
        excludes,
        tracked,
    }
    .walk(worktree_dir, &mut worktree_files)?;
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
    gix::objs::compute_hash(kind, gix::objs::Kind::Blob, data).unwrap_or(ObjectId::null(kind))
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

/// Walks the working tree the way `git status` does: symlinks are never followed (a link is
/// its own blob in Git, and following one could read outside the repository or loop), and the
/// exclude rules come from Git itself (nested `.gitignore`, negation, anchoring, `**`,
/// `.git/info/exclude` and `core.excludesFile`). Tracked paths are never excluded.
struct Scanner<'a> {
    root: &'a Path,
    excludes: gix::AttributeStack<'a>,
    tracked: BTreeSet<String>,
}

impl Scanner<'_> {
    fn walk(&mut self, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) -> Result<(), GitError> {
        let Ok(entries) = fs::read_dir(dir) else {
            return Ok(());
        };
        for entry in entries {
            let entry = entry?;
            self.visit(&entry.path(), out)?;
        }
        Ok(())
    }

    fn visit(&mut self, path: &Path, out: &mut BTreeMap<String, Vec<u8>>) -> Result<(), GitError> {
        if path.file_name().is_some_and(|n| n == ".git") {
            return Ok(());
        }
        // `symlink_metadata` does not follow links: a link is skipped, whatever it points at.
        let Ok(meta) = fs::symlink_metadata(path) else {
            return Ok(());
        };
        if meta.file_type().is_symlink() {
            return Ok(());
        }
        // Every entry comes from listing a directory under `root`, so the prefix is always there.
        let rel_path = path
            .strip_prefix(self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if meta.is_dir() {
            if !self.is_ignored(&rel_path, true) {
                self.walk(path, out)?;
            }
        } else if meta.is_file() && !self.is_ignored(&rel_path, false) {
            out.insert(rel_path, fs::read(path)?);
        }
        Ok(())
    }

    /// Whether Git's exclude rules hide `rel_path`. A tracked file, or a directory holding one,
    /// is never hidden: ignore rules only apply to untracked paths.
    fn is_ignored(&mut self, rel_path: &str, is_dir: bool) -> bool {
        let tracked = if is_dir {
            let prefix = format!("{rel_path}/");
            self.tracked.iter().any(|k| k.starts_with(&prefix))
        } else {
            self.tracked.contains(rel_path)
        };
        if tracked {
            return false;
        }
        let mode = if is_dir {
            gix::index::entry::Mode::DIR
        } else {
            gix::index::entry::Mode::FILE
        };
        self.excludes
            .at_path(rel_path, Some(mode))
            .is_ok_and(|platform| platform.is_excluded())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use std::path::PathBuf;
    use std::str::FromStr;
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

    #[fixture]
    fn git_repo() -> TempDir {
        let repo = TempDir::new().unwrap();
        run_git(repo.path(), &["init", "-q"]);
        repo
    }

    #[rstest]
    #[case("staged", Ok(Mode::Staged))]
    #[case("STAGED", Ok(Mode::Staged))]
    #[case("  all  ", Ok(Mode::All))]
    #[case("ALL", Ok(Mode::All))]
    #[case("other", Err(GitError::InvalidParams("unknown mode `other`".into())))]
    fn mode_from_str_parsing(#[case] input: &str, #[case] expected: Result<Mode, GitError>) {
        assert_eq!(Mode::from_str(input), expected);
    }

    #[rstest]
    fn gitignore_filtering(git_repo: TempDir) {
        let dir = git_repo.path();
        let gitignore = concat!(
            "\n# comments and empty lines\n\n",
            ".gitignore\n.home/\ntarget/\n*.log\n",
            "ignored_dir/\ntemp.txt\nsub/special.env\n",
        );
        fs::write(dir.join(".gitignore"), gitignore).unwrap();
        fs::create_dir_all(dir.join("target")).unwrap();
        fs::write(dir.join("target/build.txt"), "built\n").unwrap();
        fs::create_dir_all(dir.join("nested/ignored_dir")).unwrap();
        fs::write(dir.join("nested/ignored_dir/sub.txt"), "nested\n").unwrap();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/special.env"), "secret=1\n").unwrap();
        fs::write(dir.join("app.log"), "log line\n").unwrap();
        fs::write(dir.join("temp.txt"), "temp\n").unwrap();
        fs::write(dir.join("tracked.txt"), "keep this\n").unwrap();

        let hunks = extract(dir, Mode::All).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].file, "tracked.txt");
    }

    #[rstest]
    fn nested_directories_in_commit_and_worktree(git_repo: TempDir) {
        let dir = git_repo.path();
        let nested_dir = dir.join("src/nested");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("file.txt"), "hello inside\n").unwrap();
        run_git(dir, &["add", "src/nested/file.txt"]);
        run_git(dir, &["commit", "-m", "add nested"]);

        fs::write(nested_dir.join("file.txt"), "hello inside changed\n").unwrap();
        let hunks = extract(dir, Mode::All).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].file, "src/nested/file.txt");
        assert_eq!(hunks[0].kind, HunkKind::Modified);
    }

    #[rstest]
    fn staged_add_delete_and_unmodified(git_repo: TempDir) {
        let dir = git_repo.path();
        fs::write(dir.join("keep.txt"), "keep\n").unwrap();
        fs::write(dir.join("del.txt"), "to delete\n").unwrap();
        run_git(dir, &["add", "keep.txt", "del.txt"]);
        run_git(dir, &["commit", "-m", "base"]);

        run_git(dir, &["rm", "del.txt"]);
        fs::write(dir.join("added.txt"), "new file\n").unwrap();
        run_git(dir, &["add", "added.txt"]);

        let staged = extract(dir, Mode::Staged).unwrap();
        assert_eq!(staged.len(), 2);
        assert_eq!(staged[0].file, "added.txt");
        assert_eq!(staged[0].kind, HunkKind::Added);
        assert_eq!(staged[1].file, "del.txt");
        assert_eq!(staged[1].kind, HunkKind::Deleted);
    }

    #[rstest]
    fn ambiguous_duplicate_renames(git_repo: TempDir) {
        let dir = git_repo.path();
        let content = "identical duplicate content\n";
        fs::write(dir.join("d1.txt"), content).unwrap();
        fs::write(dir.join("d2.txt"), content).unwrap();
        run_git(dir, &["add", "d1.txt", "d2.txt"]);
        run_git(dir, &["commit", "-m", "base duplicate"]);

        run_git(dir, &["rm", "d1.txt", "d2.txt"]);
        fs::write(dir.join("a1.txt"), content).unwrap();
        fs::write(dir.join("a2.txt"), content).unwrap();
        run_git(dir, &["add", "a1.txt", "a2.txt"]);

        let staged = extract(dir, Mode::Staged).unwrap();
        assert_eq!(staged.len(), 4);
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

    #[test]
    #[should_panic(expected = "git [\"nonexistent-git-cmd\"] failed")]
    fn run_git_failure_panics() {
        let temp = TempDir::new().unwrap();
        run_git(temp.path(), &["nonexistent-git-cmd"]);
    }

    #[cfg(unix)]
    #[rstest]
    fn worktree_broken_symlink_ignored(git_repo: TempDir) {
        let dir = git_repo.path();
        std::os::unix::fs::symlink("nowhere", dir.join("dangling_link")).unwrap();
        assert!(extract(dir, Mode::All).unwrap().is_empty());
    }

    #[cfg(unix)]
    #[rstest]
    fn worktree_unreadable_dir_handled(git_repo: TempDir) {
        use std::os::unix::fs::PermissionsExt;
        let dir = git_repo.path();
        let unreadable = dir.join("cant_read_dir");
        fs::create_dir(&unreadable).unwrap();
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

        assert!(extract(dir, Mode::All).unwrap().is_empty());

        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    #[rstest]
    fn worktree_unreadable_file_propagates_error(git_repo: TempDir) {
        use std::os::unix::fs::PermissionsExt;
        let dir = git_repo.path();
        let unreadable = dir.join("cant_read_file.txt");
        fs::write(&unreadable, "data\n").unwrap();
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

        assert!(matches!(extract(dir, Mode::All), Err(GitError::Git(_))));

        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[rstest]
    fn commit_tree_with_submodule_entry(git_repo: TempDir) {
        let dir = git_repo.path();
        let sha = "1111111111111111111111111111111111111111";
        run_git(
            dir,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                "160000",
                sha,
                "submod",
            ],
        );
        run_git(dir, &["commit", "-m", "add submodule entry"]);

        assert!(extract(dir, Mode::All).unwrap().is_empty());
    }

    #[rstest]
    fn staged_index_with_missing_object_handled(git_repo: TempDir) {
        let dir = git_repo.path();
        let sha = "2222222222222222222222222222222222222222";
        run_git(
            dir,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                "100644",
                sha,
                "ghost.txt",
            ],
        );

        let staged = extract(dir, Mode::Staged).unwrap();
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].file, "ghost.txt");
        assert_eq!(staged[0].kind, HunkKind::Added);
    }

    #[rstest]
    fn head_pointing_to_tree_object_handled(git_repo: TempDir) {
        let dir = git_repo.path();
        run_git(dir, &["commit", "--allow-empty", "-m", "empty"]);

        let repo = gix::open(dir).unwrap();
        let tree_id = repo.head_commit().unwrap().tree_id().unwrap();
        // git refuses to point a branch at a tree; a detached HEAD file can name any object.
        fs::write(dir.join(".git/HEAD"), format!("{tree_id}\n")).unwrap();

        assert!(extract(dir, Mode::All).unwrap().is_empty());
    }

    #[rstest]
    fn head_commit_missing_tree_handled(git_repo: TempDir) {
        let dir = git_repo.path();
        let file = dir.join("solo.txt");
        fs::write(&file, "content\n").unwrap();
        run_git(dir, &["add", "solo.txt"]);
        run_git(dir, &["commit", "-m", "solo"]);

        let repo = gix::open(dir).unwrap();
        let tree_id = repo.head_commit().unwrap().tree_id().unwrap().to_string();
        let (prefix, suffix) = tree_id.split_at(2);
        let tree_path = dir.join(".git/objects").join(prefix).join(suffix);
        fs::remove_file(tree_path).unwrap();

        let hunks = extract(dir, Mode::All).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].file, "solo.txt");
    }

    fn object_path(dir: &Path, hex: &str) -> PathBuf {
        dir.join(".git/objects").join(&hex[..2]).join(&hex[2..])
    }

    #[rstest]
    #[case::head(".git/HEAD", b"malformed-head-ref\n".as_slice(), Mode::All)]
    #[case::index(".git/index", b"bad-index-data".as_slice(), Mode::Staged)]
    fn corrupted_git_state_fails_with_git_error(
        git_repo: TempDir,
        #[case] file: &str,
        #[case] bytes: &[u8],
        #[case] mode: Mode,
    ) {
        let dir = git_repo.path();
        fs::write(dir.join(file), bytes).unwrap();
        let err = extract(dir, mode).unwrap_err();
        assert_eq!(err.code(), -32000);
        assert_eq!(err.kind(), "git_error");
    }

    #[rstest]
    #[case(true)]
    #[case(false)]
    fn commit_missing_tree_entry_object_fails(git_repo: TempDir, #[case] test_tree: bool) {
        let dir = git_repo.path();
        let sub = dir.join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("inner.txt"), "nested\n").unwrap();
        fs::write(dir.join("outer.txt"), "root\n").unwrap();
        run_git(dir, &["add", "sub/inner.txt", "outer.txt"]);
        run_git(dir, &["commit", "-m", "mixed"]);

        let repo = gix::open(dir).unwrap();
        let tree = repo.head_commit().unwrap().tree().unwrap();
        let decoded = tree.decode().unwrap();
        let entry = decoded
            .entries
            .into_iter()
            .find(|e| {
                if test_tree {
                    e.mode.is_tree()
                } else {
                    e.mode.is_blob()
                }
            })
            .unwrap();
        fs::remove_file(object_path(dir, &entry.oid.to_string())).unwrap();

        assert!(matches!(extract(dir, Mode::All), Err(GitError::Git(_))));
    }

    #[rstest]
    fn commit_corrupt_tree_object_fails(git_repo: TempDir) {
        let dir = git_repo.path();
        fs::write(dir.join("doc.txt"), "some text\n").unwrap();
        run_git(dir, &["add", "doc.txt"]);
        run_git(dir, &["commit", "-m", "initial"]);

        let repo = gix::open(dir).unwrap();
        let real_tree_id = repo.head_commit().unwrap().tree_id().unwrap();

        fs::write(dir.join("corrupt_raw"), "malformed tree entry bytes\n").unwrap();
        run_git(
            dir,
            &[
                "hash-object",
                "-w",
                "-t",
                "tree",
                "--literally",
                "corrupt_raw",
            ],
        );
        let bad_tree_id = gix::objs::compute_hash(
            gix::hash::Kind::Sha1,
            gix::objs::Kind::Tree,
            b"malformed tree entry bytes\n",
        )
        .unwrap();

        // git stores objects read-only; replace the real tree with the malformed one.
        let real_path = object_path(dir, &real_tree_id.to_string());
        fs::remove_file(&real_path).unwrap();
        fs::copy(object_path(dir, &bad_tree_id.to_string()), &real_path).unwrap();

        assert!(matches!(extract(dir, Mode::All), Err(GitError::Git(_))));
    }

    #[rstest]
    fn single_deleted_multiple_added_no_rename(git_repo: TempDir) {
        let repo_root = git_repo.path();
        let source_file = repo_root.join("source.txt");
        fs::write(&source_file, "payload\n").unwrap();
        run_git(repo_root, &["add", "source.txt"]);
        run_git(repo_root, &["commit", "-m", "init source"]);

        fs::remove_file(&source_file).unwrap();
        for name in ["dup1.txt", "dup2.txt"] {
            fs::write(repo_root.join(name), "payload\n").unwrap();
        }
        run_git(repo_root, &["add", "-A"]);

        let res = extract(repo_root, Mode::Staged).unwrap();
        assert_eq!(res.len(), 3);
        assert!(!res.iter().any(|h| matches!(h.kind, HunkKind::Renamed(_))));
    }

    #[rstest]
    fn gitignore_nested_path_matching(git_repo: TempDir) {
        let dir = git_repo.path();
        fs::write(dir.join(".gitignore"), "leaf.txt\n").unwrap();
        let sub = dir.join("deep/sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("leaf.txt"), "ignored leaf\n").unwrap();
        fs::write(sub.join("other.txt"), "included other\n").unwrap();

        let hunks = extract(dir, Mode::All).unwrap();
        assert!(hunks.iter().all(|h| h.file != "deep/sub/leaf.txt"));
        assert!(hunks.iter().any(|h| h.file == "deep/sub/other.txt"));
    }

    /// The untracked files `extract` reports, which must equal `git ls-files --others --exclude-standard`.
    fn reported(dir: &Path) -> Vec<String> {
        let mut files: Vec<String> = extract(dir, Mode::All)
            .unwrap()
            .into_iter()
            .map(|h| h.file)
            .collect();
        files.sort();
        files.dedup();
        files
    }

    fn git_untracked(dir: &Path) -> Vec<String> {
        let out = std::process::Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(dir)
            .env("HOME", dir.join(".home"))
            .env("GIT_CONFIG_GLOBAL", dir.join(".home/.gitconfig"))
            .output()
            .unwrap();
        let mut files: Vec<String> = String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect();
        files.sort();
        files
    }

    #[rstest]
    #[case::nested_gitignore(&[("sub/.gitignore", "inner.txt\n"), ("sub/inner.txt", "x\n"), ("inner.txt", "y\n")])]
    #[case::negation(&[(".gitignore", "*.log\n!keep.log\n"), ("a.log", "x\n"), ("keep.log", "y\n")])]
    #[case::anchored(&[(".gitignore", "/top.txt\n"), ("top.txt", "x\n"), ("d/top.txt", "y\n")])]
    #[case::double_star(&[(".gitignore", "**/gen/*.rs\n"), ("a/gen/x.rs", "x\n"), ("a/y.rs", "y\n")])]
    #[case::info_exclude(&[(".git/info/exclude", "local.txt\n"), ("local.txt", "x\n"), ("shared.txt", "y\n")])]
    #[case::glob_prefix_only(&[(".gitignore", "*.log\n"), (".log", "x\n"), ("a.log", "y\n")])]
    fn untracked_files_match_git(git_repo: TempDir, #[case] files: &[(&str, &str)]) {
        let dir = git_repo.path();
        fs::write(dir.join(".git/info/exclude"), "/.home/\n").unwrap();
        for (path, content) in files {
            let target = dir.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            if *path == ".git/info/exclude" {
                fs::write(&target, format!("/.home/\n{content}")).unwrap();
            } else {
                fs::write(&target, content).unwrap();
            }
        }
        assert_eq!(reported(dir), git_untracked(dir));
    }

    #[rstest]
    fn tracked_files_stay_visible_when_a_rule_matches_them(git_repo: TempDir) {
        let dir = git_repo.path();
        fs::write(dir.join(".git/info/exclude"), "/.home/\n").unwrap();
        fs::write(dir.join("a.log"), "one\n").unwrap();
        run_git(dir, &["add", "-f", "a.log"]);
        run_git(dir, &["commit", "-q", "-m", "init"]);
        fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
        fs::write(dir.join("a.log"), "two\n").unwrap();
        assert!(reported(dir).contains(&"a.log".to_string()));
    }

    #[cfg(unix)]
    #[rstest]
    fn symlinks_are_never_followed(git_repo: TempDir) {
        use std::os::unix::fs::symlink;
        let dir = git_repo.path();
        fs::write(dir.join(".git/info/exclude"), "/.home/\n").unwrap();
        let outside = TempDir::new().unwrap();
        fs::write(outside.path().join("secret.txt"), "top secret\n").unwrap();
        symlink(outside.path().join("secret.txt"), dir.join("leak_file")).unwrap();
        symlink(outside.path(), dir.join("leak_dir")).unwrap();
        symlink(dir, dir.join("loop")).unwrap();
        fs::write(dir.join("real.txt"), "real\n").unwrap();

        assert_eq!(reported(dir), vec!["real.txt".to_string()]);
    }
}
