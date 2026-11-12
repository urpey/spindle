//! Writer-facing promise visibility (McKee projects).
//!
//! A promise's **nature** — type, description, status, plant placement — tells
//! a writer what the story has promised. Where that promise is **planned to pay
//! off** is author-only: the planned payoff placement, the payoff Turning Beat,
//! the target Act and any notes that can encode them are planning intent, not
//! drafting material. This module owns the single redaction rule every
//! writer-facing promise projection applies, so the rule cannot drift between
//! `get_writer_state`, the scene-context/briefing novel layer, continuity
//! sheets, subject snapshots (`get_entity` / `get_character_snapshot`) and the
//! `bible://projects/{id}/narrative-promises` resource.
//!
//! ## Per-surface semantics
//!
//! | Surface | Payoff placement | Payoff Turning Beat | Target Act | Notes | Unplanted promises |
//! | --- | --- | --- | --- | --- | --- |
//! | `get_scene_context.novel.narrative_promises_due` (and the briefing's copy) | removed | removed | removed | removed | dropped (planted after the cursor) |
//! | `get_writer_state.open_promises_due_now` | removed | not carried by the type | not carried by the type | removed | dropped (planted after the cursor) |
//! | Subject snapshots: `open_promises`, and the promise entity `kind_specific` details | removed | removed | removed | not carried by the type | the snapshot's own placement scope decides which promises are listed |
//! | Continuity sheets (`SubjectSnapshot`s) | removed | removed | removed | not carried by the type | same as subject snapshots |
//! | `bible://{project}/narrative-promises` | field omitted | field omitted | field omitted | field omitted | not filtered (resource lists the branch's promise rows) |
//!
//! Planning surfaces (`planning_status`, canon-delta reads) and write paths keep
//! the full rows: this module is only for McKee writer-facing reads. Non-McKee
//! projects never call it, so their payloads stay byte-identical.

use crate::models::{
    NarrativePromiseDueSummary, StoryPlacement, WriterStateNarrativePromiseSummary,
};
use crate::subject_snapshot::{NarrativePromiseDetails, NarrativePromiseSummary};

/// JSON field names a McKee writer-facing promise row must never carry. The
/// typed projections clear these through the helpers below; the bible
/// resources remove exactly this list (see [`redact_promise_json`]).
pub const WRITER_REDACTED_PROMISE_FIELDS: [&str; 4] = [
    "planned_payoff",
    "planned_payoff_turning_beat",
    "target_act",
    "notes",
];

/// THE redaction rule: clear the payoff location, its Turning Beat, the target
/// Act and the notes that can encode them. Every public helper below funnels
/// here.
fn clear_payoff(
    payoff: &mut Option<StoryPlacement>,
    payoff_turning_beat: Option<&mut Option<i32>>,
    target_act: Option<&mut Option<i32>>,
    notes: Option<&mut Vec<String>>,
) {
    *payoff = None;
    if let Some(slot) = payoff_turning_beat {
        *slot = None;
    }
    if let Some(slot) = target_act {
        *slot = None;
    }
    if let Some(notes) = notes {
        notes.clear();
    }
}

/// Redact one scene-context/briefing due-promise summary.
pub fn redact_due_promise_payoff(promise: &mut NarrativePromiseDueSummary) {
    clear_payoff(
        &mut promise.planned_payoff,
        None,
        None,
        Some(&mut promise.notes),
    );
}

/// Redact one `get_writer_state` due-promise summary. The type carries no
/// Turning Beat or target Act.
pub fn redact_writer_state_promise_payoff(promise: &mut WriterStateNarrativePromiseSummary) {
    clear_payoff(
        &mut promise.planned_payoff,
        None,
        None,
        Some(&mut promise.notes),
    );
}

/// Redact one subject-snapshot open promise.
pub fn redact_subject_promise_payoff(promise: &mut NarrativePromiseSummary) {
    clear_payoff(
        &mut promise.planned_payoff,
        Some(&mut promise.planned_payoff_turning_beat),
        Some(&mut promise.target_act),
        None,
    );
}

/// Redact the promise entity's kind-specific details.
pub fn redact_promise_details_payoff(details: &mut NarrativePromiseDetails) {
    clear_payoff(
        &mut details.planned_payoff,
        Some(&mut details.planned_payoff_turning_beat),
        Some(&mut details.target_act),
        None,
    );
}

