//! M4.12 / D-24: one serialised sample of every IPC type and enum variant. Written to `target/ipc-samples.json`
//! and compared (as parsed JSON) with the committed golden `src/ipc/__samples__/ipc-samples.json`, which
//! `src/ipc/types.test.ts` checks against the TS types. `UPDATE_SAMPLES=1 cargo test -p reposhelf ipc_samples`
//! rewrites the golden.

use crate::cmd::dialogs::RemoveResult;
use crate::ipc_error::IpcError;
use rs_core::jobs::{JobInfo, JobKind, JobState, Progress};
use rs_core::settings::Settings;
use rs_core::types::*;
use serde_json::{Map, Value, json, to_value as v};

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/ipc/__samples__/ipc-samples.json");
const OUT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/ipc-samples.json");

fn job(id: u64, kind: JobKind, state: JobState, progress: Option<Progress>, result: Option<JobResult>, error: Option<&str>) -> Value {
    v(JobInfo { id, kind, state, progress, title: "Clone o/r".into(), result, error: error.map(String::from), started_at: 1_700_000_000 }).unwrap()
}

/// Key → sample. Unit enum values are keyed `Type.value`; tagged variants `Type.variant`.
pub fn samples() -> Map<String, Value> {
    let mut out = Map::new();
    let mut put = |k: String, val: Value| {
        assert!(out.insert(k.clone(), val).is_none(), "duplicate sample key {k}");
    };
    for (name, val) in rs_core::types::samples::all() {
        match &val {
            Value::String(s) => put(format!("{name}.{s}"), val.clone()),
            _ => put(name.to_string(), val),
        }
    }
    // variants the rs-core list only has nested
    put("RepoStatus.corrupt".into(), v(RepoStatus::Corrupt { message: "bad pack".into() }).unwrap());
    put("HeadState.unborn".into(), v(HeadState::Unborn { name: "main".into() }).unwrap());
    put("BlobKind.image".into(), v(BlobKind::Image { mime: "image/png".into() }).unwrap());
    put("EntryKind.submodule".into(), v(EntryKind::Submodule).unwrap());
    for k in [RefKind::Branch, RefKind::RemoteBranch] {
        put(format!("RefKind.{}", v(k).unwrap().as_str().unwrap()), v(k).unwrap());
    }
    put("ChangeKind.typeChange".into(), v(ChangeKind::TypeChange).unwrap());
    put("LineKind.noNewline".into(), v(LineKind::NoNewline).unwrap());
    put("Source.imported".into(), v(Source::Imported).unwrap());
    put("Source.cloned".into(), v(Source::Cloned).unwrap());
    for k in [SortKey::Updated, SortKey::Name, SortKey::Added] {
        put(format!("SortKey.{}", v(k).unwrap().as_str().unwrap()), v(k).unwrap());
    }
    put("Time".into(), v(Time { secs: 1, offset_min: -300 }).unwrap());
    // src-tauri types
    put("IpcError".into(), v(IpcError::new("invalidPath", "invalid path: ..")).unwrap());
    put("RemoveResult".into(), v(RemoveResult { trashed: true }).unwrap());
    put("Settings".into(), v(Settings { show_dirty_note: true, ..Settings::default() }).unwrap());
    put("AheadBehindEntry".into(), v(("feature".to_string(), AheadBehind { ahead: 1, behind: 0, capped: false })).unwrap());
    // jobs
    put("JobKind.clone".into(), v(JobKind::Clone { url: "https://github.com/o/r.git".into() }).unwrap());
    put("JobKind.fetch".into(), v(JobKind::Fetch { repo_id: 3 }).unwrap());
    put("JobKind.fetchAll".into(), v(JobKind::FetchAll).unwrap());
    for s in [JobState::Waiting, JobState::Running, JobState::Done, JobState::Failed, JobState::Cancelled] {
        put(format!("JobState.{}", s.as_db()), v(s).unwrap());
    }
    put("Progress.connecting".into(), v(Progress::Connecting).unwrap());
    put("Progress.receiving".into(), v(Progress::Receiving { received: 5, total: 10, bytes: 2048 }).unwrap());
    put("Progress.resolving".into(), v(Progress::Resolving { done: 1, total: 4 }).unwrap());
    put("Progress.finalizing".into(), v(Progress::Finalizing).unwrap());
    put(
        "JobInfo".into(),
        job(7, JobKind::Clone { url: "https://github.com/o/r.git".into() }, JobState::Running, Some(Progress::Receiving { received: 1, total: 2, bytes: 3 }), None, None),
    );
    put(
        "JobInfo.done".into(),
        job(8, JobKind::Fetch { repo_id: 1 }, JobState::Done, None, Some(JobResult::Fetched(FetchOutcome { new_commits: 1, ..Default::default() })), None),
    );
    put("JobInfo.failed".into(), job(9, JobKind::FetchAll, JobState::Failed, None, None, Some("Could not reach github.com: failed to connect to host")));
    out
}

#[test]
fn ipc_samples_match_the_golden() {
    let now = Value::Object(samples());
    let text = serde_json::to_string_pretty(&now).unwrap() + "\n";
    std::fs::create_dir_all(std::path::Path::new(OUT).parent().unwrap()).unwrap();
    std::fs::write(OUT, &text).unwrap();
    if std::env::var("UPDATE_SAMPLES").is_ok_and(|v| v == "1") {
        std::fs::create_dir_all(std::path::Path::new(GOLDEN).parent().unwrap()).unwrap();
        std::fs::write(GOLDEN, &text).unwrap();
        return;
    }
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(GOLDEN).expect("golden missing: run with UPDATE_SAMPLES=1"))
        .expect("golden is not JSON");
    assert!(
        golden == now,
        "IPC samples changed (Rust types drifted from src/ipc/__samples__/ipc-samples.json). Compare target/ipc-samples.json; \
         if intended, rerun with UPDATE_SAMPLES=1 and update src/ipc/types.ts"
    );
}

#[test]
fn samples_cover_every_tagged_variant() {
    let s = samples();
    for k in [
        "RepoStatus.ok", "RepoStatus.missing", "RepoStatus.empty", "RepoStatus.corrupt", "HeadState.branch", "HeadState.detached",
        "HeadState.unborn", "BlobKind.text", "BlobKind.image", "BlobKind.binary", "BlobKind.lfsPointer", "BlobKind.submodule",
        "BlobKind.symlink", "JobResult.cloned", "JobResult.fetched",
    ] {
        assert!(s.contains_key(k), "{k}");
    }
    assert_eq!(s["IpcError"], json!({"code": "invalidPath", "message": "invalid path: .."}));
}
