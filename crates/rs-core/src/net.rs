//! Shared HTTP client (v0.2.0 M11; D-98). Every HTTP request RepoShelf makes goes through `Net`:
//!
//! - https only, default port only, no userinfo; the host must be a DNS name (no IP literal) that is not
//!   `localhost`, `*.localhost`, `*.local`, `*.internal`, `*.home.arpa` or a single label;
//! - every resolved address must be public (no loopback, private, link-local, CGNAT, multicast, documentation…);
//!   the real transport checks the addresses it actually connects to (a checking ureq `Resolver`), so a DNS
//!   answer cannot change between the check and the connection;
//! - redirects are followed by hand (≤ `Profile::redirects` hops), every hop re-checked against the same rules
//!   and the profile's host allow-list; `http:` targets are refused;
//! - size cap (announced `Content-Length` and while streaming) and a per-profile time cap; no cookies, no
//!   credentials, `User-Agent: RepoShelf/<version>`.
//!
//! The single-hop `Transport` is the test seam: tests run the production redirect / host / size logic against
//! canned answers (`Canned`), and a `Shelf` built for tests uses `Net::offline()`, which refuses (and counts)
//! every request, so no test can reach the network by accident. The ureq transport exists only with the
//! `ghreleases` feature; without it `Net::real()` answers `unsupported`.

use crate::error::{Error, Result};
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::{Host, Url};

pub const USER_AGENT: &str = concat!("RepoShelf/", env!("CARGO_PKG_VERSION"));

/// Release-asset hosts (lead decision): GitHub's download host and its CDN hosts.
pub const ASSET_HOSTS: &[&str] =
    &["github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com", "github-releases.githubusercontent.com"];
pub const API_HOSTS: &[&str] = &["api.github.com"];

/// Which hosts a request (and each redirect hop) may reach, on top of the public-address rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hosts {
    AnyPublic,
    Only(&'static [&'static str]),
}

/// Per-use limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    pub hosts: Hosts,
    pub redirects: u8,
    pub max_bytes: u64,
    pub timeout: Duration,
}

impl Profile {
    /// api.github.com JSON: no redirect (D-49), 10 s, 16 MB.
    pub const API: Profile = Profile { hosts: Hosts::Only(API_HOSTS), redirects: 0, max_bytes: 16 << 20, timeout: Duration::from_secs(10) };
    /// README images: any public host, ≤ 5 hops, 5 MB, 30 s.
    pub const IMAGE: Profile = Profile { hosts: Hosts::AnyPublic, redirects: 5, max_bytes: 5 << 20, timeout: Duration::from_secs(30) };
    /// Release assets: GitHub download hosts only, ≤ 5 hops; `max_bytes` = the size-cap setting; 2 h.
    pub fn asset(max_bytes: u64) -> Profile {
        Profile { hosts: Hosts::Only(ASSET_HOSTS), redirects: 5, max_bytes, timeout: Duration::from_secs(2 * 3600) }
    }
}

/// One response hop as the transport delivers it. Header names are lower-case.
pub struct RawResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn Read + Send>,
}

/// The test seam: one GET, redirects NOT followed, plus name resolution.
pub trait Transport: Send + Sync {
    fn resolve(&self, host: &str) -> std::io::Result<Vec<IpAddr>>;
    fn get(&self, url: &Url, headers: &[(&str, String)], timeout: Duration) -> Result<RawResponse>;
}

