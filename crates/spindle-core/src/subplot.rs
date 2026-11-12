//! Subplot structural rules over planned Sequences and Convergence Points.
//!
//! A **Subplot** is any plot line that is not the Main Plot. Its Sequences must
//! feed the Main Plot at a **Convergence Point**, and its Value Axis must keep
//! working: it must turn, it must turn in a direction its structural job claims,
//! and it must not turn deeper than the Main Plot inside the same Act. This
//! module owns the pure definitions so the planning validator and the docs
//! cannot drift:
//!
//! * [`climax_within_convergence_points`] — the blocking convergence rule;
//! * [`DORMANT_CHAPTER_RUN`] / [`chapters_are_consecutive`] — the dormancy run;
//! * [`job_requires_closing_sign`] / [`closing_supports_job`] — the documented
//!   job-support heuristic;
//! * [`act_story_ranges`] / [`act_range_for_scene`] — the Act story-range rule;
//! * [`deepest_charge_rank`] — the deeper-than-Main-Plot comparison.
//!
//! Only the convergence rule blocks acceptance; the rest are advisories that
//! surface a structural smell without stopping a deliberate choice.

use crate::charge::{Charge, ChargeSign};
use crate::planning::{PlannedAct, PlanningStoryPlacement};
use crate::plot_line::SubplotJob;
use std::cmp::Ordering;

/// How many Chapters a Subplot's Value Axis may go unturned before the planning
/// validator warns. A reader forgets a line that stops moving (GLOSSARY:
/// Subplot).
pub const DORMANT_CHAPTER_RUN: usize = 3;

/// Whether `climax` lands at or before at least one Convergence Point.
///
/// Story order is `(book_number, chapter_number, scene_order)` and the
/// comparison is inclusive: a climax placed exactly on a Convergence Point
/// satisfies it. A chapter-wide point (`scene_order: None`) compares equal
/// within its chapter, so it accepts any Scene in that chapter.
pub fn climax_within_convergence_points(
    climax: &PlanningStoryPlacement,
    convergence_points: &[PlanningStoryPlacement],
) -> bool {
    convergence_points
        .iter()
        .any(|point| climax.cmp_story_order(point) != Ordering::Greater)
}

/// Whether `next` is the Chapter immediately after `previous` in the same book.
///
/// The dormancy run is scanned over the Chapters the planned range actually
/// carries; a gap in the plan (chapter 3 planned, chapter 2 not) breaks the run
/// rather than silently counting two non-adjacent chapters as consecutive.
pub fn chapters_are_consecutive(previous: (i32, i32), next: (i32, i32)) -> bool {
    previous.0 == next.0 && next.1 == previous.1 + 1
}

/// The closing-Charge sign a Subplot's structural job claims.
///
/// The documented job-support heuristic is deliberately weak and directional:
///
/// * `contradict` argues against the Controlling Idea, so at least one turn on
///   the Subplot's Value Axis must end at a **negative** Charge;
/// * `resonate` argues with it, so at least one turn must end at a **positive**
///   Charge;
/// * `set_up` and `complicate` make no claim about direction, so they impose no
///   requirement at all.
///
/// The heuristic asks only that one turn side with the job. It does not demand
/// that every turn do so, and it says nothing about craft quality.
pub fn job_requires_closing_sign(job: SubplotJob) -> Option<ChargeSign> {
    match job {
        SubplotJob::Contradict => Some(ChargeSign::Negative),
        SubplotJob::Resonate => Some(ChargeSign::Positive),
        SubplotJob::SetUp | SubplotJob::Complicate => None,
    }
}

/// Whether one closing Charge supports the Subplot's job under
/// [`job_requires_closing_sign`].
pub fn closing_supports_job(job: SubplotJob, closing: Charge) -> bool {
    job_requires_closing_sign(job).is_none_or(|sign| closing.sign() == sign)
}

/// The deepest Charge rank in `charges`, or `None` when the iterator is empty.
///
/// The rank is [`crate::acts::charge_depth_rank`]: positive = 0, contrary = 1,
/// contradictory = 2, negation of the negation = 3.
pub fn deepest_charge_rank<'a>(charges: impl IntoIterator<Item = &'a Charge>) -> Option<u8> {
    charges
        .into_iter()
        .map(|charge| crate::acts::charge_depth_rank(charge.depth()))
        .max()
}

