//! Language statistics per tree (M2.9; FR-19, NFR-P6, ARCH §6.2). Bytes per language from object headers (no
//! content read), vendored/generated/docs/data excluded, top 6 + "Other", cached by root tree oid.

use crate::Shelf;
use crate::error::{Error, Result};
use crate::jobs::CancelToken;
use crate::types::{LanguageStat, Languages};
use git2::{ObjectType, Oid, Repository, TreeWalkMode, TreeWalkResult};
use std::collections::HashMap;

pub const TOP: usize = 6;
pub const OTHER: &str = "Other";

/// Directory names whose whole subtree is skipped.
const EXCLUDED_DIRS: &[&str] = &["vendor", "third_party", "node_modules", "dist", "build", "target", "docs", "Documentation"];

fn excluded_dir(name: &str) -> bool {
    EXCLUDED_DIRS.contains(&name) || name.starts_with(".git")
}

fn excluded_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    name.starts_with(".git")
        || lower.ends_with(".min.js")
        || lower.ends_with(".min.css")
        || lower.ends_with(".lock")
        || lower == "package-lock.json"
        || lower.ends_with(".pb.go")
        || lower.contains("_generated.")
}

/// Counted language of a git path, or `None` (excluded, data/markup, or unknown).
pub fn classify(path: &str) -> Option<&'static str> {
    let mut parts: Vec<&str> = path.split('/').collect();
    let name = parts.pop()?;
    if parts.iter().any(|d| excluded_dir(d)) || excluded_file(name) {
        return None;
    }
    crate::langs_table::lookup(path).filter(|l| l.2).map(|l| l.0)
}

fn cached(s: &Shelf, repo_id: i64, tree: Oid) -> Result<Option<Languages>> {
    s.db.with(|c| {
        let mut st = c.prepare_cached("SELECT json FROM cache_languages WHERE repo_id = ?1 AND tree = ?2")?;
        let mut rows = st.query(rusqlite::params![repo_id, tree.to_string()])?;
        Ok(match rows.next()? {
            Some(r) => serde_json::from_str(&r.get::<_, String>(0)?).ok(),
            None => None,
        })
    })
}

/// Pure computation (no cache); cancellable every 1,024 entries.
pub fn compute(repo: &Repository, tree: Oid, cancel: &CancelToken) -> Result<Languages> {
    let root = repo.find_tree(tree)?;
    let odb = repo.odb()?;
    let mut bytes: HashMap<&'static str, u64> = HashMap::new();
    let mut seen = 0u32;
    let mut cancelled = false;
    let mut failure: Option<git2::Error> = None;
    root.walk(TreeWalkMode::PreOrder, |dir, e| {
        seen += 1;
        if seen.is_multiple_of(1024) && cancel.is_cancelled() {
            cancelled = true;
            return TreeWalkResult::Abort;
        }
        let name = String::from_utf8_lossy(e.name_bytes());
        match e.kind() {
            Some(ObjectType::Tree) => {
                if excluded_dir(&name) { TreeWalkResult::Skip } else { TreeWalkResult::Ok }
            }
            Some(ObjectType::Blob) if e.filemode() != 0o120000 => {
                let path = format!("{dir}{name}");
                if let Some(lang) = classify(&path) {
                    match odb.read_header(e.id()) {
                        Ok((n, _)) => *bytes.entry(lang).or_default() += n as u64,
                        Err(err) => {
                            failure = Some(err);
                            return TreeWalkResult::Abort;
                        }
                    }
                }
                TreeWalkResult::Ok
            }
            _ => TreeWalkResult::Ok,
        }
    })
    .or_else(|e| if cancelled || failure.is_some() { Ok(()) } else { Err(e) })?;
    if cancelled {
        return Err(Error::Cancelled);
    }
    if let Some(e) = failure {
        return Err(e.into());
    }
    let total: u64 = bytes.values().sum();
    let mut v: Vec<(&str, u64)> = bytes.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let pct = |n: u64| if total == 0 { 0.0 } else { (n as f64 * 100.0 / total as f64) as f32 };
    let mut items: Vec<LanguageStat> = v.iter().take(TOP).map(|(name, n)| LanguageStat { name: name.to_string(), bytes: *n, percent: pct(*n) }).collect();
    let rest: u64 = v.iter().skip(TOP).map(|(_, n)| n).sum();
    if rest > 0 {
        items.push(LanguageStat { name: OTHER.into(), bytes: rest, percent: pct(rest) });
    }
    Ok(Languages { tree: tree.to_string(), items })
}

