//! Pure Markdown <-> HTML conversion for the MCP boundary. Notes are stored as
//! Tiptap HTML; MCP speaks Markdown. Task-list handling mirrors `src/markdown.ts`
//! so notes created via MCP are indistinguishable from app-created ones.

use comrak::nodes::{ListType, NodeValue};
use comrak::{create_formatter, parse_document, Arena, Options};
use regex::Regex;
use std::fmt::Write as _;
use std::sync::OnceLock;

// A regex/string post-process over comrak's rendered HTML cannot robustly
// tell a task item's closing </li> apart from a *nested* task item's closing
// </li> (a lazy `(.*?)</li>` match stops at the first one it finds, which is
// the inner list's, swallowing the inner <ul> and leaving its raw <input>
// unstripped). Overriding comrak's AST->HTML rendering for the two node
// types involved sidesteps that entirely: `NodeValue::TaskItem` and
// `NodeValue::List` are distinct, separately-typed nodes in the tree (a
// nested task list is just another `List`/`TaskItem` subtree), so this
// formatter's rules apply independently and correctly at every nesting
// depth. It also reuses the exact "is this list a task list" answer comrak's
// own parser already computed (`NodeList::is_task_list`), which is set as
// soon as *any* item in a list is a task item — precisely the Tiptap rule
// that a mixed list's `<ul>` must carry `data-type="taskList"` while a plain
// item inside it stays a plain `<li>`.
create_formatter!(TiptapFormatter, {
    // comrak renders a GFM task item (`- [ ] x` / `- [x] x`) as
    // `<li><input type="checkbox" .../> x</li>`. Tiptap instead expects
    // `<li data-type="taskItem" data-checked="true|false">x</li>` with no
    // `<input>` (Tiptap's NodeView renders its own checkbox), mirroring
    // `fixTaskLists` in `src/markdown.ts`.
    NodeValue::TaskItem(ref nti) => |context, entering| {
        if entering {
            context.cr()?;
            context.write_str(if nti.symbol.is_some() {
                r#"<li data-type="taskItem" data-checked="true">"#
            } else {
                r#"<li data-type="taskItem" data-checked="false">"#
            })?;
        } else {
            context.write_str("</li>")?;
            context.lf()?;
        }
    },
    NodeValue::List(ref nl) => |context, entering| {
        if entering {
            context.cr()?;
            match nl.list_type {
                ListType::Bullet => {
                    context.write_str("<ul")?;
                    if nl.is_task_list {
                        context.write_str(r#" data-type="taskList""#)?;
                    }
                    context.write_str(">")?;
                }
                ListType::Ordered => {
                    context.write_str("<ol")?;
                    if nl.is_task_list {
                        context.write_str(r#" data-type="taskList""#)?;
                    }
                    if nl.start == 1 {
                        context.write_str(">")?;
                    } else {
                        write!(context, " start=\"{}\">", nl.start)?;
                    }
                }
            }
            context.lf()?;
        } else if nl.list_type == ListType::Bullet {
            context.write_str("</ul>")?;
            context.lf()?;
        } else {
            context.write_str("</ol>")?;
            context.lf()?;
        }
    },
});

/// Markdown (GFM) -> HTML. Hard line breaks on (matches the frontend's
/// `marked` `breaks: true`), tables/strikethrough/autolinks/task items
/// enabled. Task-list items are rendered straight from comrak's AST into
/// Tiptap's `data-type="taskList"` / `data-type="taskItem"` structure via
/// `TiptapFormatter` above, so homogeneous, mixed, and arbitrarily nested
/// task lists all convert correctly.
// Consumed by the MCP note-conversion commands added in later tasks of this overhaul.
#[allow(dead_code)]
pub fn md_to_html(md: &str) -> String {
    let mut opts = Options::default();
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.tasklist = true;
    opts.extension.autolink = true;
    opts.render.hardbreaks = true;

    let arena = Arena::new();
    let root = parse_document(&arena, md, &opts);
    let mut html = String::new();
    TiptapFormatter::format_document(root, &opts, &mut html)
        .expect("formatting comrak's AST to a String is infallible");
    html.trim().to_string()
}

// Placeholder tokens standing in for the literal GFM checkbox markers while
// the document passes through `htmd`. `htmd` 0.5.5 has no element handler for
// `<input>` (a Tiptap task item's checkbox never survives conversion) and its
// text escaping rewrites a literal `[ ]`/`[x]` in text content into
// `\[ \]`/`\[x\]` (its rule for anything that looks like Markdown link
// syntax), so neither an `<input type="checkbox">` nor bare `[ ]`/`[x]` text
// makes it through unchanged. These all-uppercase, punctuation-free tokens
// trip none of `htmd`'s escaping rules, so they pass through `htmd::convert`
// byte-for-byte and can be swapped back for the real markers afterwards.
const TASK_UNCHECKED_TOKEN: &str = "NOTEFIXTASKUNCHECKEDMARKER";
const TASK_CHECKED_TOKEN: &str = "NOTEFIXTASKCHECKEDMARKER";

/// Rewrite Tiptap task items (`<li data-type="taskItem"
/// data-checked="true|false">`) into plain `<li>`s prefixed with a checkbox
/// placeholder token, and their enclosing `<ul data-type="taskList">` into a
/// plain `<ul>`, so the generic HTML->Markdown pass treats them as an
/// ordinary bullet list. `html_to_md` swaps the tokens for literal `[ ]`/
/// `[x]` markers once `htmd` has rendered the surrounding `- ` bullet.
/// Inverse of `TiptapFormatter`'s `NodeValue::TaskItem`/`NodeValue::List`
/// handling in `md_to_html`.
fn pre_tasks(html: &str) -> String {
    static LI_RE: OnceLock<Regex> = OnceLock::new();
    static CHECKED_RE: OnceLock<Regex> = OnceLock::new();
    let li_re = LI_RE.get_or_init(|| Regex::new(r#"(?is)<li\s+([^>]*)>(.*?)</li>"#).unwrap());
    let checked_re =
        CHECKED_RE.get_or_init(|| Regex::new(r#"data-checked="(true|false)""#).unwrap());

    let rewritten = li_re.replace_all(html, |c: &regex::Captures| {
        let attrs = &c[1];
        let body = &c[2];
        if attrs.contains(r#"data-type="taskItem""#) {
            if let Some(checked) = checked_re.captures(attrs) {
                let token = if &checked[1] == "true" {
                    TASK_CHECKED_TOKEN
                } else {
                    TASK_UNCHECKED_TOKEN
                };
                return format!("<li>{token} {body}</li>");
            }
        }
        c[0].to_string()
    });

    rewritten
        .replace(r#"<ul data-type="taskList">"#, "<ul>")
        .to_string()
}

/// Tiptap HTML -> GFM Markdown. Inverse of `md_to_html`: headings, emphasis,
/// lists, code, links and tables go through `htmd`; task items round-trip
/// via `pre_tasks`'s placeholder tokens so they land back on `- [ ]`/`- [x]`.
// Not yet called outside tests; wired up by a later task's MCP command.
#[allow(dead_code)]
pub fn html_to_md(html: &str) -> String {
    let prepared = pre_tasks(html);
    // Dash bullets with single-space spacing match GFM (and `md_to_html`'s
    // own comrak output), so a note round-tripped through both directions
    // keeps the same list style.
    let converter = htmd::HtmlToMarkdown::builder()
        .options(htmd::options::Options {
            bullet_list_marker: htmd::options::BulletListMarker::Dash,
            ul_bullet_spacing: 1,
            ..Default::default()
        })
        .build();
    converter
        .convert(&prepared)
        .unwrap_or_default()
        .replace(TASK_UNCHECKED_TOKEN, "[ ]")
        .replace(TASK_CHECKED_TOKEN, "[x]")
        .trim()
        .to_string()
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

    #[test]
    fn task_list_mixed_items() {
        let h = md_to_html("- a\n- [ ] b\n- [x] c");
        assert!(h.contains(r#"data-type="taskList""#), "got: {h}");
        assert!(
            h.contains(r#"data-type="taskItem" data-checked="false""#),
            "got: {h}"
        );
        assert!(
            h.contains(r#"data-type="taskItem" data-checked="true""#),
            "got: {h}"
        );
        // The plain item "a" must stay a plain <li>, not become a taskItem.
        assert!(h.contains("<li>a</li>"), "got: {h}");
        assert!(!h.contains("<input"), "input not stripped: {h}");
    }

    #[test]
    fn html_to_md_basic() {
        let m = html_to_md("<h1>Title</h1><p>a <strong>b</strong></p>");
        assert!(m.contains("# Title"), "got: {m}");
        assert!(m.contains("**b**"), "got: {m}");
    }

    #[test]
    fn html_to_md_task_items() {
        let html = r#"<ul data-type="taskList"><li data-type="taskItem" data-checked="false">todo</li><li data-type="taskItem" data-checked="true">done</li></ul>"#;
        let m = html_to_md(html);
        assert!(m.contains("- [ ] todo"), "got: {m}");
        assert!(m.contains("- [x] done"), "got: {m}");
    }

    #[test]
    fn task_list_nested() {
        let h = md_to_html("- [ ] a\n  - [x] nested\n- [x] b");
        assert!(!h.contains("<input"), "input not stripped: {h}");
        // Both the outer and the nested <ul> must be tagged as taskList.
        let task_list_count = h.matches(r#"data-type="taskList""#).count();
        assert!(task_list_count >= 2, "got: {h}");
        assert!(
            h.contains(r#"data-type="taskItem" data-checked="true""#),
            "got: {h}"
        );
        assert!(h.contains("nested"), "got: {h}");
    }
}
