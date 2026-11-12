//! Planned Acts and the deterministic breakpoints derived from them.
//!
//! An Act is planning-schedule metadata on an accepted planning artifact, not a
//! story record (ADR 0006). This module owns the pure helpers around
//! [`PlannedAct`]: the four-step Charge-depth rank used by the advisory
//! progression check, the merge rule that makes accepted Act lists
//! deterministic across planning decisions, and the derivation of pacing-curve
//! `act_breakpoints` from the Act list.

use crate::charge::ChargeDepth;
use crate::planning::PlannedAct;
use std::collections::BTreeMap;

/// Rank of a Charge depth on McKee's four-step spectrum.
///
/// Positive is the shallowest step and the negation of the negation is the
/// deepest, so a strictly increasing sequence of ranks is a story that keeps
/// deepening its Spine Axis. The rank is advisory only; it never decides
/// whether an Act list is accepted.
pub const fn charge_depth_rank(depth: ChargeDepth) -> u8 {
    match depth {
        ChargeDepth::Positive => 0,
        ChargeDepth::Contrary => 1,
        ChargeDepth::Contradictory => 2,
        ChargeDepth::NegationOfTheNegation => 3,
    }
}

/// Merge Act lists by order, with `overlay` overriding an entry of the same
/// order. The result is sorted by `order`.
///
/// Accepted Acts are the merge of every applied proposal's non-empty Act list
/// in decision order. A later decision therefore refines an Act instead of
/// duplicating it, and an Act that a later decision does not mention keeps the
/// earlier entry. This is the precedence rule documented in
/// `docs/planning-engine-design.md`.
pub fn merge_planned_acts(base: &[PlannedAct], overlay: &[PlannedAct]) -> Vec<PlannedAct> {
    let mut by_order: BTreeMap<i32, PlannedAct> =
        base.iter().map(|act| (act.order, act.clone())).collect();
    for act in overlay {
        by_order.insert(act.order, act.clone());
    }
    by_order.into_values().collect()
}