/// Language stats of `tree`, cached by `(repo_id, tree)`. The walk runs without holding the DB lock.
pub fn for_tree(s: &Shelf, repo_id: i64, repo: &Repository, tree: Oid, cancel: &CancelToken) -> Result<Languages> {
    if let Some(l) = cached(s, repo_id, tree)? {
        return Ok(l);
    }
    let langs = compute(repo, tree, cancel)?;
    let json = serde_json::to_string(&langs).map_err(|e| Error::InvalidInput(e.to_string()))?;
    s.db.with(|c| {
        c.execute("INSERT OR REPLACE INTO cache_languages (repo_id, tree, json) VALUES (?1, ?2, ?3)", rusqlite::params![repo_id, tree.to_string(), json])?;
        Ok(())
    })?;
    Ok(langs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{self, T0};
    use crate::types::Source;

    #[test]
    fn classify_rules() {
        assert_eq!(classify("src/main.rs"), Some("Rust"));
        assert_eq!(classify("web/index.html"), Some("HTML"));
        assert_eq!(classify("vendor/lib.js"), None);
        assert_eq!(classify("a/node_modules/x/index.js"), None);
        assert_eq!(classify("docs/conf.py"), None);
        assert_eq!(classify("app.min.js"), None);
        assert_eq!(classify("Cargo.lock"), None);
        assert_eq!(classify("package-lock.json"), None);
        assert_eq!(classify("api.pb.go"), None);
        assert_eq!(classify("schema_generated.rs"), None);
        assert_eq!(classify(".github/workflows/x.py"), None);
        assert_eq!(classify("README.md"), None, "prose is not counted");
        assert_eq!(classify("data.yaml"), None);
        assert_eq!(classify("Makefile"), Some("Makefile"));
        assert_eq!(classify("LICENSE"), None);
    }

    #[test]
    fn basic_percentages_and_cache() {
        let t = tempfile::tempdir().unwrap();
        let fx = testkit::basic(&t.path().join("b"));
        let s = testkit::shelf(t.path());
        let id = testkit::register_raw(&s, &fx.path, Source::Imported).unwrap();
        let repo = testkit::open(&fx.path);
        let tree = repo.find_commit(fx.head).unwrap().tree_id();
        let l = for_tree(&s, id, &repo, tree, &CancelToken::new()).unwrap();
        let got: Vec<(&str, u64, f32)> = l.items.iter().map(|i| (i.name.as_str(), i.bytes, i.percent)).collect();
        assert_eq!(got, [("Rust", 600, 60.0), ("TypeScript", 400, 40.0)], "vendor/ and docs/ excluded");
        // cache hit: served even when cancelled
        let c = CancelToken::new();
        c.cancel();
        assert_eq!(for_tree(&s, id, &repo, tree, &c).unwrap(), l);
        let rows: i64 = s.db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM cache_languages", [], |r| r.get(0))?)).unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn top_six_plus_other_and_cancel() {
        let t = tempfile::tempdir().unwrap();
        let mut b = testkit::repo(&t.path().join("r"));
        b.bare();
        for (i, ext) in ["rs", "ts", "py", "go", "c", "java", "rb", "php"].iter().enumerate() {
            b.file(&format!("f.{ext}"), &vec![b'x'; 1000 - i * 100]);
        }
        b.commit("c", T0);
        let repo = testkit::open(&t.path().join("r"));
        let tree = repo.find_commit(b.head()).unwrap().tree_id();
        let l = compute(&repo, tree, &CancelToken::new()).unwrap();
        assert_eq!(l.items.len(), 7);
        assert_eq!(l.items[0].name, "Rust");
        assert_eq!(l.items[6].name, OTHER);
        assert_eq!(l.items[6].bytes, 400 + 300);
        let sum: f32 = l.items.iter().map(|i| i.percent).sum();
        assert!((sum - 100.0).abs() < 0.01);

        let mut big = testkit::repo(&t.path().join("big"));
        big.bare();
        for i in 0..2100 {
            big.file(&format!("d/f{i}.rs"), b"x");
        }
        big.commit("c", T0);
        let repo = testkit::open(&t.path().join("big"));
        let tree = repo.find_commit(big.head()).unwrap().tree_id();
        let c = CancelToken::new();
        c.cancel();
        assert_eq!(compute(&repo, tree, &c).unwrap_err().code(), "cancelled");
    }
}
