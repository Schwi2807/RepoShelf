//! README discovery and rendering (M2.10; FR-17, ARCH §6.6).

use crate::error::{Error, Result};
use crate::types::{EntryKind, Rendered, ResolvedRev, TreeEntry};
use crate::validate::GitPath;
use git2::Repository;

/// Case-insensitive priority order; then any `README.*`.
const PRIORITY: &[&str] = &["readme.md", "readme.markdown", "readme.mdown", "readme.mkd", "readme.rst", "readme.txt", "readme"];
/// Max README/markdown source rendered (larger files show the plain placeholder in the UI).
pub const MAX_RENDER_BYTES: usize = 1024 * 1024;

fn is_file(e: &TreeEntry) -> bool {
    matches!(e.kind, EntryKind::File | EntryKind::Exec)
}

/// The README entry of a listing (FR-17 priority, case-insensitive).
pub fn find_readme(entries: &[TreeEntry]) -> Option<&TreeEntry> {
    for want in PRIORITY {
        if let Some(e) = entries.iter().find(|e| is_file(e) && e.name.to_lowercase() == *want) {
            return Some(e);
        }
    }
    entries.iter().filter(|e| is_file(e) && e.name.to_lowercase().starts_with("readme.")).min_by_key(|e| e.name.to_lowercase())
}

/// Markdown by extension (`.md`, `.markdown`, `.mdown`, `.mkd`, `.mdx`).
pub fn is_markdown(path: &str) -> bool {
    let lower = path.to_lowercase();
    [".md", ".markdown", ".mdown", ".mkd", ".mdx"].iter().any(|x| lower.ends_with(x))
}

fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

/// Escaped `<pre class="readme-plain">` for non-markdown READMEs.
pub fn render_plain(text: &str, source_path: &str) -> Rendered {
    Rendered { html: format!("<pre class=\"readme-plain\">{}</pre>", escape(text)), headings: vec![], source_path: source_path.to_string() }
}

