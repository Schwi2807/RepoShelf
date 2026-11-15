//! `GithubSource`: `RepoSource` over the GitHub REST API (v0.3.0 Explore, D-122), plus search.
//!
//! Request budget (60/h anonymous), cold: repo page = repo (1) + resolve/latest/count (1) + root listing (1) +
//! README (1) + languages (1); branches / tags 1 + 1 when the picker opens. A branch / tag name is resolved to a
//! commit sha once (`commits?sha=<name>&per_page=1`, which also yields the root tree sha, the latest commit and —
//! from the `Link` header — the commit count); everything else is addressed by that sha, so listings, files and
//! commit pages never change and are never re-asked (`Freshness::Immutable`).

use super::json::{self, GhCommit, GhRepo, GhSearch, Link};
use super::source::{RepoSource, not_a};
use super::{Freshness, api_url, get, get_json};
use crate::Shelf;
use crate::error::{Error, Result};
use crate::ghreleases::{GhRelease, parse_releases, release_assets};
use crate::types::{
    BlobKind, BlobMeta, BlobText, CommitDetail, CommitSummary, CommitsPage, EntryKind, ExploreRefs, ExploreRepo, ExploreSearch, FileDiff, LanguageStat,
    Languages, LatestCommit, RefItem, RefKind, RefsList, Release, Rendered, ResolvedRev, TreeEntry, TreeListing,
};
use crate::validate::{self, GitPath, RevSpec};
use base64::Engine;
use serde::Deserialize;
use url::Url;

/// What the contents API can return as JSON (files above this are metadata only).
pub const ONLINE_MAX_BYTES: u64 = 1 << 20;
/// Commits per page (as the local list).
pub const PAGE: u32 = crate::commits::PAGE_LIMIT;
/// Pages of 100 read for branches / tags (300 each).
const REF_PAGES: u32 = 3;
/// Repositories per search page.
pub const SEARCH_PER_PAGE: u32 = 30;
/// GitHub shows at most the first 1,000 search results.
pub const SEARCH_MAX_PAGE: u32 = 34;

#[derive(Debug, Deserialize)]
struct GhContent {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    sha: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    encoding: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    submodule_git_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRef {
    #[serde(default)]
    name: String,
    #[serde(default)]
    commit: Option<json::GhParent>,
}

pub struct GithubSource<'a> {
    shelf: &'a Shelf,
    pub owner: String,
    pub name: String,
}

/// The id of the same github.com repository on the shelf.
fn on_shelf(s: &Shelf, owner: &str, name: &str) -> Option<i64> {
    crate::library::by_slug(s, owner, name).ok().filter(|r| r.origin_host.as_deref().is_some_and(|h| h.eq_ignore_ascii_case("github.com"))).map(|r| r.id)
}

fn entry_kind(t: &str) -> Option<EntryKind> {
    Some(match t {
        "dir" => EntryKind::Dir,
        "file" => EntryKind::File,
        "symlink" => EntryKind::Symlink,
        "submodule" => EntryKind::Submodule,
        _ => return None,
    })
}

fn good_name(n: &str) -> bool {
    !n.is_empty() && n != "." && n != ".." && n.len() <= 1024 && !n.contains(['/', '\\', '\0']) && !n.chars().any(char::is_control)
}

