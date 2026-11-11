//! Windows storability rules (NFR-W2, NFR-W3, FR-9). Pure string functions, mirroring libgit2's NT rules
//! (`util/fs_path.c` `validate_component`/`validate_dospath`, `libgit2/path.c` `validate_dotgit_ntfs`).
//! Reimplemented from the PM ideas, not copied.

use std::collections::HashSet;

const NT_INVALID: &[char] = &['<', '>', ':', '"', '|', '?', '*', '\\'];

/// libgit2 `validate_dospath`: `CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9` (any case), alone or followed by
/// `.` or `:` (so `CON.txt` and `con.tar.gz` are refused, `CONx`, `console.txt`, `COM0`, `COM10` are fine).
fn is_dos_device(c: &str) -> bool {
    let b = c.as_bytes();
    let check = |name: &[u8; 3], trailing_num: bool| -> bool {
        let last = if trailing_num { 4 } else { 3 };
        if b.len() < last || !b[..3].eq_ignore_ascii_case(name) {
            return false;
        }
        if trailing_num && !(b'1'..=b'9').contains(&b[3]) {
            return false;
        }
        b.len() == last || b[last] == b'.' || b[last] == b':'
    };
    check(b"CON", false)
        || check(b"PRN", false)
        || check(b"AUX", false)
        || check(b"NUL", false)
        || check(b"COM", true)
        || check(b"LPT", true)
}

/// `.git` / `git~1` in any case, with trailing dots/spaces (NTFS elides them).
fn is_ntfs_dotgit(c: &str) -> bool {
    let t = c.trim_end_matches(['.', ' ']).to_ascii_lowercase();
    t == ".git" || t == "git~1"
}

fn component_ok(c: &str) -> bool {
    if c.is_empty() || c == "." || c == ".." {
        return false;
    }
    if c.chars().any(|ch| (ch as u32) < 32 || NT_INVALID.contains(&ch)) {
        return false;
    }
    if c.ends_with('.') || c.ends_with(' ') {
        return false;
    }
    !is_dos_device(c) && !is_ntfs_dotgit(c)
}

/// Can this git path (`/`-separated) be stored as a file on Windows? NT-invalid chars, control chars, trailing
/// dot/space, DOS device names, `.git`/`git~1` components → false.
pub fn path_ok_here(git_path: &str) -> bool {
    !git_path.is_empty() && git_path.split('/').all(component_ok)
}

/// Can this ref name be stored as a loose ref file on Windows? Accepts short (`feature/x`) or full
/// (`refs/heads/feature/x`) names. Also requires git ref-name validity.
pub fn ref_name_ok_here(name: &str) -> bool {
    let full = if name.starts_with("refs/") { name.to_string() } else { format!("refs/heads/{name}") };
    git2::Reference::is_valid_name(&full) && path_ok_here(name)
}

fn fold(s: &str, ignore_case: bool) -> String {
    if ignore_case { s.to_lowercase() } else { s.to_string() }
}

/// Two loose ref names cannot coexist: equal under case folding (on a case-insensitive fs), or one is a
/// directory prefix of the other (`llm` vs `LLM/x`).
fn clash(a: &str, b: &str) -> bool {
    a == b || b.strip_prefix(a).is_some_and(|r| r.starts_with('/')) || a.strip_prefix(b).is_some_and(|r| r.starts_with('/'))
}

/// Splits advertised ref `names` (short names relative to one namespace) into `(keep, skipped)`.
///
/// `existing` = names already stored locally in that namespace; they always win (an identical name is an update,
/// not a clash). `prefer` (e.g. the default branch) is considered first, then `names` in input order; a name is
/// kept unless it is not storable on Windows or clashes with an already-kept/existing name. Both outputs keep
/// input order.
pub fn storable_refs(names: &[String], existing: &[String], prefer: Option<&str>, ignore_case: bool) -> (Vec<String>, Vec<String>) {
    let mut taken: Vec<String> = existing.iter().map(|e| fold(e, ignore_case)).collect();
    let mut kept: HashSet<&str> = HashSet::new();
    let mut order: Vec<&String> = Vec::with_capacity(names.len());
    if let Some(p) = prefer
        && let Some(n) = names.iter().find(|n| n.as_str() == p)
    {
        order.push(n);
    }
    order.extend(names.iter().filter(|n| Some(n.as_str()) != prefer));

    for n in order {
        if kept.contains(n.as_str()) || !ref_name_ok_here(n) {
            continue;
        }
        let f = fold(n, ignore_case);
        let exact_existing = existing.iter().any(|e| e == n);
        let conflicts = taken.iter().any(|t| clash(t, &f));
        if exact_existing || !conflicts {
            kept.insert(n.as_str());
            if !exact_existing {
                taken.push(f);
            }
        }
    }
    let mut keep = Vec::new();
    let mut skipped = Vec::new();
    let mut seen = HashSet::new();
    for n in names {
        if !seen.insert(n.as_str()) {
            continue;
        }
        if kept.contains(n.as_str()) { keep.push(n.clone()) } else { skipped.push(n.clone()) }
    }
    (keep, skipped)
}

