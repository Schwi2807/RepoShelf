//! Markdown → sanitised HTML (M2.10; D-4, ARCH §6.6, NFR-S1, R-2).
//!
//! 1. comrak (GFM) with raw HTML passed through — ammonia is the single gate.
//! 2. Relative URLs are resolved against the file's directory into `rsrepo:/<git path>[#frag]` (`..` resolved
//!    lexically; escaping above the root drops the URL); fragment-only links get the `user-content-` prefix.
//! 3. ammonia allow-list; `data:` only as `data:image/(png|gif|jpeg|webp)` in `img[src]`; author-written `rsrepo:`
//!    URLs are dropped by the scheme check (only our own rewrite produces them).
//! 4. Headings collected for the outline.

use crate::types::{Heading, Rendered};
use crate::validate::GitPath;
use ammonia::{Builder, UrlRelative};
use comrak::nodes::NodeValue;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

/// Prefix of every id in rendered markdown (no clash with app ids).
pub const ID_PREFIX: &str = "user-content-";
pub const RSREPO: &str = "rsrepo:/";

/// `code[class=language-<id>]` survives only for these ids.
const CODE_LANGS: &[&str] = &[
    "asm", "bash", "c", "c++", "clojure", "cmake", "console", "cpp", "cs", "csharp", "css", "dart", "diff", "dockerfile",
    "elixir", "erlang", "fsharp", "go", "graphql", "groovy", "haskell", "hcl", "html", "ini", "java", "javascript",
    "js", "json", "jsonc", "jsx", "julia", "kotlin", "kt", "latex", "less", "lua", "makefile", "markdown", "md", "nginx",
    "nim", "objectivec", "ocaml", "perl", "php", "plaintext", "powershell", "protobuf", "ps1", "py", "python", "r", "rb",
    "ruby", "rust", "rs", "scala", "scss", "sh", "shell", "solidity", "sql", "svelte", "swift", "terraform", "tex", "text",
    "toml", "ts", "tsx", "typescript", "vb", "vim", "vue", "wasm", "x86asm", "xml", "yaml", "yml", "zig", "zsh",
];

static LANG_CLASSES: std::sync::LazyLock<Vec<String>> =
    std::sync::LazyLock::new(|| CODE_LANGS.iter().map(|l| format!("language-{l}")).collect());

fn options() -> comrak::Options<'static> {
    let mut o = comrak::Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.tasklist = true;
    o.extension.autolink = true;
    o.extension.footnotes = true;
    o.extension.header_id_prefix = Some(ID_PREFIX.to_string());
    o.extension.header_id_prefix_in_href = true;
    o.render.r#unsafe = true;
    o.render.github_pre_lang = false;
    o
}

/// Renders untrusted markdown. `base_dir` is the directory of the file (relative links resolve against it).
pub fn render(md: &str, base_dir: &GitPath) -> Rendered {
    let opts = options();
    let arena = comrak::Arena::new();
    let root = comrak::parse_document(&arena, md, &opts);
    // Same anchorizer sequence as comrak's HTML renderer, so outline anchors equal the rendered ids.
    let mut anchors = comrak::Anchorizer::new();
    let mut headings = Vec::new();
    for n in root.descendants() {
        let level = match &n.data().value {
            NodeValue::Heading(h) => h.level,
            _ => continue,
        };
        let text = n.collect_text();
        let anchor = format!("{ID_PREFIX}{}", anchors.anchorize(&text));
        headings.push(Heading { level, text, anchor });
    }
    let mut html = String::new();
    // Writing into a String cannot fail.
    let _ = comrak::format_html(root, &opts, &mut html);
    Rendered { html: sanitize(&html, base_dir), headings, source_path: String::new() }
}

/// Release notes (F5, D-97): `render` at the repository root, then every contributor avatar (`<img>` from
/// `avatars.githubusercontent.com` or `github.com/<user>.png`) becomes `@user` — a mention link, or plain text
/// when it already sits inside a link. Avatars are gone before the UI could ever fetch them.
pub fn render_release_notes(md: &str) -> String {
    strip_avatars(&render(md, &GitPath::root()).html)
}

