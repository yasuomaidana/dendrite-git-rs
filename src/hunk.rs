//! Pure line-diff hunk extraction without Git dependencies.

use imara_diff::{Algorithm, Diff, InternedInput};
use serde::{Deserialize, Serialize};

/// The kind of change represented by a hunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HunkKind {
    Modified,
    Added,
    Deleted,
    Binary,
    Renamed(String),
}

/// A single diff hunk extracted from a file change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunk {
    pub id: usize,
    pub file: String,
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub kind: HunkKind,
}

/// Checks whether a byte slice contains binary content (NUL byte in the first 8000 bytes).
pub fn is_binary(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(8000);
    bytes[..check_len].contains(&0)
}

/// Splits text into lines preserving trailing newlines where present.
pub fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    while start < text.len() {
        if let Some(pos) = text[start..].find('\n') {
            let end = start + pos + 1;
            lines.push(text[start..end].to_string());
            start = end;
        } else {
            lines.push(text[start..].to_string());
            break;
        }
    }
    lines
}

fn diff_binary(path: &str) -> Vec<Hunk> {
    vec![Hunk {
        id: 0,
        file: path.to_string(),
        old_start: 0,
        old_lines: 0,
        new_start: 0,
        new_lines: 0,
        added: Vec::new(),
        removed: Vec::new(),
        kind: HunkKind::Binary,
    }]
}

fn diff_added(path: &str, new_bytes: &[u8]) -> Vec<Hunk> {
    let text = String::from_utf8_lossy(new_bytes);
    let lines = split_lines(&text);
    let count = lines.len();
    vec![Hunk {
        id: 0,
        file: path.to_string(),
        old_start: 0,
        old_lines: 0,
        new_start: if count == 0 { 0 } else { 1 },
        new_lines: count,
        added: lines,
        removed: Vec::new(),
        kind: HunkKind::Added,
    }]
}

fn diff_deleted(path: &str, old_bytes: &[u8]) -> Vec<Hunk> {
    let text = String::from_utf8_lossy(old_bytes);
    let lines = split_lines(&text);
    let count = lines.len();
    vec![Hunk {
        id: 0,
        file: path.to_string(),
        old_start: if count == 0 { 0 } else { 1 },
        old_lines: count,
        new_start: 0,
        new_lines: 0,
        added: Vec::new(),
        removed: lines,
        kind: HunkKind::Deleted,
    }]
}

fn diff_modified(path: &str, old_bytes: &[u8], new_bytes: &[u8]) -> Vec<Hunk> {
    let old_str = String::from_utf8_lossy(old_bytes);
    let new_str = String::from_utf8_lossy(new_bytes);
    if old_str == new_str {
        return Vec::new();
    }
    let input = InternedInput::new(&*old_str, &*new_str);
    let diff = Diff::compute(Algorithm::Myers, &input);
    let mut hunks = Vec::new();
    for item in diff.hunks() {
        let old_lines = (item.before.end - item.before.start) as usize;
        let new_lines = (item.after.end - item.after.start) as usize;
        let old_start = if old_lines == 0 {
            item.before.start as usize
        } else {
            (item.before.start + 1) as usize
        };
        let new_start = if new_lines == 0 {
            item.after.start as usize
        } else {
            (item.after.start + 1) as usize
        };
        let removed: Vec<String> = input.before
            [item.before.start as usize..item.before.end as usize]
            .iter()
            .map(|&tok| input.interner[tok].to_string())
            .collect();
        let added: Vec<String> = input.after[item.after.start as usize..item.after.end as usize]
            .iter()
            .map(|&tok| input.interner[tok].to_string())
            .collect();
        hunks.push(Hunk {
            id: 0,
            file: path.to_string(),
            old_start,
            old_lines,
            new_start,
            new_lines,
            added,
            removed,
            kind: HunkKind::Modified,
        });
    }
    hunks
}

/// Zero-context line diff (imara-diff, Myers).
pub fn diff_file(path: &str, old: Option<&[u8]>, new: Option<&[u8]>) -> Vec<Hunk> {
    match (old, new) {
        (None, None) => Vec::new(),
        (Some(old_bytes), Some(new_bytes)) if old_bytes == new_bytes => Vec::new(),
        (old_opt, new_opt) if old_opt.is_some_and(is_binary) || new_opt.is_some_and(is_binary) => {
            diff_binary(path)
        }
        (None, Some(new_bytes)) => diff_added(path, new_bytes),
        (Some(old_bytes), None) => diff_deleted(path, old_bytes),
        (Some(old_bytes), Some(new_bytes)) => diff_modified(path, old_bytes, new_bytes),
    }
}

