//! Release assets (v0.2.0 F4; D-104..D-106): picking the file for this PC, the release store, the downloader.
//!
//! - Picker: the *ideas* of Project Manager's release picking (read-only reference, re-implemented): classify by
//!   name/extension, detect OS + arch from the name, keep what fits this OS/arch, prefer portable > zip / plain
//!   exe > installer (setup exe, msi) > other archives; checksums, signatures, source archives and packages for
//!   other systems are never picked. Two best candidates with the same score = not confident → nothing.
//! - Store: `<archive>\_releases\<host>\<owner>\<name>\<tag>\<file>` (every component sanitised), never inside
//!   an imported repository.
//! - Download: only an asset found in the cached GitHub answer by `(repo, tag, name)` (never a URL from the UI),
//!   GitHub hosts only (`net::ASSET_HOSTS`, redirect-checked), size cap (announced + streamed), `.partial` +
//!   rename, skip when already there, SHA-256 check when GitHub publishes a digest, cancellable, progress.
//!   Never run, never unpacked; the file gets the "downloaded from the Internet" mark (Zone.Identifier).

use crate::Shelf;
use crate::error::{Error, Result};
use crate::ghreleases::{self, GhAsset, GhRelease};
use crate::jobs::{JobCtx, Progress};
use crate::net::Profile;
use crate::paths;
use crate::types::JobResult;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Top folder of the release store inside the archive dir.
pub const STORE_DIR: &str = "_releases";
pub const PARTIAL_MARK: &str = ".partial-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Portable,
    App,
    Archive,
    Installer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Mac,
    Linux,
    Android,
    Any,
}

/// `None` = never picked (checksums, signatures, metadata, source archives, packages, debug symbols…).
pub fn classify(name: &str) -> Option<Kind> {
    let n = name.to_ascii_lowercase();
    let has = |w: &str| n.contains(w);
    let skip_ext = [
        ".sha256", ".sha256sum", ".sha512", ".md5", ".sig", ".asc", ".minisig", ".pem", ".crt", ".sbom", ".spdx", ".json", ".yml", ".yaml",
        ".txt", ".blockmap", ".pdb", ".sym", ".dsym", ".intoto.jsonl", ".provenance", ".whl", ".apk", ".aab", ".vsix", ".nupkg", ".crx", ".xpi",
        ".jar",
    ];
    if skip_ext.iter().any(|e| n.ends_with(e))
        || n.starts_with("sha256sums")
        || n.starts_with("checksums")
        || has("source")
        || has("-src.")
        || has("_src.")
        || n.starts_with("unins")
    {
        return None;
    }
    if [".msi", ".msix", ".msixbundle", ".appx", ".dmg", ".pkg", ".deb", ".rpm"].iter().any(|e| n.ends_with(e)) {
        return Some(Kind::Installer);
    }
    if n.ends_with(".appimage") {
        return Some(Kind::Portable);
    }
    if [".zip", ".7z", ".tar.gz", ".tgz", ".tar.xz", ".txz", ".tar.zst", ".tar.bz2"].iter().any(|e| n.ends_with(e)) {
        return Some(Kind::Archive);
    }
    if n.ends_with(".exe") {
        return Some(if has("setup") || has("install") {
            Kind::Installer
        } else if has("portable") {
            Kind::Portable
        } else {
            Kind::App
        });
    }
    None
}

/// OS and arch named in a file name (`x86_64` / `aarch64` / `x86` / `arm` / `riscv64`).
pub fn detect(name: &str) -> (Platform, Option<&'static str>) {
    let n = name.to_ascii_lowercase();
    let has = |w: &str| n.contains(w);
    let ends = |exts: &[&str]| exts.iter().any(|e| n.ends_with(e));
    let platform = if has("android") {
        Platform::Android
    } else if has("windows") || has("win64") || has("win32") || has("-win-") || has("_win_") || has("-win.") || has("_win.") || has("msvc")
        || ends(&[".exe", ".msi", ".msix", ".msixbundle", ".appx"])
    {
        Platform::Windows
    } else if has("darwin") || has("macos") || has("-mac") || has("_mac") || has("osx") || has("apple") || ends(&[".dmg", ".pkg"]) {
        Platform::Mac
    } else if has("linux") || has("musl") || has("-gnu") || ends(&[".appimage", ".deb", ".rpm"]) || has("freebsd") {
        Platform::Linux
    } else {
        Platform::Any
    };
    let arch = if has("x86_64") || has("x64") || has("amd64") || has("win64") {
        Some("x86_64")
    } else if has("aarch64") || has("arm64") {
        Some("aarch64")
    } else if has("riscv64") {
        Some("riscv64")
    } else if has("armv7") || has("armhf") {
        Some("arm")
    } else if has("i686") || has("i386") || has("win32") || has("-x86.") || has("_x86.") || has("-x86-") || has("_x86_") || has("ia32") {
        Some("x86")
    } else {
        None
    };
    (platform, arch)
}

