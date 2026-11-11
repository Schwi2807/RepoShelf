//! `rs bench <dir>`: times the read functions on the fixtures built by `rs fixtures <dir>` (M2.13, PoC criterion 3;
//! NFR-P2, P3, P5, P6, FR-28). Prints a table and a JSON line; the caller exits 1 on any miss.

use rs_core::jobs::CancelToken;
use rs_core::types::Source;
use rs_core::validate::{GitPath, rev};
use rs_core::{Shelf, commits, diff, langs, lastcommit, refs, repo, testkit, tree};
use std::path::Path;
use std::time::{Duration, Instant};

pub struct Row {
    pub name: &'static str,
    pub ms: f64,
    pub target_ms: f64,
}

impl Row {
    pub fn ok(&self) -> bool {
        self.ms < self.target_ms
    }
}

fn time<T>(f: impl FnOnce() -> Result<T, String>) -> Result<(T, Duration), String> {
    let t = Instant::now();
    let v = f()?;
    Ok((v, t.elapsed()))
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn e(x: rs_core::Error) -> String {
    x.to_string()
}

fn register(s: &Shelf, p: &Path) -> Result<i64, String> {
    if !p.exists() {
        return Err(format!("fixture missing: {} (run `rs fixtures` first)", p.display()));
    }
    testkit::register_raw(s, p, Source::Imported).map_err(e)
}

pub fn run(dir: &Path) -> Result<Vec<Row>, String> {
    let scratch = tempfile::tempdir().map_err(|x| x.to_string())?;
    let s = testkit::shelf(scratch.path());
    let cancel = CancelToken::new();
    let main = rev("main").map_err(e)?;
    let mut rows = Vec::new();

    // NFR-P2: 1,000-entry directory listing incl. sizes (open + resolve + list)
    let wide = register(&s, &dir.join("wide-dir"))?;
    let (_, d) = time(|| {
        let (_, r) = repo::open(&s, wide).map_err(e)?;
        let res = refs::resolve(&r, &main).map_err(e)?;
        let l = tree::list_dir(&r, &res, &GitPath::root()).map_err(e)?;
        if l.entries.len() != 1000 {
            return Err(format!("wide-dir has {} entries", l.entries.len()));
        }
        Ok(())
    })?;
    rows.push(Row { name: "tree.list_dir 1000 entries", ms: ms(d), target_ms: 300.0 });

    // NFR-P3: last commit per entry, second visit served from the cache
    let (r, tip) = {
        let (_, r) = repo::open(&s, wide).map_err(e)?;
        let tip = rs_core::validate::oid(&refs::resolve(&r, &main).map_err(e)?.commit).map_err(e)?;
        (r, tip)
    };
    let (_, first) = time(|| lastcommit::for_dir(&s, wide, &r, tip, &GitPath::root(), lastcommit::CAP, &cancel).map_err(e))?;
    let (_, second) = time(|| lastcommit::for_dir(&s, wide, &r, tip, &GitPath::root(), lastcommit::CAP, &cancel).map_err(e))?;
    println!("  (lastcommit first visit on 1,000 entries: {:.1} ms)", ms(first));
    rows.push(Row { name: "lastcommit.for_dir 2nd visit", ms: ms(second), target_ms: 50.0 });

    // NFR-P5: commits page at skip 0 and at the end of a 10k history
    let long = register(&s, &dir.join("long-history"))?;
    for (name, skip) in [("commits.page skip 0", 0u32), ("commits.page skip 9,965", 9_965u32)] {
        let (_, d) = time(|| {
            let (_, r) = repo::open(&s, long).map_err(e)?;
            let tip = rs_core::validate::oid(&refs::resolve(&r, &main).map_err(e)?.commit).map_err(e)?;
            let mut q = commits::PageQuery::new(tip);
            q.skip = skip;
            let p = commits::page(&r, &q, &cancel).map_err(e)?;
            if p.commits.len() != 35 {
                return Err(format!("page at {skip} has {} commits", p.commits.len()));
            }
            Ok(())
        })?;
        rows.push(Row { name, ms: ms(d), target_ms: 200.0 });
    }

    // NFR-P6: language stats on a 50k-file tree, first (uncached) compute
    let big = register(&s, &dir.join("big-tree"))?;
    let (_, d) = time(|| {
        let (_, r) = repo::open(&s, big).map_err(e)?;
        let res = refs::resolve(&r, &main).map_err(e)?;
        let tree = rs_core::validate::oid(&res.tree).map_err(e)?;
        let l = langs::for_tree(&s, big, &r, tree, &cancel).map_err(e)?;
        if l.items.is_empty() {
            return Err("big-tree has no languages".into());
        }
        Ok(())
    })?;
    rows.push(Row { name: "langs.for_tree 50k files", ms: ms(d), target_ms: 3000.0 });

    // FR-28: 400-file commit opens < 1 s (collapsed)
    let huge = register(&s, &dir.join("huge-commit"))?;
    let (_, d) = time(|| {
        let (_, r) = repo::open(&s, huge).map_err(e)?;
        let oid = rs_core::validate::oid(&refs::resolve(&r, &main).map_err(e)?.commit).map_err(e)?;
        let c = diff::commit_detail(&r, oid, &diff::DiffLimits::default()).map_err(e)?;
        if c.files.len() != 400 || !c.diffs_deferred {
            return Err(format!("huge-commit: {} files, deferred {}", c.files.len(), c.diffs_deferred));
        }
        Ok(())
    })?;
    rows.push(Row { name: "diff.commit_detail 400 files", ms: ms(d), target_ms: 1000.0 });
    Ok(rows)
}

pub fn print(rows: &[Row]) {
    println!("{:<32} {:>10} {:>10}  result", "measurement", "ms", "target");
    for r in rows {
        println!("{:<32} {:>10.1} {:>10.0}  {}", r.name, r.ms, r.target_ms, if r.ok() { "ok" } else { "MISS" });
    }
    let json: Vec<String> = rows
        .iter()
        .map(|r| format!("{{\"name\":{},\"ms\":{:.1},\"targetMs\":{},\"ok\":{}}}", serde_json::to_string(r.name).unwrap_or_default(), r.ms, r.target_ms, r.ok()))
        .collect();
    println!("{{\"bench\":[{}]}}", json.join(","));
}