/// GitHub user names: 1–39 of `[A-Za-z0-9-]`, not starting with `-`; bots end in `[bot]`.
fn valid_user(u: &str) -> bool {
    let base = u.strip_suffix("[bot]").unwrap_or(u);
    !base.is_empty() && base.len() <= 39 && !base.starts_with('-') && base.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn unescape_attr(v: &str) -> String {
    v.replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", "\u{a0}").replace("&amp;", "&")
}

/// `name="value"` pairs of one serialised tag (`<img a="b" c="d">`); html5ever always writes double quotes and
/// escapes `"` inside values, so a quote-aware scan is exact for sanitiser output.
fn tag_attrs(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = tag.trim_start_matches('<');
    rest = rest.split_once(char::is_whitespace).map(|(_, r)| r).unwrap_or("");
    while let Some(eq) = rest.find("=\"") {
        let name = rest[..eq].trim().to_string();
        let after = &rest[eq + 2..];
        let Some(end) = after.find('"') else { break };
        out.push((name, unescape_attr(&after[..end])));
        rest = &after[end + 1..];
    }
    out
}

/// End (exclusive) of the tag starting at `start` (`<`), skipping `>` inside double-quoted values.
fn tag_end(html: &str, start: usize) -> Option<usize> {
    let mut quoted = false;
    for (i, b) in html.as_bytes()[start..].iter().enumerate() {
        match b {
            b'"' => quoted = !quoted,
            b'>' if !quoted => return Some(start + i + 1),
            _ => {}
        }
    }
    None
}

/// `https://github.com/<user>` (optionally with a trailing `/`) → user.
fn github_profile_user(href: &str) -> Option<String> {
    let u = url::Url::parse(href).ok()?;
    if u.scheme() != "https" || u.host_str() != Some("github.com") {
        return None;
    }
    let path = u.path().trim_matches('/');
    (valid_user(path)).then(|| path.to_string())
}

/// `Some(user_from_src)` for an avatar URL (`None` inside = avatar host without a user in the URL).
fn avatar_src(src: &str) -> Option<Option<String>> {
    let u = url::Url::parse(src).ok()?;
    match u.host_str()? {
        "avatars.githubusercontent.com" => Some(None),
        "github.com" => {
            let user = u.path().trim_start_matches('/').strip_suffix(".png")?;
            valid_user(user).then(|| Some(user.to_string()))
        }
        _ => None,
    }
}

fn strip_avatars(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    // href of the `<a>` we are inside (sanitiser output never nests links)
    let mut open_link: Option<String> = None;
    while let Some(off) = html[i..].find('<') {
        let start = i + off;
        out.push_str(&html[i..start]);
        let Some(end) = tag_end(html, start) else {
            out.push_str(&html[start..]);
            return out;
        };
        let tag = &html[start..end];
        let lower = tag.to_ascii_lowercase();
        if lower.starts_with("<a ") || lower == "<a>" {
            open_link = Some(tag_attrs(tag).into_iter().find(|(k, _)| k == "href").map(|(_, v)| v).unwrap_or_default());
            out.push_str(tag);
        } else if lower.starts_with("</a") {
            open_link = None;
            out.push_str(tag);
        } else if lower.starts_with("<img") {
            let attrs = tag_attrs(tag);
            let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
            match get("src").and_then(avatar_src) {
                None => out.push_str(tag),
                Some(from_src) => {
                    let alt = get("alt").map(|a| a.trim().trim_start_matches('@').to_string()).filter(|a| valid_user(a));
                    let from_link = open_link.as_deref().and_then(github_profile_user);
                    match (open_link.is_some(), from_link.or(from_src).or(alt)) {
                        (true, Some(user)) => out.push_str(&format!("@{user}")),
                        (false, Some(user)) => out.push_str(&format!(
                            "<a href=\"https://github.com/{user}\" rel=\"noopener noreferrer\" class=\"user-mention\">@{user}</a>"
                        )),
                        (_, None) => {}
                    }
                }
            }
        } else {
            out.push_str(tag);
        }
        i = end;
    }
    out.push_str(&html[i..]);
    out
}

/// The ammonia gate (step 3) with the relative-URL rewrite (step 2).
pub fn sanitize(html: &str, base_dir: &GitPath) -> String {
    let base = base_dir.as_str().to_string();
    let tags: HashSet<&str> = [
        "h1", "h2", "h3", "h4", "h5", "h6", "p", "a", "img", "ul", "ol", "li", "table", "thead", "tbody", "tr", "th", "td",
        "pre", "code", "blockquote", "em", "strong", "del", "s", "hr", "br", "details", "summary", "sup", "sub", "kbd",
        "input", "section", "div",
    ]
    .into_iter()
    .collect();
    let clean_content: HashSet<&str> =
        ["script", "style", "iframe", "noscript", "template", "textarea", "title", "xmp", "noembed", "noframes"].into_iter().collect();
    let mut attrs: HashMap<&str, HashSet<&str>> = HashMap::new();
    attrs.insert("a", ["href", "title", "id"].into_iter().collect());
    // F3 (D-96): GitHub-style presentational `align` (values checked in `filter_attr`); no inline styles.
    attrs.insert("img", ["src", "alt", "title", "width", "height", "align"].into_iter().collect());
    for t in ["th", "td", "div", "p", "table"] {
        attrs.insert(t, ["align"].into_iter().collect());
    }
    for t in ["h1", "h2", "h3", "h4", "h5", "h6"] {
        attrs.insert(t, ["id", "align"].into_iter().collect());
    }
    for t in ["li", "sup"] {
        attrs.insert(t, ["id"].into_iter().collect());
    }
    attrs.insert("input", ["type", "checked", "disabled"].into_iter().collect());
    attrs.insert("ol", ["start"].into_iter().collect());
    attrs.insert("details", ["open"].into_iter().collect());
    let mut forced: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    forced.insert("input", [("type", "checkbox"), ("disabled", "")].into_iter().collect());
    let mut classes: HashMap<&str, HashSet<&str>> = HashMap::new();
    classes.insert("code", LANG_CLASSES.iter().map(String::as_str).collect());

    Builder::empty()
        .tags(tags)
        .clean_content_tags(clean_content)
        .tag_attributes(attrs)
        .set_tag_attribute_values(forced)
        .allowed_classes(classes)
        .url_schemes(["http", "https", "mailto", "data"].into_iter().collect())
        .link_rel(Some("noopener noreferrer"))
        .id_prefix(Some(ID_PREFIX))
        .strip_comments(true)
        .attribute_filter(filter_attr)
        .url_relative(UrlRelative::Custom(Box::new(Rewriter(base))))
        .clean(html)
        .to_string()
}

/// Step 2 as an ammonia evaluator (owns the base dir).
struct Rewriter(String);

impl<'a> ammonia::UrlRelativeEvaluate<'a> for Rewriter {
    fn evaluate<'url>(&self, url: &'url str) -> Option<Cow<'url, str>> {
        rewrite(&self.0, url).map(Cow::Owned)
    }
}

