//! McKee amendment cascade over unwritten Scenes (issue #34, parent spec #21).
//!
//! Accepting a plan amendment that changes a planned Scene's opening or closing
//! Charge (or the plan binding a Key Fact / Narrative Promise payoff relies on)
//! can leave every later planned, undrafted Scene in a broken value chain. This
//! module detects that from the amendment's recorded `prior_state` against the
//! plan the apply actually wrote (never from the amendment class alone), re-runs
//! the SAME value-chain and promise/Key-Fact rule functions the planning gate
//! uses (see `planning::values` / `planning::promises`), marks failing Scenes
//! stale (`scene_plan_stale`, V0062) and stages a reconciliation proposal through
//! the normal ADR 0003 lifecycle — never a parallel amendment table.
//!
//! Scope is explicitly later planned, undrafted Scenes: a drafted Scene has a
//! persisted row on the active branch and is never marked, repaired or rewritten
//! here (ADR 0003 D3). Drafting is decided per Scene placement, so an undrafted
//! Scene in a partly drafted chapter is still validated. The first changed
//! position comes from a positional diff of the prior/post planned-scene arrays,
//! never from the closing-Charge vector alone; for a drop or reorder the
//! renumbering puts everything from the first differing index onward in scope.
//! Scenes before that position are never touched.
//!
//! A `promise_followup` apply is not a no-op: the promise it executed is folded
//! into the same merged source set, and the Scenes after its executed placement
//! are re-validated so a repair can clear the mark it was staged for.
//!
//! **Honest limitations** (documented in `docs/planning-engine-design.md`): no
//! amendment class can edit a Key Fact row or an existing Narrative Promise, and
//! none can edit a planned Beat's `promise` reference. A failure whose only
//! repair would need one of those edits is marked stale with no proposal and an
//! explicit reason — never a fabricated empty repair.

use super::*;
use crate::sqlite::records::{ChapterPlan, Project, StoredPlanAmendment, StoredPlannedScene};
use crate::sqlite::repository::{
    CascadeSceneFinding, MarkScenePlanStaleParams, PromiseGate, ScenePlanStaleBatch,
    ScenePlanStaleClearWrite, ScenePlanStaleMarkWrite, StagePlanAmendmentParams,
    cascade_promise_findings, cascade_value_findings, merge_promise_gates,
};
use spindle_core::beats::BeatKind;
use spindle_core::charge::Charge;
use spindle_core::models::{
    ContentRating, PlanAmendmentCascadeOutcome, PlanAmendmentCascadeTrigger, PlannedScene,
};
use spindle_core::planning::{PlannedPromiseEvent, PlanningRef, PlanningStoryPlacement};
use spindle_core::scene_value::{SceneDilemma, SceneGap, SceneValueAxis};
use std::collections::{BTreeMap, BTreeSet};

/// What one applied amendment actually executed, handed to the cascade by the
/// dispatcher. `prior_state` is the pre-apply chapter plan (chapter-targeting
/// classes only); `created_promise_id` is the promise a `promise_followup`
/// created; `payload` is the *effective* payload (after an operator edit).
pub(super) struct AmendmentApplyEffect<'a> {
    pub prior_state: Option<&'a str>,
    pub created_promise_id: Option<&'a str>,
    pub payload: &'a serde_json::Value,
}

/// The Scene coordinates a cascade is allowed to touch.
struct CascadeScope {
    anchor_book: i32,
    anchor_chapter: i32,
    /// Scenes at or after this order in the anchor chapter are in scope.
    from_order: i32,
    /// Scenes a just-applied reconciliation repaired are re-validated even when
    /// the positional anchor lands later (defence in depth; the repair itself
    /// normally anchors here).
    repaired: BTreeSet<(i32, i32, i32)>,
}

impl CascadeScope {
    fn contains(&self, book_number: i32, chapter_number: i32, scene_order: i32) -> bool {
        if self
            .repaired
            .contains(&(book_number, chapter_number, scene_order))
        {
            return true;
        }
        (book_number, chapter_number) > (self.anchor_book, self.anchor_chapter)
            || (book_number == self.anchor_book
                && chapter_number == self.anchor_chapter
                && scene_order >= self.from_order)
    }
}

/// The plan-shape identity of one planned Scene: summary/value plan/cast/
/// location/beats. Deliberately NOT just the closing-Charge vector (a length-only
/// change would otherwise anchor at index 0 and sweep earlier Scenes).
#[derive(PartialEq)]
struct SceneIdentity<'a> {
    summary: &'a str,
    purpose: &'a str,
    beat_structure: &'a [String],
    beats: &'a [spindle_core::beats::PlannedBeat],
    turning_beat: Option<u32>,
    character_ids: &'a [String],
    location_id: Option<&'a str>,
    content_rating: Option<&'a ContentRating>,
    primary_value_axis: Option<&'a SceneValueAxis>,
    secondary_value_axes: &'a [SceneValueAxis],
    opposition: Option<&'a str>,
    gap: Option<&'a SceneGap>,
    scene_question: Option<&'a str>,
    inaction_cost: Option<&'a str>,
    risk: Option<i32>,
    dilemma: Option<&'a SceneDilemma>,
}

