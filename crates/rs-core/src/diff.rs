//! Commit detail and per-file diffs (M2.7; FR-27, FR-28, R-8, ARCH §6.5).
//!
//! First parent (or the empty tree for a root commit) → commit tree, renames at 50 % similarity. Stats only while
//! the commit has ≤ 300 files; inline hunks only while the whole commit has ≤ 20k changed lines, and then only
//! for files with ≤ 2,000 changed lines (others: "Load diff" → `file_diff`). Lines are lossy UTF-8 without CR/LF.

use crate::commits::summary_of;
use crate::error::{Error, Result};
use crate::types::{ChangeKind, CommitDetail, DiffLine, FileChange, FileDiff, Hunk, LineKind};
use git2::{Delta, Diff, DiffFindOptions, DiffOptions, Oid, Patch, Repository};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffLimits {
    /// More files → file list only, no stats, no diffs.
    pub max_files: u32,
    /// More changed lines in the commit → stats + file list, diffs on demand.
    pub max_lines: u32,
    /// More changed lines in one file → collapsed ("Load diff").
    pub file_lines: u32,
    /// Either side larger → `too_large`, never rendered.
    pub blob_bytes: u64,
}

impl Default for DiffLimits {
    fn default() -> Self {
        DiffLimits { max_files: 300, max_lines: 20_000, file_lines: 2_000, blob_bytes: 1024 * 1024 }
    }
}

/// Upper bound of lines returned by a forced "Load diff".
pub const FORCE_MAX_LINES: u32 = 100_000;
pub const NO_NEWLINE: &str = "\\ No newline at end of file";

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Lockfiles, minified bundles, `dist/`, generated sources.
pub fn is_generated(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    matches!(
        name,
        "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "cargo.lock" | "gemfile.lock" | "poetry.lock" | "composer.lock" | "go.sum" | "bun.lockb"
    ) || name.contains(".min.")
        || name.ends_with(".pb.go")
        || name.contains("_generated.")
        || lower.starts_with("dist/")
        || lower.contains("/dist/")
}

fn build<'r>(repo: &'r Repository, c: &git2::Commit, limits: &DiffLimits) -> Result<Diff<'r>> {
    let tree = c.tree()?;
    let parent_tree = if c.parent_count() > 0 { Some(c.parent(0)?.tree()?) } else { None };
    let mut o = DiffOptions::new();
    o.context_lines(3).max_size(limits.blob_bytes as i64).ignore_submodules(true).include_typechange(true);
    let mut d = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut o))?;
    let mut f = DiffFindOptions::new();
    f.renames(true).copies(false).rename_threshold(50).rename_limit(1000);
    d.find_similar(Some(&mut f))?;
    Ok(d)
}

fn kind(d: Delta) -> ChangeKind {
    match d {
        Delta::Added | Delta::Untracked => ChangeKind::Added,
        Delta::Deleted => ChangeKind::Deleted,
        Delta::Renamed => ChangeKind::Renamed,
        Delta::Copied => ChangeKind::Copied,
        Delta::Typechange => ChangeKind::TypeChange,
        _ => ChangeKind::Modified,
    }
}

fn blob_size(odb: &git2::Odb, oid: Oid) -> u64 {
    if oid.is_zero() { 0 } else { odb.read_header(oid).map(|(n, _)| n as u64).unwrap_or(0) }
}

/// File list entry for delta `i` (no stats).
fn change(diff: &Diff, odb: &git2::Odb, i: usize, limits: &DiffLimits) -> Option<FileChange> {
    let d = diff.get_delta(i)?;
    let k = kind(d.status());
    let old_path = d.old_file().path_bytes().map(lossy).filter(|_| k != ChangeKind::Added);
    let new_path = d.new_file().path_bytes().map(lossy).filter(|_| k != ChangeKind::Deleted);
    let too_large = blob_size(odb, d.old_file().id()) > limits.blob_bytes || blob_size(odb, d.new_file().id()) > limits.blob_bytes;
    let generated = new_path.as_deref().or(old_path.as_deref()).is_some_and(is_generated);
    Some(FileChange {
        index: i as u32,
        kind: k,
        old_path,
        new_path,
        additions: 0,
        deletions: 0,
        binary: d.flags().is_binary() && !too_large,
        too_large,
        generated,
    })
}

