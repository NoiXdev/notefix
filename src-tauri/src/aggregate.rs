// src-tauri/src/aggregate.rs
//
// C2 combined view: read every context's DB and tag its notes with the context
// they belong to. Read-only aggregation — no writes to app state. Each DB is
// migrated first, because a context not activated since a schema bump would
// otherwise be missing columns that `load_notes` selects.

use crate::storage::{NoteMeta, Store};

/// A context descriptor (subset of the registry entry) the aggregator reads from.
pub struct Ctx {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub path: String,
}

/// A note's list metadata tagged with its context (combined-view, lazy variant).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaggedMeta {
    pub context_id: String,
    pub context_label: String,
    pub kind: String,
    pub note: NoteMeta,
}

/// A search hit tagged with its context (combined-view search).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaggedHit {
    pub context_id: String,
    pub context_label: String,
    pub kind: String,
    pub note: NoteMeta,
    pub snippet: String,
}

/// Like [`aggregate`] but returns list metadata only (no content).
pub fn aggregate_meta(contexts: &[Ctx]) -> Vec<TaggedMeta> {
    let mut out = Vec::new();
    for c in contexts {
        let Ok(store) = Store::open(std::path::Path::new(&c.path)) else {
            continue;
        };
        if crate::migrate::run_migrations(&store.conn).is_err() {
            continue;
        }
        let Ok(notes) = store.load_notes_meta() else {
            continue;
        };
        for note in notes {
            out.push(TaggedMeta {
                context_id: c.id.clone(),
                context_label: c.label.clone(),
                kind: c.kind.clone(),
                note,
            });
        }
    }
    out.sort_by(|a, b| {
        b.note
            .pinned
            .cmp(&a.note.pinned)
            .then(b.note.updated_at.cmp(&a.note.updated_at))
    });
    out
}

/// Search every context and tag each hit with its context.
pub fn search_all(contexts: &[Ctx], query: &str, limit: usize) -> Vec<TaggedHit> {
    let mut out = Vec::new();
    for c in contexts {
        let Ok(store) = Store::open(std::path::Path::new(&c.path)) else {
            continue;
        };
        if crate::migrate::run_migrations(&store.conn).is_err() {
            continue;
        }
        let Ok(hits) = store.search_notes(query, limit) else {
            continue;
        };
        for h in hits {
            out.push(TaggedHit {
                context_id: c.id.clone(),
                context_label: c.label.clone(),
                kind: c.kind.clone(),
                note: h.note,
                snippet: h.snippet,
            });
        }
    }
    out.sort_by(|a, b| {
        b.note
            .pinned
            .cmp(&a.note.pinned)
            .then(b.note.updated_at.cmp(&a.note.updated_at))
    });
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Note;

    fn seed(path: &std::path::Path, notes: &[Note]) {
        let s = Store::open(path).unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        for n in notes {
            s.save_note(n).unwrap();
        }
    }

    #[test]
    fn aggregates_and_tags_across_contexts_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("a.db");
        let p2 = dir.path().join("b.db");
        seed(
            &p1,
            &[Note {
                id: "a1".into(),
                content: "alpha".into(),
                updated_at: 10,
                ..Default::default()
            }],
        );
        seed(
            &p2,
            &[
                Note {
                    id: "b1".into(),
                    content: "beta".into(),
                    updated_at: 30,
                    ..Default::default()
                },
                Note {
                    id: "b2".into(),
                    content: "pinned".into(),
                    updated_at: 5,
                    pinned: true,
                    ..Default::default()
                },
            ],
        );

        let ctxs = vec![
            Ctx {
                id: "c1".into(),
                label: "Local".into(),
                kind: "local".into(),
                path: p1.to_string_lossy().into(),
            },
            Ctx {
                id: "c2".into(),
                label: "srv".into(),
                kind: "server".into(),
                path: p2.to_string_lossy().into(),
            },
        ];
        let got = aggregate_meta(&ctxs);

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
        seed(
            &good,
            &[Note {
                id: "g".into(),
                content: "x".into(),
                updated_at: 1,
                ..Default::default()
            }],
        );
        let ctxs = vec![
            Ctx {
                id: "bad".into(),
                label: "".into(),
                kind: "local".into(),
                path: "/no/such/dir/x.db".into(),
            },
            Ctx {
                id: "ok".into(),
                label: "".into(),
                kind: "local".into(),
                path: good.to_string_lossy().into(),
            },
        ];
        let got = aggregate_meta(&ctxs);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].note.id, "g");
    }

    #[test]
    fn search_all_matches_and_tags() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("a.db");
        seed(
            &p1,
            &[
                Note {
                    id: "a1".into(),
                    content: "<p>apple pie</p>".into(),
                    updated_at: 10,
                    ..Default::default()
                },
                Note {
                    id: "a2".into(),
                    content: "<p>banana bread</p>".into(),
                    updated_at: 20,
                    ..Default::default()
                },
            ],
        );
        let ctxs = vec![Ctx {
            id: "c1".into(),
            label: "L".into(),
            kind: "local".into(),
            path: p1.to_string_lossy().into(),
        }];
        let hits = search_all(&ctxs, "apple", 50);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].note.id, "a1");
        assert_eq!(hits[0].context_id, "c1");
        assert!(hits[0].snippet.to_lowercase().contains("apple"));
    }
}