fn host_platform(os: &str) -> Option<Platform> {
    match os {
        "windows" => Some(Platform::Windows),
        "macos" => Some(Platform::Mac),
        "linux" => Some(Platform::Linux),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    One(usize),
    Nothing(String),
}

/// Scores every asset that fits `os` / `arch` (as in `std::env::consts`); the single best one wins.
pub fn pick(assets: &[GhAsset], os: &str, arch: &str) -> Pick {
    let Some(host) = host_platform(os) else { return Pick::Nothing(format!("no release files are picked for {os}")) };
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (i, a) in assets.iter().enumerate() {
        let Some(kind) = classify(&a.name) else { continue };
        let (platform, a_arch) = detect(&a.name);
        if platform != host {
            continue;
        }
        let arch_score = match a_arch {
            None => 0,
            Some(x) if x == arch => 5,
            Some("x86") if arch == "x86_64" => -3,
            // Windows on Arm runs x64 / x86 builds emulated
            Some("x86_64") | Some("x86") if arch == "aarch64" && os == "windows" => -4,
            Some(_) => continue,
        };
        let n = a.name.to_ascii_lowercase();
        let mut score = match kind {
            Kind::Portable => 40,
            Kind::Archive if n.ends_with(".zip") || os != "windows" => 30,
            Kind::App => 30,
            Kind::Installer => 20,
            Kind::Archive => 10,
        } + arch_score;
        if os == "windows" && n.contains("msvc") {
            score += 2;
        }
        if ["debug", "symbols", "-cli", "_cli", "headless"].iter().any(|w| n.contains(w)) {
            score -= 15;
        }
        scored.push((score, i));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    match scored.as_slice() {
        [] => Pick::Nothing(format!("no file for this PC ({os}, {arch})")),
        [(s0, i), (s1, j), ..] if s0 == s1 => {
            Pick::Nothing(format!("several files fit this PC equally well ({} / {})", assets[*i].name, assets[*j].name))
        }
        [(_, i), ..] => Pick::One(*i),
    }
}

/// One path component from untrusted text (tag, asset name, owner): Windows-invalid and control characters →
/// `_`, no trailing dot/space, no `.`/`..`/empty, DOS device names and `.git` prefixed with `_`, ≤ 120 chars.
pub fn safe_component(s: &str) -> String {
    let mut t: String = s
        .chars()
        .map(|c| if (c as u32) < 32 || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\' | '/') { '_' } else { c })
        .take(120)
        .collect();
    while t.ends_with('.') || t.ends_with(' ') {
        t.pop();
    }
    if t.is_empty() || t == "." || t == ".." {
        return "_".into();
    }
    if !crate::winpath::path_ok_here(&t) {
        t = format!("_{t}");
    }
    t
}

/// Percent-encodes a tag for a URL path segment.
pub fn url_path_segment(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// The release-store folder of one tag (github.com origins only).
pub fn store_dir(s: &Shelf, origin: Option<&str>, tag: &str) -> Result<PathBuf> {
    let (o, n) = ghreleases::github_repo(origin)?;
    Ok(s.dirs.archive.join(STORE_DIR).join("github.com").join(safe_component(&o)).join(safe_component(&n)).join(safe_component(tag)))
}

/// Where an asset lands.
pub fn store_file(s: &Shelf, origin: Option<&str>, tag: &str, asset: &str) -> Result<PathBuf> {
    Ok(store_dir(s, origin, tag)?.join(safe_component(asset)))
}

pub enum Latest {
    Download { tag: String, asset: String },
    Already { tag: String, asset: String },
    Nothing(String),
}

/// The auto-download decision for GitHub's latest stable release.
pub fn latest_for_this_pc(s: &Shelf, repo_id: i64, gh: &[GhRelease]) -> Result<Latest> {
    let Some(rel) = ghreleases::latest(gh) else { return Ok(Latest::Nothing("no stable release on GitHub".into())) };
    let a = match pick(&rel.assets, std::env::consts::OS, std::env::consts::ARCH) {
        Pick::One(i) => &rel.assets[i],
        Pick::Nothing(why) => return Ok(Latest::Nothing(format!("{} — {why}", rel.tag_name))),
    };
    let cap = crate::settings::load(&s.db)?.download_cap_bytes();
    if a.size > cap {
        return Ok(Latest::Nothing(format!("{} is larger than the download limit ({})", a.name, crate::net::human_bytes(cap))));
    }
    let file = store_file(s, ghreleases::origin_of(s, repo_id)?.as_deref(), &rel.tag_name, &a.name)?;
    let (tag, asset) = (rel.tag_name.clone(), a.name.clone());
    Ok(if paths::long(&file).is_file() { Latest::Already { tag, asset } } else { Latest::Download { tag, asset } })
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn sha256_file(p: &Path) -> Result<String> {
    let mut f = std::fs::File::open(paths::long(p))?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(hex(&h.finalize()))
}

fn now_millis() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

/// Marks a downloaded file as coming from the Internet (SmartScreen / Office protected view apply). Best effort.
fn mark_from_internet(file: &Path, url: &str) {
    #[cfg(windows)]
    {
        let ads = format!("{}:Zone.Identifier", paths::long(file).display());
        let _ = std::fs::write(ads, format!("[ZoneTransfer]\r\nZoneId=3\r\nHostUrl={url}\r\n"));
    }
    #[cfg(not(windows))]
    {
        let _ = (file, url);
    }
}

/// Downloads one asset of the cached GitHub answer into the release store (the `ReleaseDownload` job).
pub fn download(s: &Shelf, repo_id: i64, tag: &str, asset: &str, ctx: &JobCtx) -> Result<JobResult> {
    let origin = ghreleases::origin_of(s, repo_id)?;
    let (o, n) = ghreleases::github_repo(origin.as_deref())?;
    let gh = ghreleases::cached(s, repo_id)?.ok_or_else(|| Error::not_found("GitHub release list (load the release notes first)"))?;
    let rel = gh.iter().find(|r| r.tag_name == tag).ok_or_else(|| Error::not_found("release"))?;
    let a = rel.assets.iter().find(|a| a.name == asset).ok_or_else(|| Error::not_found("release file"))?;
    let prefix = format!("https://github.com/{o}/{n}/releases/download/");
    if !a.browser_download_url.starts_with(&prefix) {
        return Err(Error::Refused(format!("The download link of {} does not point to this repository's releases.", a.name)));
    }
    let cap = crate::settings::load(&s.db)?.download_cap_bytes();
    if a.size > cap {
        return Err(Error::Refused(format!("{} is larger than the download limit ({}).", a.name, crate::net::human_bytes(cap))));
    }
    let want_sha = a.sha256();
    let file = store_file(s, origin.as_deref(), tag, asset)?;
    let done = |path: &Path, size: u64, checked: bool, already: bool| JobResult::Downloaded {
        repo_id,
        tag: tag.to_string(),
        file: path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
        path: path.display().to_string(),
        size,
        sha256_checked: checked,
        already,
    };
    // already there: same size (and digest, when published) → nothing to do
    if let Ok(m) = std::fs::metadata(paths::long(&file))
        && m.is_file()
        && (a.size == 0 || m.len() == a.size)
    {
        match &want_sha {
            None => return Ok(done(&file, m.len(), false, true)),
            Some(w) if *w == sha256_file(&file)? => return Ok(done(&file, m.len(), true, true)),
            Some(_) => {} // differs: download again and replace
        }
    }
    ctx.check()?;
    ctx.progress(Progress::Connecting);
    let dir = file.parent().ok_or_else(|| Error::InvalidInput("bad store path".into()))?.to_path_buf();
    std::fs::create_dir_all(paths::long(&dir))?;
    let fname = file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let partial = dir.join(format!(".{fname}{PARTIAL_MARK}{}", now_millis()));
    let outcome = (|| -> Result<(u64, bool)> {
        let resp = s.net.get(&a.browser_download_url, &[("Accept", "application/octet-stream".to_string())], Profile::asset(cap))?;
        if resp.status != 200 {
            return Err(Error::Network(format!("GitHub answered {} for {}", resp.status, a.name)));
        }
        if let Some(len) = resp.content_length()
            && a.size > 0
            && len != a.size
        {
            return Err(Error::Refused(format!("GitHub announced {} bytes for {} but is sending {len}. Nothing was saved.", a.size, a.name)));
        }
        let total = a.size;
        let mut out = std::fs::File::create(paths::long(&partial))?;
        let mut h = Sha256::new();
        let mut bytes = 0u64;
        resp.stream(&mut |chunk| {
            ctx.check()?;
            out.write_all(chunk)?;
            h.update(chunk);
            bytes += chunk.len() as u64;
            ctx.progress(Progress::Downloading { bytes, total });
            Ok(())
        })?;
        out.sync_all()?;
        drop(out);
        if a.size > 0 && bytes != a.size {
            return Err(Error::Network(format!("The download stopped early ({bytes} of {} bytes). Nothing was saved.", a.size)));
        }
        let got = hex(&h.finalize());
        if let Some(w) = &want_sha
            && *w != got
        {
            return Err(Error::Refused(format!("Checksum mismatch for {}: GitHub lists sha256 {w}, the download has {got}. The file was discarded.", a.name)));
        }
        ctx.progress(Progress::Finalizing);
        Ok((bytes, want_sha.is_some()))
    })();
    match outcome {
        Ok((bytes, checked)) => {
            if let Err(e) = std::fs::rename(paths::long(&partial), paths::long(&file)) {
                let _ = std::fs::remove_file(paths::long(&partial));
                return Err(e.into());
            }
            mark_from_internet(&file, &a.browser_download_url);
            Ok(done(&file, bytes, checked, false))
        }
        Err(e) => {
            let _ = std::fs::remove_file(paths::long(&partial));
            Err(e)
        }
    }
}

/// Removes leftover `.x.partial-*` files under `<archive>\_releases` (a killed download). Returns the count.
pub fn cleanup_partials(archive: &Path) -> Result<u32> {
    let mut n = 0;
    let mut stack = vec![(paths::long(&archive.join(STORE_DIR)), 0u8)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(items) = std::fs::read_dir(&dir) else { continue };
        for it in items.flatten() {
            let Ok(ft) = it.file_type() else { continue };
            let name = it.file_name().to_string_lossy().into_owned();
            if ft.is_dir() && depth < 6 {
                stack.push((it.path(), depth + 1));
            } else if ft.is_file() && name.starts_with('.') && name.contains(PARTIAL_MARK) {
                std::fs::remove_file(it.path())?;
                n += 1;
            }
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Canned, Net};
    use crate::testkit;
    use crate::types::Source;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn asset(name: &str, size: u64) -> GhAsset {
        GhAsset {
            name: name.into(),
            size,
            download_count: None,
            browser_download_url: format!("https://github.com/o/r/releases/download/v1.0.0/{name}"),
            content_type: None,
            digest: None,
        }
    }

    fn names(list: &[&str]) -> Vec<GhAsset> {
        list.iter().map(|n| asset(n, 1)).collect()
    }

    fn picked(list: &[&str], os: &str, arch: &str) -> Option<String> {
        let a = names(list);
        match pick(&a, os, arch) {
            Pick::One(i) => Some(a[i].name.clone()),
            Pick::Nothing(_) => None,
        }
    }

    #[test]
    fn picker_table() {
        let typical = [
            "tool-1.0.0-x86_64-pc-windows-msvc.zip",
            "tool-1.0.0-x86_64-pc-windows-gnu.zip",
            "tool-1.0.0-aarch64-pc-windows-msvc.zip",
            "tool-1.0.0-x86_64-unknown-linux-gnu.tar.gz",
            "tool-1.0.0-aarch64-apple-darwin.tar.gz",
            "tool-1.0.0.sha256",
            "SHA256SUMS",
            "tool-1.0.0-source.tar.gz",
        ];
        assert_eq!(picked(&typical, "windows", "x86_64").as_deref(), Some("tool-1.0.0-x86_64-pc-windows-msvc.zip"));
        assert_eq!(picked(&typical, "windows", "aarch64").as_deref(), Some("tool-1.0.0-aarch64-pc-windows-msvc.zip"));
        assert_eq!(picked(&typical, "linux", "x86_64").as_deref(), Some("tool-1.0.0-x86_64-unknown-linux-gnu.tar.gz"));
        assert_eq!(picked(&typical, "macos", "aarch64").as_deref(), Some("tool-1.0.0-aarch64-apple-darwin.tar.gz"));
        assert_eq!(picked(&typical, "android", "aarch64"), None);
        // Electron / Tauri style: portable beats setup beats msi
        let app = ["App-Setup-2.1.0.exe", "App-2.1.0-portable.exe", "App-2.1.0.msi", "App-2.1.0.dmg", "App-2.1.0.AppImage", "latest.yml", "App-Setup-2.1.0.exe.blockmap"];
        assert_eq!(picked(&app, "windows", "x86_64").as_deref(), Some("App-2.1.0-portable.exe"));
        assert_eq!(picked(&app[..1], "windows", "x86_64").as_deref(), Some("App-Setup-2.1.0.exe"));
        assert_eq!(picked(&["App-2.1.0.msi", "App-Setup-2.1.0.exe"], "windows", "x86_64"), None, "tie: setup exe and msi score alike → not confident");
        assert_eq!(picked(&app, "linux", "x86_64").as_deref(), Some("App-2.1.0.AppImage"));
        // x64-only build on Windows on Arm (emulated) and 32-bit on x64
        assert_eq!(picked(&["t-win-x64.zip"], "windows", "aarch64").as_deref(), Some("t-win-x64.zip"));
        assert_eq!(picked(&["t-win32.zip", "t-win64.zip"], "windows", "x86_64").as_deref(), Some("t-win64.zip"));
        assert_eq!(picked(&["t-linux-arm64.tar.gz"], "linux", "x86_64"), None);
        // nothing confident: only an OS-less zip, only checksums/signatures, two equal candidates
        assert_eq!(picked(&["tool.zip"], "windows", "x86_64"), None);
        assert_eq!(picked(&["a.exe.sig", "a.exe.asc", "checksums.txt", "a.minisig"], "windows", "x86_64"), None);
        assert_eq!(picked(&["a-windows.zip", "b-windows.zip"], "windows", "x86_64"), None);
        assert_eq!(picked(&["tool-cli-win64.zip", "tool-gui-win64.zip"], "windows", "x86_64").as_deref(), Some("tool-gui-win64.zip"));
        assert!(matches!(pick(&[], "windows", "x86_64"), Pick::Nothing(_)));
    }

    #[test]
    fn safe_components() {
        for (raw, want) in [
            ("v1.0.0", "v1.0.0"),
            ("release/1.0", "release_1.0"),
            ("..", "_"),
            (".", "_"),
            ("", "_"),
            ("a:b*c?", "a_b_c_"),
            ("con", "_con"),
            ("NUL.txt", "_NUL.txt"),
            ("x. . ", "x"),
            (".git", "_.git"),
            ("a\\..\\b", "a_.._b"),
        ] {
            assert_eq!(safe_component(raw), want, "{raw}");
            assert!(crate::winpath::path_ok_here(&safe_component(raw)), "{raw}");
        }
        assert_eq!(safe_component(&"x".repeat(300)).len(), 120);
        assert_eq!(url_path_segment("v1.0/ä b"), "v1.0%2F%C3%A4%20b");
    }

    struct Fixture {
        _t: tempfile::TempDir,
        s: Shelf,
        id: i64,
        c: Arc<Canned>,
    }

    fn sha(b: &[u8]) -> String {
        hex(&Sha256::digest(b))
    }

    /// A github.com repo whose cached release list has one asset with `body` (and its digest when `digest`).
    fn fixture(body: &[u8], digest: Option<String>, size: u64) -> Fixture {
        let t = tempfile::tempdir().unwrap();
        let c = Canned::new();
        let s = testkit::shelf(t.path()).with_net(Net::new(c.clone()));
        let mut b = testkit::repo(&t.path().join("r"));
        b.file("a", b"1").commit("one", testkit::T0).tag("v1.0.0", false).set_head().origin("https://github.com/o/r.git");
        let id = testkit::register_raw(&s, &t.path().join("r"), Source::Imported).unwrap();
        let url = "https://github.com/o/r/releases/download/v1.0.0/app-win-x64.zip";
        let json = serde_json::json!([{
            "tag_name": "v1.0.0", "prerelease": false,
            "assets": [{"name": "app-win-x64.zip", "size": size, "browser_download_url": url, "digest": digest}]
        }]);
        ghreleases::store(&s, id, &json.to_string()).unwrap();
        c.redirect(url, "https://objects.githubusercontent.com/blob/1?token=x").on("https://objects.githubusercontent.com/blob/1?token=x", 200, &[], body.to_vec());
        Fixture { _t: t, s, id, c }
    }

    fn store_path(f: &Fixture) -> PathBuf {
        f.s.dirs.archive.join("_releases").join("github.com").join("o").join("r").join("v1.0.0").join("app-win-x64.zip")
    }

    fn no_partials(f: &Fixture) {
        let dir = store_path(f).parent().unwrap().to_path_buf();
        let left: Vec<_> = std::fs::read_dir(&dir).map(|d| d.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
        assert!(left.iter().all(|n| !n.to_string_lossy().contains(PARTIAL_MARK)), "{left:?}");
    }

    #[test]
    fn download_verifies_renames_and_skips_existing() {
        let body = b"PK\x03\x04 fake zip".to_vec();
        let f = fixture(&body, Some(format!("sha256:{}", sha(&body).to_uppercase())), body.len() as u64);
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let ev = events.clone();
        let ctx = JobCtx::new(Arc::new(AtomicBool::new(false)), move |p| ev.lock().unwrap().push(p.clone()));
        let r = download(&f.s, f.id, "v1.0.0", "app-win-x64.zip", &ctx).unwrap();
        let JobResult::Downloaded { path, size, sha256_checked, already, file, .. } = r else { panic!("{r:?}") };
        assert_eq!((size, sha256_checked, already, file.as_str()), (body.len() as u64, true, false, "app-win-x64.zip"));
        assert_eq!(PathBuf::from(&path), store_path(&f));
        assert_eq!(std::fs::read(&path).unwrap(), body);
        assert!(events.lock().unwrap().iter().any(|p| matches!(p, Progress::Downloading { .. })));
        no_partials(&f);
        assert_eq!(f.c.seen().len(), 2, "one redirect hop");
        // listed as downloaded
        let rel = ghreleases::releases_with_cached(&f.s, f.id).unwrap();
        assert_eq!(rel[0].assets[0].downloaded.as_deref(), Some(path.as_str()));
        // again: nothing fetched
        let r = download(&f.s, f.id, "v1.0.0", "app-win-x64.zip", &JobCtx::noop()).unwrap();
        assert!(matches!(r, JobResult::Downloaded { already: true, sha256_checked: true, .. }), "{r:?}");
        assert_eq!(f.c.seen().len(), 2);
        // unknown tag / asset
        assert_eq!(download(&f.s, f.id, "v9", "app-win-x64.zip", &JobCtx::noop()).unwrap_err().code(), "notFound");
        assert_eq!(download(&f.s, f.id, "v1.0.0", "x.zip", &JobCtx::noop()).unwrap_err().code(), "notFound");
    }

    #[test]
    fn checksum_mismatch_discards() {
        let body = b"real bytes".to_vec();
        let f = fixture(&body, Some(format!("sha256:{}", "0".repeat(64))), body.len() as u64);
        let e = download(&f.s, f.id, "v1.0.0", "app-win-x64.zip", &JobCtx::noop()).unwrap_err();
        assert!(e.to_string().contains("Checksum mismatch"), "{e}");
        assert!(!store_path(&f).exists());
        no_partials(&f);
    }

    #[test]
    fn size_cap_and_announced_size() {
        let body = vec![7u8; 3 << 20];
        let f = fixture(&body, None, body.len() as u64);
        crate::settings::save(&f.s.db, &crate::settings::Settings { download_size_cap_mb: 1, ..Default::default() }).unwrap();
        assert_eq!(download(&f.s, f.id, "v1.0.0", "app-win-x64.zip", &JobCtx::noop()).unwrap_err().code(), "refused");
        assert!(f.c.seen().is_empty(), "refused from the announced size, before any request");
        // the API under-announces: the streamed cap still holds
        let f = fixture(&body, None, 10);
        crate::settings::save(&f.s.db, &crate::settings::Settings { download_size_cap_mb: 1, ..Default::default() }).unwrap();
        assert_eq!(download(&f.s, f.id, "v1.0.0", "app-win-x64.zip", &JobCtx::noop()).unwrap_err().code(), "refused");
        assert!(!store_path(&f).exists());
        no_partials(&f);
    }

    #[test]
    fn cancel_removes_the_partial() {
        let body = vec![1u8; 300 * 1024];
        let f = fixture(&body, None, body.len() as u64);
        let flag = Arc::new(AtomicBool::new(false));
        let fl = flag.clone();
        let ctx = JobCtx::new(flag.clone(), move |p| {
            if matches!(p, Progress::Downloading { .. }) {
                fl.store(true, Ordering::SeqCst);
            }
        });
        assert_eq!(download(&f.s, f.id, "v1.0.0", "app-win-x64.zip", &ctx).unwrap_err().code(), "cancelled");
        assert!(!store_path(&f).exists());
        no_partials(&f);
    }

    #[test]
    fn foreign_download_urls_and_hosts_are_refused() {
        let t = tempfile::tempdir().unwrap();
        let c = Canned::new();
        let s = testkit::shelf(t.path()).with_net(Net::new(c.clone()));
        let mut b = testkit::repo(&t.path().join("r"));
        b.file("a", b"1").commit("one", testkit::T0).set_head().origin("https://github.com/o/r.git");
        let id = testkit::register_raw(&s, &t.path().join("r"), Source::Imported).unwrap();
        let json = serde_json::json!([{"tag_name": "v1", "assets": [
            {"name": "a.zip", "size": 1, "browser_download_url": "https://evil.example/a.zip"},
            {"name": "b.zip", "size": 1, "browser_download_url": "https://github.com/other/repo/releases/download/v1/b.zip"},
            {"name": "c.zip", "size": 1, "browser_download_url": "https://github.com/o/r/releases/download/v1/c.zip"}
        ]}]);
        ghreleases::store(&s, id, &json.to_string()).unwrap();
        c.redirect("https://github.com/o/r/releases/download/v1/c.zip", "https://evil.example/c.zip");
        for a in ["a.zip", "b.zip", "c.zip"] {
            assert_eq!(download(&s, id, "v1", a, &JobCtx::noop()).unwrap_err().code(), "refused", "{a}");
        }
        assert_eq!(c.seen_urls(), ["https://github.com/o/r/releases/download/v1/c.zip"]);
    }

    #[test]
    fn latest_decision_and_partial_cleanup() {
        let body = b"x".to_vec();
        let f = fixture(&body, None, 1);
        let gh = ghreleases::cached(&f.s, f.id).unwrap().unwrap();
        let on_windows_x64 = std::env::consts::OS == "windows" && std::env::consts::ARCH == "x86_64";
        match latest_for_this_pc(&f.s, f.id, &gh).unwrap() {
            Latest::Download { tag, asset } => {
                assert!(on_windows_x64);
                assert_eq!((tag.as_str(), asset.as_str()), ("v1.0.0", "app-win-x64.zip"));
            }
            Latest::Nothing(why) => assert!(!on_windows_x64, "{why}"),
            Latest::Already { .. } => panic!(),
        }
        if on_windows_x64 {
            download(&f.s, f.id, "v1.0.0", "app-win-x64.zip", &JobCtx::noop()).unwrap();
            assert!(matches!(latest_for_this_pc(&f.s, f.id, &gh).unwrap(), Latest::Already { .. }));
        }
        let dir = store_path(&f).parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".big.zip.partial-123"), b"x").unwrap();
        std::fs::write(dir.join("keep.zip"), b"x").unwrap();
        assert_eq!(cleanup_partials(&f.s.dirs.archive).unwrap(), 1);
        assert!(dir.join("keep.zip").exists());
        // the clone cleanup leaves the store alone
        assert_eq!(crate::clone::cleanup_partials(&f.s.dirs.archive).unwrap(), 0);
        assert!(dir.join("keep.zip").exists());
    }
}
