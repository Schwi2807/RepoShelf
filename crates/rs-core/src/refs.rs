//! Ref resolution and listings (M2.3; ARCH §3.5; FR-13, FR-29, FR-30).

use crate::commits::summary_of;
use crate::error::{Error, Result};
use crate::jobs::CancelToken;
use crate::types::{AheadBehind, BranchRow, RefItem, RefKind, RefsList, ResolvedRev, TagRow};
use crate::validate::RevSpec;
use git2::{ErrorCode, ObjectType, Oid, Repository};

pub const HEADS: &str = "refs/heads/";
pub const TAGS: &str = "refs/tags/";
pub const REMOTES: &str = "refs/remotes/";
pub const ORIGIN: &str = "refs/remotes/origin/";

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn kind_of(full: &str) -> RefKind {
    if full.starts_with(HEADS) {
        RefKind::Branch
    } else if full.starts_with(TAGS) {
        RefKind::Tag
    } else if full.starts_with(REMOTES) {
        RefKind::RemoteBranch
    } else {
        RefKind::Commit
    }
}

fn resolved(input: &str, kind: RefKind, full_name: Option<String>, commit: &git2::Commit) -> ResolvedRev {
    ResolvedRev { input: input.to_string(), kind, full_name, commit: commit.id().to_string(), tree: commit.tree_id().to_string() }
}

