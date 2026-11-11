//! Blob sniffing, metadata, text and bytes (M2.5; FR-22..25, NFR-P4, R-7, ARCH §6.8).

use crate::error::{Error, Result};
use crate::types::{BlobKind, BlobMeta, BlobText, EntryKind, ResolvedRev};
use crate::validate::GitPath;
use git2::{Oid, Repository};

/// Above this: no highlighting (FR-25, NFR-P4).
pub const HIGHLIGHT_MAX: u64 = 1024 * 1024;
/// Above this: not sent to the webview, Raw/Save only.
pub const DISPLAY_MAX: u64 = 5 * 1024 * 1024;
/// Binary sniff window.
pub const SNIFF_BYTES: usize = 8 * 1024;
const LFS_PREFIX: &[u8] = b"version https://git-lfs.github.com/spec/v1";
const LFS_MAX: u64 = 1024;
const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Image mime by extension (FR-24); svg is only ever shown via `<img>` from a blob URL (NFR-S7).
pub fn image_mime(path: &str) -> Option<&'static str> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

fn lfs_size(head: &[u8]) -> Option<u64> {
    let text = std::str::from_utf8(head).ok()?;
    text.lines().find_map(|l| l.strip_prefix("size ")).and_then(|n| n.trim().parse().ok())
}

/// Kind of a regular blob from its path, first bytes and size: LFS pointer, image (by extension), binary (NUL in
/// the first 8 KB) or text. Symlinks and submodules are decided by the tree entry mode, not here.
pub fn sniff(path: &str, head: &[u8], size: u64) -> BlobKind {
    if size <= LFS_MAX && head.starts_with(LFS_PREFIX) {
        return BlobKind::LfsPointer { size: lfs_size(head) };
    }
    if let Some(mime) = image_mime(path) {
        return BlobKind::Image { mime: mime.to_string() };
    }
    let window = &head[..head.len().min(SNIFF_BYTES)];
    if window.contains(&0) { BlobKind::Binary } else { BlobKind::Text }
}

/// highlight.js id from the extension / file name, else from a shebang line.
pub fn lang_hint(path: &str, head: &[u8]) -> Option<String> {
    if let Some(l) = crate::langs_table::lookup(path) {
        return Some(l.1.to_string());
    }
    let first = head.split(|b| *b == b'\n').next()?;
    let first = std::str::from_utf8(first).ok()?.trim_end_matches('\r');
    crate::langs_table::from_shebang(first).map(|l| l.1.to_string())
}

/// (lines, sloc): a final line without newline counts; sloc = lines with non-whitespace.
fn count_lines(text: &str) -> (u32, u32) {
    if text.is_empty() {
        return (0, 0);
    }
    let mut lines = 0u32;
    let mut sloc = 0u32;
    for l in text.split_inclusive('\n') {
        lines += 1;
        if !l.trim().is_empty() {
            sloc += 1;
        }
    }
    (lines, sloc)
}

/// Decoded text: BOM stripped, lossy UTF-8 (`true` when bytes were replaced).
pub fn decode(bytes: &[u8]) -> (String, bool) {
    let b = bytes.strip_prefix(BOM).unwrap_or(bytes);
    match std::str::from_utf8(b) {
        Ok(s) => (s.to_string(), false),
        Err(_) => (String::from_utf8_lossy(b).into_owned(), true),
    }
}