fn scene_identity(scene: &StoredPlannedScene) -> SceneIdentity<'_> {
    SceneIdentity {
        summary: &scene.summary,
        purpose: &scene.purpose,
        beat_structure: &scene.beat_structure,
        beats: &scene.beats,
        turning_beat: scene.turning_beat,
        character_ids: &scene.character_ids,
        location_id: scene.location_id.as_deref(),
        content_rating: scene.content_rating.as_ref(),
        primary_value_axis: scene.primary_value_axis.as_ref(),
        secondary_value_axes: &scene.secondary_value_axes,
        opposition: scene.opposition.as_deref(),
        gap: scene.gap.as_ref(),
        scene_question: scene.scene_question.as_deref(),
        inaction_cost: scene.inaction_cost.as_deref(),
        risk: scene.risk,
        dilemma: scene.dilemma.as_ref(),
    }
}

/// The binding identity a Key Fact / promise payoff resolves to. Summary and
/// purpose are deliberately excluded: rewriting prose must never look like the
/// binding moved to a different Scene.
#[derive(PartialEq)]
struct SceneBinding<'a> {
    primary_value_axis: Option<&'a SceneValueAxis>,
    secondary_value_axes: &'a [SceneValueAxis],
    opposition: Option<&'a str>,
    gap: Option<&'a SceneGap>,
    scene_question: Option<&'a str>,
    inaction_cost: Option<&'a str>,
    risk: Option<i32>,
    dilemma: Option<&'a SceneDilemma>,
    beats: &'a [spindle_core::beats::PlannedBeat],
    turning_beat: Option<u32>,
    character_ids: &'a [String],
    location_id: Option<&'a str>,
}

fn scene_binding(scene: &StoredPlannedScene) -> SceneBinding<'_> {
    SceneBinding {
        primary_value_axis: scene.primary_value_axis.as_ref(),
        secondary_value_axes: &scene.secondary_value_axes,
        opposition: scene.opposition.as_deref(),
        gap: scene.gap.as_ref(),
        scene_question: scene.scene_question.as_deref(),
        inaction_cost: scene.inaction_cost.as_deref(),
        risk: scene.risk,
        dilemma: scene.dilemma.as_ref(),
        beats: &scene.beats,
        turning_beat: scene.turning_beat,
        character_ids: &scene.character_ids,
        location_id: scene.location_id.as_deref(),
    }
}

fn ordered_scenes(scenes: &[StoredPlannedScene]) -> Vec<&StoredPlannedScene> {
    let mut ordered: Vec<&StoredPlannedScene> = scenes.iter().collect();
    ordered.sort_by_key(|scene| scene.scene_order);
    ordered
}

fn scene_at(plan: &ChapterPlan, scene_order: i32) -> Option<&StoredPlannedScene> {
    plan.scenes
        .iter()
        .find(|scene| scene.scene_order == scene_order)
}

/// The order of the first planned-Scene position that differs between the prior
/// and post-apply arrays, using the plan-shape identity above. `None` means the
/// arrays are positionally identical.
fn first_changed_order(prior: &ChapterPlan, post: &ChapterPlan) -> Option<i32> {
    let before = ordered_scenes(&prior.scenes);
    let after = ordered_scenes(&post.scenes);
    let shared = before.len().min(after.len());
    for index in 0..shared {
        if scene_identity(before[index]) != scene_identity(after[index]) {
            return Some(
                after
                    .get(index)
                    .map_or(after.len() as i32 + 1, |s| s.scene_order),
            );
        }
    }
    if before.len() != after.len() {
        return Some(
            after
                .get(shared)
                .map_or(after.len() as i32 + 1, |s| s.scene_order),
        );
    }
    None
}

fn closing_signature(
    scenes: &[StoredPlannedScene],
) -> Vec<(i32, Vec<(String, spindle_core::charge::Charge)>)> {
    ordered_scenes(scenes)
        .into_iter()
        .map(|scene| {
            let mut axes = Vec::new();
            if let Some(primary) = scene.primary_value_axis.as_ref() {
                axes.push((primary.axis.trim().to_string(), primary.closing));
            }
            for axis in &scene.secondary_value_axes {
                axes.push((axis.axis.trim().to_string(), axis.closing));
            }
            (scene.scene_order, axes)
        })
        .collect()
}

/// Whether the applied amendment changed any planned Scene's closing Charge in
/// the target chapter, and the first Scene order where it did.
fn closing_charge_change(prior: &ChapterPlan, post: &ChapterPlan) -> (bool, Option<i32>) {
    let before = closing_signature(&prior.scenes);
    let after = closing_signature(&post.scenes);
    let shared = before.len().min(after.len());
    for index in 0..shared {
        if before[index] != after[index] {
            return (true, Some(after.get(index).map_or(0, |(order, _)| *order)));
        }
    }
    if before.len() != after.len() {
        // A shrunken chapter has no post-apply Scene at the first missing index;
        // anchor past the tail instead of at 0 (which would sweep earlier Scenes).
        return (
            true,
            after
                .get(shared)
                .map_or(Some(after.len() as i32 + 1), |(order, _)| Some(*order)),
        );
    }
    (false, None)
}

