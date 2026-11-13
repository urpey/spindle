//! McKee realized reports and deterministic verify (issue #32).
//!
//! These are the service-seam tests: save a McKee Scene with a realized report
//! through both continuity-carrying save paths, read it back, run the
//! deterministic scene-verify set and inspect the findings, and check that the
//! system advances Narrative Promise status with the source scene recorded.
//! Every test observes persisted, readable behaviour — never a private helper.
//!
//! The McKee project fixture uses `create_mckee_project`, the same
//! `pub(crate)` service path the planning `CreateProject` apply uses
//! (`spindle-adapters/src/sqlite/service/planning/apply.rs`), so the marker is
//! set exactly as a `new-book` project's is. The planning gates themselves are
//! covered by the planning test suite and are not re-tested here.

use super::*;
use spindle_core::charge::{Charge, ChargeDepth, ChargeSign};
use spindle_core::models::{
    CharacterArcMilestone, CheckConsistencyInput, CommitSceneCharacterStateEntry,
    CommitSceneCharacterStatePatch, CommitSceneInput, ConsistencyScopeInput,
    CreateCharacterArcInput, CreateCharacterInput, CreateLocationInput,
    CreateNarrativePromiseInput, CreateNarrativePromisesInput, CreateProjectInput,
    PlanChapterInput, PlanChapterSceneInput, ReaderContract, RegisterCanonicalFactInput,
    SCENE_VERIFY_CHECKS, SaveSceneDraftInput, StoryPlacement, UpdatePromiseStatusInput,
};
use spindle_core::planning::{
    BeginPlanningInput, PlanningReadinessInput, PlanningScope, PlanningTargetRange,
};
use spindle_core::realized::{
    CHARACTER_EFFECT_CHECK, CharacterEffect, REALIZED_KEY_FACT_CHECK, REALIZED_PROMISE_EVENT_CHECK,
    REALIZED_VALUE_TURN_CHECK, RealizedCharacterEffect, RealizedPromiseEvent,
    RealizedPromiseEventKind, RealizedReport, RealizedValueTurn,
};
use spindle_core::scene_value::{SceneGap, SceneValueAxis};

fn positive() -> Charge {
    Charge::new(ChargeSign::Positive, ChargeDepth::Positive).unwrap()
}

fn contrary() -> Charge {
    Charge::new(ChargeSign::Negative, ChargeDepth::Contrary).unwrap()
}

fn contradictory() -> Charge {
    Charge::new(ChargeSign::Negative, ChargeDepth::Contradictory).unwrap()
}

fn placement(book: i32, chapter: i32, scene: i32) -> StoryPlacement {
    StoryPlacement {
        book_number: book,
        chapter_number: chapter,
        scene_order: Some(scene),
        note: None,
    }
}

fn axis(name: &str, opening: Charge, closing: Charge) -> SceneValueAxis {
    SceneValueAxis {
        axis: name.into(),
        opening,
        closing,
    }
}

fn gap() -> SceneGap {
    SceneGap {
        expected: "The gate log will clear Ada".into(),
        actual: "The log is missing a page".into(),
    }
}

/// A McKee-structured project with one planned chapter (two Scenes), a location,
/// two characters, an arc with a planned `change` milestone at Scene 1, a
/// promise planted at Scene 1 and paid off at Scene 2, and a planned Key Fact at
/// Scene 1. Everything is created through service methods; no model calls.
struct MckeeFixture {
    project_id: String,
    branch_id: String,
    ada: String,
    ben: String,
    promise_id: String,
    key_fact_id: String,
    arc_id: String,
}

impl MckeeFixture {
    fn project_id(&self) -> &str {
        &self.project_id
    }
}