/// The entry at `path` (must not be a directory): (oid, mode).
fn entry(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<(Oid, i32)> {
    if path.is_root() {
        return Err(Error::not_found("file"));
    }
    let root = crate::tree::root_tree(repo, rev)?;
    match crate::tree::lookup(repo, &root, path)? {
        Some((_, 0o040000)) => Err(Error::not_found("file")),
        Some(e) => Ok(e),
        None => Err(Error::not_found("path")),
    }
}

/// Submodule URL for `path` from the root `.gitmodules` at this rev.
fn submodule_url(repo: &Repository, rev: &ResolvedRev, path: &str) -> Option<String> {
    let root = crate::tree::root_tree(repo, rev).ok()?;
    let oid = root.get_name(".gitmodules")?.id();
    let blob = repo.find_blob(oid).ok()?;
    let text = String::from_utf8_lossy(blob.content());
    let mut cur_path: Option<String> = None;
    let mut cur_url: Option<String> = None;
    let mut found = None;
    let mut flush = |p: &mut Option<String>, u: &mut Option<String>| {
        if p.as_deref() == Some(path) && found.is_none() {
            found = u.clone();
        }
        *p = None;
        *u = None;
    };
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            flush(&mut cur_path, &mut cur_url);
        } else if let Some((k, v)) = l.split_once('=') {
            match k.trim() {
                "path" => cur_path = Some(v.trim().to_string()),
                "url" => cur_url = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    flush(&mut cur_path, &mut cur_url);
    found
}

fn meta_of<'r>(repo: &'r Repository, rev: &ResolvedRev, path: &GitPath) -> Result<(BlobMeta, Option<git2::Blob<'r>>)> {
    let (oid, mode) = entry(repo, rev, path)?;
    let p = path.as_str();
    let base = |kind: BlobKind, size: u64| BlobMeta {
        path: p.to_string(),
        oid: oid.to_string(),
        size,
        kind,
        lines: None,
        sloc: None,
        lossy: false,
        highlight: false,
        displayable: false,
        lang_hint: None,
    };
    match crate::tree::kind_of_mode(mode) {
        EntryKind::Submodule => return Ok((base(BlobKind::Submodule { url: submodule_url(repo, rev, p) }, 0), None)),
        EntryKind::Dir => return Err(Error::not_found("file")),
        _ => {}
    }
    let blob = repo.find_blob(oid)?;
    let size = blob.size() as u64;
    if mode == 0o120000 {
        let target = String::from_utf8_lossy(blob.content()).into_owned();
        return Ok((base(BlobKind::Symlink { target }, size), Some(blob)));
    }
    let content = blob.content();
    let kind = sniff(p, &content[..content.len().min(SNIFF_BYTES)], size);
    let mut m = base(kind, size);
    m.displayable = size <= DISPLAY_MAX;
    if m.kind == BlobKind::Text {
        m.highlight = size <= HIGHLIGHT_MAX;
        m.lang_hint = lang_hint(p, &content[..content.len().min(512)]);
        if m.displayable {
            let (text, lossy) = decode(content);
            let (lines, sloc) = count_lines(&text);
            m.lines = Some(lines);
            m.sloc = Some(sloc);
            m.lossy = lossy;
        }
    }
    Ok((m, Some(blob)))
}

/// Metadata of the file at `path` (FR-22..25).
pub fn meta(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<BlobMeta> {
    Ok(meta_of(repo, rev, path)?.0)
}

/// Metadata plus decoded text (BOM stripped, lossy) for displayable text files; `None` otherwise.
pub fn text(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<BlobText> {
    let (meta, blob) = meta_of(repo, rev, path)?;
    let text = match (&meta.kind, blob) {
        (BlobKind::Text, Some(b)) if meta.displayable => Some(decode(b.content()).0),
        _ => None,
    };
    Ok(BlobText { meta, text })
}

/// Raw bytes (Raw view, Save a copy, images); `Refused` above `max`.
pub fn bytes(repo: &Repository, rev: &ResolvedRev, path: &GitPath, max: u64) -> Result<Vec<u8>> {
    let (oid, mode) = entry(repo, rev, path)?;
    if mode == 0o160000 {
        return Err(Error::Refused("A submodule has no content in this repository".into()));
    }
    let blob = repo.find_blob(oid)?;
    if blob.size() as u64 > max {
        return Err(Error::Refused(format!("File is larger than {:.1} MB", max as f64 / (1024.0 * 1024.0))));
    }
    Ok(blob.content().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit;
    use crate::validate::{git_path, rev};

    fn fx() -> (tempfile::TempDir, Repository, ResolvedRev) {
        let t = tempfile::tempdir().unwrap();
        let p = testkit::binaries(&t.path().join("b"));
        let repo = testkit::open(&p);
        let r = crate::refs::resolve(&repo, &rev("main").unwrap()).unwrap();
        (t, repo, r)
    }

    fn m(repo: &Repository, r: &ResolvedRev, p: &str) -> BlobMeta {
        meta(repo, r, &git_path(p).unwrap()).unwrap()
    }

    #[test]
    fn sniff_rules() {
        assert_eq!(sniff("a.txt", b"hello", 5), BlobKind::Text);
        assert_eq!(sniff("a.bin", b"he\0llo", 6), BlobKind::Binary);
        assert_eq!(sniff("x.PNG", b"\x89PNG", 4), BlobKind::Image { mime: "image/png".into() });
        assert_eq!(sniff("m.bin", testkit::LFS_POINTER.as_bytes(), 120), BlobKind::LfsPointer { size: Some(12345) });
        // too big to be a pointer
        assert_eq!(sniff("m.bin", testkit::LFS_POINTER.as_bytes(), 5000), BlobKind::Text);
        let mut late_nul = vec![b'a'; SNIFF_BYTES + 10];
        late_nul[SNIFF_BYTES + 5] = 0;
        assert_eq!(sniff("a", &late_nul, late_nul.len() as u64), BlobKind::Text, "NUL after 8 KB ignored");
    }

    #[test]
    fn one_meta_per_placeholder_kind() {
        let (_t, repo, r) = fx();
        assert_eq!(m(&repo, &r, "bin/data.bin").kind, BlobKind::Binary);
        assert_eq!(m(&repo, &r, "img/logo.png").kind, BlobKind::Image { mime: "image/png".into() });
        assert_eq!(m(&repo, &r, "img/evil.svg").kind, BlobKind::Image { mime: "image/svg+xml".into() });
        assert_eq!(m(&repo, &r, "lfs/model.bin").kind, BlobKind::LfsPointer { size: Some(12345) });
        assert_eq!(m(&repo, &r, "link").kind, BlobKind::Symlink { target: "README.md".into() });
        assert_eq!(m(&repo, &r, "sub/lib").kind, BlobKind::Submodule { url: Some("https://example.com/lib.git".into()) });

        let large = m(&repo, &r, "big/large.txt");
        assert_eq!(large.kind, BlobKind::Text);
        assert!(!large.highlight && large.displayable);
        assert_eq!(large.size, testkit::LARGE_TEXT_BYTES as u64);
        let huge = m(&repo, &r, "big/huge.txt");
        assert!(!huge.highlight && !huge.displayable && huge.lines.is_none());

        let latin = text(&repo, &r, &git_path("text/latin1.txt").unwrap()).unwrap();
        assert!(latin.meta.lossy);
        assert_eq!(latin.text.as_deref(), Some("caf\u{fffd} au lait\n"));
        let bom = text(&repo, &r, &git_path("text/bom.txt").unwrap()).unwrap();
        assert_eq!(bom.text.as_deref(), Some("hello bom\nline two\n"));
        assert!(!bom.meta.lossy);
        assert_eq!((bom.meta.lines, bom.meta.sloc), (Some(2), Some(2)));
        let blank = m(&repo, &r, "text/blank.txt");
        assert_eq!((blank.lines, blank.sloc), (Some(4), Some(2)));

        let rs = m(&repo, &r, "README.md");
        assert!(rs.highlight && rs.displayable);
        assert_eq!(rs.lang_hint.as_deref(), Some("markdown"));
        assert_eq!(m(&repo, &r, "run").lang_hint.as_deref(), Some("python"), "shebang");

        // no text for non-text kinds or undisplayable sizes
        assert_eq!(text(&repo, &r, &git_path("bin/data.bin").unwrap()).unwrap().text, None);
        assert_eq!(text(&repo, &r, &git_path("big/huge.txt").unwrap()).unwrap().text, None);
        assert_eq!(text(&repo, &r, &git_path("link").unwrap()).unwrap().text, None);

        // non-UTF-8 file name reached via its lossy path; Windows-unstorable names read fine (D-34)
        assert_eq!(text(&repo, &r, &git_path("names/caf\u{fffd}.txt").unwrap()).unwrap().text.as_deref(), Some("latin-1 name\n"));
        assert_eq!(text(&repo, &r, &git_path("win/a:b.txt").unwrap()).unwrap().text.as_deref(), Some("colon name\n"));

        // errors
        assert_eq!(meta(&repo, &r, &git_path("img").unwrap()).unwrap_err().code(), "notFound");
        assert_eq!(meta(&repo, &r, &git_path("nope").unwrap()).unwrap_err().code(), "notFound");
        assert_eq!(meta(&repo, &r, &GitPath::root()).unwrap_err().code(), "notFound");
    }

    #[test]
    fn bytes_equal_blob_and_git_show() {
        let (t, repo, r) = fx();
        for p in ["bin/data.bin", "img/logo.png", "text/crlf.txt", "text/bom.txt", "big/huge.txt", "win/a:b.txt"] {
            let got = bytes(&repo, &r, &git_path(p).unwrap(), u64::MAX).unwrap();
            let tree = repo.find_tree(Oid::from_str(&r.tree).unwrap()).unwrap();
            let oid = crate::tree::lookup(&repo, &tree, &git_path(p).unwrap()).unwrap().unwrap().0;
            assert_eq!(got, repo.find_blob(oid).unwrap().content(), "{p}");
            if matches!(p, "text/crlf.txt" | "text/bom.txt") {
                // FR-23: equals `git show ref:path` byte-for-byte (UTF-8 files, so the CLI string is exact).
                if let Some(out) = testkit::git_cli(&t.path().join("b"), &["cat-file", "blob", &format!("main:{p}")]) {
                    assert_eq!(out.as_bytes(), got.as_slice(), "{p}");
                }
            }
        }
        assert_eq!(bytes(&repo, &r, &git_path("big/huge.txt").unwrap(), DISPLAY_MAX).unwrap_err().code(), "refused");
        assert_eq!(bytes(&repo, &r, &git_path("sub/lib").unwrap(), u64::MAX).unwrap_err().code(), "refused");
    }
}
