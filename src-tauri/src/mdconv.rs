//! Pure Markdown <-> HTML conversion for the MCP boundary. Notes are stored as
//! Tiptap HTML; MCP speaks Markdown. Task-list handling mirrors `src/markdown.ts`
//! so notes created via MCP are indistinguishable from app-created ones.

use comrak::{markdown_to_html, Options};

/// Markdown (GFM) -> HTML. Hard line breaks on (matches the frontend's
/// `marked` `breaks: true`), tables/strikethrough/autolinks/task items enabled.
// Consumed by the MCP note-conversion commands added in later tasks of this overhaul.
#[allow(dead_code)]
pub fn md_to_html(md: &str) -> String {
    let mut opts = Options::default();
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.tasklist = true;
    opts.extension.autolink = true;
    opts.render.hardbreaks = true;
    let html = markdown_to_html(md, &opts);
    html.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_and_emphasis() {
        let h = md_to_html("# Title\n\nsome **bold** and *italic*");
        assert!(h.contains("<h1>Title</h1>"), "got: {h}");
        assert!(h.contains("<strong>bold</strong>"), "got: {h}");
        assert!(h.contains("<em>italic</em>"), "got: {h}");
    }

    #[test]
    fn bullet_and_ordered_lists() {
        let h = md_to_html("- a\n- b");
        assert!(h.contains("<ul>") && h.contains("<li>a</li>"), "got: {h}");
        let o = md_to_html("1. one\n2. two");
        assert!(o.contains("<ol>") && o.contains("<li>one</li>"), "got: {o}");
    }

    #[test]
    fn code_and_tables() {
        let c = md_to_html("`inline` and\n\n```\nblock\n```");
        assert!(c.contains("<code>inline</code>"), "got: {c}");
        assert!(c.contains("<pre>"), "got: {c}");
        let t = md_to_html("| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(
            t.contains("<table>") && t.contains("<td>1</td>"),
            "got: {t}"
        );
    }
}