async fn mckee_fixture(svc: &SqliteSpindleService) -> MckeeFixture {
    let project = svc
        .create_mckee_project(CreateProjectInput {
            name: "Harbor".into(),
            project_type: "novel".into(),
            genre: "mystery".into(),
            reader_contract: ReaderContract {
                promise: "Find the heir".into(),
                style_notes: Vec::new(),
                boundaries: Vec::new(),
            },
        })
        .await
        .unwrap();
    let project_id = project.project_id.clone();
    let branch_id = project.branch_id.clone();

    let location = svc
        .create_location(CreateLocationInput {
            project_id: project_id.clone(),
            name: "Harbor gate".into(),
            kind: "city".into(),
            realm: None,
            summary: "A tidal port".into(),
            initial_state: Default::default(),
        })
        .await
        .unwrap();

    let mut characters = Vec::new();
    for (name, role) in [("Ada", "protagonist"), ("Ben", "opposition")] {
        let character = svc
            .create_character(CreateCharacterInput {
                project_id: project_id.clone(),
                name: name.into(),
                summary: format!("{name} of the harbor"),
                role: role.into(),
                realm: None,
                aliases: Vec::new(),
                voice_profile: Default::default(),
                emotional_profile: Default::default(),
                initial_state: None,
            })
            .await
            .unwrap();
        characters.push(character.character_id);
    }
    let ada = characters[0].clone();
    let ben = characters[1].clone();

    let arc = svc
        .create_character_arc(CreateCharacterArcInput {
            project_id: project_id.clone(),
            character_id: ada.clone(),
            arc_type: "growth".into(),
            starting_state: "Alone".into(),
            ending_state: "Trusting".into(),
            milestones: vec![CharacterArcMilestone {
                label: "Ask for help".into(),
                placement: Some(placement(1, 1, 1)),
                description: "Ada trusts Ben".into(),
                unlocks: Vec::new(),
                reached_at: None,
                kind: Some(spindle_core::models::ArcMilestoneKind::Change),
            }],
            thematic_purpose: "Risk connection".into(),
            connected_theme_ids: Vec::new(),
        })
        .await
        .unwrap();

    let promise = svc
        .create_narrative_promises(CreateNarrativePromisesInput {
            project_id: project_id.clone(),
            items: vec![spindle_core::models::BatchCreateNarrativePromiseItem {
                promise_type: "mystery".into(),
                description: "Who forged the seal?".into(),
                planted_at: placement(1, 1, 1),
                planned_payoff: Some(placement(1, 1, 2)),
                notes: Vec::new(),
            }],
            dry_run: false,
        })
        .await
        .unwrap();

    let key_fact = svc
        .register_canonical_fact(RegisterCanonicalFactInput {
            project_id: project_id.clone(),
            scene_id: None,
            book_number: 1,
            chapter_number: 1,
            fact_type: None,
            key: None,
            value: None,
            context: None,
            subject_table: Some("character".into()),
            subject_id: Some(ada.clone()),
            predicate: Some(spindle_core::key_fact::key_fact_predicate(
                spindle_core::key_fact::KeyFactKind::ItemGained,
                "Ada:broken_seal",
            )),
            value_kind: None,
            value_text: Some("Ada takes the broken seal as surety".into()),
            value_number: None,
            value_unit: None,
            value_json: None,
            aliases: Vec::new(),
            scope: None,
            valid_from: Some(placement(1, 1, 1)),
            valid_until: None,
            legacy_untyped: None,
            supersedes_fact_id: None,
            secrecy: None,
        })
        .await
        .unwrap();

    let scene_plan = |scene_order: i32, primary: SceneValueAxis, secondary: SceneValueAxis| {
        PlanChapterSceneInput {
            scene_order,
            summary: format!("Scene {scene_order}"),
            beat_structure: Vec::new(),
            beats: Vec::new(),
            turning_beat: Some(3),
            character_ids: vec![ada.clone(), ben.clone()],
            location_id: Some(location.location_id.clone()),
            content_rating: Some(spindle_core::models::ContentRating::General),
            purpose: "Turn the value".into(),
            research_required: None,
            research_tags: Vec::new(),
            explicit_query: None,
            primary_value_axis: Some(primary),
            secondary_value_axes: vec![secondary],
            opposition: Some("The harbor magistrate".into()),
            gap: Some(gap()),
            scene_question: Some("Who altered the tide log?".into()),
            inaction_cost: Some("The next tide erases the evidence".into()),
            risk: Some(3),
            dilemma: None,
        }
    };
    svc.plan_chapter(PlanChapterInput {
        project_id: project_id.clone(),
        book_number: 1,
        chapter_number: 1,
        pov_character_id: Some(ada.clone()),
        synopsis: "Ada wagers the signet".into(),
        target_theme_ids: Vec::new(),
        target_conflict_ids: Vec::new(),
        target_plot_line_ids: Vec::new(),
        scenes: vec![
            scene_plan(
                1,
                axis("trust", positive(), contradictory()),
                axis("freedom", positive(), contrary()),
            ),
            scene_plan(
                2,
                axis("trust", contradictory(), contrary()),
                axis("freedom", contrary(), contradictory()),
            ),
        ],
    })
    .await
    .unwrap();

    MckeeFixture {
        project_id,
        branch_id: branch_id.clone(),
        ada,
        ben,
        promise_id: promise.narrative_promise_ids[0].clone(),
        key_fact_id: key_fact.canonical_fact_id,
        arc_id: arc.character_arc_id,
    }
}

/// The report a matching Scene 1 produces: both planned axes turn exactly as
/// planned, the Gap and Turning Beat are reported, the planned Key Fact is
/// listed, the planned change milestone is reported as `change`, and the
/// promise setup is reported.
fn matching_report(fixture: &MckeeFixture) -> RealizedReport {
    RealizedReport {
        value_turns: vec![
            RealizedValueTurn {
                axis: "trust".into(),
                opening: positive(),
                closing: contradictory(),
                gap: Some(gap()),
                turning_beat: Some(3),
                turning_beat_summary: None,
            },
            RealizedValueTurn {
                axis: "freedom".into(),
                opening: positive(),
                closing: contrary(),
                gap: None,
                turning_beat: None,
                turning_beat_summary: Some("The gate closes behind her".into()),
            },
        ],
        character_effects: vec![RealizedCharacterEffect {
            character_id: fixture.ada.clone(),
            effect: CharacterEffect::Change,
            note: Some("Ada asks for help".into()),
        }],
        promise_events: vec![RealizedPromiseEvent {
            narrative_promise_id: fixture.promise_id.clone(),
            event: RealizedPromiseEventKind::Setup,
            note: None,
        }],
        key_fact_ids: vec![fixture.key_fact_id.clone()],
    }
}

async fn save_mckee_scene(
    svc: &SqliteSpindleService,
    fixture: &MckeeFixture,
    scene_order: i32,
    report: Option<RealizedReport>,
) -> String {
    svc.save_scene_draft(SaveSceneDraftInput {
        project_id: fixture.project_id.clone(),
        book_number: 1,
        chapter_number: 1,
        chapter_id: None,
        scene_order,
        full_text: "Ada follows the ledger to the tidal gate and loses a page of it.".to_string(),
        summary: "Ada follows the ledger".to_string(),
        content_rating: spindle_core::models::ContentRating::General,
        tone: None,
        realized_report: report,
        ..Default::default()
    })
    .await
    .unwrap()
    .scene_id
}

