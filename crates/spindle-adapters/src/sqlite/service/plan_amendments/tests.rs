use super::*;

// ── Replan differ (replan_chapter — evolution §3.5, ADR 0003) ────────────

/// Per-class payload validation units (ADR 0003 D1): one valid + one invalid
/// payload per class. This is the Part B contract — the payload shapes are
/// pinned here so a later apply dispatcher can rely on them.
#[test]
fn plan_amendment_payload_validation_per_class() {
    use serde_json::json;
    // (class, valid payload, invalid payload)
    let cases: [(&str, serde_json::Value, serde_json::Value); 8] = [
        (
            "synopsis_update",
            json!({"synopsis": "The gate has already fallen."}),
            json!({"synopsis": "   "}), // empty
        ),
        (
            "scene_add",
            json!({"scene_order": 2, "summary": "s", "purpose": "p", "character_ids": [], "insert_at": 1}),
            json!({"scene_order": 2, "summary": "s", "purpose": "p"}), // missing character_ids + insert_at
        ),
        (
            "scene_drop",
            json!({"scene_order": 3}),
            json!({"scene_order": "not-a-number"}),
        ),
        (
            "scene_replace",
            json!({"scene_order": 1, "summary": "s", "purpose": "p", "character_ids": ["character:x"]}),
            json!({"scene_order": 1, "purpose": "p", "character_ids": []}), // missing summary
        ),
        (
            "scene_reorder",
            json!({"new_order": [3, 1, 2]}),
            json!({"new_order": []}), // empty permutation
        ),
        (
            "thread_promote",
            json!({"kind": "theme", "id": "theme:x"}),
            json!({"kind": "invalid", "id": "theme:x"}), // bad kind
        ),
        (
            "thread_retire",
            json!({"kind": "conflict", "id": "conflict:x"}),
            json!({"kind": "conflict", "id": ""}), // empty id
        ),
        (
            "promise_followup",
            json!({"promise_type": "mystery", "description": "d", "planted_at_placement": {"book_number": 1, "chapter_number": 6}}),
            json!({"promise_type": "mystery", "description": "d"}), // missing placement
        ),
    ];
    for (class, valid, invalid) in cases {
        assert!(
            plan_amendment_payload_is_valid(class, &valid),
            "{class} valid payload must pass"
        );
        assert!(
            !plan_amendment_payload_is_valid(class, &invalid),
            "{class} invalid payload must be rejected"
        );
    }
    // An unknown class never validates regardless of payload.
    assert!(!plan_amendment_payload_is_valid(
        "made_up",
        &serde_json::json!({})
    ));
}

#[test]
fn replan_prompt_renders_realized_and_target_sections_deterministically() {
    // Pure render: the realized digest surfaces the source chapter's summary
    // + list fields + ids; the target section names only the future chapter
    // number and its plan_revision. No I/O.
    let realized = ReplanRealizedDigest {
        source_chapter: 3,
        summary: "The gate fell early.".to_string(),
        key_events: vec!["gate breached".to_string()],
        character_changes: Vec::new(),
        relationship_shifts: Vec::new(),
        arc_advances: Vec::new(),
        promise_events: Vec::new(),
        beats: vec![ReplanBeatDigest {
            scene_order: 1,
            motif_ids: vec!["motif:ash".to_string()],
            theme_ids: Vec::new(),
            conflict_ids: vec!["conflict:siege".to_string()],
            intensity: Some(0.8),
        }],
        promise_states: vec![(
            "narrative_promise:door".to_string(),
            "planted".to_string(),
            "soon".to_string(),
        )],
        arc_states: vec![("character_arc:mara".to_string(), "active".to_string(), 0.5)],
    };
    let realized_rendered = build_replan_realized_section(&realized);
    assert!(realized_rendered.contains("REALIZED REALITY of chapter 3"));
    assert!(realized_rendered.contains("The gate fell early."));
    assert!(realized_rendered.contains("gate breached"));
    assert!(realized_rendered.contains("motif:ash"));
    assert!(realized_rendered.contains("conflict:siege"));
    assert!(realized_rendered.contains("narrative_promise:door"));
    assert!(realized_rendered.contains("character_arc:mara"));

    let targets = vec![ReplanTargetDigest {
        chapter_number: 5,
        synopsis: "Chapter five as planned.".to_string(),
        scenes: vec![ReplanTargetSceneDigest {
            scene_order: 1,
            summary: "opening".to_string(),
            purpose: "establish".to_string(),
            character_ids: vec!["character:x".to_string()],
            location_id: Some("location:hall".to_string()),
            content_rating: Some("general".to_string()),
        }],
        target_theme_ids: vec!["theme:loss".to_string()],
        target_conflict_ids: Vec::new(),
        target_plot_line_ids: Vec::new(),
        plan_revision: 0,
    }];
    let targets_rendered = build_replan_targets_section(&targets);
    assert!(targets_rendered.contains("Chapter 5 (plan_revision 0)"));
    assert!(targets_rendered.contains("Chapter five as planned."));
    assert!(targets_rendered.contains("theme:loss"));
    assert!(targets_rendered.contains("character:x"));
    assert!(targets_rendered.contains("location:hall"));
}

#[tokio::test]
async fn replan_chapter_stages_amendments_supersedes_on_rerun_and_counts_discards() {
    use spindle_core::models::ReplanChapterInput;
    let (_tmp, svc) = fresh_service_local().await;
    // Target chapter 5's synopsis carries the happy sentinel (chapter 5) and
    // the discard sentinel (MOCK_REPLAN_BAD) so the stub returns 2 valid + 2
    // discardable amendments in one pass.
    let project = project_with_target_plan(
        &svc,
        "Replan Happy",
        5,
        "MOCK_REPLAN_SYNOPSIS[5] MOCK_REPLAN_BAD chapter five synopsis",
    )
    .await;
    save_source_summary(&svc, &project.project_id, 3).await;

    let out = svc
        .replan_chapter(ReplanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            source_chapter: 3,
        })
        .await
        .unwrap();
    assert_eq!(out.status, "staged");
    assert_eq!(out.superseded_count, 0);
    assert_eq!(out.staged.len(), 2, "synopsis_update + thread_retire stage");
    assert_eq!(
        out.discarded_count, 2,
        "unknown class + empty rationale discarded"
    );
    // Every staged amendment carries a non-empty rationale.
    assert!(out.staged.iter().all(|a| !a.rationale.trim().is_empty()));
    let classes: Vec<&str> = out
        .staged
        .iter()
        .map(|a| a.amendment_class.as_str())
        .collect();
    assert!(classes.contains(&"synopsis_update"));
    assert!(classes.contains(&"thread_retire"));
    assert!(out.staged.iter().all(|a| a.target_chapter == Some(5)));

    // Rerun supersedes the prior staged rows for the same source chapter.
    let rerun = svc
        .replan_chapter(ReplanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            source_chapter: 3,
        })
        .await
        .unwrap();
    assert_eq!(rerun.status, "staged");
    assert_eq!(
        rerun.superseded_count, 2,
        "the prior pass's 2 staged rows superseded"
    );
    assert_eq!(rerun.staged.len(), 2);
}

#[tokio::test]
async fn replan_chapter_skips_when_source_has_no_summary() {
    use spindle_core::models::ReplanChapterInput;
    let (_tmp, svc) = fresh_service_local().await;
    let project =
        project_with_target_plan(&svc, "Replan NoSummary", 5, "MOCK_REPLAN_SYNOPSIS[5] x").await;
    // No summary saved for chapter 3.
    let out = svc
        .replan_chapter(ReplanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            source_chapter: 3,
        })
        .await
        .unwrap();
    assert_eq!(out.status, "no_summary");
    assert!(out.staged.is_empty());
    assert_eq!(out.superseded_count, 0);
}

#[tokio::test]
async fn replan_chapter_skips_when_no_eligible_targets() {
    use spindle_core::models::ReplanChapterInput;
    let (_tmp, svc) = fresh_service_local().await;
    // Only a plan for chapter 2 (BEFORE the source chapter 3) — no target
    // strictly after chapter 3.
    let project = project_with_target_plan(&svc, "Replan NoTargets", 2, "earlier chapter").await;
    save_source_summary(&svc, &project.project_id, 3).await;
    let out = svc
        .replan_chapter(ReplanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            source_chapter: 3,
        })
        .await
        .unwrap();
    assert_eq!(out.status, "no_targets");
    assert!(out.staged.is_empty());
}

#[tokio::test]
async fn replan_chapter_excludes_drafted_target_at_staging() {
    use spindle_core::models::ReplanChapterInput;
    let (_tmp, svc) = fresh_service_local().await;
    // Plan chapter 5 (the only future chapter) but then draft a scene in it —
    // it must drop out of the eligible set, leaving no targets.
    let project = project_with_target_plan(
        &svc,
        "Replan Drafted",
        5,
        "MOCK_REPLAN_SYNOPSIS[5] chapter five",
    )
    .await;
    save_source_summary(&svc, &project.project_id, 3).await;
    svc.save_scene_draft(SaveSceneDraftInput {
        project_id: project.project_id.clone(),
        book_number: 1,
        chapter_number: 5,
        chapter_id: None,
        scene_order: 1,
        full_text: "chapter five already has drafted prose".to_string(),
        summary: "drafted".to_string(),
        content_rating: spindle_core::models::ContentRating::General,
        tone: None,
        generation_id: None,
        source_path: None,
        ..Default::default()
    })
    .await
    .unwrap();

    let out = svc
        .replan_chapter(ReplanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            source_chapter: 3,
        })
        .await
        .unwrap();
    assert_eq!(
        out.status, "no_targets",
        "a drafted target is excluded at staging (ADR D3 immutability sanity)"
    );
    assert!(out.staged.is_empty());
}

#[tokio::test]
async fn replan_chapter_dispatch_falls_to_review_route() {
    // `replan` is absent from default_routes; the ladder must fall to
    // `review`. We prove it via the dispatch recorder: the dispatch went to
    // the `review` route and never to a `replan` route.
    use spindle_core::models::ReplanChapterInput;
    let (_tmp, svc) = fresh_service_local().await;
    let log = svc
        .repository
        .model_router()
        .install_dispatch_recorder()
        .unwrap();

    let project = project_with_target_plan(
        &svc,
        "Replan Ladder",
        5,
        "MOCK_REPLAN_SYNOPSIS[5] chapter five",
    )
    .await;
    save_source_summary(&svc, &project.project_id, 3).await;
    let out = svc
        .replan_chapter(ReplanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            source_chapter: 3,
        })
        .await
        .unwrap();
    assert_eq!(out.status, "staged");

    let records = log.lock().unwrap().clone();
    let dispatched_routes: Vec<String> = records
        .iter()
        .filter_map(|r| match r {
            crate::ai::DispatchRecord::Dispatch { route, .. } => Some(route.clone()),
            _ => None,
        })
        .collect();
    assert!(
        dispatched_routes.iter().any(|r| r == "review"),
        "the replan dispatch fell to the review route: {dispatched_routes:?}"
    );
    assert!(
        !dispatched_routes.iter().any(|r| r == "replan"),
        "replan is not a real route and never dispatches: {dispatched_routes:?}"
    );
}

#[tokio::test]
async fn replan_chapter_skips_on_transport_error() {
    // Config a review agent whose endpoint fails so the ladder's review leg
    // errors → honest `skipped` status (never an error return, never blocks).
    use spindle_core::models::{ConfigureAgentsInput, ReplanChapterInput};
    let (tmp, svc) = fresh_service_local().await;
    let config_path = tmp.path().join("replan-routes.toml");
    std::fs::write(
        &config_path,
        r#"
[health_check]
enabled = false

[[agents]]
id = "review-dead"
name = "Review Dead"
provider = "openai_compatible"
endpoint = "http://127.0.0.1:1/unreachable"
model = "dead"
ratings = ["general", "teen", "mature", "explicit"]

[[routing]]
route = "review"
agent = "review-dead"
"#,
    )
    .unwrap();
    svc.configure_agents(ConfigureAgentsInput {
        config_path: Some(config_path.display().to_string()),
    })
    .unwrap();

    let project = project_with_target_plan(
        &svc,
        "Replan Dead",
        5,
        "MOCK_REPLAN_SYNOPSIS[5] chapter five",
    )
    .await;
    save_source_summary(&svc, &project.project_id, 3).await;
    let out = svc
        .replan_chapter(ReplanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            source_chapter: 3,
        })
        .await
        .unwrap();
    assert_eq!(out.status, "skipped");
    assert!(out.staged.is_empty());
    let reason = out.skip_reason.expect("skip reason present");
    assert!(
        reason.contains("replan SKIPPED"),
        "honest skip reason: {reason}"
    );
}

#[tokio::test]
async fn replan_chapter_caps_staged_at_eight_and_reports_dropped() {
    use spindle_core::models::ReplanChapterInput;
    let (_tmp, svc) = fresh_service_local().await;
    // The cap sentinel makes the stub return 10 valid synopsis_update
    // amendments for chapter 5; the differ stages 8 and drops 2.
    let project =
        project_with_target_plan(&svc, "Replan Cap", 5, "MOCK_REPLAN_CAP[5] chapter five").await;
    save_source_summary(&svc, &project.project_id, 3).await;
    let out = svc
        .replan_chapter(ReplanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            source_chapter: 3,
        })
        .await
        .unwrap();
    assert_eq!(out.status, "staged");
    assert_eq!(out.staged.len(), 8, "per-pass cap");
    assert_eq!(
        out.discarded_count, 2,
        "overflow beyond the cap is dropped + counted"
    );
}