/// Assigns IDs to hunks in file path order then position.
pub fn number(hunks: &mut [Hunk]) {
    hunks.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| a.new_start.cmp(&b.new_start))
            .then_with(|| a.old_start.cmp(&b.old_start))
    });
    for (i, hunk) in hunks.iter_mut().enumerate() {
        hunk.id = i + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// The expected shape of one hunk: its kind, the lines it adds and the lines it removes.
    fn change(
        kind: HunkKind,
        added: &[&str],
        removed: &[&str],
    ) -> (HunkKind, Vec<String>, Vec<String>) {
        let own = |lines: &[&str]| lines.iter().map(|l| l.to_string()).collect();
        (kind, own(added), own(removed))
    }

    /// A one-line modified hunk of `file` starting at `start`.
    fn hunk(file: &str, start: usize) -> Hunk {
        Hunk {
            id: 0,
            file: file.into(),
            old_start: start,
            old_lines: 1,
            new_start: start,
            new_lines: 1,
            added: vec![format!("{file}{start}\n")],
            removed: vec![],
            kind: HunkKind::Modified,
        }
    }

    #[rstest]
    #[case(
        "modify_one_line",
        "f.txt",
        Some(b"line 1\nline 2\nline 3\n".as_slice()),
        Some(b"line 1\nline changed\nline 3\n".as_slice()),
        vec![change(HunkKind::Modified, &["line changed\n"], &["line 2\n"])]
    )]
    #[case(
        "insert",
        "f.txt",
        Some(b"line 1\nline 3\n".as_slice()),
        Some(b"line 1\nline 2\nline 3\n".as_slice()),
        vec![change(HunkKind::Modified, &["line 2\n"], &[])]
    )]
    #[case(
        "delete",
        "f.txt",
        Some(b"line 1\nline 2\nline 3\n".as_slice()),
        Some(b"line 1\nline 3\n".as_slice()),
        vec![change(HunkKind::Modified, &[], &["line 2\n"])]
    )]
    #[case(
        "two_separate_hunks",
        "multi.txt",
        Some(b"line 1\nkeep 1\nline 2\nkeep 2\nline 3\n".as_slice()),
        Some(b"change 1\nkeep 1\nchange 2\nkeep 2\nline 3\n".as_slice()),
        vec![
            change(HunkKind::Modified, &["change 1\n"], &["line 1\n"]),
            change(HunkKind::Modified, &["change 2\n"], &["line 2\n"]),
        ]
    )]
    #[case(
        "add_file",
        "f.txt",
        None,
        Some(b"first\nsecond\n".as_slice()),
        vec![change(HunkKind::Added, &["first\n", "second\n"], &[])]
    )]
    #[case(
        "delete_file",
        "f.txt",
        Some(b"first\nsecond\n".as_slice()),
        None,
        vec![change(HunkKind::Deleted, &[], &["first\n", "second\n"])]
    )]
    #[case(
        "binary",
        "f.bin",
        Some(b"hello\0binary".as_slice()),
        Some(b"changed\0binary".as_slice()),
        vec![change(HunkKind::Binary, &[], &[])]
    )]
    #[case(
        "identical",
        "f.txt",
        Some(b"identical\ncontent\n".as_slice()),
        Some(b"identical\ncontent\n".as_slice()),
        vec![]
    )]
    #[case(
        "empty_old",
        "f.txt",
        Some(b"".as_slice()),
        Some(b"new content\n".as_slice()),
        vec![change(HunkKind::Modified, &["new content\n"], &[])]
    )]
    #[case(
        "trailing_newline_differences",
        "f.txt",
        Some(b"trailing\n".as_slice()),
        Some(b"trailing".as_slice()),
        vec![change(HunkKind::Modified, &["trailing"], &["trailing\n"])]
    )]
    #[case(
        "both_none",
        "f.txt",
        None,
        None,
        vec![]
    )]
    #[case(
        "add_empty_file",
        "f.txt",
        None,
        Some(b"".as_slice()),
        vec![change(HunkKind::Added, &[], &[])]
    )]
    #[case(
        "delete_empty_file",
        "f.txt",
        Some(b"".as_slice()),
        None,
        vec![change(HunkKind::Deleted, &[], &[])]
    )]
    #[case(
        "lossy_utf8_identical",
        "f.txt",
        Some(b"\xff".as_slice()),
        Some(b"\xfe".as_slice()),
        vec![]
    )]
    #[case(
        "binary_added",
        "f.bin",
        None,
        Some(b"hello\0bin".as_slice()),
        vec![change(HunkKind::Binary, &[], &[])]
    )]
    #[case(
        "binary_deleted",
        "f.bin",
        Some(b"hello\0bin".as_slice()),
        None,
        vec![change(HunkKind::Binary, &[], &[])]
    )]
    #[case(
        "text_to_binary",
        "f.bin",
        Some(b"hello text".as_slice()),
        Some(b"hello\0bin".as_slice()),
        vec![change(HunkKind::Binary, &[], &[])]
    )]
    #[case(
        "add_file_without_trailing_newline",
        "f.txt",
        None,
        Some(b"first\nsecond".as_slice()),
        vec![change(HunkKind::Added, &["first\n", "second"], &[])]
    )]
    #[case(
        "delete_file_without_trailing_newline",
        "f.txt",
        Some(b"first\nsecond".as_slice()),
        None,
        vec![change(HunkKind::Deleted, &[], &["first\n", "second"])]
    )]
    fn diff_file_scenarios(
        #[case] _name: &str,
        #[case] file: &str,
        #[case] old: Option<&[u8]>,
        #[case] new: Option<&[u8]>,
        #[case] expected: Vec<(HunkKind, Vec<String>, Vec<String>)>,
    ) {
        let hunks = diff_file(file, old, new);
        assert_eq!(hunks.len(), expected.len());
        for (hunk, (exp_kind, exp_added, exp_removed)) in hunks.iter().zip(expected.iter()) {
            assert_eq!(hunk.file, file);
            assert_eq!(&hunk.kind, exp_kind);
            assert_eq!(&hunk.added, exp_added);
            assert_eq!(&hunk.removed, exp_removed);
        }
    }

    #[test]
    fn numbering_assigns_ids_in_path_order_then_position() {
        let mut h1 = hunk("a.txt", 5);
        h1.old_start = 10;
        let mut h2 = hunk("a.txt", 5);
        h2.old_start = 2;
        let mut hunks = vec![hunk("b.txt", 10), hunk("a.txt", 20), h1, h2];
        number(&mut hunks);
        assert_eq!(hunks[0].id, 1);
        assert_eq!(hunks[0].file, "a.txt");
        assert_eq!(hunks[0].new_start, 5);
        assert_eq!(hunks[0].old_start, 2);

        assert_eq!(hunks[1].id, 2);
        assert_eq!(hunks[1].file, "a.txt");
        assert_eq!(hunks[1].new_start, 5);
        assert_eq!(hunks[1].old_start, 10);

        assert_eq!(hunks[2].id, 3);
        assert_eq!(hunks[2].file, "a.txt");
        assert_eq!(hunks[2].new_start, 20);

        assert_eq!(hunks[3].id, 4);
        assert_eq!(hunks[3].file, "b.txt");
        assert_eq!(hunks[3].new_start, 10);
    }

    #[test]
    fn split_lines_empty_returns_empty_vec() {
        assert!(split_lines("").is_empty());
    }

    #[rstest]
    #[case("alpha\nbeta", vec!["alpha\n", "beta"])]
    #[case("single", vec!["single"])]
    fn split_lines_without_trailing_newline(#[case] text: &str, #[case] expected: Vec<&str>) {
        assert_eq!(split_lines(text), expected);
    }

    #[test]
    fn hunk_serialization_round_trip() {
        let hunk = Hunk {
            id: 1,
            file: "renamed.txt".into(),
            old_start: 0,
            old_lines: 0,
            new_start: 0,
            new_lines: 0,
            added: vec![],
            removed: vec![],
            kind: HunkKind::Renamed("original.txt".into()),
        };
        let serialized = serde_json::to_string(&hunk).unwrap();
        let deserialized: Hunk = serde_json::from_str(&serialized).unwrap();
        assert_eq!(hunk, deserialized);
    }

    #[test]
    fn is_binary_with_large_slice() {
        let mut large = vec![b'a'; 10000];
        assert!(!is_binary(&large));
        large[8000] = 0;
        assert!(!is_binary(&large));
        large[100] = 0;
        assert!(is_binary(&large));
    }

    #[test]
    fn number_empty_slice() {
        let mut hunks: Vec<Hunk> = Vec::new();
        number(&mut hunks);
        assert!(hunks.is_empty());
    }
}
