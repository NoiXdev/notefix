use crate::storage::Note;

/// Serialize notes to pretty JSON. Empty `ids` => all notes; otherwise only the
/// notes whose id is in `ids` (keeping the input order of `notes`).
pub fn notes_to_json(notes: &[Note], ids: &[String]) -> serde_json::Result<String> {
    let selected: Vec<&Note> = if ids.is_empty() {
        notes.iter().collect()
    } else {
        notes.iter().filter(|n| ids.contains(&n.id)).collect()
    };
    serde_json::to_string_pretty(&selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Note;

    fn note(id: &str) -> Note {
        Note { id: id.into(), content: "<p>x</p>".into(), updated_at: 1, pinned: false, archived: false, color: String::new(), due_at: None, folder_id: None, position: 0 }
    }

    #[test]
    fn empty_ids_exports_all() {
        let notes = vec![note("a"), note("b")];
        let json = notes_to_json(&notes, &[]).unwrap();
        assert!(json.contains("\"id\": \"a\""));
        assert!(json.contains("\"id\": \"b\""));
    }

    #[test]
    fn ids_filter_to_selected() {
        let notes = vec![note("a"), note("b"), note("c")];
        let json = notes_to_json(&notes, &["b".to_string()]).unwrap();
        assert!(json.contains("\"id\": \"b\""));
        assert!(!json.contains("\"id\": \"a\""));
        assert!(!json.contains("\"id\": \"c\""));
    }

    #[test]
    fn uses_camel_case_updated_at() {
        let json = notes_to_json(&[note("a")], &[]).unwrap();
        assert!(json.contains("\"updatedAt\""));
    }
}
