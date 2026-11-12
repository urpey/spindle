//! McKee Key Facts and Narrative Promises: the planning-level gates.
//!
//! Three rules live here, all stage-recordable Blocking findings so an author
//! sees them on the staged proposal before acceptance:
//!
//! * a **long-range promise** (planned at the Series-Arc or Book level) must
//!   name a `target_act` that exists in the accepted/merged Act list; a
//!   short-range Chapter/Scene placement may omit it;
//! * a **secret-reveal Key Fact** (`revealed_secret`) must pay off an earlier
//!   planted promise whose planned payoff lands in that Scene;
//! * an **information-dependent Turning Beat** must name a promise planted at an
//!   earlier placement than its Scene.
//!
//! The merged sources are the same ones the other McKee gates use: accepted
//! `narrative_promise` rows visible from the branch lineage, promises declared
//! by staged ancestors and the current artifact (all folded into
//! [`ValidatorContext`]), plus accepted, staged-ancestor and current planned
//! Scenes (one merge, shared with the value-chain pass). A planned payoff is
//! matched to a Scene by exact `(book_number, chapter_number, scene_order)`; a
//! chapter-wide payoff placement does not name a Scene and therefore cannot
//! satisfy a Scene-level reveal.
//!
//! The existing payoff-before-setup ordering rule stays where it already lives
//! (the generic operation/schedule validation) and is not weakened or
//! duplicated here.

use super::values::{SceneGateEntry, evaluated_gate_scenes_in_conn};
use super::*;
use spindle_core::beats::BeatKind;
use spindle_core::models::PlannedScene;
use spindle_core::planning::{PlannedPromiseAction, PlanningOperationItem};
use spindle_core::subject::SubjectTable;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

/// Whether a promise planned at this level is long-range (Series-Arc / Book).
fn is_long_range_level(level: PlanningLevel) -> bool {
    matches!(level, PlanningLevel::SeriesArc | PlanningLevel::Book)
}

/// A promise as the deus-ex-machina gate reads it: where it is planted and
/// every planned payoff placement known for it.
struct PromiseGate {
    planted_at: Option<PlanningStoryPlacement>,
    payoffs: Vec<PlanningStoryPlacement>,
}

impl PromiseGate {
    fn planted_before(&self, scene: &PlanningStoryPlacement) -> bool {
        self.planted_at
            .as_ref()
            .is_some_and(|planted| planted.cmp_story_order(scene) == std::cmp::Ordering::Less)
    }

    fn pays_off_at(&self, scene: &PlanningStoryPlacement) -> bool {
        self.payoffs.iter().any(|payoff| same_scene(payoff, scene))
    }
}

fn same_scene(left: &PlanningStoryPlacement, right: &PlanningStoryPlacement) -> bool {
    left.book_number == right.book_number
        && left.chapter_number == right.chapter_number
        && left.scene_order == right.scene_order
}