fn opening_signature(scenes: &[StoredPlannedScene]) -> BTreeMap<i32, Vec<(String, Charge)>> {
    ordered_scenes(scenes)
        .into_iter()
        .map(|scene| {
            let mut axes = Vec::new();
            if let Some(primary) = scene.primary_value_axis.as_ref() {
                axes.push((primary.axis.trim().to_string(), primary.opening));
            }
            for axis in &scene.secondary_value_axes {
                axes.push((axis.axis.trim().to_string(), axis.opening));
            }
            (scene.scene_order, axes)
        })
        .collect()
}

/// The first Scene order whose opening Charge changed on any axis. A
/// `scene_replace` that rewrites only the opening Charge keeps the closing
/// signature identical, but it still breaks the chain at that Scene, so it must
/// trigger the cascade and include the amended Scene itself.
fn opening_charge_change(prior: &ChapterPlan, post: &ChapterPlan) -> Option<i32> {
    let before = opening_signature(&prior.scenes);
    let after = opening_signature(&post.scenes);
    for (order, axes) in &after {
        if before.get(order) != Some(axes) {
            return Some(*order);
        }
    }
    None
}

fn binding_at<'a>(plan: &'a ChapterPlan, scene_order: i32) -> Option<SceneBinding<'a>> {
    scene_at(plan, scene_order).map(scene_binding)
}

/// Detect a change in any accepted Key Fact bound to the target chapter.
///
/// No amendment class edits a Key Fact row, so the expressible effect is the
/// planned Scene the fact is bound to. The comparison uses the Scene binding
/// (never the summary text), so a prose-only rewrite does not fire.
fn key_fact_binding_change(
    prior: &ChapterPlan,
    post: &ChapterPlan,
    key_facts: &[crate::sqlite::records::CanonicalFact],
    book_number: i32,
    chapter_number: i32,
) -> Option<i32> {
    let mut changed: Option<i32> = None;
    for fact in key_facts {
        if !spindle_core::key_fact::is_key_fact_predicate(&fact.predicate)
            || fact.superseded_by.is_some()
        {
            continue;
        }
        let Some(from) = fact.valid_from.as_ref() else {
            continue;
        };
        if from.book_number != book_number
            || from.chapter_number != chapter_number
            || from.scene_order.is_none()
        {
            continue;
        }
        let order = from.scene_order.unwrap_or_default();
        if binding_at(prior, order) != binding_at(post, order) {
            changed = Some(changed.map_or(order, |current: i32| current.min(order)));
        }
    }
    changed
}

/// Detect a change in a Narrative Promise payoff that lands in the target
/// chapter: the planned Scene it resolves to, or the kind of the Beat at its
/// planned payoff Turning Beat position. Again keyed on the binding, so a
/// summary-only edit does not fire.
fn promise_payoff_change(
    prior: &ChapterPlan,
    post: &ChapterPlan,
    promises: &[crate::sqlite::records::NarrativePromise],
    book_number: i32,
    chapter_number: i32,
) -> Option<i32> {
    let mut changed: Option<i32> = None;
    for promise in promises {
        if promise.archived_at.is_some() {
            continue;
        }
        let Some(payoff) = promise.planned_payoff.as_ref() else {
            continue;
        };
        if payoff.book_number != book_number || payoff.chapter_number != chapter_number {
            continue;
        }
        let Some(order) = payoff.scene_order else {
            continue;
        };
        let before = binding_at(prior, order);
        let after = binding_at(post, order);
        let beat_kind = |plan: &ChapterPlan| {
            plan.scenes
                .iter()
                .find(|scene| scene.scene_order == order)
                .and_then(|scene| {
                    promise
                        .planned_payoff_turning_beat
                        .and_then(|position| scene.beats.get(position.saturating_sub(1) as usize))
                        .map(|beat| beat.kind)
                })
        };
        if before != after || beat_kind(prior) != beat_kind(post) {
            changed = Some(changed.map_or(order, |current: i32| current.min(order)));
        }
    }
    changed
}

/// The plan class of a staged reconciliation.
enum Reconciliation {
    SceneReplace {
        payload: serde_json::Value,
        rationale: String,
    },
    PromiseFollowup {
        payload: serde_json::Value,
        rationale: String,
    },
}

impl Reconciliation {
    fn class(&self) -> &'static str {
        match self {
            Reconciliation::SceneReplace { .. } => "scene_replace",
            Reconciliation::PromiseFollowup { .. } => "promise_followup",
        }
    }

    fn payload(&self) -> &serde_json::Value {
        match self {
            Reconciliation::SceneReplace { payload, .. }
            | Reconciliation::PromiseFollowup { payload, .. } => payload,
        }
    }

    fn into_parts(self) -> (&'static str, serde_json::Value, String) {
        match self {
            Reconciliation::SceneReplace { payload, rationale } => {
                ("scene_replace", payload, rationale)
            }
            Reconciliation::PromiseFollowup { payload, rationale } => {
                ("promise_followup", payload, rationale)
            }
        }
    }
}

