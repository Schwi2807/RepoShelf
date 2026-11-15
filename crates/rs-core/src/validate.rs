//! Input validation (ARCH §3.2, §5.2, §5.3; NFR-S4, NFR-S5, FR-6, FR-20, D-1). Pure functions: nothing here
//! touches git, the filesystem or the network.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_GIT_PATH_BYTES: usize = 4096;
pub const MAX_REV_BYTES: usize = 1024;
pub const MAX_DESCRIPTION_CHARS: usize = 350;
pub const MAX_TOPICS: usize = 20;
pub const MAX_URL_BYTES: usize = 2048;

/// A validated git path inside a tree ("" = root). Never joined onto a filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitPath(String);

impl GitPath {
    pub fn root() -> GitPath {
        GitPath(String::new())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

/// A syntactically valid ref spec (branch/tag/remote name, `refs/...`, `HEAD` or a hex prefix). Classification
/// happens in `refs::resolve` (ARCH §3.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RevSpec(String);

impl RevSpec {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated, rebuilt public https clone URL (ARCH §5.3). `clone::clone` accepts only this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneUrl {
    /// `https://host[:port]/seg/…/name.git`
    pub url: String,
    pub host: String,
    pub owner: String,
    pub name: String,
    /// `<host>/<owner>/<name>.git`; middle segments folded into the name dir (`b-c.git`).
    pub rel_dir: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataInput {
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub topics: Vec<String>,
}

impl MetadataInput {
    /// Normalised copy: trimmed description (≤ 350 chars, empty → None), homepage (http/https), topics.
    pub fn validate(&self) -> Result<MetadataInput> {
        let description = match self.description.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(d) => {
                let n = d.chars().count();
                if n > MAX_DESCRIPTION_CHARS {
                    return Err(Error::InvalidInput(format!(
                        "Description is {n} characters; the limit is {MAX_DESCRIPTION_CHARS}."
                    )));
                }
                Some(d.to_string())
            }
        };
        let homepage = match self.homepage.as_deref() {
            None => None,
            Some(h) => homepage(h)?,
        };
        Ok(MetadataInput { description, homepage, topics: topics(&self.topics)? })
    }
}

/// `X:` alone or followed by `/` or `\` (D-34: `a:b.txt` is a legal git file name and stays browsable).
fn is_drive_prefix(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || b[2] == b'/' || b[2] == b'\\')
}

/// Git path inside a tree. Refuses leading `/`, `\`, NUL, a drive prefix, `.`/`..`/empty components, > 4096 bytes.
pub fn git_path(p: &str) -> Result<GitPath> {
    let bad = |why: &str| Err(Error::InvalidPath(format!("{why}: {:?}", truncate(p, 120))));
    if p.is_empty() {
        return Ok(GitPath::root());
    }
    if p.len() > MAX_GIT_PATH_BYTES {
        return bad("path is too long");
    }
    if p.contains('\0') {
        return bad("NUL byte in path");
    }
    if p.contains('\\') {
        return bad("backslash in path");
    }
    if p.starts_with('/') {
        return bad("absolute path");
    }
    if is_drive_prefix(p) {
        return bad("drive letter in path");
    }
    for c in p.split('/') {
        match c {
            "" => return bad("empty path component"),
            "." | ".." => return bad("`.` or `..` in path"),
            _ => {}
        }
    }
    Ok(GitPath(p.to_string()))
}

/// Ref syntax only (no lookup). `HEAD` or a name valid as `refs/heads/<s>`; hex-looking names stay names.
pub fn rev(s: &str) -> Result<RevSpec> {
    let bad = |why: &str| Err(Error::InvalidRev(format!("{why}: {:?}", truncate(s, 120))));
    if s.is_empty() {
        return bad("empty ref");
    }
    if s.len() > MAX_REV_BYTES {
        return bad("ref is too long");
    }
    if s.chars().any(|c| c.is_control() || c == ' ') {
        return bad("spaces or control characters in ref");
    }
    // Option-looking names (`--upload-pack=…`): git itself refuses branch/tag names starting with '-'; refused here
    // as defence in depth even though no git CLI is ever run (D-88, found by the M8 hostile IPC table).
    if s.starts_with('-') {
        return bad("ref starts with '-'");
    }
    for needle in ["..", "@{", "^", "~", ":", "\\"] {
        if s.contains(needle) {
            return bad("revision expressions are not allowed");
        }
    }
    if s == "HEAD" {
        return Ok(RevSpec(s.to_string()));
    }
    if !git2::Reference::is_valid_name(&format!("refs/heads/{s}")) {
        return bad("not a valid ref name");
    }
    Ok(RevSpec(s.to_string()))
}

/// Exactly 40 hex characters (returned lower-case).
pub fn oid(s: &str) -> Result<git2::Oid> {
    if s.len() != 40 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::InvalidRev(format!("not a full commit id: {:?}", truncate(s, 60))));
    }
    git2::Oid::from_str(&s.to_ascii_lowercase()).map_err(|e| Error::InvalidRev(e.message().to_string()))
}

