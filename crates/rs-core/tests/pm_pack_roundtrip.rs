//! v0.4.0 cross-app check (D-137), driven by `C:\Dev\projects\project-manager\tools\shelf-roundtrip.sh`.
//! Ignored unless that script sets `RS_PM_PACK` (a pack made by Project Manager's real code) and
//! `RS_PM_UPSTREAM` (a local folder standing in for GitHub, used through the canned-network seam).

use rs_core::jobs::JobCtx;
use rs_core::types::Source;
use rs_core::{fetch, library, repo, testkit};
use std::path::Path;

fn env(k: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| panic!("{k} not set (run tools/shelf-roundtrip.sh in project-manager)"))
}

#[test]
#[ignore = "driven by project-manager/tools/shelf-roundtrip.sh"]
fn import_fetch_and_fast_forward_a_project_manager_pack() {
    let pack = env("RS_PM_PACK");
    let upstream = env("RS_PM_UPSTREAM");
    let expect_tip = env("RS_PM_EXPECT_TIP");
    let t = tempfile::tempdir().unwrap();
    let s = testkit::shelf(t.path());

    let sum = library::import(&s, Path::new(&pack)).unwrap();
    assert_eq!(sum.source, Source::Imported);
    let d = repo::detail(&s, sum.id).unwrap();
    assert!(d.is_bare, "pack is bare");
    assert!(d.fetchable, "https origin → Fetch offered");
    assert_eq!(d.can_fast_forward, Some(false), "marked pack, nothing new yet");
    println!("imported {}/{} branch {:?}", sum.owner, sum.name, sum.default_branch);

    let o = fetch::fetch_mapped(&s, sum.id, &upstream, &JobCtx::noop()).unwrap();
    println!("fetched {} new commits, tags {:?}", o.new_commits, o.new_tags);
    assert!(o.new_commits >= 1);
    assert_eq!(repo::detail(&s, sum.id).unwrap().can_fast_forward, Some(true));
    let tip = fetch::fast_forward_default(&s, sum.id).unwrap();
    assert_eq!(tip, expect_tip, "default branch now at upstream's tip");
    assert_eq!(repo::detail(&s, sum.id).unwrap().can_fast_forward, Some(false));
    println!("fast-forwarded to {tip}");
}