/// Run the scene-scoped verify subset (or a named subset) for one Scene.
async fn verify_scene(
    svc: &SqliteSpindleService,
    project_id: &str,
    scene_order: i32,
    checks: &[&str],
) -> Vec<spindle_core::models::ConsistencyIssue> {
    let mut scope = ConsistencyScopeInput::chapter_range(1, 1, 1, 1);
    scope.scene_order = Some(scene_order);
    svc.check_consistency(CheckConsistencyInput {
        project_id: project_id.to_string(),
        scope,
        checks: checks.iter().map(|check| check.to_string()).collect(),
        deep_scan_offset: None,
        severity_filter: Vec::new(),
        deep_check: Some(false),
        subjects: Vec::new(),
        format: None,
        budget_tokens: None,
    })
    .await
    .unwrap()
    .issues
}

fn findings_of<'a>(
    issues: &'a [spindle_core::models::ConsistencyIssue],
    check_type: &str,
) -> Vec<&'a spindle_core::models::ConsistencyIssue> {
    issues
        .iter()
        .filter(|issue| issue.check_type == check_type)
        .collect()
}

#[tokio::test]
async fn a_realized_report_persists_and_reads_back_on_both_save_paths() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    let report = matching_report(&fixture);

    // Save path 1: the direct draft save (`authoring_save_scene_draft` maps onto
    // this shared path).
    let scene_id = save_mckee_scene(&svc, &fixture, 1, Some(report.clone())).await;
    let stored = svc.repository.get_scene(&scene_id).await.unwrap();
    assert_eq!(stored.realized_report.as_ref(), Some(&report));

    // Read back through `get_entity(scene)`.
    let entity = svc.read_entity_by_id(&scene_id).await.unwrap();
    assert_eq!(
        entity["realized_report"]["value_turns"][0]["axis"], "trust",
        "the realized report is readable through the scene entity: {entity}"
    );
    assert_eq!(
        entity["realized_report"]["promise_events"][0]["event"],
        "setup"
    );

    // Save path 2: `commit_scene`'s continuity package.
    let scene_two = svc
        .commit_scene(CommitSceneInput {
            project_id: fixture.project_id.clone(),
            branch_id: Some(fixture.branch_id.clone()),
            preparation_id: None,
            context_revision_token: None,
            dependency_tokens: None,
            scene_id: None,
            book_number: Some(1),
            chapter_number: Some(1),
            scene_order: Some(2),
            expected_scene_version: None,
            full_text: "Ben refuses the signet, and Ada pays anyway.".to_string(),
            summary: "Ben refuses the signet".to_string(),
            content_rating: spindle_core::models::ContentRating::General,
            location_id: None,
            tone: None,
            authorship: spindle_core::models::SceneAuthorship::Assistant,
            generation_receipt: None,
            continuity: spindle_core::models::CommitSceneContinuityPackage {
                realized_report: Some(RealizedReport {
                    value_turns: vec![RealizedValueTurn {
                        axis: "trust".into(),
                        opening: contradictory(),
                        closing: contrary(),
                        gap: None,
                        turning_beat: Some(3),
                        turning_beat_summary: None,
                    }],
                    key_fact_ids: Vec::new(),
                    character_effects: Vec::new(),
                    promise_events: vec![RealizedPromiseEvent {
                        narrative_promise_id: fixture.promise_id.clone(),
                        event: RealizedPromiseEventKind::Payoff,
                        note: Some("Ben testifies after all".into()),
                    }],
                }),
                ..Default::default()
            },
            beat_annotations: Vec::new(),
            chapter_summary: None,
            run_id: None,
            expected_phase: None,
            dry_run: false,
            idempotency_key: None,
        })
        .await
        .unwrap();
    assert_eq!(scene_two.blocking_count, 0);
    let committed = svc.repository.get_scene(&scene_two.scene_id).await.unwrap();
    assert_eq!(
        committed
            .realized_report
            .as_ref()
            .map(|report| report.value_turns[0].closing),
        Some(contrary()),
        "commit_scene persists the realized report from the continuity package"
    );
}

#[tokio::test]
async fn a_scene_without_a_report_and_a_non_mckee_project_payload_are_unchanged() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    let scene_id = save_mckee_scene(&svc, &fixture, 1, None).await;
    let stored = svc.repository.get_scene(&scene_id).await.unwrap();
    assert_eq!(stored.realized_report, None);
    let entity = svc.read_entity_by_id(&scene_id).await.unwrap();
    assert!(
        entity.get("realized_report").is_none(),
        "an unset report is omitted, not blanked: {entity}"
    );

    // A non-McKee project's Scene entity keeps its exact previous payload.
    let project = svc
        .create_project(CreateProjectInput {
            name: "Existing".into(),
            project_type: "novel".into(),
            genre: "mystery".into(),
            reader_contract: ReaderContract {
                promise: "Unchanged".into(),
                style_notes: Vec::new(),
                boundaries: Vec::new(),
            },
        })
        .await
        .unwrap();
    let scene = svc
        .save_scene_draft(SaveSceneDraftInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            chapter_number: 1,
            chapter_id: None,
            scene_order: 1,
            full_text: "A quiet opening.".to_string(),
            summary: "quiet".to_string(),
            content_rating: spindle_core::models::ContentRating::General,
            ..Default::default()
        })
        .await
        .unwrap();
    let entity = svc.read_entity_by_id(&scene.scene_id).await.unwrap();
    assert!(entity.get("realized_report").is_none());
    let issues = verify_scene(
        &svc,
        &project.project_id,
        1,
        &[
            REALIZED_VALUE_TURN_CHECK,
            REALIZED_KEY_FACT_CHECK,
            REALIZED_PROMISE_EVENT_CHECK,
            CHARACTER_EFFECT_CHECK,
        ],
    )
    .await;
    assert!(
        issues.is_empty(),
        "a non-McKee project gets no realized-report findings: {issues:?}"
    );
}

