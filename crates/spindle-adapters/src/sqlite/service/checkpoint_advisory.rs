//! The three advisory McKee checkpoint answers, computed deterministically.
//!
//! Checkpoint review, the reader-simulation pass and the dual-persona output all
//! live in one checkpoint report. For a McKee-structured project that report
//! additionally carries [`CheckpointAdvisory`]: Central Question progress, the
//! side of the Controlling Idea / Counter-Idea debate the reader is on, and the
//! Subplots the reader has likely forgotten.
//!
//! The answers are **advisory** — the checkpoint gate never reads them — and
//! they are computed from data already in the Story Bible:
//!
//! * **Central Question progress** reads the accepted Act list (planning
//!   intent) and the accepted series/book Story Spine.
//! * **The debate side** reads the last written Scene whose realized report
//!   turns the Spine Axis. Planning intent cannot answer it: only prose the
//!   reader has actually read can.
//! * **Dormant Subplots** counts the read Chapters since each Subplot's Value
//!   Axis last turned in a written Scene, using the same
//!   [`DORMANT_CHAPTER_RUN`](spindle_core::subplot::DORMANT_CHAPTER_RUN) window
//!   the planning validator uses.
//!
//! No model call is made, so fixed fixtures drive the tests. A non-McKee project
//! returns `None`; its report bytes stay unchanged.

use super::*;
use crate::sqlite::records::{PlotLine, Scene};
use spindle_core::charge::{Charge, ChargeSign};
use spindle_core::checkpoint_advisory::{
    CheckpointAdvisory, CheckpointAdvisoryInput, ControllingIdeaDebate, ControllingIdeaDebateSide,
    DormantSubplot,
};
use spindle_core::story_spine::StorySpine;
use spindle_core::subplot::DORMANT_CHAPTER_RUN;
use std::collections::BTreeSet;

impl SqliteSpindleService {
    /// The three advisory answers for a checkpoint range, or `None` when the
    /// project is not McKee-structured.
    ///
    /// `through_chapter` is the checkpoint range's last Chapter: the reader is
    /// assumed to have read every written Scene in the book up to it. Callers
    /// treat the result as report enrichment; an error here must never block a
    /// checkpoint, so the tool layer logs and omits it.
    pub async fn checkpoint_advisory(
        &self,
        input: CheckpointAdvisoryInput,
    ) -> Result<Option<CheckpointAdvisory>> {
        let project = self.repository.get_project(&input.project_id).await?;
        if !project.mckee_structured {
            return Ok(None);
        }
        let Some(branch_id) = project.active_branch_id.clone() else {
            return Ok(None);
        };
        if input.book_number <= 0 || input.through_chapter <= 0 {
            return Ok(None);
        }

        let spines = self.repository.list_story_spines(&input.project_id).await?;
        let spine = book_spine(&spines, input.book_number);
        let acts = self
            .repository
            .accepted_planning_acts(&input.project_id, &branch_id)
            .await?;
        let central_question = spindle_core::checkpoint_advisory::central_question_progress(
            &acts,
            spine,
            input.book_number,
            input.through_chapter,
        );

        let scenes = self
            .repository
            .list_scenes_by_project_and_branch(&input.project_id, &branch_id)
            .await?;
        let plot_lines = self
            .repository
            .list_plot_lines_by_project_and_branch(&input.project_id, &branch_id)
            .await?;

        let controlling_idea_debate = debate_answer(
            spine,
            &plot_lines,
            &scenes,
            input.book_number,
            input.through_chapter,
        );
        let dormant_subplots = dormant_subplot_answers(
            &plot_lines,
            &scenes,
            input.book_number,
            input.through_chapter,
        );

        Ok(Some(CheckpointAdvisory {
            project_id: input.project_id,
            book_number: input.book_number,
            through_chapter: input.through_chapter,
            central_question,
            controlling_idea_debate,
            dormant_subplots,
        }))
    }
}

/// The book sub-spine when one is accepted, otherwise the series spine.
fn book_spine(spines: &[StorySpine], book_number: i32) -> Option<&StorySpine> {
    spines
        .iter()
        .find(|spine| spine.book_number == Some(book_number))
        .or_else(|| spines.iter().find(|spine| spine.is_series_spine()))
}

/// The written Scenes in one book at or before `through_chapter`, in story order.
fn read_scenes(scenes: &[Scene], book_number: i32, through_chapter: i32) -> Vec<&Scene> {
    scenes
        .iter()
        .filter(|scene| {
            scene.book_number == book_number
                && scene.chapter_number <= through_chapter
                && !scene.full_text.trim().is_empty()
        })
        .collect()
}

/// Every read Chapter of the book, in order.
fn read_chapters(scenes: &[&Scene], book_number: i32) -> BTreeSet<i32> {
    scenes
        .iter()
        .filter(|scene| scene.book_number == book_number)
        .map(|scene| scene.chapter_number)
        .collect()
}