/// Why no mechanical reconciliation exists for these findings. Stated on the
/// stale mark so the operator is never left guessing why there is no proposal.
fn no_repair_reason(findings: &[CascadeSceneFinding]) -> String {
    let codes: BTreeSet<&str> = findings
        .iter()
        .map(|finding| finding.code.as_str())
        .collect();
    let detail = if codes.iter().any(|code| code.starts_with("turning_beat_")) {
        "no supported amendment class can edit a planned Beat's promise reference, so the \
         Turning Beat failure cannot be repaired mechanically"
    } else if codes.contains("key_fact_secret_reveal_requires_character_subject") {
        "the Key Fact's subject is not a character record and no plan amendment can change the \
         Key Fact row"
    } else if codes.contains("key_fact_secret_reveal_without_promise") {
        "there is no earlier planned, undrafted placement to plant the required promise"
    } else if codes
        .iter()
        .any(|code| code.starts_with("value_chain") || code.starts_with("scene_"))
    {
        "the value-plan failure has no single-Scene Charge repair that keeps a Value Turn"
    } else {
        "the failure has no mechanical plan-amendment repair"
    };
    format!("No reconciliation proposal is possible: {detail}. Manual Chapter replan required.")
}

fn mark_is_promise_only(row: &crate::sqlite::records::StoredScenePlanStale) -> bool {
    !row.finding_codes.is_empty()
        && row.finding_codes.iter().all(|code| {
            code.starts_with("key_fact_secret_reveal_without_promise")
                || code.starts_with("turning_beat_")
        })
}

fn placement_from_stored(
    placement: &crate::sqlite::records::StoredStoryPlacement,
) -> PlanningStoryPlacement {
    PlanningStoryPlacement {
        book_number: placement.book_number,
        chapter_number: placement.chapter_number,
        scene_order: placement.scene_order,
        note: placement.note.clone(),
    }
}

fn planning_ref_key(reference: &PlanningRef) -> String {
    match reference {
        PlanningRef::ProposalRef(key) => key.clone(),
        PlanningRef::ExistingRecordId(id) => id.clone(),
    }
}

fn push_trigger(
    triggers: &mut Vec<PlanAmendmentCascadeTrigger>,
    trigger: PlanAmendmentCascadeTrigger,
) {
    if !triggers.contains(&trigger) {
        triggers.push(trigger);
    }
}

