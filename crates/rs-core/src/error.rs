//! Error type shared by every rs-core module (ARCH §3.1).

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{what} not found")]
    NotFound { what: String },
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("invalid ref: {0}")]
    InvalidRev(String),
    #[error("{0}")]
    InvalidUrl(String),
    #[error("{0}")]
    InvalidInput(String),
    #[error("already on the shelf: {0}")]
    Duplicate(String),
    #[error("not a git repository: {0}")]
    NotARepo(String),
    #[error("repository folder is missing: {0}")]
    RepoMissing(String),
    #[error("repository is damaged: {0}")]
    RepoCorrupt(String),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Refused(String),
    #[error("network: {0}")]
    Network(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("git: {0}")]
    Git(#[from] git2::Error),
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Stable camelCase code sent over IPC (`IpcError.code`).
    pub fn code(&self) -> &'static str {
        match self {
            Error::NotFound { .. } => "notFound",
            Error::InvalidPath(_) => "invalidPath",
            Error::InvalidRev(_) => "invalidRev",
            Error::InvalidUrl(_) => "invalidUrl",
            Error::InvalidInput(_) => "invalidInput",
            Error::Duplicate(_) => "duplicate",
            Error::NotARepo(_) => "notARepo",
            Error::RepoMissing(_) => "repoMissing",
            Error::RepoCorrupt(_) => "repoCorrupt",
            Error::Cancelled => "cancelled",
            Error::Refused(_) => "refused",
            Error::Network(_) => "network",
            Error::Unsupported(_) => "unsupported",
            Error::Git(_) => "git",
            Error::Db(_) => "db",
            Error::Io(_) => "io",
        }
    }

    pub fn not_found(what: impl Into<String>) -> Self {
        Error::NotFound { what: what.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn every_variant() -> Vec<Error> {
        vec![
            Error::not_found("ref"),
            Error::InvalidPath("x".into()),
            Error::InvalidRev("x".into()),
            Error::InvalidUrl("x".into()),
            Error::InvalidInput("x".into()),
            Error::Duplicate("x".into()),
            Error::NotARepo("x".into()),
            Error::RepoMissing("x".into()),
            Error::RepoCorrupt("x".into()),
            Error::Cancelled,
            Error::Refused("x".into()),
            Error::Network("x".into()),
            Error::Unsupported("x".into()),
            Error::Git(git2::Error::from_str("x")),
            Error::Db(rusqlite::Error::InvalidQuery),
            Error::Io(std::io::Error::other("x")),
        ]
    }

    #[test]
    fn codes_are_unique_camel_case() {
        let all = every_variant();
        let codes: HashSet<&str> = all.iter().map(Error::code).collect();
        assert_eq!(codes.len(), all.len(), "duplicate code");
        for c in codes {
            let mut chars = c.chars();
            assert!(chars.next().unwrap().is_ascii_lowercase(), "{c}");
            assert!(chars.all(|ch| ch.is_ascii_alphanumeric()), "{c}");
        }
    }

    #[test]
    fn messages_are_readable() {
        assert_eq!(Error::not_found("ref").to_string(), "ref not found");
        assert_eq!(Error::Cancelled.to_string(), "cancelled");
        assert_eq!(Error::Refused("Branch has diverged".into()).to_string(), "Branch has diverged");
    }
}