/// Derive a book's pacing-curve `act_breakpoints` from its Act list.
///
/// Keys are `act_<order>`. A value is the Act climax chapter normalized against
/// the book's final position. Spindle does not know a book's exact final chapter
/// count at planning time, so the highest Act-climax chapter in that book is
/// used as the denominator: with climaxes at chapters 2 and 4 the breakpoints
/// are `act_1 = 0.5` and `act_2 = 1.0`. A single Act therefore always yields
/// `1.0`, and every other value stays in `(0.0, 1.0]`: the numerator is a
/// positive chapter, the denominator is at least the numerator, so no clamp or
/// guard is needed.
///
/// Only Acts whose climax placement is in `book_number` contribute. A book with
/// no Act climax (for example a later book of a series whose Acts were planned
/// on another book) yields an empty map instead of borrowing another book's
/// chapter numbers.
///
/// `acts` is expected to be a merged Act list, whose orders are unique (see
/// [`merge_planned_acts`]). Should a caller pass a raw list with a repeated
/// order, the later entry wins, matching the merge rule.
pub fn derive_act_breakpoints(acts: &[PlannedAct], book_number: i32) -> BTreeMap<String, f64> {
    let mut chapters_by_order: BTreeMap<i32, i32> = BTreeMap::new();
    for act in acts {
        let Some(climax) = act.climax.as_ref() else {
            continue;
        };
        if act.order <= 0 || climax.book_number != book_number || climax.chapter_number <= 0 {
            continue;
        }
        chapters_by_order.insert(act.order, climax.chapter_number);
    }
    let Some(highest_chapter) = chapters_by_order.values().copied().max() else {
        return BTreeMap::new();
    };
    chapters_by_order
        .into_iter()
        .map(|(order, chapter)| {
            (
                format!("act_{order}"),
                chapter as f64 / highest_chapter as f64,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charge::{Charge, ChargeSign};
    use crate::planning::PlanningStoryPlacement;

    fn act(order: i32, book: i32, chapter: i32, depth: ChargeDepth, answers: bool) -> PlannedAct {
        PlannedAct {
            order,
            climax: Some(PlanningStoryPlacement {
                book_number: book,
                chapter_number: chapter,
                scene_order: Some(1),
                note: None,
            }),
            charge: Some(Charge::new(ChargeSign::Negative, depth).unwrap()),
            central_question_part: format!("part {order}"),
            answers_central_question: answers,
            cause: None,
        }
    }

    #[test]
    fn charge_depth_ranks_follow_the_four_step_spectrum() {
        assert_eq!(charge_depth_rank(ChargeDepth::Positive), 0);
        assert_eq!(charge_depth_rank(ChargeDepth::Contrary), 1);
        assert_eq!(charge_depth_rank(ChargeDepth::Contradictory), 2);
        assert_eq!(charge_depth_rank(ChargeDepth::NegationOfTheNegation), 3);
    }

    #[test]
    fn breakpoints_normalize_by_the_highest_act_climax_chapter() {
        let acts = vec![
            act(1, 1, 2, ChargeDepth::Contrary, false),
            act(2, 1, 4, ChargeDepth::Contradictory, true),
        ];
        assert_eq!(
            derive_act_breakpoints(&acts, 1),
            BTreeMap::from([("act_1".to_string(), 0.5), ("act_2".to_string(), 1.0)])
        );
    }

    #[test]
    fn a_single_act_always_normalizes_to_one() {
        let acts = vec![act(1, 1, 7, ChargeDepth::Contrary, true)];
        assert_eq!(
            derive_act_breakpoints(&acts, 1),
            BTreeMap::from([("act_1".to_string(), 1.0)])
        );
    }

    #[test]
    fn breakpoints_ignore_other_books_and_invalid_placements() {
        let mut missing_climax = act(3, 1, 2, ChargeDepth::Contradictory, false);
        missing_climax.climax = None;
        let acts = vec![
            act(1, 2, 3, ChargeDepth::Contrary, false),
            act(2, 1, 0, ChargeDepth::Contradictory, true),
            missing_climax,
            act(4, 1, 6, ChargeDepth::NegationOfTheNegation, true),
        ];
        assert_eq!(
            derive_act_breakpoints(&acts, 1),
            BTreeMap::from([("act_4".to_string(), 1.0)])
        );
        assert!(derive_act_breakpoints(&acts, 3).is_empty());
        assert!(derive_act_breakpoints(&[], 1).is_empty());
    }

    #[test]
    fn a_repeated_order_keeps_the_later_entry() {
        let acts = vec![
            act(1, 1, 2, ChargeDepth::Contrary, false),
            act(1, 1, 6, ChargeDepth::Contradictory, true),
        ];
        assert_eq!(
            derive_act_breakpoints(&acts, 1),
            BTreeMap::from([("act_1".to_string(), 1.0)]),
            "an unmerged list follows the same later-wins rule as the merge"
        );
    }

    #[test]
    fn act_cause_is_additive_and_skipped_when_absent() {
        let legacy = serde_json::json!({
            "order": 1,
            "climax": {"book_number": 1, "chapter_number": 1, "scene_order": 2, "note": null},
            "charge": {"sign": "negative", "depth": "contrary"},
            "central_question_part": "part 1",
            "answers_central_question": false
        });
        let act: PlannedAct = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(act.cause, None);
        assert_eq!(
            serde_json::to_value(&act).unwrap(),
            legacy,
            "an Act without a cause must serialize byte-identically"
        );

        let mut with_cause = legacy;
        with_cause["cause"] = serde_json::json!({
            "plot_line_id": {"kind": "proposal_ref", "value": "plot:sub"},
            "sequence_key": "seq:sub"
        });
        let act: PlannedAct = serde_json::from_value(with_cause.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(&act).unwrap(),
            with_cause,
            "the cause reference round-trips"
        );
    }

    #[test]
    fn merge_overrides_by_order_and_keeps_unmentioned_acts() {
        let base = vec![
            act(1, 1, 2, ChargeDepth::Contrary, false),
            act(2, 1, 4, ChargeDepth::Contradictory, true),
        ];
        let overlay = vec![act(2, 1, 5, ChargeDepth::NegationOfTheNegation, true)];
        let merged = merge_planned_acts(&base, &overlay);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].order, 1);
        assert_eq!(merged[1].order, 2);
        assert_eq!(merged[1].climax.as_ref().unwrap().chapter_number, 5);
    }
}
