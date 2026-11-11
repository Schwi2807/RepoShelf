//! Shared data types (ARCH §3.3), mirrored 1:1 in `src/ipc/types.ts` (M4.12).
//!
//! Wire shape (D-32): structs use camelCase fields. Unit-only enums serialise as camelCase strings
//! (`"imported"`). Enums with data are internally tagged with `"type"` and camelCase fields
//! (`{"type":"corrupt","message":"…"}`, unit variants `{"type":"ok"}`), so TS gets discriminated unions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Imported,
    Cloned,
}

impl Source {
    pub fn as_db(self) -> &'static str {
        match self {
            Source::Imported => "imported",
            Source::Cloned => "cloned",
        }
    }
    pub fn from_db(s: &str) -> Option<Source> {
        match s {
            "imported" => Some(Source::Imported),
            "cloned" => Some(Source::Cloned),
            _ => None,
        }
    }
}

/// Dashboard row (FR-1), all from DB — no git on list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub id: i64,
    pub owner: String,
    pub name: String,
    pub source: Source,
    pub path: String,
    pub description: Option<String>,
    pub topics: Vec<String>,
    pub homepage: Option<String>,
    pub origin_url: Option<String>,
    pub origin_host: Option<String>,
    pub default_branch: Option<String>,
    pub primary_language: Option<String>,
    pub last_commit_at: Option<i64>,
    pub added_at: i64,
    /// D-19
    pub cloned_at: Option<i64>,
    pub last_opened_at: Option<i64>,
    pub last_fetched_at: Option<i64>,
    pub status: RepoStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RepoStatus {
    Ok,
    Missing,
    Corrupt { message: String },
    Empty,
}