/// A checked, final response (after redirects).
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub final_url: Url,
    body: Box<dyn Read + Send>,
    max_bytes: u64,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn content_length(&self) -> Option<u64> {
        self.header("content-length").and_then(|v| v.trim().parse().ok())
    }
    /// `Content-Type` without parameters, lower-cased.
    pub fn mime(&self) -> String {
        self.header("content-type").unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase()
    }
    /// Whole body, refused above the profile's cap (announced or streamed).
    pub fn bytes(self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let max = self.max_bytes;
        self.stream(&mut |chunk| {
            out.extend_from_slice(chunk);
            Ok(())
        })?;
        debug_assert!(out.len() as u64 <= max);
        Ok(out)
    }
    /// Streams the body in chunks to `sink` (a `Cancelled` from the sink stops it), enforcing the cap.
    pub fn stream(mut self, sink: &mut dyn FnMut(&[u8]) -> Result<()>) -> Result<u64> {
        if let Some(n) = self.content_length()
            && n > self.max_bytes
        {
            return Err(too_large(self.max_bytes));
        }
        let mut buf = vec![0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let n = self.body.read(&mut buf).map_err(|e| Error::Network(format!("the download broke off: {e}")))?;
            if n == 0 {
                return Ok(total);
            }
            total += n as u64;
            if total > self.max_bytes {
                return Err(too_large(self.max_bytes));
            }
            sink(&buf[..n])?;
        }
    }
}

fn too_large(max: u64) -> Error {
    Error::Refused(format!("The file is larger than the limit ({}).", human_bytes(max)))
}

pub fn human_bytes(n: u64) -> String {
    const MB: u64 = 1 << 20;
    if n >= 1 << 30 && n % (1 << 30) == 0 {
        format!("{} GB", n >> 30)
    } else if n >= MB {
        format!("{} MB", n / MB)
    } else {
        format!("{n} bytes")
    }
}

/// The client every module uses (held by `Shelf`). Cheap to clone.
#[derive(Clone)]
pub struct Net {
    t: Arc<dyn Transport>,
    /// GitHub API rate limit: no further API calls before this Unix time (per Shelf, D-101).
    rate_limited_until: Arc<Mutex<Option<i64>>>,
}

impl std::fmt::Debug for Net {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Net")
    }
}

impl Net {
    pub fn new(t: Arc<dyn Transport>) -> Net {
        Net { t, rate_limited_until: Arc::new(Mutex::new(None)) }
    }

    /// Production transport (feature `ghreleases`); without it every request answers `unsupported`.
    pub fn real() -> Net {
        #[cfg(feature = "ghreleases")]
        {
            Net::new(Arc::new(real::Ureq))
        }
        #[cfg(not(feature = "ghreleases"))]
        {
            Net::new(Arc::new(Unavailable))
        }
    }

    /// Refuses (and counts, `testkit::net_calls`) every request: the default for test shelves.
    pub fn offline() -> Net {
        Net::new(Arc::new(Offline))
    }