/// Fetch refspecs mapping `src` (e.g. `refs/heads/`) to `dst` (e.g. `refs/remotes/origin/`): one wildcard when
/// nothing was skipped, else one explicit refspec per kept name.
pub fn refspecs(src: &str, dst: &str, keep: &[String], skipped: &[String]) -> Vec<String> {
    if skipped.is_empty() {
        vec![format!("+{src}*:{dst}*")]
    } else {
        keep.iter().map(|n| format!("+{src}{n}:{dst}{n}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn nt_invalid_names() {
        for bad in [
            "a:b.txt", "CON.txt", "con.txt", "CON.tar.gz", "con", "Con:", "nul", "aux.c", "prn.x", "COM1", "com9.txt", "LPT3",
            "dir/CON", "x.", "x ", "dir./y", "a<b", "a>b", "a\"b", "a|b", "a?b", "a*b", "a\u{1}b", ".git", ".GIT", ".git.",
            ".git ", "git~1", "GIT~1", "sub/.git", "", "a//b",
        ] {
            assert!(!path_ok_here(bad), "{bad:?} should be refused");
        }
    }

    #[test]
    fn near_misses_are_fine() {
        for ok in [
            "CONx", "console.txt", "COM0", "COM10", "LPT0.txt", "auxiliary", "nul-ish", "README.md", "src/main.rs", ".gitignore",
            ".github/workflows/ci.yml", "git~2x", ".gitx", "a.b.c", "ünïcode/名前.txt", "x..y",
        ] {
            assert!(path_ok_here(ok), "{ok:?} should be accepted");
        }
    }

    #[test]
    fn ref_names() {
        assert!(ref_name_ok_here("main"));
        assert!(ref_name_ok_here("feature/x"));
        assert!(ref_name_ok_here("refs/tags/v1.0"));
        assert!(!ref_name_ok_here("con"));
        assert!(!ref_name_ok_here("feat/aux.x"));
        assert!(!ref_name_ok_here("a|b"));
        assert!(!ref_name_ok_here("x<y"));
        assert!(!ref_name_ok_here("a..b")); // git-invalid
        assert!(!ref_name_ok_here("x.lock"));
    }

    #[test]
    fn case_and_dir_clash_without_prefer() {
        let (keep, skip) = storable_refs(&s(&["llm", "LLM/x", "main"]), &[], None, true);
        assert_eq!(keep, s(&["llm", "main"]));
        assert_eq!(skip, s(&["LLM/x"]));
    }

    #[test]
    fn case_and_dir_clash_with_prefer() {
        let (keep, skip) = storable_refs(&s(&["llm", "LLM/x", "main"]), &[], Some("LLM/x"), true);
        assert_eq!(keep, s(&["LLM/x", "main"]));
        assert_eq!(skip, s(&["llm"]));
    }

    #[test]
    fn case_sensitive_fs_only_dir_clashes() {
        let (keep, skip) = storable_refs(&s(&["llm", "LLM/x", "Main", "main", "a", "a/b"]), &[], None, false);
        assert_eq!(keep, s(&["llm", "LLM/x", "Main", "main", "a"]));
        assert_eq!(skip, s(&["a/b"]));
    }

    #[test]
    fn existing_names_win_and_updates_are_kept() {
        let existing = s(&["Feature", "main"]);
        let (keep, skip) = storable_refs(&s(&["main", "feature", "feature/y", "dev"]), &existing, None, true);
        assert_eq!(keep, s(&["main", "dev"]));
        assert_eq!(skip, s(&["feature", "feature/y"]));
    }

    #[test]
    fn unstorable_names_skipped() {
        let (keep, skip) = storable_refs(&s(&["main", "con", "v1|x", "ok/aux.c"]), &[], Some("main"), true);
        assert_eq!(keep, s(&["main"]));
        assert_eq!(skip, s(&["con", "v1|x", "ok/aux.c"]));
    }

    #[test]
    fn wildcard_refspec_when_nothing_skipped() {
        let keep = s(&["main", "dev"]);
        assert_eq!(refspecs("refs/heads/", "refs/remotes/origin/", &keep, &[]), vec!["+refs/heads/*:refs/remotes/origin/*"]);
        assert_eq!(
            refspecs("refs/heads/", "refs/remotes/origin/", &keep, &s(&["LLM/x"])),
            vec!["+refs/heads/main:refs/remotes/origin/main", "+refs/heads/dev:refs/remotes/origin/dev"]
        );
    }
}
