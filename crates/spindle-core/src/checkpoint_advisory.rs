//! Advisory answers to the three McKee checkpoint questions.
//!
//! A checkpoint already reports consistency, pacing, promises and the sampled
//! reader/dual-persona reviews. For a McKee-structured project it additionally
//! answers three questions an author asks at the checkpoint gate:
//!
//! 1. how much of the **Central Question** has been answered so far: an Act
//!    counts only when its reserved climax **Scene has actually been written**
//!    (a persisted Scene at that placement), never merely because planning
//!    placed the climax inside the read range;
//! 2. which side of the **Controlling Idea / Counter-Idea** debate the reader is
//!    currently on (the side the last realized Spine Axis turn proves, across
//!    the series in story order);
//! 3. which **Subplots** the reader has likely forgotten (a Subplot whose Value
//!    Axis has not turned in the last [`DORMANT_CHAPTER_RUN`] read Chapters).
//!
//! Every answer is **advisory**: the checkpoint report carries it, the review
//! gate never reads it, and it can never approve or block a checkpoint. The
//! answers are computed deterministically from planning intent (Story Spine,
//! accepted Acts, plot lines) and written prose (the Scene's realized report);
//! no model call is involved, so a fixed fixture drives the tests.
//!
//! Existing projects (no `mckee_structured` marker) get no advisory at all: the
//! report field is `None` and is skipped while serializing, so their
//! checkpoint-report bytes are unchanged.
//!
//! [`DORMANT_CHAPTER_RUN`]: crate::subplot::DORMANT_CHAPTER_RUN

use crate::models::StoryPlacement;
use crate::planning::PlannedAct;
use crate::story_spine::StorySpine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input for the deterministic checkpoint advisory read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckpointAdvisoryInput {
    /// Project whose checkpoint is being reviewed.
    pub project_id: String,
    /// Book the checkpoint range belongs to.
    pub book_number: i32,
    /// Last Chapter the reader has reached (the checkpoint range's end).
    pub through_chapter: i32,
}

/// The three advisory answers for one checkpoint (McKee projects only).
///
/// Advisory only: nothing in the checkpoint gate reads these fields, so they can
/// inform the author's review without ever approving or blocking the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckpointAdvisory {
    /// Project whose checkpoint is being reviewed.
    pub project_id: String,
    /// Book the checkpoint range belongs to.
    pub book_number: i32,
    /// Last Chapter the reader has reached (the checkpoint range's end).
    pub through_chapter: i32,
    /// Question 1: how much of the Central Question has been answered.
    pub central_question: CentralQuestionProgress,
    /// Question 2: which side of the Controlling Idea / Counter-Idea debate the
    /// reader is currently on.
    pub controlling_idea_debate: ControllingIdeaDebate,
    /// Question 3: Subplots the reader has likely forgotten. Empty when every
    /// Subplot turned recently.
    #[serde(default)]
    pub dormant_subplots: Vec<DormantSubplot>,
}

/// Structured progress against the Story Spine's Central Question.
///
/// **Answered means written**, not planned: an Act counts only when its
/// reserved climax Scene has actually been persisted inside the checkpoint
/// range (its placement is at or before the checkpoint's last Chapter in story
/// order). A planned climax the reader has not reached yet is never presented
/// as answered; [`Self::acts_planned_reached`] keeps that planning-only position
/// signal available under its own honest label. Being reached is still a
/// position signal: the realized Value Turns are the writer's report and are
/// checked by scene verify instead.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CentralQuestionProgress {
    /// The question the story exists to answer. Empty when no spine is accepted.
    #[serde(default)]
    pub question: String,
    /// Total number of accepted Acts, in order.
    #[serde(default)]
    pub acts_total: usize,
    /// Acts whose reserved climax Scene is written and inside the range.
    #[serde(default)]
    pub acts_answered: usize,
    /// The `central_question_part` of each answered Act, in Act order.
    #[serde(default)]
    pub answered_parts: Vec<String>,
    /// The `central_question_part` of each Act without a written climax.
    #[serde(default)]
    pub remaining_parts: Vec<String>,
    /// Whether the spine's final climax Scene is written and inside the range.
    #[serde(default)]
    pub final_climax_reached: bool,
    /// Acts whose reserved climax is merely **planned** at or before the
    /// range's end. Planning intent only: this is not proof the reader has
    /// read the Scene, so it is never reported as answered.
    #[serde(default)]
    pub acts_planned_reached: usize,
    /// Lowest `order` among the Acts flagged as answering the whole Central
    /// Question, independent of repository row order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answering_act_order: Option<i32>,
}

