//! End-to-end McKee fixture: contract level → committed Scene, all through the
//! service layer and without a single model call.
//!
//! This is the integration proof issue #35 asks for: the shipped pieces work
//! together in the order the `new-book` flow uses them.
//!
//! 1. planning stages and a human accepts the **contract** (which creates the
//!    McKee-marked project) and the **Story Spine**;
//! 2. **cast & world** stages the cast, the location, the world rule and
//!    Mara's Character Design;
//! 3. the **series arc** stages the theme, the Main Plot, a narrative promise,
//!    Mara's arc and the ordered Act list (Acts are outline intent, ADR 0006);
//! 4. the **book** stages its outline; the **chapter** stages its Sequences;
//! 5. the **scene manifest** stages two planned Scenes with structured Beats,
//!    a chained value plan, Gap, Turning Beat, Inaction Cost and risk;
//! 6. the first planned Scene is committed through `commit_scene` with a typed
//!    realized report, and `check_consistency`'s deterministic McKee verify
//!    passes clean;
//! 7. the checkpoint advisory reads the accepted Acts and the written prose
//!    back: Central Question progress (written-only, with the planned position
//!    reported separately), the series-scoped debate side, and a Subplot that
//!    stays dormant for three read Chapters.
//!
//! No route is configured (local-only model router), so a hidden model call
//! would fail the test rather than pass silently.

use spindle_adapters::ai::ModelRouter;
use spindle_adapters::sqlite::{Repository, SqlitePool, SqliteSpindleService};
use spindle_core::charge::{Charge, ChargeDepth, ChargeSign};
use spindle_core::checkpoint_advisory::{CheckpointAdvisoryInput, ControllingIdeaDebateSide};
use spindle_core::models::{
    CheckConsistencyInput, CommitSceneContinuityPackage, CommitSceneInput, ConsistencyScopeInput,
    ContentRating, CreateChapterInput, SaveSceneDraftInput, SceneAuthorship,
};
use spindle_core::planning::{
    BeginPlanningInput, DecidePlanningInput, PlannedAct, PlannedPromiseAction, PlannedPromiseEvent,
    PlannedSequence, PlanningDecision, PlanningLevel, PlanningOperationItem, PlanningProposal,
    PlanningRef, PlanningSchedule, PlanningSession, PlanningStoryPlacement, PlanningTargetRange,
    StagePlanningInput,
};
use spindle_core::realized::{
    RealizedPromiseEvent, RealizedPromiseEventKind, RealizedReport, RealizedValueTurn,
};
use spindle_core::scene_value::SceneGap;
use std::collections::BTreeMap;
use tempfile::TempDir;

async fn fresh_service() -> (TempDir, SqliteSpindleService) {
    let tmp = TempDir::new().unwrap();
    let pool = SqlitePool::open(&tmp.path().join("mckee-e2e.db"))
        .await
        .unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let repo = Repository::with_model_router(pool, data_dir, ModelRouter::local_only());
    (tmp, SqliteSpindleService::new(repo))
}

fn planned(key: &str, name: &str, input: serde_json::Value) -> PlanningOperationItem {
    serde_json::from_value(serde_json::json!({
        "output_ref": key,
        "operation": {"operation": name, "input": input}
    }))
    .unwrap()
}

fn reference(key: &str) -> serde_json::Value {
    serde_json::json!({"kind": "proposal_ref", "value": key})
}

fn place(book_number: i32, chapter_number: i32, scene_order: i32) -> PlanningStoryPlacement {
    PlanningStoryPlacement {
        book_number,
        chapter_number,
        scene_order: Some(scene_order),
        note: None,
    }
}

fn charge(sign: ChargeSign, depth: ChargeDepth) -> Charge {
    Charge::new(sign, depth).unwrap()
}