/// One Act's story range, derived from the merged Act climax placements.
///
/// The documented rule: with Acts ordered by `order`, Act N covers the Scenes
/// **after** the previous bounded Act's climax and **at or before** its own
/// climax. Act 1 therefore has no lower bound. A Scene after the last Act's
/// climax belongs to no Act and is never depth-compared. An Act with no climax,
/// or with a chapter-wide climax, is skipped and creates no boundary; the Act
/// list gate already reports that defect, and inventing a boundary here would
/// compare Scenes against a range the author never planned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActStoryRange {
    /// The Act's 1-based order.
    pub order: i32,
    /// Exclusive lower bound: the previous bounded Act's climax, if any.
    pub after: Option<PlanningStoryPlacement>,
    /// Inclusive upper bound: this Act's climax.
    pub through: PlanningStoryPlacement,
}

/// Derive the Act story ranges from a merged Act list. See [`ActStoryRange`].
pub fn act_story_ranges(acts: &[PlannedAct]) -> Vec<ActStoryRange> {
    let mut ordered: Vec<&PlannedAct> = acts.iter().collect();
    ordered.sort_by_key(|act| act.order);
    let mut ranges = Vec::new();
    let mut previous: Option<PlanningStoryPlacement> = None;
    for act in ordered {
        let Some(climax) = act.climax.as_ref() else {
            continue;
        };
        if climax.scene_order.is_none() {
            continue;
        }
        ranges.push(ActStoryRange {
            order: act.order,
            after: previous.clone(),
            through: climax.clone(),
        });
        previous = Some(climax.clone());
    }
    ranges
}