fn hunks(patch: &Patch, max_lines: u32) -> Result<(Vec<Hunk>, bool)> {
    let mut out = Vec::with_capacity(patch.num_hunks());
    let mut emitted = 0u32;
    for h in 0..patch.num_hunks() {
        let (hunk, n) = patch.hunk(h)?;
        let mut lines = Vec::with_capacity(n);
        for l in 0..n {
            let line = patch.line_in_hunk(h, l)?;
            let k = match line.origin() {
                '+' => LineKind::Add,
                '-' => LineKind::Del,
                '=' | '>' | '<' => LineKind::NoNewline,
                _ => LineKind::Ctx,
            };
            if k == LineKind::NoNewline {
                lines.push(DiffLine { kind: k, old_no: None, new_no: None, text: NO_NEWLINE.into() });
                continue;
            }
            if emitted >= max_lines {
                if !lines.is_empty() {
                    out.push(Hunk { header: lossy(hunk.header()).trim_end().to_string(), lines });
                }
                return Ok((out, true));
            }
            let text = lossy(line.content());
            let text = text.strip_suffix('\n').unwrap_or(&text);
            let text = text.strip_suffix('\r').unwrap_or(text);
            lines.push(DiffLine { kind: k, old_no: line.old_lineno(), new_no: line.new_lineno(), text: text.to_string() });
            emitted += 1;
        }
        out.push(Hunk { header: lossy(hunk.header()).trim_end().to_string(), lines });
    }
    Ok((out, false))
}

/// Commit page data (FR-27, FR-28).
pub fn commit_detail(repo: &Repository, oid: Oid, limits: &DiffLimits) -> Result<CommitDetail> {
    let c = repo.find_commit(oid)?;
    let diff = build(repo, &c, limits)?;
    let odb = repo.odb()?;
    let n = diff.deltas().len();
    let mut files: Vec<FileChange> = (0..n).filter_map(|i| change(&diff, &odb, i, limits)).collect();
    let mut additions = 0u32;
    let mut deletions = 0u32;
    let mut diffs = Vec::new();
    let mut diffs_deferred = n as u32 > limits.max_files;
    if !diffs_deferred {
        let mut patches = Vec::with_capacity(n);
        for f in files.iter_mut() {
            let p = Patch::from_diff(&diff, f.index as usize)?;
            if let Some(p) = &p {
                // libgit2 decides "binary" while loading the blobs for the patch.
                f.binary = p.delta().flags().is_binary() && !f.too_large;
                let (_, add, del) = p.line_stats()?;
                f.additions = add as u32;
                f.deletions = del as u32;
                additions += add as u32;
                deletions += del as u32;
            }
            patches.push(p);
        }
        if additions + deletions > limits.max_lines {
            diffs_deferred = true;
        } else {
            for (f, p) in files.iter().zip(patches) {
                let Some(p) = p else { continue };
                if f.binary || f.too_large || f.additions + f.deletions > limits.file_lines {
                    continue; // collapsed: "Binary file" / "Load diff"
                }
                let (h, truncated) = hunks(&p, limits.file_lines)?;
                diffs.push(FileDiff { index: f.index, hunks: h, truncated });
            }
        }
    }
    Ok(CommitDetail {
        commit: summary_of(&c),
        message: lossy(c.message_bytes()),
        parent_compared: c.parent_id(0).ok().map(|p| p.to_string()),
        is_merge: c.parent_count() > 1,
        files,
        additions,
        deletions,
        diffs,
        diffs_deferred,
    })
}