/// Three structured Beats with exactly one `turn`, for one fixture Scene.
fn scene_beats() -> serde_json::Value {
    serde_json::json!([
        {
            "kind": "action",
            "actor_character_id": reference("cast:mara"),
            "tactic": "Test the tower door",
            "said": "The bell rang.",
            "wanted": "A reason she can name",
            "gap": {"expected": "A weather-vane fault", "actual": "The clapper is missing"}
        },
        {
            "kind": "reaction",
            "actor_character_id": reference("cast:mara"),
            "tactic": "Talk herself out of it",
            "said": "Fog moves like that.",
            "wanted": "To stay safe",
            "gap": {"expected": "The sound explains itself", "actual": "It rings again"}
        },
        {
            "kind": "turn",
            "actor_character_id": reference("cast:mara"),
            "tactic": "Climb to the lantern room",
            "said": "Then I'll look.",
            "wanted": "The truth of the bell",
            "gap": {"expected": "Proof of a prank", "actual": "The bell was never fitted"}
        }
    ])
}

fn value_axis(
    opening_sign: &str,
    opening_depth: &str,
    closing_sign: &str,
    closing_depth: &str,
) -> serde_json::Value {
    serde_json::json!({
        "axis": "truth",
        "opening": {"sign": opening_sign, "depth": opening_depth},
        "closing": {"sign": closing_sign, "depth": closing_depth},
    })
}

/// Stage one planning artifact against the session and its already-accepted
/// parents. No model route is configured; staging is pure validation.
async fn stage(
    svc: &SqliteSpindleService,
    session: &PlanningSession,
    level: PlanningLevel,
    parents: &[&PlanningProposal],
    operations: Vec<PlanningOperationItem>,
    schedule: PlanningSchedule,
    target_range: Option<PlanningTargetRange>,
) -> PlanningProposal {
    svc.stage_planning(StagePlanningInput {
        session_id: session.id.clone(),
        scope: session.scope.clone(),
        intent_hash: session.intent_hash.clone(),
        schema_version: 1,
        level,
        parents: parents
            .iter()
            .map(|parent| (parent.id.clone(), parent.payload_hash.clone()))
            .collect(),
        operations,
        rationale: format!("Accept the {level:?} level of the fixture"),
        target_range,
        schedule,
        assumptions: Vec::new(),
        author_questions: Vec::new(),
    })
    .await
    .unwrap_or_else(|error| panic!("staging {level:?} failed: {error:#}"))
}

/// The human accepts exactly one staged proposal (ADR 0004: no auto-accept).
async fn accept(
    svc: &SqliteSpindleService,
    session: &PlanningSession,
    proposal: &PlanningProposal,
) {
    svc.decide_planning(DecidePlanningInput {
        session_id: session.id.clone(),
        scope: session.scope.clone(),
        decision: PlanningDecision::Accept,
        expected_hashes: BTreeMap::from([(proposal.id.clone(), proposal.payload_hash.clone())]),
        actor: "author:test".into(),
        note: "The author accepts this level".into(),
    })
    .await
    .unwrap_or_else(|error| panic!("accepting {:?} failed: {error:#}", proposal.id));
}