#[tokio::test]
async fn a_matching_realized_report_verifies_clean() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    save_mckee_scene(&svc, &fixture, 1, Some(matching_report(&fixture))).await;

    let checks: Vec<&str> = SCENE_VERIFY_CHECKS.to_vec();
    let issues = verify_scene(&svc, fixture.project_id(), 1, &checks).await;
    for check in [
        REALIZED_VALUE_TURN_CHECK,
        REALIZED_KEY_FACT_CHECK,
        REALIZED_PROMISE_EVENT_CHECK,
        CHARACTER_EFFECT_CHECK,
    ] {
        assert!(
            findings_of(&issues, check).is_empty(),
            "a matching scene must verify clean for {check}: {issues:?}"
        );
    }
}

#[tokio::test]
async fn a_realized_value_turn_that_differs_from_the_plan_is_a_finding() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    let mut report = matching_report(&fixture);
    // Wrong closing Charge on the primary axis, wrong secondary closing Charge,
    // and a Turning Beat one position off.
    report.value_turns[0].closing = contrary();
    report.value_turns[1].closing = contradictory();
    report.value_turns[0].turning_beat = Some(2);
    save_mckee_scene(&svc, &fixture, 1, Some(report)).await;

    let issues = verify_scene(&svc, fixture.project_id(), 1, &[REALIZED_VALUE_TURN_CHECK]).await;
    let findings = findings_of(&issues, REALIZED_VALUE_TURN_CHECK);
    let messages = findings
        .iter()
        .map(|finding| finding.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        messages.contains("realizes 'trust' as positive -> contrary"),
        "{messages}"
    );
    assert!(
        messages.contains("realizes 'freedom' as positive -> contradictory"),
        "{messages}"
    );
    assert!(
        messages.contains("Turning Beat at beat 2 but the plan places it at beat 3"),
        "{messages}"
    );
    for finding in &findings {
        assert_eq!(finding.severity, "error");
        assert!(
            finding.entity_ids.iter().any(|id| id.starts_with("scene:")),
            "the finding is attributed to the scene: {finding:?}"
        );
    }
}

#[tokio::test]
async fn a_planned_axis_gap_or_report_that_is_absent_is_a_finding() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;

    // No report at all, for a Scene whose plan carries a Value Turn.
    save_mckee_scene(&svc, &fixture, 1, None).await;
    let issues = verify_scene(&svc, fixture.project_id(), 1, &[REALIZED_VALUE_TURN_CHECK]).await;
    assert!(
        findings_of(&issues, REALIZED_VALUE_TURN_CHECK)
            .iter()
            .any(|finding| finding.message.contains("no realized report")),
        "{issues:?}"
    );

    // A report that drops the secondary axis and the Gap.
    let mut partial = matching_report(&fixture);
    partial.value_turns.remove(1);
    partial.value_turns[0].gap = None;
    save_mckee_scene(&svc, &fixture, 1, Some(partial)).await;
    let issues = verify_scene(&svc, fixture.project_id(), 1, &[REALIZED_VALUE_TURN_CHECK]).await;
    let messages = findings_of(&issues, REALIZED_VALUE_TURN_CHECK)
        .iter()
        .map(|finding| finding.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        messages.contains("plans the Value Axis 'freedom'"),
        "{messages}"
    );
    assert!(messages.contains("plans a Gap"), "{messages}");

    // A report that turns an axis the plan never planned.
    let mut extra = matching_report(&fixture);
    extra.value_turns.push(RealizedValueTurn {
        axis: "justice".into(),
        opening: positive(),
        closing: contrary(),
        gap: None,
        turning_beat: None,
        turning_beat_summary: None,
    });
    save_mckee_scene(&svc, &fixture, 1, Some(extra)).await;
    let issues = verify_scene(&svc, fixture.project_id(), 1, &[REALIZED_VALUE_TURN_CHECK]).await;
    assert!(
        findings_of(&issues, REALIZED_VALUE_TURN_CHECK)
            .iter()
            .any(|finding| finding.message.contains("'justice'")),
        "{issues:?}"
    );
}

#[tokio::test]
async fn a_planned_key_fact_or_promise_event_the_report_omits_is_a_finding() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    let mut report = matching_report(&fixture);
    report.key_fact_ids.clear();
    report.promise_events.clear();
    save_mckee_scene(&svc, &fixture, 1, Some(report)).await;

    let issues = verify_scene(
        &svc,
        fixture.project_id(),
        1,
        &[REALIZED_KEY_FACT_CHECK, REALIZED_PROMISE_EVENT_CHECK],
    )
    .await;
    let key_fact_findings = findings_of(&issues, REALIZED_KEY_FACT_CHECK);
    assert_eq!(key_fact_findings.len(), 1, "{issues:?}");
    assert!(
        key_fact_findings[0].message.contains("Ada:broken_seal"),
        "the finding names the author's Key Fact key: {issues:?}"
    );
    assert!(
        key_fact_findings[0]
            .entity_ids
            .contains(&fixture.key_fact_id)
    );
    let promise_findings = findings_of(&issues, REALIZED_PROMISE_EVENT_CHECK);
    assert_eq!(promise_findings.len(), 1, "{issues:?}");
    assert!(
        promise_findings[0].message.contains("Who forged the seal?"),
        "{issues:?}"
    );
    assert!(
        promise_findings[0].entity_ids.contains(&fixture.promise_id),
        "the finding names the promise: {issues:?}"
    );

    // A matching report clears both findings again.
    save_mckee_scene(&svc, &fixture, 1, Some(matching_report(&fixture))).await;
    let issues = verify_scene(
        &svc,
        fixture.project_id(),
        1,
        &[REALIZED_KEY_FACT_CHECK, REALIZED_PROMISE_EVENT_CHECK],
    )
    .await;
    assert!(issues.is_empty(), "{issues:?}");
}