/// "Load diff" for file `index` of the commit. Without `force`, more than 2,000 lines are truncated; binary and
/// over-1 MB files are refused.
pub fn file_diff(repo: &Repository, oid: Oid, index: u32, force: bool) -> Result<FileDiff> {
    let limits = DiffLimits::default();
    let c = repo.find_commit(oid)?;
    let diff = build(repo, &c, &limits)?;
    let odb = repo.odb()?;
    let f = change(&diff, &odb, index as usize, &limits).ok_or_else(|| Error::not_found("file in commit"))?;
    if f.too_large {
        return Err(Error::Refused("Large diff not rendered".into()));
    }
    let p = match Patch::from_diff(&diff, index as usize)? {
        Some(p) if !f.binary && !p.delta().flags().is_binary() => p,
        _ => return Err(Error::Refused("Binary file".into())),
    };
    let max = if force { FORCE_MAX_LINES } else { limits.file_lines };
    let (h, truncated) = hunks(&p, max)?;
    Ok(FileDiff { index, hunks: h, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{self, T0};

    struct Fx {
        _t: tempfile::TempDir,
        path: std::path::PathBuf,
        root: Oid,
        second: Oid,
    }

    fn lines(n: usize, tag: &str) -> Vec<u8> {
        (0..n).map(|i| format!("{tag} line {i}\n")).collect::<String>().into_bytes()
    }

    fn fixture() -> Fx {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("d");
        let mut b = testkit::repo(&path);
        let mut renamed = lines(40, "keep");
        b.bare()
            .file("mod.txt", &lines(30, "orig"))
            .file("gone.txt", b"bye\n")
            .file("old/name.rs", &renamed)
            .file("img.bin", &[0u8, 1, 2, 3])
            .file("crlf.txt", b"a\r\nb\r\n")
            .file("eol.txt", b"no newline")
            .commit("root", T0);
        let root = b.head();
        let mut modified = lines(30, "orig");
        modified.splice(0..0, b"inserted at top\n".iter().copied());
        modified.extend_from_slice(b"appended\n");
        renamed.extend_from_slice(b"one more line\n");
        b.file("mod.txt", &modified)
            .remove("gone.txt")
            .remove("old/name.rs")
            .file("new/name.rs", &renamed)
            .file("img.bin", &[0u8, 9, 9, 9])
            .file("crlf.txt", b"a\r\nB\r\n")
            .file("eol.txt", b"no newline, changed")
            .file("big.txt", &lines(2500, "big"))
            .file("package-lock.json", b"{}\n")
            .commit("second\n\nWith a body.\n", T0 + 60);
        Fx { second: b.head(), root, path, _t: t }
    }

    /// (files, additions, deletions) from `git show --numstat`.
    fn git_numstat(path: &std::path::Path, oid: Oid) -> Option<(usize, u32, u32)> {
        let out = testkit::git_cli(path, &["show", "--numstat", "-M50%", "--format=", &oid.to_string()])?;
        let mut files = 0;
        let (mut a, mut d) = (0, 0);
        for l in out.lines().filter(|l| !l.trim().is_empty()) {
            files += 1;
            let mut p = l.split('\t');
            a += p.next().unwrap().parse::<u32>().unwrap_or(0);
            d += p.next().unwrap().parse::<u32>().unwrap_or(0);
        }
        Some((files, a, d))
    }

    #[test]
    fn stats_kinds_and_lines() {
        let fx = fixture();
        let repo = testkit::open(&fx.path);
        let d = commit_detail(&repo, fx.second, &DiffLimits::default()).unwrap();
        assert_eq!(d.message, "second\n\nWith a body.\n");
        assert_eq!(d.commit.summary, "second");
        assert_eq!(d.parent_compared, Some(fx.root.to_string()));
        assert!(!d.is_merge && !d.diffs_deferred);
        let f = |p: &str| d.files.iter().find(|f| f.new_path.as_deref() == Some(p) || f.old_path.as_deref() == Some(p)).unwrap().clone();
        assert_eq!((f("mod.txt").kind, f("mod.txt").additions, f("mod.txt").deletions), (ChangeKind::Modified, 2, 0));
        assert_eq!((f("gone.txt").kind, f("gone.txt").new_path.clone()), (ChangeKind::Deleted, None));
        let r = f("new/name.rs");
        assert_eq!((r.kind, r.old_path.as_deref(), r.additions, r.deletions), (ChangeKind::Renamed, Some("old/name.rs"), 1, 0));
        assert!(f("img.bin").binary);
        assert!(f("package-lock.json").generated && !f("mod.txt").generated);
        assert_eq!((f("big.txt").kind, f("big.txt").additions), (ChangeKind::Added, 2500));
        // > 2,000 changed lines and binary files are collapsed (no inline diff)
        let inline = |p: &str| d.diffs.iter().any(|x| x.index == f(p).index);
        assert!(!inline("big.txt") && !inline("img.bin"));
        assert!(inline("mod.txt") && inline("new/name.rs"));
        // CRLF stripped, NoNewline marker
        let crlf = d.diffs.iter().find(|x| x.index == f("crlf.txt").index).unwrap();
        assert!(crlf.hunks.iter().flat_map(|h| &h.lines).all(|l| !l.text.contains('\r') && !l.text.contains('\n')));
        let eol = d.diffs.iter().find(|x| x.index == f("eol.txt").index).unwrap();
        assert!(eol.hunks[0].lines.iter().any(|l| l.kind == LineKind::NoNewline && l.text == NO_NEWLINE));
        let m = d.diffs.iter().find(|x| x.index == f("mod.txt").index).unwrap();
        assert!(m.hunks[0].header.starts_with("@@ -1,"), "{}", m.hunks[0].header);
        let first_add = m.hunks[0].lines.iter().find(|l| l.kind == LineKind::Add).unwrap();
        assert_eq!((first_add.text.as_str(), first_add.old_no, first_add.new_no), ("inserted at top", None, Some(1)));
        // totals vs git
        let ours = (d.files.len(), d.additions, d.deletions);
        assert_eq!(ours, (8, 2 + 1 + 1 + 1 + 2500 + 1, 1 + 1 + 1));
        match git_numstat(&fx.path, fx.second) {
            Some(g) => assert_eq!(ours, g, "git show --numstat"),
            None => eprintln!("git not on PATH: comparison skipped"),
        }
        // Load diff: truncated without force, full with force; binary refused
        let big = file_diff(&repo, fx.second, f("big.txt").index, false).unwrap();
        assert!(big.truncated);
        assert_eq!(big.hunks.iter().map(|h| h.lines.len()).sum::<usize>(), 2000);
        let big = file_diff(&repo, fx.second, f("big.txt").index, true).unwrap();
        assert!(!big.truncated);
        assert_eq!(big.hunks.iter().map(|h| h.lines.len()).sum::<usize>(), 2500);
        assert_eq!(file_diff(&repo, fx.second, f("img.bin").index, true).unwrap_err().code(), "refused");
        assert_eq!(file_diff(&repo, fx.second, 999, true).unwrap_err().code(), "notFound");
    }

    #[test]
    fn root_and_merge() {
        let fx = fixture();
        let repo = testkit::open(&fx.path);
        let d = commit_detail(&repo, fx.root, &DiffLimits::default()).unwrap();
        assert_eq!(d.parent_compared, None);
        assert!(d.files.iter().all(|f| f.kind == ChangeKind::Added && f.old_path.is_none()));
        assert_eq!(d.files.len(), 6);
        if let Some(g) = git_numstat(&fx.path, fx.root) {
            assert_eq!((d.files.len(), d.additions, d.deletions), g);
        }
        let t = tempfile::tempdir().unwrap();
        let m = testkit::merges(&t.path().join("m"));
        let repo = testkit::open(&m.path);
        let d = commit_detail(&repo, m.m1, &DiffLimits::default()).unwrap();
        assert!(d.is_merge);
        assert_eq!(d.parent_compared, Some(m.c2.to_string()), "first parent");
        assert_eq!(d.files.iter().map(|f| f.new_path.clone().unwrap()).collect::<Vec<_>>(), ["dir/x.txt"]);
        // `git show` prints a combined diff for merges; compare against the first parent explicitly
        if let Some(out) = testkit::git_cli(&m.path, &["diff", "--numstat", &m.c2.to_string(), &m.m1.to_string()]) {
            assert_eq!(out.lines().count(), 1);
            assert!(out.starts_with("1\t1\tdir/x.txt"), "{out}");
        }
        assert_eq!((d.additions, d.deletions), (1, 1));
    }

    #[test]
    fn caps_defer_diffs() {
        let t = tempfile::tempdir().unwrap();
        let (p, oid) = testkit::huge_commit(&t.path().join("h"));
        let repo = testkit::open(&p);
        let d = commit_detail(&repo, oid, &DiffLimits::default()).unwrap();
        assert!(d.diffs_deferred && d.diffs.is_empty());
        assert_eq!(d.files.len(), testkit::HUGE_COMMIT_FILES as usize);
        // under the file cap but over the line cap: stats, no inline diffs
        let limits = DiffLimits { max_files: 1000, ..Default::default() };
        let d = commit_detail(&repo, oid, &limits).unwrap();
        assert!(d.diffs_deferred && d.diffs.is_empty());
        assert!(d.additions + d.deletions > 20_000);
        // a single file is still loadable on demand
        assert!(!file_diff(&repo, oid, 0, false).unwrap().hunks.is_empty());
    }

    #[test]
    fn generated_heuristic() {
        for p in ["package-lock.json", "a/yarn.lock", "Cargo.lock", "web/app.min.js", "dist/x.js", "pkg/dist/y.css", "api.pb.go", "x_generated.rs"] {
            assert!(is_generated(p), "{p}");
        }
        for p in ["src/main.rs", "distance.rs", "README.md"] {
            assert!(!is_generated(p), "{p}");
        }
    }
}
