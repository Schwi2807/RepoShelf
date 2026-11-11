//! Releases from version tags (M2.11; FR-31, ARCH §6.7).

use crate::error::Result;
use crate::types::{Release, Version};
use crate::validate::GitPath;
use git2::Repository;
use std::cmp::Ordering;

/// `^v?\d+(\.\d+){0,3}([-+.][0-9A-Za-z.-]+)?$` → version. `pre` holds the suffix including its separator
/// (`-rc1`, `+build`, `.Final`); only a `-` suffix makes a prerelease.
pub fn is_version_tag(name: &str) -> Option<Version> {
    let s = name.strip_prefix('v').unwrap_or(name);
    let mut nums = [0u64; 4];
    let mut count = 0;
    let mut rest = s;
    loop {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || count == 4 {
            return None;
        }
        nums[count] = rest[..digits].parse().ok()?;
        count += 1;
        rest = &rest[digits..];
        // another numeric part?
        if let Some(after) = rest.strip_prefix('.')
            && after.bytes().next().is_some_and(|b| b.is_ascii_digit())
            && count < 4
        {
            rest = after;
            continue;
        }
        break;
    }
    let pre = if rest.is_empty() {
        None
    } else {
        let mut ch = rest.chars();
        let sep = ch.next()?;
        let tail = ch.as_str();
        if !matches!(sep, '-' | '+' | '.') || tail.is_empty() || !tail.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
            return None;
        }
        Some(rest.to_string())
    };
    Some(Version { major: nums[0], minor: nums[1], patch: nums[2], extra: nums[3], pre })
}

impl Version {
    pub fn is_prerelease(&self) -> bool {
        self.pre.as_deref().is_some_and(|p| p.starts_with('-'))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Numeric parts, then: prerelease < release (other suffixes rank as releases), then suffix text.
impl Ord for Version {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.major, self.minor, self.patch, self.extra)
            .cmp(&(o.major, o.minor, o.patch, o.extra))
            .then((!self.is_prerelease()).cmp(&!o.is_prerelease()))
            .then(self.pre.cmp(&o.pre))
    }
}

/// One card per version-like tag, newest first (tag date; ties by version desc). `latest` = newest stable.
pub fn list(repo: &Repository) -> Result<Vec<Release>> {
    let mut found: Vec<(Release, Version)> = Vec::new();
    for t in crate::refs::tags(repo)? {
        let Some(v) = is_version_tag(&t.item.name) else { continue };
        let (title, notes_html) = if t.annotated {
            let msg = repo
                .find_reference(&t.item.full_name)
                .ok()
                .and_then(|r| r.target())
                .and_then(|oid| repo.find_tag(oid).ok())
                .and_then(|tag| tag.message_bytes().map(|b| String::from_utf8_lossy(b).into_owned()))
                .unwrap_or_default();
            let mut parts = msg.splitn(2, '\n');
            let first = parts.next().unwrap_or("").trim().to_string();
            let body = parts.next().unwrap_or("").trim();
            let title = if first.is_empty() || first == t.item.name { t.item.name.clone() } else { first };
            let notes = (!body.is_empty()).then(|| crate::markdown::render(body, &GitPath::root()).html);
            (title, notes)
        } else {
            (t.item.name.clone(), None)
        };
        found.push((
            Release {
                tag: t.item.name.clone(),
                commit: t.item.target.clone(),
                date: t.date,
                title,
                notes_html,
                prerelease: v.is_prerelease(),
                latest: false,
                previous_tag: None,
                gh_notes: false,
            },
            v,
        ));
    }
    found.sort_by(|a, b| b.0.date.cmp(&a.0.date).then(b.1.cmp(&a.1)));
    let mut out: Vec<Release> = found.into_iter().map(|(r, _)| r).collect();
    if let Some(i) = out.iter().position(|r| !r.prerelease) {
        out[i].latest = true;
    }
    for i in 0..out.len() {
        out[i].previous_tag = out.get(i + 1).map(|r| r.tag.clone());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit;

    #[test]
    fn version_syntax() {
        let v = |s: &str| is_version_tag(s);
        assert_eq!(v("v1.2.3").unwrap(), Version { major: 1, minor: 2, patch: 3, extra: 0, pre: None });
        assert_eq!(v("1").unwrap().major, 1);
        assert_eq!(v("1.2.3.4").unwrap().extra, 4);
        assert_eq!(v("v2.0.0-rc1").unwrap().pre.as_deref(), Some("-rc1"));
        assert!(v("v2.0.0-rc1").unwrap().is_prerelease());
        assert!(!v("1.0+build.5").unwrap().is_prerelease());
        assert_eq!(v("2024.1.0").unwrap().major, 2024);
        for bad in ["", "v", "va1", "release-1", "1.2.", "1.2.3-", "1.2.3_x", "v1.2.3 beta", "snapshot", "-1"] {
            assert!(v(bad).is_none(), "{bad}");
        }
        let mut list = ["1.0.0", "1.0.0-rc1", "0.9", "1.0.1", "1.0.0-beta", "1.0.0.1"].map(|s| v(s).unwrap());
        list.sort();
        let order: Vec<String> = list.iter().map(|x| format!("{}.{}.{}.{}{}", x.major, x.minor, x.patch, x.extra, x.pre.clone().unwrap_or_default())).collect();
        assert_eq!(order, ["0.9.0.0", "1.0.0.0-beta", "1.0.0.0-rc1", "1.0.0.0", "1.0.0.1", "1.0.1.0"]);
    }

    #[test]
    fn fixture_cards() {
        let t = tempfile::tempdir().unwrap();
        let p = testkit::releases(&t.path().join("r"));
        let repo = testkit::open(&p);
        let rs = list(&repo).unwrap();
        let tags: Vec<_> = rs.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["v2.0.0-rc1", "v1.1.0", "v1.0.0", "v0.9.0"], "snapshot is not a version");
        assert!(rs[0].prerelease && !rs[0].latest);
        assert!(rs[1].latest && !rs[1].prerelease, "latest = newest stable");
        assert_eq!(rs.iter().filter(|r| r.latest).count(), 1);
        // lightweight tag: title = name, no notes
        assert_eq!((rs[1].title.as_str(), rs[1].notes_html.as_deref()), ("v1.1.0", None));
        // annotated: title from the first line unless it repeats the tag name; notes sanitised markdown
        assert_eq!(rs[0].title, "Release candidate 1");
        let n = rs[0].notes_html.as_deref().unwrap();
        assert!(n.contains("Try it") && !n.contains("<script") && !n.contains("alert"), "{n}");
        assert_eq!(rs[2].title, "v1.0.0");
        assert!(rs[2].notes_html.as_deref().unwrap().contains("<li>item one</li>"));
        assert_eq!(rs[3].title, "Beta release");
        assert!(rs[3].notes_html.as_deref().unwrap().contains("<strong>public</strong>"));
        // compare chain
        assert_eq!(rs[0].previous_tag.as_deref(), Some("v1.1.0"));
        assert_eq!(rs[3].previous_tag, None);
        assert!(rs[0].date > rs[1].date);
        assert!(rs.iter().all(|r| !r.gh_notes && r.commit.len() == 40));
    }
}