#[tokio::test]
async fn a_change_effect_is_accepted_only_at_a_planned_change_milestone() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;

    // Scene 2 has no planned change milestone: reporting `change` there is a
    // character jump.
    let mut report = RealizedReport {
        value_turns: vec![RealizedValueTurn {
            axis: "trust".into(),
            opening: contradictory(),
            closing: contrary(),
            gap: None,
            turning_beat: Some(3),
            turning_beat_summary: None,
        }],
        character_effects: vec![RealizedCharacterEffect {
            character_id: fixture.ben.clone(),
            effect: CharacterEffect::Change,
            note: None,
        }],
        promise_events: Vec::new(),
        key_fact_ids: Vec::new(),
    };
    save_mckee_scene(&svc, &fixture, 2, Some(report.clone())).await;
    let issues = verify_scene(&svc, fixture.project_id(), 2, &[CHARACTER_EFFECT_CHECK]).await;
    let findings = findings_of(&issues, CHARACTER_EFFECT_CHECK);
    assert_eq!(findings.len(), 1, "{issues:?}");
    assert!(
        findings[0].message.contains("no planned arc milestone"),
        "{issues:?}"
    );
    assert!(findings[0].entity_ids.contains(&fixture.ben));

    // Scene 1 has a planned change milestone for Ada; reporting only `reveal`
    // downgrades planned growth.
    report.character_effects = vec![RealizedCharacterEffect {
        character_id: fixture.ada.clone(),
        effect: CharacterEffect::Reveal,
        note: None,
    }];
    save_mckee_scene(&svc, &fixture, 1, Some(report.clone())).await;
    let issues = verify_scene(&svc, fixture.project_id(), 1, &[CHARACTER_EFFECT_CHECK]).await;
    let findings = findings_of(&issues, CHARACTER_EFFECT_CHECK);
    assert_eq!(findings.len(), 1, "{issues:?}");
    assert!(
        findings[0].message.contains("planned change")
            && findings[0].message.contains("records 'reveal'"),
        "{issues:?}"
    );

    // Reporting `change` at the planned milestone clears it.
    report.character_effects = vec![RealizedCharacterEffect {
        character_id: fixture.ada.clone(),
        effect: CharacterEffect::Change,
        note: None,
    }];
    save_mckee_scene(&svc, &fixture, 1, Some(report)).await;
    let issues = verify_scene(&svc, fixture.project_id(), 1, &[CHARACTER_EFFECT_CHECK]).await;
    assert!(issues.is_empty(), "{issues:?}");

    // The milestone is still the one the arc carries (the fixture is not
    // weakened by the check): reading it back proves the planned intent.
    let arcs = svc
        .repository
        .list_character_arcs_by_project(fixture.project_id())
        .await
        .unwrap();
    let arc = arcs
        .iter()
        .find(|arc| arc.id == fixture.arc_id)
        .expect("the arc exists");
    assert_eq!(
        arc.milestones[0].kind,
        Some(spindle_core::models::ArcMilestoneKind::Change)
    );
}

#[tokio::test]
async fn the_system_advances_promise_status_with_the_source_scene_recorded() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    let scene_id = save_mckee_scene(&svc, &fixture, 1, Some(matching_report(&fixture))).await;

    let promise = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();
    assert_eq!(promise.status, "planted");
    let event = promise.status_history.last().expect("a status event");
    assert_eq!(event.status, "planted");
    assert_eq!(event.source_scene_id.as_deref(), Some(scene_id.as_str()));
    assert!(
        event
            .note
            .as_deref()
            .is_some_and(|note| note.contains("system: realized") && note.contains(&scene_id)),
        "{event:?}"
    );

    // Scene 2 reports the payoff; the status advances again, same source-scene
    // discipline.
    let scene_two = save_mckee_scene(
        &svc,
        &fixture,
        2,
        Some(RealizedReport {
            value_turns: vec![RealizedValueTurn {
                axis: "trust".into(),
                opening: contradictory(),
                closing: contrary(),
                gap: None,
                turning_beat: Some(3),
                turning_beat_summary: None,
            }],
            character_effects: Vec::new(),
            promise_events: vec![RealizedPromiseEvent {
                narrative_promise_id: fixture.promise_id.clone(),
                event: RealizedPromiseEventKind::Payoff,
                note: None,
            }],
            key_fact_ids: Vec::new(),
        }),
    )
    .await;
    let promise = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();
    assert_eq!(promise.status, "paid_off");
    let event = promise.status_history.last().expect("a payoff event");
    assert_eq!(event.status, "paid_off");
    assert_eq!(event.source_scene_id.as_deref(), Some(scene_two.as_str()));

    // Re-saving the same Scene does not append the same event twice.
    let events_before = promise.status_history.len();
    save_mckee_scene(
        &svc,
        &fixture,
        2,
        Some(RealizedReport {
            value_turns: vec![RealizedValueTurn {
                axis: "trust".into(),
                opening: contradictory(),
                closing: contrary(),
                gap: None,
                turning_beat: Some(3),
                turning_beat_summary: None,
            }],
            character_effects: Vec::new(),
            promise_events: vec![RealizedPromiseEvent {
                narrative_promise_id: fixture.promise_id.clone(),
                event: RealizedPromiseEventKind::Payoff,
                note: None,
            }],
            key_fact_ids: Vec::new(),
        }),
    )
    .await;
    let promise = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();
    assert_eq!(
        promise.status_history.len(),
        events_before,
        "a re-save is idempotent for the same scene + status"
    );
}

