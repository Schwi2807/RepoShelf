//! GitHub REST JSON → RepoShelf types (v0.3.0 Explore). Everything from the network is untrusted: strings are
//! cleaned and capped, URLs checked, hashes validated. Only the fields we use are declared (the rest is ignored).

use crate::error::{Error, Result};
use crate::types::{ChangeKind, CommitSummary, DiffLine, ExploreRepo, FileChange, Hunk, LineKind, Signature, Time};
use serde::Deserialize;

fn bad(what: &str) -> Error {
    Error::InvalidInput(format!("Unexpected GitHub answer: {what}"))
}

/// Parsed `Link` header: is there a next page, and which page is the last one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Link {
    pub next: bool,
    pub last_page: Option<u32>,
}

impl Link {
    pub fn parse(h: Option<&str>) -> Link {
        let mut l = Link::default();
        let Some(h) = h else { return l };
        for part in h.split(',') {
            let Some((target, rel)) = part.split_once(';') else { continue };
            let rel = rel.trim();
            let target = target.trim().trim_start_matches('<').trim_end_matches('>');
            if rel.contains("rel=\"next\"") {
                l.next = true;
            } else if rel.contains("rel=\"last\"") {
                l.last_page = url::Url::parse(target).ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "page").and_then(|(_, v)| v.parse().ok()));
            }
        }
        l
    }
}

/// `2024-05-17T09:30:12Z` / `…+09:00` / `…-05:30` (fractional seconds tolerated) → (Unix seconds, UTC offset minutes).
pub fn iso_time(s: &str) -> Option<Time> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !(b[10] == b'T' || b[10] == b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |a: usize, n: usize| s.get(a..a + n)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, sec) = (num(0, 4)?, num(5, 2)?, num(8, 2)?, num(11, 2)?, num(14, 2)?, num(17, 2)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let mut i = 19;
    if b.get(i) == Some(&b'.') {
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    }
    let offset_min = match b.get(i)? {
        b'Z' | b'z' => 0,
        sign @ (b'+' | b'-') => {
            let oh = s.get(i + 1..i + 3)?.parse::<i32>().ok()?;
            let om = s.get(i + 4..i + 6).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
            let m = oh * 60 + om;
            if *sign == b'-' { -m } else { m }
        }
        _ => return None,
    };
    // days from civil (Howard Hinnant)
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + h * 3600 + mi * 60 + sec - i64::from(offset_min) * 60;
    Some(Time { secs, offset_min })
}

pub fn iso_secs(s: Option<&str>) -> Option<i64> {
    s.and_then(iso_time).map(|t| t.secs)
}

/// Trims, drops control characters, caps the length; empty → `None`.
pub fn clean(v: Option<String>, max: usize) -> Option<String> {
    let t: String = v?.chars().filter(|c| !c.is_control()).take(max).collect();
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

/// A 40-hex object id, lower-cased.
pub fn oid(s: &str) -> Result<String> {
    if s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()) { Ok(s.to_ascii_lowercase()) } else { Err(bad("a commit id is not 40 hex digits")) }
}