    pub fn rate_limited_until(&self) -> Option<i64> {
        *self.rate_limited_until.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn set_rate_limited_until(&self, t: Option<i64>) {
        *self.rate_limited_until.lock().unwrap_or_else(|e| e.into_inner()) = t;
    }

    /// Scheme / host / port rules (no DNS).
    pub fn check_url(url: &Url, hosts: Hosts) -> Result<String> {
        let refuse = |why: &str| Err(Error::Refused(format!("Refused {why}: {}", short(url))));
        if url.scheme() != "https" {
            return refuse("a non-https address");
        }
        if !url.username().is_empty() || url.password().is_some() {
            return refuse("an address with a user name");
        }
        if url.port().is_some() {
            return refuse("an address with a port");
        }
        let host = match url.host() {
            Some(Host::Domain(d)) => d.trim_end_matches('.').to_ascii_lowercase(),
            Some(Host::Ipv4(_) | Host::Ipv6(_)) => return refuse("an IP address"),
            None => return refuse("an address without a host"),
        };
        // `url` turns decimal/hex IPv4 spellings into Host::Ipv4, so a domain here is a real name.
        let local = host == "localhost"
            || !host.contains('.')
            || [".localhost", ".local", ".internal", ".home.arpa", ".lan", ".intranet"].iter().any(|s| host.ends_with(s));
        if local {
            return refuse("a local host");
        }
        if let Hosts::Only(list) = hosts
            && !list.contains(&host.as_str())
        {
            return refuse("a host outside the allowed list");
        }
        Ok(host)
    }

    /// GET with the profile's rules; redirects (301/302/303/307/308) followed by hand.
    pub fn get(&self, url: &str, headers: &[(&str, String)], p: Profile) -> Result<Response> {
        let mut url = Url::parse(url).map_err(|e| Error::InvalidUrl(format!("{e}: {url}")))?;
        let mut hops = 0u8;
        loop {
            let host = Net::check_url(&url, p.hosts)?;
            let addrs = self.t.resolve(&host).map_err(|e| Error::Network(format!("could not resolve {host}: {e}")))?;
            if addrs.is_empty() {
                return Err(Error::Network(format!("could not resolve {host}")));
            }
            if let Some(bad) = addrs.iter().find(|a| !is_public(a)) {
                return Err(Error::Refused(format!("Refused {host}: it resolves to a non-public address ({bad}).")));
            }
            let mut h: Vec<(&str, String)> = headers.to_vec();
            if !h.iter().any(|(k, _)| k.eq_ignore_ascii_case("user-agent")) {
                h.push(("User-Agent", USER_AGENT.to_string()));
            }
            let raw = self.t.get(&url, &h, p.timeout)?;
            if matches!(raw.status, 301 | 302 | 303 | 307 | 308) {
                let loc = raw.headers.iter().find(|(k, _)| k == "location").map(|(_, v)| v.clone()).unwrap_or_default();
                if p.redirects == 0 {
                    return Ok(Response { status: raw.status, headers: raw.headers, final_url: url, body: raw.body, max_bytes: p.max_bytes });
                }
                if hops >= p.redirects {
                    return Err(Error::Refused(format!("Too many redirects (more than {}).", p.redirects)));
                }
                if loc.is_empty() {
                    return Err(Error::Network("a redirect without a target".into()));
                }
                url = url.join(&loc).map_err(|e| Error::InvalidUrl(format!("bad redirect target: {e}")))?;
                hops += 1;
                continue;
            }
            return Ok(Response { status: raw.status, headers: raw.headers, final_url: url, body: raw.body, max_bytes: p.max_bytes });
        }
    }
}

fn short(u: &Url) -> String {
    let s = u.as_str();
    if s.len() > 120 { format!("{}…", &s[..s.char_indices().nth(119).map(|(i, _)| i).unwrap_or(s.len())]) } else { s.to_string() }
}

/// Globally routable unicast only.
pub fn is_public(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_v4(&v4);
            }
            let s = v6.segments();
            let unique_local = (s[0] & 0xfe00) == 0xfc00;
            let link_local = (s[0] & 0xffc0) == 0xfe80;
            let site_local = (s[0] & 0xffc0) == 0xfec0;
            let documentation = s[0] == 0x2001 && s[1] == 0x0db8;
            let nat64 = s[0] == 0x0064 && s[1] == 0xff9b; // embeds an IPv4 address
            let teredo_6to4 = (s[0] == 0x2001 && s[1] == 0) || s[0] == 0x2002;
            let compat = s[..6].iter().all(|x| *x == 0); // ::a.b.c.d and ::, ::1
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || unique_local
                || link_local
                || site_local
                || documentation
                || nat64
                || teredo_6to4
                || compat
                || *v6 == Ipv6Addr::LOCALHOST)
        }
    }
}

fn is_public_v4(ip: &Ipv4Addr) -> bool {
    let o = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || o[0] == 0
        || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64/10 shared address space
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0/24
        || (o[0] == 198 && (o[1] & 0xfe) == 18) // 198.18/15 benchmarking
        || o[0] >= 240) // reserved
}

struct Offline;

impl Transport for Offline {
    fn resolve(&self, _host: &str) -> std::io::Result<Vec<IpAddr>> {
        Ok(vec![IpAddr::V4(Ipv4Addr::new(140, 82, 112, 3))])
    }
    fn get(&self, url: &Url, _h: &[(&str, String)], _t: Duration) -> Result<RawResponse> {
        crate::clone::note_net_call();
        Err(Error::Network(format!("offline test shelf: {url}")))
    }
}

#[cfg(not(feature = "ghreleases"))]
struct Unavailable;