fn seg_ok(seg: &str, extra: &[char]) -> bool {
    !seg.is_empty()
        && seg != "."
        && seg != ".."
        && seg.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') || extra.contains(&c))
}

/// Host directory name for the archive (`:` of a port or IPv6 is not storable on Windows).
fn host_dir(host: &str, port: Option<u16>) -> String {
    let h: String = host
        .trim_matches(|c| c == '[' || c == ']')
        .chars()
        .map(|c| if c == ':' { '_' } else { c })
        .collect();
    match port {
        Some(p) => format!("{h}_{p}"),
        None => h,
    }
}

/// Clone URL rules (ARCH §5.3). Rejections happen on the raw input before `url::Url::parse` (which would
/// silently fold `..`, `\` and whitespace).
pub fn clone_url(input: &str) -> Result<CloneUrl> {
    let bad = |m: &str| Err(Error::InvalidUrl(m.to_string()));
    let s = input.trim();
    if s.is_empty() {
        return bad("Enter a repository URL, e.g. https://github.com/owner/repo or owner/repo.");
    }
    if s.len() > MAX_URL_BYTES {
        return bad("That URL is too long.");
    }
    if s.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return bad("The URL contains spaces or control characters.");
    }
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("git@") || lower.starts_with("ssh://") || lower.starts_with("git+ssh://") || lower.starts_with("ssh+git://") {
        return bad("SSH addresses are not supported. Use the https:// address of the repository.");
    }
    if lower.starts_with("git://") {
        return bad("git:// addresses are not supported. Use the https:// address of the repository.");
    }
    if lower.starts_with("file:") {
        return bad("Local repositories cannot be cloned. Use Import folder… to add a repository from this PC.");
    }
    if lower.starts_with("http://") {
        return bad("Only https:// addresses are allowed (http:// is not encrypted).");
    }
    if s.contains('\\') {
        return bad("Backslashes are not allowed. Local paths cannot be cloned; use Import folder… instead.");
    }
    if is_drive_prefix(s) {
        return bad("Local paths cannot be cloned. Use Import folder… to add a repository from this PC.");
    }
    if s.starts_with('/') || s.starts_with('.') {
        return bad("Local paths cannot be cloned. Use Import folder… to add a repository from this PC.");
    }

    let full = if let Some((scheme, _)) = s.split_once("://") {
        if !scheme.eq_ignore_ascii_case("https") {
            return bad("Only https:// addresses are supported.");
        }
        s.to_string()
    } else {
        if s.contains('@') {
            return bad("Addresses with a user name or password are not allowed. Only public repositories can be cloned.");
        }
        let segs: Vec<&str> = s.trim_end_matches('/').split('/').collect();
        match segs.len() {
            0 | 1 => return bad("Enter owner/repo or a full https:// address."),
            2 => {
                if !segs.iter().all(|x| seg_ok(x, &[])) {
                    return bad("owner/repo may only contain letters, digits, '.', '_' and '-'.");
                }
                format!("https://github.com/{}/{}", segs[0], segs[1])
            }
            _ => {
                if !segs[0].contains('.') {
                    return bad("Add the host, e.g. github.com/owner/repo or https://host/owner/repo.");
                }
                format!("https://{s}")
            }
        }
    };

    // Raw path checks before the URL parser normalises anything.
    let after_scheme = &full[full.find("://").map(|i| i + 3).unwrap_or(0)..];
    let (authority, rest) = match after_scheme.find(['/', '?', '#']) {
        Some(i) => (&after_scheme[..i], &after_scheme[i..]),
        None => (after_scheme, ""),
    };
    if authority.is_empty() {
        return bad("The address has no host.");
    }
    if authority.contains('@') {
        return bad("Addresses with a user name or password are not allowed. Only public repositories can be cloned.");
    }
    let raw_path = rest.split(['?', '#']).next().unwrap_or("");
    let raw_segs: Vec<&str> = raw_path.split('/').skip(1).collect();
    let trimmed: &[&str] = match raw_segs.split_last() {
        Some((&"", init)) => init,
        _ => &raw_segs,
    };
    if trimmed.iter().any(|x| x.is_empty() || *x == "." || *x == ".." || x.contains('%')) {
        return bad("The repository path contains empty, '.', '..' or encoded parts.");
    }

    let url = url::Url::parse(&full).map_err(|e| Error::InvalidUrl(format!("That is not a valid URL ({e}).")))?;
    if url.scheme() != "https" {
        return bad("Only https:// addresses are supported.");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return bad("Addresses with a user name or password are not allowed. Only public repositories can be cloned.");
    }
    let host = match url.host() {
        Some(url::Host::Domain(d)) if !d.is_empty() => d.to_string(),
        Some(url::Host::Ipv4(ip)) => ip.to_string(),
        Some(url::Host::Ipv6(ip)) => format!("[{ip}]"),
        _ => return bad("The address has no host."),
    };
    let mut segs: Vec<String> = url
        .path_segments()
        .map(|it| it.filter(|x| !x.is_empty()).map(str::to_string).collect())
        .unwrap_or_default();
    if let Some(last) = segs.last_mut()
        && let Some(stripped) = last.strip_suffix(".git")
    {
        *last = stripped.to_string();
    }
    if segs.len() < 2 {
        return bad("The address needs an owner and a repository name, e.g. https://github.com/owner/repo.");
    }
    if !segs.iter().all(|x| seg_ok(x, &['~'])) {
        return bad("The repository path may only contain letters, digits, '.', '_', '-' and '~'.");
    }
    let owner = segs[0].clone();
    let name = segs[segs.len() - 1].clone();
    let port = url.port();
    let authority = match port {
        Some(p) => format!("{host}:{p}"),
        None => host.clone(),
    };
    let rebuilt = format!("https://{authority}/{}.git", segs.join("/"));
    let name_dir = format!("{}.git", segs[1..].join("-"));
    let host_d = host_dir(&host, port);
    for comp in [host_d.as_str(), owner.as_str(), name_dir.as_str()] {
        if !crate::winpath::path_ok_here(comp) {
            return Err(Error::InvalidUrl(format!(
                "The name {comp:?} cannot be stored as a folder on Windows, so this repository cannot be archived."
            )));
        }
    }
    let rel_dir: PathBuf = [host_d.as_str(), owner.as_str(), name_dir.as_str()].iter().collect();
    Ok(CloneUrl { url: rebuilt, host, owner, name, rel_dir })
}

