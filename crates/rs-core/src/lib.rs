//! RepoShelf core (no Tauri). M0: only the markdown smoke pipeline.

/// Smoke check for the D-4 pipeline: comrak (GFM) → ammonia allow-list.
pub fn render_smoke(md: &str) -> String {
    let mut opts = comrak::Options::default();
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.tasklist = true;
    opts.extension.autolink = true;
    // Raw HTML passes comrak so ammonia (not comrak's escaping) is the one sanitiser.
    opts.render.r#unsafe = true;
    let html = comrak::markdown_to_html(md, &opts);
    ammonia::clean(&html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_markdown_strips_script() {
        let out = render_smoke("# Title\n\n<script>alert(1)</script>\n\n| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(out.contains("<h1>"), "{out}");
        assert!(out.contains("<table>"), "{out}");
        assert!(!out.contains("<script"), "{out}");
        assert!(!out.contains("alert(1)"), "{out}");
    }
}