/// `data:` only for raster images in `img[src]`. The scheme is classified exactly as ammonia does (`Url::parse`,
/// which drops tabs/newlines and leading control characters), and the kept value is the normalised URL.
fn filter_attr<'a>(element: &str, attribute: &str, value: &'a str) -> Option<Cow<'a, str>> {
    match attribute {
        "align" => return align_value(value),
        "width" | "height" => return size_value(value),
        _ => {}
    }
    if !matches!(attribute, "href" | "src") {
        return Some(Cow::Borrowed(value));
    }
    let squashed: String = value.chars().filter(|c| !c.is_ascii_control() && !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    let parsed = url::Url::parse(value).ok();
    let is_data = parsed.as_ref().is_some_and(|u| u.scheme() == "data") || squashed.starts_with("data:");
    if !is_data {
        return Some(Cow::Borrowed(value));
    }
    let norm = parsed.filter(|u| u.scheme() == "data")?.as_str().to_string();
    let lower = norm.to_ascii_lowercase();
    let ok = element == "img"
        && attribute == "src"
        && ["data:image/png", "data:image/gif", "data:image/jpeg", "data:image/webp"]
            .iter()
            .any(|p| lower.strip_prefix(p).is_some_and(|rest| rest.starts_with(';') || rest.starts_with(',')));
    ok.then_some(Cow::Owned(norm))
}

/// `align` keeps only `left` / `center` / `right` (lower-cased); anything else drops the attribute.
fn align_value(v: &str) -> Option<Cow<'_, str>> {
    let l = v.trim().to_ascii_lowercase();
    matches!(l.as_str(), "left" | "center" | "right").then_some(Cow::Owned(l))
}