/// The side of the Controlling Idea / Counter-Idea debate the reader currently
/// stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControllingIdeaDebateSide {
    /// The last realized Spine Axis turn closed positive: the story currently
    /// proves the Controlling Idea.
    ControllingIdea,
    /// The last realized Spine Axis turn closed negative: the story currently
    /// proves the Counter-Idea.
    CounterIdea,
    /// No realized Spine Axis turn exists yet, so no side is proven.
    Undecided,
}

/// Question 2's answer: which side of the debate the reader is on, and the
/// deterministic basis for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ControllingIdeaDebate {
    /// The Controlling Idea as `value because cause` (empty when unaccepted).
    #[serde(default)]
    pub controlling_idea: String,
    /// The Counter-Idea the story argues against (empty when unaccepted).
    #[serde(default)]
    pub counter_idea: String,
    /// The Spine Axis whose realized turns decide the side.
    #[serde(default)]
    pub axis: String,
    /// The side the last realized turn proves.
    pub reader_side: ControllingIdeaDebateSide,
    /// Human-readable, deterministic basis (which turn was used, or why none).
    pub basis: String,
}

/// One Subplot the reader has likely forgotten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DormantSubplot {
    /// Plot-line record id (`plot_line:...`).
    pub plot_line_id: String,
    /// Plot-line name.
    pub name: String,
    /// The Subplot's Value Axis; the axis its turns are counted on.
    pub value_axis: String,
    /// Book of the last written Scene whose realized report turned this axis.
    /// Unknown when no written Scene has turned it yet. Series-scoped: the last
    /// turn may be in an earlier book than the checkpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_turn_book: Option<i32>,
    /// Chapter (in [`Self::last_turn_book`]) of that last turn; unknown when no
    /// written Scene has turned the axis yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_turn_chapter: Option<i32>,
    /// Number of read Chapters after the last turn (all read Chapters when the
    /// axis never turned). Read Chapters are series-scoped and in story order.
    pub chapters_without_turn: usize,
}

/// Compute question 1 from the accepted Act list, the book/series spine and
/// the Scenes the reader has actually reached.
///
/// A book sub-spine wins over the series spine; with neither accepted the
/// question is empty and the progress still reports the Act count. An Act counts
/// as answered only when its reserved climax Scene is present in
/// `written_scenes` (a persisted Scene at that placement, book/chapter exact and
/// `scene_order` exact when the plan names one). `written_scenes` must already
/// be limited to the checkpoint range, i.e. at or before `(book_number,
/// through_chapter)` in story order; the planned-position count still applies
/// [`placement_reached`] itself. An Act with no climax is never counted.
pub fn central_question_progress(
    acts: &[PlannedAct],
    spine: Option<&StorySpine>,
    written_scenes: &[StoryPlacement],
    book_number: i32,
    through_chapter: i32,
) -> CentralQuestionProgress {
    let question = spine
        .map(|spine| spine.central_question.trim().to_string())
        .unwrap_or_default();
    // Lowest order wins: repository row order must never decide which Act is
    // reported as the one that answers the whole question.
    let answering_act_order = acts
        .iter()
        .filter(|act| act.answers_central_question)
        .map(|act| act.order)
        .min();

    let mut ordered: Vec<&PlannedAct> = acts.iter().collect();
    ordered.sort_by_key(|act| act.order);

    let mut answered_parts = Vec::new();
    let mut remaining_parts = Vec::new();
    let mut acts_answered = 0usize;
    let mut acts_planned_reached = 0usize;
    for act in ordered {
        let planned_reached = act.climax.as_ref().is_some_and(|climax| {
            placement_reached(
                climax.book_number,
                climax.chapter_number,
                book_number,
                through_chapter,
            )
        });
        if planned_reached {
            acts_planned_reached += 1;
        }
        let written = act.climax.as_ref().is_some_and(|climax| {
            placement_is_written(
                climax.book_number,
                climax.chapter_number,
                climax.scene_order,
                written_scenes,
            )
        });
        let part = act.central_question_part.trim();
        if written {
            acts_answered += 1;
            if !part.is_empty() {
                answered_parts.push(part.to_string());
            }
        } else if !part.is_empty() {
            remaining_parts.push(part.to_string());
        }
    }

    CentralQuestionProgress {
        question,
        acts_total: acts.len(),
        acts_answered,
        answered_parts,
        remaining_parts,
        final_climax_reached: spine
            .and_then(|spine| spine.final_climax.as_ref())
            .is_some_and(|climax| {
                placement_is_written(
                    climax.book_number,
                    climax.chapter_number,
                    climax.scene_order,
                    written_scenes,
                )
            }),
        acts_planned_reached,
        answering_act_order,
    }
}