pub fn is_oid(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------------------------------------------
// Repository

#[derive(Debug, Deserialize)]
pub struct GhOwner {
    #[serde(default)]
    pub login: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GhLicense {
    #[serde(default)]
    pub spdx_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GhRepo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub owner: Option<GhOwner>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub stargazers_count: u64,
    #[serde(default)]
    pub forks_count: u64,
    #[serde(default)]
    pub open_issues_count: u64,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub license: Option<GhLicense>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub pushed_at: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub fork: bool,
    #[serde(default)]
    pub private: bool,
}

/// Owner / repo name as GitHub spells them (checked with the same rules as the arguments).
fn names(r: &GhRepo) -> Result<(String, String)> {
    let owner = r.owner.as_ref().map(|o| o.login.as_str()).unwrap_or("");
    crate::validate::github_owner(owner).map_err(|_| bad("a repository without a valid owner"))?;
    crate::validate::github_repo(&r.name).map_err(|_| bad("a repository without a valid name"))?;
    Ok((owner.to_string(), r.name.clone()))
}

impl GhRepo {
    /// `on_shelf`: filled by the caller (a DB lookup).
    pub fn into_repo(self, on_shelf: Option<i64>) -> Result<ExploreRepo> {
        let (owner, name) = names(&self)?;
        let homepage = clean(self.homepage, 500).filter(|h| h.starts_with("https://") || h.starts_with("http://"));
        let avatar = self
            .owner
            .as_ref()
            .and_then(|o| o.avatar_url.clone())
            .filter(|u| url::Url::parse(u).ok().is_some_and(|u| u.scheme() == "https" && u.host_str() == Some("avatars.githubusercontent.com")));
        let license = self.license.and_then(|l| {
            let id = clean(l.spdx_id, 40).filter(|i| i != "NOASSERTION");
            id.or_else(|| clean(l.name, 80))
        });
        Ok(ExploreRepo {
            full_name: format!("{owner}/{name}"),
            html_url: format!("https://github.com/{owner}/{name}"),
            clone_url: format!("https://github.com/{owner}/{name}.git"),
            owner,
            name,
            description: clean(self.description, 1000),
            homepage,
            topics: self.topics.into_iter().filter_map(|t| clean(Some(t), 50)).take(20).collect(),
            default_branch: clean(self.default_branch, 255),
            stars: self.stargazers_count,
            forks: self.forks_count,
            open_issues: self.open_issues_count,
            language: clean(self.language, 60),
            license,
            size_kb: self.size,
            created_at: iso_secs(self.created_at.as_deref()),
            updated_at: iso_secs(self.updated_at.as_deref()),
            pushed_at: iso_secs(self.pushed_at.as_deref()),
            archived: self.archived,
            fork: self.fork,
            is_private: self.private,
            owner_avatar_url: avatar,
            on_shelf,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct GhSearch {
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub incomplete_results: bool,
    #[serde(default)]
    pub items: Vec<GhRepo>,
}

// ---------------------------------------------------------------------------------------------------------------
// Commits

#[derive(Debug, Deserialize)]
pub struct GhSig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GhTree {
    #[serde(default)]
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct GhCommitInner {
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub author: Option<GhSig>,
    #[serde(default)]
    pub committer: Option<GhSig>,
    #[serde(default)]
    pub tree: Option<GhTree>,
}

#[derive(Debug, Deserialize)]
pub struct GhParent {
    #[serde(default)]
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct GhStats {
    #[serde(default)]
    pub additions: u32,
    #[serde(default)]
    pub deletions: u32,
}

#[derive(Debug, Deserialize)]
pub struct GhFile {
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub additions: u32,
    #[serde(default)]
    pub deletions: u32,
    #[serde(default)]
    pub changes: u32,
    #[serde(default)]
    pub patch: Option<String>,
    #[serde(default)]
    pub previous_filename: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GhCommit {
    pub sha: String,
    pub commit: GhCommitInner,
    #[serde(default)]
    pub parents: Vec<GhParent>,
    #[serde(default)]
    pub stats: Option<GhStats>,
    #[serde(default)]
    pub files: Vec<GhFile>,
}

fn signature(s: Option<&GhSig>) -> Signature {
    let name = s.and_then(|s| clean(s.name.clone(), 200)).unwrap_or_else(|| "Unknown".into());
    let email = s.and_then(|s| clean(s.email.clone(), 200)).unwrap_or_default();
    let time = s.and_then(|s| s.date.as_deref()).and_then(iso_time).unwrap_or(Time { secs: 0, offset_min: 0 });
    Signature { name, email, time }
}

/// Longest first line shown as a commit summary.
const SUMMARY_MAX: usize = 500;
/// A commit message is capped here (GitHub allows more; the page shows text only).
const MESSAGE_MAX: usize = 64 * 1024;

impl GhCommit {
    pub fn summary(&self) -> Result<CommitSummary> {
        let first = self.commit.message.lines().next().unwrap_or("");
        let parents = self.parents.iter().map(|p| oid(&p.sha)).collect::<Result<Vec<_>>>()?;
        Ok(CommitSummary {
            oid: oid(&self.sha)?,
            summary: first.chars().filter(|c| !c.is_control()).take(SUMMARY_MAX).collect(),
            author: signature(self.commit.author.as_ref()),
            committer: signature(self.commit.committer.as_ref().or(self.commit.author.as_ref())),
            parents,
        })
    }

    pub fn message(&self) -> String {
        self.commit.message.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\t')).take(MESSAGE_MAX).collect()
    }

    pub fn tree_sha(&self) -> Result<String> {
        oid(self.commit.tree.as_ref().map(|t| t.sha.as_str()).unwrap_or(""))
    }
}

pub fn change_kind(status: &str) -> ChangeKind {
    match status {
        "added" => ChangeKind::Added,
        "removed" => ChangeKind::Deleted,
        "renamed" => ChangeKind::Renamed,
        "copied" => ChangeKind::Copied,
        "changed" => ChangeKind::TypeChange,
        _ => ChangeKind::Modified,
    }
}

/// Files above this many changed lines in one commit are not shown inline (as `diff::DiffLimits`).
pub const MAX_COMMIT_LINES: u32 = 20_000;
pub const MAX_FILE_LINES: u32 = 2_000;

impl GhFile {
    /// `index` = position in GitHub's `files` array (what `explore_commit_file_diff` asks for).
    pub fn change(&self, index: u32) -> FileChange {
        let kind = change_kind(&self.status);
        let path = clean(Some(self.filename.clone()), 4096).unwrap_or_default();
        let prev = clean(self.previous_filename.clone(), 4096).unwrap_or_else(|| path.clone());
        let no_patch = self.patch.is_none();
        FileChange {
            index,
            kind,
            old_path: (kind != ChangeKind::Added).then(|| prev.clone()),
            new_path: (kind != ChangeKind::Deleted).then(|| path.clone()),
            additions: self.additions,
            deletions: self.deletions,
            // No `patch` and no counted lines: a binary file (an empty or renamed-only file looks the same).
            binary: no_patch && self.changes == 0 && matches!(kind, ChangeKind::Added | ChangeKind::Modified | ChangeKind::Deleted | ChangeKind::TypeChange),
            // No `patch` but lines were counted: GitHub left the diff out because it is too large.
            too_large: no_patch && self.changes > 0,
            generated: crate::diff::is_generated(&path),
        }
    }
}

/// Unified-diff `patch` text of one file (hunks only, no `---`/`+++` header) → hunks. Stops after `max_lines`
/// diff lines (`true` = truncated). Lines are cleaned of a trailing CR.
pub fn parse_patch(patch: &str, max_lines: u32) -> (Vec<Hunk>, bool) {
    let mut hunks: Vec<Hunk> = Vec::new();
    let (mut old_no, mut new_no) = (0u32, 0u32);
    let mut emitted = 0u32;
    for raw in patch.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(rest) = line.strip_prefix("@@ ") {
            // @@ -a[,b] +c[,d] @@ section
            let (a, c) = rest.split_once(" @@").map(|(h, _)| h).unwrap_or(rest).split_once(' ').unwrap_or((rest, ""));
            let start = |s: &str| s.trim_start_matches(['-', '+']).split(',').next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(1);
            old_no = start(a);
            new_no = start(c);
            hunks.push(Hunk { header: line.to_string(), lines: Vec::new() });
            continue;
        }
        let Some(h) = hunks.last_mut() else { continue };
        let (kind, text) = match line.as_bytes().first() {
            Some(b'+') => (LineKind::Add, &line[1..]),
            Some(b'-') => (LineKind::Del, &line[1..]),
            Some(b'\\') => {
                h.lines.push(DiffLine { kind: LineKind::NoNewline, old_no: None, new_no: None, text: crate::diff::NO_NEWLINE.into() });
                continue;
            }
            Some(b' ') => (LineKind::Ctx, &line[1..]),
            // the final empty piece after the last newline, or a context line whose space was trimmed
            None | Some(_) => {
                if raw.is_empty() {
                    continue;
                }
                (LineKind::Ctx, line)
            }
        };
        if emitted >= max_lines {
            hunks.retain(|h| !h.lines.is_empty());
            return (hunks, true);
        }
        let (o, n) = match kind {
            LineKind::Add => (None, Some(new_no)),
            LineKind::Del => (Some(old_no), None),
            _ => (Some(old_no), Some(new_no)),
        };
        if o.is_some() {
            old_no += 1;
        }
        if n.is_some() {
            new_no += 1;
        }
        let h = hunks.last_mut().expect("a hunk header came first");
        h.lines.push(DiffLine { kind, old_no: o, new_no: n, text: text.to_string() });
        emitted += 1;
    }
    (hunks, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_header() {
        let l = Link::parse(Some(
            "<https://api.github.com/repositories/1/commits?per_page=1&page=2>; rel=\"next\", <https://api.github.com/repositories/1/commits?per_page=1&page=1234>; rel=\"last\"",
        ));
        assert_eq!(l, Link { next: true, last_page: Some(1234) });
        assert_eq!(Link::parse(Some("<https://x/y?page=1>; rel=\"prev\", <https://x/y?page=1>; rel=\"first\"")), Link::default());
        assert_eq!(Link::parse(None), Link::default());
        assert_eq!(Link::parse(Some("garbage")), Link::default());
    }

    #[test]
    fn iso_dates() {
        let t = |s: &str| iso_time(s).map(|t| (t.secs, t.offset_min));
        assert_eq!(t("1970-01-01T00:00:00Z"), Some((0, 0)));
        assert_eq!(t("2011-04-14T16:00:49Z"), Some((1_302_796_849, 0)));
        assert_eq!(t("2011-04-14T16:00:49.123Z"), Some((1_302_796_849, 0)));
        assert_eq!(t("2011-04-14T16:00:49+09:00"), Some((1_302_796_849 - 9 * 3600, 540)));
        assert_eq!(t("2011-04-14T16:00:49-05:30"), Some((1_302_796_849 + 5 * 3600 + 1800, -330)));
        assert_eq!(t("2024-02-29T12:00:00Z"), Some((1_709_208_000, 0)), "leap day");
        assert_eq!(t("2000-03-01T00:00:00Z"), Some((951_868_800, 0)));
        for bad in ["", "yesterday", "2011-13-14T16:00:49Z", "2011-04-14T25:00:49Z", "2011-04-14 16:00:49", "2011-04-14T16:00:49"] {
            assert_eq!(t(bad), None, "{bad}");
        }
    }

    #[test]
    fn patch_hunks() {
        let p = "@@ -1,3 +1,4 @@ fn main\n a\n-b\n+B\n+c\n d\n\\ No newline at end of file\n@@ -10 +11,2 @@\n x\n+y";
        let (h, trunc) = parse_patch(p, 100);
        assert!(!trunc);
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].header, "@@ -1,3 +1,4 @@ fn main");
        let l = &h[0].lines;
        assert_eq!((l[0].kind, l[0].old_no, l[0].new_no, l[0].text.as_str()), (LineKind::Ctx, Some(1), Some(1), "a"));
        assert_eq!((l[1].kind, l[1].old_no, l[1].new_no), (LineKind::Del, Some(2), None));
        assert_eq!((l[2].kind, l[2].old_no, l[2].new_no, l[2].text.as_str()), (LineKind::Add, None, Some(2), "B"));
        assert_eq!((l[3].kind, l[3].new_no), (LineKind::Add, Some(3)));
        assert_eq!((l[4].kind, l[4].old_no, l[4].new_no), (LineKind::Ctx, Some(3), Some(4)));
        assert_eq!(l[5].kind, LineKind::NoNewline);
        assert_eq!((h[1].lines[0].old_no, h[1].lines[0].new_no), (Some(10), Some(11)));
        assert_eq!(h[1].lines[1].new_no, Some(12));
        // a "--" line inside a hunk is a deleted "-" line, CRs are dropped
        let (h, _) = parse_patch("@@ -1,2 +1,1 @@\n--x\r\n y", 100);
        assert_eq!((h[0].lines[0].kind, h[0].lines[0].text.as_str()), (LineKind::Del, "-x"));
        // truncation
        let long = format!("@@ -1,300 +1,300 @@\n{}", " l\n".repeat(300));
        let (h, trunc) = parse_patch(&long, 10);
        assert!(trunc);
        assert_eq!(h[0].lines.len(), 10);
        assert!(parse_patch("no hunk header", 10).0.is_empty());
    }

    #[test]
    fn clean_and_oid() {
        assert_eq!(clean(Some("  a\u{0}b\u{1b}  ".into()), 10).as_deref(), Some("ab"));
        assert_eq!(clean(Some("xxxxx".into()), 3).as_deref(), Some("xxx"));
        assert_eq!(clean(Some("  ".into()), 3), None);
        assert!(oid(&"A".repeat(40)).is_ok_and(|o| o == "a".repeat(40)));
        for bad in ["", "abc", &"g".repeat(40), &"a".repeat(41)] {
            assert_eq!(oid(bad).unwrap_err().code(), "invalidInput");
        }
    }
}