/// S2 header + About (FR-12, FR-18).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoDetail {
    pub summary: RepoSummary,
    pub is_bare: bool,
    pub head: HeadState,
    pub branch_count: u32,
    pub tag_count: u32,
    pub release_count: u32,
    pub license: Option<String>,
    pub fetchable: bool,
    pub can_fast_forward: Option<bool>,
    pub dirty: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum HeadState {
    Branch { name: String },
    Detached { oid: String },
    Unborn { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRev {
    pub input: String,
    pub kind: RefKind,
    pub full_name: Option<String>,
    pub commit: String,
    pub tree: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefKind {
    Branch,
    RemoteBranch,
    Tag,
    Commit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefItem {
    pub name: String,
    pub full_name: String,
    pub kind: RefKind,
    pub target: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefsList {
    pub default_branch: Option<String>,
    pub branches: Vec<RefItem>,
    pub remote_branches: Vec<RefItem>,
    pub tags: Vec<RefItem>,
}

/// Epoch seconds + UTC offset in minutes (formatted in TS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Time {
    pub secs: i64,
    pub offset_min: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub name: String,
    pub email: String,
    pub time: Time,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    pub oid: String,
    pub summary: String,
    pub author: Signature,
    pub committer: Signature,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestCommit {
    pub commit: CommitSummary,
    /// `None` = still computing, see `commit_count`.
    pub count: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    Dir,
    File,
    Exec,
    Symlink,
    Submodule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub path_raw: Option<Vec<u8>>,
    pub kind: EntryKind,
    pub oid: String,
    pub size: Option<u64>,
    pub storable_here: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeListing {
    pub rev: ResolvedRev,
    pub path: String,
    /// Sorted per FR-15.
    pub entries: Vec<TreeEntry>,
    /// README entry name.
    pub readme: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryCommit {
    pub name: String,
    /// `None` = older than the cap.
    pub commit: Option<CommitSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastCommits {
    pub tip: String,
    pub path: String,
    pub entries: Vec<EntryCommit>,
    pub capped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum BlobKind {
    Text,
    Image { mime: String },
    Binary,
    LfsPointer { size: Option<u64> },
    Submodule { url: Option<String> },
    Symlink { target: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobMeta {
    pub path: String,
    pub oid: String,
    pub size: u64,
    pub kind: BlobKind,
    pub lines: Option<u32>,
    pub sloc: Option<u32>,
    pub lossy: bool,
    /// size ≤ 1 MB
    pub highlight: bool,
    /// size ≤ 5 MB
    pub displayable: bool,
    pub lang_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobText {
    pub meta: BlobMeta,
    /// `None` if not displayable or not text.
    pub text: Option<String>,
}

/// Sanitised HTML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rendered {
    pub html: String,
    pub headings: Vec<Heading>,
    pub source_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub anchor: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageStat {
    pub name: String,
    pub bytes: u64,
    pub percent: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Languages {
    pub tree: String,
    /// Top 6 + "Other".
    pub items: Vec<LanguageStat>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitsPage {
    pub tip: String,
    pub skip: u32,
    pub commits: Vec<CommitSummary>,
    pub has_more: bool,
    pub scanned: u32,
    pub capped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
    TypeChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub index: u32,
    pub kind: ChangeKind,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub additions: u32,
    pub deletions: u32,
    pub binary: bool,
    pub too_large: bool,
    pub generated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineKind {
    Ctx,
    Add,
    Del,
    NoNewline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub index: u32,
    pub hunks: Vec<Hunk>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub commit: CommitSummary,
    pub message: String,
    pub parent_compared: Option<String>,
    pub is_merge: bool,
    pub files: Vec<FileChange>,
    pub additions: u32,
    pub deletions: u32,
    /// Inline unless over the commit caps.
    pub diffs: Vec<FileDiff>,
    pub diffs_deferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRow {
    pub item: RefItem,
    pub last: CommitSummary,
    pub ahead_behind: Option<AheadBehind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
    pub capped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagRow {
    pub item: RefItem,
    pub commit: CommitSummary,
    pub annotated: bool,
    pub tagger: Option<Signature>,
    pub date: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub tag: String,
    pub commit: String,
    pub date: i64,
    pub title: String,
    pub notes_html: Option<String>,
    pub prerelease: bool,
    pub latest: bool,
    pub previous_tag: Option<String>,
    pub gh_notes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub path: String,
    pub name: String,
    pub is_bare: bool,
    pub origin_url: Option<String>,
    pub duplicate_of: Option<i64>,
    /// D-18: opaque token for `import_confirm`; filled in by src-tauri (empty from rs-core).
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClonePreview {
    pub url: String,
    pub host: String,
    pub owner: String,
    pub name: String,
    pub target_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum JobResult {
    Cloned { repo_id: i64, skipped_refs: Vec<String> },
    Fetched(FetchOutcome),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchOutcome {
    pub new_commits: u32,
    pub new_tags: Vec<String>,
    pub kept_local_tags: Vec<String>,
    pub skipped_refs: Vec<String>,
    pub updated_branches: Vec<String>,
}

/// src-tauri trashes managed clones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Removed {
    pub path: String,
    pub source: Source,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortSpec {
    pub key: SortKey,
    pub desc: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        SortSpec { key: SortKey::Updated, desc: true }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    #[default]
    Updated,
    Name,
    Added,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceFilter {
    #[default]
    All,
    Imported,
    Cloned,
}

/// Release ordering (ordering itself lands in `releases.rs`, M2.11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub extra: u64,
    pub pre: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub archive_dir: String,
    pub cache_bytes: u64,
    pub repo_count: u32,
    pub archive_bytes: Option<u64>,
}

#[cfg(any(test, feature = "testkit"))]
pub mod samples {
    //! One sample of every type / enum shape (also the seed for the M4 IPC golden, src-tauri `ipc_samples`).
    use super::*;

    pub fn sig() -> Signature {
        Signature {
            name: "A U Thor".into(),
            email: "author@example.com".into(),
            time: Time { secs: 1_700_000_000, offset_min: 60 },
        }
    }
    pub fn commit() -> CommitSummary {
        CommitSummary {
            oid: "a".repeat(40),
            summary: "Initial".into(),
            author: sig(),
            committer: sig(),
            parents: vec!["b".repeat(40)],
        }
    }
    pub fn summary() -> RepoSummary {
        RepoSummary {
            id: 1,
            owner: "local".into(),
            name: "demo".into(),
            source: Source::Imported,
            path: "C:\\x\\demo".into(),
            description: Some("d".into()),
            topics: vec!["rust".into()],
            homepage: None,
            origin_url: Some("https://github.com/o/demo.git".into()),
            origin_host: Some("github.com".into()),
            default_branch: Some("main".into()),
            primary_language: Some("Rust".into()),
            last_commit_at: Some(1),
            added_at: 2,
            cloned_at: None,
            last_opened_at: Some(3),
            last_fetched_at: None,
            status: RepoStatus::Corrupt { message: "bad".into() },
        }
    }
    pub fn ref_item() -> RefItem {
        RefItem {
            name: "main".into(),
            full_name: "refs/heads/main".into(),
            kind: RefKind::RemoteBranch,
            target: "c".repeat(40),
            is_default: true,
        }
    }
    pub fn resolved() -> ResolvedRev {
        ResolvedRev {
            input: "main".into(),
            kind: RefKind::Branch,
            full_name: Some("refs/heads/main".into()),
            commit: "a".repeat(40),
            tree: "d".repeat(40),
        }
    }
    pub fn blob_meta(kind: BlobKind) -> BlobMeta {
        BlobMeta {
            path: "a.rs".into(),
            oid: "e".repeat(40),
            size: 10,
            kind,
            lines: Some(1),
            sloc: Some(1),
            lossy: false,
            highlight: true,
            displayable: true,
            lang_hint: Some("rust".into()),
        }
    }

    /// (name, JSON) for every type and every enum variant.
    pub fn all() -> Vec<(&'static str, serde_json::Value)> {
        use serde_json::to_value as v;
        let tree_entry = TreeEntry {
            name: "a".into(),
            path: "src/a".into(),
            path_raw: Some(vec![0xff]),
            kind: EntryKind::Submodule,
            oid: "f".repeat(40),
            size: Some(3),
            storable_here: false,
        };
        let line = DiffLine { kind: LineKind::NoNewline, old_no: Some(1), new_no: None, text: "x".into() };
        let file_diff = FileDiff { index: 0, hunks: vec![Hunk { header: "@@".into(), lines: vec![line.clone()] }], truncated: false };
        let outcome = FetchOutcome {
            new_commits: 2,
            new_tags: vec!["v1".into()],
            kept_local_tags: vec![],
            skipped_refs: vec!["LLM/x".into()],
            updated_branches: vec!["main".into()],
        };
        let mut out = vec![
            ("RepoSummary", v(summary()).unwrap()),
            ("RepoStatus.ok", v(RepoStatus::Ok).unwrap()),
            ("RepoStatus.missing", v(RepoStatus::Missing).unwrap()),
            ("RepoStatus.empty", v(RepoStatus::Empty).unwrap()),
            (
                "RepoDetail",
                v(RepoDetail {
                    summary: summary(),
                    is_bare: true,
                    head: HeadState::Unborn { name: "main".into() },
                    branch_count: 1,
                    tag_count: 2,
                    release_count: 3,
                    license: Some("MIT".into()),
                    fetchable: true,
                    can_fast_forward: Some(false),
                    dirty: None,
                })
                .unwrap(),
            ),
            ("HeadState.branch", v(HeadState::Branch { name: "main".into() }).unwrap()),
            ("HeadState.detached", v(HeadState::Detached { oid: "a".repeat(40) }).unwrap()),
            ("ResolvedRev", v(resolved()).unwrap()),
            ("RefsList", v(RefsList { default_branch: Some("main".into()), branches: vec![ref_item()], remote_branches: vec![], tags: vec![] }).unwrap()),
            ("LatestCommit", v(LatestCommit { commit: commit(), count: Some(5) }).unwrap()),
            ("TreeEntry", v(tree_entry.clone()).unwrap()),
            ("TreeListing", v(TreeListing { rev: resolved(), path: "".into(), entries: vec![tree_entry], readme: Some("README.md".into()) }).unwrap()),
            (
                "LastCommits",
                v(LastCommits { tip: "a".repeat(40), path: "".into(), entries: vec![EntryCommit { name: "a".into(), commit: Some(commit()) }], capped: true })
                    .unwrap(),
            ),
            ("BlobKind.text", v(BlobKind::Text).unwrap()),
            ("BlobKind.binary", v(BlobKind::Binary).unwrap()),
            ("BlobKind.lfsPointer", v(BlobKind::LfsPointer { size: Some(9) }).unwrap()),
            ("BlobKind.submodule", v(BlobKind::Submodule { url: None }).unwrap()),
            ("BlobKind.symlink", v(BlobKind::Symlink { target: "b".into() }).unwrap()),
            ("BlobMeta", v(blob_meta(BlobKind::Image { mime: "image/png".into() })).unwrap()),
            ("BlobText", v(BlobText { meta: blob_meta(BlobKind::Text), text: Some("x".into()) }).unwrap()),
            (
                "Rendered",
                v(Rendered { html: "<h1>x</h1>".into(), headings: vec![Heading { level: 1, text: "x".into(), anchor: "x".into() }], source_path: "README.md".into() })
                    .unwrap(),
            ),
            ("Languages", v(Languages { tree: "d".repeat(40), items: vec![LanguageStat { name: "Rust".into(), bytes: 10, percent: 100.0 }] }).unwrap()),
            ("CommitsPage", v(CommitsPage { tip: "a".repeat(40), skip: 0, commits: vec![commit()], has_more: true, scanned: 35, capped: false }).unwrap()),
            (
                "FileChange",
                v(FileChange {
                    index: 0,
                    kind: ChangeKind::TypeChange,
                    old_path: Some("a".into()),
                    new_path: Some("b".into()),
                    additions: 1,
                    deletions: 2,
                    binary: false,
                    too_large: false,
                    generated: true,
                })
                .unwrap(),
            ),
            ("DiffLine", v(line).unwrap()),
            ("FileDiff", v(file_diff.clone()).unwrap()),
            (
                "CommitDetail",
                v(CommitDetail {
                    commit: commit(),
                    message: "m".into(),
                    parent_compared: Some("b".repeat(40)),
                    is_merge: false,
                    files: vec![],
                    additions: 0,
                    deletions: 0,
                    diffs: vec![file_diff],
                    diffs_deferred: false,
                })
                .unwrap(),
            ),
            ("BranchRow", v(BranchRow { item: ref_item(), last: commit(), ahead_behind: Some(AheadBehind { ahead: 1, behind: 2, capped: false }) }).unwrap()),
            ("TagRow", v(TagRow { item: ref_item(), commit: commit(), annotated: true, tagger: Some(sig()), date: 5 }).unwrap()),
            (
                "Release",
                v(Release {
                    tag: "v1.0.0".into(),
                    commit: "a".repeat(40),
                    date: 1,
                    title: "v1".into(),
                    notes_html: None,
                    prerelease: false,
                    latest: true,
                    previous_tag: None,
                    gh_notes: false,
                })
                .unwrap(),
            ),
            (
                "ImportPreview",
                v(ImportPreview { path: "C:\\r".into(), name: "r".into(), is_bare: false, origin_url: None, duplicate_of: Some(1), token: "t".into() }).unwrap(),
            ),
            (
                "ClonePreview",
                v(ClonePreview { url: "https://github.com/o/r.git".into(), host: "github.com".into(), owner: "o".into(), name: "r".into(), target_dir: "x".into() })
                    .unwrap(),
            ),
            ("JobResult.cloned", v(JobResult::Cloned { repo_id: 4, skipped_refs: vec![] }).unwrap()),
            ("JobResult.fetched", v(JobResult::Fetched(outcome)).unwrap()),
            ("Removed", v(Removed { path: "p".into(), source: Source::Cloned }).unwrap()),
            ("SortSpec", v(SortSpec { key: SortKey::Added, desc: false }).unwrap()),
            ("Version", v(Version { major: 1, minor: 2, patch: 3, extra: 0, pre: Some("rc1".into()) }).unwrap()),
            (
                "AppInfo",
                v(AppInfo { version: "0.0.1".into(), data_dir: "d".into(), archive_dir: "a".into(), cache_bytes: 1, repo_count: 2, archive_bytes: None }).unwrap(),
            ),
        ];
        for t in [Theme::Light, Theme::Dark, Theme::System] {
            out.push(("Theme", v(t).unwrap()));
        }
        for s in [SourceFilter::All, SourceFilter::Imported, SourceFilter::Cloned] {
            out.push(("SourceFilter", v(s).unwrap()));
        }
        for k in [EntryKind::Dir, EntryKind::File, EntryKind::Exec, EntryKind::Symlink] {
            out.push(("EntryKind", v(k).unwrap()));
        }
        for k in [ChangeKind::Added, ChangeKind::Deleted, ChangeKind::Modified, ChangeKind::Renamed, ChangeKind::Copied] {
            out.push(("ChangeKind", v(k).unwrap()));
        }
        for k in [LineKind::Ctx, LineKind::Add, LineKind::Del] {
            out.push(("LineKind", v(k).unwrap()));
        }
        for k in [RefKind::Tag, RefKind::Commit] {
            out.push(("RefKind", v(k).unwrap()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn is_camel(s: &str) -> bool {
        let mut c = s.chars();
        c.next().is_some_and(|f| f.is_ascii_lowercase()) && c.all(|ch| ch.is_ascii_alphanumeric())
    }

    fn check(name: &str, v: &Value) {
        match v {
            Value::Object(m) => {
                for (k, inner) in m {
                    assert!(is_camel(k), "{name}: key `{k}` is not camelCase");
                    check(name, inner);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| check(name, x)),
            _ => {}
        }
    }

    #[test]
    fn every_sample_serialises_camel_case() {
        let all = samples::all();
        assert!(all.len() > 50);
        for (name, v) in &all {
            check(name, v);
            // Unit-only enums are plain camelCase strings; tagged enums carry a camelCase "type".
            if let Value::String(s) = v {
                assert!(is_camel(s), "{name}: value `{s}`");
            }
            if let Some(Value::String(t)) = v.get("type") {
                assert!(is_camel(t), "{name}: type `{t}`");
            }
        }
    }

    #[test]
    fn enum_shapes_are_as_documented() {
        use serde_json::json;
        assert_eq!(serde_json::to_value(Source::Imported).unwrap(), json!("imported"));
        assert_eq!(serde_json::to_value(RefKind::RemoteBranch).unwrap(), json!("remoteBranch"));
        assert_eq!(serde_json::to_value(LineKind::NoNewline).unwrap(), json!("noNewline"));
        assert_eq!(serde_json::to_value(ChangeKind::TypeChange).unwrap(), json!("typeChange"));
        assert_eq!(serde_json::to_value(RepoStatus::Ok).unwrap(), json!({"type": "ok"}));
        assert_eq!(
            serde_json::to_value(RepoStatus::Corrupt { message: "m".into() }).unwrap(),
            json!({"type": "corrupt", "message": "m"})
        );
        assert_eq!(
            serde_json::to_value(BlobKind::LfsPointer { size: Some(3) }).unwrap(),
            json!({"type": "lfsPointer", "size": 3})
        );
        assert_eq!(
            serde_json::to_value(JobResult::Cloned { repo_id: 4, skipped_refs: vec![] }).unwrap(),
            json!({"type": "cloned", "repoId": 4, "skippedRefs": []})
        );
        let f = serde_json::to_value(JobResult::Fetched(FetchOutcome { new_commits: 2, ..Default::default() })).unwrap();
        assert_eq!(f["type"], json!("fetched"));
        assert_eq!(f["newCommits"], json!(2));
        // round trip
        let s = samples::summary();
        let back: RepoSummary = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        assert!(serde_json::to_value(&s).unwrap().get("clonedAt").is_some());
        let p: ImportPreview = serde_json::from_value(json!({
            "path": "p", "name": "n", "isBare": false, "originUrl": null, "duplicateOf": null, "token": "abc"
        }))
        .unwrap();
        assert_eq!(p.token, "abc");
    }
}
