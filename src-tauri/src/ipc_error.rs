//! `IpcError { code, message }` (ARCH §3.1, M4.2): what every command returns on failure, plus the friendly
//! wording for network/git failures. Job errors never pass through `IpcError` (the runner stores a string), so
//! `friendly` is also applied by the job worker wrapper in `state.rs`.

use rs_core::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    /// `rs_core::Error::code()` (`"notFound"`, `"invalidPath"`, …).
    pub code: String,
    pub message: String,
}

pub type CmdResult<T> = Result<T, IpcError>;

pub const CREDENTIALS: &str =
    "The server asked for credentials: private or missing repository. Only public repositories can be cloned.";

impl IpcError {
    pub fn new(code: &str, message: impl Into<String>) -> IpcError {
        IpcError { code: code.to_string(), message: message.into() }
    }
    /// Maps an rs-core error; `host` (when known) names the server in network messages.
    pub fn from_core(e: &Error, host: Option<&str>) -> IpcError {
        IpcError { code: e.code().to_string(), message: friendly(e, host) }
    }
}

impl From<Error> for IpcError {
    fn from(e: Error) -> IpcError {
        IpcError::from_core(&e, None)
    }
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

fn server(host: Option<&str>) -> String {
    match host {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => "the server".to_string(),
    }
}

/// HTTP status in libgit2's WinHTTP/HTTP message `request failed with status code: <n>`.
fn http_status(msg: &str) -> Option<u32> {
    let rest = msg.split("status code:").nth(1)?;
    rest.trim().split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

/// User-readable text. Git errors: credentials (auth, 401, 403, an auth-replay loop), 404, and
/// "Could not reach <host>" for net/http/ssl/os failures (strings from vendored libgit2 `winhttp.c`). The D-44
/// `url.*.insteadOf` refusal gets plain wording. Everything else keeps rs-core's message.
pub fn friendly(e: &Error, host: Option<&str>) -> String {
    match e {
        Error::Git(g) => friendly_git(g, host),
        Error::Refused(m) if m.contains("url.*.insteadOf") => format!(
            "Your git settings redirect this address to a different one (a url.*.insteadOf rule in your git config). \
             RepoShelf only connects to the exact address you entered, so it stopped. Remove that rule to clone or \
             fetch {} here.",
            server(host)
        ),
        Error::Network(m) => format!("Could not reach {}: {m}", server(host)),
        other => other.to_string(),
    }
}

fn friendly_git(g: &git2::Error, host: Option<&str>) -> String {
    use git2::{ErrorClass as C, ErrorCode};
    let msg = g.message();
    let lower = msg.to_ascii_lowercase();
    if g.code() == ErrorCode::Auth
        || matches!(http_status(msg), Some(401 | 403 | 407))
        || lower.contains("authentication replays")
        || lower.contains("authentication required")
    {
        return CREDENTIALS.to_string();
    }
    if http_status(msg) == Some(404) {
        return format!("Repository not found on {} (404). Check the address.", server(host));
    }
    if let Some(status) = http_status(msg) {
        return format!("{} answered with HTTP {status}. Try again later.", server(host));
    }
    match g.class() {
        C::Net | C::Http | C::Ssl | C::Os | C::Callback => format!("Could not reach {}: {msg}", server(host)),
        _ => format!("git: {msg}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{ErrorClass, ErrorCode};

    fn git(code: ErrorCode, class: ErrorClass, msg: &str) -> Error {
        Error::Git(git2::Error::new(code, class, msg))
    }

    #[test]
    fn codes_pass_through() {
        let e = IpcError::from(Error::InvalidPath("..".into()));
        assert_eq!(e.code, "invalidPath");
        assert_eq!(e.message, "invalid path: ..");
        assert_eq!(IpcError::from(Error::not_found("repo")).code, "notFound");
        let v = serde_json::to_value(IpcError::new("x", "y")).unwrap();
        assert_eq!(v, serde_json::json!({"code": "x", "message": "y"}));
    }

    #[test]
    fn credentials_messages() {
        for e in [
            git(ErrorCode::Auth, ErrorClass::Http, "invalid authentication scheme"),
            git(ErrorCode::GenericError, ErrorClass::Http, "request failed with status code: 401"),
            git(ErrorCode::GenericError, ErrorClass::Http, "request failed with status code: 403"),
            git(ErrorCode::GenericError, ErrorClass::Http, "too many redirects or authentication replays"),
        ] {
            let i = IpcError::from_core(&e, Some("github.com"));
            assert_eq!(i.code, "git");
            assert_eq!(i.message, CREDENTIALS, "{e}");
        }
    }

    #[test]
    fn not_found_and_other_status() {
        let e = git(ErrorCode::GenericError, ErrorClass::Http, "request failed with status code: 404");
        assert_eq!(friendly(&e, Some("codeberg.org")), "Repository not found on codeberg.org (404). Check the address.");
        let e = git(ErrorCode::GenericError, ErrorClass::Http, "request failed with status code: 502");
        assert!(friendly(&e, None).starts_with("the server answered with HTTP 502"));
    }

    #[test]
    fn unreachable_host() {
        for (class, msg) in [
            (ErrorClass::Os, "failed to send request: The server name or address could not be resolved"),
            (ErrorClass::Os, "failed to connect to host"),
            (ErrorClass::Net, "unexpected disconnect"),
            (ErrorClass::Ssl, "the SSL certificate is invalid"),
            (ErrorClass::Http, "unknown security error 12175"),
        ] {
            let m = friendly(&git(ErrorCode::GenericError, class, msg), Some("example.org"));
            assert!(m.starts_with("Could not reach example.org: "), "{m}");
            assert!(m.ends_with(msg), "{m}");
        }
        let m = friendly(&Error::Network("GitHub release notes: timeout".into()), Some("api.github.com"));
        assert_eq!(m, "Could not reach api.github.com: GitHub release notes: timeout");
    }

    #[test]
    fn other_git_errors_keep_their_text() {
        let m = friendly(&git(ErrorCode::NotFound, ErrorClass::Reference, "reference 'x' not found"), None);
        assert_eq!(m, "git: reference 'x' not found");
    }

    #[test]
    fn insteadof_refusal_is_plain() {
        let e = Error::Refused(
            "Git configuration rewrites https://a/b.git to http://a/b.git (url.*.insteadOf). RepoShelf only connects to the address itself."
                .into(),
        );
        let i = IpcError::from_core(&e, Some("a"));
        assert_eq!(i.code, "refused");
        assert!(i.message.starts_with("Your git settings redirect"), "{}", i.message);
        assert!(i.message.contains("insteadOf"));
        // other refusals unchanged
        assert_eq!(friendly(&Error::Refused("Branch has diverged from origin".into()), None), "Branch has diverged from origin");
    }
}
