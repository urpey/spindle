//! Planned Sequences: the chapter-level grouping of planned Scenes under one
//! plot line.
//!
//! A Sequence is planning-schedule metadata on an accepted Chapter artifact, not
//! a story record (ADR 0006). It may cross chapters and books: its
//! `member_scenes` are planned Scene placements in story order, and its
//! `climax_scene` is one of those members. This module owns the pure helpers
//! around [`PlannedSequence`]: the deterministic merge by `sequence_key` and the
//! placement comparisons the structural validator needs.

use crate::planning::{PlannedSequence, PlanningStoryPlacement};
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// Merge Sequence lists by `sequence_key`, with `overlay` overriding an entry
/// with the same key. The result is sorted by `sequence_key`.
///
/// Accepted Sequences are the merge of every applied Chapter proposal's
/// non-empty Sequence list in decision order, then the staged ancestors in the
/// same approved closure, then the current artifact. A later entry therefore
/// refines a Sequence (adds members, moves the climax) instead of duplicating
/// it, and a Sequence the later entry does not mention keeps the earlier one.
/// This is the precedence rule documented in `docs/planning-engine-design.md`.
pub fn merge_planned_sequences(
    base: &[PlannedSequence],
    overlay: &[PlannedSequence],
) -> Vec<PlannedSequence> {
    let mut by_key: BTreeMap<String, PlannedSequence> = base
        .iter()
        .map(|sequence| (sequence.sequence_key.clone(), sequence.clone()))
        .collect();
    for sequence in overlay {
        by_key.insert(sequence.sequence_key.clone(), sequence.clone());
    }
    by_key.into_values().collect()
}

/// Whether two placements name the same Scene position (the note is ignored).
pub fn same_scene_position(left: &PlanningStoryPlacement, right: &PlanningStoryPlacement) -> bool {
    left.book_number == right.book_number
        && left.chapter_number == right.chapter_number
        && left.scene_order == right.scene_order
}

/// Whether `members` is strictly ordered by `(book_number, chapter_number,
/// scene_order)`. Members without a `scene_order` are never strictly ordered:
/// a Sequence names exact planned Scenes.
pub fn members_strictly_ordered(members: &[PlanningStoryPlacement]) -> bool {
    members.iter().all(|member| member.scene_order.is_some())
        && members.windows(2).all(|window| {
            window[0].cmp_story_order(&window[1]) == Ordering::Less
                && !same_scene_position(&window[0], &window[1])
        })
}

/// Whether `scene` belongs to `members` by exact story placement.
pub fn placement_is_member(
    scene: &PlanningStoryPlacement,
    members: &[PlanningStoryPlacement],
) -> bool {
    members
        .iter()
        .any(|member| same_scene_position(member, scene))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planning::PlannedSequence;

    fn place(book: i32, chapter: i32, scene: i32) -> PlanningStoryPlacement {
        PlanningStoryPlacement {
            book_number: book,
            chapter_number: chapter,
            scene_order: Some(scene),
            note: None,
        }
    }

    fn sequence(key: &str, members: Vec<PlanningStoryPlacement>) -> PlannedSequence {
        PlannedSequence {
            sequence_key: key.into(),
            plot_line_id: Some(crate::planning::PlanningRef::ProposalRef(
                "plot:main".into(),
            )),
            question: "Who holds the harbor?".into(),
            inciting_scene: members.first().cloned(),
            climax_scene: members.last().cloned(),
            member_scenes: members,
        }
    }

    #[test]
    fn merge_overrides_by_key_and_keeps_unmentioned_sequences() {
        let base = vec![
            sequence("seq:opening", vec![place(1, 1, 1), place(1, 1, 2)]),
            sequence("seq:close", vec![place(1, 2, 1)]),
        ];
        let overlay = vec![sequence("seq:close", vec![place(1, 2, 1), place(1, 3, 1)])];
        let merged = merge_planned_sequences(&base, &overlay);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].sequence_key, "seq:close");
        assert_eq!(merged[0].member_scenes.len(), 2);
        assert_eq!(merged[1].sequence_key, "seq:opening");
    }

    #[test]
    fn members_must_be_in_strict_story_order() {
        assert!(members_strictly_ordered(&[
            place(1, 1, 1),
            place(1, 1, 2),
            place(1, 2, 1),
            place(2, 1, 1)
        ]));
        assert!(!members_strictly_ordered(&[place(1, 1, 2), place(1, 1, 1)]));
        assert!(!members_strictly_ordered(&[place(1, 1, 1), place(1, 1, 1)]));
        let mut missing = place(1, 1, 1);
        missing.scene_order = None;
        assert!(!members_strictly_ordered(&[missing, place(1, 1, 2)]));
    }

    #[test]
    fn membership_compares_exact_positions_ignoring_the_note() {
        let mut noted = place(1, 2, 1);
        noted.note = Some("climax".into());
        assert!(placement_is_member(
            &noted,
            &[place(1, 1, 1), place(1, 2, 1)]
        ));
        assert!(!placement_is_member(&place(1, 2, 2), &[place(1, 1, 1)]));
        assert!(same_scene_position(&place(1, 2, 1), &noted));
    }
}
