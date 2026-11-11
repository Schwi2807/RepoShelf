//! `rs` — RepoShelf developer tool (not shipped, D-15).

const USAGE: &str = "\
rs — RepoShelf developer tool

USAGE:
    rs fixtures <dir>            build every fixture repo under <dir>
    rs bench <dir>               time the read functions on the fixtures in <dir>
    rs inspect <repo-path> <cmd> print one read function's result as JSON
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") | Some("help") => print!("{USAGE}"),
        Some(cmd) => {
            eprintln!("rs: '{cmd}' is not implemented yet\n");
            eprint!("{USAGE}");
            std::process::exit(2);
        }
    }
}