/// FR-20 topics: lower-cased, `^[a-z0-9][a-z0-9-]{0,49}$`, ≤ 20, de-duplicated (first wins), blanks skipped.
pub fn topics(v: &[String]) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for raw in v {
        let t = raw.trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        let ok = t.len() <= 50
            && t.as_bytes()[0].is_ascii_alphanumeric()
            && t.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !ok {
            return Err(Error::InvalidInput(format!(
                "Topic {:?}: use lowercase letters, digits and '-', start with a letter or digit, at most 50 characters.",
                truncate(raw, 60)
            )));
        }
        if !out.contains(&t) {
            out.push(t);
        }
    }
    if out.len() > MAX_TOPICS {
        return Err(Error::InvalidInput(format!("{} topics; the limit is {MAX_TOPICS}.", out.len())));
    }
    Ok(out)
}

fn http_url(s: &str, what: &str) -> Result<String> {
    let bad = |m: String| Err(Error::InvalidInput(m));
    let url = match url::Url::parse(s) {
        Ok(u) => u,
        Err(_) => return bad(format!("{what} must be a full http:// or https:// address.")),
    };
    if !matches!(url.scheme(), "http" | "https") {
        return bad(format!("{what} must start with http:// or https://."));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return bad(format!("{what} must not contain a user name or password."));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return bad(format!("{what} has no host."));
    }
    Ok(url.to_string())
}

