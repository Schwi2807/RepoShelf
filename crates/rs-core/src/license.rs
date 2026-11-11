//! License detection (M2.10; FR-18, ARCH §6.8): root `LICENSE*`/`LICENCE*`/`COPYING*`, first 2 KB matched against
//! well-known phrases; a license file that matches nothing is "Other".

use git2::{Repository, Tree};

const SNIFF_BYTES: usize = 2048;

fn is_license_name(name: &str) -> bool {
    let u = name.to_ascii_uppercase();
    ["LICENSE", "LICENCE", "COPYING"].iter().any(|p| u.starts_with(p))
}

/// Normalised (lower-case, single spaces) first 2 KB → SPDX-like id.
pub fn classify(text: &str) -> &'static str {
    let t: String = text.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    let has = |p: &str| t.contains(p);
    if has("gnu affero general public license") {
        "AGPL-3.0"
    } else if has("gnu lesser general public license") {
        if has("version 2.1") { "LGPL-2.1" } else { "LGPL-3.0" }
    } else if has("gnu general public license") {
        if has("version 3") { "GPL-3.0" } else { "GPL-2.0" }
    } else if has("apache license") && has("version 2.0") {
        "Apache-2.0"
    } else if has("mozilla public license") && has("2.0") {
        "MPL-2.0"
    } else if has("permission is hereby granted, free of charge") {
        "MIT"
    } else if has("permission to use, copy, modify, and/or distribute this software for any purpose") {
        "ISC"
    } else if has("redistribution and use in source and binary forms") {
        if has("neither the name") || has("names of its contributors") { "BSD-3-Clause" } else { "BSD-2-Clause" }
    } else if has("this is free and unencumbered software released into the public domain") {
        "Unlicense"
    } else if has("cc0 1.0") || (has("creative commons") && has("cc0")) {
        "CC0-1.0"
    } else {
        "Other"
    }
}

/// License of the root tree, if a license file exists.
pub fn detect_license(repo: &Repository, tree: &Tree) -> Option<String> {
    let mut candidates: Vec<_> = tree
        .iter()
        .filter(|e| e.kind() == Some(git2::ObjectType::Blob))
        .filter_map(|e| std::str::from_utf8(e.name_bytes()).ok().filter(|n| is_license_name(n)).map(|n| (n.to_string(), e.id())))
        .collect();
    // Prefer the plain names (LICENSE before LICENSE-THIRD-PARTY).
    candidates.sort_by_key(|(n, _)| (n.len(), n.clone()));
    let (_, oid) = candidates.into_iter().next()?;
    let blob = repo.find_blob(oid).ok()?;
    let head = &blob.content()[..blob.size().min(SNIFF_BYTES)];
    Some(classify(&String::from_utf8_lossy(head)).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{self, T0};

    #[test]
    fn phrases() {
        assert_eq!(classify(testkit::MIT_LICENSE), "MIT");
        assert_eq!(classify("                                 Apache License\n                           Version 2.0, January 2004"), "Apache-2.0");
        assert_eq!(classify("GNU GENERAL PUBLIC LICENSE\n Version 3, 29 June 2007"), "GPL-3.0");
        assert_eq!(classify("GNU GENERAL PUBLIC LICENSE\n Version 2, June 1991"), "GPL-2.0");
        assert_eq!(classify("GNU LESSER GENERAL PUBLIC LICENSE Version 2.1, February 1999"), "LGPL-2.1");
        assert_eq!(classify("GNU AFFERO GENERAL PUBLIC LICENSE Version 3"), "AGPL-3.0");
        assert_eq!(classify("Mozilla Public License Version 2.0"), "MPL-2.0");
        assert_eq!(classify("Redistribution and use in source and binary forms ... Neither the name of"), "BSD-3-Clause");
        assert_eq!(classify("Redistribution and use in source and binary forms, with or without"), "BSD-2-Clause");
        assert_eq!(classify("Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee"), "ISC");
        assert_eq!(classify("This is free and unencumbered software released into the public domain."), "Unlicense");
        assert_eq!(classify("CC0 1.0 Universal"), "CC0-1.0");
        assert_eq!(classify("All rights reserved."), "Other");
    }

    #[test]
    fn detects_from_tree() {
        let t = tempfile::tempdir().unwrap();
        let mut b = testkit::repo(&t.path().join("r"));
        b.bare()
            .file("LICENSE-APACHE", b"Apache License\nVersion 2.0, January 2004\n")
            .file("LICENSE", testkit::MIT_LICENSE.as_bytes())
            .commit("c", T0);
        let repo = testkit::open(&t.path().join("r"));
        let tree = repo.find_commit(b.head()).unwrap().tree().unwrap();
        assert_eq!(detect_license(&repo, &tree).as_deref(), Some("MIT"));

        let mut b2 = testkit::repo(&t.path().join("r2"));
        b2.bare().file("COPYING", b"Apache License\nVersion 2.0\n").commit("c", T0);
        let repo2 = testkit::open(&t.path().join("r2"));
        let tree = repo2.find_commit(b2.head()).unwrap().tree().unwrap();
        assert_eq!(detect_license(&repo2, &tree).as_deref(), Some("Apache-2.0"));

        let mut b3 = testkit::repo(&t.path().join("r3"));
        b3.bare().file("README", b"x").commit("c", T0);
        let repo3 = testkit::open(&t.path().join("r3"));
        let tree = repo3.find_commit(b3.head()).unwrap().tree().unwrap();
        assert_eq!(detect_license(&repo3, &tree), None);
    }
}
