use serde::Serialize;

use crate::storage::Note;

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub notes: i64,
    pub archived: i64,
    pub characters: i64,
    pub words: i64,
}

/// Strip HTML tags to plain text (inserting a space at tag boundaries).
pub fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => { in_tag = true; if !out.is_empty() && !out.ends_with(' ') { out.push(' '); } }
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

pub fn compute(notes: &[Note]) -> Stats {
    let mut characters = 0i64;
    let mut words = 0i64;
    let mut archived = 0i64;
    for n in notes {
        let text = strip_html(&n.content);
        characters += text.chars().count() as i64;
        words += text.split_whitespace().count() as i64;
        if n.archived {
            archived += 1;
        }
    }
    Stats { notes: notes.len() as i64, archived, characters, words }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Note;

    fn note(content: &str, archived: bool) -> Note {
        Note { id: "x".into(), content: content.into(), updated_at: 1, pinned: false, archived, color: String::new(), due_at: None, folder_id: None, position: 0 }
    }

    #[test]
    fn strip_html_removes_tags_and_separates() {
        assert_eq!(strip_html("<h1>Hallo</h1><p>Welt</p>"), "Hallo Welt");
    }

    #[test]
    fn compute_counts_notes_archived_chars_words() {
        let notes = vec![note("<p>eins zwei</p>", false), note("<p>drei</p>", true)];
        let s = compute(&notes);
        assert_eq!(s.notes, 2);
        assert_eq!(s.archived, 1);
        assert_eq!(s.words, 3); // "eins zwei" + "drei"
        assert_eq!(s.characters, "eins zwei".chars().count() as i64 + "drei".chars().count() as i64);
    }

    #[test]
    fn compute_empty_is_zero() {
        assert_eq!(compute(&[]), Stats { notes: 0, archived: 0, characters: 0, words: 0 });
    }
}