/// FR-20 homepage: empty → None; else http/https without userinfo.
pub fn homepage(s: &str) -> Result<Option<String>> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    http_url(s, "Homepage").map(Some)
}

/// URL handed to the system browser (NFR-S3): http/https only, no userinfo.
pub fn external_url(s: &str) -> Result<String> {
    http_url(s.trim(), "Link").map_err(|e| Error::InvalidUrl(e.to_string()))
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// GitHub user / organisation name in an Explore address (D-123): 1–39 of `A-Za-z0-9-`, no leading `-`.
pub fn github_owner(s: &str) -> Result<&str> {
    let ok = !s.is_empty() && s.len() <= 39 && !s.starts_with('-') && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if ok { Ok(s) } else { Err(Error::InvalidInput(format!("Not a GitHub user or organisation name: {:?}", truncate(s, 60)))) }
}

/// GitHub repository name in an Explore address (D-123): 1–100 of `A-Za-z0-9._-`, no leading `-`, not `.` / `..`,
/// no `.git` suffix (the clone address adds it).
pub fn github_repo(s: &str) -> Result<&str> {
    let ok = !s.is_empty()
        && s.len() <= 100
        && !s.starts_with('-')
        && s != "."
        && s != ".."
        && !s.to_ascii_lowercase().ends_with(".git")
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if ok { Ok(s) } else { Err(Error::InvalidInput(format!("Not a GitHub repository name: {:?}", truncate(s, 60)))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSTILE: &[&str] = &["..", "a/../b", "/etc", "C:\\x", "c:/x", "a\\b", "a\0b", "refs/../x", "HEAD~1", "a..b", "@{-1}"];


    #[test]
    fn github_owner_and_repo_names() {
        for ok in ["octo", "Octo-Cat", "a", "a1-b2", &"x".repeat(39)] {
            assert_eq!(github_owner(ok).unwrap(), ok);
        }
        for bad in ["", "-x", "a b", "a/b", "a\\b", "a.b", "a_b", "..", "%2e%2e", "a\0b", "über", &"x".repeat(40), "a\nb", "C:"] {
            assert!(matches!(github_owner(bad), Err(Error::InvalidInput(_))), "owner {bad:?} accepted");
        }
        for ok in ["repo", ".github", "a.b_c-d", "x.git.y", &"x".repeat(100)] {
            assert_eq!(github_repo(ok).unwrap(), ok);
        }
        for bad in ["", "-x", ".", "..", "a/b", "a\\b", "a b", "a.git", "A.GIT", "%2e%2e", "a\0b", "a:b", "a?b", "a#b", &"x".repeat(101)] {
            assert!(matches!(github_repo(bad), Err(Error::InvalidInput(_))), "repo {bad:?} accepted");
        }
    }

    #[test]
    fn hostile_table_refused_as_path() {
        for h in HOSTILE {
            // HEAD~1, a..b and @{-1} are legal file names; everything else must be refused as a path.
            if matches!(*h, "HEAD~1" | "a..b" | "@{-1}") {
                continue;
            }
            assert!(matches!(git_path(h), Err(Error::InvalidPath(_))), "path {h:?} accepted");
        }
        for bad in ["a//b", "a/", "./a", "a/.", "\\\\srv\\x", "Z:"] {
            assert!(matches!(git_path(bad), Err(Error::InvalidPath(_))), "path {bad:?} accepted");
        }
        assert!(git_path(&"a/".repeat(2049)).is_err());
    }

    #[test]
    fn hostile_table_refused_as_rev() {
        for h in HOSTILE {
            assert!(matches!(rev(h), Err(Error::InvalidRev(_))), "rev {h:?} accepted");
        }
        for bad in ["", "a b", "a^", "a:b", "x.lock", "a/.b", "a/", "main\n", "-x", "--upload-pack=x", "--"] {
            assert!(matches!(rev(bad), Err(Error::InvalidRev(_))), "rev {bad:?} accepted");
        }
    }

    #[test]
    fn good_paths_and_revs() {
        assert!(git_path("").unwrap().is_root());
        for ok in ["README.md", "src/a.rs", "a:b.txt", "dir with space/x", "..a", "a..", "ünï/cødé"] {
            assert_eq!(git_path(ok).unwrap().as_str(), ok);
        }
        for ok in ["main", "HEAD", "feature/x", "refs/tags/v1.0", "origin/main", "v1.2.3-rc1", "dead", "2024", "a1b2c3d"] {
            assert_eq!(rev(ok).unwrap().as_str(), ok, "{ok}");
        }
    }

    #[test]
    fn hex_looking_names_stay_names() {
        // syntax only: classification is refs::resolve's job, so these are plain RevSpecs
        for s in ["dead", "beef", "cafe1234", "0123456789abcdef0123456789abcdef01234567"] {
            assert_eq!(rev(s).unwrap().as_str(), s);
        }
    }

    #[test]
    fn oid_needs_40_hex() {
        assert!(oid(&"a".repeat(40)).is_ok());
        assert_eq!(oid(&"A".repeat(40)).unwrap().to_string(), "a".repeat(40));
        for bad in ["abc", &"g".repeat(40), &"a".repeat(41), "", "HEAD"] {
            assert!(matches!(oid(bad), Err(Error::InvalidRev(_))), "{bad}");
        }
    }

    fn rejected(input: &str) -> String {
        match clone_url(input) {
            Err(Error::InvalidUrl(m)) => m,
            other => panic!("{input:?} not rejected: {other:?}"),
        }
    }

    #[test]
    fn every_rejected_clone_form_has_a_message() {
        let cases: &[(&str, &str)] = &[
            ("git@github.com:o/r.git", "SSH"),
            ("ssh://git@github.com/o/r", "SSH"),
            ("git://github.com/o/r", "git://"),
            ("file:///C:/x/repo", "Import folder"),
            ("http://github.com/o/r", "http://"),
            ("https://user:pw@github.com/o/r", "user name or password"),
            ("https://token@github.com/o/r", "user name or password"),
            ("user@github.com/o/r", "user name or password"),
            ("C:\\repos\\x", "Backslashes"),
            ("c:/repos/x", "Local paths"),
            ("/home/x/repo", "Local paths"),
            ("./repo", "Local paths"),
            ("../repo", "Local paths"),
            ("a/b/c", "Add the host"),
            ("justone", "owner/repo"),
            ("", "Enter a repository URL"),
            ("ftp://example.com/o/r", "https://"),
            ("https://github.com/onlyowner", "owner and a repository"),
            ("https://github.com/a/../b/c", "'..'"),
            ("https://github.com/a/%2e%2e/c", "encoded"),
            ("https://github.com/a//b", "empty"),
            ("https://github.com/o/r with space", "spaces"),
            ("https:///o/r", "host"),
            ("o/r!", "letters, digits"),
            ("https://example.com/aux/r", "cannot be stored"),
            ("https://example.com/x./r", "cannot be stored"),
            ("https://example.com/con/r", "cannot be stored"),
        ];
        for (input, needle) in cases {
            let m = rejected(input);
            assert!(m.contains(needle), "{input:?}: message {m:?} lacks {needle:?}");
            assert!(m.len() > 15, "message too terse: {m}");
        }
    }

    #[test]
    fn shorthand_and_normalisation() {
        let u = clone_url("octo/demo").unwrap();
        assert_eq!(u.url, "https://github.com/octo/demo.git");
        assert_eq!((u.host.as_str(), u.owner.as_str(), u.name.as_str()), ("github.com", "octo", "demo"));
        assert_eq!(u.rel_dir, PathBuf::from("github.com").join("octo").join("demo.git"));

        for same in [
            "https://github.com/octo/demo",
            "https://github.com/octo/demo.git",
            "https://github.com/octo/demo/",
            "https://GitHub.com/octo/demo.git/",
            "  https://github.com/octo/demo?tab=readme#top  ",
            "github.com/octo/demo",
            "octo/demo.git",
        ] {
            assert_eq!(clone_url(same).unwrap(), u, "{same}");
        }
    }

    #[test]
    fn nested_groups_fold_into_name_dir() {
        let u = clone_url("gitlab.com/a/b/c").unwrap();
        assert_eq!(u.url, "https://gitlab.com/a/b/c.git");
        assert_eq!((u.owner.as_str(), u.name.as_str()), ("a", "c"));
        assert_eq!(u.rel_dir, PathBuf::from("gitlab.com").join("a").join("b-c.git"));
    }

    #[test]
    fn ports_and_ip_hosts_get_storable_dirs() {
        let u = clone_url("https://git.example.org:8443/o/r").unwrap();
        assert_eq!(u.url, "https://git.example.org:8443/o/r.git");
        assert_eq!(u.rel_dir, PathBuf::from("git.example.org_8443").join("o").join("r.git"));
        let u = clone_url("https://[::1]/o/r").unwrap();
        assert_eq!(u.host, "[::1]");
        assert_eq!(u.rel_dir, PathBuf::from("__1").join("o").join("r.git"));
        let u = clone_url("https://example.com:443/o/r").unwrap();
        assert_eq!(u.url, "https://example.com/o/r.git");
    }

    #[test]
    fn topics_rules() {
        let t = |v: &[&str]| topics(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(t(&["Rust", "rust", " cli ", "", "tauri-2"]).unwrap(), vec!["rust", "cli", "tauri-2"]);
        for bad in ["-lead", "has space", "under_score", "ümlaut", "dot.ted", &"a".repeat(51)] {
            assert!(matches!(t(&[bad]), Err(Error::InvalidInput(_))), "{bad}");
        }
        assert!(t(&[&"a".repeat(50)]).is_ok());
        let twenty: Vec<String> = (0..20).map(|i| format!("t{i}")).collect();
        assert_eq!(topics(&twenty).unwrap().len(), 20);
        let mut many = twenty.clone();
        many.push("t20".into());
        assert!(matches!(topics(&many), Err(Error::InvalidInput(_))));
        // duplicates do not count against the limit
        let mut dup = twenty;
        dup.push("T0".into());
        assert_eq!(topics(&dup).unwrap().len(), 20);
    }

    #[test]
    fn homepage_rules() {
        assert_eq!(homepage("").unwrap(), None);
        assert_eq!(homepage("  ").unwrap(), None);
        assert_eq!(homepage("https://example.com").unwrap().as_deref(), Some("https://example.com/"));
        assert!(homepage("http://example.com/x").unwrap().is_some());
        for bad in ["javascript:alert(1)", "ftp://x.org", "example.com", "https://u:p@x.org", "file:///c:/x", "mailto:a@b.c"] {
            assert!(matches!(homepage(bad), Err(Error::InvalidInput(_))), "{bad}");
        }
    }

    #[test]
    fn external_url_rules() {
        assert!(external_url("https://example.com/a?b#c").is_ok());
        for bad in ["javascript:alert(1)", "file:///C:/x", "ftp://x.org", "https://user@x.org", "data:text/html,x", ""] {
            assert!(matches!(external_url(bad), Err(Error::InvalidUrl(_))), "{bad}");
        }
    }

    #[test]
    fn metadata_validate() {
        let m = MetadataInput {
            description: Some(format!("  {}  ", "é".repeat(350))),
            homepage: Some("".into()),
            topics: vec!["Rust".into()],
        };
        let v = m.validate().unwrap();
        assert_eq!(v.description.as_ref().unwrap().chars().count(), 350); // chars, not bytes
        assert_eq!(v.homepage, None);
        assert_eq!(v.topics, vec!["rust"]);
        let long = MetadataInput { description: Some("x".repeat(351)), ..Default::default() };
        assert!(matches!(long.validate(), Err(Error::InvalidInput(_))));
        let blank = MetadataInput { description: Some("   ".into()), ..Default::default() };
        assert_eq!(blank.validate().unwrap().description, None);
        let bad_home = MetadataInput { homepage: Some("javascript:x".into()), ..Default::default() };
        assert!(bad_home.validate().is_err());
    }
}
