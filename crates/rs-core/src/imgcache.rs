//! Remote README images (v0.2.0 F2; D-105). The webview stays offline: Rust fetches the image through `net`
//! (`Profile::IMAGE`: public https hosts, ≤ 5 redirects, ≤ 5 MB, 30 s), keeps only real images (the server must
//! say `image/*`, and the bytes must be PNG / JPEG / GIF / WebP / BMP / ICO / AVIF by magic number, or an SVG
//! document — served to the UI only as an inert `<img>` blob), and caches them on disk under
//! `<data>\img-cache\<sha256(url)>.<ext>`, so a README seen once works offline. LRU by file time, 200 MB cap.

use crate::Shelf;
use crate::error::{Error, Result};
use crate::net::{Hosts, Net, Profile};
use crate::paths;
use crate::types::RemoteImage;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const DIR: &str = "img-cache";
pub const CACHE_CAP: u64 = 200 << 20;
pub const MAX_URL_LEN: usize = 4096;

/// (extension, mime) — the only types ever stored or answered.
const TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("ico", "image/x-icon"),
    ("avif", "image/avif"),
    ("svg", "image/svg+xml"),
];

pub fn cache_dir(s: &Shelf) -> PathBuf {
    s.dirs.data.join(DIR)
}

/// sha-256 hex of the URL.
pub fn key(url: &str) -> String {
    Sha256::digest(url.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn valid_key(k: &str) -> bool {
    k.len() == 64 && k.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Image type from the bytes (raster by magic number; SVG by its root element).
pub fn sniff(b: &[u8]) -> Option<&'static str> {
    let ext = if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if b.starts_with(&[0xff, 0xd8, 0xff]) {
        "jpg"
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        "gif"
    } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "webp"
    } else if b.starts_with(b"BM") && b.len() > 14 {
        "bmp"
    } else if b.starts_with(&[0, 0, 1, 0]) && b.len() > 6 {
        "ico"
    } else if b.len() >= 12 && &b[4..8] == b"ftyp" && (&b[8..12] == b"avif" || &b[8..12] == b"avis") {
        "avif"
    } else if is_svg(b) {
        "svg"
    } else {
        return None;
    };
    Some(ext)
}

fn is_svg(b: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&b[..b.len().min(4096)]).to_ascii_lowercase();
    let t = head.trim_start_matches('\u{feff}').trim_start();
    (t.starts_with("<svg") || t.starts_with("<?xml") || t.starts_with("<!--") || t.starts_with("<!doctype svg")) && t.contains("<svg")
}

fn mime_of(ext: &str) -> &'static str {
    TYPES.iter().find(|(e, _)| *e == ext).map(|(_, m)| *m).unwrap_or("application/octet-stream")
}

fn cached_file(dir: &Path, k: &str) -> Option<(PathBuf, &'static str)> {
    TYPES.iter().map(|(e, _)| (dir.join(format!("{k}.{e}")), *e)).find(|(p, _)| p.is_file()).map(|(p, e)| (p, mime_of(e)))
}

/// `image_fetch`: cached copy, else one fetch. Refused when `loadRemoteImages` is off or the URL is not a
/// public https address; `network` / `refused` errors make the UI show its "Remote image" chip.
pub fn fetch(s: &Shelf, url: &str) -> Result<RemoteImage> {
    if !crate::settings::load(&s.db)?.load_remote_images {
        return Err(Error::Refused("Remote images are turned off (Settings → Network).".into()));
    }
    if url.len() > MAX_URL_LEN {
        return Err(Error::InvalidUrl("image address too long".into()));
    }
    let parsed = url::Url::parse(url).map_err(|e| Error::InvalidUrl(format!("{e}")))?;
    Net::check_url(&parsed, Hosts::AnyPublic)?;
    let k = key(parsed.as_str());
    let dir = cache_dir(s);
    if let Some((p, mime)) = cached_file(&dir, &k) {
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        if let Ok(f) = std::fs::File::options().append(true).open(paths::long(&p)) {
            let _ = f.set_modified(SystemTime::now()); // LRU touch
        }
        return Ok(RemoteImage { key: k, mime: mime.into(), size });
    }
    let resp = s.net.get(parsed.as_str(), &[("Accept", "image/avif,image/webp,image/png,image/svg+xml,image/*;q=0.8".into())], Profile::IMAGE)?;
    if resp.status != 200 {
        return Err(Error::Network(format!("{} answered {}", parsed.host_str().unwrap_or(""), resp.status)));
    }
    let declared = resp.mime();
    if !declared.starts_with("image/") {
        let shown = if declared.is_empty() { "no type".to_string() } else { declared };
        return Err(Error::Refused(format!("Not an image ({shown}).")));
    }
    let bytes = resp.bytes()?;
    let ext = sniff(&bytes).ok_or_else(|| Error::Refused("The server sent something that is not a supported image.".into()))?;
    std::fs::create_dir_all(paths::long(&dir))?;
    let file = dir.join(format!("{k}.{ext}"));
    // unique per call: the same badge is often fetched by several lazy `<img>` at once
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".{k}.part-{}-{seq}", std::process::id()));
    std::fs::write(paths::long(&tmp), &bytes)?;
    if let Err(e) = std::fs::rename(paths::long(&tmp), paths::long(&file)) {
        let _ = std::fs::remove_file(paths::long(&tmp));
        // a concurrent call stored the same image first (Windows refuses to replace a file that is open)
        if !paths::long(&file).is_file() {
            return Err(e.into());
        }
    }
    prune(&dir, CACHE_CAP, Some(&file))?;
    Ok(RemoteImage { key: k, mime: mime_of(ext).into(), size: bytes.len() as u64 })
}

