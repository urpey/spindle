//! Service-layer tests for the three advisory McKee checkpoint answers.
//!
//! These exercise the observable service read: a non-McKee project gets nothing,
//! a McKee project gets the debate side its written prose proves, and a Subplot
//! that stopped turning is listed. No model call is made (local-only router).

use super::*;
use crate::sqlite::SqlitePool;
use spindle_core::charge::{Charge, ChargeDepth, ChargeSign};
use spindle_core::checkpoint_advisory::{CheckpointAdvisoryInput, ControllingIdeaDebateSide};
use spindle_core::models::{
    ContentRating, CreateChapterInput, CreatePlotLineInput, CreateProjectInput, ReaderContract,
    SaveSceneDraftInput,
};
use spindle_core::plot_line::SubplotJob;
use spindle_core::realized::{RealizedReport, RealizedValueTurn};
use spindle_core::story_spine::{ControllingIdea, CreateStorySpineInput, StorySpineEnding};
use tempfile::TempDir;

async fn fresh_service() -> (TempDir, SqliteSpindleService) {
    let tmp = TempDir::new().unwrap();
    let pool = SqlitePool::open(&tmp.path().join("advisory.db"))
        .await
        .unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let repo = Repository::with_model_router(pool, data_dir, crate::ai::ModelRouter::local_only());
    (tmp, SqliteSpindleService::new(repo))
}

fn reader_contract() -> ReaderContract {
    ReaderContract {
        promise: "A vow tested under pressure.".into(),
        style_notes: Vec::new(),
        boundaries: Vec::new(),
    }
}

async fn create_project(svc: &SqliteSpindleService, mckee: bool) -> String {
    let input = CreateProjectInput {
        name: "Advisory".into(),
        project_type: "novel".into(),
        genre: "fantasy".into(),
        reader_contract: reader_contract(),
    };
    if mckee {
        let (project, _, _, _) = svc
            .repository()
            .create_project_with_mckee_marker(&input)
            .await
            .unwrap();
        project.id
    } else {
        svc.create_project(input).await.unwrap().project_id
    }
}

fn positive() -> Charge {
    Charge::new(ChargeSign::Positive, ChargeDepth::Positive).unwrap()
}

fn contrary() -> Charge {
    Charge::new(ChargeSign::Negative, ChargeDepth::Contrary).unwrap()
}

fn contradictory() -> Charge {
    Charge::new(ChargeSign::Negative, ChargeDepth::Contradictory).unwrap()
}

async fn series_spine(svc: &SqliteSpindleService, project_id: &str) {
    svc.repository()
        .create_story_spine(&CreateStorySpineInput {
            project_id: project_id.to_string(),
            series_spine_id: None,
            book_number: None,
            central_question: "Can Mara keep the vow without losing the harbor?".into(),
            controlling_idea: ControllingIdea {
                value: "trust".into(),
                cause: "the witness returns".into(),
            },
            counter_idea: "trust is a trap".into(),
            spine_axis: "trust".into(),
            ending_type: StorySpineEnding::Idealist,
            inciting_incident: None,
            final_climax: None,
            theme_ids: Vec::new(),
        })
        .await
        .unwrap();
}

async fn chapter(svc: &SqliteSpindleService, project_id: &str, number: i32) {
    svc.create_chapter(CreateChapterInput {
        project_id: project_id.to_string(),
        book_number: Some(1),
        book_id: None,
        chapter_number: Some(number),
        title: None,
    })
    .await
    .unwrap();
}

