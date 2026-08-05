//! Pure Markdown <-> HTML conversion for the MCP boundary. Notes are stored as
//! Tiptap HTML; MCP speaks Markdown. Task-list handling mirrors `src/markdown.ts`
//! so notes created via MCP are indistinguishable from app-created ones.

use comrak::{markdown_to_html, Options};
use regex::Regex;
use std::sync::OnceLock;

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
    tiptap_task_lists(&html).trim().to_string()
}

/// Rewrite comrak's GFM checkbox output into Tiptap's task-list structure,
/// mirroring `fixTaskLists` in `src/markdown.ts`. comrak emits, per item:
///   <li><input type="checkbox" disabled="" /> text</li>   (unchecked)
///   <li><input type="checkbox" checked="" disabled="" /> text</li> (checked)
/// A <ul> containing any such <li> becomes data-type="taskList".
fn tiptap_task_lists(html: &str) -> String {
    static ITEM: OnceLock<Regex> = OnceLock::new();
    // Capture optional `checked` and the remaining item body.
    let item = ITEM.get_or_init(|| {
        Regex::new(r#"(?is)<li>\s*<input([^>]*?)type="checkbox"([^>]*?)/?>\s*(.*?)</li>"#).unwrap()
    });
    let mut out = html.to_string();
    // Only bother if there is at least one checkbox item.
    if !out.contains(r#"type="checkbox""#) {
        return out;
    }
    out = item
        .replace_all(&out, |c: &regex::Captures| {
            let attrs = format!("{}{}", &c[1], &c[2]);
            let checked = attrs.contains("checked");
            format!(
                r#"<li data-type="taskItem" data-checked="{}">{}</li>"#,
                if checked { "true" } else { "false" },
                c[3].trim()
            )
        })
        .to_string();
    // Tag any <ul> that now contains a taskItem. comrak groups consecutive task
    // items into one <ul>; mark every <ul> whose first item is a taskItem.
    static LIST: OnceLock<Regex> = OnceLock::new();
    let list =
        LIST.get_or_init(|| Regex::new(r#"(?is)<ul>(\s*<li data-type="taskItem")"#).unwrap());
    out = list
        .replace_all(&out, r#"<ul data-type="taskList">$1"#)
        .to_string();
    out
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

    #[test]
    fn task_list_becomes_tiptap_structure() {
        let h = md_to_html("- [ ] todo\n- [x] done");
        // Tiptap task-list markers present:
        assert!(h.contains(r#"data-type="taskList""#), "got: {h}");
        assert!(
            h.contains(r#"data-type="taskItem" data-checked="false""#),
            "got: {h}"
        );
        assert!(
            h.contains(r#"data-type="taskItem" data-checked="true""#),
            "got: {h}"
        );
        // The raw checkbox <input> must be gone (Tiptap renders its own):
        assert!(!h.contains("<input"), "input not stripped: {h}");
        // Item text survives:
        assert!(h.contains("todo") && h.contains("done"), "got: {h}");
    }

    #[test]
    fn plain_list_is_not_marked_as_tasklist() {
        let h = md_to_html("- a\n- b");
        assert!(!h.contains("taskList"), "plain list wrongly tagged: {h}");
    }
}