/// `image_bytes`: the cached bytes of a key `image_fetch` returned.
pub fn bytes(s: &Shelf, k: &str) -> Result<Vec<u8>> {
    if !valid_key(k) {
        return Err(Error::InvalidInput("bad image key".into()));
    }
    let (p, _) = cached_file(&cache_dir(s), k).ok_or_else(|| Error::not_found("cached image"))?;
    Ok(std::fs::read(paths::long(&p))?)
}

/// Oldest files (by modification time) go first until the folder is ≤ `cap`; `keep` is never removed.
pub fn prune(dir: &Path, cap: u64, keep: Option<&Path>) -> Result<u32> {
    let Ok(rd) = std::fs::read_dir(paths::long(dir)) else { return Ok(0) };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file().then(|| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.0);
    let keep = keep.map(paths::long);
    let mut n = 0;
    for (_, len, p) in files {
        if total <= cap {
            break;
        }
        if keep.as_deref() == Some(p.as_path()) || keep.as_deref().is_some_and(|k| k.file_name() == p.file_name()) {
            continue;
        }
        if std::fs::remove_file(&p).is_ok() {
            total -= len;
            n += 1;
        }
    }
    Ok(n)
}

/// Bytes on disk (for `app_info.cacheBytes`).
pub fn size(s: &Shelf) -> u64 {
    std::fs::read_dir(paths::long(&cache_dir(s)))
        .map(|rd| rd.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum())
        .unwrap_or(0)
}

