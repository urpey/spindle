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

/// The maximum `sequence_key` length, matching proposal output refs.
pub const SEQUENCE_KEY_MAX_LEN: usize = 128;

/// Whether `key` is usable as a stable Sequence identity and a
/// JSON-pointer-ish finding locator segment.
///
/// A key is non-empty, at most [`SEQUENCE_KEY_MAX_LEN`] bytes, and contains
/// only ASCII alphanumerics plus `-`, `_` and `:` (the same shape proposal
/// output refs use). Whitespace, `/` and `~` are rejected so the key can be
/// embedded in a locator such as `/merged_sequences/{key}/climax_scene`.
pub fn sequence_key_is_valid(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= SEQUENCE_KEY_MAX_LEN
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_:".contains(&byte))
}

/// Merge Sequence lists by `sequence_key`: a later entry **refines** a Sequence
/// instead of replacing it. The result is sorted by `sequence_key`.
///
/// The key is **global**, not scoped to one plot line: the merged set is keyed
/// by it alone. A later entry that reuses a key while changing `plot_line_id` is
/// therefore a data error, reported by the planning validator as
/// `sequence_plot_line_changed`; this pure merge still applies the overlay so
/// the rest of the evaluation sees the proposed data and can report the defect.
///
/// For the same key the overlay overrides the scalar fields (`plot_line_id`,
/// `question`, `inciting_scene`, `climax_scene`) and **unions**
/// `member_scenes` — members the base already lists survive, new members are
/// added, duplicates (same story position) collapse, and the union is kept in
/// strict story order. A Sequence the overlay does not mention keeps the base
/// entry unchanged.
///
/// Accepted Sequences are the merge of every applied Chapter proposal's
/// non-empty Sequence list in decision order, then the staged ancestors in the
/// same approved closure, then the current artifact. A later Chapter artifact
/// therefore extends a Sequence another chapter opened. This is the precedence
/// rule documented in `docs/planning-engine-design.md`.
pub fn merge_planned_sequences(
    base: &[PlannedSequence],
    overlay: &[PlannedSequence],
) -> Vec<PlannedSequence> {
    let mut by_key: BTreeMap<String, PlannedSequence> = base
        .iter()
        .map(|sequence| (sequence.sequence_key.clone(), sequence.clone()))
        .collect();
    for sequence in overlay {
        match by_key.entry(sequence.sequence_key.clone()) {
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let existing = entry.get_mut();
                existing.plot_line_id = sequence.plot_line_id.clone();
                existing.question = sequence.question.clone();
                existing.inciting_scene = sequence.inciting_scene.clone();
                existing.climax_scene = sequence.climax_scene.clone();
                for member in &sequence.member_scenes {
                    if !existing
                        .member_scenes
                        .iter()
                        .any(|known| same_scene_position(known, member))
                    {
                        existing.member_scenes.push(member.clone());
                    }
                }
                // Refinement may add a member anywhere in the Sequence; keep the
                // union in story order so the structural rule still reads it.
                existing
                    .member_scenes
                    .sort_by(|left, right| left.cmp_story_order(right));
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(sequence.clone());
            }
        }
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
    fn sequence_keys_must_be_pointer_safe() {
        for key in ["seq:opening", "seq_2", "opening", "a-b"] {
            assert!(sequence_key_is_valid(key), "{key} should be valid");
        }
        for key in [
            "",
            "seq opening",
            " seq:opening",
            "seq:opening ",
            "seq/opening",
            "seq~opening",
            "seq.opening",
        ] {
            assert!(!sequence_key_is_valid(key), "{key:?} should be invalid");
        }
        assert!(!sequence_key_is_valid(
            &"a".repeat(SEQUENCE_KEY_MAX_LEN + 1)
        ));
        assert!(sequence_key_is_valid(&"a".repeat(SEQUENCE_KEY_MAX_LEN)));
    }

    #[test]
    fn merge_refines_by_key_and_keeps_unmentioned_sequences() {
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
    fn merge_unions_members_and_keeps_earlier_ones_in_story_order() {
        let base = vec![sequence("seq:ledger", vec![place(1, 1, 1), place(1, 1, 2)])];
        // The later artifact adds a member between the existing two and repeats
        // one it already knows; the union is deduped and sorted in story order.
        let mut overlay = sequence(
            "seq:ledger",
            vec![place(1, 2, 1), place(1, 1, 2), place(1, 3, 1)],
        );
        overlay.question = "Who forged the ledger?".into();
        let merged = merge_planned_sequences(&base, &[overlay]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].question, "Who forged the ledger?");
        assert_eq!(
            merged[0]
                .member_scenes
                .iter()
                .map(|member| (
                    member.book_number,
                    member.chapter_number,
                    member.scene_order.unwrap()
                ))
                .collect::<Vec<_>>(),
            vec![(1, 1, 1), (1, 1, 2), (1, 2, 1), (1, 3, 1)]
        );
        assert!(members_strictly_ordered(&merged[0].member_scenes));
        // The overlay's climax wins, and members the base listed survive.
        assert_eq!(merged[0].climax_scene, Some(place(1, 3, 1)));
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