/// Remove the writer-redacted fields from one JSON promise row. Used by the
/// bible resources, which serialize promise records directly; the fields are
/// omitted (not blanked) so a reader never mistakes a redacted value for the
/// stored one.
pub fn redact_promise_json(promise: &mut serde_json::Value) {
    if let Some(object) = promise.as_object_mut() {
        for field in WRITER_REDACTED_PROMISE_FIELDS {
            object.remove(field);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::StoryPlacement;

    fn placement(book: i32, chapter: i32, scene: i32) -> StoryPlacement {
        StoryPlacement {
            book_number: book,
            chapter_number: chapter,
            scene_order: Some(scene),
            note: None,
        }
    }

    #[test]
    fn due_promise_keeps_nature_and_loses_payoff_and_notes() {
        let mut promise = NarrativePromiseDueSummary {
            narrative_promise_id: "narrative_promise:bell".into(),
            promise_type: "mystery".into(),
            description: "Who rang the clapperless bell?".into(),
            status: "open".into(),
            planted_at: placement(1, 1, 1),
            planned_payoff: Some(placement(3, 9, 2)),
            urgency: "high".into(),
            chapters_since_plant: 4,
            notes: vec!["payoff at book 3 chapter 9".into()],
        };
        redact_due_promise_payoff(&mut promise);
        assert_eq!(promise.promise_type, "mystery");
        assert_eq!(promise.description, "Who rang the clapperless bell?");
        assert_eq!(promise.status, "open");
        assert_eq!(promise.planted_at, placement(1, 1, 1));
        assert_eq!(promise.planned_payoff, None);
        assert!(promise.notes.is_empty());
    }

    #[test]
    fn writer_state_promise_loses_payoff_and_notes() {
        let mut promise = WriterStateNarrativePromiseSummary {
            narrative_promise_id: "narrative_promise:bell".into(),
            promise_type: "mystery".into(),
            description: "Who rang the clapperless bell?".into(),
            status: "open".into(),
            planted_at: placement(1, 1, 1),
            planned_payoff: Some(placement(3, 9, 2)),
            notes: vec!["book 3 chapter 9".into()],
        };
        redact_writer_state_promise_payoff(&mut promise);
        assert_eq!(promise.description, "Who rang the clapperless bell?");
        assert_eq!(promise.planned_payoff, None);
        assert!(promise.notes.is_empty());
    }

    #[test]
    fn snapshot_and_details_lose_every_payoff_field() {
        let mut summary = NarrativePromiseSummary {
            promise_type: "mystery".into(),
            description: "Who rang the clapperless bell?".into(),
            status: "open".into(),
            planned_payoff: Some(placement(3, 9, 2)),
            planned_payoff_turning_beat: Some(3),
            target_act: Some(2),
            provenance: crate::provenance::Provenance::Chapter {
                chapter_id: crate::provenance::RecordId::new("chapter:1"),
            },
        };
        redact_subject_promise_payoff(&mut summary);
        assert_eq!(summary.planned_payoff, None);
        assert_eq!(summary.planned_payoff_turning_beat, None);
        assert_eq!(summary.target_act, None);
        assert_eq!(summary.description, "Who rang the clapperless bell?");

        let mut details = NarrativePromiseDetails {
            promise_type: "mystery".into(),
            description: "Who rang the clapperless bell?".into(),
            status: "open".into(),
            planned_payoff: Some(placement(3, 9, 2)),
            planned_payoff_turning_beat: Some(3),
            target_act: Some(2),
        };
        redact_promise_details_payoff(&mut details);
        assert_eq!(details.planned_payoff, None);
        assert_eq!(details.planned_payoff_turning_beat, None);
        assert_eq!(details.target_act, None);
        assert_eq!(details.promise_type, "mystery");
    }

    #[test]
    fn json_rows_omit_the_redacted_fields_without_blanking_them() {
        let mut row = serde_json::json!({
            "id": "narrative_promise:bell",
            "promise_type": "mystery",
            "description": "Who rang the clapperless bell?",
            "planted_at": {"book_number": 1, "chapter_number": 1},
            "planned_payoff": {"book_number": 3, "chapter_number": 9},
            "planned_payoff_turning_beat": 3,
            "target_act": 2,
            "notes": ["book 3 chapter 9"],
            "status": "open"
        });
        redact_promise_json(&mut row);
        for field in WRITER_REDACTED_PROMISE_FIELDS {
            assert!(row.get(field).is_none(), "{field} must be omitted");
        }
        assert_eq!(row["promise_type"], "mystery");
        assert_eq!(row["description"], "Who rang the clapperless bell?");
        assert_eq!(row["status"], "open");
        // A non-object value is left alone rather than panicking.
        let mut scalar = serde_json::json!("unchanged");
        redact_promise_json(&mut scalar);
        assert_eq!(scalar, "unchanged");
    }
}
