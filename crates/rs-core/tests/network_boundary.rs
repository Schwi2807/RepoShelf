//! M3.9 (NFR-O1): browsing and shelf management never touch the network. Every M2 read function and every
//! library function runs on fixtures and the per-thread connection counter (`testkit::net_calls`) stays 0.

use rs_core::jobs::CancelToken;
use rs_core::testkit::{self, net_calls, read_everything};
use rs_core::validate::MetadataInput;
use rs_core::{clone, library};

#[test]
fn reads_and_library_functions_make_no_network_calls() {
    let t = tempfile::tempdir().unwrap();
    let s = testkit::shelf(t.path());
    let start = net_calls();

    let basic = testkit::basic(&t.path().join("basic"));
    let releases = testkit::releases(&t.path().join("releases"));
    let merges = testkit::merges(&t.path().join("merges"));
    let (corrupt, _) = testkit::corrupt(&t.path().join("corrupt"));
    let missing = testkit::missing(&t.path().join("missing"));

    let _ = library::preview_import(&s, &basic.path).unwrap();
    let b = library::import(&s, &basic.path).unwrap();
    let r = library::import(&s, &releases).unwrap();
    let m = library::import(&s, &merges.path).unwrap();
    let c = library::import(&s, &corrupt).unwrap();
    let gone = library::import(&s, &missing).unwrap();
    std::fs::rename(&missing, t.path().join("missing.moved")).unwrap();
    let _ = library::import(&s, &basic.path).unwrap_err();

    let calls: usize = [b.id, r.id, m.id].iter().map(|id| read_everything(&s, *id)).sum();
    assert!(calls > 100, "{calls}");

    let _ = library::list(&s).unwrap();
    let _ = library::by_slug(&s, &b.owner, &b.name).unwrap();
    let _ = library::refresh_summaries(&s, &CancelToken::new()).unwrap();
    let _ = library::refresh_one(&s, c.id, &CancelToken::new()).unwrap();
    library::touch_opened(&s, b.id).unwrap();
    let _ = library::recent(&s, 5).unwrap();
    let _ = library::set_metadata(&s, b.id, MetadataInput { description: Some("d".into()), homepage: None, topics: vec!["t".into()] }).unwrap();
    let _ = library::relocate(&s, gone.id, &t.path().join("missing.moved")).unwrap();
    let _ = library::remove(&s, gone.id).unwrap();
    let _ = clone::preview(&s, "owner/repo").unwrap();
    let _ = clone::cleanup_partials(&s.dirs.archive).unwrap();
    let _ = rs_core::repo::detail(&s, b.id).unwrap();

    assert_eq!(net_calls(), start, "a browse/library function connected to the network");
}