#[tokio::test]
async fn an_unusable_realized_promise_event_fails_the_save_before_it_writes() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    let before = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();
    let mut report = matching_report(&fixture);
    report.promise_events[0].narrative_promise_id = "narrative_promise:nope".into();
    let err = svc
        .save_scene_draft(SaveSceneDraftInput {
            project_id: fixture.project_id.clone(),
            book_number: 1,
            chapter_number: 1,
            chapter_id: None,
            scene_order: 1,
            full_text: "Ada follows the ledger.".to_string(),
            summary: "Ada follows the ledger".to_string(),
            content_rating: spindle_core::models::ContentRating::General,
            realized_report: Some(report),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("unknown narrative promise"),
        "{err}"
    );
    let scenes = svc
        .repository
        .list_scenes_by_project_and_branch(fixture.project_id(), &fixture.branch_id)
        .await
        .unwrap();
    assert!(
        scenes.is_empty(),
        "a rejected report writes no scene at all: {scenes:?}"
    );
    let promise = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();
    assert_eq!(
        promise.status_history.len(),
        before.status_history.len(),
        "no status event is recorded for a rejected save"
    );
    assert_eq!(promise.status, before.status);
}

async fn prepare_run_for(
    svc: &SqliteSpindleService,
    project_id: &str,
    budget: Option<i32>,
) -> spindle_core::models::AuthoringPrepareRunOutput {
    svc.prepare_authoring_run(spindle_core::models::AuthoringPrepareRunInput {
        project_id: project_id.to_string(),
        book_number: 1,
        start_chapter: 1,
        end_chapter: Some(1),
        chapter_count: None,
        mining_policy: None,
        max_revise_attempts: budget,
        checkpoint_policy: None,
        replan_policy: None,
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn a_mckee_run_requires_a_non_zero_revise_budget_at_prepare_time() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;

    // An explicit zero budget blocks readiness with a clear requirement.
    let zero = prepare_run_for(&svc, &fixture.project_id, Some(0)).await;
    assert!(!zero.ready_to_draft, "a zero budget blocks a McKee run");
    assert!(
        zero.missing_requirements
            .iter()
            .any(|requirement| requirement.contains("max_revise_attempts")),
        "{:?}",
        zero.missing_requirements
    );

    // An absent budget is the run default of zero, so it blocks too.
    let absent = prepare_run_for(&svc, &fixture.project_id, None).await;
    assert!(
        !absent.ready_to_draft,
        "an absent budget blocks a McKee run"
    );
    assert!(
        absent
            .missing_requirements
            .iter()
            .any(|requirement| requirement.contains("max_revise_attempts")),
        "{:?}",
        absent.missing_requirements
    );

    // A real budget removes that requirement (other route requirements may
    // still apply on this model-router-less fixture).
    let non_zero = prepare_run_for(&svc, &fixture.project_id, Some(1)).await;
    assert!(
        !non_zero
            .missing_requirements
            .iter()
            .any(|requirement| requirement.contains("max_revise_attempts")),
        "{:?}",
        non_zero.missing_requirements
    );

    // A non-McKee project keeps today's behaviour: the budget is not a
    // requirement there.
    let project = svc
        .create_project(CreateProjectInput {
            name: "Existing".into(),
            project_type: "novel".into(),
            genre: "mystery".into(),
            reader_contract: ReaderContract {
                promise: "Unchanged".into(),
                style_notes: Vec::new(),
                boundaries: Vec::new(),
            },
        })
        .await
        .unwrap();
    let legacy = prepare_run_for(&svc, &project.project_id, Some(0)).await;
    assert!(
        !legacy
            .missing_requirements
            .iter()
            .any(|requirement| requirement.contains("max_revise_attempts")),
        "{:?}",
        legacy.missing_requirements
    );
}

#[tokio::test]
async fn mckee_planning_readiness_enforces_the_revise_budget() {
    // Review fix (issue #32): readiness must enforce the same McKee budget
    // requirement as the run, not pass `None` and advertise a ready range the
    // run would refuse.
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    let scope = PlanningScope {
        project_id: fixture.project_id.clone(),
        branch_id: fixture.branch_id.clone(),
    };
    let session = svc
        .begin_planning(BeginPlanningInput {
            premise: "Readiness must match the run gate.".into(),
            scope: Some(scope.clone()),
        })
        .await
        .unwrap();
    let readiness = svc
        .planning_readiness(PlanningReadinessInput {
            session_id: session.id,
            scope: Some(scope),
            target_range: Some(PlanningTargetRange {
                book_number: 1,
                chapter_start: 1,
                chapter_end: 1,
                scenes_per_chapter: 2,
            }),
            mining_policy: None,
            checkpoint_policy: None,
        })
        .await
        .unwrap();
    assert!(
        !readiness.ready_to_draft,
        "a McKee range without a revise budget is not ready: {readiness:?}"
    );
    assert!(
        readiness
            .prepare_run
            .missing_requirements
            .iter()
            .any(|requirement| requirement.contains("max_revise_attempts")),
        "{:?}",
        readiness.prepare_run.missing_requirements
    );
}

#[tokio::test]
async fn a_chapter_level_change_milestone_is_satisfied_anywhere_in_the_chapter() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    // A chapter-level `change` milestone (`scene_order: None`) is the shape
    // planning accepts; it is not pinned to one Scene.
    svc.create_character_arc(CreateCharacterArcInput {
        project_id: fixture.project_id.clone(),
        character_id: fixture.ben.clone(),
        arc_type: "growth".into(),
        starting_state: "Guarded".into(),
        ending_state: "Trusting".into(),
        milestones: vec![CharacterArcMilestone {
            label: "Chapter turn".into(),
            placement: Some(StoryPlacement {
                book_number: 1,
                chapter_number: 1,
                scene_order: None,
                note: None,
            }),
            description: "Ben chooses to testify".into(),
            unlocks: Vec::new(),
            reached_at: None,
            kind: Some(spindle_core::models::ArcMilestoneKind::Change),
        }],
        thematic_purpose: "Trust is earned in public".into(),
        connected_theme_ids: Vec::new(),
    })
    .await
    .unwrap();

    // Scene 1 also has Ada's Scene-level change milestone, so every Scene 1
    // report must satisfy it; the chapter-level rule only concerns Ben.
    let report = |effects: Vec<RealizedCharacterEffect>| RealizedReport {
        value_turns: Vec::new(),
        character_effects: effects,
        promise_events: Vec::new(),
        key_fact_ids: Vec::new(),
    };
    let ada_change = || RealizedCharacterEffect {
        character_id: fixture.ada.clone(),
        effect: CharacterEffect::Change,
        note: None,
    };
    let ben = |effect: CharacterEffect| RealizedCharacterEffect {
        character_id: fixture.ben.clone(),
        effect,
        note: None,
    };

    // Only a reveal anywhere in the chapter: the finding is attributed to the
    // Scene making the reveal claim.
    save_mckee_scene(
        &svc,
        &fixture,
        1,
        Some(report(vec![ada_change(), ben(CharacterEffect::Reveal)])),
    )
    .await;
    let issues = verify_scene(&svc, fixture.project_id(), 1, &[CHARACTER_EFFECT_CHECK]).await;
    let findings = findings_of(&issues, CHARACTER_EFFECT_CHECK);
    assert_eq!(findings.len(), 1, "{issues:?}");
    assert!(
        findings[0]
            .message
            .contains("chapter-level change milestone")
            && findings[0].message.contains("only 'reveal'"),
        "{issues:?}"
    );
    assert!(findings[0].entity_ids.contains(&fixture.ben));

    // A `change` in another Scene of the chapter satisfies the milestone, so the
    // reveal at Scene 1 is no longer a false error.
    save_mckee_scene(
        &svc,
        &fixture,
        2,
        Some(report(vec![ben(CharacterEffect::Change)])),
    )
    .await;
    let issues = verify_scene(&svc, fixture.project_id(), 1, &[CHARACTER_EFFECT_CHECK]).await;
    assert!(
        issues.is_empty(),
        "a change elsewhere in the chapter clears the reveal: {issues:?}"
    );
    let issues = verify_scene(&svc, fixture.project_id(), 2, &[CHARACTER_EFFECT_CHECK]).await;
    assert!(
        issues.is_empty(),
        "a change at a Scene in the chapter is accepted for a chapter-level milestone: {issues:?}"
    );

    // The same Scene reporting the change is accepted too.
    save_mckee_scene(
        &svc,
        &fixture,
        1,
        Some(report(vec![ada_change(), ben(CharacterEffect::Change)])),
    )
    .await;
    let issues = verify_scene(&svc, fixture.project_id(), 1, &[CHARACTER_EFFECT_CHECK]).await;
    assert!(issues.is_empty(), "{issues:?}");
}

#[tokio::test]
async fn a_failed_continuity_commit_rolls_back_the_scene_and_its_promise_status() {
    // The report's status history is written in the Scene's save transaction.
    // A later failure in the same commit unit (a foreign character state) must
    // roll both the Scene row and the status rows back.
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    let before = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();

    let other = svc
        .create_project(CreateProjectInput {
            name: "Elsewhere".into(),
            project_type: "novel".into(),
            genre: "mystery".into(),
            reader_contract: ReaderContract {
                promise: "Unrelated".into(),
                style_notes: Vec::new(),
                boundaries: Vec::new(),
            },
        })
        .await
        .unwrap();
    let foreign_character = svc
        .create_character(CreateCharacterInput {
            project_id: other.project_id.clone(),
            name: "Outsider".into(),
            summary: "Not in this book".into(),
            role: "support".into(),
            realm: None,
            aliases: Vec::new(),
            voice_profile: Default::default(),
            emotional_profile: Default::default(),
            initial_state: None,
        })
        .await
        .unwrap();

    let mut continuity = spindle_core::models::CommitSceneContinuityPackage {
        realized_report: Some(matching_report(&fixture)),
        ..Default::default()
    };
    continuity.character_states = vec![CommitSceneCharacterStateEntry {
        character_id: foreign_character.character_id,
        changes: CommitSceneCharacterStatePatch::default(),
    }];

    let err = svc
        .commit_scene(CommitSceneInput {
            project_id: fixture.project_id.clone(),
            branch_id: Some(fixture.branch_id.clone()),
            preparation_id: None,
            context_revision_token: None,
            dependency_tokens: None,
            scene_id: None,
            book_number: Some(1),
            chapter_number: Some(1),
            scene_order: Some(1),
            expected_scene_version: None,
            full_text: "Ada follows the ledger to the tidal gate.".to_string(),
            summary: "Ada follows the ledger".to_string(),
            content_rating: spindle_core::models::ContentRating::General,
            location_id: None,
            tone: None,
            authorship: spindle_core::models::SceneAuthorship::Assistant,
            generation_receipt: None,
            continuity,
            beat_annotations: Vec::new(),
            chapter_summary: None,
            run_id: None,
            expected_phase: None,
            dry_run: false,
            idempotency_key: None,
        })
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("does not belong to project"),
        "{err}"
    );

    let scenes = svc
        .repository
        .list_scenes_by_project_and_branch(fixture.project_id(), &fixture.branch_id)
        .await
        .unwrap();
    assert!(
        scenes.is_empty(),
        "the rolled-back Scene must not exist: {scenes:?}"
    );
    let promise = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();
    assert_eq!(
        promise.status, before.status,
        "a failed unit must not advance the promise"
    );
    assert_eq!(
        promise.status_history.len(),
        before.status_history.len(),
        "a failed unit must not leave a status event behind"
    );
}