/// Gate entry point. Records findings and never changes data.
pub(super) fn validate_mckee_promises(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    scope: &Option<PlanningScope>,
    ctx: &ValidatorContext,
    findings: &mut Vec<PlanningFinding>,
) -> Result<()> {
    let Some(scope) = scope else {
        return Ok(());
    };
    if !project_is_mckee_in_conn(conn, &scope.project_id)? {
        return Ok(());
    }
    let scenes = evaluated_gate_scenes_in_conn(conn, input, scope)?;
    validate_promise_operations(conn, input, scope, &scenes, findings)?;

    let promises = merged_promise_gates(input, ctx);
    validate_information_dependent_turning_beats(&scenes, &promises, ctx, findings);
    validate_key_facts(conn, input, &promises, findings)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Narrative promise operations
// ---------------------------------------------------------------------------

fn validate_promise_operations(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    scope: &PlanningScope,
    scenes: &[SceneGateEntry],
    findings: &mut Vec<PlanningFinding>,
) -> Result<()> {
    let long_range = is_long_range_level(input.level);
    let mut evaluated_acts: Option<Vec<PlannedAct>> = None;
    for (index, item) in input.operations.iter().enumerate() {
        let PlanningOperation::CreateNarrativePromise(promise) = &item.operation else {
            continue;
        };
        let path = format!("/operations/{index}/operation/input");

        match promise.target_act {
            Some(order) if order < 1 => findings.push(planning_finding(
                "promise_target_act_invalid",
                PlanningFindingSeverity::Blocking,
                format!("{path}/target_act"),
                vec![item.output_ref.clone()],
                "a 1-based Act order".to_string(),
                order.to_string(),
                "Name the Act's 1-based order in the accepted/merged Act list.",
            )),
            Some(order) => {
                if evaluated_acts.is_none() {
                    evaluated_acts = Some(evaluated_mckee_acts_in_conn(conn, input, scope)?);
                }
                let acts = evaluated_acts.as_deref().unwrap_or_default();
                if !acts.iter().any(|act| act.order == order) {
                    findings.push(planning_finding(
                        "promise_target_act_unknown",
                        PlanningFindingSeverity::Blocking,
                        format!("{path}/target_act"),
                        vec![item.output_ref.clone()],
                        format!("an Act order in the merged Act list {:?}", acts.iter().map(|act| act.order).collect::<Vec<_>>()),
                        format!("Act order {order}"),
                        "Point target_act at an Act the accepted/merged Act list actually contains (or plan that Act first).",
                    ));
                }
            }
            None if long_range => findings.push(planning_finding(
                "promise_long_range_missing_target_act",
                PlanningFindingSeverity::Blocking,
                format!("{path}/target_act"),
                vec![item.output_ref.clone()],
                "a target Act order for a long-range (Series-Arc/Book) promise".to_string(),
                "no target_act".to_string(),
                "Give this promise a target_act: long-range promises must have a planned Act destination, even when the payoff scene is outside the current horizon.",
            )),
            None => {}
        }

        if let Some(position) = promise.planned_payoff_turning_beat {
            let beat_path = format!("{path}/planned_payoff_turning_beat");
            if position < 1 {
                findings.push(planning_finding(
                    "promise_payoff_turning_beat_invalid",
                    PlanningFindingSeverity::Blocking,
                    beat_path,
                    vec![item.output_ref.clone()],
                    "a 1-based Beat position".to_string(),
                    position.to_string(),
                    "Number the payoff Turning Beat from 1, exactly like the Scene's turning_beat.",
                ));
            } else if let Some(payoff) = promise.planned_payoff.as_ref() {
                if let Some(scene) = scenes
                    .iter()
                    .find(|scene| same_scene(&scene.placement(), payoff))
                {
                    let position = position as usize;
                    if position > scene.beats.len() {
                        findings.push(planning_finding(
                            "promise_payoff_turning_beat_out_of_range",
                            PlanningFindingSeverity::Blocking,
                            beat_path,
                            vec![item.output_ref.clone()],
                            format!(
                                "a 1-based position in the payoff Scene's {} Beat(s)",
                                scene.beats.len()
                            ),
                            format!("position {position}"),
                            "Point the payoff Turning Beat at an existing Beat of the planned payoff Scene.",
                        ));
                    } else if scene.beats[position - 1].kind != BeatKind::Turn {
                        findings.push(planning_finding(
                            "promise_payoff_turning_beat_not_turn",
                            PlanningFindingSeverity::Blocking,
                            beat_path,
                            vec![item.output_ref.clone()],
                            "a Beat of kind 'turn'".to_string(),
                            format!("kind '{}'", scene.beats[position - 1].kind),
                            "A payoff Turning Beat reference names the Scene's Turn Beat; reference that Beat, or leave the field unset when the payoff is not on the Turning Beat.",
                        ));
                    }
                }
            } else {
                findings.push(planning_finding(
                    "promise_payoff_turning_beat_without_payoff",
                    PlanningFindingSeverity::Blocking,
                    beat_path,
                    vec![item.output_ref.clone()],
                    "planned_payoff with the payoff Turning Beat".to_string(),
                    "no planned_payoff".to_string(),
                    "Set planned_payoff (the Scene that owns the Beat), or drop planned_payoff_turning_beat.",
                ));
            }
        }
    }
    Ok(())
}

/// The merged promise view: accepted rows visible from the branch lineage,
/// promises declared by staged ancestors and the current artifact (already
/// folded into `ctx.promises`), plus the planned payoff placements recorded by
/// the merged promise schedule.
fn merged_promise_gates(
    input: &StagePlanningInput,
    ctx: &ValidatorContext,
) -> BTreeMap<String, PromiseGate> {
    let mut promises: BTreeMap<String, PromiseGate> = ctx
        .promises
        .iter()
        .map(|(key, def)| {
            (
                key.clone(),
                PromiseGate {
                    planted_at: Some(def.planted_at.clone()),
                    payoffs: def.planned_payoff.iter().cloned().collect(),
                },
            )
        })
        .collect();
    for event in ctx
        .ancestor_promise_events
        .iter()
        .chain(input.schedule.promise_events.iter())
    {
        let key = ctx.normalize_ref(&event.promise_ref);
        let gate = promises.entry(key).or_insert_with(|| PromiseGate {
            planted_at: None,
            payoffs: Vec::new(),
        });
        match event.action {
            PlannedPromiseAction::Plant => {
                if gate.planted_at.is_none() {
                    gate.planted_at = Some(event.placement.clone());
                }
            }
            PlannedPromiseAction::Payoff if !gate.pays_off_at(&event.placement) => {
                gate.payoffs.push(event.placement.clone());
            }
            _ => {}
        }
    }
    promises
}

// ---------------------------------------------------------------------------
// Information-dependent Turning Beats
// ---------------------------------------------------------------------------

fn validate_information_dependent_turning_beats(
    scenes: &[SceneGateEntry],
    promises: &BTreeMap<String, PromiseGate>,
    ctx: &ValidatorContext,
    findings: &mut Vec<PlanningFinding>,
) {
    for scene in scenes {
        let Some(beat) = scene.turning_beat_entry() else {
            // A missing or out-of-range Turning Beat is already reported by the
            // Beat shape checks; there is nothing to judge here.
            continue;
        };
        if !beat.information_dependent {
            continue;
        }
        let position = scene.turning_beat.unwrap_or_default() as usize;
        let path = format!("{}/beats/{}", scene.path, position.saturating_sub(1));
        let Some(promise_ref) = beat.promise.as_ref() else {
            findings.push(planning_finding(
                "turning_beat_information_dependent_without_promise",
                PlanningFindingSeverity::Blocking,
                path,
                Vec::new(),
                "a promise this Turning Beat pays off, planted at an earlier placement".to_string(),
                "information-dependent without promise_id".to_string(),
                format!(
                    "{} Name the earlier-planted promise the reader was given, or make the Turn depend on what the Scene itself establishes.",
                    scene.repair_hint
                ),
            ));
            continue;
        };
        let key = ctx.normalize_ref(promise_ref);
        let Some(promise) = promises.get(&key) else {
            findings.push(planning_finding(
                "turning_beat_promise_unknown",
                PlanningFindingSeverity::Blocking,
                path,
                vec![key.clone()],
                "a narrative promise visible on this branch".to_string(),
                format!("unknown promise '{key}'"),
                format!(
                    "{} Reference a promise created in an accepted or earlier staged artifact.",
                    scene.repair_hint
                ),
            ));
            continue;
        };
        if !promise.planted_before(&scene.placement()) {
            findings.push(planning_finding(
                "turning_beat_promise_not_planted_earlier",
                PlanningFindingSeverity::Blocking,
                path,
                vec![key.clone()],
                format!(
                    "a promise planted earlier than book {} chapter {} scene {}",
                    scene.book_number,
                    scene.chapter_number,
                    scene.scene_order
                ),
                promise
                    .planted_at
                    .as_ref()
                    .map(|planted| {
                        format!(
                            "promise planted at book {} chapter {} scene {:?}",
                            planted.book_number, planted.chapter_number, planted.scene_order
                        )
                    })
                    .unwrap_or_else(|| "promise has no planned plant placement".to_string()),
                format!(
                    "{} Plant the promise in an earlier planned Scene before this Turning Beat pays it off.",
                    scene.repair_hint
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Key Facts
// ---------------------------------------------------------------------------

/// Validate Key Facts. A Key Fact is planned-and-pending: its placement names
/// the Scene that must dramatise it, but that Scene may still be unplanned, so
/// the placement only has to be structurally valid. When explicit
/// `planned_scene_id` is supplied it must resolve to a chapter plan that agrees
/// with the placement; a plan whose Scene does not exist yet cannot disagree.
fn validate_key_facts(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    promises: &BTreeMap<String, PromiseGate>,
    findings: &mut Vec<PlanningFinding>,
) -> Result<()> {
    for (index, item) in input.operations.iter().enumerate() {
        let PlanningOperation::CreateKeyFact(key_fact) = &item.operation else {
            continue;
        };
        let path = format!("/operations/{index}/operation/input");
        validate_key_fact_subject(key_fact, &path, &item.output_ref, findings);
        if key_fact.key.trim().is_empty() {
            findings.push(planning_finding(
                "key_fact_empty_key",
                PlanningFindingSeverity::Blocking,
                format!("{path}/key"),
                vec![item.output_ref.clone()],
                "a non-empty unique key".to_string(),
                "empty key".to_string(),
                "Give the Key Fact a stable key: it becomes the canonical fact's predicate.",
            ));
        }
        if key_fact.value.trim().is_empty() {
            findings.push(planning_finding(
                "key_fact_empty_value",
                PlanningFindingSeverity::Blocking,
                format!("{path}/value"),
                vec![item.output_ref.clone()],
                "a non-empty value".to_string(),
                "empty value".to_string(),
                "State the fact's canonical value text.",
            ));
        }

        let placement = &key_fact.placement;
        let placement_shape_ok = placement.has_positive_coordinates()
            && placement.scene_order.is_some_and(|order| order >= 1);
        if !placement_shape_ok {
            findings.push(planning_finding(
                "key_fact_invalid_placement",
                PlanningFindingSeverity::Blocking,
                format!("{path}/placement"),
                vec![item.output_ref.clone()],
                "positive book_number, chapter_number and a 1-based scene_order".to_string(),
                format!("{placement:?}"),
                "A Key Fact names the planned Scene that must dramatise it, so all three placement coordinates are required. The Scene itself may be planned later.",
            ));
        }

        if let Some(reference) = key_fact.planned_scene_id.as_ref() {
            let reference_path = format!("{path}/planned_scene_id");
            match resolved_planned_scene_reference(conn, input, reference)? {
                None => findings.push(planning_finding(
                    "key_fact_scene_reference_unknown",
                    PlanningFindingSeverity::Blocking,
                    reference_path,
                    vec![item.output_ref.clone()],
                    "a plan_chapter output or existing chapter_plan record".to_string(),
                    format!("unresolved {reference:?}"),
                    "Reference the planned chapter plan that owns the Scene, or drop planned_scene_id and keep the placement.",
                )),
                Some((book_number, chapter_number, scene_orders)) => {
                    let placement_order = placement.scene_order.unwrap_or_default();
                    if placement.book_number != book_number
                        || placement.chapter_number != chapter_number
                        || !scene_orders.contains(&placement_order)
                    {
                        findings.push(planning_finding(
                            "key_fact_scene_reference_mismatch",
                            PlanningFindingSeverity::Blocking,
                            reference_path,
                            vec![item.output_ref.clone()],
                            format!(
                                "a chapter plan for book {book_number} chapter {chapter_number} covering Scene {}",
                                placement.scene_order.unwrap_or_default()
                            ),
                            format!(
                                "placement is book {} chapter {} scene {:?}",
                                placement.book_number, placement.chapter_number, placement.scene_order
                            ),
                            "Make planned_scene_id and placement name the same planned Scene, or drop one of them.",
                        ));
                    }
                }
            }
        }

        if !placement_shape_ok {
            continue;
        }
        if key_fact.kind.is_secret_reveal() {
            if !secret_reveal_subject_is_character(conn, input, key_fact) {
                findings.push(planning_finding(
                    "key_fact_secret_reveal_requires_character_subject",
                    PlanningFindingSeverity::Blocking,
                    format!("{path}/subject_table"),
                    vec![item.output_ref.clone()],
                    "a 'character' subject_table with a character subject_id".to_string(),
                    format!(
                        "subject_table={:?}, subject_id={:?}",
                        key_fact.subject_table, key_fact.subject_id
                    ),
                    "A revealed_secret Key Fact is registered as a secret canonical fact whose initial holder is its subject character; name that character, or keep the secret in the existing secret-knowledge flow.",
                ));
            }
            let promised = promises
                .values()
                .any(|promise| promise.planted_before(placement) && promise.pays_off_at(placement));
            if !promised {
                findings.push(planning_finding(
                    "key_fact_secret_reveal_without_promise",
                    PlanningFindingSeverity::Blocking,
                    format!("{path}/kind"),
                    vec![item.output_ref.clone()],
                    "an earlier-planted promise whose planned payoff lands in this Scene".to_string(),
                    "revealed_secret with no matching promise".to_string(),
                    "Plant the promise in an earlier planned Scene and place its planned payoff on this Scene (ideally its Turning Beat); a reveal the story never set up is deus ex machina.",
                ));
            }
        }
    }
    Ok(())
}

/// Whether a secret-reveal Key Fact's subject is a character.
///
/// A proposal reference that is not resolvable yet (a staged-ancestor
/// `create_character`) is accepted here: the resolved record id is checked
/// again when the proposal applies.
fn secret_reveal_subject_is_character(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    key_fact: &PlanningCreateKeyFactInput,
) -> bool {
    if key_fact.subject_table.as_deref() != Some("character") {
        return false;
    }
    match key_fact.subject_id.as_ref() {
        Some(PlanningRef::ExistingRecordId(id)) => id.starts_with("character:"),
        Some(PlanningRef::ProposalRef(key)) => apply::refs_in_conn(conn, &input.session_id)
            .ok()
            .and_then(|refs| refs.get(key).cloned())
            .is_none_or(|id| id.starts_with("character:")),
        None => false,
    }
}

fn validate_key_fact_subject(
    key_fact: &PlanningCreateKeyFactInput,
    path: &str,
    output_ref: &str,
    findings: &mut Vec<PlanningFinding>,
) {
    match (&key_fact.subject_table, &key_fact.subject_id) {
        (None, None) => {}
        (Some(table), Some(id)) => {
            let parsed = SubjectTable::from_str(table);
            let known = parsed.is_ok();
            if !known {
                findings.push(planning_finding(
                    "key_fact_unknown_subject_table",
                    PlanningFindingSeverity::Blocking,
                    format!("{path}/subject_table"),
                    vec![output_ref.to_string()],
                    "a canonical-fact subject table (character, location, faction, …)".to_string(),
                    format!("'{table}'"),
                    "Use a subject table the existing canonical-fact model knows, or omit the subject for a project-wide fact.",
                ));
            }
            if let (Ok(expected), PlanningRef::ExistingRecordId(id)) = (parsed, id) {
                let prefix = id.split(':').next().unwrap_or("");
                if prefix != expected.as_str() {
                    findings.push(planning_finding(
                        "key_fact_subject_mismatch",
                        PlanningFindingSeverity::Blocking,
                        format!("{path}/subject_id"),
                        vec![output_ref.to_string()],
                        format!("a '{}' record id", expected.as_str()),
                        format!("'{id}'"),
                        "Make subject_table and subject_id name the same record.",
                    ));
                }
            }
        }
        (Some(_), None) => findings.push(planning_finding(
            "key_fact_subject_incomplete",
            PlanningFindingSeverity::Blocking,
            format!("{path}/subject_id"),
            vec![output_ref.to_string()],
            "subject_table together with subject_id".to_string(),
            "subject_table without subject_id".to_string(),
            "Give the subject's record id, or omit subject_table for a project-wide fact.",
        )),
        (None, Some(_)) => findings.push(planning_finding(
            "key_fact_subject_incomplete",
            PlanningFindingSeverity::Blocking,
            format!("{path}/subject_table"),
            vec![output_ref.to_string()],
            "subject_table together with subject_id".to_string(),
            "subject_id without subject_table".to_string(),
            "Give the subject's table too, or omit subject_id for a project-wide fact.",
        )),
    }
}

/// Resolve the optional planned-Scene reference to the chapter plan it names.
///
/// A proposal reference names a `plan_chapter` output (in this artifact, a
/// staged ancestor, or an accepted parent proposal whose receipt carries the
/// resolved `chapter_plan` id); an existing record id names a `chapter_plan`
/// row. `None` means the reference could not be resolved to a plan.
fn resolved_planned_scene_reference(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    reference: &PlanningRef,
) -> Result<Option<(i32, i32, BTreeSet<i32>)>> {
    match reference {
        PlanningRef::ExistingRecordId(id) => Ok(chapter_plan_shape_in_conn(conn, id)?),
        PlanningRef::ProposalRef(key) => {
            if let Some(plan) = plan_chapter_operation(&input.operations, key) {
                return Ok(Some((
                    plan.book_number,
                    plan.chapter_number,
                    plan.scenes.iter().map(|scene| scene.scene_order).collect(),
                )));
            }
            for ancestor in collect_staged_ancestor_proposals_in_conn(conn, input)? {
                if let Some(plan) = plan_chapter_operation(&ancestor.artifact.operations, key) {
                    return Ok(Some((
                        plan.book_number,
                        plan.chapter_number,
                        plan.scenes.iter().map(|scene| scene.scene_order).collect(),
                    )));
                }
            }
            let refs = apply::refs_in_conn(conn, &input.session_id)?;
            let Some(id) = refs.get(key) else {
                return Ok(None);
            };
            chapter_plan_shape_in_conn(conn, id)
        }
    }
}

fn plan_chapter_operation<'a>(
    operations: &'a [PlanningOperationItem],
    key: &str,
) -> Option<&'a PlanningPlanChapterInput> {
    operations
        .iter()
        .find(|item| item.output_ref == key)
        .and_then(|item| match &item.operation {
            PlanningOperation::PlanChapter(plan) => Some(plan),
            _ => None,
        })
}

fn chapter_plan_shape_in_conn(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<Option<(i32, i32, BTreeSet<i32>)>> {
    let row = conn
        .query_row(
            "SELECT book_number, chapter_number, scenes FROM chapter_plan WHERE id=?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional_inner()?;
    let Some((book_number, chapter_number, scenes_json)) = row else {
        return Ok(None);
    };
    let scenes: Vec<PlannedScene> = serde_json::from_str(&scenes_json).unwrap_or_default();
    Ok(Some((
        book_number,
        chapter_number,
        scenes.into_iter().map(|scene| scene.scene_order).collect(),
    )))
}
