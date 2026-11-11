//! `rs` — RepoShelf developer tool (not shipped, D-15).

mod bench;
mod fixtures;
mod inspect;

use std::path::PathBuf;

const USAGE: &str = "\
rs — RepoShelf developer tool

USAGE:
    rs fixtures <dir>            build every fixture repo under <dir>
    rs fixtures --register <n> <dir>
                                 build <n> copies of the basic fixture under <dir>\\registered and import them
                                 into the shelf of RS_DATA_DIR / RS_ARCHIVE_DIR (both must be set)
    rs bench <dir>               time the read functions on the fixtures in <dir> (exit 1 on a missed target)
    rs inspect <repo-path> <cmd> print one read function's result as JSON

INSPECT COMMANDS:
    detail | refs | branches | tags | releases
    resolve <rev> | tree <rev> [path] | blob <rev> <path> | log <rev> [skip] [path]
    commit <oid> | last <rev> [dir] | langs <rev> | readme <rev> [dir] | count <rev>
";

fn fail(msg: &str, code: i32) -> ! {
    eprintln!("rs: {msg}");
    std::process::exit(code);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") | Some("help") => print!("{USAGE}"),
        Some("fixtures") if args.get(1).map(String::as_str) == Some("--register") => {
            let n = args.get(2).and_then(|v| v.parse::<u32>().ok()).filter(|n| (1..=500).contains(n));
            let (Some(n), Some(dir)) = (n, args.get(3)) else { fail("fixtures --register needs <n> (1..500) and <dir>", 2) };
            if let Err(e) = fixtures::register(&PathBuf::from(dir), n) {
                fail(&e, 2);
            }
        }
        Some("fixtures") => {
            let Some(dir) = args.get(1) else { fail("fixtures needs <dir>", 2) };
            if let Err(e) = fixtures::run(&PathBuf::from(dir)) {
                fail(&e, 2);
            }
        }
        Some("bench") => {
            let Some(dir) = args.get(1) else { fail("bench needs <dir>", 2) };
            match bench::run(&PathBuf::from(dir)) {
                Ok(rows) => {
                    bench::print(&rows);
                    if rows.iter().any(|r| !r.ok()) {
                        std::process::exit(1);
                    }
                }
                Err(e) => fail(&e, 2),
            }
        }
        Some("inspect") => {
            if args.len() < 3 {
                fail("inspect needs <repo-path> <cmd>", 2);
            }
            match inspect::run(&PathBuf::from(&args[1]), &args[2], &args[3..]) {
                Ok(json) => println!("{json}"),
                Err(e) => fail(&e, 1),
            }
        }
        Some(cmd) => {
            eprintln!("rs: unknown command '{cmd}'\n");
            eprint!("{USAGE}");
            std::process::exit(2);
        }
    }
}
