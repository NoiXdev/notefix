// src-tauri/src/aggregate.rs
//
// C2 combined view: read every context's DB and tag its notes with the context
// they belong to. Read-only aggregation — no writes to app state. Each DB is
// migrated first, because a context not activated since a schema bump would
// otherwise be missing columns that `load_notes` selects.

use crate::storage::{Note, Store};

/// A note paired with the context it lives in (for the combined-view badge).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaggedNote {
    pub context_id: String,
    pub context_label: String,
    pub kind: String,
    pub note: Note,
}

/// A context descriptor (subset of the registry entry) the aggregator reads from.
pub struct Ctx {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub path: String,
}

/// Open + migrate + read every context, tag each note, and sort pinned-first
/// then newest-first. A context whose DB can't be opened/migrated/read is
/// skipped (the others still aggregate).
pub fn aggregate(contexts: &[Ctx]) -> Vec<TaggedNote> {
    let mut out = Vec::new();
    for c in contexts {
        let Ok(store) = Store::open(std::path::Path::new(&c.path)) else { continue };
        if crate::migrate::run_migrations(&store.conn).is_err() { continue; }
        let Ok(notes) = store.load_notes() else { continue };
        for note in notes {
            out.push(TaggedNote {
                context_id: c.id.clone(),
                context_label: c.label.clone(),
                kind: c.kind.clone(),
                note,
            });
        }
    }
    out.sort_by(|a, b| {
        b.note.pinned.cmp(&a.note.pinned).then(b.note.updated_at.cmp(&a.note.updated_at))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(path: &std::path::Path, notes: &[Note]) {
        let s = Store::open(path).unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        for n in notes { s.save_note(n).unwrap(); }
    }

    #[test]
    fn aggregates_and_tags_across_contexts_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("a.db");
        let p2 = dir.path().join("b.db");
        seed(&p1, &[Note { id: "a1".into(), content: "alpha".into(), updated_at: 10, ..Default::default() }]);
        seed(&p2, &[
            Note { id: "b1".into(), content: "beta".into(), updated_at: 30, ..Default::default() },
            Note { id: "b2".into(), content: "pinned".into(), updated_at: 5, pinned: true, ..Default::default() },
        ]);

        let ctxs = vec![
            Ctx { id: "c1".into(), label: "Local".into(), kind: "local".into(), path: p1.to_string_lossy().into() },
            Ctx { id: "c2".into(), label: "srv".into(), kind: "server".into(), path: p2.to_string_lossy().into() },
        ];
        let got = aggregate(&ctxs);

        assert_eq!(got.len(), 3);
        // pinned first, then newest-first: b2 (pinned), b1 (30), a1 (10)
        assert_eq!(got[0].note.id, "b2");
        assert_eq!(got[0].context_id, "c2");
        assert_eq!(got[1].note.id, "b1");
        assert_eq!(got[2].note.id, "a1");
        assert_eq!(got[2].context_id, "c1");
        assert_eq!(got[2].context_label, "Local");
    }

    #[test]
    fn skips_unreadable_context() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.db");
        seed(&good, &[Note { id: "g".into(), content: "x".into(), updated_at: 1, ..Default::default() }]);
        let ctxs = vec![
            Ctx { id: "bad".into(), label: "".into(), kind: "local".into(), path: "/no/such/dir/x.db".into() },
            Ctx { id: "ok".into(), label: "".into(), kind: "local".into(), path: good.to_string_lossy().into() },
        ];
        let got = aggregate(&ctxs);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].note.id, "g");
    }
}