#[cfg(not(feature = "ghreleases"))]
impl Transport for Unavailable {
    fn resolve(&self, _host: &str) -> std::io::Result<Vec<IpAddr>> {
        Ok(vec![IpAddr::V4(Ipv4Addr::new(140, 82, 112, 3))])
    }
    fn get(&self, _url: &Url, _h: &[(&str, String)], _t: Duration) -> Result<RawResponse> {
        Err(Error::Unsupported("Network features are not included in this build.".into()))
    }
}

#[cfg(feature = "ghreleases")]
mod real {
    //! ureq 3 with redirects off, https only, no cookies (ureq's `cookies` feature is not enabled), and a
    //! resolver that refuses non-public addresses — the addresses ureq then connects to.
    use super::*;
    use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
    use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

    #[derive(Debug, Default)]
    struct PublicOnly(DefaultResolver);

    impl Resolver for PublicOnly {
        fn resolve(&self, uri: &ureq::http::Uri, config: &ureq::config::Config, timeout: NextTimeout) -> std::result::Result<ResolvedSocketAddrs, ureq::Error> {
            let addrs = self.0.resolve(uri, config, timeout)?;
            if addrs.iter().any(|a| !is_public(&a.ip())) {
                return Err(ureq::Error::Io(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "host resolves to a non-public address")));
            }
            Ok(addrs)
        }
    }

    pub struct Ureq;

    impl Transport for Ureq {
        fn resolve(&self, host: &str) -> std::io::Result<Vec<IpAddr>> {
            use std::net::ToSocketAddrs;
            Ok((host, 443).to_socket_addrs()?.map(|a| a.ip()).collect())
        }

        fn get(&self, url: &Url, headers: &[(&str, String)], timeout: Duration) -> Result<RawResponse> {
            let config = ureq::Agent::config_builder()
                .https_only(true)
                .max_redirects(0)
                .max_redirects_will_error(false)
                .http_status_as_error(false)
                .save_redirect_history(false)
                .timeout_connect(Some(Duration::from_secs(15)))
                .timeout_global(Some(timeout))
                .user_agent(USER_AGENT)
                .build();
            let agent = ureq::Agent::with_parts(config, DefaultConnector::default(), PublicOnly::default());
            crate::clone::note_net_call();
            let mut req = agent.get(url.as_str());
            for (k, v) in headers {
                req = req.header(*k, v.as_str());
            }
            let resp = req.call().map_err(|e| Error::Network(format!("{}: {e}", url.host_str().unwrap_or(""))))?;
            let status = resp.status().as_u16();
            let headers = resp
                .headers()
                .iter()
                .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_ascii_lowercase(), v.to_string())))
                .collect();
            let body = resp.into_body().into_reader();
            Ok(RawResponse { status, headers, body: Box::new(body) })
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Test seam: canned answers (rs-core tests, src-tauri tests via testkit)

#[cfg(any(test, feature = "testkit"))]
pub use canned::Canned;

#[cfg(any(test, feature = "testkit"))]
mod canned {
    use super::*;
    use std::collections::HashMap;

    /// One canned answer.
    #[derive(Clone, Debug)]
    pub struct Answer {
        pub status: u16,
        pub headers: Vec<(String, String)>,
        pub body: Vec<u8>,
    }

    /// Recorded request: URL + headers.
    pub type Seen = (String, Vec<(String, String)>);

    /// URL → queue of answers (the last one repeats); host → addresses (default: one public address).
    /// Unknown URLs answer `Network("no canned answer")`. Every request is recorded.
    #[derive(Default)]
    pub struct Canned {
        routes: Mutex<HashMap<String, Vec<Answer>>>,
        dns: Mutex<HashMap<String, Vec<IpAddr>>>,
        seen: Mutex<Vec<Seen>>,
    }