impl SqliteSpindleService {
    /// Run the McKee cascade after one amendment was really applied. Returns the
    /// observable outcome; the caller records a failure as `cascade_error` on the
    /// decision result instead of pretending the apply did not happen.
    pub(super) async fn run_amendment_cascade(
        &self,
        project: &Project,
        amendment: &StoredPlanAmendment,
        effect: AmendmentApplyEffect<'_>,
        branch_id: &str,
        decided_by: &str,
    ) -> Result<PlanAmendmentCascadeOutcome> {
        let mut outcome = PlanAmendmentCascadeOutcome {
            triggers: Vec::new(),
            marked_stale: 0,
            cleared_stale: 0,
            reconciliation_amendment_ids: Vec::new(),
        };
        if !project.mckee_structured {
            return Ok(outcome);
        }

        // ── The same source set the planning gate builds ────────────────────
        // Accepted promise rows + promise events declared by staged/applied
        // planning proposals (the gate's staged-ancestor source kind), normalized
        // through the branch-wide applied receipt map.
        let promise_rows = self
            .repository
            .list_narrative_promises_by_project_and_branch(&amendment.project_id, branch_id)
            .await?;
        let key_facts = self
            .repository
            .list_canonical_facts_by_project_and_branch(&amendment.project_id, branch_id)
            .await?;
        let promise_events: Vec<PlannedPromiseEvent> = self
            .repository
            .planning_promise_events(&amendment.project_id, branch_id)
            .await?;
        let ref_map = self
            .repository
            .applied_planning_ref_map(&amendment.project_id, branch_id)
            .await?;
        let canonical = |raw: &str| ref_map.get(raw).cloned().unwrap_or_else(|| raw.to_string());
        let gates: BTreeMap<String, PromiseGate> = merge_promise_gates(
            promise_rows
                .iter()
                .filter(|promise| promise.archived_at.is_none())
                .map(|promise| {
                    (
                        canonical(&promise.id),
                        placement_from_stored(&promise.planted_at),
                        promise.planned_payoff.as_ref().map(placement_from_stored),
                    )
                }),
            promise_events,
            |reference| canonical(&planning_ref_key(reference)),
        );

        let plans = self
            .repository
            .list_chapter_plans_by_project_and_branch(&amendment.project_id, branch_id)
            .await?;
        let mut all_scenes: Vec<(i32, i32, PlannedScene)> = plans
            .iter()
            .flat_map(|plan| {
                plan.scenes
                    .iter()
                    .cloned()
                    .map(move |scene| (plan.book_number, plan.chapter_number, scene.into_core()))
            })
            .collect();
        all_scenes.sort_by_key(|(book, chapter, scene)| (*book, *chapter, scene.scene_order));

        let drafted_rows = self
            .repository
            .list_scenes_by_project_and_branch(&amendment.project_id, branch_id)
            .await?;
        let drafted_scenes: BTreeSet<(i32, i32, i32)> = drafted_rows
            .iter()
            .map(|scene| (scene.book_number, scene.chapter_number, scene.scene_order))
            .collect();
        let drafted_chapters: BTreeSet<(i32, i32)> = drafted_rows
            .iter()
            .map(|scene| (scene.book_number, scene.chapter_number))
            .collect();

        // ── Trigger detection and scope ─────────────────────────────────────
        let repaired_rows = self
            .repository
            .list_scene_plan_stale_for_reconciliation(
                &amendment.project_id,
                branch_id,
                &amendment.id,
                Some("stale"),
            )
            .await?;
        let repaired: BTreeSet<(i32, i32, i32)> = repaired_rows
            .iter()
            .map(|row| (row.book_number, row.chapter_number, row.scene_order))
            .collect();

        let promise_followup = amendment.amendment_class == "promise_followup";
        let mut positions: Vec<(i32, i32, i32)> = Vec::new();

        if promise_followup {
            let placement = effect
                .payload
                .get("planted_at_placement")
                .context("promise_followup missing planted_at_placement")?;
            let book = placement
                .get("book_number")
                .and_then(|value| value.as_i64())
                .context("planted_at_placement missing book_number")? as i32;
            let chapter = placement
                .get("chapter_number")
                .and_then(|value| value.as_i64())
                .context("planted_at_placement missing chapter_number")?
                as i32;
            let order = placement
                .get("scene_order")
                .and_then(|value| value.as_i64())
                .map(|value| value as i32)
                .unwrap_or(0);
            positions.push((book, chapter, order));
            push_trigger(
                &mut outcome.triggers,
                PlanAmendmentCascadeTrigger::PromiseFollowup,
            );
            // The created promise is already an accepted row, so the shared
            // source assembly above includes it. Record the read for the
            // contract: the cascade is triggered by what this apply executed.
            let _ = effect.created_promise_id;
            if !repaired.is_empty() {
                push_trigger(
                    &mut outcome.triggers,
                    PlanAmendmentCascadeTrigger::ReconciliationRepair,
                );
                positions.extend(repaired.iter().copied());
            }
        } else {
            let prior_state = effect
                .prior_state
                .context("chapter-targeting amendment applied without prior_state")?;
            let prior_plan: ChapterPlan =
                serde_json::from_str(prior_state).context("parsing amendment prior_state")?;
            let target_chapter = amendment
                .target_chapter
                .context("chapter-targeting amendment missing target_chapter")?;
            let post_plan = self
                .current_chapter_plan(&amendment.project_id, amendment.book_number, target_chapter)
                .await?;

            if let Some(order) = first_changed_order(&prior_plan, &post_plan) {
                positions.push((amendment.book_number, target_chapter, order));
            }
            let (closing_changed, closing_order) = closing_charge_change(&prior_plan, &post_plan);
            if closing_changed {
                push_trigger(
                    &mut outcome.triggers,
                    PlanAmendmentCascadeTrigger::ClosingCharge,
                );
            }
            if let Some(order) = closing_order {
                positions.push((amendment.book_number, target_chapter, order));
            }
            if let Some(order) = opening_charge_change(&prior_plan, &post_plan) {
                push_trigger(
                    &mut outcome.triggers,
                    PlanAmendmentCascadeTrigger::OpeningCharge,
                );
                positions.push((amendment.book_number, target_chapter, order));
            }
            if let Some(order) = key_fact_binding_change(
                &prior_plan,
                &post_plan,
                &key_facts,
                amendment.book_number,
                target_chapter,
            ) {
                push_trigger(&mut outcome.triggers, PlanAmendmentCascadeTrigger::KeyFact);
                positions.push((amendment.book_number, target_chapter, order));
            }
            if let Some(order) = promise_payoff_change(
                &prior_plan,
                &post_plan,
                &promise_rows,
                amendment.book_number,
                target_chapter,
            ) {
                push_trigger(
                    &mut outcome.triggers,
                    PlanAmendmentCascadeTrigger::PromisePayoff,
                );
                positions.push((amendment.book_number, target_chapter, order));
            }
            if !repaired.is_empty() {
                push_trigger(
                    &mut outcome.triggers,
                    PlanAmendmentCascadeTrigger::ReconciliationRepair,
                );
                positions.extend(repaired.iter().copied());
            }
            // A positional difference alone (a prose-only summary edit, or a
            // purpose edit) is not a trigger: only a value-chain/binding change
            // re-validates downstream Scenes. Never let the diff itself mark.
            if outcome.triggers.is_empty() {
                return Ok(outcome);
            }
        }

        let Some((anchor_book, anchor_chapter, from_order)) = positions
            .iter()
            .min_by_key(|(book, chapter, order)| {
                crate::format::story_index(*book, *chapter, *order)
            })
            .copied()
        else {
            return Ok(outcome);
        };
        let scope = CascadeScope {
            anchor_book,
            anchor_chapter,
            from_order,
            repaired,
        };

        // ── Re-validate the accepted plans with the shared rules ────────────
        let mut findings: Vec<CascadeSceneFinding> = cascade_value_findings(&all_scenes);
        findings.extend(cascade_promise_findings(
            &all_scenes,
            &gates,
            &key_facts,
            |reference| canonical(&planning_ref_key(reference)),
        ));

        let mut by_scene: BTreeMap<(i32, i32, i32), Vec<CascadeSceneFinding>> = BTreeMap::new();
        for finding in findings {
            let coordinates = (
                finding.book_number,
                finding.chapter_number,
                finding.scene_order,
            );
            if !scope.contains(coordinates.0, coordinates.1, coordinates.2)
                || drafted_scenes.contains(&coordinates)
            {
                continue;
            }
            by_scene.entry(coordinates).or_default().push(finding);
        }

        let active_rows = self
            .repository
            .list_scene_plan_stale(&amendment.project_id, branch_id, Some("stale"))
            .await?;
        let active: BTreeMap<(i32, i32, i32), &crate::sqlite::records::StoredScenePlanStale> =
            active_rows
                .iter()
                .map(|row| ((row.book_number, row.chapter_number, row.scene_order), row))
                .collect();
        let scene_lookup: BTreeMap<(i32, i32, i32), &StoredPlannedScene> = plans
            .iter()
            .flat_map(|plan| {
                plan.scenes.iter().map(move |scene| {
                    (
                        (plan.book_number, plan.chapter_number, scene.scene_order),
                        scene,
                    )
                })
            })
            .collect();
        // Reuse a still-staged repair only while its payload still repairs what
        // the chain requires. Read once (async) so the decision loop is sync.
        let mut existing_staged: BTreeMap<(i32, i32, i32), StoredPlanAmendment> = BTreeMap::new();
        for (coordinates, row) in &active {
            let Some(id) = row.reconciliation_amendment_id.as_deref() else {
                continue;
            };
            if let Some(candidate) = self
                .repository
                .read_plan_amendment_public(id)
                .await?
                .filter(|candidate| candidate.status == "staged")
            {
                existing_staged.insert(*coordinates, candidate);
            }
        }
        let earliest_undrafted: Option<i32> = plans
            .iter()
            .filter(|plan| {
                plan.book_number == amendment.book_number
                    && !drafted_chapters.contains(&(plan.book_number, plan.chapter_number))
            })
            .map(|plan| plan.chapter_number)
            .min();

        let mut failing: BTreeSet<(i32, i32, i32)> = BTreeSet::new();
        let mut clears: Vec<ScenePlanStaleClearWrite> = Vec::new();
        let mut mark_writes: Vec<ScenePlanStaleMarkWrite> = Vec::new();

        let build_mark_write = |coordinates: (i32, i32, i32),
                                scene_findings: &[CascadeSceneFinding]|
         -> Result<ScenePlanStaleMarkWrite> {
            let mut codes: Vec<String> = scene_findings
                .iter()
                .map(|finding| finding.code.clone())
                .collect();
            codes.sort();
            codes.dedup();
            let mut reason = crate::format::truncate_at_chars(
                &scene_findings
                    .iter()
                    .map(|finding| finding.message.clone())
                    .collect::<Vec<_>>()
                    .join("; "),
                500,
            );
            let desired = self.build_reconciliation(
                amendment,
                coordinates,
                scene_findings,
                scene_lookup.get(&coordinates).copied(),
                &all_scenes,
                earliest_undrafted,
                &drafted_scenes,
            );
            if desired.is_none() {
                let note = no_repair_reason(scene_findings);
                reason = crate::format::truncate_at_chars(&format!("{reason} {note}"), 500);
            }
            let existing = existing_staged.get(&coordinates);
            let (reconciliation_id, stage, supersede) = match (existing, desired) {
                (Some(existing), Some(desired))
                    if existing.amendment_class == desired.class()
                        && existing.payload == *desired.payload() =>
                {
                    (Some(existing.id.clone()), None, Vec::new())
                }
                (existing, desired) => {
                    let supersede = existing
                        .map(|existing| {
                            vec![(
                                existing.id.clone(),
                                format!(
                                    "superseded by the amendment cascade after {}: the required \
                                     repair changed",
                                    amendment.id
                                ),
                            )]
                        })
                        .unwrap_or_default();
                    match desired {
                        Some(desired) => {
                            let (class, payload, rationale) = desired.into_parts();
                            let stage = StagePlanAmendmentParams {
                                project_id: amendment.project_id.clone(),
                                branch_id: branch_id.to_string(),
                                source_chapter: amendment.source_chapter,
                                book_number: coordinates.0,
                                authoring_run_id: None,
                                amendment_class: class.to_string(),
                                target_chapter: (class != "promise_followup")
                                    .then_some(coordinates.1),
                                payload,
                                rationale,
                                confidence: "high".to_string(),
                            };
                            (None, Some(stage), supersede)
                        }
                        None => (None, None, supersede),
                    }
                }
            };
            Ok(ScenePlanStaleMarkWrite {
                mark: MarkScenePlanStaleParams {
                    project_id: amendment.project_id.clone(),
                    branch_id: branch_id.to_string(),
                    book_number: coordinates.0,
                    chapter_number: coordinates.1,
                    scene_order: coordinates.2,
                    reason,
                    finding_codes: codes,
                    triggering_amendment_id: amendment.id.clone(),
                    reconciliation_amendment_id: reconciliation_id,
                },
                stage,
                supersede,
            })
        };

        if promise_followup {
            // Only the Scenes this `promise_followup` was staged to repair are
            // re-validated (plus promise-only marks in scope that now pass). A
            // promise can never create a new failure, so no new Scene is marked.
            for coordinates in &scope.repaired {
                if drafted_scenes.contains(coordinates) {
                    continue;
                }
                match by_scene.get(coordinates) {
                    None => {
                        if let Some(row) = active.get(coordinates) {
                            clears.push(ScenePlanStaleClearWrite {
                                book_number: coordinates.0,
                                chapter_number: coordinates.1,
                                scene_order: coordinates.2,
                                supersede_amendment_id: row.reconciliation_amendment_id.clone(),
                                reason: format!(
                                    "cleared by the amendment cascade: amendment {} repaired the \
                                     Scene's promise/Key-Fact failure",
                                    amendment.id
                                ),
                            });
                        }
                    }
                    Some(scene_findings) => {
                        let write = build_mark_write(*coordinates, scene_findings)?;
                        failing.insert(*coordinates);
                        mark_writes.push(write);
                    }
                }
            }
            for (coordinates, row) in &active {
                if scope.repaired.contains(coordinates)
                    || drafted_scenes.contains(coordinates)
                    || !scope.contains(coordinates.0, coordinates.1, coordinates.2)
                    || !mark_is_promise_only(row)
                    || by_scene.contains_key(coordinates)
                {
                    continue;
                }
                clears.push(ScenePlanStaleClearWrite {
                    book_number: coordinates.0,
                    chapter_number: coordinates.1,
                    scene_order: coordinates.2,
                    supersede_amendment_id: row.reconciliation_amendment_id.clone(),
                    reason: format!(
                        "cleared by the amendment cascade: the promise amending {} is now \
                         planted earlier and pays off here",
                        amendment.id
                    ),
                });
            }
        } else {
            for (coordinates, scene_findings) in &by_scene {
                failing.insert(*coordinates);
                mark_writes.push(build_mark_write(*coordinates, scene_findings)?);
            }
            for (coordinates, row) in &active {
                if failing.contains(coordinates)
                    || drafted_scenes.contains(coordinates)
                    || !scope.contains(coordinates.0, coordinates.1, coordinates.2)
                {
                    continue;
                }
                clears.push(ScenePlanStaleClearWrite {
                    book_number: coordinates.0,
                    chapter_number: coordinates.1,
                    scene_order: coordinates.2,
                    supersede_amendment_id: row.reconciliation_amendment_id.clone(),
                    reason: format!(
                        "cleared by the amendment cascade: the Scene passes the re-validated \
                         value-chain and promise rules after amendment {}",
                        amendment.id
                    ),
                });
            }
        }

        if mark_writes.is_empty() && clears.is_empty() {
            return Ok(outcome);
        }
        let marked = mark_writes.len();
        let cleared = clears.len();
        let staged_ids = self
            .repository
            .apply_scene_plan_stale_batch(ScenePlanStaleBatch {
                project_id: amendment.project_id.clone(),
                branch_id: branch_id.to_string(),
                decided_by: decided_by.to_string(),
                marks: mark_writes,
                clears,
            })
            .await?;
        outcome.marked_stale = marked;
        outcome.cleared_stale = cleared;
        outcome.reconciliation_amendment_ids = staged_ids;
        Ok(outcome)
    }