/// Save one written Scene reporting a single realized Value Turn, or none.
async fn save_scene(
    svc: &SqliteSpindleService,
    project_id: &str,
    chapter_number: i32,
    scene_order: i32,
    axis: Option<&str>,
    opening: Charge,
    closing: Charge,
) {
    let realized_report = axis.map(|axis| RealizedReport {
        value_turns: vec![RealizedValueTurn {
            axis: axis.to_string(),
            opening,
            closing,
            gap: None,
            turning_beat: None,
            turning_beat_summary: None,
        }],
        ..Default::default()
    });
    svc.save_scene_draft(SaveSceneDraftInput {
        project_id: project_id.to_string(),
        book_number: 1,
        chapter_number,
        chapter_id: None,
        scene_order,
        full_text: format!("Scene {chapter_number}.{scene_order} prose about the vow."),
        summary: format!("Scene {chapter_number}.{scene_order}"),
        content_rating: ContentRating::General,
        realized_report,
        ..Default::default()
    })
    .await
    .unwrap();
}

async fn advisory(
    svc: &SqliteSpindleService,
    project_id: &str,
    through_chapter: i32,
) -> Option<spindle_core::checkpoint_advisory::CheckpointAdvisory> {
    svc.checkpoint_advisory(CheckpointAdvisoryInput {
        project_id: project_id.to_string(),
        book_number: 1,
        through_chapter,
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn a_non_mckee_project_has_no_checkpoint_advisory() {
    let (_tmp, svc) = fresh_service().await;
    let project_id = create_project(&svc, false).await;
    assert!(
        advisory(&svc, &project_id, 3).await.is_none(),
        "a project without the McKee marker must stay byte-identical: no advisory"
    );
}

#[tokio::test]
async fn the_debate_side_follows_the_last_written_spine_axis_turn() {
    let (_tmp, svc) = fresh_service().await;
    let project_id = create_project(&svc, true).await;
    series_spine(&svc, &project_id).await;
    chapter(&svc, &project_id, 1).await;
    chapter(&svc, &project_id, 2).await;

    // No prose yet: the reader has not been told either side.
    let empty = advisory(&svc, &project_id, 1).await.unwrap();
    assert_eq!(
        empty.controlling_idea_debate.reader_side,
        ControllingIdeaDebateSide::Undecided
    );
    assert!(
        empty
            .controlling_idea_debate
            .basis
            .contains("no written Scene"),
        "{:?}",
        empty.controlling_idea_debate
    );
    assert_eq!(empty.controlling_idea_debate.axis, "trust");
    assert_eq!(
        empty.controlling_idea_debate.controlling_idea,
        "trust because the witness returns"
    );
    assert_eq!(
        empty.controlling_idea_debate.counter_idea,
        "trust is a trap"
    );

    // A negative closing turn puts the reader on the Counter-Idea side.
    save_scene(
        &svc,
        &project_id,
        1,
        1,
        Some("trust"),
        positive(),
        contrary(),
    )
    .await;
    let counter = advisory(&svc, &project_id, 1).await.unwrap();
    assert_eq!(
        counter.controlling_idea_debate.reader_side,
        ControllingIdeaDebateSide::CounterIdea
    );
    assert!(
        counter
            .controlling_idea_debate
            .basis
            .contains("book 1 chapter 1"),
        "{:?}",
        counter.controlling_idea_debate
    );

    // The latest written turn wins: chapter 2 closes positive again.
    save_scene(
        &svc,
        &project_id,
        2,
        1,
        Some("trust"),
        contrary(),
        positive(),
    )
    .await;
    let controlling = advisory(&svc, &project_id, 2).await.unwrap();
    assert_eq!(
        controlling.controlling_idea_debate.reader_side,
        ControllingIdeaDebateSide::ControllingIdea
    );
    assert!(
        controlling
            .controlling_idea_debate
            .basis
            .contains("book 1 chapter 2"),
        "{:?}",
        controlling.controlling_idea_debate
    );

    // A turn in a Chapter past the checkpoint range has not been read yet.
    let earlier = advisory(&svc, &project_id, 1).await.unwrap();
    assert_eq!(
        earlier.controlling_idea_debate.reader_side,
        ControllingIdeaDebateSide::CounterIdea
    );
}

#[tokio::test]
async fn a_subplot_that_stops_turning_is_listed_as_likely_forgotten() {
    let (_tmp, svc) = fresh_service().await;
    let project_id = create_project(&svc, true).await;
    series_spine(&svc, &project_id).await;

    let main = svc
        .create_plot_line(CreatePlotLineInput {
            project_id: project_id.clone(),
            name: "The vow".into(),
            plot_type: "main".into(),
            summary: "Mara's public oath".into(),
            status: None,
            convergence_points: Vec::new(),
            connected_conflict_ids: Vec::new(),
            connected_theme_ids: Vec::new(),
            value_axis: Some("trust".into()),
            subplot_job: None,
            is_main_plot: true,
        })
        .await
        .unwrap();
    let loyalty = svc
        .create_plot_line(CreatePlotLineInput {
            project_id: project_id.clone(),
            name: "The brother's loyalty".into(),
            plot_type: "parallel".into(),
            summary: "A brother keeps choosing sides".into(),
            status: None,
            convergence_points: Vec::new(),
            connected_conflict_ids: Vec::new(),
            connected_theme_ids: Vec::new(),
            value_axis: Some("loyalty".into()),
            subplot_job: Some(SubplotJob::Contradict),
            is_main_plot: false,
        })
        .await
        .unwrap();
    let inheritance = svc
        .create_plot_line(CreatePlotLineInput {
            project_id: project_id.clone(),
            name: "The inheritance".into(),
            plot_type: "parallel".into(),
            summary: "Who inherits the harbor".into(),
            status: None,
            convergence_points: Vec::new(),
            connected_conflict_ids: Vec::new(),
            connected_theme_ids: Vec::new(),
            value_axis: Some("legitimacy".into()),
            subplot_job: Some(SubplotJob::Resonate),
            is_main_plot: false,
        })
        .await
        .unwrap();
    for number in 1..=4 {
        chapter(&svc, &project_id, number).await;
    }

    // Chapter 1: the Main Plot and the loyalty Subplot both turn; the
    // inheritance Subplot is present but does not turn.
    save_scene(
        &svc,
        &project_id,
        1,
        1,
        Some("trust"),
        positive(),
        contrary(),
    )
    .await;
    save_scene(
        &svc,
        &project_id,
        1,
        2,
        Some("loyalty"),
        positive(),
        contradictory(),
    )
    .await;
    save_scene(&svc, &project_id, 1, 3, None, positive(), positive()).await;
    // Chapters 2-4 keep the Main Plot turning; loyalty never turns again, and
    // the inheritance line turns in chapter 4.
    for number in 2..=4 {
        save_scene(
            &svc,
            &project_id,
            number,
            1,
            Some("trust"),
            positive(),
            contrary(),
        )
        .await;
    }
    save_scene(
        &svc,
        &project_id,
        4,
        2,
        Some("legitimacy"),
        contrary(),
        positive(),
    )
    .await;

    let answers = advisory(&svc, &project_id, 4).await.unwrap();
    let dormant = &answers.dormant_subplots;
    assert_eq!(
        dormant.len(),
        1,
        "only the Subplot that stopped turning is dormant: {dormant:?}"
    );
    assert_eq!(dormant[0].plot_line_id, loyalty.plot_line_id);
    assert_eq!(dormant[0].name, "The brother's loyalty");
    assert_eq!(dormant[0].value_axis, "loyalty");
    assert_eq!(dormant[0].last_turn_chapter, Some(1));
    assert_eq!(
        dormant[0].chapters_without_turn, 3,
        "chapters 2, 3 and 4 were read without a loyalty turn"
    );
    assert!(
        dormant
            .iter()
            .all(|subplot| subplot.plot_line_id != main.plot_line_id
                && subplot.plot_line_id != inheritance.plot_line_id),
        "the Main Plot and the recently-turned Subplot must not be listed"
    );

    // Before the reader has read a full dormancy window, nothing is claimed.
    assert!(
        advisory(&svc, &project_id, 2)
            .await
            .unwrap()
            .dormant_subplots
            .is_empty()
    );
}
