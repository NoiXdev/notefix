use base64::Engine;

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

/// Ersetzt jede `noteimg://localhost/<pfad>`-URL durch eine `data:`-URL; `read`
/// liefert (mime, bytes) für einen Relativpfad oder None (dann unverändert).
pub fn inline_images<F: Fn(&str) -> Option<(String, Vec<u8>)>>(content: &str, read: F) -> String {
    let re = regex::Regex::new(r#"noteimg://localhost/([^"'\s\\)]+)"#).unwrap();
    re.replace_all(content, |c: &regex::Captures| match read(&c[1]) {
        Some((mime, bytes)) => format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(bytes)),
        None => c[0].to_string(),
    }).to_string()
}

/// Ersetzt `noteimg://localhost/<pfad>` durch `images/<pfad>` und sammelt die Pfade.
pub fn to_bundle(content: &str) -> (String, Vec<String>) {
    let re = regex::Regex::new(r#"noteimg://localhost/([^"'\s\\)]+)"#).unwrap();
    let mut paths = Vec::new();
    let new = re.replace_all(content, |c: &regex::Captures| {
        paths.push(c[1].to_string());
        format!("images/{}", &c[1])
    }).to_string();
    (new, paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Note;

    fn note(id: &str) -> Note {
        Note { id: id.into(), content: "<p>x</p>".into(), updated_at: 1, pinned: false, archived: false, color: String::new(), due_at: None, folder_id: None, position: 0, deleted_at: None, dirty: false }
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

    #[test]
    fn inline_images_replaces_with_data_url() {
        let c = "<img src=\"noteimg://localhost/a/b/x.png\">";
        let out = inline_images(c, |rel| { assert_eq!(rel, "a/b/x.png"); Some(("image/png".into(), vec![1,2,3])) });
        assert!(out.contains("data:image/png;base64,"));
        assert!(!out.contains("noteimg://"));
    }

    #[test]
    fn to_bundle_rewrites_and_collects() {
        let (new, paths) = to_bundle("<img src=\"noteimg://localhost/a/b/x.png\">");
        assert!(new.contains("src=\"images/a/b/x.png\""));
        assert_eq!(paths, vec!["a/b/x.png".to_string()]);
    }
}