/// Exact ref lookup peeled to a commit; `Ok(None)` when the ref does not exist.
fn try_ref<'r>(repo: &'r Repository, full: &str) -> Result<Option<git2::Commit<'r>>> {
    match repo.find_reference(full) {
        Ok(r) => match r.peel_to_commit() {
            Ok(c) => Ok(Some(c)),
            Err(e) if e.code() == ErrorCode::NotFound || e.class() == git2::ErrorClass::Object => {
                Err(Error::InvalidRev(format!("{full} does not point at a commit")))
            }
            Err(e) => Err(e.into()),
        },
        Err(e) if e.code() == ErrorCode::NotFound || e.code() == ErrorCode::InvalidSpec => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// ARCH §3.5, first hit wins: exact `refs/…`; `refs/heads/<s>`; `refs/tags/<s>`; `refs/remotes/<s>`; `HEAD`;
/// hex 4–40 → commit (ambiguous → `InvalidRev`). Not found → `NotFound{what:"ref"}`.
pub fn resolve(repo: &Repository, spec: &RevSpec) -> Result<ResolvedRev> {
    let s = spec.as_str();
    if s.starts_with("refs/") {
        return match try_ref(repo, s)? {
            Some(c) => Ok(resolved(s, kind_of(s), Some(s.to_string()), &c)),
            None => Err(Error::not_found("ref")),
        };
    }
    for prefix in [HEADS, TAGS, REMOTES] {
        let full = format!("{prefix}{s}");
        if let Some(c) = try_ref(repo, &full)? {
            return Ok(resolved(s, kind_of(&full), Some(full), &c));
        }
    }
    if s == "HEAD" {
        let head = match repo.head() {
            Ok(h) => h,
            Err(e) if e.code() == ErrorCode::UnbornBranch || e.code() == ErrorCode::NotFound => return Err(Error::not_found("ref")),
            Err(e) => return Err(e.into()),
        };
        let c = head.peel_to_commit()?;
        return Ok(if head.is_branch() {
            let full = lossy(head.name_bytes());
            resolved(s, RefKind::Branch, Some(full), &c)
        } else {
            resolved(s, RefKind::Commit, None, &c)
        });
    }
    if (4..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return match repo.find_object_by_prefix(&s.to_ascii_lowercase(), None) {
            Ok(obj) => match obj.peel(ObjectType::Commit) {
                Ok(o) => {
                    let c = o.into_commit().map_err(|_| Error::not_found("ref"))?;
                    Ok(resolved(s, RefKind::Commit, None, &c))
                }
                Err(_) => Err(Error::not_found("ref")),
            },
            Err(e) if e.code() == ErrorCode::Ambiguous => Err(Error::InvalidRev(format!("ambiguous commit prefix: {s}"))),
            Err(e) if e.code() == ErrorCode::NotFound => Err(Error::not_found("ref")),
            Err(e) => Err(e.into()),
        };
    }
    Err(Error::not_found("ref"))
}

/// Short name of the branch HEAD points at (also when unborn); `None` when detached.
pub fn head_branch(repo: &Repository) -> Option<String> {
    let head = repo.find_reference("HEAD").ok()?;
    let target = head.symbolic_target_bytes()?;
    target.strip_prefix(HEADS.as_bytes()).map(lossy)
}

/// (full name, short name, peeled commit oid) of every ref under `prefix` whose target peels to a commit.
pub(crate) fn refs_under(repo: &Repository, prefix: &str) -> Result<Vec<(String, String, Oid)>> {
    let mut out = Vec::new();
    for r in repo.references_glob(&format!("{prefix}*"))? {
        let r = r?;
        if r.kind() == Some(git2::ReferenceType::Symbolic) {
            continue; // e.g. refs/remotes/origin/HEAD
        }
        let full = lossy(r.name_bytes());
        let Ok(c) = r.peel(ObjectType::Commit) else { continue };
        let short = full[prefix.len()..].to_string();
        out.push((full, short, c.id()));
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

fn items(list: Vec<(String, String, Oid)>, kind: RefKind, default: Option<&str>, short_prefix: &str) -> Vec<RefItem> {
    list.into_iter()
        .map(|(full, short, oid)| RefItem {
            is_default: kind == RefKind::Branch && Some(short.as_str()) == default,
            name: format!("{short_prefix}{short}"),
            full_name: full,
            kind,
            target: oid.to_string(),
        })
        .collect()
}

/// Branches (default first, then by name), `origin/*` remote branches, tags (by name).
pub fn list(repo: &Repository) -> Result<RefsList> {
    let default_branch = head_branch(repo);
    let d = default_branch.as_deref();
    let mut branches = items(refs_under(repo, HEADS)?, RefKind::Branch, d, "");
    branches.sort_by_key(|b| !b.is_default);
    Ok(RefsList {
        default_branch: default_branch.clone(),
        branches,
        remote_branches: items(refs_under(repo, ORIGIN)?, RefKind::RemoteBranch, None, "origin/"),
        tags: items(refs_under(repo, TAGS)?, RefKind::Tag, None, ""),
    })
}

/// Local + `origin/*` branches with their last commit (default first, then newest commit first). No ahead/behind.
pub fn branches(repo: &Repository) -> Result<Vec<BranchRow>> {
    let l = list(repo)?;
    let mut rows = Vec::new();
    for item in l.branches.into_iter().chain(l.remote_branches) {
        let c = repo.find_commit(Oid::from_str(&item.target)?)?;
        rows.push(BranchRow { last: summary_of(&c), item, ahead_behind: None });
    }
    rows.sort_by(|a, b| b.item.is_default.cmp(&a.item.is_default).then(b.last.committer.time.secs.cmp(&a.last.committer.time.secs)).then(a.item.name.cmp(&b.item.name)));
    Ok(rows)
}

/// Counts commits reachable from `from` but not from `hide`, stopping at `cap`. Cancellable every 256.
fn count_only(repo: &Repository, from: Oid, hide: Oid, cap: u32, cancel: &CancelToken) -> Result<(u32, bool)> {
    let mut w = repo.revwalk()?;
    w.push(from)?;
    w.hide(hide)?;
    let mut n = 0u32;
    for oid in w {
        oid?;
        if n.is_multiple_of(256) {
            cancel.check()?;
        }
        if n >= cap {
            return Ok((n, true));
        }
        n += 1;
    }
    Ok((n, false))
}

/// Ahead/behind of each branch name (as shown in `BranchRow.item.name`, e.g. `feature` or `origin/x`) relative to
/// `base`, each side capped at `cap`.
pub fn ahead_behind(repo: &Repository, base: &str, names: &[String], cap: u32, cancel: &CancelToken) -> Result<Vec<(String, AheadBehind)>> {
    let base_oid = Oid::from_str(&resolve(repo, &crate::validate::rev(base)?)?.commit)?;
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        cancel.check()?;
        let tip = Oid::from_str(&resolve(repo, &crate::validate::rev(name)?)?.commit)?;
        let (ahead, c1) = count_only(repo, tip, base_oid, cap, cancel)?;
        let (behind, c2) = count_only(repo, base_oid, tip, cap, cancel)?;
        out.push((name.clone(), AheadBehind { ahead, behind, capped: c1 || c2 }));
    }
    Ok(out)
}

/// Tags newest first (tagger date for annotated, else commit date; ties by name), with annotated flag.
pub fn tags(repo: &Repository) -> Result<Vec<TagRow>> {
    let mut rows = Vec::new();
    for r in repo.references_glob(&format!("{TAGS}*"))? {
        let r = r?;
        let full = lossy(r.name_bytes());
        let Some(target) = r.target() else { continue };
        let Ok(commit) = r.peel_to_commit() else { continue };
        let tag = repo.find_tag(target).ok();
        let tagger = tag.as_ref().and_then(|t| t.tagger().map(|s| crate::commits::signature_of(&s)));
        let summary = summary_of(&commit);
        let date = tagger.as_ref().map(|s| s.time.secs).unwrap_or(summary.committer.time.secs);
        rows.push(TagRow {
            item: RefItem {
                name: full[TAGS.len()..].to_string(),
                full_name: full,
                kind: RefKind::Tag,
                target: commit.id().to_string(),
                is_default: false,
            },
            commit: summary,
            annotated: tag.is_some(),
            tagger,
            date,
        });
    }
    rows.sort_by(|a, b| b.date.cmp(&a.date).then(b.item.name.cmp(&a.item.name)));
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{self, T0};
    use crate::validate::rev;

    fn r(repo: &Repository, s: &str) -> Result<ResolvedRev> {
        resolve(repo, &rev(s)?)
    }

    fn fixture(t: &std::path::Path) -> (Repository, Oid, Oid) {
        let mut b = testkit::repo(&t.join("r"));
        b.bare().file("a", b"1").commit("one", T0);
        let one = b.head();
        b.tag("same", false).tag("v1", true);
        b.branch("same").file("a", b"2").commit("two on branch same", T0 + 60);
        let two = b.head();
        b.branch("dead").checkout("main");
        b.git().reference("refs/remotes/origin/main", one, true, "remote").unwrap();
        b.git().reference_symbolic("refs/remotes/origin/HEAD", "refs/remotes/origin/main", true, "remote head").unwrap();
        (testkit::open(&t.join("r")), one, two)
    }

    #[test]
    fn resolution_order() {
        let t = tempfile::tempdir().unwrap();
        let (repo, one, two) = fixture(t.path());
        // branch beats same-name tag
        let x = r(&repo, "same").unwrap();
        assert_eq!((x.kind, x.commit.clone()), (RefKind::Branch, two.to_string()));
        assert_eq!(x.full_name.as_deref(), Some("refs/heads/same"));
        // refs/tags/x reaches the tag
        let x = r(&repo, "refs/tags/same").unwrap();
        assert_eq!((x.kind, x.commit), (RefKind::Tag, one.to_string()));
        // annotated tag peels to the commit
        let x = r(&repo, "v1").unwrap();
        assert_eq!((x.kind, x.commit.clone()), (RefKind::Tag, one.to_string()));
        assert_eq!(x.tree, repo.find_commit(one).unwrap().tree_id().to_string());
        // remote
        let x = r(&repo, "origin/main").unwrap();
        assert_eq!((x.kind, x.full_name.as_deref()), (RefKind::RemoteBranch, Some("refs/remotes/origin/main")));
        // hex-looking branch name resolves as the branch, not as an oid prefix
        let x = r(&repo, "dead").unwrap();
        assert_eq!((x.kind, x.commit), (RefKind::Branch, two.to_string()));
        // HEAD
        let x = r(&repo, "HEAD").unwrap();
        assert_eq!((x.kind, x.full_name.as_deref(), x.commit), (RefKind::Branch, Some("refs/heads/main"), one.to_string()));
        // oid prefix and full oid
        let x = r(&repo, &two.to_string()[..10]).unwrap();
        assert_eq!((x.kind, x.commit), (RefKind::Commit, two.to_string()));
        let x = r(&repo, &one.to_string()).unwrap();
        assert_eq!(x.commit, one.to_string());
        // not found
        assert_eq!(r(&repo, "nope").unwrap_err().code(), "notFound");
        assert_eq!(r(&repo, "refs/heads/nope").unwrap_err().code(), "notFound");
        assert_eq!(r(&repo, "beef1234").unwrap_err().code(), "notFound");
    }

    #[test]
    fn ambiguous_prefix_is_invalid_rev() {
        let t = tempfile::tempdir().unwrap();
        let mut b = testkit::repo(&t.path().join("r"));
        b.bare();
        // Write blobs until two share a 4-hex prefix.
        let mut seen = std::collections::HashMap::new();
        let mut prefix = None;
        for i in 0..100_000u32 {
            let oid = b.git().blob(format!("{i}").as_bytes()).unwrap().to_string();
            if let Some(_prev) = seen.insert(oid[..4].to_string(), oid.clone()) {
                prefix = Some(oid[..4].to_string());
                break;
            }
        }
        let repo = testkit::open(&t.path().join("r"));
        let e = r(&repo, &prefix.unwrap()).unwrap_err();
        assert_eq!(e.code(), "invalidRev", "{e}");
    }

    #[test]
    fn listings() {
        let t = tempfile::tempdir().unwrap();
        let (repo, one, _two) = fixture(t.path());
        let l = list(&repo).unwrap();
        assert_eq!(l.default_branch.as_deref(), Some("main"));
        let names: Vec<_> = l.branches.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["main", "dead", "same"]);
        assert!(l.branches[0].is_default);
        assert_eq!(l.remote_branches.len(), 1, "origin/HEAD skipped");
        assert_eq!(l.remote_branches[0].name, "origin/main");
        assert_eq!(l.tags.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["same", "v1"]);
        assert_eq!(l.tags[1].target, one.to_string(), "peeled");

        let rows = branches(&repo).unwrap();
        assert_eq!(rows[0].item.name, "main");
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[1].last.summary, "two on branch same");

        let tg = tags(&repo).unwrap();
        let v1 = tg.iter().find(|t| t.item.name == "v1").unwrap();
        assert!(v1.annotated && v1.tagger.is_some());
        assert!(!tg.iter().find(|t| t.item.name == "same").unwrap().annotated);
    }

    #[test]
    fn tags_newest_first() {
        let t = tempfile::tempdir().unwrap();
        let mut b = testkit::repo(&t.path().join("r"));
        b.bare().file("a", b"1").commit("one", T0).tag("old", false);
        b.file("a", b"2").commit("two", T0 + 100).tag("new-annotated", true);
        b.file("a", b"3").commit("three", T0 + 200).tag("newest", false);
        let repo = testkit::open(&t.path().join("r"));
        let names: Vec<_> = tags(&repo).unwrap().into_iter().map(|t| t.item.name).collect();
        assert_eq!(names, ["newest", "new-annotated", "old"]);
    }

    #[test]
    fn ahead_behind_matches_git_rev_list() {
        let t = tempfile::tempdir().unwrap();
        let fx = testkit::merges(&t.path().join("m"));
        let repo = testkit::open(&fx.path);
        let names: Vec<String> = ["side", "drop", "rev"].iter().map(|s| s.to_string()).collect();
        let got = ahead_behind(&repo, "main", &names, 10_000, &CancelToken::new()).unwrap();
        // side and drop are merged into main: 0 ahead
        assert_eq!(got[0].1, AheadBehind { ahead: 0, behind: 9, capped: false });
        for (name, ab) in &got {
            if let Some(out) = testkit::git_cli(&fx.path, &["rev-list", "--left-right", "--count", &format!("{name}...main")]) {
                let v: Vec<u32> = out.split_whitespace().map(|n| n.parse().unwrap()).collect();
                assert_eq!((ab.ahead, ab.behind), (v[0], v[1]), "{name}");
            } else {
                eprintln!("git not on PATH: comparison skipped");
            }
        }
        // cap
        let capped = ahead_behind(&repo, "main", &names[..1], 3, &CancelToken::new()).unwrap();
        assert_eq!(capped[0].1, AheadBehind { ahead: 0, behind: 3, capped: true });
        // cancel
        let c = CancelToken::new();
        c.cancel();
        assert_eq!(ahead_behind(&repo, "main", &names, 10, &c).unwrap_err().code(), "cancelled");
    }
}