/// "Clear cache": removes every cached image.
pub fn clear(s: &Shelf) -> Result<()> {
    let dir = cache_dir(s);
    let Ok(rd) = std::fs::read_dir(paths::long(&dir)) else { return Ok(()) };
    for e in rd.flatten() {
        if e.file_type().is_ok_and(|t| t.is_file()) {
            std::fs::remove_file(e.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Canned;
    use crate::testkit;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    const SVG: &[u8] = b"<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>";

    fn shelf(t: &Path, c: &std::sync::Arc<Canned>) -> Shelf {
        testkit::shelf(t).with_net(Net::new(c.clone()))
    }

    #[test]
    fn fetch_caches_and_works_offline_afterwards() {
        let t = tempfile::tempdir().unwrap();
        let c = Canned::new();
        c.redirect("https://img.shields.io/badge/x", "https://cdn.example.com/b.svg").on(
            "https://cdn.example.com/b.svg",
            200,
            &[("content-type", "image/svg+xml;charset=utf-8")],
            SVG.to_vec(),
        );
        c.on("https://example.com/a.png", 200, &[("content-type", "image/png")], PNG.to_vec());
        let s = shelf(t.path(), &c);
        let svg = fetch(&s, "https://img.shields.io/badge/x").unwrap();
        assert_eq!(svg.mime, "image/svg+xml");
        assert_eq!(svg.key, key("https://img.shields.io/badge/x"));
        assert_eq!(bytes(&s, &svg.key).unwrap(), SVG);
        let png = fetch(&s, "https://example.com/a.png").unwrap();
        assert_eq!((png.mime.as_str(), png.size), ("image/png", PNG.len() as u64));
        assert_eq!(c.seen().len(), 3);
        // a second shelf on the same data dir with no network at all: served from disk
        let offline = testkit::shelf(t.path());
        assert_eq!(fetch(&offline, "https://example.com/a.png").unwrap(), png);
        assert_eq!(bytes(&offline, &png.key).unwrap(), PNG);
        assert!(size(&offline) > 0);
        clear(&offline).unwrap();
        assert_eq!(size(&offline), 0);
        assert_eq!(bytes(&offline, &png.key).unwrap_err().code(), "notFound");
    }

    #[test]
    fn refusals() {
        let t = tempfile::tempdir().unwrap();
        let c = Canned::new();
        c.on("https://example.com/page", 200, &[("content-type", "text/html")], b"<html>".to_vec())
            .on("https://example.com/lie.png", 200, &[("content-type", "image/png")], b"<html><script>".to_vec())
            .on("https://example.com/404.png", 404, &[], Vec::new())
            .on("https://example.com/big.png", 200, &[("content-type", "image/png")], vec![0x89; 6 << 20])
            .dns("internal.example.com", &["10.0.0.1"]);
        let s = shelf(t.path(), &c);
        for (u, code) in [
            ("https://example.com/page", "refused"),
            ("https://example.com/lie.png", "refused"),
            ("https://example.com/404.png", "network"),
            ("https://example.com/big.png", "refused"),
            ("https://internal.example.com/a.png", "refused"),
            ("http://example.com/a.png", "refused"),
            ("https://127.0.0.1/a.png", "refused"),
            ("data:image/png;base64,AA", "refused"),
            ("rsrepo:/a.png", "refused"),
            ("not a url", "invalidUrl"),
        ] {
            assert_eq!(fetch(&s, u).unwrap_err().code(), code, "{u}");
        }
        assert!(!c.seen_urls().iter().any(|u| u.contains("internal") || u.starts_with("http:")));
        assert_eq!(size(&s), 0, "nothing cached");
        assert_eq!(bytes(&s, "../x").unwrap_err().code(), "invalidInput");
        assert_eq!(bytes(&s, &"A".repeat(64)).unwrap_err().code(), "invalidInput");
        // setting off → refused before any request
        crate::settings::save(&s.db, &crate::settings::Settings { load_remote_images: false, ..Default::default() }).unwrap();
        let n = c.seen().len();
        assert_eq!(fetch(&s, "https://example.com/a.png").unwrap_err().code(), "refused");
        assert_eq!(c.seen().len(), n);
    }

    #[test]
    fn concurrent_fetches_of_one_url_all_succeed() {
        let t = tempfile::tempdir().unwrap();
        let c = Canned::new();
        c.on("https://img.example.com/badge.png", 200, &[("content-type", "image/png")], PNG.to_vec());
        let s = std::sync::Arc::new(shelf(t.path(), &c));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let s = s.clone();
                std::thread::spawn(move || {
                    let img = fetch(&s, "https://img.example.com/badge.png").unwrap();
                    assert_eq!(bytes(&s, &img.key).unwrap(), PNG);
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let left: Vec<String> =
            std::fs::read_dir(cache_dir(&s)).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left.len(), 1, "one cached file, no temp leftovers: {left:?}");
    }

    #[test]
    fn sniffing() {
        assert_eq!(sniff(PNG), Some("png"));
        assert_eq!(sniff(b"\xff\xd8\xff\xe0"), Some("jpg"));
        assert_eq!(sniff(b"GIF89a.."), Some("gif"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(sniff(b"\0\0\0\x1cftypavif\0\0"), Some("avif"));
        assert_eq!(sniff(b"\xef\xbb\xbf  <svg viewBox='0 0 1 1'/>"), Some("svg"));
        assert_eq!(sniff(b"<!-- c --><svg/>"), Some("svg"));
        assert_eq!(sniff(b"<html><svg/></html>"), None);
        assert_eq!(sniff(b"%PDF-1.7"), None);
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn lru_prune_keeps_newest_under_cap() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("c");
        std::fs::create_dir_all(&dir).unwrap();
        let base = SystemTime::now() - std::time::Duration::from_secs(1000);
        for i in 0..5u64 {
            let p = dir.join(format!("{i}.png"));
            std::fs::write(&p, vec![0u8; 100]).unwrap();
            std::fs::File::options().append(true).open(&p).unwrap().set_modified(base + std::time::Duration::from_secs(i * 10)).unwrap();
        }
        // touch the oldest: it becomes the newest
        std::fs::File::options().append(true).open(dir.join("0.png")).unwrap().set_modified(SystemTime::now()).unwrap();
        assert_eq!(prune(&dir, 250, Some(&dir.join("1.png"))).unwrap(), 3);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["0.png", "1.png"], "kept: the touched one and the protected one");
    }
}