/// The last written Scene whose realized report turns `axis` (trim-exact), with
/// that Scene's Chapter.
fn last_realized_turn<'a>(scenes: &'a [&'a Scene], axis: &str) -> Option<(&'a Scene, Charge)> {
    let axis = axis.trim();
    if axis.is_empty() {
        return None;
    }
    scenes
        .iter()
        .filter_map(|scene| {
            let report = scene.realized_report.as_ref()?;
            let turn = report
                .value_turns
                .iter()
                .find(|turn| turn.axis.trim() == axis)?;
            (turn.opening != turn.closing).then_some((*scene, turn.closing))
        })
        .max_by_key(|(scene, _)| (scene.book_number, scene.chapter_number, scene.scene_order))
}

/// Question 2: the side the last realized Spine Axis turn proves.
fn debate_answer(
    spine: Option<&StorySpine>,
    plot_lines: &[PlotLine],
    scenes: &[Scene],
    book_number: i32,
    through_chapter: i32,
) -> ControllingIdeaDebate {
    let controlling_idea = spine.map(format_controlling_idea).unwrap_or_default();
    let counter_idea = spine
        .map(|spine| spine.counter_idea.trim().to_string())
        .unwrap_or_default();
    let axis = spine
        .map(|spine| spine.spine_axis.trim())
        .filter(|axis| !axis.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            // A McKee project's Main Plot Value Axis equals the Spine Axis, so it
            // is a safe fallback only while the spine itself is unreadable.
            plot_lines
                .iter()
                .find(|line| line.main_plot && line.archived_at.is_none())
                .and_then(|line| line.value_axis.as_deref())
                .map(str::trim)
                .filter(|axis| !axis.is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_default();

    let read = read_scenes(scenes, book_number, through_chapter);
    let Some((scene, closing)) = last_realized_turn(&read, &axis) else {
        return ControllingIdeaDebate {
            controlling_idea,
            counter_idea,
            axis: axis.clone(),
            reader_side: ControllingIdeaDebateSide::Undecided,
            basis: if axis.is_empty() {
                "no Spine Axis is planned, so no realized turn can decide the side".to_string()
            } else {
                format!("no written Scene has realized a turn on the Spine Axis '{axis}' yet")
            },
        };
    };

    let side = match closing.sign() {
        ChargeSign::Positive => ControllingIdeaDebateSide::ControllingIdea,
        ChargeSign::Negative => ControllingIdeaDebateSide::CounterIdea,
    };
    let basis = format!(
        "the last realized turn on '{axis}' closes {} in book {} chapter {} (scene {})",
        spindle_core::realized::charge_label(closing),
        scene.book_number,
        scene.chapter_number,
        scene.scene_order
    );
    ControllingIdeaDebate {
        controlling_idea,
        counter_idea,
        axis,
        reader_side: side,
        basis,
    }
}

/// `value because cause`, the Controlling Idea's claim.
fn format_controlling_idea(spine: &StorySpine) -> String {
    let value = spine.controlling_idea.value.trim();
    let cause = spine.controlling_idea.cause.trim();
    match (value.is_empty(), cause.is_empty()) {
        (false, false) => format!("{value} because {cause}"),
        (false, true) => value.to_string(),
        (true, false) => cause.to_string(),
        (true, true) => String::new(),
    }
}

/// Question 3: Subplots whose Value Axis has not turned in the last
/// [`DORMANT_CHAPTER_RUN`] read Chapters.
fn dormant_subplot_answers(
    plot_lines: &[PlotLine],
    scenes: &[Scene],
    book_number: i32,
    through_chapter: i32,
) -> Vec<DormantSubplot> {
    let read = read_scenes(scenes, book_number, through_chapter);
    let chapters = read_chapters(&read, book_number);
    if chapters.len() < DORMANT_CHAPTER_RUN {
        // Fewer read Chapters than the dormancy window: nothing can have gone
        // dormant yet, and naming a Subplot would be premature.
        return Vec::new();
    }

    let mut answers = Vec::new();
    for line in plot_lines {
        if line.main_plot || line.archived_at.is_some() {
            continue;
        }
        let Some(axis) = line
            .value_axis
            .as_deref()
            .map(str::trim)
            .filter(|axis| !axis.is_empty())
        else {
            continue;
        };
        let last_turn = last_realized_turn(&read, axis);
        let last_turn_chapter = last_turn.map(|(scene, _)| scene.chapter_number);
        let chapters_without_turn = match last_turn_chapter {
            Some(chapter) => chapters.iter().filter(|read| **read > chapter).count(),
            None => chapters.len(),
        };
        if chapters_without_turn < DORMANT_CHAPTER_RUN {
            continue;
        }
        answers.push(DormantSubplot {
            plot_line_id: line.id.clone(),
            name: line.name.clone(),
            value_axis: axis.to_string(),
            last_turn_chapter,
            chapters_without_turn,
        });
    }
    answers.sort_by(|left, right| {
        left.chapters_without_turn
            .cmp(&right.chapters_without_turn)
            .reverse()
            .then_with(|| left.name.cmp(&right.name))
    });
    answers
}

#[cfg(test)]
mod tests;