/// Whether a placement has a persisted written Scene.
///
/// The comparison is exact on book and chapter; a `Some(scene_order)` must match
/// the written Scene's order, while a chapter-wide placement (`None`) accepts
/// any written Scene in that chapter.
fn placement_is_written(
    book_number: i32,
    chapter_number: i32,
    scene_order: Option<i32>,
    written_scenes: &[StoryPlacement],
) -> bool {
    written_scenes.iter().any(|scene| {
        scene.book_number == book_number
            && scene.chapter_number == chapter_number
            && scene_order.is_none_or(|order| scene.scene_order == Some(order))
    })
}

/// Whether a story placement is at or before `(book_number, through_chapter)`.
pub fn placement_reached(
    placement_book: i32,
    placement_chapter: i32,
    book_number: i32,
    through_chapter: i32,
) -> bool {
    placement_book < book_number
        || (placement_book == book_number && placement_chapter <= through_chapter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planning::PlanningStoryPlacement;
    use crate::story_spine::StorySpineEnding;

    fn act(order: i32, chapter: i32, part: &str, answers: bool) -> PlannedAct {
        PlannedAct {
            order,
            climax: Some(PlanningStoryPlacement {
                book_number: 1,
                chapter_number: chapter,
                scene_order: Some(1),
                note: None,
            }),
            charge: None,
            central_question_part: part.to_string(),
            answers_central_question: answers,
            cause: None,
        }
    }

    fn spine(question: &str) -> StorySpine {
        StorySpine {
            story_spine_id: "story_spine:test".into(),
            project_id: "project:test".into(),
            series_spine_id: None,
            book_number: None,
            central_question: question.into(),
            controlling_idea: Default::default(),
            counter_idea: String::new(),
            spine_axis: "trust".into(),
            ending_type: StorySpineEnding::Idealist,
            inciting_incident: None,
            final_climax: Some(StoryPlacement {
                book_number: 1,
                chapter_number: 9,
                scene_order: Some(1),
                note: None,
            }),
            theme_ids: Vec::new(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn written(book_number: i32, chapter_number: i32, scene_order: i32) -> StoryPlacement {
        StoryPlacement {
            book_number,
            chapter_number,
            scene_order: Some(scene_order),
            note: None,
        }
    }

    #[test]
    fn progress_counts_only_acts_whose_climax_scene_is_written() {
        let acts = vec![
            act(1, 2, "the bell", false),
            act(2, 4, "the ledger", false),
            act(3, 9, "the confession", true),
        ];
        let read = [written(1, 2, 1), written(1, 4, 1)];
        let progress = central_question_progress(&acts, Some(&spine("Who rang it?")), &read, 1, 4);
        assert_eq!(progress.question, "Who rang it?");
        assert_eq!(progress.acts_total, 3);
        assert_eq!(progress.acts_answered, 2);
        assert_eq!(progress.acts_planned_reached, 2);
        assert_eq!(progress.answered_parts, vec!["the bell", "the ledger"]);
        assert_eq!(progress.remaining_parts, vec!["the confession"]);
        assert_eq!(progress.answering_act_order, Some(3));
        assert!(!progress.final_climax_reached);

        let complete = central_question_progress(
            &acts,
            Some(&spine("Who rang it?")),
            &[written(1, 2, 1), written(1, 4, 1), written(1, 9, 1)],
            1,
            9,
        );
        assert_eq!(complete.acts_answered, 3);
        assert_eq!(complete.acts_planned_reached, 3);
        assert_eq!(complete.question, "Who rang it?");
        assert!(complete.final_climax_reached);
        assert!(complete.remaining_parts.is_empty());

        // Without an accepted spine the Act progress is still reported.
        let no_spine = central_question_progress(&acts, None, &read, 1, 4);
        assert_eq!(no_spine.question, "");
        assert_eq!(no_spine.acts_answered, 2);
    }

    #[test]
    fn a_planned_but_unwritten_climax_is_never_reported_as_answered() {
        let acts = vec![act(1, 2, "the bell", true)];
        // The planned climax is inside the read range, but no prose exists at
        // that placement yet: planning position is reported separately.
        let progress = central_question_progress(&acts, Some(&spine("Who rang it?")), &[], 1, 9);
        assert_eq!(progress.acts_answered, 0);
        assert_eq!(progress.acts_planned_reached, 1);
        assert!(progress.answered_parts.is_empty());
        assert_eq!(progress.remaining_parts, vec!["the bell"]);
        assert!(!progress.final_climax_reached);

        // A written Scene in the same chapter but a different scene slot does
        // not satisfy an exact climax placement.
        let wrong_scene = central_question_progress(
            &acts,
            Some(&spine("Who rang it?")),
            &[written(1, 2, 2)],
            1,
            9,
        );
        assert_eq!(wrong_scene.acts_answered, 0);
    }

    #[test]
    fn the_lowest_ordered_answering_act_wins_regardless_of_row_order() {
        // Reverse insertion order: repository row order must not decide.
        let acts = vec![
            act(3, 9, "third", true),
            act(1, 2, "first", true),
            act(2, 4, "second", false),
        ];
        let progress = central_question_progress(&acts, None, &[], 1, 9);
        assert_eq!(
            progress.answering_act_order,
            Some(1),
            "the answer must use the lowest flagged `order`, not the first row"
        );
    }

    #[test]
    fn acts_without_a_climax_are_never_counted_as_answered() {
        let mut dangling = act(2, 3, "ahead", false);
        dangling.climax = None;
        let progress = central_question_progress(
            &[act(1, 1, "seen", true), dangling],
            None,
            &[written(1, 1, 1)],
            1,
            9,
        );
        assert_eq!(progress.acts_total, 2);
        assert_eq!(progress.acts_answered, 1);
        assert_eq!(progress.answered_parts, vec!["seen"]);
        assert_eq!(progress.remaining_parts, vec!["ahead"]);
        assert_eq!(progress.answering_act_order, Some(1));
    }

    #[test]
    fn earlier_books_are_reached_and_later_books_are_not() {
        assert!(placement_reached(1, 9, 2, 1));
        assert!(!placement_reached(3, 1, 2, 9));
        assert!(placement_reached(2, 3, 2, 3));
        assert!(!placement_reached(2, 4, 2, 3));
    }

    #[test]
    fn the_advisory_round_trips_without_optional_fields() {
        let advisory = CheckpointAdvisory {
            project_id: "project:test".into(),
            book_number: 1,
            through_chapter: 2,
            central_question: CentralQuestionProgress::default(),
            controlling_idea_debate: ControllingIdeaDebate {
                controlling_idea: String::new(),
                counter_idea: String::new(),
                axis: String::new(),
                reader_side: ControllingIdeaDebateSide::Undecided,
                basis: "no realized Spine Axis turn yet".into(),
            },
            dormant_subplots: Vec::new(),
        };
        let wire = serde_json::to_value(&advisory).unwrap();
        assert_eq!(
            wire["controlling_idea_debate"]["reader_side"],
            serde_json::json!("undecided")
        );
        assert!(wire["central_question"]["answering_act_order"].is_null());
        assert_eq!(wire["dormant_subplots"], serde_json::json!([]));
        let round_trip: CheckpointAdvisory = serde_json::from_value(wire).unwrap();
        assert_eq!(round_trip, advisory);
    }
}