// ── Plan-amendment ratification (list_plan_amendments +
//    decide_plan_amendments) — ADR 0003 D3/D4/D5. Exercises the apply
//    dispatcher: pre-flight-all, the immutability guard, per-class replay
//    through the plan write path, prior_state capture + plan_revision
//    increment, and the honesty guarantees (atomicity, mid-apply failure).
mod plan_ratify {
    use super::*;
    use crate::sqlite::repository::StagePlanAmendmentParams;
    use spindle_core::models::{
        DecidePlanAmendmentsInput, ListPlanAmendmentsInput, PlanAmendmentDecisionInput,
        PlanChapterInput, PlanChapterSceneInput,
    };

    /// Create a project, then plan a future chapter (`chapter_number`) with
    /// the given scenes, plus a source chapter summary so the outline has a
    /// stable base for amendments. Returns the project.
    async fn project_with_future_chapter(
        svc: &SqliteSpindleService,
        name: &str,
        chapter_number: i32,
        scenes: Vec<PlanChapterSceneInput>,
        target_theme_ids: Vec<String>,
    ) -> CreateProjectOutput {
        let project = svc
            .create_project(CreateProjectInput {
                name: name.to_string(),
                project_type: "novel".to_string(),
                genre: "fantasy".to_string(),
                reader_contract: ReaderContract {
                    promise: "living outline".to_string(),
                    style_notes: Vec::new(),
                    boundaries: Vec::new(),
                },
            })
            .await
            .unwrap();
        svc.plan_chapter(PlanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            chapter_number,
            pov_character_id: None,
            synopsis: "the planned future".to_string(),
            target_theme_ids,
            target_conflict_ids: Vec::new(),
            target_plot_line_ids: Vec::new(),
            scenes,
        })
        .await
        .unwrap();
        project
    }

    fn scene(order: i32, summary: &str) -> PlanChapterSceneInput {
        PlanChapterSceneInput {
            scene_order: order,
            summary: summary.to_string(),
            beat_structure: Vec::new(),
            beats: Vec::new(),
            turning_beat: None,
            character_ids: Vec::new(),
            location_id: None,
            content_rating: None,
            purpose: format!("purpose {order}"),
            research_required: None,
            research_tags: Vec::new(),
            explicit_query: None,
            primary_value_axis: None,
            secondary_value_axes: Vec::new(),
            opposition: None,
            gap: None,
            scene_question: None,
            inaction_cost: None,
            risk: None,
            dilemma: None,
        }
    }

    /// A minimal valid charge pair for the McKee amendment fixtures.
    fn value_charge(
        sign: spindle_core::charge::ChargeSign,
        depth: spindle_core::charge::ChargeDepth,
    ) -> spindle_core::charge::Charge {
        spindle_core::charge::Charge::new(sign, depth).expect("valid charge pair")
    }

    fn value_axis(
        opening: spindle_core::charge::Charge,
        closing: spindle_core::charge::Charge,
    ) -> spindle_core::scene_value::SceneValueAxis {
        spindle_core::scene_value::SceneValueAxis {
            axis: "truth".into(),
            opening,
            closing,
        }
    }

    /// Action plus one Turn Beat for the McKee amendment fixtures.
    fn value_beats() -> Vec<spindle_core::beats::PlannedBeat> {
        use spindle_core::beats::{BeatKind, PlannedBeat};
        use spindle_core::scene_value::SceneGap;
        vec![
            PlannedBeat {
                kind: BeatKind::Action,
                actor_character_id: Some("character:alice".into()),
                tactic: "Ask for the sealed ledger".into(),
                said: "Show me the ledger.".into(),
                wanted: "The truth of the forged seal".into(),
                gap: SceneGap {
                    expected: "The ledger clears her".into(),
                    actual: "A page is missing".into(),
                },
                swain_label: None,

                information_dependent: false,
                promise_id: None,
            },
            PlannedBeat {
                kind: BeatKind::Turn,
                actor_character_id: Some("character:alice".into()),
                tactic: "Name the forger before the hall".into(),
                said: "Then the hall forged its own record.".into(),
                wanted: "A truth the hall cannot bury".into(),
                gap: SceneGap {
                    expected: "The hall hears her".into(),
                    actual: "The hall orders her silence".into(),
                },
                swain_label: None,

                information_dependent: false,
                promise_id: None,
            },
        ]
    }

    /// A scene carrying a complete, shape-valid McKee value plan.
    fn value_scene(
        order: i32,
        summary: &str,
        opening: spindle_core::charge::Charge,
        closing: spindle_core::charge::Charge,
    ) -> PlanChapterSceneInput {
        PlanChapterSceneInput {
            scene_order: order,
            summary: summary.to_string(),
            beat_structure: vec!["turn".into()],
            beats: value_beats(),
            turning_beat: Some(2),
            character_ids: vec!["character:alice".to_string()],
            location_id: Some("location:hall".to_string()),
            content_rating: Some(spindle_core::models::ContentRating::General),
            purpose: format!("turn the truth axis in {order}"),
            research_required: None,
            research_tags: Vec::new(),
            explicit_query: None,
            primary_value_axis: Some(value_axis(opening, closing)),
            secondary_value_axes: Vec::new(),
            opposition: Some("the rival".into()),
            gap: Some(spindle_core::scene_value::SceneGap {
                expected: "a welcome".into(),
                actual: "a refusal".into(),
            }),
            scene_question: Some("Will the truth hold?".into()),
            inaction_cost: Some("the harbor loses its warning".into()),
            risk: Some(order),
            dilemma: Some(spindle_core::scene_value::SceneDilemma {
                character_id: "character:alice".into(),
                contradiction: "guarded".into(),
            }),
        }
    }

    /// A McKee-marked project with one planned, undrafted future chapter whose
    /// scenes carry full value plans.
    async fn mckee_project_with_value_plan(
        svc: &SqliteSpindleService,
        name: &str,
        chapter_number: i32,
    ) -> CreateProjectOutput {
        use spindle_core::charge::{ChargeDepth, ChargeSign};
        let project = svc
            .create_mckee_project(CreateProjectInput {
                name: name.to_string(),
                project_type: "novel".to_string(),
                genre: "fantasy".to_string(),
                reader_contract: ReaderContract {
                    promise: "McKee living outline".to_string(),
                    style_notes: Vec::new(),
                    boundaries: Vec::new(),
                },
            })
            .await
            .unwrap();
        svc.plan_chapter(PlanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            chapter_number,
            pov_character_id: None,
            synopsis: "the planned McKee future".to_string(),
            target_theme_ids: Vec::new(),
            target_conflict_ids: Vec::new(),
            target_plot_line_ids: Vec::new(),
            scenes: vec![
                value_scene(
                    1,
                    "open",
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                ),
                value_scene(
                    2,
                    "close",
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                ),
            ],
        })
        .await
        .unwrap();
        project
    }

    /// Stage one plan amendment directly on the active branch (bypassing the
    /// model) so the dispatcher can be driven deterministically per class.
    async fn stage(
        svc: &SqliteSpindleService,
        project_id: &str,
        class: &str,
        target_chapter: Option<i32>,
        payload: serde_json::Value,
    ) -> String {
        let branch = svc
            .repository()
            .get_active_branch(project_id)
            .await
            .unwrap();
        svc.repository()
            .stage_plan_amendment(StagePlanAmendmentParams {
                project_id: project_id.to_string(),
                branch_id: branch.id,
                source_chapter: 3,
                book_number: 1,
                authoring_run_id: None,
                amendment_class: class.to_string(),
                target_chapter,
                payload,
                rationale: format!("{class}: realized event id x moved the outline"),
                confidence: "high".to_string(),
            })
            .await
            .unwrap()
            .id
    }

    /// Read the current plan for a chapter (active branch) or panic.
    async fn plan_of(
        svc: &SqliteSpindleService,
        project_id: &str,
        chapter: i32,
    ) -> crate::sqlite::records::ChapterPlan {
        let plans = svc
            .repository()
            .list_chapter_plans_by_project(project_id)
            .await
            .unwrap();
        plans
            .into_iter()
            .find(|p| p.chapter_number == chapter)
            .expect("plan for chapter")
    }

    async fn apply_one(
        svc: &SqliteSpindleService,
        project_id: &str,
        amendment_id: &str,
    ) -> spindle_core::models::DecidePlanAmendmentsOutput {
        svc.decide_plan_amendments(DecidePlanAmendmentsInput {
            project_id: project_id.to_string(),
            decisions: vec![PlanAmendmentDecisionInput {
                amendment_id: amendment_id.to_string(),
                action: "apply".to_string(),
                edit: None,
                note: None,
            }],
            decided_by: None,
        })
        .await
        .unwrap()
    }

    // ── Class round-trips (Test 1) ───────────────────────────────────────

    #[tokio::test]
    async fn apply_synopsis_update_rewrites_synopsis_and_bumps_revision() {
        let (_tmp, svc) = fresh_service_local().await;
        let project =
            project_with_future_chapter(&svc, "PA synopsis", 5, vec![scene(1, "a")], Vec::new())
                .await;
        let before = plan_of(&svc, &project.project_id, 5).await;
        let id = stage(
            &svc,
            &project.project_id,
            "synopsis_update",
            Some(5),
            serde_json::json!({"synopsis": "the gate has already fallen"}),
        )
        .await;

        let out = apply_one(&svc, &project.project_id, &id).await;
        assert_eq!(out.applied_count, 1);
        assert_eq!(out.results[0].outcome, "applied");

        let after = plan_of(&svc, &project.project_id, 5).await;
        assert_eq!(after.synopsis, "the gate has already fallen");
        // plan_revision NULL/0 → 1.
        assert_eq!(after.plan_revision, Some(1));
        // prior_state snapshot equals the pre-apply row.
        let amendments = svc
            .list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project.project_id.clone(),
                status: Some("applied".to_string()),
                book_number: None,
                source_chapter: None,
            })
            .await
            .unwrap();
        let prior = amendments.amendments[0]
            .prior_state
            .as_deref()
            .expect("prior_state captured");
        let prior_plan: crate::sqlite::records::ChapterPlan = serde_json::from_str(prior).unwrap();
        assert_eq!(prior_plan.synopsis, before.synopsis);
        assert_eq!(prior_plan.plan_revision, before.plan_revision);
    }

    #[tokio::test]
    async fn apply_scene_add_inserts_and_renumbers() {
        let (_tmp, svc) = fresh_service_local().await;
        let project = project_with_future_chapter(
            &svc,
            "PA add",
            5,
            vec![scene(1, "first"), scene(2, "second")],
            Vec::new(),
        )
        .await;
        // Insert a new scene at position 2 (1-based insert_at), pushing the
        // old scene 2 to order 3.
        let id = stage(
            &svc,
            &project.project_id,
            "scene_add",
            Some(5),
            serde_json::json!({
                "scene_order": 99, "summary": "inserted", "purpose": "bridge",
                "character_ids": [], "insert_at": 2
            }),
        )
        .await;
        let out = apply_one(&svc, &project.project_id, &id).await;
        assert_eq!(out.results[0].outcome, "applied", "{out:?}");

        let after = plan_of(&svc, &project.project_id, 5).await;
        let summaries: Vec<(i32, String)> = after
            .scenes
            .iter()
            .map(|s| (s.scene_order, s.summary.clone()))
            .collect();
        assert_eq!(
            summaries,
            vec![
                (1, "first".to_string()),
                (2, "inserted".to_string()),
                (3, "second".to_string()),
            ],
            "insert_at renumbers the spine 1..n"
        );
        assert_eq!(after.plan_revision, Some(1));
    }

    #[tokio::test]
    async fn apply_scene_add_carries_structured_beats() {
        let (_tmp, svc) = fresh_service_local().await;
        let project = project_with_future_chapter(
            &svc,
            "PA add beats",
            5,
            vec![scene(1, "first")],
            Vec::new(),
        )
        .await;
        // A non-McKee project may still carry structured Beats; `scene_add`
        // must insert the Beat list and the 1-based Turning Beat instead of
        // defaulting them away.
        let id = stage(
            &svc,
            &project.project_id,
            "scene_add",
            Some(5),
            serde_json::json!({
                "scene_order": 99,
                "summary": "inserted with beats",
                "purpose": "carry structured Beats through scene_add",
                "character_ids": ["character:alice"],
                "insert_at": 2,
                "beats": [
                    {
                        "kind": "action",
                        "actor_character_id": "character:alice",
                        "tactic": "Ask for the sealed ledger",
                        "said": "Show me the ledger.",
                        "wanted": "The truth of the forged seal",
                        "gap": {"expected": "The ledger clears her", "actual": "A page is missing"}
                    },
                    {
                        "kind": "turn",
                        "actor_character_id": "character:alice",
                        "tactic": "Name the forger before the hall",
                        "said": "Then the hall forged its own record.",
                        "wanted": "A truth the hall cannot bury",
                        "gap": {"expected": "The hall hears her", "actual": "The hall orders her silence"},
                        "swain_label": "conflict"
                    }
                ],
                "turning_beat": 2
            }),
        )
        .await;
        let out = apply_one(&svc, &project.project_id, &id).await;
        assert_eq!(out.results[0].outcome, "applied", "{out:?}");

        let after = plan_of(&svc, &project.project_id, 5).await;
        let inserted = after
            .scenes
            .iter()
            .find(|scene| scene.summary == "inserted with beats")
            .expect("the inserted scene is stored");
        assert_eq!(inserted.scene_order, 2);
        assert_eq!(
            inserted.beats.len(),
            2,
            "scene_add must not drop a non-McKee project's structured Beats"
        );
        assert_eq!(inserted.beats[1].kind, spindle_core::beats::BeatKind::Turn);
        assert_eq!(
            inserted.beats[0].actor_character_id.as_deref(),
            Some("character:alice")
        );
        assert_eq!(inserted.turning_beat, Some(2), "the Turning Beat is kept");
    }

    #[tokio::test]
    async fn apply_scene_drop_removes_and_renumbers() {
        let (_tmp, svc) = fresh_service_local().await;
        let project = project_with_future_chapter(
            &svc,
            "PA drop",
            5,
            vec![scene(1, "keep-a"), scene(2, "drop-me"), scene(3, "keep-b")],
            Vec::new(),
        )
        .await;
        let id = stage(
            &svc,
            &project.project_id,
            "scene_drop",
            Some(5),
            serde_json::json!({"scene_order": 2}),
        )
        .await;
        let out = apply_one(&svc, &project.project_id, &id).await;
        assert_eq!(out.results[0].outcome, "applied", "{out:?}");

        let after = plan_of(&svc, &project.project_id, 5).await;
        let summaries: Vec<(i32, String)> = after
            .scenes
            .iter()
            .map(|s| (s.scene_order, s.summary.clone()))
            .collect();
        assert_eq!(
            summaries,
            vec![(1, "keep-a".to_string()), (2, "keep-b".to_string())],
            "drop renumbers the survivors 1..n"
        );
    }

    #[tokio::test]
    async fn apply_scene_replace_swaps_fields() {
        let (_tmp, svc) = fresh_service_local().await;
        let project = project_with_future_chapter(
            &svc,
            "PA replace",
            5,
            vec![scene(1, "old summary")],
            Vec::new(),
        )
        .await;
        let id = stage(
            &svc,
            &project.project_id,
            "scene_replace",
            Some(5),
            serde_json::json!({
                "scene_order": 1, "summary": "new summary", "purpose": "new purpose",
                "character_ids": ["character:x"]
            }),
        )
        .await;
        let out = apply_one(&svc, &project.project_id, &id).await;
        assert_eq!(out.results[0].outcome, "applied", "{out:?}");
        let after = plan_of(&svc, &project.project_id, 5).await;
        assert_eq!(after.scenes[0].summary, "new summary");
        assert_eq!(after.scenes[0].purpose, "new purpose");
        assert_eq!(
            after.scenes[0].character_ids,
            vec!["character:x".to_string()]
        );
    }

    #[tokio::test]
    async fn apply_scene_reorder_permutes_spine() {
        let (_tmp, svc) = fresh_service_local().await;
        let project = project_with_future_chapter(
            &svc,
            "PA reorder",
            5,
            vec![scene(1, "A"), scene(2, "B"), scene(3, "C")],
            Vec::new(),
        )
        .await;
        // Reverse: new_order [3,2,1] means scene currently at 3 becomes first.
        let id = stage(
            &svc,
            &project.project_id,
            "scene_reorder",
            Some(5),
            serde_json::json!({"new_order": [3, 2, 1]}),
        )
        .await;
        let out = apply_one(&svc, &project.project_id, &id).await;
        assert_eq!(out.results[0].outcome, "applied", "{out:?}");
        let after = plan_of(&svc, &project.project_id, 5).await;
        let summaries: Vec<(i32, String)> = after
            .scenes
            .iter()
            .map(|s| (s.scene_order, s.summary.clone()))
            .collect();
        assert_eq!(
            summaries,
            vec![
                (1, "C".to_string()),
                (2, "B".to_string()),
                (3, "A".to_string()),
            ],
            "reorder applies the permutation then renumbers 1..n"
        );
    }

    #[tokio::test]
    async fn apply_thread_promote_and_retire() {
        let (_tmp, svc) = fresh_service_local().await;
        let project = project_with_future_chapter(
            &svc,
            "PA thread",
            5,
            vec![scene(1, "a")],
            vec!["theme:existing".to_string()],
        )
        .await;
        // Promote adds a theme id.
        let promote = stage(
            &svc,
            &project.project_id,
            "thread_promote",
            Some(5),
            serde_json::json!({"kind": "theme", "id": "theme:added"}),
        )
        .await;
        apply_one(&svc, &project.project_id, &promote).await;
        let after = plan_of(&svc, &project.project_id, 5).await;
        assert!(
            after
                .target_theme_ids
                .contains(&"theme:existing".to_string())
        );
        assert!(after.target_theme_ids.contains(&"theme:added".to_string()));

        // Retire removes the existing theme id.
        let retire = stage(
            &svc,
            &project.project_id,
            "thread_retire",
            Some(5),
            serde_json::json!({"kind": "theme", "id": "theme:existing"}),
        )
        .await;
        apply_one(&svc, &project.project_id, &retire).await;
        let after = plan_of(&svc, &project.project_id, 5).await;
        assert!(
            !after
                .target_theme_ids
                .contains(&"theme:existing".to_string())
        );
        assert!(after.target_theme_ids.contains(&"theme:added".to_string()));
        // Two applies bumped the revision twice.
        assert_eq!(after.plan_revision, Some(2));
    }

    #[tokio::test]
    async fn apply_promise_followup_creates_promise_at_placement() {
        let (_tmp, svc) = fresh_service_local().await;
        let project =
            project_with_future_chapter(&svc, "PA promise", 5, vec![scene(1, "a")], Vec::new())
                .await;
        let id = stage(
            &svc,
            &project.project_id,
            "promise_followup",
            None, // promise_followup carries no target_chapter
            serde_json::json!({
                "promise_type": "mystery",
                "description": "the successor thread",
                "planted_at_placement": {"book_number": 1, "chapter_number": 6, "scene_order": 1}
            }),
        )
        .await;
        let out = apply_one(&svc, &project.project_id, &id).await;
        assert_eq!(out.results[0].outcome, "applied", "{out:?}");
        let promise_id = out.results[0]
            .applied_record_id
            .as_deref()
            .expect("promise id returned");
        assert!(promise_id.starts_with("narrative_promise:"));
        let promise = svc
            .repository()
            .get_narrative_promise(promise_id)
            .await
            .unwrap();
        assert_eq!(promise.description, "the successor thread");
        assert_eq!(promise.planted_at.chapter_number, 6);
        // No prior_state for promise_followup (it targets no chapter row).
        let applied = svc
            .list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project.project_id.clone(),
                status: Some("applied".to_string()),
                book_number: None,
                source_chapter: None,
            })
            .await
            .unwrap();
        assert!(applied.amendments[0].prior_state.is_none());
    }

    // ── Immutability guard (Test 2) ──────────────────────────────────────

    #[tokio::test]
    async fn immutability_guard_rejects_drafted_target_at_apply() {
        let (_tmp, svc) = fresh_service_local().await;
        let project =
            project_with_future_chapter(&svc, "PA guard", 5, vec![scene(1, "planned")], Vec::new())
                .await;
        let id = stage(
            &svc,
            &project.project_id,
            "synopsis_update",
            Some(5),
            serde_json::json!({"synopsis": "must not apply"}),
        )
        .await;
        // Draft a scene into chapter 5 AFTER staging — the guard is checked at
        // apply (ADR D3), so this makes the amendment fail at decision time.
        svc.save_scene_draft(SaveSceneDraftInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            chapter_number: 5,
            chapter_id: None,
            scene_order: 1,
            full_text: "chapter five now has drafted prose".to_string(),
            summary: "drafted".to_string(),
            content_rating: spindle_core::models::ContentRating::General,
            tone: None,
            generation_id: None,
            source_path: None,
            ..Default::default()
        })
        .await
        .unwrap();

        let err = svc
            .decide_plan_amendments(DecidePlanAmendmentsInput {
                project_id: project.project_id.clone(),
                decisions: vec![PlanAmendmentDecisionInput {
                    amendment_id: id.clone(),
                    action: "apply".to_string(),
                    edit: None,
                    note: None,
                }],
                decided_by: None,
            })
            .await
            .expect_err("guard must fail pre-flight");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("persisted scene") || msg.contains("immutab"),
            "error names the immutability guard: {msg}"
        );
        // Zero writes: the amendment is still staged, the synopsis unchanged.
        let staged = svc
            .list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project.project_id.clone(),
                status: Some("staged".to_string()),
                book_number: None,
                source_chapter: None,
            })
            .await
            .unwrap();
        assert_eq!(staged.amendments.len(), 1);
        assert_eq!(staged.amendments[0].id, id);
        let after = plan_of(&svc, &project.project_id, 5).await;
        assert_eq!(after.synopsis, "the planned future");
    }

    // ── Pre-flight atomicity (Test 3) ────────────────────────────────────

    #[tokio::test]
    async fn preflight_atomicity_one_bad_decision_aborts_batch() {
        let (_tmp, svc) = fresh_service_local().await;
        let project =
            project_with_future_chapter(&svc, "PA atomic", 5, vec![scene(1, "a")], Vec::new())
                .await;
        let good1 = stage(
            &svc,
            &project.project_id,
            "synopsis_update",
            Some(5),
            serde_json::json!({"synopsis": "good one"}),
        )
        .await;
        // A scene_drop naming an order that does not exist — dangling
        // reference the pre-flight catches.
        let bad = stage(
            &svc,
            &project.project_id,
            "scene_drop",
            Some(5),
            serde_json::json!({"scene_order": 99}),
        )
        .await;
        let good2 = stage(
            &svc,
            &project.project_id,
            "thread_promote",
            Some(5),
            serde_json::json!({"kind": "theme", "id": "theme:z"}),
        )
        .await;

        let err = svc
            .decide_plan_amendments(DecidePlanAmendmentsInput {
                project_id: project.project_id.clone(),
                decisions: vec![
                    PlanAmendmentDecisionInput {
                        amendment_id: good1.clone(),
                        action: "apply".to_string(),
                        edit: None,
                        note: None,
                    },
                    PlanAmendmentDecisionInput {
                        amendment_id: bad.clone(),
                        action: "apply".to_string(),
                        edit: None,
                        note: None,
                    },
                    PlanAmendmentDecisionInput {
                        amendment_id: good2.clone(),
                        action: "apply".to_string(),
                        edit: None,
                        note: None,
                    },
                ],
                decided_by: None,
            })
            .await
            .expect_err("one bad decision aborts");
        assert!(format!("{err:#}").contains("pre-flight"));
        // Zero writes: all three stay staged, the synopsis unchanged.
        let staged = svc
            .list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project.project_id.clone(),
                status: Some("staged".to_string()),
                book_number: None,
                source_chapter: None,
            })
            .await
            .unwrap();
        assert_eq!(staged.amendments.len(), 3);
        let after = plan_of(&svc, &project.project_id, 5).await;
        assert_eq!(after.synopsis, "the planned future");
    }

    // ── Mid-apply honesty via the seam (Test 3, seam arm) ────────────────

    #[tokio::test]
    async fn mid_apply_failure_stops_and_reports_honestly() {
        // Pre-flight passes for two synopsis_update applies against two
        // distinct future chapters. Between pre-flight and apply the SECOND
        // chapter's plan row is deleted, so its apply fails at write time
        // (the current plan vanished). Earlier applies stay applied; the
        // failing row stays staged; the later row reads not_reached. The
        // deletion runs through the injectable after-pre-flight hook, so the
        // failure is driven through the genuine dispatch path, not a mock.
        let (_tmp, svc) = fresh_service_local().await;
        let project =
            project_with_future_chapter(&svc, "PA mid", 5, vec![scene(1, "a")], Vec::new()).await;
        // Add a second future chapter (6) and a third (7).
        for ch in [6, 7] {
            svc.plan_chapter(PlanChapterInput {
                project_id: project.project_id.clone(),
                book_number: 1,
                chapter_number: ch,
                pov_character_id: None,
                synopsis: format!("chapter {ch} planned"),
                target_theme_ids: Vec::new(),
                target_conflict_ids: Vec::new(),
                target_plot_line_ids: Vec::new(),
                scenes: vec![scene(1, "s")],
            })
            .await
            .unwrap();
        }
        let first = stage(
            &svc,
            &project.project_id,
            "synopsis_update",
            Some(5),
            serde_json::json!({"synopsis": "first applied"}),
        )
        .await;
        let second = stage(
            &svc,
            &project.project_id,
            "synopsis_update",
            Some(6),
            serde_json::json!({"synopsis": "second fails"}),
        )
        .await;
        let third = stage(
            &svc,
            &project.project_id,
            "synopsis_update",
            Some(7),
            serde_json::json!({"synopsis": "third not reached"}),
        )
        .await;

        let branch = svc
            .repository()
            .get_active_branch(&project.project_id)
            .await
            .unwrap();
        let repo = svc.repository().clone();
        let project_id = project.project_id.clone();
        let branch_id = branch.id.clone();
        let out = svc
            .decide_plan_amendments_with_seam(
                DecidePlanAmendmentsInput {
                    project_id: project.project_id.clone(),
                    decisions: vec![
                        PlanAmendmentDecisionInput {
                            amendment_id: first.clone(),
                            action: "apply".to_string(),
                            edit: None,
                            note: None,
                        },
                        PlanAmendmentDecisionInput {
                            amendment_id: second.clone(),
                            action: "apply".to_string(),
                            edit: None,
                            note: None,
                        },
                        PlanAmendmentDecisionInput {
                            amendment_id: third.clone(),
                            action: "apply".to_string(),
                            edit: None,
                            note: None,
                        },
                    ],
                    decided_by: None,
                },
                move || {
                    let repo = repo.clone();
                    let project_id = project_id.clone();
                    let branch_id = branch_id.clone();
                    async move {
                        repo.delete_chapter_plans_for_chapter(&project_id, &branch_id, 1, 6)
                            .await
                            .unwrap();
                    }
                },
            )
            .await
            .unwrap();

        assert_eq!(out.applied_count, 1);
        assert_eq!(out.failed_count, 1);
        assert_eq!(out.results[0].outcome, "applied");
        assert_eq!(out.results[1].outcome, "failed");
        assert!(out.results[1].error.is_some());
        assert_eq!(out.results[2].outcome, "not_reached");

        // First apply is real (chapter 5 synopsis rewritten, not rolled back).
        let ch5 = plan_of(&svc, &project.project_id, 5).await;
        assert_eq!(ch5.synopsis, "first applied");
        // The failed + not-reached rows stay staged.
        let staged = svc
            .list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project.project_id.clone(),
                status: Some("staged".to_string()),
                book_number: None,
                source_chapter: None,
            })
            .await
            .unwrap();
        let staged_ids: Vec<_> = staged.amendments.iter().map(|a| a.id.clone()).collect();
        assert!(staged_ids.contains(&second));
        assert!(staged_ids.contains(&third));
    }

    // ── Reorder / insert coherence (Test 4) ──────────────────────────────

    #[tokio::test]
    async fn incoherent_reorder_fails_preflight() {
        let (_tmp, svc) = fresh_service_local().await;
        let project = project_with_future_chapter(
            &svc,
            "PA badreorder",
            5,
            vec![scene(1, "A"), scene(2, "B"), scene(3, "C")],
            Vec::new(),
        )
        .await;
        // Wrong length (2 orders for a 3-scene chapter).
        let wrong_len = stage(
            &svc,
            &project.project_id,
            "scene_reorder",
            Some(5),
            serde_json::json!({"new_order": [1, 2]}),
        )
        .await;
        let err = svc
            .decide_plan_amendments(DecidePlanAmendmentsInput {
                project_id: project.project_id.clone(),
                decisions: vec![PlanAmendmentDecisionInput {
                    amendment_id: wrong_len.clone(),
                    action: "apply".to_string(),
                    edit: None,
                    note: None,
                }],
                decided_by: None,
            })
            .await
            .expect_err("wrong-length permutation rejected");
        assert!(format!("{err:#}").contains("pre-flight"));

        // Duplicate order (dup 1, missing 3).
        let dup = stage(
            &svc,
            &project.project_id,
            "scene_reorder",
            Some(5),
            serde_json::json!({"new_order": [1, 1, 2]}),
        )
        .await;
        let err2 = svc
            .decide_plan_amendments(DecidePlanAmendmentsInput {
                project_id: project.project_id.clone(),
                decisions: vec![PlanAmendmentDecisionInput {
                    amendment_id: dup.clone(),
                    action: "apply".to_string(),
                    edit: None,
                    note: None,
                }],
                decided_by: None,
            })
            .await
            .expect_err("duplicate permutation rejected");
        assert!(format!("{err2:#}").contains("pre-flight"));
    }

    // ── List filtering (list_plan_amendments) ────────────────────────────

    #[tokio::test]
    async fn list_filters_by_status_and_provenance() {
        let (_tmp, svc) = fresh_service_local().await;
        let project =
            project_with_future_chapter(&svc, "PA list", 5, vec![scene(1, "a")], Vec::new()).await;
        let staged = stage(
            &svc,
            &project.project_id,
            "synopsis_update",
            Some(5),
            serde_json::json!({"synopsis": "x"}),
        )
        .await;
        // All (no filter) returns it.
        let all = svc
            .list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project.project_id.clone(),
                status: None,
                book_number: None,
                source_chapter: None,
            })
            .await
            .unwrap();
        assert_eq!(all.amendments.len(), 1);
        assert_eq!(all.amendments[0].id, staged);
        // Provenance filter (book 1, source chapter 3 — the stage helper's
        // provenance) returns it; a different source chapter returns none.
        let scoped = svc
            .list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project.project_id.clone(),
                status: Some("staged".to_string()),
                book_number: Some(1),
                source_chapter: Some(3),
            })
            .await
            .unwrap();
        assert_eq!(scoped.amendments.len(), 1);
        let empty = svc
            .list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project.project_id.clone(),
                status: None,
                book_number: Some(1),
                source_chapter: Some(99),
            })
            .await
            .unwrap();
        assert!(empty.amendments.is_empty());
        // A lone book_number (no source_chapter) is an input error.
        let err = svc
            .list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project.project_id.clone(),
                status: None,
                book_number: Some(1),
                source_chapter: None,
            })
            .await;
        assert!(err.is_err(), "lone book_number is rejected");
    }

    #[tokio::test]
    async fn apply_amendments_reseeds_undrafted_chapter_in_active_authoring_run() {
        use crate::sqlite::records::{AuthoringRun, AuthoringRunChapter, AuthoringRunScene};
        use spindle_core::models::ContentRating;

        let (_tmp, svc) = fresh_service_local().await;
        let project = project_with_future_chapter(
            &svc,
            "PA active-run reseed",
            2,
            vec![
                PlanChapterSceneInput {
                    scene_order: 1,
                    summary: "scene one".to_string(),
                    beat_structure: Vec::new(),
                    beats: Vec::new(),
                    turning_beat: None,
                    character_ids: vec!["character:alice".to_string()],
                    location_id: Some("location:hall".to_string()),
                    content_rating: Some(ContentRating::General),
                    purpose: "open".to_string(),
                    research_required: None,
                    research_tags: Vec::new(),
                    explicit_query: None,
                    primary_value_axis: None,
                    secondary_value_axes: Vec::new(),
                    opposition: None,
                    gap: None,
                    scene_question: None,
                    inaction_cost: None,
                    risk: None,
                    dilemma: None,
                },
                PlanChapterSceneInput {
                    scene_order: 2,
                    summary: "scene two".to_string(),
                    beat_structure: Vec::new(),
                    beats: Vec::new(),
                    turning_beat: None,
                    character_ids: vec!["character:bob".to_string()],
                    location_id: Some("location:courtyard".to_string()),
                    content_rating: Some(ContentRating::Teen),
                    purpose: "turn".to_string(),
                    research_required: None,
                    research_tags: Vec::new(),
                    explicit_query: None,
                    primary_value_axis: None,
                    secondary_value_axes: Vec::new(),
                    opposition: None,
                    gap: None,
                    scene_question: None,
                    inaction_cost: None,
                    risk: None,
                    dilemma: None,
                },
            ],
            Vec::new(),
        )
        .await;
        let branch = svc
            .repository()
            .get_active_branch(&project.project_id)
            .await
            .unwrap();

        let run_id = "authoring_run:test-reseed".to_string();
        let now = chrono::Utc::now();
        let pre_amendment_rel = "scenes/chapter-0002/scene-001.json".to_string();
        let unowned_scene2_rel = "scenes/chapter-0002/scene-002.json".to_string();
        let pre_amendment_file = crate::workspace::runtime_dir(svc.repository().data_dir())
            .join("../artifacts")
            .join(&pre_amendment_rel);
        let unowned_scene2_file = crate::workspace::runtime_dir(svc.repository().data_dir())
            .join("../artifacts")
            .join(&unowned_scene2_rel);
        // Alternate root outside runtime_dir(data_dir) (e.g., data_dir/../artifacts)
        // that could belong to another workspace or CLI run.
        let alternate_root_file = svc
            .repository()
            .data_dir()
            .join("../artifacts")
            .join(&pre_amendment_rel);
        std::fs::create_dir_all(pre_amendment_file.parent().unwrap()).unwrap();
        std::fs::create_dir_all(alternate_root_file.parent().unwrap()).unwrap();
        std::fs::write(
            &pre_amendment_file,
            r#"{"chapter_number":2,"scene_order":1,"prompt":"stale pre-amendment prompt"}"#,
        )
        .unwrap();
        std::fs::write(
            &unowned_scene2_file,
            r#"{"chapter_number":2,"scene_order":2,"prompt":"unrecorded artifact"}"#,
        )
        .unwrap();
        std::fs::write(
            &alternate_root_file,
            r#"{"chapter_number":2,"scene_order":1,"prompt":"other workspace artifact"}"#,
        )
        .unwrap();
        assert!(pre_amendment_file.exists());
        assert!(unowned_scene2_file.exists());
        assert!(alternate_root_file.exists());

        svc.repository()
            .save_authoring_run(
                AuthoringRun {
                    id: run_id.clone(),
                    project_id: project.project_id.clone(),
                    active_branch_id: branch.id.clone(),
                    book_number: 1,
                    start_chapter: 1,
                    end_chapter: 2,
                    checkpoint_interval: 2,
                    last_checkpoint_end_chapter: 0,
                    artifacts_dir: "../artifacts".to_string(),
                    editorial_directives: Vec::new(),
                    status: "active".to_string(),
                    created_at: now,
                    updated_at: now,
                    mining_policy: None,
                    max_revise_attempts: None,
                    checkpoint_policy: None,
                    replan_policy: Some("propose_after_chapter".to_string()),
                },
                vec![AuthoringRunChapter {
                    authoring_run_id: run_id.clone(),
                    chapter_number: 2,
                    planned: true,
                    synopsis: "the planned future".to_string(),
                    pov_character_id: None,
                    status: "pending".to_string(),
                    summary_saved: false,
                    summary_artifact_path: None,
                    replan_status: None,
                    replan_detail: None,
                }],
                vec![
                    AuthoringRunScene {
                        authoring_run_id: run_id.clone(),
                        chapter_number: 2,
                        scene_order: 1,
                        character_ids: vec!["character:alice".to_string()],
                        location_id: "location:hall".to_string(),
                        content_rating: "general".to_string(),
                        tone: Some("wry".to_string()),
                        source_path: None,
                        phase: "pending".to_string(),
                        scene_id: None,
                        scene_artifact_path: Some(pre_amendment_rel.clone()),
                        draft_diagnostics: None,
                        blocked_reason: None,
                        research_required: None,
                        research_tags: Vec::new(),
                        explicit_query: None,
                        mine_status: None,
                        mine_detail: None,
                        verify_status: None,
                        verify_detail: None,
                        revise_attempts: 0,
                        last_finding_fingerprint: None,
                    },
                    AuthoringRunScene {
                        authoring_run_id: run_id.clone(),
                        chapter_number: 2,
                        scene_order: 2,
                        character_ids: vec!["character:bob".to_string()],
                        location_id: "location:courtyard".to_string(),
                        content_rating: "teen".to_string(),
                        tone: Some("tense".to_string()),
                        source_path: None,
                        phase: "pending".to_string(),
                        scene_id: None,
                        scene_artifact_path: None,
                        draft_diagnostics: None,
                        blocked_reason: None,
                        research_required: None,
                        research_tags: Vec::new(),
                        explicit_query: None,
                        mine_status: None,
                        mine_detail: None,
                        verify_status: None,
                        verify_detail: None,
                        revise_attempts: 0,
                        last_finding_fingerprint: None,
                    },
                ],
                Vec::new(),
            )
            .await
            .unwrap();

        // 1. synopsis_update (revision 2): invalidates pre-amendment artifact and assigns -r2 paths
        let syn_id = stage(
            &svc,
            &project.project_id,
            "synopsis_update",
            Some(2),
            serde_json::json!({"synopsis": "the updated future"}),
        )
        .await;
        apply_one(&svc, &project.project_id, &syn_id).await;
        let (_, chs, scs, _) = svc
            .repository()
            .get_authoring_run(&run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(chs[0].synopsis, "the updated future");
        assert!(
            !pre_amendment_file.exists(),
            "run-owned pre-amendment scene artifact in resolved runtime root must be removed when reseeding"
        );
        assert!(
            alternate_root_file.exists(),
            "same-named artifact in alternate root (data_dir/../artifacts) must survive"
        );
        assert!(
            unowned_scene2_file.exists(),
            "unrecorded legacy filename for scene with scene_artifact_path=None must survive"
        );
        assert_eq!(
            scs[0].scene_artifact_path.as_deref(),
            Some("scenes/chapter-0002/scene-001-r1.json")
        );
        assert_eq!(
            scs[1].scene_artifact_path.as_deref(),
            Some("scenes/chapter-0002/scene-002-r1.json")
        );

        // 2. scene_add (omitting location_id and content_rating; inherits from neighbor)
        let add_id = stage(
            &svc,
            &project.project_id,
            "scene_add",
            Some(2),
            serde_json::json!({
                "scene_order": 2,
                "summary": "inserted bridge",
                "purpose": "bridge",
                "character_ids": ["character:carol"],
                "insert_at": 2
            }),
        )
        .await;
        apply_one(&svc, &project.project_id, &add_id).await;
        let (_, _, scs, _) = svc
            .repository()
            .get_authoring_run(&run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(scs.len(), 3);
        assert_eq!(scs[0].scene_order, 1);
        assert_eq!(scs[0].tone.as_deref(), Some("wry"));
        assert_eq!(scs[1].scene_order, 2);
        assert_eq!(scs[1].character_ids, vec!["character:carol".to_string()]);
        assert!(!scs[1].location_id.is_empty());
        assert_eq!(scs[2].scene_order, 3);
        assert_eq!(scs[2].character_ids, vec!["character:bob".to_string()]);
        assert_eq!(scs[2].tone.as_deref(), Some("tense"));

        // 3. scene_reorder ([3, 1, 2])
        let reorder_id = stage(
            &svc,
            &project.project_id,
            "scene_reorder",
            Some(2),
            serde_json::json!({"new_order": [3, 1, 2]}),
        )
        .await;
        apply_one(&svc, &project.project_id, &reorder_id).await;
        let (_, _, scs, _) = svc
            .repository()
            .get_authoring_run(&run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            scs.iter()
                .map(|s| (s.scene_order, s.character_ids.clone()))
                .collect::<Vec<_>>(),
            vec![
                (1, vec!["character:bob".to_string()]),
                (2, vec!["character:alice".to_string()]),
                (3, vec!["character:carol".to_string()]),
            ]
        );
        // Preserves lineage metadata (tone) across reorder
        assert_eq!(scs[0].tone.as_deref(), Some("tense"));
        assert_eq!(scs[1].tone.as_deref(), Some("wry"));

        // 4. scene_drop (drop scene_order 2)
        let drop_id = stage(
            &svc,
            &project.project_id,
            "scene_drop",
            Some(2),
            serde_json::json!({"scene_order": 2}),
        )
        .await;
        apply_one(&svc, &project.project_id, &drop_id).await;
        let (_, _, scs, _) = svc
            .repository()
            .get_authoring_run(&run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            scs.iter()
                .map(|s| (s.scene_order, s.character_ids.clone()))
                .collect::<Vec<_>>(),
            vec![
                (1, vec!["character:bob".to_string()]),
                (2, vec!["character:carol".to_string()]),
            ]
        );
    }

    // ── McKee value-chain amendments ──────────────────────────────────────

    #[tokio::test]
    async fn mckee_scene_add_amendment_is_refused_with_a_readable_error() {
        let (_tmp, svc) = fresh_service_local().await;
        let project = mckee_project_with_value_plan(&svc, "PA McKee add", 2).await;
        let before = plan_of(&svc, &project.project_id, 2).await;
        let id = stage(
            &svc,
            &project.project_id,
            "scene_add",
            Some(2),
            serde_json::json!({
                "scene_order": 99, "summary": "inserted", "purpose": "bridge",
                "character_ids": ["character:alice"], "insert_at": 2
            }),
        )
        .await;

        let err = svc
            .decide_plan_amendments(DecidePlanAmendmentsInput {
                project_id: project.project_id.clone(),
                decisions: vec![PlanAmendmentDecisionInput {
                    amendment_id: id.clone(),
                    action: "apply".to_string(),
                    edit: None,
                    note: None,
                }],
                decided_by: None,
            })
            .await
            .expect_err("scene_add on a McKee project must be refused");
        let message = format!("{err:#}");
        assert!(
            message.contains("scene_add is refused for McKee-structured projects"),
            "{message}"
        );
        assert!(
            message.contains("Restage the Chapter planning artifact"),
            "{message}"
        );
        assert!(
            message
                .contains("Adding Sequence membership through a plan amendment is not supported"),
            "{message}"
        );

        // Zero writes: the plan is unchanged and the amendment stays staged.
        let after = plan_of(&svc, &project.project_id, 2).await;
        assert_eq!(after.scenes.len(), before.scenes.len());
        let staged = svc
            .list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project.project_id.clone(),
                status: Some("staged".to_string()),
                book_number: None,
                source_chapter: None,
            })
            .await
            .unwrap();
        assert_eq!(staged.amendments.len(), 1, "{staged:?}");
    }

    #[tokio::test]
    async fn mckee_scene_replace_amendment_keeps_the_value_plan() {
        let (_tmp, svc) = fresh_service_local().await;
        let project = mckee_project_with_value_plan(&svc, "PA McKee replace", 2).await;
        let before = plan_of(&svc, &project.project_id, 2).await;
        let id = stage(
            &svc,
            &project.project_id,
            "scene_replace",
            Some(2),
            serde_json::json!({
                "scene_order": 1, "summary": "new summary", "purpose": "new purpose",
                "character_ids": ["character:bob"]
            }),
        )
        .await;
        let out = apply_one(&svc, &project.project_id, &id).await;
        assert_eq!(out.results[0].outcome, "applied", "{out:?}");

        let after = plan_of(&svc, &project.project_id, 2).await;
        assert_eq!(after.scenes[0].summary, "new summary");
        assert_eq!(after.scenes[0].purpose, "new purpose");
        assert_eq!(
            after.scenes[0].character_ids,
            vec!["character:bob".to_string()]
        );
        assert_eq!(
            after.scenes[0].primary_value_axis, before.scenes[0].primary_value_axis,
            "scene_replace must not drop the primary Value Axis"
        );
        assert_eq!(
            after.scenes[0].secondary_value_axes,
            before.scenes[0].secondary_value_axes
        );
        assert_eq!(after.scenes[0].opposition, before.scenes[0].opposition);
        assert_eq!(after.scenes[0].gap, before.scenes[0].gap);
        assert_eq!(
            after.scenes[0].scene_question,
            before.scenes[0].scene_question
        );
        assert_eq!(
            after.scenes[0].inaction_cost,
            before.scenes[0].inaction_cost
        );
        assert_eq!(after.scenes[0].risk, before.scenes[0].risk);
        assert_eq!(after.scenes[0].dilemma, before.scenes[0].dilemma);

        // The surviving new plan is still shape-valid for the value-chain rules.
        let errors = spindle_core::scene_value::scene_value_errors(
            after.scenes[0].primary_value_axis.as_ref(),
            &after.scenes[0].secondary_value_axes,
            after.scenes[0].risk,
            after.scenes[0].gap.as_ref(),
            after.scenes[0].dilemma.as_ref().map(|dilemma| {
                (
                    dilemma.character_id.as_str(),
                    dilemma.contradiction.as_str(),
                )
            }),
        );
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[tokio::test]
    async fn mckee_drop_reorder_and_synopsis_amendments_keep_the_value_plan() {
        let (_tmp, svc) = fresh_service_local().await;
        let project = mckee_project_with_value_plan(&svc, "PA McKee other classes", 2).await;
        let before = plan_of(&svc, &project.project_id, 2).await;

        // synopsis_update never touches scene value plans.
        let synopsis_id = stage(
            &svc,
            &project.project_id,
            "synopsis_update",
            Some(2),
            serde_json::json!({"synopsis": "a different synopsis"}),
        )
        .await;
        apply_one(&svc, &project.project_id, &synopsis_id).await;
        let after_synopsis = plan_of(&svc, &project.project_id, 2).await;
        assert_eq!(
            after_synopsis.scenes[0].primary_value_axis,
            before.scenes[0].primary_value_axis
        );

        // scene_reorder keeps each scene's value plan attached to that scene.
        let reorder_id = stage(
            &svc,
            &project.project_id,
            "scene_reorder",
            Some(2),
            serde_json::json!({"new_order": [2, 1]}),
        )
        .await;
        apply_one(&svc, &project.project_id, &reorder_id).await;
        let after_reorder = plan_of(&svc, &project.project_id, 2).await;
        let open = after_reorder
            .scenes
            .iter()
            .find(|scene| scene.summary == "open")
            .expect("open scene survives the reorder");
        assert_eq!(open.primary_value_axis, before.scenes[0].primary_value_axis);
        assert!(open.inaction_cost.is_some());

        // scene_drop leaves the survivors' value plans intact.
        let drop_id = stage(
            &svc,
            &project.project_id,
            "scene_drop",
            Some(2),
            serde_json::json!({"scene_order": 2}),
        )
        .await;
        apply_one(&svc, &project.project_id, &drop_id).await;
        let after_drop = plan_of(&svc, &project.project_id, 2).await;
        assert_eq!(after_drop.scenes.len(), 1);
        assert!(after_drop.scenes[0].primary_value_axis.is_some());
        assert!(after_drop.scenes[0].inaction_cost.is_some());
    }

    #[tokio::test]
    async fn mckee_direct_plan_write_rejects_invalid_value_shapes() {
        use spindle_core::charge::{ChargeDepth, ChargeSign};
        let (_tmp, svc) = fresh_service_local().await;
        let project = svc
            .create_mckee_project(CreateProjectInput {
                name: "PA McKee direct write".to_string(),
                project_type: "novel".to_string(),
                genre: "fantasy".to_string(),
                reader_contract: ReaderContract {
                    promise: "McKee direct write".to_string(),
                    style_notes: Vec::new(),
                    boundaries: Vec::new(),
                },
            })
            .await
            .unwrap();
        let mut malformed = value_scene(
            1,
            "malformed",
            value_charge(ChargeSign::Positive, ChargeDepth::Positive),
            value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
        );
        malformed.secondary_value_axes = vec![
            value_axis(
                value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
            ),
            value_axis(
                value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
            ),
            value_axis(
                value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
            ),
        ];
        malformed.risk = Some(9);
        let err = svc
            .plan_chapter(PlanChapterInput {
                project_id: project.project_id.clone(),
                book_number: 1,
                chapter_number: 2,
                pov_character_id: None,
                synopsis: "invalid value plan".to_string(),
                target_theme_ids: Vec::new(),
                target_conflict_ids: Vec::new(),
                target_plot_line_ids: Vec::new(),
                scenes: vec![malformed],
            })
            .await
            .expect_err("a McKee plan_chapter write rejects invalid value shapes");
        assert!(format!("{err:#}").contains("invalid value plan"), "{err:#}");
    }

    #[tokio::test]
    async fn non_mckee_projects_keep_optional_value_fields_and_scene_add() {
        use spindle_core::charge::{ChargeDepth, ChargeSign};
        let (_tmp, svc) = fresh_service_local().await;
        let project = svc
            .create_project(CreateProjectInput {
                name: "PA non-McKee value fields".to_string(),
                project_type: "novel".to_string(),
                genre: "fantasy".to_string(),
                reader_contract: ReaderContract {
                    promise: "unmarked project keeps its payloads".to_string(),
                    style_notes: Vec::new(),
                    boundaries: Vec::new(),
                },
            })
            .await
            .unwrap();
        // The value fields are optional for an unmarked project: even a shape
        // that the McKee rules would refuse is stored exactly as sent (previous
        // behaviour byte for byte) instead of being judged by McKee rules.
        let mut unmarked = value_scene(
            1,
            "unmarked",
            value_charge(ChargeSign::Positive, ChargeDepth::Positive),
            value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
        );
        unmarked.secondary_value_axes = vec![
            value_axis(
                value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
            ),
            value_axis(
                value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
            ),
            value_axis(
                value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
            ),
        ];
        unmarked.risk = Some(9);
        svc.plan_chapter(PlanChapterInput {
            project_id: project.project_id.clone(),
            book_number: 1,
            chapter_number: 2,
            pov_character_id: None,
            synopsis: "the unmarked future".to_string(),
            target_theme_ids: Vec::new(),
            target_conflict_ids: Vec::new(),
            target_plot_line_ids: Vec::new(),
            scenes: vec![unmarked],
        })
        .await
        .expect("an unmarked project is not judged by McKee shape rules");
        let stored = plan_of(&svc, &project.project_id, 2).await;
        assert_eq!(stored.scenes[0].secondary_value_axes.len(), 3);
        assert_eq!(stored.scenes[0].risk, Some(9));

        // scene_add stays available for an unmarked project.
        let id = stage(
            &svc,
            &project.project_id,
            "scene_add",
            Some(2),
            serde_json::json!({
                "scene_order": 99, "summary": "inserted", "purpose": "bridge",
                "character_ids": [], "insert_at": 2
            }),
        )
        .await;
        let out = apply_one(&svc, &project.project_id, &id).await;
        assert_eq!(out.results[0].outcome, "applied", "{out:?}");
        let after = plan_of(&svc, &project.project_id, 2).await;
        assert_eq!(after.scenes.len(), 2);
        assert_eq!(
            after.scenes[0].secondary_value_axes.len(),
            3,
            "existing value fields stay byte-identical across the amendment"
        );
    }

    // ── McKee amendment cascade over unwritten Scenes (issue #34) ────────────────
    mod cascade {
        use super::*;
        use spindle_core::charge::{ChargeDepth, ChargeSign};
        use spindle_core::models::{
            CreateNarrativePromiseInput, PlanAmendmentCascadeTrigger, RegisterCanonicalFactInput,
            StoryPlacement,
        };

        /// A three-chapter McKee plan whose value chain is valid. Chapter 3 is
        /// drafted (one saved scene); chapters 1–2 are accepted but undrafted.
        async fn cascade_fixture(
            svc: &SqliteSpindleService,
            name: &str,
            drafted_chapter: Option<i32>,
        ) -> CreateProjectOutput {
            let project = svc
                .create_mckee_project(CreateProjectInput {
                    name: name.to_string(),
                    project_type: "novel".to_string(),
                    genre: "fantasy".to_string(),
                    reader_contract: ReaderContract {
                        promise: "McKee cascade".to_string(),
                        style_notes: Vec::new(),
                        boundaries: Vec::new(),
                    },
                })
                .await
                .unwrap();
            let chapters: [(i32, Vec<PlanChapterSceneInput>); 3] = [
                (
                    1,
                    vec![
                        value_scene(
                            1,
                            "c1s1",
                            value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                            value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                        ),
                        value_scene(
                            2,
                            "c1s2",
                            value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                            value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                        ),
                        value_scene(
                            3,
                            "c1s3",
                            value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                            value_charge(ChargeSign::Negative, ChargeDepth::NegationOfTheNegation),
                        ),
                    ],
                ),
                (
                    2,
                    vec![
                        value_scene(
                            1,
                            "c2s1",
                            value_charge(ChargeSign::Negative, ChargeDepth::NegationOfTheNegation),
                            value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                        ),
                        value_scene(
                            2,
                            "c2s2",
                            value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                            value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                        ),
                    ],
                ),
                (
                    3,
                    vec![
                        value_scene(
                            1,
                            "c3s1",
                            value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                            value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                        ),
                        value_scene(
                            2,
                            "c3s2",
                            value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                            value_charge(ChargeSign::Negative, ChargeDepth::NegationOfTheNegation),
                        ),
                    ],
                ),
            ];
            for (chapter_number, scenes) in chapters {
                svc.plan_chapter(PlanChapterInput {
                    project_id: project.project_id.clone(),
                    book_number: 1,
                    chapter_number,
                    pov_character_id: None,
                    synopsis: format!("chapter {chapter_number}"),
                    target_theme_ids: Vec::new(),
                    target_conflict_ids: Vec::new(),
                    target_plot_line_ids: Vec::new(),
                    scenes,
                })
                .await
                .unwrap();
            }
            if let Some(chapter_number) = drafted_chapter {
                svc.save_scene_draft(SaveSceneDraftInput {
                    project_id: project.project_id.clone(),
                    book_number: 1,
                    chapter_number,
                    chapter_id: None,
                    scene_order: 1,
                    full_text: "Drafted prose that must never move.".to_string(),
                    summary: "the drafted scene".to_string(),
                    content_rating: spindle_core::models::ContentRating::General,
                    tone: None,
                    ..Default::default()
                })
                .await
                .unwrap();
            }
            project
        }

        /// A `scene_replace` that rewrites one Scene's primary axis (and repeats the
        /// class's required keys), used both as a trigger and as an operator edit.
        fn axis_repair(
            scene_order: i32,
            summary: &str,
            opening: spindle_core::charge::Charge,
            closing: spindle_core::charge::Charge,
        ) -> serde_json::Value {
            serde_json::json!({
                "scene_order": scene_order,
                "summary": summary,
                "purpose": format!("purpose {scene_order}"),
                "character_ids": ["character:alice"],
                "primary_value_axis": {
                    "axis": "truth",
                    "opening": {"sign": opening.sign(), "depth": opening.depth()},
                    "closing": {"sign": closing.sign(), "depth": closing.depth()},
                }
            })
        }

        async fn stale_list(
            svc: &SqliteSpindleService,
            project_id: &str,
        ) -> spindle_core::models::ListPlanAmendmentsOutput {
            svc.list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project_id.to_string(),
                status: None,
                book_number: None,
                source_chapter: None,
            })
            .await
            .unwrap()
        }

        async fn staged_amendments(
            svc: &SqliteSpindleService,
            project_id: &str,
        ) -> Vec<spindle_core::models::PlanAmendment> {
            svc.list_plan_amendments(ListPlanAmendmentsInput {
                project_id: project_id.to_string(),
                status: Some("staged".to_string()),
                book_number: None,
                source_chapter: None,
            })
            .await
            .unwrap()
            .amendments
        }

        fn charge_json(charge: spindle_core::charge::Charge) -> serde_json::Value {
            serde_json::json!({"sign": charge.sign(), "depth": charge.depth()})
        }

        #[tokio::test]
        async fn cascade_marks_later_undrafted_scene_stale_and_stages_reconciliation() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade trigger", Some(3)).await;
            let id = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                // c1s3 now closes at Negative/Contrary instead of
                // Negative/NegationOfTheNegation: c2s1 no longer continues the axis.
                axis_repair(
                    3,
                    "c1s3",
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                ),
            )
            .await;

            let out = apply_one(&svc, &project.project_id, &id).await;
            assert_eq!(out.results[0].outcome, "applied", "{out:?}");
            let outcome = out.results[0]
                .cascade
                .as_ref()
                .expect("a closing-Charge change must run the cascade");
            assert!(
                outcome
                    .triggers
                    .contains(&PlanAmendmentCascadeTrigger::ClosingCharge),
                "{outcome:?}"
            );

            let list = stale_list(&svc, &project.project_id).await;
            assert_eq!(list.stale_scenes.len(), 1, "{list:?}");
            let stale = &list.stale_scenes[0];
            assert_eq!(
                (
                    stale.book_number,
                    stale.chapter_number,
                    stale.scene_order,
                    stale.status.as_str()
                ),
                (1, 2, 1, "stale")
            );
            assert!(
                stale
                    .finding_codes
                    .contains(&"value_chain_broken".to_string()),
                "{stale:?}"
            );
            assert_eq!(
                stale.triggering_amendment_id, id,
                "the mark names the triggering amendment"
            );
            let reconciliation_id = stale
                .reconciliation_amendment_id
                .clone()
                .expect("a chain break has a mechanical repair");
            assert_eq!(
                out.staged_reconciliation_ids,
                vec![reconciliation_id.clone()],
                "{out:?}"
            );

            let staged = staged_amendments(&svc, &project.project_id).await;
            let reconciliation = staged
                .iter()
                .find(|amendment| amendment.id == reconciliation_id)
                .expect("the reconciliation is staged through the normal queue");
            assert_eq!(reconciliation.amendment_class, "scene_replace");
            assert_eq!(reconciliation.target_chapter, Some(2));
            assert_eq!(reconciliation.payload["scene_order"], 1);
            assert_eq!(
                reconciliation.payload["primary_value_axis"]["opening"],
                charge_json(value_charge(ChargeSign::Negative, ChargeDepth::Contrary)),
                "the repair follows the amended previous closing Charge"
            );
            assert_eq!(staged.len(), 1, "nothing else is staged: {staged:?}");
        }

        #[tokio::test]
        async fn cascade_reconciliation_accept_clears_the_mark_and_repairs_the_chain() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade repair", Some(3)).await;
            let trigger = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                axis_repair(
                    3,
                    "c1s3",
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                ),
            )
            .await;
            apply_one(&svc, &project.project_id, &trigger).await;
            let stale = stale_list(&svc, &project.project_id).await.stale_scenes[0].clone();
            let reconciliation_id = stale.reconciliation_amendment_id.clone().unwrap();

            let out = apply_one(&svc, &project.project_id, &reconciliation_id).await;
            assert_eq!(out.results[0].outcome, "applied", "{out:?}");
            let outcome = out.results[0]
                .cascade
                .as_ref()
                .expect("a reconciliation apply re-validates its own Scene");
            assert!(
                outcome
                    .triggers
                    .contains(&PlanAmendmentCascadeTrigger::ReconciliationRepair),
                "{outcome:?}"
            );
            assert_eq!(outcome.cleared_stale, 1, "{outcome:?}");
            assert!(out.stale_scenes.is_empty(), "{out:?}");

            assert!(
                stale_list(&svc, &project.project_id)
                    .await
                    .stale_scenes
                    .is_empty()
            );
            let repaired = plan_of(&svc, &project.project_id, 2).await;
            let axis = repaired.scenes[0]
                .primary_value_axis
                .as_ref()
                .expect("the repair keeps the axis");
            assert_eq!(
                axis.opening,
                value_charge(ChargeSign::Negative, ChargeDepth::Contrary)
            );
            assert_eq!(
                axis.closing,
                value_charge(ChargeSign::Positive, ChargeDepth::Positive)
            );
            // The history row is retained as cleared, not deleted.
            let all = svc
                .repository()
                .list_scene_plan_stale(&project.project_id, &project.branch_id, Some("cleared"))
                .await
                .unwrap();
            assert_eq!(all.len(), 1, "{all:?}");
        }

        #[tokio::test]
        async fn cascade_noop_amendment_stages_and_marks_nothing() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade no-op", Some(3)).await;
            let synopsis = stage(
                &svc,
                &project.project_id,
                "synopsis_update",
                Some(1),
                serde_json::json!({"synopsis": "same chain, new synopsis"}),
            )
            .await;
            let out = apply_one(&svc, &project.project_id, &synopsis).await;
            assert_eq!(out.results[0].outcome, "applied", "{out:?}");
            assert!(
                out.results[0].cascade.is_none(),
                "no trigger means no cascade report: {out:?}"
            );
            assert!(
                stale_list(&svc, &project.project_id)
                    .await
                    .stale_scenes
                    .is_empty()
            );
            assert!(
                staged_amendments(&svc, &project.project_id)
                    .await
                    .is_empty()
            );

            // A prose-only scene_replace keeps the closing Charges identical.
            let prose = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(2),
                serde_json::json!({
                    "scene_order": 1, "summary": "c2s1 rewritten prose",
                    "purpose": "purpose 1", "character_ids": ["character:alice"]
                }),
            )
            .await;
            apply_one(&svc, &project.project_id, &prose).await;
            assert!(
                stale_list(&svc, &project.project_id)
                    .await
                    .stale_scenes
                    .is_empty()
            );
            assert!(
                staged_amendments(&svc, &project.project_id)
                    .await
                    .is_empty()
            );
        }

        #[tokio::test]
        async fn cascade_repeat_trigger_reuses_a_still_staged_reconciliation() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade repeat", Some(3)).await;
            let first = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                axis_repair(
                    3,
                    "c1s3",
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                ),
            )
            .await;
            apply_one(&svc, &project.project_id, &first).await;
            let before = stale_list(&svc, &project.project_id).await;
            let c2s1 = before
                .stale_scenes
                .iter()
                .find(|stale| stale.chapter_number == 2 && stale.scene_order == 1)
                .expect("c2s1 is stale");
            let mark_id = c2s1.id.clone();
            let reconciliation_id = c2s1.reconciliation_amendment_id.clone().unwrap();

            // A second trigger upstream: c1s1 now closes at Negative/Contradictory,
            // which breaks c1s2 and leaves c2s1 exactly as broken as before (same
            // required repair, so the staged proposal is still the right one).
            let second = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                axis_repair(
                    1,
                    "c1s1",
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                ),
            )
            .await;
            apply_one(&svc, &project.project_id, &second).await;

            let staged = staged_amendments(&svc, &project.project_id).await;
            let chapter_two_repairs: Vec<&spindle_core::models::PlanAmendment> = staged
                .iter()
                .filter(|amendment| {
                    amendment.amendment_class == "scene_replace"
                        && amendment.target_chapter == Some(2)
                        && amendment.payload["scene_order"] == 1
                })
                .collect();
            assert_eq!(
                chapter_two_repairs.len(),
                1,
                "a repeat cascade never duplicates a Scene's staged repair: {staged:?}"
            );
            assert_eq!(chapter_two_repairs[0].id, reconciliation_id);

            let after = stale_list(&svc, &project.project_id).await;
            let c2s1_after = after
                .stale_scenes
                .iter()
                .find(|stale| stale.chapter_number == 2 && stale.scene_order == 1)
                .expect("c2s1 is still stale");
            assert_eq!(
                c2s1_after.id, mark_id,
                "a repeat cascade updates the one row instead of inserting a duplicate"
            );
            assert_eq!(
                after
                    .stale_scenes
                    .iter()
                    .filter(|stale| stale.chapter_number == 2 && stale.scene_order == 1)
                    .count(),
                1,
                "one mark per Scene: {after:?}"
            );
            assert_eq!(
                c2s1_after.reconciliation_amendment_id.as_deref(),
                Some(reconciliation_id.as_str())
            );
            // The new break at c1s2 has no mechanical repair (its expected opening
            // would equal its closing Charge, a non-event): marked, proposal-less.
            let c1s2 = after
                .stale_scenes
                .iter()
                .find(|stale| stale.chapter_number == 1 && stale.scene_order == 2)
                .expect("c1s2 is stale");
            assert!(c1s2.reconciliation_amendment_id.is_none(), "{c1s2:?}");
            assert!(
                c1s2.reason
                    .contains("No reconciliation proposal is possible"),
                "a proposal-less mark states why honestly: {c1s2:?}"
            );
        }

        #[tokio::test]
        async fn cascade_leaves_drafted_scene_prose_and_plan_untouched() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade drafted", Some(3)).await;
            let plan_before =
                serde_json::to_string(&plan_of(&svc, &project.project_id, 3).await).unwrap();
            let scene_before = svc
                .repository()
                .list_scenes_by_project_and_branch(&project.project_id, &project.branch_id)
                .await
                .unwrap()
                .into_iter()
                .find(|scene| scene.chapter_number == 3 && scene.scene_order == 1)
                .expect("the drafted scene exists");
            let text_before = scene_before.full_text.clone();

            let trigger = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                axis_repair(
                    3,
                    "c1s3",
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                ),
            )
            .await;
            apply_one(&svc, &project.project_id, &trigger).await;

            let plan_after =
                serde_json::to_string(&plan_of(&svc, &project.project_id, 3).await).unwrap();
            assert_eq!(
                plan_before, plan_after,
                "a drafted chapter's plan is untouched"
            );
            let scene_after = svc
                .repository()
                .list_scenes_by_project_and_branch(&project.project_id, &project.branch_id)
                .await
                .unwrap()
                .into_iter()
                .find(|scene| scene.id == scene_before.id)
                .expect("the drafted scene still exists");
            assert_eq!(text_before, scene_after.full_text);
            assert_eq!(scene_before.realized_report, scene_after.realized_report);

            let list = stale_list(&svc, &project.project_id).await;
            assert!(
                list.stale_scenes
                    .iter()
                    .all(|stale| stale.chapter_number != 3),
                "a drafted Scene is never marked stale: {list:?}"
            );
            let staged = staged_amendments(&svc, &project.project_id).await;
            assert!(
                staged
                    .iter()
                    .all(|amendment| amendment.target_chapter != Some(3)),
                "no amendment targets the drafted chapter: {staged:?}"
            );
        }

        #[tokio::test]
        async fn cascade_detects_key_fact_and_promise_payoff_binding_changes() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade bindings", Some(3)).await;
            let placement = StoryPlacement {
                book_number: 1,
                chapter_number: 1,
                scene_order: Some(2),
                note: None,
            };
            svc.create_narrative_promise(CreateNarrativePromiseInput {
                project_id: project.project_id.clone(),
                promise_type: "mystery".to_string(),
                description: "the forged seal".to_string(),
                planted_at: StoryPlacement {
                    book_number: 1,
                    chapter_number: 1,
                    scene_order: Some(1),
                    note: None,
                },
                planned_payoff: Some(placement.clone()),
                planned_payoff_turning_beat: None,
                target_act: None,
                notes: Vec::new(),
            })
            .await
            .unwrap();
            svc.register_canonical_fact(RegisterCanonicalFactInput {
                project_id: project.project_id.clone(),
                scene_id: None,
                book_number: 1,
                chapter_number: 1,
                fact_type: None,
                key: Some("seal".to_string()),
                value: Some("the seal was forged".to_string()),
                context: None,
                subject_table: None,
                subject_id: None,
                predicate: Some("key_fact.revealed_secret:seal".to_string()),
                value_kind: None,
                value_text: Some("the seal was forged".to_string()),
                value_number: None,
                value_unit: None,
                value_json: None,
                aliases: Vec::new(),
                scope: None,
                valid_from: Some(placement),
                valid_until: None,
                legacy_untyped: None,
                supersedes_fact_id: None,
                secrecy: None,
            })
            .await
            .unwrap();

            // Dropping the Scene both bindings resolve to changes the binding
            // (and, incidentally, the closing-Charge vector).
            let drop = stage(
                &svc,
                &project.project_id,
                "scene_drop",
                Some(1),
                serde_json::json!({"scene_order": 2}),
            )
            .await;
            let out = apply_one(&svc, &project.project_id, &drop).await;
            let outcome = out.results[0].cascade.as_ref().expect("cascade ran");
            assert!(
                outcome
                    .triggers
                    .contains(&PlanAmendmentCascadeTrigger::KeyFact),
                "a Key Fact's planned-Scene binding changed: {outcome:?}"
            );
            assert!(
                outcome
                    .triggers
                    .contains(&PlanAmendmentCascadeTrigger::PromisePayoff),
                "a promise payoff binding changed: {outcome:?}"
            );
            assert!(
                outcome
                    .triggers
                    .contains(&PlanAmendmentCascadeTrigger::ClosingCharge),
                "{outcome:?}"
            );
        }

        #[tokio::test]
        async fn cascade_drop_renumbers_and_marks_the_renumbered_scene() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade drop", Some(3)).await;
            let drop = stage(
                &svc,
                &project.project_id,
                "scene_drop",
                Some(2),
                serde_json::json!({"scene_order": 1}),
            )
            .await;
            let out = apply_one(&svc, &project.project_id, &drop).await;
            assert_eq!(out.results[0].outcome, "applied", "{out:?}");

            let plan = plan_of(&svc, &project.project_id, 2).await;
            assert_eq!(plan.scenes.len(), 1, "the drop renumbers the chapter");
            assert_eq!(
                plan.scenes[0].summary, "c2s2",
                "the former second Scene is renumbered to order 1"
            );

            let list = stale_list(&svc, &project.project_id).await;
            let renamed = list
                .stale_scenes
                .iter()
                .find(|stale| stale.chapter_number == 2 && stale.scene_order == 1)
                .expect("the renumbered downstream Scene is stale: {list:?}");
            assert!(
                renamed
                    .finding_codes
                    .contains(&"value_chain_broken".to_string()),
                "{renamed:?}"
            );
            let reconciliation_id = renamed
                .reconciliation_amendment_id
                .clone()
                .expect("the renumbered Scene has a mechanical Charge repair");
            let staged = staged_amendments(&svc, &project.project_id).await;
            let reconciliation = staged
                .iter()
                .find(|amendment| amendment.id == reconciliation_id)
                .expect("the reconciliation is in the normal queue");
            assert_eq!(reconciliation.amendment_class, "scene_replace");
            assert_eq!(reconciliation.target_chapter, Some(2));
            assert_eq!(
                reconciliation.payload["scene_order"], 1,
                "the repair targets the renumbered placement"
            );
        }

        #[tokio::test]
        async fn cascade_reorder_marks_renumbered_downstream_scenes() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade reorder", Some(3)).await;
            let reorder = stage(
                &svc,
                &project.project_id,
                "scene_reorder",
                Some(2),
                serde_json::json!({"new_order": [2, 1]}),
            )
            .await;
            let out = apply_one(&svc, &project.project_id, &reorder).await;
            assert_eq!(out.results[0].outcome, "applied", "{out:?}");
            let outcome = out.results[0].cascade.as_ref().expect("reorder triggers");
            assert!(
                outcome
                    .triggers
                    .contains(&spindle_core::models::PlanAmendmentCascadeTrigger::ClosingCharge),
                "{outcome:?}"
            );

            let list = stale_list(&svc, &project.project_id).await;
            let marked: Vec<i32> = list
                .stale_scenes
                .iter()
                .filter(|stale| stale.chapter_number == 2 && stale.status == "stale")
                .map(|stale| stale.scene_order)
                .collect();
            assert_eq!(
                marked,
                vec![1, 2],
                "both renumbered Scenes are stale: {list:?}"
            );
            let first = list
                .stale_scenes
                .iter()
                .find(|stale| stale.chapter_number == 2 && stale.scene_order == 1)
                .unwrap();
            assert!(
                first.reconciliation_amendment_id.is_some(),
                "the first renumbered Scene has a repair: {first:?}"
            );
        }

        #[tokio::test]
        async fn cascade_opening_charge_only_change_triggers_and_includes_the_amended_scene() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade opening only", Some(3)).await;
            // c2s1 keeps its closing Charge but opens at the wrong Charge, so the
            // closing-Charge signature is unchanged and only the positional /
            // opening diff can trigger the cascade.
            let rewrite = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(2),
                axis_repair(
                    1,
                    "c2s1",
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                ),
            )
            .await;
            let out = apply_one(&svc, &project.project_id, &rewrite).await;
            assert_eq!(out.results[0].outcome, "applied", "{out:?}");
            let outcome = out.results[0]
                .cascade
                .as_ref()
                .expect("an opening-only change triggers the cascade");
            assert!(
                outcome
                    .triggers
                    .contains(&PlanAmendmentCascadeTrigger::OpeningCharge),
                "{outcome:?}"
            );
            assert!(
                !outcome
                    .triggers
                    .contains(&PlanAmendmentCascadeTrigger::ClosingCharge),
                "the closing signature did not change: {outcome:?}"
            );

            let list = stale_list(&svc, &project.project_id).await;
            let amended = list
                .stale_scenes
                .iter()
                .find(|stale| stale.chapter_number == 2 && stale.scene_order == 1)
                .expect("the amended Scene itself is in scope: {list:?}");
            assert!(
                amended
                    .finding_codes
                    .contains(&"value_chain_broken".to_string()),
                "{amended:?}"
            );
            assert!(
                amended.reconciliation_amendment_id.is_some(),
                "the amended Scene's chain break is repairable: {amended:?}"
            );
        }

        #[tokio::test]
        async fn cascade_marks_an_undrafted_scene_in_a_partly_drafted_chapter() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade partly drafted", None).await;
            // Chapter 2 scene 2 is drafted; scene 1 is still unwritten.
            svc.save_scene_draft(SaveSceneDraftInput {
                project_id: project.project_id.clone(),
                book_number: 1,
                chapter_number: 2,
                chapter_id: None,
                scene_order: 2,
                full_text: "Drafted c2s2 prose.".to_string(),
                summary: "the drafted second scene".to_string(),
                content_rating: spindle_core::models::ContentRating::General,
                tone: None,
                ..Default::default()
            })
            .await
            .unwrap();

            let trigger = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                axis_repair(
                    3,
                    "c1s3",
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                ),
            )
            .await;
            apply_one(&svc, &project.project_id, &trigger).await;

            let list = stale_list(&svc, &project.project_id).await;
            assert!(
                list.stale_scenes
                    .iter()
                    .any(|stale| stale.chapter_number == 2 && stale.scene_order == 1),
                "an undrafted Scene in a partly drafted chapter is validated: {list:?}"
            );
            assert!(
                list.stale_scenes
                    .iter()
                    .all(|stale| !(stale.chapter_number == 2 && stale.scene_order == 2)),
                "the drafted Scene itself is never marked: {list:?}"
            );
        }

        #[tokio::test]
        async fn cascade_keeps_a_mark_whose_scene_became_drafted() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade became drafted", Some(3)).await;
            let first = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                axis_repair(
                    3,
                    "c1s3",
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                ),
            )
            .await;
            apply_one(&svc, &project.project_id, &first).await;
            let before = stale_list(&svc, &project.project_id).await;
            let mark = before
                .stale_scenes
                .iter()
                .find(|stale| stale.chapter_number == 2 && stale.scene_order == 1)
                .expect("c2s1 is stale")
                .clone();

            // The Scene is drafted after it was marked stale.
            svc.save_scene_draft(SaveSceneDraftInput {
                project_id: project.project_id.clone(),
                book_number: 1,
                chapter_number: 2,
                chapter_id: None,
                scene_order: 1,
                full_text: "Drafted c2s1 prose.".to_string(),
                summary: "the drafted stale scene".to_string(),
                content_rating: spindle_core::models::ContentRating::General,
                tone: None,
                ..Default::default()
            })
            .await
            .unwrap();

            // A later upstream trigger re-runs the cascade. A drafted Scene is
            // excluded from the failing set, but that is not a repair: the mark
            // must remain as honest stale/drafted history.
            let second = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                axis_repair(
                    1,
                    "c1s1",
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                ),
            )
            .await;
            apply_one(&svc, &project.project_id, &second).await;

            let after = stale_list(&svc, &project.project_id).await;
            let kept = after
                .stale_scenes
                .iter()
                .find(|stale| stale.chapter_number == 2 && stale.scene_order == 1)
                .expect("a mark whose Scene became drafted is not silently cleared");
            assert_eq!(kept.id, mark.id, "the row is kept, not recreated");
            assert_eq!(kept.status, "stale");
        }

        #[tokio::test]
        async fn cascade_ignores_a_prose_only_summary_edit() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade prose only", Some(3)).await;
            // A promise payoff and a Key Fact both resolve to c1s2. Rewriting
            // that Scene's summary must not look like its binding changed.
            svc.create_narrative_promise(CreateNarrativePromiseInput {
                project_id: project.project_id.clone(),
                promise_type: "mystery".to_string(),
                description: "the forged seal".to_string(),
                planted_at: StoryPlacement {
                    book_number: 1,
                    chapter_number: 1,
                    scene_order: Some(1),
                    note: None,
                },
                planned_payoff: Some(StoryPlacement {
                    book_number: 1,
                    chapter_number: 1,
                    scene_order: Some(2),
                    note: None,
                }),
                planned_payoff_turning_beat: None,
                target_act: None,
                notes: Vec::new(),
            })
            .await
            .unwrap();
            svc.register_canonical_fact(RegisterCanonicalFactInput {
                project_id: project.project_id.clone(),
                scene_id: None,
                book_number: 1,
                chapter_number: 1,
                fact_type: None,
                key: Some("seal".to_string()),
                value: Some("the seal was forged".to_string()),
                context: None,
                subject_table: None,
                subject_id: None,
                predicate: Some("key_fact.revealed_secret:seal".to_string()),
                value_kind: None,
                value_text: Some("the seal was forged".to_string()),
                value_number: None,
                value_unit: None,
                value_json: None,
                aliases: Vec::new(),
                scope: None,
                valid_from: Some(StoryPlacement {
                    book_number: 1,
                    chapter_number: 1,
                    scene_order: Some(2),
                    note: None,
                }),
                valid_until: None,
                legacy_untyped: None,
                supersedes_fact_id: None,
                secrecy: None,
            })
            .await
            .unwrap();

            let prose = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                serde_json::json!({
                    "scene_order": 2,
                    "summary": "a wholly rewritten summary of the same Scene",
                    "purpose": "the same purpose",
                    "character_ids": ["character:alice"],
                }),
            )
            .await;
            let out = apply_one(&svc, &project.project_id, &prose).await;
            assert_eq!(out.results[0].outcome, "applied", "{out:?}");
            assert!(
                out.results[0].cascade.is_none(),
                "prose-only edits never trigger the cascade: {out:?}"
            );
            assert!(
                stale_list(&svc, &project.project_id)
                    .await
                    .stale_scenes
                    .is_empty()
            );
        }

        #[tokio::test]
        async fn cascade_stages_a_promise_followup_for_a_secret_reveal_without_promise() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade promise repair", Some(3)).await;
            svc.register_canonical_fact(RegisterCanonicalFactInput {
                project_id: project.project_id.clone(),
                scene_id: None,
                book_number: 1,
                chapter_number: 2,
                fact_type: None,
                key: Some("seal".to_string()),
                value: Some("the seal was forged".to_string()),
                context: None,
                subject_table: Some("character".to_string()),
                subject_id: Some("character:alice".to_string()),
                predicate: Some("key_fact.revealed_secret:seal".to_string()),
                value_kind: None,
                value_text: Some("the seal was forged".to_string()),
                value_number: None,
                value_unit: None,
                value_json: None,
                aliases: Vec::new(),
                scope: None,
                valid_from: Some(StoryPlacement {
                    book_number: 1,
                    chapter_number: 2,
                    scene_order: Some(2),
                    note: None,
                }),
                valid_until: None,
                legacy_untyped: None,
                supersedes_fact_id: None,
                secrecy: None,
            })
            .await
            .unwrap();

            // Break the chain at c1s2 only, so c2s2 stays in scope but its value
            // chain passes; its only failure is the missing promise.
            let trigger = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                axis_repair(
                    1,
                    "c1s1",
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                ),
            )
            .await;
            apply_one(&svc, &project.project_id, &trigger).await;

            let list = stale_list(&svc, &project.project_id).await;
            let reveal = list
                .stale_scenes
                .iter()
                .find(|stale| stale.chapter_number == 2 && stale.scene_order == 2)
                .expect("the unprepared reveal is stale: {list:?}");
            assert!(
                reveal
                    .finding_codes
                    .contains(&"key_fact_secret_reveal_without_promise".to_string()),
                "{reveal:?}"
            );
            let reconciliation_id = reveal
                .reconciliation_amendment_id
                .clone()
                .expect("a missing setup promise has a mechanical repair");
            let staged = staged_amendments(&svc, &project.project_id).await;
            let reconciliation = staged
                .iter()
                .find(|amendment| amendment.id == reconciliation_id)
                .expect("the promise followup is staged in the normal queue");
            assert_eq!(reconciliation.amendment_class, "promise_followup");
            assert_eq!(reconciliation.target_chapter, None);
            assert_eq!(
                reconciliation.payload["planned_payoff"]["chapter_number"],
                2
            );
            assert_eq!(reconciliation.payload["planned_payoff"]["scene_order"], 2);
            assert_eq!(
                reconciliation.payload["planted_at_placement"]["chapter_number"],
                2
            );
            assert_eq!(
                reconciliation.payload["planted_at_placement"]["scene_order"], 1,
                "the promise is planted at the latest earlier undrafted placement"
            );

            // Applying the followup is NOT a blanket no-op: the promise now
            // backs the reveal, so the cascade clears the mark it was staged for.
            let out = apply_one(&svc, &project.project_id, &reconciliation_id).await;
            assert_eq!(out.results[0].outcome, "applied", "{out:?}");
            let outcome = out.results[0]
                .cascade
                .as_ref()
                .expect("a promise_followup apply re-validates downstream");
            assert!(
                outcome
                    .triggers
                    .contains(&spindle_core::models::PlanAmendmentCascadeTrigger::PromiseFollowup),
                "{outcome:?}"
            );
            assert_eq!(outcome.cleared_stale, 1, "{outcome:?}");
            assert!(
                stale_list(&svc, &project.project_id)
                    .await
                    .stale_scenes
                    .iter()
                    .all(|stale| !(stale.chapter_number == 2 && stale.scene_order == 2)),
                "the repaired reveal is no longer stale"
            );
            let promises = svc
                .repository()
                .list_narrative_promises_by_project_and_branch(
                    &project.project_id,
                    &project.branch_id,
                )
                .await
                .unwrap();
            assert!(
                promises.iter().any(|promise| {
                    promise.planted_at.chapter_number == 2
                        && promise.planted_at.scene_order == Some(1)
                        && promise.planned_payoff.as_ref().is_some_and(|payoff| {
                            payoff.chapter_number == 2 && payoff.scene_order == Some(2)
                        })
                }),
                "the created promise carries the reconciliation placements: {promises:?}"
            );
        }

        #[tokio::test]
        async fn cascade_staged_ancestor_promise_event_is_not_marked_stale() {
            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade staged ancestor", None).await;
            // Re-plan chapter 2 scene 1 with an information-dependent Turning
            // Beat whose promise exists only in a staged planning proposal.
            let mut scenes = vec![
                value_scene(
                    1,
                    "c2s1",
                    value_charge(ChargeSign::Negative, ChargeDepth::NegationOfTheNegation),
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                ),
                value_scene(
                    2,
                    "c2s2",
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                ),
            ];
            scenes[0].beats[1].information_dependent = true;
            scenes[0].beats[1].promise_id = Some("promise:staged".to_string());
            svc.plan_chapter(PlanChapterInput {
                project_id: project.project_id.clone(),
                book_number: 1,
                chapter_number: 2,
                pov_character_id: None,
                synopsis: "chapter 2".to_string(),
                target_theme_ids: Vec::new(),
                target_conflict_ids: Vec::new(),
                target_plot_line_ids: Vec::new(),
                scenes,
            })
            .await
            .unwrap();

            use spindle_core::planning::{
                BeginPlanningInput, PlannedPromiseAction, PlannedPromiseEvent,
                PlanningCreateNarrativePromiseInput, PlanningLevel, PlanningOperation,
                PlanningOperationItem, PlanningRef, PlanningSchedule, PlanningScope,
                PlanningStoryPlacement, StagePlanningInput,
            };
            let session = svc
                .begin_planning(BeginPlanningInput {
                    premise: "staged ancestor promise".to_string(),
                    scope: Some(PlanningScope {
                        project_id: project.project_id.clone(),
                        branch_id: project.branch_id.clone(),
                    }),
                })
                .await
                .unwrap();
            let promise = PlanningCreateNarrativePromiseInput {
                promise_type: "mystery".to_string(),
                description: "the staged promise".to_string(),
                planted_at: PlanningStoryPlacement {
                    book_number: 1,
                    chapter_number: 1,
                    scene_order: Some(1),
                    note: None,
                },
                planned_payoff: Some(PlanningStoryPlacement {
                    book_number: 1,
                    chapter_number: 2,
                    scene_order: Some(1),
                    note: None,
                }),
                planned_payoff_turning_beat: None,
                target_act: None,
                notes: Vec::new(),
            };
            let staged = svc
                .stage_planning(StagePlanningInput {
                    session_id: session.id.clone(),
                    scope: session.scope.clone(),
                    intent_hash: session.intent_hash.clone(),
                    schema_version: 1,
                    level: PlanningLevel::SeriesArc,
                    parents: std::collections::BTreeMap::new(),
                    operations: vec![PlanningOperationItem {
                        output_ref: "promise:staged".to_string(),
                        operation: PlanningOperation::CreateNarrativePromise(promise.clone()),
                    }],
                    rationale: "stage the promise the later reveal pays off".to_string(),
                    target_range: None,
                    schedule: PlanningSchedule {
                        promise_events: vec![
                            PlannedPromiseEvent {
                                promise_ref: PlanningRef::ProposalRef("promise:staged".to_string()),
                                action: PlannedPromiseAction::Plant,
                                placement: PlanningStoryPlacement {
                                    book_number: 1,
                                    chapter_number: 1,
                                    scene_order: Some(1),
                                    note: None,
                                },
                                note: None,
                            },
                            PlannedPromiseEvent {
                                promise_ref: PlanningRef::ProposalRef("promise:staged".to_string()),
                                action: PlannedPromiseAction::Payoff,
                                placement: PlanningStoryPlacement {
                                    book_number: 1,
                                    chapter_number: 2,
                                    scene_order: Some(1),
                                    note: None,
                                },
                                note: None,
                            },
                        ],
                        ..Default::default()
                    },
                    assumptions: Vec::new(),
                    author_questions: Vec::new(),
                })
                .await
                .unwrap();
            assert_eq!(
                staged.status,
                spindle_core::planning::PlanningProposalStatus::Staged
            );

            let trigger = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(1),
                axis_repair(
                    1,
                    "c1s1",
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                ),
            )
            .await;
            apply_one(&svc, &project.project_id, &trigger).await;

            let list = stale_list(&svc, &project.project_id).await;
            assert!(
                list.stale_scenes
                    .iter()
                    .all(|stale| !(stale.chapter_number == 2 && stale.scene_order == 1)),
                "the gate-visible staged-ancestor promise prevents a stale mark: {list:?}"
            );
            assert!(
                list.stale_scenes.iter().all(|stale| {
                    !stale
                        .finding_codes
                        .iter()
                        .any(|code| code.starts_with("turning_beat_"))
                }),
                "no Turning Beat finding was produced: {list:?}"
            );
        }

        #[tokio::test]
        async fn cascade_batch_rolls_back_staged_repairs_and_marks_on_failure() {
            use crate::sqlite::repository::{
                MarkScenePlanStaleParams, ScenePlanStaleBatch, ScenePlanStaleMarkWrite,
                StagePlanAmendmentParams,
            };

            let (_tmp, svc) = fresh_service_local().await;
            let project = cascade_fixture(&svc, "Cascade rollback", Some(3)).await;
            let batch = ScenePlanStaleBatch {
                project_id: project.project_id.clone(),
                branch_id: project.branch_id.clone(),
                decided_by: "operator".to_string(),
                marks: vec![ScenePlanStaleMarkWrite {
                    mark: MarkScenePlanStaleParams {
                        project_id: project.project_id.clone(),
                        branch_id: project.branch_id.clone(),
                        book_number: 1,
                        chapter_number: 2,
                        scene_order: 1,
                        reason: "forced failure".to_string(),
                        finding_codes: vec!["value_chain_broken".to_string()],
                        // Dangling FK: the mark insert fails after the stage insert,
                        // so the whole transaction must roll back.
                        triggering_amendment_id: "plan_amendment:missing".to_string(),
                        reconciliation_amendment_id: None,
                    },
                    stage: Some(StagePlanAmendmentParams {
                        project_id: project.project_id.clone(),
                        branch_id: project.branch_id.clone(),
                        source_chapter: 1,
                        book_number: 1,
                        authoring_run_id: None,
                        amendment_class: "scene_replace".to_string(),
                        target_chapter: Some(2),
                        payload: serde_json::json!({
                            "scene_order": 1,
                            "summary": "c2s1",
                            "purpose": "purpose 1",
                            "character_ids": ["character:alice"],
                        }),
                        rationale: "rolled back repair".to_string(),
                        confidence: "high".to_string(),
                    }),
                    supersede: Vec::new(),
                }],
                clears: Vec::new(),
            };
            let err = svc
                .repository()
                .apply_scene_plan_stale_batch(batch)
                .await
                .expect_err("a dangling trigger reference must fail the batch");
            assert!(
                format!("{err:#}").to_lowercase().contains("foreign key"),
                "the failure is the injected FK violation: {err:#}"
            );
            assert!(
                staged_amendments(&svc, &project.project_id)
                    .await
                    .is_empty(),
                "the staged repair is rolled back with the failed mark"
            );
            assert!(
                stale_list(&svc, &project.project_id)
                    .await
                    .stale_scenes
                    .is_empty(),
                "no orphan mark survives the failed transaction"
            );
        }

        #[tokio::test]
        async fn non_mckee_amendment_outputs_and_plans_stay_unchanged() {
            let (_tmp, svc) = fresh_service_local().await;
            let scenes = vec![
                value_scene(
                    1,
                    "open",
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                ),
                value_scene(
                    2,
                    "close",
                    value_charge(ChargeSign::Negative, ChargeDepth::Contrary),
                    value_charge(ChargeSign::Negative, ChargeDepth::Contradictory),
                ),
            ];
            let project =
                project_with_future_chapter(&svc, "Cascade unmarked", 2, scenes, Vec::new()).await;
            let before = plan_of(&svc, &project.project_id, 2).await;
            let id = stage(
                &svc,
                &project.project_id,
                "scene_replace",
                Some(2),
                axis_repair(
                    1,
                    "open",
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                    value_charge(ChargeSign::Positive, ChargeDepth::Positive),
                ),
            )
            .await;
            // Issue #34 fix: the value-plan payload keys are McKee-only. A
            // non-McKee apply must REJECT them explicitly instead of silently
            // discarding them (the pre-#34 pinned discard).
            let err = svc
                .decide_plan_amendments(DecidePlanAmendmentsInput {
                    project_id: project.project_id.clone(),
                    decisions: vec![PlanAmendmentDecisionInput {
                        amendment_id: id.clone(),
                        action: "apply".to_string(),
                        edit: None,
                        note: None,
                    }],
                    decided_by: None,
                })
                .await
                .expect_err("a non-McKee value-plan payload must be refused");
            assert!(
                err.to_string().contains("not McKee-structured"),
                "the rejection names the asymmetry readably: {err:#}"
            );
            let after = plan_of(&svc, &project.project_id, 2).await;
            assert_eq!(
                before.scenes[0].primary_value_axis, after.scenes[0].primary_value_axis,
                "zero writes: the non-McKee plan is untouched"
            );

            let list = svc
                .list_plan_amendments(ListPlanAmendmentsInput {
                    project_id: project.project_id.clone(),
                    status: None,
                    book_number: None,
                    source_chapter: None,
                })
                .await
                .unwrap();
            let value = serde_json::to_value(&list).unwrap();
            assert!(
                value.get("stale_scenes").is_none(),
                "a non-McKee list output has no stale key: {value:?}"
            );
            let staged = staged_amendments(&svc, &project.project_id).await;
            assert_eq!(
                staged.len(),
                1,
                "only the rejected amendment remains staged"
            );
            assert_eq!(staged[0].id, id, "nothing downstream is staged: {staged:?}");
            assert_eq!(
                list.amendments
                    .iter()
                    .find(|amendment| amendment.id == id)
                    .map(|amendment| amendment.status.as_str()),
                Some("staged"),
                "the refused apply leaves the decision unrecorded"
            );
        }
    }
}