impl<'a> GithubSource<'a> {
    /// Validates the address (`validate::github_owner` / `github_repo`); nothing is requested yet.
    pub fn new(shelf: &'a Shelf, owner: &str, name: &str) -> Result<GithubSource<'a>> {
        validate::github_owner(owner)?;
        validate::github_repo(name)?;
        Ok(GithubSource { shelf, owner: owner.to_string(), name: name.to_string() })
    }

    fn url(&self, tail: &[&str], query: &[(&str, &str)]) -> Url {
        let mut seg: Vec<&str> = vec!["repos", &self.owner, &self.name];
        seg.extend_from_slice(tail);
        api_url(&seg, query)
    }

    fn contents_url(&self, path: &GitPath, reference: &str) -> Url {
        let mut tail = vec!["contents"];
        tail.extend(path.as_str().split('/').filter(|s| !s.is_empty()));
        self.url(&tail, &[("ref", reference)])
    }

    /// `GET /repos/{o}/{r}`.
    pub fn repo(&self) -> Result<ExploreRepo> {
        let r: GhRepo = get_json(self.shelf, &self.url(&[], &[]), Freshness::Revalidate)?;
        let mut x = r.into_repo(None)?;
        x.on_shelf = on_shelf(self.shelf, &x.owner, &x.name);
        Ok(x)
    }

    /// `GET /repos/{o}/{r}/languages` → the language bar (top 6 + Other).
    pub fn languages(&self) -> Result<Languages> {
        let m: std::collections::HashMap<String, u64> = get_json(self.shelf, &self.url(&["languages"], &[]), Freshness::Revalidate)?;
        let mut v: Vec<(String, u64)> = m.into_iter().filter_map(|(k, n)| json::clean(Some(k), 60).map(|k| (k, n))).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let total: u64 = v.iter().map(|x| x.1).sum();
        let pct = |n: u64| if total == 0 { 0.0 } else { (n as f64 * 100.0 / total as f64) as f32 };
        let mut items: Vec<LanguageStat> =
            v.iter().take(crate::langs::TOP).map(|(name, n)| LanguageStat { name: name.clone(), bytes: *n, percent: pct(*n) }).collect();
        let rest: u64 = v.iter().skip(crate::langs::TOP).map(|x| x.1).sum();
        if rest > 0 {
            items.push(LanguageStat { name: crate::langs::OTHER.into(), bytes: rest, percent: pct(rest) });
        }
        Ok(Languages { tree: String::new(), items })
    }

    /// The newest commit reachable from `rev` (a name or a sha), with the commit count from the `Link` header.
    fn newest(&self, rev: &str) -> Result<(GhCommit, Link)> {
        let fresh = if json::is_oid(rev) { Freshness::Immutable } else { Freshness::Revalidate };
        let a = get(self.shelf, &self.url(&["commits"], &[("sha", rev), ("per_page", "1")]), fresh)?;
        let list: Vec<GhCommit> = a.json()?;
        let c = list.into_iter().next().ok_or_else(|| Error::not_found("commit"))?;
        Ok((c, a.link))
    }

    /// Branch or tag, when a branches / tags answer is already saved (no request); else a branch.
    fn ref_kind_saved(&self, name: &str) -> (RefKind, Option<String>) {
        let has = |tail: &str| -> bool {
            let u = self.url(&[tail], &[("per_page", "100")]);
            crate::explore::cache::get(&self.shelf.db, u.as_str())
                .ok()
                .flatten()
                .and_then(|e| serde_json::from_slice::<Vec<GhRef>>(&e.body).ok())
                .is_some_and(|v| v.iter().any(|r| r.name == name))
        };
        if has("branches") {
            (RefKind::Branch, Some(format!("refs/heads/{name}")))
        } else if has("tags") {
            (RefKind::Tag, Some(format!("refs/tags/{name}")))
        } else {
            (RefKind::Branch, Some(format!("refs/heads/{name}")))
        }
    }

    fn ref_pages(&self, tail: &str) -> Result<(Vec<GhRef>, bool)> {
        let mut out = Vec::new();
        for page in 1..=REF_PAGES {
            let p = page.to_string();
            let mut q = vec![("per_page", "100")];
            if page > 1 {
                q.push(("page", &p));
            }
            let a = get(self.shelf, &self.url(&[tail], &q), Freshness::Revalidate)?;
            out.extend(a.json::<Vec<GhRef>>()?);
            if !a.link.next {
                return Ok((out, false));
            }
        }
        Ok((out, true))
    }

    /// Branches (default first, then by name) and tags (GitHub's order), with the truncation flags.
    pub fn refs_ex(&self) -> Result<ExploreRefs> {
        let default = self.repo()?.default_branch;
        let (b, bt) = self.ref_pages("branches")?;
        let (t, tt) = self.ref_pages("tags")?;
        let item = |r: GhRef, kind: RefKind, prefix: &str| -> Option<RefItem> {
            let name = json::clean(Some(r.name), 255)?;
            let target = json::oid(&r.commit?.sha).ok()?;
            let is_default = kind == RefKind::Branch && default.as_deref() == Some(name.as_str());
            Some(RefItem { full_name: format!("{prefix}{name}"), name, kind, target, is_default })
        };
        let mut branches: Vec<RefItem> = b.into_iter().filter_map(|r| item(r, RefKind::Branch, crate::refs::HEADS)).collect();
        branches.sort_by(|a, b| b.is_default.cmp(&a.is_default).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        let tags: Vec<RefItem> = t.into_iter().filter_map(|r| item(r, RefKind::Tag, crate::refs::TAGS)).collect();
        Ok(ExploreRefs {
            refs: RefsList { default_branch: default, branches, remote_branches: Vec::new(), tags },
            branches_truncated: bt,
            tags_truncated: tt,
        })
    }

    /// One file / symlink / submodule entry (the contents API answers an array for a directory).
    fn fetch_file(&self, rev: &ResolvedRev, path: &GitPath) -> Result<GhContent> {
        if path.is_root() {
            return Err(not_a("file"));
        }
        let v: serde_json::Value = get_json(self.shelf, &self.contents_url(path, &rev.commit), Freshness::Immutable)?;
        if v.is_array() {
            return Err(not_a("file"));
        }
        serde_json::from_value(v).map_err(|e| Error::InvalidInput(format!("Unexpected GitHub answer: {e}")))
    }

    /// The decoded bytes, or `None` when GitHub sent no content (the file is over 1 MB).
    fn bytes_of(c: &GhContent) -> Result<Option<Vec<u8>>> {
        if c.size > ONLINE_MAX_BYTES {
            return Ok(None);
        }
        let (Some(text), Some("base64")) = (c.content.as_deref(), c.encoding.as_deref()) else {
            return Ok(if c.size == 0 { Some(Vec::new()) } else { None });
        };
        let clean: String = text.chars().filter(|ch| !ch.is_ascii_whitespace()).collect();
        let b = base64::engine::general_purpose::STANDARD.decode(clean).map_err(|_| Error::InvalidInput("Unexpected GitHub answer: bad file content".into()))?;
        if b.len() as u64 != c.size {
            return Err(Error::InvalidInput("Unexpected GitHub answer: the file content is incomplete".into()));
        }
        Ok(Some(b))
    }

    fn meta_and_bytes(&self, rev: &ResolvedRev, path: &GitPath) -> Result<(BlobMeta, Option<Vec<u8>>)> {
        let c = self.fetch_file(rev, path)?;
        let p = path.as_str();
        let oid = json::oid(&c.sha)?;
        let base = |kind: BlobKind, size: u64| BlobMeta {
            path: p.to_string(),
            oid: oid.clone(),
            size,
            kind,
            lines: None,
            sloc: None,
            lossy: false,
            highlight: false,
            displayable: false,
            lang_hint: None,
        };
        match c.kind.as_str() {
            "submodule" => return Ok((base(BlobKind::Submodule { url: json::clean(c.submodule_git_url.clone(), 500) }, 0), None)),
            "symlink" => {
                let target = json::clean(c.target.clone(), 4096).unwrap_or_default();
                return Ok((base(BlobKind::Symlink { target }, c.size), None));
            }
            "file" => {}
            _ => return Err(not_a("file")),
        }
        let size = c.size;
        let Some(bytes) = Self::bytes_of(&c)? else {
            // Over 1 MB: metadata only ("Too large to preview online — Save to shelf").
            let kind = match crate::blob::image_mime(p) {
                Some(m) => BlobKind::Image { mime: m.to_string() },
                None => BlobKind::Text,
            };
            return Ok((base(kind, size), None));
        };
        let head = &bytes[..bytes.len().min(crate::blob::SNIFF_BYTES)];
        let mut m = base(crate::blob::sniff(p, head, size), size);
        m.displayable = size <= ONLINE_MAX_BYTES;
        if m.kind == BlobKind::Text {
            m.highlight = size <= crate::blob::HIGHLIGHT_MAX;
            m.lang_hint = crate::blob::lang_hint(p, &bytes[..bytes.len().min(512)]);
            let (text, lossy) = crate::blob::decode(&bytes);
            let (lines, sloc) = crate::blob::count_lines(&text);
            m.lines = Some(lines);
            m.sloc = Some(sloc);
            m.lossy = lossy;
        }
        Ok((m, Some(bytes)))
    }

    /// `GET /repos/{o}/{r}/commits/{sha}` with the file list (immutable).
    fn commit_full(&self, hex: &str) -> Result<GhCommit> {
        let sha = json::oid(hex).map_err(|_| Error::InvalidRev("not a full commit id".into()))?;
        get_json(self.shelf, &self.url(&["commits", &sha], &[]), Freshness::Immutable)
    }

    /// Where the READMEs may live besides the listing's own (root only), as `README::render_readme`.
    fn readme_in(&self, rev: &ResolvedRev, path: &str) -> Result<Option<Rendered>> {
        let gp = validate::git_path(path)?;
        let c = match self.fetch_file(rev, &gp) {
            Ok(c) => c,
            Err(Error::NotFound { .. }) => return Ok(None),
            Err(e) => return Err(e),
        };
        if c.kind != "file" {
            return Ok(None);
        }
        match Self::bytes_of(&c)? {
            Some(b) => Ok(Some(crate::readme::render_bytes(&b, path)?)),
            None => Err(Error::Refused(format!("{path} is too large to render online"))),
        }
    }
}

impl RepoSource for GithubSource<'_> {
    fn resolve(&self, rev: &RevSpec) -> Result<ResolvedRev> {
        let input = rev.as_str();
        // "HEAD" is the default branch online.
        let name = if input == "HEAD" { self.repo()?.default_branch.ok_or_else(|| Error::not_found("default branch"))? } else { input.to_string() };
        let (c, _) = self.newest(&name)?;
        let (kind, full_name) = if json::is_oid(&name) { (RefKind::Commit, None) } else { self.ref_kind_saved(&name) };
        Ok(ResolvedRev { input: input.to_string(), kind, full_name, commit: json::oid(&c.sha)?, tree: c.tree_sha()? })
    }

    fn latest_commit(&self, rev: &ResolvedRev) -> Result<LatestCommit> {
        // Same request as `resolve` for a branch / tag name (one saved answer serves both).
        let by = if json::is_oid(&rev.input) || rev.input == "HEAD" { rev.commit.as_str() } else { rev.input.as_str() };
        let (c, link) = self.newest(by)?;
        Ok(LatestCommit { commit: c.summary()?, count: Some(u64::from(link.last_page.unwrap_or(1))) })
    }

    fn refs(&self) -> Result<RefsList> {
        Ok(self.refs_ex()?.refs)
    }

    fn tree(&self, rev: &ResolvedRev, path: &GitPath) -> Result<TreeListing> {
        let v: serde_json::Value = get_json(self.shelf, &self.contents_url(path, &rev.commit), Freshness::Immutable)?;
        let serde_json::Value::Array(items) = v else { return Err(not_a("directory")) };
        let mut entries = Vec::with_capacity(items.len());
        for it in items {
            let Ok(c) = serde_json::from_value::<GhContent>(it) else { continue };
            let Some(kind) = entry_kind(&c.kind) else { continue };
            let Ok(oid) = json::oid(&c.sha) else { continue };
            if !good_name(&c.name) {
                continue;
            }
            let full = crate::tree::join(path.as_str(), &c.name);
            entries.push(TreeEntry {
                storable_here: crate::winpath::path_ok_here(&full),
                name: c.name,
                path: full,
                path_raw: None,
                kind,
                oid,
                size: matches!(kind, EntryKind::File | EntryKind::Symlink).then_some(c.size),
            });
        }
        crate::tree::sort_entries(&mut entries);
        let readme = crate::readme::find_readme(&entries).map(|e| e.name.clone());
        Ok(TreeListing { rev: rev.clone(), path: path.as_str().to_string(), entries, readme })
    }

    fn blob_meta(&self, rev: &ResolvedRev, path: &GitPath) -> Result<BlobMeta> {
        Ok(self.meta_and_bytes(rev, path)?.0)
    }

    fn blob_text(&self, rev: &ResolvedRev, path: &GitPath) -> Result<BlobText> {
        let (meta, bytes) = self.meta_and_bytes(rev, path)?;
        let text = match (&meta.kind, bytes) {
            (BlobKind::Text, Some(b)) if meta.displayable => Some(crate::blob::decode(&b).0),
            _ => None,
        };
        Ok(BlobText { meta, text })
    }

    fn blob_bytes(&self, rev: &ResolvedRev, path: &GitPath, max: u64) -> Result<Vec<u8>> {
        let c = self.fetch_file(rev, path)?;
        match c.kind.as_str() {
            "file" => {}
            "submodule" => return Err(Error::Refused("A submodule has no content in this repository".into())),
            _ => return Err(not_a("file")),
        }
        let b = Self::bytes_of(&c)?.ok_or_else(|| Error::Refused("Too large to preview online. Save the repository to the shelf to open it.".into()))?;
        if b.len() as u64 > max {
            return Err(Error::Refused(format!("File is larger than {:.1} MB", max as f64 / (1024.0 * 1024.0))));
        }
        Ok(b)
    }

    fn readme(&self, rev: &ResolvedRev, dir: &GitPath) -> Result<Option<Rendered>> {
        let listing = self.tree(rev, dir)?;
        if let Some(e) = crate::readme::find_readme(&listing.entries) {
            return self.readme_in(rev, &e.path);
        }
        if dir.is_root() {
            for (folder, file) in [(".github", ".github/README.md"), ("docs", "docs/README.md")] {
                if listing.entries.iter().any(|e| e.kind == EntryKind::Dir && e.name == folder)
                    && let Some(r) = self.readme_in(rev, file)?
                {
                    return Ok(Some(r));
                }
            }
        }
        Ok(None)
    }

    fn render_file(&self, rev: &ResolvedRev, path: &GitPath) -> Result<Rendered> {
        let c = self.fetch_file(rev, path)?;
        if c.kind != "file" {
            return Err(not_a("file"));
        }
        match Self::bytes_of(&c)? {
            Some(b) => crate::readme::render_bytes(&b, path.as_str()),
            None => Err(Error::Refused(format!("{} is too large to render online", path.as_str()))),
        }
    }

    fn commits_page(&self, rev: &ResolvedRev, path: Option<&GitPath>, skip: u32) -> Result<CommitsPage> {
        if !skip.is_multiple_of(PAGE) {
            return Err(Error::InvalidInput(format!("skip must be a multiple of {PAGE} for an online repository")));
        }
        let page = (skip / PAGE + 1).to_string();
        let per = PAGE.to_string();
        let mut q = vec![("sha", rev.commit.as_str()), ("per_page", per.as_str()), ("page", page.as_str())];
        if let Some(p) = path.filter(|p| !p.is_root()) {
            q.push(("path", p.as_str()));
        }
        let a = get(self.shelf, &self.url(&["commits"], &q), Freshness::Immutable)?;
        let list: Vec<GhCommit> = a.json()?;
        let commits = list.iter().map(GhCommit::summary).collect::<Result<Vec<CommitSummary>>>()?;
        let n = commits.len() as u32;
        Ok(CommitsPage { tip: rev.commit.clone(), skip: skip + n, commits, has_more: a.link.next, scanned: n, capped: false })
    }

    fn commit_detail(&self, hex: &str) -> Result<CommitDetail> {
        let c = self.commit_full(hex)?;
        let files: Vec<_> = c.files.iter().enumerate().map(|(i, f)| f.change(i as u32)).collect();
        let (additions, deletions) = match &c.stats {
            Some(s) => (s.additions, s.deletions),
            None => (files.iter().map(|f| f.additions).sum(), files.iter().map(|f| f.deletions).sum()),
        };
        let deferred = additions.saturating_add(deletions) > json::MAX_COMMIT_LINES;
        let mut diffs = Vec::new();
        if !deferred {
            for (f, ch) in c.files.iter().zip(&files) {
                let Some(patch) = &f.patch else { continue };
                if ch.binary || ch.too_large || ch.additions + ch.deletions > json::MAX_FILE_LINES {
                    continue; // collapsed: "Load diff"
                }
                let (hunks, truncated) = json::parse_patch(patch, json::MAX_FILE_LINES);
                diffs.push(FileDiff { index: ch.index, hunks, truncated });
            }
        }
        Ok(CommitDetail {
            commit: c.summary()?,
            message: c.message(),
            parent_compared: c.parents.first().and_then(|p| json::oid(&p.sha).ok()),
            is_merge: c.parents.len() > 1,
            files,
            additions,
            deletions,
            diffs,
            diffs_deferred: deferred,
        })
    }

    fn commit_file_diff(&self, hex: &str, index: u32, force: bool) -> Result<FileDiff> {
        let c = self.commit_full(hex)?;
        let f = c.files.get(index as usize).ok_or_else(|| Error::InvalidInput("file index out of range".into()))?;
        let Some(patch) = &f.patch else {
            return Err(Error::Refused("GitHub did not include a diff for this file (binary or too large). Save the repository to the shelf to see it.".into()));
        };
        let max = if force { crate::diff::FORCE_MAX_LINES } else { json::MAX_FILE_LINES };
        let (hunks, truncated) = json::parse_patch(patch, max);
        Ok(FileDiff { index, hunks, truncated })
    }

    fn releases(&self) -> Result<Vec<Release>> {
        let a = get(self.shelf, &self.url(&["releases"], &[("per_page", "100")]), Freshness::Revalidate)?;
        let text = String::from_utf8(a.body).map_err(|_| Error::InvalidInput("GitHub sent non-UTF-8 JSON".into()))?;
        Ok(release_cards(&self.owner, &self.name, &parse_releases(&text)?))
    }
}

/// One card per published release, in GitHub's order (newest first); `latest` = the first stable one.
pub fn release_cards(owner: &str, name: &str, gh: &[GhRelease]) -> Vec<Release> {
    let latest = gh.iter().position(|r| !r.prerelease);
    let tags: Vec<Option<String>> = gh.iter().map(|g| json::clean(Some(g.tag_name.clone()), 255)).collect();
    let mut out = Vec::new();
    for (i, g) in gh.iter().enumerate() {
        let Some(tag) = tags[i].clone() else { continue };
        let enc = crate::assets::url_path_segment(&tag);
        let mut assets = release_assets(g);
        assets.retain(|a| a.url.starts_with("https://github.com/"));
        out.push(Release {
            commit: g.target_commitish.clone().filter(|c| json::is_oid(c)).map(|c| c.to_ascii_lowercase()).unwrap_or_default(),
            date: json::iso_secs(g.published_at.as_deref()).or(json::iso_secs(g.created_at.as_deref())).unwrap_or(0),
            title: json::clean(g.name.clone(), 200).unwrap_or_else(|| tag.clone()),
            notes_html: g.body.as_deref().filter(|b| !b.trim().is_empty()).map(crate::markdown::render_release_notes),
            prerelease: g.prerelease,
            latest: latest == Some(i),
            previous_tag: tags.iter().skip(i + 1).flatten().next().cloned(),
            gh_notes: true,
            html_url: g.html_url.clone().filter(|u| u.starts_with("https://github.com/")),
            source_zip_url: Some(format!("https://github.com/{owner}/{name}/archive/refs/tags/{enc}.zip")),
            source_tar_url: Some(format!("https://github.com/{owner}/{name}/archive/refs/tags/{enc}.tar.gz")),
            assets,
            tag,
        });
    }
    out
}

/// `GET /search/repositories` (counts against the 10-30 / min search bucket, not the core one).
pub fn search(s: &Shelf, query: &str, sort: Option<&str>, page: u32) -> Result<ExploreSearch> {
    let q = query.trim();
    if q.is_empty() || q.chars().count() > 256 || q.chars().any(char::is_control) {
        return Err(Error::InvalidInput("Enter 1–256 characters to search for.".into()));
    }
    if !(1..=SEARCH_MAX_PAGE).contains(&page) {
        return Err(Error::InvalidInput(format!("GitHub only shows the first {} search pages.", SEARCH_MAX_PAGE)));
    }
    if let Some(so) = sort
        && !["stars", "forks", "updated", "help-wanted-issues"].contains(&so)
    {
        return Err(Error::InvalidInput("Unknown sort order.".into()));
    }
    let per = SEARCH_PER_PAGE.to_string();
    let pg = page.to_string();
    let mut params = vec![("q", q), ("per_page", per.as_str()), ("page", pg.as_str())];
    if let Some(so) = sort {
        params.push(("sort", so));
        params.push(("order", "desc"));
    }
    let a = get(s, &api_url(&["search", "repositories"], &params), Freshness::Revalidate)?;
    let r: GhSearch = a.json()?;
    // An entry GitHub spells oddly is skipped; one bad row does not hide the page.
    let items: Vec<ExploreRepo> = r
        .items
        .into_iter()
        .filter_map(|x| x.into_repo(None).ok())
        .map(|mut x| {
            x.on_shelf = on_shelf(s, &x.owner, &x.name);
            x
        })
        .collect();
    Ok(ExploreSearch { query: q.to_string(), total_count: r.total_count, incomplete: r.incomplete_results, page, has_more: a.link.next && page < SEARCH_MAX_PAGE, items })
}
