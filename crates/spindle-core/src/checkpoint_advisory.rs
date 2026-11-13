//! Advisory answers to the three McKee checkpoint questions.
//!
//! A checkpoint already reports consistency, pacing, promises and the sampled
//! reader/dual-persona reviews. For a McKee-structured project it additionally
//! answers three questions an author asks at the checkpoint gate:
//!
//! 1. how much of the **Central Question** has been answered so far (a
//!    structured progress signal over the accepted Act list);
//! 2. which side of the **Controlling Idea / Counter-Idea** debate the reader is
//!    currently on (the side the last realized Spine Axis turn proves);
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
/// "Answered" here means **reached**: an Act's reserved climax is at or before
/// the checkpoint's last Chapter, so the reader has read the part of the
/// question that Act turns. This is a position signal, not a claim that the
/// prose actually delivered the turn (the realized Value Turns are the writer's
/// report and are checked by scene verify instead).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CentralQuestionProgress {
    /// The question the story exists to answer. Empty when no spine is accepted.
    #[serde(default)]
    pub question: String,
    /// Total number of accepted Acts, in order.
    #[serde(default)]
    pub acts_total: usize,
    /// Acts whose reserved climax is at or before the checkpoint range's end.
    #[serde(default)]
    pub acts_answered: usize,
    /// The `central_question_part` of each reached Act, in Act order.
    #[serde(default)]
    pub answered_parts: Vec<String>,
    /// The `central_question_part` of each Act still ahead of the reader.
    #[serde(default)]
    pub remaining_parts: Vec<String>,
    /// Whether the spine's final climax is at or before the range's end.
    #[serde(default)]
    pub final_climax_reached: bool,
    /// `order` of the Act flagged as answering the whole Central Question.
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
    /// Last Chapter whose written Scene turned this axis. Unknown when no
    /// written Scene has turned it yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_turn_chapter: Option<i32>,
    /// Number of read Chapters after the last turn (all read Chapters when the
    /// axis never turned).
    pub chapters_without_turn: usize,
}

/// Compute question 1 from the accepted Act list and the book/series spine.
///
/// A book sub-spine wins over the series spine; with neither accepted the
/// question is empty and the progress still reports the Act count. An Act counts
/// as answered when its reserved climax is at or before `(book_number,
/// through_chapter)` in story order; an Act with no climax is never counted.
pub fn central_question_progress(
    acts: &[PlannedAct],
    spine: Option<&StorySpine>,
    book_number: i32,
    through_chapter: i32,
) -> CentralQuestionProgress {
    let question = spine
        .map(|spine| spine.central_question.trim().to_string())
        .unwrap_or_default();
    let answering_act_order = acts
        .iter()
        .find(|act| act.answers_central_question)
        .map(|act| act.order);

    let mut ordered: Vec<&PlannedAct> = acts.iter().collect();
    ordered.sort_by_key(|act| act.order);

    let mut answered_parts = Vec::new();
    let mut remaining_parts = Vec::new();
    let mut acts_answered = 0usize;
    for act in ordered {
        let reached = act.climax.as_ref().is_some_and(|climax| {
            placement_reached(
                climax.book_number,
                climax.chapter_number,
                book_number,
                through_chapter,
            )
        });
        let part = act.central_question_part.trim();
        if reached {
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
                placement_reached(
                    climax.book_number,
                    climax.chapter_number,
                    book_number,
                    through_chapter,
                )
            }),
        answering_act_order,
    }
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
    use crate::models::StoryPlacement;
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

    #[test]
    fn progress_counts_the_acts_the_reader_has_reached() {
        let acts = vec![
            act(1, 2, "the bell", false),
            act(2, 4, "the ledger", false),
            act(3, 9, "the confession", true),
        ];
        let progress = central_question_progress(&acts, Some(&spine("Who rang it?")), 1, 4);
        assert_eq!(progress.question, "Who rang it?");
        assert_eq!(progress.acts_total, 3);
        assert_eq!(progress.acts_answered, 2);
        assert_eq!(progress.answered_parts, vec!["the bell", "the ledger"]);
        assert_eq!(progress.remaining_parts, vec!["the confession"]);
        assert_eq!(progress.answering_act_order, Some(3));
        assert!(!progress.final_climax_reached);

        let complete = central_question_progress(&acts, Some(&spine("Who rang it?")), 1, 9);
        assert_eq!(complete.acts_answered, 3);
        assert_eq!(complete.question, "Who rang it?");
        assert!(complete.final_climax_reached);
        assert!(complete.remaining_parts.is_empty());

        // Without an accepted spine the Act progress is still reported.
        let no_spine = central_question_progress(&acts, None, 1, 4);
        assert_eq!(no_spine.question, "");
        assert_eq!(no_spine.acts_answered, 2);
    }

    #[test]
    fn acts_without_a_climax_are_never_counted_as_answered() {
        let mut dangling = act(2, 3, "ahead", false);
        dangling.climax = None;
        let progress = central_question_progress(&[act(1, 1, "seen", true), dangling], None, 1, 9);
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
