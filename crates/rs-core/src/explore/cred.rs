//! The optional GitHub token (v0.3.0 Explore, D-117): one secret in Windows Credential Manager, behind a trait so
//! tests use an in-memory fake and never touch the real store.
//!
//! The token never reaches the SQLite DB, logs, job text, error messages or IPC: `Token` has a redacted `Debug`, no
//! `Display` / `Serialize`, and every error built here names the failure class only (the credential crate's
//! errors can carry the raw secret bytes, so they are never formatted).

use crate::error::{Error, Result};
use std::sync::Arc;
#[cfg(any(test, feature = "testkit", all(feature = "ghreleases", windows)))]
use std::sync::Mutex;

/// Credential Manager target: service + user.
pub const SERVICE: &str = "dev.schwi.reposhelf";
pub const USER: &str = "github-token";
/// Longest token accepted (GitHub's fine-grained tokens are ~93 characters).
pub const MAX_TOKEN_LEN: usize = 255;

/// A GitHub access token. No `Display`, no `Serialize`; `Debug` prints `Token(***)`.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(***)")
    }
}

impl Token {
    /// Trims and checks: 1–255 characters of `[A-Za-z0-9_-]` (what GitHub's `ghp_…` / `github_pat_…` / `gho_…`
    /// tokens and the old 40-hex ones use). The refusal never repeats the input.
    pub fn parse(raw: &str) -> Result<Token> {
        let t = raw.trim();
        let ok = !t.is_empty() && t.len() <= MAX_TOKEN_LEN && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if !ok {
            return Err(Error::InvalidInput(
                "A GitHub token is 1–255 letters, digits, '_' or '-' with no spaces. Copy it again from github.com/settings/tokens.".into(),
            ));
        }
        Ok(Token(t.to_string()))
    }
    /// For the `Authorization` header and the credential store only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// Where the token lives. Calls are serialised by the implementation.
pub trait CredentialStore: Send + Sync {
    fn get(&self) -> Result<Option<Token>>;
    fn set(&self, token: &Token) -> Result<()>;
    /// Removing an absent token is fine.
    fn delete(&self) -> Result<()>;
}

/// In-memory fake (tests, testkit and e2e builds — never the real Credential Manager).
#[cfg(any(test, feature = "testkit"))]
#[derive(Default)]
pub struct MemoryStore {
    slot: Mutex<Option<Token>>,
    /// Calls made, for tests: `get` / `set` / `delete`.
    pub calls: Mutex<Vec<&'static str>>,
}

#[cfg(any(test, feature = "testkit"))]
impl MemoryStore {
    pub fn with_token(raw: &str) -> Arc<MemoryStore> {
        let s = MemoryStore::default();
        *s.slot.lock().unwrap() = Some(Token::parse(raw).unwrap());
        Arc::new(s)
    }
    pub fn peek(&self) -> Option<Token> {
        self.slot.lock().unwrap().clone()
    }
}

#[cfg(any(test, feature = "testkit"))]
impl CredentialStore for MemoryStore {
    fn get(&self) -> Result<Option<Token>> {
        self.calls.lock().unwrap().push("get");
        Ok(self.slot.lock().unwrap().clone())
    }
    fn set(&self, token: &Token) -> Result<()> {
        self.calls.lock().unwrap().push("set");
        *self.slot.lock().unwrap() = Some(token.clone());
        Ok(())
    }
    fn delete(&self) -> Result<()> {
        self.calls.lock().unwrap().push("delete");
        *self.slot.lock().unwrap() = None;
        Ok(())
    }
}

/// Builds without a credential store (no `ghreleases`, or not Windows): browsing works without a token.
pub struct UnavailableStore;

fn unavailable() -> Error {
    Error::Unsupported("Saving a GitHub token is not available in this build.".into())
}

impl CredentialStore for UnavailableStore {
    fn get(&self) -> Result<Option<Token>> {
        Ok(None)
    }
    fn set(&self, _token: &Token) -> Result<()> {
        Err(unavailable())
    }
    fn delete(&self) -> Result<()> {
        Ok(())
    }
}

/// Windows Credential Manager through `windows-native-keyring-store` (the store object is used directly, no global
/// default store). Compile-checked by the gate; only exercised by hand (an `#[ignore]`d test below).
#[cfg(all(feature = "ghreleases", windows))]
pub struct WindowsStore {
    store: Mutex<Arc<windows_native_keyring_store::Store>>,
    service: String,
    user: String,
}

#[cfg(all(feature = "ghreleases", windows))]
impl WindowsStore {
    pub fn new() -> Result<WindowsStore> {
        WindowsStore::with_target(SERVICE, USER)
    }

    /// Another Credential Manager target (the hand test uses a throw-away one, never the app's own entry).
    pub fn with_target(service: &str, user: &str) -> Result<WindowsStore> {
        let store = windows_native_keyring_store::Store::new().map_err(|e| fail(&e))?;
        Ok(WindowsStore { store: Mutex::new(store), service: service.to_string(), user: user.to_string() })
    }