/// `width` / `height`: 1–5 digits with an optional `%` (a trailing `px` is dropped, as browsers read it).
fn size_value(v: &str) -> Option<Cow<'_, str>> {
    let t = v.trim();
    let t = t.strip_suffix("px").unwrap_or(t);
    let (digits, pct) = match t.strip_suffix('%') {
        Some(d) => (d, "%"),
        None => (t, ""),
    };
    (!digits.is_empty() && digits.len() <= 5 && digits.bytes().all(|b| b.is_ascii_digit())).then(|| Cow::Owned(format!("{digits}{pct}")))
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn percent_encode_path(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/!$&'()*+,;=:@".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// Step 2: relative URL → `rsrepo:/<git path>[#frag]`, fragment-only → `#user-content-…`; `None` drops it.
fn rewrite(base_dir: &str, url: &str) -> Option<String> {
    let u = url.trim();
    if u.starts_with("//") || u.contains('\\') {
        return None; // scheme-relative (another host) or Windows-style
    }
    let (path_q, frag) = match u.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (u, None),
    };
    let path = path_q.split_once('?').map(|(p, _)| p).unwrap_or(path_q);
    if path.is_empty() {
        let f = frag.filter(|f| !f.is_empty())?;
        let f = percent_encode_path(&percent_decode(f)?);
        return Some(if f.starts_with(ID_PREFIX) { format!("#{f}") } else { format!("#{ID_PREFIX}{f}") });
    }
    let decoded = percent_decode(path)?;
    let mut segs: Vec<&str> = if decoded.starts_with('/') || base_dir.is_empty() { Vec::new() } else { base_dir.split('/').collect() };
    for seg in decoded.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segs.pop()?; // above the repository root
            }
            s => segs.push(s),
        }
    }
    let joined = segs.join("/");
    crate::validate::git_path(&joined).ok()?;
    let mut out = format!("{RSREPO}{}", percent_encode_path(&joined));
    if let Some(f) = frag.filter(|f| !f.is_empty()) {
        out.push('#');
        out.push_str(&percent_encode_path(&percent_decode(f)?));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::git_path;

    fn root() -> GitPath {
        GitPath::root()
    }

    #[test]
    fn security_fixture_is_neutralised() {
        let md = String::from_utf8_lossy(crate::testkit::SECURITY_README).into_owned();
        let out = render(&md, &root()).html;
        let lower = out.to_lowercase();
        for bad in [
            "<script", "alert(", "onerror", "onload", "onclick", "javascript:", "<iframe", "style=", "<form", "data:text/html",
            "data:image/svg", "<object", "<embed", "<svg", "evil.example.com", "rsrepo:/a/", "outside.md", "password",
            "<button",
        ] {
            assert!(!lower.contains(bad), "`{bad}` survived:\n{out}");
        }
        // remote image kept as https (the UI blocks it), raster data image kept
        assert!(out.contains("src=\"https://images.example.com/tracker.png\""), "{out}");
        assert!(out.contains("src=\"data:image/png;base64,iVBORw0KGgo=\""), "{out}");
        // relative link and image rewritten
        assert!(out.contains("href=\"rsrepo:/docs/page.md\""), "{out}");
        assert!(out.contains("src=\"rsrepo:/svg-with-script.svg\""), "{out}");
        // GFM survives: tables, task lists, fenced code class
        assert!(out.contains("<table>") && out.contains("<td>a</td>"), "{out}");
        assert!(out.contains("<input type=\"checkbox\" checked=\"\" disabled=\"\">"), "{out}");
        assert!(out.contains("<code class=\"language-rust\">"), "{out}");
        // links get rel; fragment link lands on the prefixed heading id
        assert!(out.contains("rel=\"noopener noreferrer\""), "{out}");
        assert!(out.contains("href=\"#user-content-security-fixture\""), "{out}");
        assert!(out.contains("id=\"user-content-security-fixture\""), "{out}");
        // F3: `<div>` survives, but bare (onclick dropped, id prefixed only on headings/a/li/sup)
        assert!(out.contains("<div>id clash</div>"), "{out}");
    }

    /// F3 (v0.2.0, D-96). The v0.1.0 sanitiser (proved in commit c7f178f) unwrapped `<div>` and kept `align` only
    /// on td/th, so nothing was centred, and passed `width="100px;evil"` unchecked. Now GitHub's presentational
    /// attributes survive with checked values (the browser applies them natively; no inline styles).
    #[test]
    fn align_fixture_matches_github() {
        let md = include_str!("../../../tests/fixtures/align/README.md");
        let out = render(md, &root()).html;
        assert!(out.contains("<div align=\"center\">"), "{out}");
        assert!(out.contains("<img src=\"rsrepo:/logo.png\" width=\"120\" height=\"80\" alt=\"Logo\">"), "{out}");
        assert!(out.contains("<h1 align=\"center\">Centred title</h1>"), "{out}");
        assert!(out.contains("<p align=\"center\">Centred paragraph"), "{out}");
        assert!(out.contains("<h2 align=\"right\">Right heading</h2>"), "{out}");
        assert!(out.contains("<table align=\"center\">"), "{out}");
        assert!(out.contains("<td align=\"center\">cell</td><th align=\"right\">head</th>"), "{out}");
        assert!(out.contains("<img align=\"right\" width=\"200\" src=\"rsrepo:/side.png\" alt=\"Side\">"), "{out}");
        // bad values dropped: justify, style, class, `100px;evil`; `50%` kept
        assert!(out.contains("<div>bad align value</div>"), "{out}");
        assert!(out.contains("<img alt=\"w\" height=\"50%\" src=\"rsrepo:/a.png\">"), "{out}");
        for bad in ["style=", "class=\"x\"", "justify", "evil"] {
            assert!(!out.contains(bad), "`{bad}` survived: {out}");
        }
        assert_eq!(size_value("100px").as_deref(), Some("100"));
        assert_eq!(size_value(" 40% ").as_deref(), Some("40%"));
        for bad in ["", "%", "-1", "1e3", "123456", "10em", "calc(1)"] {
            assert_eq!(size_value(bad), None, "{bad}");
        }
        assert_eq!(align_value("CENTER").as_deref(), Some("center"));
        assert_eq!(align_value("middle"), None);
    }

    /// F5 (D-97): contributor avatars in release notes become `@user`; other images stay.
    #[test]
    fn release_notes_strip_avatars() {
        let md = "## Contributors\n\n\
            <a href=\"https://github.com/alice\"><img src=\"https://avatars.githubusercontent.com/u/1?v=4\" width=\"32\" alt=\"alice\"></a>\n\
            <a href=\"https://github.com/bob\">\n  <img src=\"https://avatars.githubusercontent.com/u/2?s=64&v=4\" alt=\"x>y\">\n</a>\n\n\
            <img src=\"https://avatars.githubusercontent.com/u/3\" alt=\"@carol\"> and \
            <img src=\"https://github.com/dave.png\" alt=\"\"> and \
            <img src=\"https://avatars.githubusercontent.com/in/29110\" alt=\"dependabot[bot]\"> and \
            <img src=\"https://avatars.githubusercontent.com/u/4\" alt=\"not a user!\">\n\n\
            ![shot](https://example.com/screenshot.png) ![logo](logo.png)\n";
        let out = render_release_notes(md);
        assert!(!out.contains("avatars.githubusercontent.com"), "{out}");
        assert!(!out.contains("dave.png"), "{out}");
        assert!(out.contains("<a href=\"https://github.com/alice\" rel=\"noopener noreferrer\">@alice</a>"), "{out}");
        assert!(out.contains("@bob"), "user from the wrapping link wins over a bad alt: {out}");
        assert!(!out.contains("x&gt;y") && !out.contains("x>y"), "{out}");
        for u in ["carol", "dave", "dependabot[bot]"] {
            assert!(
                out.contains(&format!("<a href=\"https://github.com/{u}\" rel=\"noopener noreferrer\" class=\"user-mention\">@{u}</a>")),
                "{u}: {out}"
            );
        }
        assert!(!out.contains("not a user"), "no user name → avatar dropped: {out}");
        assert!(out.contains("src=\"https://example.com/screenshot.png\""), "{out}");
        assert!(out.contains("src=\"rsrepo:/logo.png\""), "{out}");
        // READMEs are untouched
        assert!(render(md, &root()).html.contains("avatars.githubusercontent.com"));
        assert_eq!(github_profile_user("https://github.com/a/b"), None);
        assert_eq!(github_profile_user("http://github.com/a"), None);
        assert!(!valid_user("-x") && !valid_user("a_b") && !valid_user(&"a".repeat(40)) && valid_user("a-1"));
    }

    #[test]
    fn relative_urls() {
        let base = git_path("docs/sub").unwrap();
        let cases = [
            ("a.md", Some("rsrepo:/docs/sub/a.md")),
            ("./a.md#part", Some("rsrepo:/docs/sub/a.md#part")),
            ("../x/My%20File.md?raw=1", Some("rsrepo:/docs/x/My%20File.md")),
            ("../../README.md", Some("rsrepo:/README.md")),
            ("/src/main.rs", Some("rsrepo:/src/main.rs")),
            ("../../../outside", None),
            ("#usage", Some("#user-content-usage")),
            ("#user-content-x", Some("#user-content-x")),
            ("#", None),
            ("//evil.example.com/x", None),
            ("a\\b", None),
            ("%zz", None),
        ];
        for (u, want) in cases {
            assert_eq!(rewrite(base.as_str(), u).as_deref(), want, "{u}");
        }
        assert_eq!(rewrite("", "a b.png").as_deref(), Some("rsrepo:/a%20b.png"));
        assert_eq!(rewrite("", "..").as_deref(), None);
    }

    #[test]
    fn obfuscated_data_urls_are_dropped() {
        // Url::parse drops tabs/newlines/leading C0 controls, so these reach ammonia's scheme check as `data:`.
        let html = "<p><a href=\"da&#9;ta:text/html,<b>x</b>\">tab</a> <a href=\"&#1;data:text/html,x\">ctl</a> \
                    <a href=\" DATA:text/html,x\">upper</a></p>\
                    <img src=\"da&#10;ta:image/svg+xml,<svg onload=alert(1)>\" alt=\"nl\">\
                    <img src=\"&#9;data:image/png;base64,AAAA\" alt=\"ok\">";
        let out = sanitize(html, &root());
        assert!(!out.contains("href="), "no data link survives: {out}");
        assert!(!out.contains("svg") && !out.contains("text/html") && !out.contains("onload"), "{out}");
        assert!(out.contains("<img alt=\"nl\">"), "svg data src removed: {out}");
        assert!(out.contains("src=\"data:image/png;base64,AAAA\""), "raster kept, normalised: {out}");
    }

    #[test]
    fn headings_and_anchors_match_ids() {
        let md = "# Hi\n\n## Hi\n\n### Usage *now*\n\n[jump](#usage-now) [up](#hi)\n\n<h2 id=\"app\">Raw</h2>\n";
        let r = render(md, &root());
        let anchors: Vec<_> = r.headings.iter().map(|h| (h.level, h.text.as_str(), h.anchor.as_str())).collect();
        assert_eq!(anchors, [(1, "Hi", "user-content-hi"), (2, "Hi", "user-content-hi-1"), (3, "Usage now", "user-content-usage-now")]);
        for h in &r.headings {
            assert!(r.html.contains(&format!("id=\"{}\"", h.anchor)), "{} missing in {}", h.anchor, r.html);
        }
        assert!(r.html.contains("href=\"#user-content-usage-now\""), "{}", r.html);
        // raw-HTML ids are prefixed too
        assert!(r.html.contains("id=\"user-content-app\""), "{}", r.html);
    }

    #[test]
    fn code_classes_and_data_urls() {
        let r = render("```rust\nfn x() {}\n```\n\n```evil onclick\nx\n```\n\n<a href=\"data:image/png;base64,AA\">png link</a>\n\n<img src=\"data:image/gif,xx\">\n<code class=\"language-rust hljs-evil\">c</code>\n", &root());
        assert!(r.html.contains("<code class=\"language-rust\">fn x()"), "{}", r.html);
        assert!(!r.html.contains("language-evil"), "{}", r.html);
        assert!(!r.html.contains("hljs-evil"), "{}", r.html);
        assert!(!r.html.contains("href=\"data:"), "data: only in img: {}", r.html);
        assert!(r.html.contains("src=\"data:image/gif,xx\""), "{}", r.html);
        let r = render("autolink https://example.com and <mailto:me@example.com>\n\n~~gone~~ and footnote[^1]\n\n[^1]: note\n", &root());
        assert!(r.html.contains("href=\"https://example.com\""), "{}", r.html);
        assert!(r.html.contains("href=\"mailto:me@example.com\""), "{}", r.html);
        assert!(r.html.contains("<del>gone</del>"), "{}", r.html);
        assert!(r.html.contains("id=\"user-content-fn-1\"") && r.html.contains("href=\"#user-content-fn-1\""), "{}", r.html);
    }
}
