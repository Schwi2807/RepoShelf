//! `rs inspect <repo-path> <cmd> [args]`: one read function's result as pretty JSON (debugging aid, M2.13).
//! The repo is registered in an in-memory shelf; nothing is written to it.

use rs_core::jobs::CancelToken;
use rs_core::types::Source;
use rs_core::validate::{GitPath, git_path, oid, rev};
use rs_core::{blob, commits, diff, lastcommit, langs, readme, refs, releases, repo, testkit, tree};
use serde::Serialize;
use std::path::Path;

fn json<T: Serialize>(v: &T) -> Result<String, String> {
    serde_json::to_string_pretty(v).map_err(|e| e.to_string())
}

fn e(x: rs_core::Error) -> String {
    format!("{} ({})", x, x.code())
}

pub fn run(path: &Path, cmd: &str, args: &[String]) -> Result<String, String> {
    let scratch = tempfile::tempdir().map_err(|x| x.to_string())?;
    let s = testkit::shelf(scratch.path());
    let id = testkit::register_raw(&s, path, Source::Imported).map_err(e)?;
    let arg = |i: usize| args.get(i).map(String::as_str);
    let need = |i: usize, what: &str| arg(i).ok_or_else(|| format!("{cmd} needs <{what}>"));
    let gp = |i: usize| match arg(i) {
        Some(p) => git_path(p).map_err(e),
        None => Ok(GitPath::root()),
    };
    let c = CancelToken::new();
    if cmd == "detail" {
        return json(&repo::detail(&s, id).map_err(e)?);
    }
    let (_, r) = repo::open(&s, id).map_err(e)?;
    let resolve = |i: usize| -> Result<_, String> { refs::resolve(&r, &rev(need(i, "rev")?).map_err(e)?).map_err(e) };
    match cmd {
        "refs" => json(&refs::list(&r).map_err(e)?),
        "branches" => json(&refs::branches(&r).map_err(e)?),
        "tags" => json(&refs::tags(&r).map_err(e)?),
        "releases" => json(&releases::list(&r).map_err(e)?),
        "resolve" => json(&resolve(0)?),
        "tree" => json(&tree::list_dir(&r, &resolve(0)?, &gp(1)?).map_err(e)?),
        "blob" => json(&blob::text(&r, &resolve(0)?, &git_path(need(1, "path")?).map_err(e)?).map_err(e)?),
        "log" => {
            let res = resolve(0)?;
            let mut q = commits::PageQuery::new(oid(&res.commit).map_err(e)?);
            q.skip = arg(1).map(|s| s.parse().map_err(|_| "skip must be a number".to_string())).transpose()?.unwrap_or(0);
            q.path = arg(2).map(|p| git_path(p).map_err(e)).transpose()?;
            json(&commits::page(&r, &q, &c).map_err(e)?)
        }
        "count" => {
            let res = resolve(0)?;
            json(&commits::count(&s, id, &r, oid(&res.commit).map_err(e)?, &c).map_err(e)?)
        }
        "commit" => json(&diff::commit_detail(&r, oid(need(0, "oid")?).map_err(e)?, &diff::DiffLimits::default()).map_err(e)?),
        "last" => {
            let res = resolve(0)?;
            json(&lastcommit::for_dir(&s, id, &r, oid(&res.commit).map_err(e)?, &gp(1)?, lastcommit::CAP, &c).map_err(e)?)
        }
        "langs" => {
            let res = resolve(0)?;
            json(&langs::for_tree(&s, id, &r, oid(&res.tree).map_err(e)?, &c).map_err(e)?)
        }
        "readme" => json(&readme::render_readme(&r, &resolve(0)?, &gp(1)?).map_err(e)?),
        _ => Err(format!("unknown inspect command '{cmd}'")),
    }
}
