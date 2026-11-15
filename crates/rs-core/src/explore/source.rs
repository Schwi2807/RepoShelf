//! `RepoSource` (D-122): the read operations the GitHub-style screens need, answered from the local git object
//! DB (`LocalSource`, a thin adapter over the existing modules) or from the GitHub API (`github::GithubSource`).
//! Both return the same wire types, so the screens do not care where the data came from. A parity test runs the
//! same assertions against both.
//!
//! What GitHub's API cannot give cheaply is simply absent from the trait: last commit per tree entry
//! (`LastCommits`), ahead/behind, "since previous tag" — the Explore commands do not offer them (see the M10a
//! handoff in docs/WORKLOG.md).

use crate::Shelf;
use crate::error::{Error, Result};
use crate::jobs::CancelToken;
use crate::types::{BlobMeta, BlobText, CommitDetail, CommitsPage, FileDiff, LatestCommit, RefsList, Release, Rendered, ResolvedRev, TreeListing};
use crate::validate::{GitPath, RevSpec};

pub trait RepoSource {
    fn resolve(&self, rev: &RevSpec) -> Result<ResolvedRev>;
    fn latest_commit(&self, rev: &ResolvedRev) -> Result<LatestCommit>;
    fn refs(&self) -> Result<RefsList>;
    fn tree(&self, rev: &ResolvedRev, path: &GitPath) -> Result<TreeListing>;
    fn blob_meta(&self, rev: &ResolvedRev, path: &GitPath) -> Result<BlobMeta>;
    fn blob_text(&self, rev: &ResolvedRev, path: &GitPath) -> Result<BlobText>;
    /// Raw bytes; `Refused` above `max`.
    fn blob_bytes(&self, rev: &ResolvedRev, path: &GitPath, max: u64) -> Result<Vec<u8>>;
    fn readme(&self, rev: &ResolvedRev, dir: &GitPath) -> Result<Option<Rendered>>;
    fn render_file(&self, rev: &ResolvedRev, path: &GitPath) -> Result<Rendered>;
    /// One page of 35 (`commits::PAGE_LIMIT`) starting `skip` commits in (a multiple of 35 online).
    fn commits_page(&self, rev: &ResolvedRev, path: Option<&GitPath>, skip: u32) -> Result<CommitsPage>;
    /// `oid`: 40 hex.
    fn commit_detail(&self, oid: &str) -> Result<CommitDetail>;
    fn commit_file_diff(&self, oid: &str, index: u32, force: bool) -> Result<FileDiff>;
    fn releases(&self) -> Result<Vec<Release>>;
}

/// The local git object DB of one shelf repository.
pub struct LocalSource<'a> {
    pub shelf: &'a Shelf,
    pub repo_id: i64,
}

impl<'a> LocalSource<'a> {
    pub fn new(shelf: &'a Shelf, repo_id: i64) -> LocalSource<'a> {
        LocalSource { shelf, repo_id }
    }
    fn open(&self) -> Result<git2::Repository> {
        Ok(crate::repo::open(self.shelf, self.repo_id)?.1)
    }
}

fn oid(hex: &str) -> Result<git2::Oid> {
    crate::validate::oid(hex)
}

impl RepoSource for LocalSource<'_> {
    fn resolve(&self, rev: &RevSpec) -> Result<ResolvedRev> {
        crate::refs::resolve(&self.open()?, rev)
    }
    fn latest_commit(&self, rev: &ResolvedRev) -> Result<LatestCommit> {
        crate::commits::latest(self.shelf, self.repo_id, &self.open()?, rev)
    }
    fn refs(&self) -> Result<RefsList> {
        crate::refs::list(&self.open()?)
    }
    fn tree(&self, rev: &ResolvedRev, path: &GitPath) -> Result<TreeListing> {
        crate::tree::list_dir(&self.open()?, rev, path)
    }
    fn blob_meta(&self, rev: &ResolvedRev, path: &GitPath) -> Result<BlobMeta> {
        crate::blob::meta(&self.open()?, rev, path)
    }
    fn blob_text(&self, rev: &ResolvedRev, path: &GitPath) -> Result<BlobText> {
        crate::blob::text(&self.open()?, rev, path)
    }
    fn blob_bytes(&self, rev: &ResolvedRev, path: &GitPath, max: u64) -> Result<Vec<u8>> {
        crate::blob::bytes(&self.open()?, rev, path, max)
    }
    fn readme(&self, rev: &ResolvedRev, dir: &GitPath) -> Result<Option<Rendered>> {
        crate::readme::render_readme(&self.open()?, rev, dir)
    }
    fn render_file(&self, rev: &ResolvedRev, path: &GitPath) -> Result<Rendered> {
        crate::readme::render_file(&self.open()?, rev, path)
    }
    fn commits_page(&self, rev: &ResolvedRev, path: Option<&GitPath>, skip: u32) -> Result<CommitsPage> {
        let mut q = crate::commits::PageQuery::new(oid(&rev.commit)?);
        q.path = path.filter(|p| !p.is_root()).cloned();
        q.skip = skip;
        crate::commits::page(&self.open()?, &q, &CancelToken::new())
    }
    fn commit_detail(&self, hex: &str) -> Result<CommitDetail> {
        crate::diff::commit_detail(&self.open()?, oid(hex)?, &crate::diff::DiffLimits::default())
    }
    fn commit_file_diff(&self, hex: &str, index: u32, force: bool) -> Result<FileDiff> {
        crate::diff::file_diff(&self.open()?, oid(hex)?, index, force)
    }
    fn releases(&self) -> Result<Vec<Release>> {
        crate::ghreleases::releases_with_cached(self.shelf, self.repo_id)
    }
}

/// The same refusal both sources use for a path that is not a file / directory of the right kind.
pub(crate) fn not_a(what: &str) -> Error {
    Error::not_found(what)
}