#[tokio::test]
async fn resaving_a_scene_without_its_payoff_restores_the_pre_scene_status() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    let payoff_report = RealizedReport {
        value_turns: vec![RealizedValueTurn {
            axis: "trust".into(),
            opening: contradictory(),
            closing: contrary(),
            gap: None,
            turning_beat: Some(3),
            turning_beat_summary: None,
        }],
        character_effects: Vec::new(),
        promise_events: vec![RealizedPromiseEvent {
            narrative_promise_id: fixture.promise_id.clone(),
            event: RealizedPromiseEventKind::Payoff,
            note: None,
        }],
        key_fact_ids: Vec::new(),
    };
    let scene_two = save_mckee_scene(&svc, &fixture, 2, Some(payoff_report)).await;
    let paid = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();
    assert_eq!(paid.status, "paid_off");
    let events_before = paid.status_history.len();

    // Re-saving Scene 2 without the payoff must not leave a stale paid_off: an
    // append-only corrective event restores the pre-Scene status.
    save_mckee_scene(&svc, &fixture, 2, Some(RealizedReport::default())).await;
    let restored = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();
    assert_eq!(
        restored.status, "planted",
        "the pre-scene status is restored"
    );
    assert_eq!(
        restored.status_history.len(),
        events_before + 1,
        "one corrective event is appended"
    );
    let corrective = restored.status_history.last().unwrap();
    assert_eq!(corrective.status, "planted");
    assert_eq!(corrective.previous_status, "paid_off");
    assert_eq!(
        corrective.source_scene_id.as_deref(),
        Some(scene_two.as_str())
    );
    assert!(
        corrective
            .note
            .as_deref()
            .is_some_and(|note| note.contains("no longer reports")),
        "{corrective:?}"
    );
    assert!(
        restored
            .status_history
            .iter()
            .any(|event| event.status == "paid_off"
                && event.source_scene_id.as_deref() == Some(scene_two.as_str())),
        "the original payoff event stays as history (append-only)"
    );

    // An unchanged re-save that still reports no event is a no-op.
    let stable_events = restored.status_history.len();
    save_mckee_scene(&svc, &fixture, 2, Some(RealizedReport::default())).await;
    let stable = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();
    assert_eq!(stable.status, "planted");
    assert_eq!(
        stable.status_history.len(),
        stable_events,
        "a repeated removal does not append a second corrective event"
    );
}