    /// The crate warns that one entry used from several threads is not reliably ordered: the lock is held for
    /// the whole operation (`op` receives the entry), so get / set / delete run one at a time.
    fn with_entry<T>(&self, op: impl FnOnce(&keyring_core::Entry) -> keyring_core::Result<T>) -> Result<T> {
        use keyring_core::api::CredentialStoreApi;
        let guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard.build(&self.service, &self.user, None).map_err(|e| fail(&e))?;
        op(&entry).map_err(|e| fail(&e))
    }
}

/// Failure class only: `BadEncoding` / `BadDataFormat` carry the raw secret bytes, so no error is ever formatted.
#[cfg(all(feature = "ghreleases", windows))]
fn fail(e: &keyring_core::Error) -> Error {
    use keyring_core::Error as K;
    let class = match e {
        K::PlatformFailure(_) => "platform failure",
        K::NoStorageAccess(_) => "storage not accessible",
        K::NoEntry => "no entry",
        K::BadEncoding(_) | K::BadDataFormat(..) | K::BadStoreFormat(_) => "unreadable entry",
        K::TooLong(..) | K::Invalid(..) => "invalid value",
        _ => "error",
    };
    Error::Refused(format!("Windows Credential Manager failed ({class})."))
}

#[cfg(all(feature = "ghreleases", windows))]
impl CredentialStore for WindowsStore {
    fn get(&self) -> Result<Option<Token>> {
        match self.with_entry(|e| e.get_password()) {
            Ok(p) => Ok(Token::parse(&p).ok()),
            Err(e) if is_no_entry(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn set(&self, token: &Token) -> Result<()> {
        self.with_entry(|e| e.set_password(token.expose()))
    }
    fn delete(&self) -> Result<()> {
        match self.with_entry(|e| e.delete_credential()) {
            Ok(()) => Ok(()),
            Err(e) if is_no_entry(&e) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// `fail` maps `NoEntry` to this exact class text.
#[cfg(all(feature = "ghreleases", windows))]
fn is_no_entry(e: &Error) -> bool {
    matches!(e, Error::Refused(m) if m.ends_with("(no entry)."))
}

/// The store `Shelf::open` uses: Credential Manager in a shipped build, the in-memory fake in test / testkit / e2e
/// builds (so no test can write to the real store), nothing where there is no HTTP client.
#[cfg(all(feature = "ghreleases", windows, not(any(test, feature = "testkit"))))]
pub fn default_store() -> Arc<dyn CredentialStore> {
    match WindowsStore::new() {
        Ok(s) => Arc::new(s),
        Err(_) => Arc::new(UnavailableStore),
    }
}

#[cfg(any(test, feature = "testkit"))]
pub fn default_store() -> Arc<dyn CredentialStore> {
    Arc::new(MemoryStore::default())
}

#[cfg(not(any(test, feature = "testkit", all(feature = "ghreleases", windows))))]
pub fn default_store() -> Arc<dyn CredentialStore> {
    Arc::new(UnavailableStore)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_rules_and_redaction() {
        let t = Token::parse("  ghp_AbC123-xyz_9  ").unwrap();
        assert_eq!(t.expose(), "ghp_AbC123-xyz_9");
        assert_eq!(format!("{t:?}"), "Token(***)");
        assert!(!format!("{t:#?}").contains("ghp_"));
        for bad in ["", "   ", "has space", "a\nb", "a\0b", "tab\there", "ünï", "a/b", "a=b", "a;b", &"x".repeat(256)] {
            let e = Token::parse(bad).unwrap_err();
            assert_eq!(e.code(), "invalidInput");
            assert!(bad.trim().is_empty() || !e.to_string().contains(bad.trim()), "the refusal repeats the input");
        }
        assert_eq!(Token::parse(&"x".repeat(255)).unwrap().expose().len(), 255);
    }

    #[test]
    fn memory_store_round_trip_and_default_is_the_fake() {
        let s = default_store();
        assert!(s.get().unwrap().is_none());
        s.set(&Token::parse("ghp_one").unwrap()).unwrap();
        assert_eq!(s.get().unwrap().unwrap().expose(), "ghp_one");
        s.delete().unwrap();
        s.delete().unwrap();
        assert!(s.get().unwrap().is_none());
        assert_eq!(UnavailableStore.set(&Token::parse("x").unwrap()).unwrap_err().code(), "unsupported");
        assert!(UnavailableStore.get().unwrap().is_none());
    }

    /// Hand check only (`cargo test -p rs-core --features ghreleases -- --ignored real_credential_manager`): writes
    /// and deletes one throw-away Credential Manager entry with its own target, never the app's. Not run by the gate.
    #[cfg(all(feature = "ghreleases", windows))]
    #[test]
    #[ignore = "writes to the real Windows Credential Manager"]
    fn real_credential_manager_round_trip() {
        let s = WindowsStore::with_target("dev.schwi.reposhelf.handtest", "throwaway").unwrap();
        s.set(&Token::parse("ghp_throwaway_test_value").unwrap()).unwrap();
        assert_eq!(s.get().unwrap().unwrap().expose(), "ghp_throwaway_test_value");
        s.delete().unwrap();
        assert!(s.get().unwrap().is_none());
    }
}