/// The Act range that contains `scene`, if any. See [`ActStoryRange`].
pub fn act_range_for_scene<'a>(
    ranges: &'a [ActStoryRange],
    scene: &PlanningStoryPlacement,
) -> Option<&'a ActStoryRange> {
    ranges.iter().find(|range| {
        let after = range
            .after
            .as_ref()
            .is_none_or(|after| scene.cmp_story_order(after) == Ordering::Greater);
        let through = scene.cmp_story_order(&range.through) != Ordering::Greater;
        after && through
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charge::{Charge, ChargeDepth};

    fn place(book: i32, chapter: i32, scene: Option<i32>) -> PlanningStoryPlacement {
        PlanningStoryPlacement {
            book_number: book,
            chapter_number: chapter,
            scene_order: scene,
            note: None,
        }
    }

    fn charge(sign: ChargeSign, depth: ChargeDepth) -> Charge {
        Charge::new(sign, depth).unwrap()
    }

    fn act(order: i32, book: i32, chapter: i32, scene: Option<i32>) -> PlannedAct {
        PlannedAct {
            order,
            climax: Some(place(book, chapter, scene)),
            charge: None,
            central_question_part: format!("part {order}"),
            answers_central_question: false,
            cause: None,
        }
    }

    #[test]
    fn a_climax_at_or_before_a_convergence_point_satisfies_it() {
        let points = vec![place(1, 3, Some(2)), place(2, 1, Some(1))];
        for climax in [
            place(1, 1, Some(1)),
            place(1, 3, Some(2)),
            place(1, 3, Some(1)),
            place(2, 1, Some(1)),
        ] {
            assert!(
                climax_within_convergence_points(&climax, &points),
                "{climax:?} should satisfy a convergence point"
            );
        }
        for climax in [place(2, 2, Some(1)), place(3, 1, Some(1))] {
            assert!(
                !climax_within_convergence_points(&climax, &points),
                "{climax:?} comes after every convergence point"
            );
        }
        assert!(!climax_within_convergence_points(
            &place(1, 1, Some(1)),
            &[]
        ));
    }

    #[test]
    fn a_chapter_wide_convergence_point_accepts_any_scene_in_its_chapter() {
        let points = vec![place(1, 2, None)];
        assert!(climax_within_convergence_points(
            &place(1, 2, Some(9)),
            &points
        ));
        assert!(!climax_within_convergence_points(
            &place(1, 3, Some(1)),
            &points
        ));
    }

    #[test]
    fn chapters_must_be_adjacent_in_the_same_book() {
        assert!(chapters_are_consecutive((1, 1), (1, 2)));
        assert!(chapters_are_consecutive((2, 7), (2, 8)));
        assert!(!chapters_are_consecutive((1, 1), (1, 3)));
        assert!(!chapters_are_consecutive((1, 1), (2, 2)));
        assert!(!chapters_are_consecutive((1, 2), (1, 1)));
    }

    #[test]
    fn the_job_heuristic_only_constrains_contradict_and_resonate() {
        assert_eq!(
            job_requires_closing_sign(SubplotJob::Contradict),
            Some(ChargeSign::Negative)
        );
        assert_eq!(
            job_requires_closing_sign(SubplotJob::Resonate),
            Some(ChargeSign::Positive)
        );
        assert_eq!(job_requires_closing_sign(SubplotJob::SetUp), None);
        assert_eq!(job_requires_closing_sign(SubplotJob::Complicate), None);

        let positive = charge(ChargeSign::Positive, ChargeDepth::Positive);
        let negative = charge(ChargeSign::Negative, ChargeDepth::Contradictory);
        assert!(closing_supports_job(SubplotJob::Contradict, negative));
        assert!(!closing_supports_job(SubplotJob::Contradict, positive));
        assert!(closing_supports_job(SubplotJob::Resonate, positive));
        assert!(!closing_supports_job(SubplotJob::Resonate, negative));
        for job in [SubplotJob::SetUp, SubplotJob::Complicate] {
            assert!(closing_supports_job(job, positive));
            assert!(closing_supports_job(job, negative));
        }
    }

    #[test]
    fn the_deepest_rank_is_the_highest_step_on_the_spectrum() {
        assert_eq!(deepest_charge_rank(&[]), None);
        assert_eq!(
            deepest_charge_rank(&[
                charge(ChargeSign::Positive, ChargeDepth::Positive),
                charge(ChargeSign::Negative, ChargeDepth::NegationOfTheNegation),
                charge(ChargeSign::Negative, ChargeDepth::Contrary),
            ]),
            Some(3)
        );
        assert_eq!(
            deepest_charge_rank(&[charge(ChargeSign::Positive, ChargeDepth::Positive)]),
            Some(0)
        );
    }

    #[test]
    fn act_ranges_are_after_the_previous_climax_up_to_this_climax() {
        let acts = vec![act(1, 1, 1, Some(2)), act(2, 1, 2, Some(2))];
        let ranges = act_story_ranges(&acts);
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].after, None);
        assert_eq!(ranges[0].through, place(1, 1, Some(2)));
        assert_eq!(ranges[1].after, Some(place(1, 1, Some(2))));
        assert_eq!(ranges[1].through, place(1, 2, Some(2)));

        let in_act_one = place(1, 1, Some(1));
        let on_act_one_climax = place(1, 1, Some(2));
        let after_act_one_before_act_two_climax = place(1, 2, Some(1));
        let after_act_two = place(1, 2, Some(3));
        assert_eq!(
            act_range_for_scene(&ranges, &in_act_one).map(|range| range.order),
            Some(1)
        );
        assert_eq!(
            act_range_for_scene(&ranges, &on_act_one_climax).map(|range| range.order),
            Some(1)
        );
        assert_eq!(
            act_range_for_scene(&ranges, &after_act_one_before_act_two_climax)
                .map(|range| range.order),
            Some(2)
        );
        assert_eq!(act_range_for_scene(&ranges, &after_act_two), None);
    }

    #[test]
    fn acts_without_an_exact_climax_are_skipped_and_create_no_boundary() {
        let mut no_climax = act(2, 1, 2, Some(2));
        no_climax.climax = None;
        let mut chapter_wide = act(3, 1, 3, None);
        chapter_wide.climax = Some(place(1, 3, None));
        let acts = vec![act(1, 1, 1, Some(2)), no_climax, chapter_wide];
        let ranges = act_story_ranges(&acts);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].order, 1);
        assert_eq!(
            act_range_for_scene(&ranges, &place(1, 4, Some(1))).map(|range| range.order),
            None,
            "a skipped Act must not extend the previous range"
        );
    }

    #[test]
    fn act_ranges_are_ordered_by_order_and_cross_books() {
        let acts = vec![
            act(2, 2, 1, Some(2)),
            act(1, 1, 100, Some(1)),
            act(3, 2, 2, Some(1)),
        ];
        let ranges = act_story_ranges(&acts);
        assert_eq!(
            ranges.iter().map(|range| range.order).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            act_range_for_scene(&ranges, &place(2, 1, Some(1))).map(|range| range.order),
            Some(2)
        );
        assert_eq!(
            act_range_for_scene(&ranges, &place(2, 2, Some(1))).map(|range| range.order),
            Some(3)
        );
    }
}