    /// Build the reconciliation proposal for one failing Scene.
    ///
    /// A missing setup promise (`key_fact_secret_reveal_without_promise`) gets a
    /// `promise_followup` planted at the latest earlier valid planned placement,
    /// paying off at the failing Scene. A broken value-chain link gets a
    /// `scene_replace` whose value-plan payload repairs the opening Charge. Any
    /// other failure has no mechanical repair and returns `None` (the caller puts
    /// the reason on the mark).
    #[allow(clippy::too_many_arguments)]
    fn build_reconciliation(
        &self,
        amendment: &StoredPlanAmendment,
        coordinates: (i32, i32, i32),
        findings: &[CascadeSceneFinding],
        scene: Option<&StoredPlannedScene>,
        all_scenes: &[(i32, i32, PlannedScene)],
        earliest_undrafted: Option<i32>,
        drafted_scenes: &BTreeSet<(i32, i32, i32)>,
    ) -> Option<Reconciliation> {
        let missing_promise = findings
            .iter()
            .any(|finding| finding.code == "key_fact_secret_reveal_without_promise");
        if missing_promise {
            let failing_index =
                crate::format::story_index(coordinates.0, coordinates.1, coordinates.2);
            let plant = all_scenes
                .iter()
                .filter(|(book, chapter, scene)| {
                    let index = crate::format::story_index(*book, *chapter, scene.scene_order);
                    index < failing_index
                        && earliest_undrafted.is_none_or(|earliest| *chapter >= earliest)
                        && !drafted_scenes.contains(&(*book, *chapter, scene.scene_order))
                })
                .max_by_key(|(book, chapter, scene)| {
                    crate::format::story_index(*book, *chapter, scene.scene_order)
                });
            if let Some((plant_book, plant_chapter, plant_scene)) = plant {
                let turning_beat = scene.and_then(|scene| {
                    let position = scene.turning_beat?;
                    let beat = scene.beats.get(position.saturating_sub(1) as usize)?;
                    (beat.kind == BeatKind::Turn).then_some(position as i64)
                });
                let mut payload = serde_json::json!({
                    "promise_type": "mystery",
                    "description": format!(
                        "Cascade reconciliation: the promise the secret reveal at book {} chapter \
                         {} scene {} pays off",
                        coordinates.0, coordinates.1, coordinates.2
                    ),
                    "planted_at_placement": {
                        "book_number": plant_book,
                        "chapter_number": plant_chapter,
                        "scene_order": plant_scene.scene_order,
                    },
                    "planned_payoff": {
                        "book_number": coordinates.0,
                        "chapter_number": coordinates.1,
                        "scene_order": coordinates.2,
                    },
                });
                if let Some(position) = turning_beat {
                    payload["planned_payoff_turning_beat"] = serde_json::json!(position);
                }
                let rationale = crate::format::truncate_at_chars(
                    &format!(
                        "Cascade after amendment {}: the secret reveal at book {} chapter {} \
                         scene {} has no earlier-planted promise; plant it at book {} chapter {} \
                         scene {} with its payoff here.",
                        amendment.id,
                        coordinates.0,
                        coordinates.1,
                        coordinates.2,
                        plant_book,
                        plant_chapter,
                        plant_scene.scene_order
                    ),
                    500,
                );
                return Some(Reconciliation::PromiseFollowup { payload, rationale });
            }
            // No earlier valid placement exists for the promise. If the same
            // Scene also has an expressible Charge repair, stage that instead of
            // giving up on both failures; otherwise fall through to the honest
            // no-proposal reason.
        }

        let scene = scene?;
        let repairs: BTreeMap<String, spindle_core::charge::Charge> = findings
            .iter()
            .filter_map(|finding| Some((finding.axis.clone()?, finding.expected_opening?)))
            .collect();
        if repairs.is_empty() {
            return None;
        }

        let mut primary = scene.primary_value_axis.clone();
        let mut repaired_any = false;
        if let Some(axis) = primary.as_mut()
            && let Some(expected) = repairs.get(axis.axis.trim())
        {
            axis.opening = *expected;
            repaired_any = true;
        }
        let mut secondary = scene.secondary_value_axes.clone();
        for axis in secondary.iter_mut() {
            if let Some(expected) = repairs.get(axis.axis.trim()) {
                axis.opening = *expected;
                repaired_any = true;
            }
        }
        if !repaired_any {
            return None;
        }
        // A repaired primary axis that opens and closes at the same Charge is a
        // non-event, so the mechanical repair cannot be staged honestly.
        if primary
            .as_ref()
            .is_some_and(|axis| axis.opening == axis.closing)
        {
            return None;
        }

        let expected = primary
            .as_ref()
            .map(|axis| format!("{} / {}", axis.opening.sign(), axis.opening.depth()))
            .or_else(|| {
                secondary
                    .first()
                    .map(|axis| format!("{} / {}", axis.opening.sign(), axis.opening.depth()))
            })
            .unwrap_or_default();
        let rationale = crate::format::truncate_at_chars(
            &format!(
                "Cascade after amendment {}: book {} chapter {} scene {} no longer continues its \
                 Value Axis — set the opening Charge to {expected} to follow the amended closing \
                 Charge.",
                amendment.id, coordinates.0, coordinates.1, coordinates.2
            ),
            500,
        );
        let mut payload = serde_json::json!({
            "scene_order": scene.scene_order,
            "summary": scene.summary.clone(),
            "purpose": scene.purpose.clone(),
            "character_ids": scene.character_ids.clone(),
        });
        let object = payload.as_object_mut()?;
        if let Some(axis) = primary {
            object.insert(
                "primary_value_axis".to_string(),
                serde_json::to_value(axis).ok()?,
            );
        }
        if !secondary.is_empty() {
            object.insert(
                "secondary_value_axes".to_string(),
                serde_json::to_value(secondary).ok()?,
            );
        }
        Some(Reconciliation::SceneReplace { payload, rationale })
    }
}