#[tokio::test]
async fn contract_to_committed_scene_with_a_realized_report_verifies_clean() {
    use serde_json::json;

    let (_tmp, svc) = fresh_service().await;

    // ── Contract root: this is the only path that sets the McKee marker ──
    let unattached = svc
        .begin_planning(BeginPlanningInput {
            premise: "A harbor archivist hears a bell only she can hear.".into(),
            scope: None,
        })
        .await
        .unwrap();
    let contract = stage(
        &svc,
        &unattached,
        PlanningLevel::Contract,
        &[],
        vec![planned(
            "project:root",
            "create_project",
            json!({
                "name": "Harbor Bell",
                "project_type": "novel",
                "genre": "mystery",
                "reader_contract": {
                    "promise": "Find the heir",
                    "style_notes": [],
                    "boundaries": []
                }
            }),
        )],
        PlanningSchedule::default(),
        None,
    )
    .await;
    accept(&svc, &unattached, &contract).await;
    let session = svc.get_planning_session(&unattached.id).await.unwrap();
    let scope = session
        .scope
        .clone()
        .expect("the contract attaches a project");
    let project_id = scope.project_id.clone();

    // ── Story Spine at the contract level (one series spine) ──
    let spine = stage(
        &svc,
        &session,
        PlanningLevel::Contract,
        &[&contract],
        vec![planned(
            "spine:series",
            "create_story_spine",
            json!({
                "central_question": "Can Ada publish the truth before the harbor closes?",
                "controlling_idea": {
                    "value": "truth",
                    "cause": "Ada publishes the forged ledger and the harbor pays for it"
                },
                "counter_idea": "Truth is a luxury the harbor cannot afford",
                "spine_axis": "truth",
                "ending_type": "idealist",
                "inciting_incident": {"book_number": 1, "chapter_number": 1, "scene_order": 1},
                "final_climax": {"book_number": 1, "chapter_number": 2, "scene_order": 2}
            }),
        )],
        PlanningSchedule::default(),
        None,
    )
    .await;
    accept(&svc, &session, &spine).await;

    // ── Cast & world, including the Character Design ──
    let cast = stage(
        &svc,
        &session,
        PlanningLevel::CastWorld,
        &[&contract, &spine],
        vec![
            planned(
                "cast:mara",
                "create_character",
                json!({"name": "Mara", "summary": "Harbor archivist", "role": "protagonist"}),
            ),
            planned(
                "loc:watchtower",
                "create_location",
                json!({"name": "Watchtower", "kind": "building", "summary": "Tidal signal tower"}),
            ),
            planned(
                "rule:tides",
                "create_world_rule",
                json!({"rule_name": "Harbor tides", "rule_type": "physics", "description": "The tidal gate opens at dawn"}),
            ),
            planned(
                "design:mara",
                "create_character_design",
                json!({
                    "character_id": reference("cast:mara"),
                    "contradictions": ["guarded", "resolute"],
                    "public_self": "Harbor archivist who keeps the tide tables",
                    "personal_self": "Loyal to the sister she raised",
                    "private_self": "Fears deep water after the wreck",
                    "hidden_self": "She caused the fire that killed her brother",
                    "conscious_desire": "Find the missing heir and clear her name",
                    "unconscious_desire": "To be punished for the fire",
                    "pressure_on_protagonist": "Makes the protagonist choose between truth and safety",
                    "behavior_cues": ["counts the exits before she speaks", "refuses praise"]
                }),
            ),
        ],
        PlanningSchedule::default(),
        None,
    )
    .await;
    accept(&svc, &session, &cast).await;

    // ── Series arc: THEME, Main Plot, promise, arc and the ordered Act list ──
    let series = stage(
        &svc,
        &session,
        PlanningLevel::SeriesArc,
        &[&contract, &cast, &spine],
        vec![
            planned(
                "theme:truth",
                "create_theme",
                json!({
                    "theme_statement": "Truth requires risk",
                    "thesis_antithesis": "Safety versus honesty",
                    "introduction_point": {"book_number": 1, "chapter_number": 1, "scene_order": 1},
                    "resolution_point": {"book_number": 1, "chapter_number": 2, "scene_order": 2}
                }),
            ),
            planned(
                "conflict:harbor",
                "create_conflict",
                json!({
                    "name": "The impossible bell",
                    "conflict_type": "mystery",
                    "stakes": "Harbor safety",
                    "escalation_stages": ["clue", "confrontation"]
                }),
            ),
            planned(
                "plot:main",
                "create_plot_line",
                json!({
                    "name": "Find the bell ringer",
                    "plot_type": "main",
                    "summary": "Trace the signal cable",
                    "value_axis": "truth",
                    "is_main_plot": true,
                    "connected_theme_ids": [reference("theme:truth")],
                    "connected_conflict_ids": [reference("conflict:harbor")]
                }),
            ),
            planned(
                "plot:brother",
                "create_plot_line",
                json!({
                    "name": "The brother's silence",
                    "plot_type": "parallel",
                    "summary": "Her brother chooses the harbor over the truth",
                    "value_axis": "loyalty",
                    "subplot_job": "contradict",
                    "is_main_plot": false,
                    "connected_theme_ids": [reference("theme:truth")]
                }),
            ),
            planned(
                "promise:bell",
                "create_narrative_promise",
                json!({
                    "promise_type": "mystery",
                    "description": "Who rang the clapperless bell?",
                    "planted_at": {"book_number": 1, "chapter_number": 1, "scene_order": 1},
                    "planned_payoff": {"book_number": 1, "chapter_number": 2, "scene_order": 2},
                    "planned_payoff_turning_beat": 3,
                    "target_act": 2
                }),
            ),
            planned(
                "arc:mara",
                "create_character_arc",
                json!({
                    "character_id": reference("cast:mara"),
                    "arc_type": "growth",
                    "starting_state": "Guarded",
                    "ending_state": "Resolute",
                    "thematic_purpose": "Risk the truth",
                    "connected_theme_ids": [reference("theme:truth")],
                    "milestones": [
                        {
                            "label": "Inspect the bell",
                            "description": "Mara climbs the lantern tower",
                            "placement": {"book_number": 1, "chapter_number": 1, "scene_order": 2}
                        },
                        {
                            "label": "Ring the signal",
                            "description": "Mara exposes the forged logbook",
                            "placement": {"book_number": 1, "chapter_number": 2, "scene_order": 2}
                        }
                    ]
                }),
            ),
        ],
        PlanningSchedule {
            promise_events: vec![
                PlannedPromiseEvent {
                    promise_ref: PlanningRef::ProposalRef("promise:bell".into()),
                    action: PlannedPromiseAction::Plant,
                    placement: place(1, 1, 1),
                    note: None,
                },
                PlannedPromiseEvent {
                    promise_ref: PlanningRef::ProposalRef("promise:bell".into()),
                    action: PlannedPromiseAction::Payoff,
                    placement: place(1, 2, 2),
                    note: None,
                },
            ],
            acts: vec![
                PlannedAct {
                    order: 1,
                    climax: Some(place(1, 1, 2)),
                    charge: Some(charge(ChargeSign::Negative, ChargeDepth::Contrary)),
                    central_question_part: "Mara learns the bell was rung deliberately".into(),
                    answers_central_question: false,
                    cause: None,
                },
                PlannedAct {
                    order: 2,
                    climax: Some(place(1, 2, 2)),
                    charge: Some(charge(ChargeSign::Negative, ChargeDepth::Contradictory)),
                    central_question_part: "The signal is a confession".into(),
                    answers_central_question: true,
                    cause: None,
                },
            ],
            ..Default::default()
        },
        Some(PlanningTargetRange {
            book_number: 1,
            chapter_start: 1,
            chapter_end: 2,
            scenes_per_chapter: 2,
        }),
    )
    .await;
    accept(&svc, &session, &series).await;

    // ── Book outline ──
    let book = stage(
        &svc,
        &session,
        PlanningLevel::Book,
        &[&contract, &series],
        vec![planned(
            "outline:book",
            "set_book_outline",
            json!({
                "book_id": reference("project:root:book"),
                "book_number": 1,
                "content": "Mara uncovers the signal conspiracy at the watchtower."
            }),
        )],
        PlanningSchedule::default(),
        None,
    )
    .await;
    accept(&svc, &session, &book).await;

    // ── Chapter and its Sequence ──
    let sequence = PlannedSequence {
        sequence_key: "seq:bell".into(),
        plot_line_id: Some(PlanningRef::ProposalRef("plot:main".into())),
        question: "Who rang the impossible bell?".into(),
        inciting_scene: Some(place(1, 1, 1)),
        climax_scene: Some(place(1, 2, 2)),
        member_scenes: vec![
            place(1, 1, 1),
            place(1, 1, 2),
            place(1, 2, 1),
            place(1, 2, 2),
        ],
    };
    let chapter = stage(
        &svc,
        &session,
        PlanningLevel::Chapter,
        &[&contract, &series, &book],
        vec![
            planned(
                "chapter:1",
                "create_chapter",
                json!({
                    "book_id": reference("project:root:book"),
                    "book_number": 1,
                    "chapter_number": 1,
                    "title": "The First Watch"
                }),
            ),
            planned(
                "outline:chapter:1",
                "set_chapter_outline",
                json!({
                    "entity_id": reference("chapter:1"),
                    "book_number": 1,
                    "chapter_number": 1,
                    "content": "Mara hears the impossible bell.",
                    "beats": [{"order": 1, "summary": "Hear the bell", "status": "planned"}]
                }),
            ),
        ],
        PlanningSchedule {
            sequences: vec![sequence],
            ..Default::default()
        },
        None,
    )
    .await;
    accept(&svc, &session, &chapter).await;

    // ── Scene manifest: two planned Scenes with a chained value plan ──
    let scene = stage(
        &svc,
        &session,
        PlanningLevel::Scene,
        &[&cast, &series, &chapter],
        vec![planned(
            "plan:ch1",
            "plan_chapter",
            json!({
                "book_number": 1,
                "chapter_number": 1,
                "pov_character_id": reference("cast:mara"),
                "synopsis": "Mara hears the impossible bell.",
                "target_theme_ids": [reference("theme:truth")],
                "target_conflict_ids": [reference("conflict:harbor")],
                "target_plot_line_ids": [reference("plot:main")],
                "scenes": [
                    {
                        "scene_order": 1,
                        "summary": "Mara hears the bronze bell.",
                        "beat_structure": ["hook", "turn"],
                        "beats": scene_beats(),
                        "turning_beat": 3,
                        "character_ids": [reference("cast:mara")],
                        "location_id": reference("loc:watchtower"),
                        "content_rating": "general",
                        "purpose": "Plant the impossible bell mystery",
                        "primary_value_axis": value_axis("positive", "positive", "negative", "contrary"),
                        "opposition": "The silent bell tower",
                        "gap": {"expected": "The bell is silent", "actual": "It rings a warning only Mara hears"},
                        "scene_question": "Who rang the clapperless bell?",
                        "inaction_cost": "The harbor misses the warning",
                        "risk": 1
                    },
                    {
                        "scene_order": 2,
                        "summary": "Mara climbs to the lantern room.",
                        "beat_structure": ["escalation", "decision"],
                        "beats": scene_beats(),
                        "turning_beat": 3,
                        "character_ids": [reference("cast:mara")],
                        "location_id": reference("loc:watchtower"),
                        "content_rating": "general",
                        "purpose": "Confirm the bell clapper is missing",
                        "primary_value_axis": value_axis("negative", "contrary", "positive", "positive"),
                        "opposition": "The fog on the stair",
                        "gap": {"expected": "The clapper was stolen", "actual": "The clapper was never fitted"},
                        "scene_question": "Will Mara trust the impossible sound?",
                        "inaction_cost": "The warning dies with the fog",
                        "risk": 2,
                        "dilemma": {"character_id": reference("cast:mara"), "contradiction": "guarded"}
                    }
                ]
            }),
        )],
        PlanningSchedule::default(),
        Some(PlanningTargetRange {
            book_number: 1,
            chapter_start: 1,
            chapter_end: 1,
            scenes_per_chapter: 2,
        }),
    )
    .await;
    accept(&svc, &session, &scene).await;

    // The accepted plan is readable: two complete planned Scenes, the Act list
    // and the Spine all made it into the Story Bible.
    let plans = svc
        .repository()
        .list_chapter_plans_by_project_and_branch(&project_id, &scope.branch_id)
        .await
        .unwrap();
    let planned_chapter = plans
        .iter()
        .find(|plan| plan.book_number == 1 && plan.chapter_number == 1)
        .expect("the accepted scene manifest is readable");
    assert_eq!(planned_chapter.scenes.len(), 2);
    assert!(planned_chapter.scenes[0].primary_value_axis.is_some());
    let acts = svc
        .repository()
        .accepted_planning_acts(&project_id, &scope.branch_id)
        .await
        .unwrap();
    assert_eq!(acts.len(), 2);

    // ── Commit the first planned Scene with its realized report ──
    let branch = svc
        .repository()
        .get_active_branch(&project_id)
        .await
        .unwrap();
    let promises = svc
        .repository()
        .list_narrative_promises_by_project_and_branch(&project_id, &branch.id)
        .await
        .unwrap();
    let bell_promise = promises
        .iter()
        .find(|promise| promise.description.contains("clapperless bell"))
        .expect("the accepted promise is readable")
        .id
        .clone();

    let realized = RealizedReport {
        value_turns: vec![RealizedValueTurn {
            axis: "truth".into(),
            opening: charge(ChargeSign::Positive, ChargeDepth::Positive),
            closing: charge(ChargeSign::Negative, ChargeDepth::Contrary),
            gap: Some(SceneGap {
                expected: "The bell is silent".into(),
                actual: "It rings a warning only Mara hears".into(),
            }),
            turning_beat: Some(3),
            turning_beat_summary: None,
        }],
        promise_events: vec![RealizedPromiseEvent {
            narrative_promise_id: bell_promise,
            event: RealizedPromiseEventKind::Setup,
            note: None,
        }],
        ..Default::default()
    };
    let commit = svc
        .commit_scene(CommitSceneInput {
            project_id: project_id.clone(),
            branch_id: None,
            preparation_id: None,
            context_revision_token: None,
            dependency_tokens: None,
            scene_id: None,
            book_number: Some(1),
            chapter_number: Some(1),
            scene_order: Some(1),
            expected_scene_version: None,
            full_text: "Fog pressed the watchtower glass while Mara counted the tide tables for the \
                        third time that morning; the bell above her had no clapper, yet it rang once, \
                        low and certain, and the harbor below went still as if every door had been \
                        shut at the same instant. She set down her pen, crossed the narrow room, and \
                        put her palm against the cold bronze, and it rang again, a warning only she \
                        could hear, and she understood that the warning was the point."
                .into(),
            summary: "Mara hears a bell whose clapper was never fitted.".into(),
            content_rating: ContentRating::General,
            location_id: None,
            tone: None,
            authorship: SceneAuthorship::Assistant,
            generation_receipt: None,
            continuity: CommitSceneContinuityPackage {
                realized_report: Some(realized),
                ..Default::default()
            },
            beat_annotations: Vec::new(),
            chapter_summary: None,
            run_id: None,
            expected_phase: None,
            dry_run: false,
            idempotency_key: Some("mckee-e2e-scene-1-1".into()),
        })
        .await
        .unwrap();
    assert_eq!(
        commit.blocking_count, 0,
        "a realized report that matches the plan must commit clean: {:?}",
        commit.issues
    );
    let stored = svc.repository().get_scene(&commit.scene_id).await.unwrap();
    assert!(
        stored.realized_report.is_some(),
        "the committed Scene must persist the realized report"
    );

    // ── Deterministic McKee verify is clean for the committed Scene ──
    let verify = svc
        .check_consistency(CheckConsistencyInput {
            deep_scan_offset: None,
            project_id: project_id.clone(),
            scope: ConsistencyScopeInput {
                scope_type: "chapter_range".into(),
                book_number: None,
                start_book_number: Some(1),
                start_chapter_number: Some(1),
                end_book_number: Some(1),
                end_chapter_number: Some(1),
                scene_order: Some(1),
            },
            checks: vec![
                "realized_value_turn".into(),
                "realized_key_fact".into(),
                "realized_promise_event".into(),
                "character_effect".into(),
            ],
            severity_filter: Vec::new(),
            deep_check: Some(false),
            subjects: Vec::new(),
            format: None,
            budget_tokens: None,
        })
        .await
        .unwrap();
    let realized_findings: Vec<_> = verify
        .issues
        .iter()
        .filter(|issue| {
            issue.check_type.starts_with("realized_") || issue.check_type == "character_effect"
        })
        .collect();
    assert!(
        realized_findings.is_empty(),
        "the committed Scene must verify clean: {realized_findings:?}"
    );

    // ── The checkpoint advisory reads planning intent + written prose ──
    let advisory = svc
        .checkpoint_advisory(CheckpointAdvisoryInput {
            project_id: project_id.clone(),
            book_number: 1,
            through_chapter: 1,
        })
        .await
        .unwrap()
        .expect("a McKee project always answers the checkpoint questions");
    assert_eq!(
        advisory.central_question.question,
        "Can Ada publish the truth before the harbor closes?"
    );
    assert_eq!(advisory.central_question.acts_total, 2);
    assert_eq!(
        advisory.central_question.acts_answered, 0,
        "Act 1's climax Scene (1.2) is planned but unwritten, so nothing is answered yet: {:?}",
        advisory.central_question
    );
    assert_eq!(
        advisory.central_question.acts_planned_reached, 1,
        "the planning-only position signal stays separate and labelled: {:?}",
        advisory.central_question
    );
    assert!(
        advisory.central_question.answered_parts.is_empty(),
        "planned parts must never be reported as answered: {:?}",
        advisory.central_question
    );
    assert_eq!(
        advisory.central_question.remaining_parts,
        vec![
            "Mara learns the bell was rung deliberately",
            "The signal is a confession"
        ]
    );
    assert_eq!(
        advisory.controlling_idea_debate.reader_side,
        ControllingIdeaDebateSide::CounterIdea,
        "the written turn closes negative: {:?}",
        advisory.controlling_idea_debate
    );
    assert!(
        advisory.dormant_subplots.is_empty(),
        "one read Chapter cannot fill the dormancy window yet: {:?}",
        advisory.dormant_subplots
    );

    // ── Dormancy: three read Chapters without a Subplot turn ──
    // Chapters 2 and 3 carry written prose but never turn the brother's
    // "loyalty" axis, so the Subplot is listed at the chapter-3 checkpoint.
    for number in 2..=3 {
        svc.create_chapter(CreateChapterInput {
            project_id: project_id.clone(),
            book_number: Some(1),
            book_id: None,
            chapter_number: Some(number),
            title: None,
        })
        .await
        .unwrap();
        svc.save_scene_draft(SaveSceneDraftInput {
            project_id: project_id.clone(),
            book_number: 1,
            chapter_number: number,
            chapter_id: None,
            scene_order: 1,
            full_text: format!(
                "Chapter {number}: the harbor keeps its silence while Mara reads the tide tables."
            ),
            summary: format!("A read chapter {number} with no loyalty turn."),
            content_rating: ContentRating::General,
            realized_report: Some(RealizedReport {
                value_turns: vec![RealizedValueTurn {
                    axis: "truth".into(),
                    opening: charge(ChargeSign::Positive, ChargeDepth::Positive),
                    closing: charge(ChargeSign::Negative, ChargeDepth::Contrary),
                    gap: None,
                    turning_beat: None,
                    turning_beat_summary: None,
                }],
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .unwrap();
    }

    let at_three = svc
        .checkpoint_advisory(CheckpointAdvisoryInput {
            project_id: project_id.clone(),
            book_number: 1,
            through_chapter: 3,
        })
        .await
        .unwrap()
        .expect("a McKee project always answers the checkpoint questions");
    assert_eq!(
        at_three.central_question.acts_answered, 0,
        "still no climax Scene is written: {:?}",
        at_three.central_question
    );
    assert_eq!(
        at_three.central_question.acts_planned_reached, 2,
        "both planned climaxes are inside the chapter-3 range: {:?}",
        at_three.central_question
    );
    assert_eq!(
        at_three.dormant_subplots.len(),
        1,
        "the never-turning Subplot must be listed once three chapters are read: {:?}",
        at_three.dormant_subplots
    );
    let dormant = &at_three.dormant_subplots[0];
    assert_eq!(dormant.name, "The brother's silence");
    assert_eq!(dormant.value_axis, "loyalty");
    assert_eq!(dormant.last_turn_book, None);
    assert_eq!(dormant.last_turn_chapter, None);
    assert_eq!(
        dormant.chapters_without_turn, 3,
        "chapters 1-3 were read without a loyalty turn"
    );
}