    impl Canned {
        pub fn new() -> Arc<Canned> {
            Arc::new(Canned::default())
        }
        pub fn on(&self, url: &str, status: u16, headers: &[(&str, &str)], body: impl Into<Vec<u8>>) -> &Self {
            let a = Answer {
                status,
                headers: headers.iter().map(|(k, v)| (k.to_ascii_lowercase(), v.to_string())).collect(),
                body: body.into(),
            };
            self.routes.lock().unwrap().entry(url.to_string()).or_default().push(a);
            self
        }
        pub fn redirect(&self, from: &str, to: &str) -> &Self {
            self.on(from, 302, &[("location", to)], Vec::new())
        }
        pub fn dns(&self, host: &str, ips: &[&str]) -> &Self {
            self.dns.lock().unwrap().insert(host.to_string(), ips.iter().map(|i| i.parse().unwrap()).collect());
            self
        }
        pub fn seen(&self) -> Vec<Seen> {
            self.seen.lock().unwrap().clone()
        }
        pub fn seen_urls(&self) -> Vec<String> {
            self.seen().into_iter().map(|(u, _)| u).collect()
        }
    }

    impl Transport for Canned {
        fn resolve(&self, host: &str) -> std::io::Result<Vec<IpAddr>> {
            Ok(self.dns.lock().unwrap().get(host).cloned().unwrap_or_else(|| vec![IpAddr::V4(Ipv4Addr::new(140, 82, 112, 3))]))
        }
        fn get(&self, url: &Url, headers: &[(&str, String)], _t: Duration) -> Result<RawResponse> {
            crate::clone::note_net_call();
            self.seen.lock().unwrap().push((url.to_string(), headers.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()));
            let mut routes = self.routes.lock().unwrap();
            let q = routes.get_mut(url.as_str()).ok_or_else(|| Error::Network(format!("no canned answer for {url}")))?;
            let a = if q.len() > 1 { q.remove(0) } else { q[0].clone() };
            let mut headers = a.headers;
            if !headers.iter().any(|(k, _)| k == "content-length") {
                headers.push(("content-length".into(), a.body.len().to_string()));
            }
            Ok(RawResponse { status: a.status, headers, body: Box::new(std::io::Cursor::new(a.body)) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(c: &Arc<Canned>) -> Net {
        Net::new(c.clone())
    }

    #[test]
    fn url_rules() {
        let ok = |u: &str| Net::check_url(&Url::parse(u).unwrap(), Hosts::AnyPublic);
        assert_eq!(ok("https://Example.COM./a.png").unwrap(), "example.com");
        for bad in [
            "http://example.com/a",
            "ftp://example.com/a",
            "https://user:pw@example.com/a",
            "https://example.com:8443/a",
            "https://127.0.0.1/a",
            "https://2130706433/a",
            "https://0x7f.1/a",
            "https://[::1]/a",
            "https://localhost/a",
            "https://a.localhost/a",
            "https://printer.local/a",
            "https://svc.internal/a",
            "https://router.home.arpa/a",
            "https://intranet/a",
        ] {
            assert_eq!(ok(bad).unwrap_err().code(), "refused", "{bad}");
        }
        let gh = |u: &str| Net::check_url(&Url::parse(u).unwrap(), Hosts::Only(ASSET_HOSTS));
        assert!(gh("https://github.com/o/r/releases/download/v1/a.zip").is_ok());
        assert!(gh("https://objects.githubusercontent.com/x").is_ok());
        assert_eq!(gh("https://evil.example.com/x").unwrap_err().code(), "refused");
        assert_eq!(gh("https://api.github.com/x").unwrap_err().code(), "refused");
    }

    #[test]
    fn public_addresses() {
        for ip in ["140.82.112.3", "8.8.8.8", "2606:50c0:8000::153", "1.1.1.1"] {
            assert!(is_public(&ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "127.0.0.1", "10.1.2.3", "172.16.0.1", "172.31.255.255", "192.168.1.1", "169.254.169.254", "0.0.0.0", "100.64.0.1",
            "192.0.0.8", "198.18.0.1", "224.0.0.1", "255.255.255.255", "203.0.113.5", "240.0.0.1", "::1", "::", "fe80::1",
            "fd00::1", "fc00::1", "ff02::1", "::ffff:127.0.0.1", "::ffff:10.0.0.1", "64:ff9b::7f00:1", "2001:db8::1", "::127.0.0.1",
            "2002:7f00:1::",
        ] {
            assert!(!is_public(&ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn resolving_to_a_private_address_is_refused_before_any_request() {
        let c = Canned::new();
        c.dns("rebind.example.com", &["93.184.216.34", "10.0.0.5"]).on("https://rebind.example.com/a.png", 200, &[], b"x".to_vec());
        let e = net(&c).get("https://rebind.example.com/a.png", &[], Profile::IMAGE).err().unwrap();
        assert_eq!(e.code(), "refused", "{e}");
        assert!(c.seen().is_empty());
    }

    #[test]
    fn redirects_are_followed_by_hand_and_each_hop_checked() {
        let c = Canned::new();
        c.redirect("https://github.com/o/r/releases/download/v1/a.zip", "https://objects.githubusercontent.com/x?sig=1")
            .on("https://objects.githubusercontent.com/x?sig=1", 200, &[("Content-Type", "application/zip")], b"PK".to_vec());
        let r = net(&c).get("https://github.com/o/r/releases/download/v1/a.zip", &[], Profile::asset(100)).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.final_url.host_str(), Some("objects.githubusercontent.com"));
        assert_eq!(r.mime(), "application/zip");
        assert_eq!(r.bytes().unwrap(), b"PK");
        let seen = c.seen();
        assert_eq!(seen.len(), 2);
        assert!(seen[0].1.iter().any(|(k, v)| k == "User-Agent" && v.starts_with("RepoShelf/")), "{seen:?}");

        // to http, to an off-list host, to a private address, relative, and a loop
        let c = Canned::new();
        c.redirect("https://github.com/a", "http://objects.githubusercontent.com/x")
            .redirect("https://github.com/b", "https://evil.example.com/x")
            .redirect("https://github.com/c", "https://objects.githubusercontent.com/y")
            .dns("objects.githubusercontent.com", &["192.168.0.10"])
            .redirect("https://github.com/d", "/e")
            .on("https://github.com/e", 200, &[], b"ok".to_vec())
            .redirect("https://github.com/loop", "/loop");
        let n = net(&c);
        for u in ["https://github.com/a", "https://github.com/b", "https://github.com/c", "https://github.com/loop"] {
            assert_eq!(n.get(u, &[], Profile::asset(100)).err().unwrap().code(), "refused", "{u}");
        }
        assert_eq!(n.get("https://github.com/d", &[], Profile::asset(100)).unwrap().bytes().unwrap(), b"ok");
        assert_eq!(c.seen_urls().iter().filter(|u| u.ends_with("/loop")).count(), 6, "1 + 5 hops, then refused");
        assert!(!c.seen_urls().iter().any(|u| u.contains("evil") || u.starts_with("http:")));
    }

    #[test]
    fn api_profile_never_follows_redirects() {
        let c = Canned::new();
        c.on("https://api.github.com/repos/o/r", 301, &[("location", "https://api.github.com/repositories/1")], Vec::new());
        let r = net(&c).get("https://api.github.com/repos/o/r", &[], Profile::API).unwrap();
        assert_eq!(r.status, 301);
        assert_eq!(c.seen().len(), 1);
        assert_eq!(net(&c).get("https://example.com/x", &[], Profile::API).err().unwrap().code(), "refused");
    }

    #[test]
    fn size_cap_announced_and_streamed() {
        let c = Canned::new();
        c.on("https://example.com/big", 200, &[], vec![0u8; 50]).on("https://example.com/liar", 200, &[("content-length", "4")], vec![0u8; 50]);
        let n = net(&c);
        let p = Profile { max_bytes: 10, ..Profile::IMAGE };
        assert_eq!(n.get("https://example.com/big", &[], p).unwrap().bytes().unwrap_err().code(), "refused");
        assert_eq!(n.get("https://example.com/liar", &[], p).unwrap().bytes().unwrap_err().code(), "refused");
        assert_eq!(n.get("https://example.com/big", &[], Profile::IMAGE).unwrap().bytes().unwrap().len(), 50);
    }

    #[test]
    fn offline_net_refuses_and_counts() {
        let before = crate::clone::net_calls_on_this_thread();
        assert_eq!(Net::offline().get("https://example.com/a", &[], Profile::IMAGE).err().unwrap().code(), "network");
        assert_eq!(crate::clone::net_calls_on_this_thread(), before + 1);
    }
}