fn parent_dir(path: &str) -> &str {
    path.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

fn render_blob(repo: &Repository, oid: git2::Oid, path: &str) -> Result<Rendered> {
    let blob = repo.find_blob(oid)?;
    render_bytes(blob.content(), path)
}

/// Renders README / markdown bytes found at `path` (Markdown → sanitised HTML, anything else → escaped plain
/// text); shared by the git DB and Explore (GitHub API) so both pass through the same sanitiser.
pub fn render_bytes(content: &[u8], path: &str) -> Result<Rendered> {
    if content.len() > MAX_RENDER_BYTES {
        return Err(Error::Refused(format!("{path} is too large to render")));
    }
    let content = content.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(content);
    let text = String::from_utf8_lossy(content);
    if is_markdown(path) {
        let base = crate::validate::git_path(parent_dir(path))?;
        let mut r = crate::markdown::render(&text, &base);
        r.source_path = path.to_string();
        Ok(r)
    } else {
        Ok(render_plain(&text, path))
    }
}

/// README of directory `dir` at `rev` (at the root also `.github/README.md`, `docs/README.md`).
pub fn render_readme(repo: &Repository, rev: &ResolvedRev, dir: &GitPath) -> Result<Option<Rendered>> {
    let listing = crate::tree::list_dir(repo, rev, dir)?;
    if let Some(e) = find_readme(&listing.entries) {
        return Ok(Some(render_blob(repo, git2::Oid::from_str(&e.oid)?, &e.path)?));
    }
    if dir.is_root() {
        let root = crate::tree::root_tree(repo, rev)?;
        for p in [".github/README.md", "docs/README.md"] {
            let gp = crate::validate::git_path(p)?;
            if let Some((oid, mode)) = crate::tree::lookup(repo, &root, &gp)?
                && matches!(crate::tree::kind_of_mode(mode), EntryKind::File | EntryKind::Exec)
            {
                return Ok(Some(render_blob(repo, oid, p)?));
            }
        }
    }
    Ok(None)
}

/// Rendered preview of a single file (`.md` → markdown, anything else → plain).
pub fn render_file(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<Rendered> {
    let root = crate::tree::root_tree(repo, rev)?;
    match crate::tree::lookup(repo, &root, path)? {
        Some((oid, mode)) if matches!(crate::tree::kind_of_mode(mode), EntryKind::File | EntryKind::Exec) => {
            render_blob(repo, oid, path.as_str())
        }
        Some(_) => Err(Error::not_found("file")),
        None => Err(Error::not_found("path")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{self, T0};
    use crate::validate::{git_path, rev};

    fn entry(name: &str, kind: EntryKind) -> TreeEntry {
        TreeEntry { name: name.into(), path: name.into(), path_raw: None, kind, oid: "0".repeat(40), size: Some(1), storable_here: true }
    }

    #[test]
    fn priority_order() {
        let names = |v: &[&str]| v.iter().map(|n| entry(n, EntryKind::File)).collect::<Vec<_>>();
        let pick = |v: &[&str]| find_readme(&names(v)).map(|e| e.name.clone());
        assert_eq!(pick(&["readme.txt", "README.md", "README"]), Some("README.md".into()));
        assert_eq!(pick(&["Readme.Markdown", "README.rst"]), Some("Readme.Markdown".into()));
        assert_eq!(pick(&["README.rst", "readme.txt"]), Some("README.rst".into()));
        assert_eq!(pick(&["README", "README.zh.md"]), Some("README".into()));
        assert_eq!(pick(&["README.zh.md", "README.de.md"]), Some("README.de.md".into()));
        assert_eq!(pick(&["main.rs", "READMEX"]), None);
        // a directory named README.md is not a readme
        assert!(find_readme(&[entry("README.md", EntryKind::Dir)]).is_none());
    }

    #[test]
    fn readme_render_basic_and_fallbacks() {
        let t = tempfile::tempdir().unwrap();
        let fx = testkit::basic(&t.path().join("b"));
        let repo = testkit::open(&fx.path);
        let r = crate::refs::resolve(&repo, &rev("main").unwrap()).unwrap();
        let out = render_readme(&repo, &r, &GitPath::root()).unwrap().unwrap();
        assert_eq!(out.source_path, "README.md");
        assert!(out.html.contains("<table>"), "{}", out.html);
        assert!(out.html.contains("<img src=\"rsrepo:/docs/logo.png\""), "{}", out.html);
        assert!(out.html.contains("href=\"rsrepo:/docs/guide.md\""), "{}", out.html);
        assert!(out.html.contains("class=\"language-rust\""), "{}", out.html);
        // docs/guide.md links back up to ../README.md
        let g = render_file(&repo, &r, &git_path("docs/guide.md").unwrap()).unwrap();
        assert!(g.html.contains("href=\"rsrepo:/README.md\""), "{}", g.html);
        assert_eq!(render_readme(&repo, &r, &git_path("src").unwrap()).unwrap(), None);

        // plain READMEs and root fallbacks
        let mut b = testkit::repo(&t.path().join("p"));
        b.bare().file("sub/README", b"<b>plain</b>\n").file(".github/README.md", b"# From github dir\n").commit("c", T0);
        let repo = testkit::open(&t.path().join("p"));
        let r = crate::refs::resolve(&repo, &rev("main").unwrap()).unwrap();
        let sub = render_readme(&repo, &r, &git_path("sub").unwrap()).unwrap().unwrap();
        assert_eq!(sub.html, "<pre class=\"readme-plain\">&lt;b&gt;plain&lt;/b&gt;\n</pre>");
        let root = render_readme(&repo, &r, &GitPath::root()).unwrap().unwrap();
        assert_eq!(root.source_path, ".github/README.md");
        assert!(root.html.contains("From github dir"));
        assert_eq!(root.headings[0].text, "From github dir");
        assert_eq!(render_file(&repo, &r, &git_path("sub").unwrap()).unwrap_err().code(), "notFound");
    }
}
