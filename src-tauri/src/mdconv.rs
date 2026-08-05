//! Pure Markdown <-> HTML conversion for the MCP boundary. Notes are stored as
//! Tiptap HTML; MCP speaks Markdown. Task-list handling mirrors `src/markdown.ts`
//! so notes created via MCP are indistinguishable from app-created ones.

use comrak::nodes::{ListType, NodeValue};
use comrak::{create_formatter, parse_document, Arena, Options};
use std::fmt::Write as _;

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

/// Custom `<li>` handler, installed on top of `htmd`'s built-in one, that
/// renders a Tiptap task item (`<li data-type="taskItem"
/// data-checked="true|false">`) as a GFM checkbox item (`- [x]`/`- [ ]`)
/// instead of a plain bullet.
///
/// `htmd` 0.5.5 has no element handler for `<input>` (a task item's checkbox
/// never survives conversion) and its text escaping rewrites a literal
/// `[ ]`/`[x]` appearing in text content into `\[ \]`/`\[x\]` (its rule for
/// anything that could be mistaken for Markdown link syntax). So the marker
/// can't be produced by pre-editing the HTML or the item's text before
/// handing it to `htmd` (an earlier version of this function tried exactly
/// that, via a regex pre-pass and placeholder tokens) -- it has to be
/// written by this handler itself, directly into the converted output,
/// after `htmd`'s own text escaping has already run on the item's children.
///
/// Registering this via `HtmlToMarkdownBuilder::add_handler` makes `htmd`
/// call it once per `<li>` as it walks the *parsed DOM tree*, the same way
/// it calls its own built-in list-item handler. That is what makes nesting
/// depth a non-issue here, unlike a regex over raw HTML text: a task item
/// nested inside another task item's `<ul>` gets its own independent call to
/// this function, so a nested checked/unchecked state can never be
/// swallowed as opaque text by a lazy match on an ancestor's closing `</li>`.
/// A plain `<li>` (no `data-checked`) falls back to `htmd`'s built-in
/// handler unchanged, via `Handlers::fallback`.
///
/// Inverse of `TiptapFormatter`'s `NodeValue::TaskItem`/`NodeValue::List`
/// handling in `md_to_html`.
fn task_item_handler(
    handlers: &dyn htmd::element_handler::Handlers,
    element: htmd::Element,
) -> Option<htmd::element_handler::HandlerResult> {
    let checked = element
        .attrs
        .iter()
        .find(|attr| &attr.name.local == "data-checked")
        .map(|attr| attr.value.to_string());
    let mark = match checked.as_deref() {
        Some("true") => "x",
        Some("false") => " ",
        // Not a Tiptap task item (or an attribute value it never actually
        // emits): defer to htmd's own <li> handler.
        _ => return handlers.fallback(element),
    };

    let bullet = match handlers.options().bullet_list_marker {
        htmd::options::BulletListMarker::Asterisk => '*',
        htmd::options::BulletListMarker::Dash => '-',
    };
    let spacing = " ".repeat(handlers.options().ul_bullet_spacing as usize);
    let prefix = format!("{bullet}{spacing}[{mark}] ");

    let content = handlers.walk_children(element.node).content;
    let content = content.trim_start_matches('\n');
    // Indent any continuation lines (e.g. a nested list under this item) so
    // they line up under the item's own text, matching htmd's plain bullet
    // list-item indentation.
    let indent = " ".repeat(prefix.chars().count());
    let indented = content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 || line.is_empty() {
                line.to_string()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Some(format!("\n{prefix}{indented}").into())
}

/// Tiptap HTML -> GFM Markdown. Inverse of `md_to_html`: headings, emphasis,
/// lists, code, links and tables go through `htmd`'s built-in handlers; task
/// items, at any nesting depth, go through `task_item_handler` above so they
/// land back on `- [ ]`/`- [x]`.
// Not yet called outside tests; wired up by a later task's MCP command.
#[allow(dead_code)]
pub fn html_to_md(html: &str) -> String {
    // Dash bullets with single-space spacing match GFM (and `md_to_html`'s
    // own comrak output), so a note round-tripped through both directions
    // keeps the same list style.
    let converter = htmd::HtmlToMarkdown::builder()
        .options(htmd::options::Options {
            bullet_list_marker: htmd::options::BulletListMarker::Dash,
            ul_bullet_spacing: 1,
            ..Default::default()
        })
        .add_handler(vec!["li"], task_item_handler)
        .build();
    converter
        .convert(html)
        .unwrap_or_default()
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
    fn html_to_md_nested_task_items() {
        // The exact HTML `md_to_html` produces for "- [ ] a\n  - [x] nested\n- [x] b":
        // an outer taskList <ul> whose first <li> is itself an unchecked task item
        // containing a nested taskList <ul> with one checked task item, followed by
        // a sibling checked task item.
        let html = concat!(
            r#"<ul data-type="taskList">"#,
            "\n",
            r#"<li data-type="taskItem" data-checked="false">a"#,
            "\n",
            r#"<ul data-type="taskList">"#,
            "\n",
            r#"<li data-type="taskItem" data-checked="true">nested</li>"#,
            "\n",
            r#"</ul>"#,
            "\n",
            r#"</li>"#,
            "\n",
            r#"<li data-type="taskItem" data-checked="true">b</li>"#,
            "\n",
            r#"</ul>"#,
        );
        let m = html_to_md(html);
        assert!(m.contains("- [ ] a"), "outer unchecked item missing: {m}");
        assert!(m.contains("- [x] b"), "sibling checked item missing: {m}");
        // The nested item must keep its checked marker (not become a plain bullet)
        // and stay indented under its parent.
        assert!(
            m.contains("  - [x] nested") || m.contains("    - [x] nested"),
            "nested checked item lost its marker or indentation: {m}"
        );
        assert!(
            !m.contains("- nested"),
            "nested item became a plain bullet, checked state was lost: {m}"
        );
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