#[tokio::test]
async fn a_mckee_project_rejects_manual_promise_status_changes() {
    let (_tmp, svc) = fresh_service_local().await;
    let fixture = mckee_fixture(&svc).await;
    let err = svc
        .update_promise_status(UpdatePromiseStatusInput {
            narrative_promise_id: fixture.promise_id.clone(),
            status: "paid_off".into(),
            note: None,
            at: None,
            source_scene_id: None,
            replaces_event_id: None,
        })
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("system-written") && err.to_string().contains("realized_report"),
        "{err}"
    );
    // The stored status did not advance.
    let promise = svc
        .repository
        .get_narrative_promise(&fixture.promise_id)
        .await
        .unwrap();
    assert_eq!(promise.status, "planted");

    // Non-McKee projects keep the direct status path.
    let project = svc
        .create_project(CreateProjectInput {
            name: "Legacy".into(),
            project_type: "novel".into(),
            genre: "mystery".into(),
            reader_contract: ReaderContract {
                promise: "Unchanged".into(),
                style_notes: Vec::new(),
                boundaries: Vec::new(),
            },
        })
        .await
        .unwrap();
    let promise = svc
        .create_narrative_promise(CreateNarrativePromiseInput {
            project_id: project.project_id.clone(),
            promise_type: "mystery".into(),
            description: "Who took the letter?".into(),
            planted_at: placement(1, 1, 1),
            planned_payoff: None,
            notes: Vec::new(),
            planned_payoff_turning_beat: None,
            target_act: None,
        })
        .await
        .unwrap();
    let output = svc
        .update_promise_status(UpdatePromiseStatusInput {
            narrative_promise_id: promise.narrative_promise_id.clone(),
            status: "paid_off".into(),
            note: None,
            at: None,
            source_scene_id: None,
            replaces_event_id: None,
        })
        .await
        .unwrap();
    assert_eq!(output.status, "paid_off");
}
