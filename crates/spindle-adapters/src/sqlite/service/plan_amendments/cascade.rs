//! McKee amendment cascade over unwritten Scenes (issue #34, parent spec #21).
//!
//! Accepting a plan amendment that changes a planned Scene's closing Charge (or
//! the plan binding a Key Fact / Narrative Promise payoff relies on) can leave
//! every later planned, undrafted Scene in a broken value chain. This module
//! detects that from the amendment's recorded `prior_state` against the plan the
//! apply actually wrote (never from the amendment class alone), re-runs the SAME
//! value-chain and promise/Key-Fact rules the planning gate uses, marks failing
//! Scenes stale (`scene_plan_stale`, V0062) and stages one `scene_replace`
//! reconciliation proposal per repairable Scene through the normal ADR 0003
//! lifecycle — never a parallel amendment table.
//!
//! Scope is explicitly later planned, undrafted Scenes: a drafted Scene has a
//! persisted row on the active branch and is never touched, marked or repairable
//! here (ADR 0003 D3). A no-op amendment (the chain and promise structure stay
//! valid) stages nothing and marks nothing.
//!
//! **Honest limitations** (documented in `docs/planning-engine-design.md`):
//! no current amendment class can edit a Key Fact or an existing Narrative
//! Promise, so a direct Key-Fact/payoff change is not expressible. The detection
//! hook still exists and fires for the plan-shape effect a class *can* produce:
//! dropping/reordering/renaming the planned Scene a Key Fact or payoff is bound
//! to changes that binding. A Scene that fails without a mechanical Charge
//! repair is marked stale with no proposal (`reconciliation_amendment_id` NULL)
//! and needs a manual Chapter replan.

use super::*;
use crate::sqlite::records::{ChapterPlan, Project, StoredPlanAmendment, StoredPlannedScene};
use crate::sqlite::repository::{
    CascadeSceneFinding, MarkScenePlanStaleParams, StagePlanAmendmentParams,
    cascade_promise_findings, cascade_value_findings,
};
use spindle_core::models::PlanAmendmentCascadeOutcome;
use std::collections::{BTreeMap, BTreeSet};

const TRIGGER_CLOSING_CHARGE: &str = "closing_charge";
const TRIGGER_KEY_FACT: &str = "key_fact";
const TRIGGER_PROMISE_PAYOFF: &str = "promise_payoff";
const TRIGGER_RECONCILIATION_REPAIR: &str = "reconciliation_repair";

/// The Scene coordinates a cascade is allowed to touch.
struct CascadeScope {
    target_book: i32,
    target_chapter: i32,
    /// Scenes after this order in the target chapter are in scope (exclusive).
    after_order: i32,
    /// Scenes a just-applied reconciliation repaired are re-validated even
    /// though they are not "after" themselves.
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
        if (book_number, chapter_number) > (self.target_book, self.target_chapter) {
            return true;
        }
        book_number == self.target_book
            && chapter_number == self.target_chapter
            && scene_order > self.after_order
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
        prior_state: &str,
        branch_id: &str,
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
        let Some(target_chapter) = amendment.target_chapter else {
            // `promise_followup` targets a placement, not a chapter row: it
            // cannot change a planned Scene's value chain, and it captures no
            // prior state to compare (see the module limitations).
            return Ok(outcome);
        };

        let prior_plan: ChapterPlan =
            serde_json::from_str(prior_state).context("parsing amendment prior_state")?;
        let post_plan = self
            .current_chapter_plan(&amendment.project_id, amendment.book_number, target_chapter)
            .await?;

        // ── Trigger detection (prior_state vs post-apply plan/rows) ──────────
        let (closing_changed, closing_order) = closing_charge_change(&prior_plan, &post_plan);
        if closing_changed {
            outcome.triggers.push(TRIGGER_CLOSING_CHARGE.to_string());
        }
        let mut change_orders: Vec<i32> = closing_order.into_iter().collect();

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
        if !repaired.is_empty() {
            outcome
                .triggers
                .push(TRIGGER_RECONCILIATION_REPAIR.to_string());
            change_orders.extend(repaired.iter().map(|(_, _, order)| *order));
        }

        let key_facts = self
            .repository
            .list_canonical_facts_by_project_and_branch(&amendment.project_id, branch_id)
            .await?;
        let promises = self
            .repository
            .list_narrative_promises_by_project_and_branch(&amendment.project_id, branch_id)
            .await?;

        let key_fact_changed = key_fact_binding_change(
            &prior_plan,
            &post_plan,
            &key_facts,
            amendment.book_number,
            target_chapter,
        );
        if key_fact_changed.is_some() {
            outcome.triggers.push(TRIGGER_KEY_FACT.to_string());
        }
        change_orders.extend(key_fact_changed);

        let promise_changed = promise_payoff_change(
            &prior_plan,
            &post_plan,
            &promises,
            amendment.book_number,
            target_chapter,
        );
        if promise_changed.is_some() {
            outcome.triggers.push(TRIGGER_PROMISE_PAYOFF.to_string());
        }
        change_orders.extend(promise_changed);

        if outcome.triggers.is_empty() {
            return Ok(outcome);
        }
        let after_order = change_orders.into_iter().min().unwrap_or(0);
        let scope = CascadeScope {
            target_book: amendment.book_number,
            target_chapter,
            after_order,
            repaired,
        };

        // ── Re-validate the whole branch's accepted plans, report in scope ───
        let drafted: BTreeSet<(i32, i32)> = self
            .repository
            .list_scenes_by_project_and_branch(&amendment.project_id, branch_id)
            .await?
            .into_iter()
            .map(|scene| (scene.book_number, scene.chapter_number))
            .collect();
        let plans = self
            .repository
            .list_chapter_plans_by_project_and_branch(&amendment.project_id, branch_id)
            .await?;
        let all_scenes: Vec<(i32, i32, spindle_core::models::PlannedScene)> = plans
            .iter()
            .flat_map(|plan| {
                plan.scenes
                    .iter()
                    .cloned()
                    .map(move |scene| (plan.book_number, plan.chapter_number, scene.into_core()))
            })
            .collect();

        let mut findings: Vec<CascadeSceneFinding> = cascade_value_findings(&all_scenes);
        findings.extend(cascade_promise_findings(&all_scenes, &promises, &key_facts));

        let mut by_scene: BTreeMap<(i32, i32, i32), Vec<CascadeSceneFinding>> = BTreeMap::new();
        for finding in findings {
            let coordinates = (
                finding.book_number,
                finding.chapter_number,
                finding.scene_order,
            );
            if !scope.contains(coordinates.0, coordinates.1, coordinates.2)
                || drafted.contains(&(coordinates.0, coordinates.1))
            {
                continue;
            }
            by_scene.entry(coordinates).or_default().push(finding);
        }

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

        // ── Mark failing Scenes stale + stage one reconciliation each ────────
        let active_rows = self
            .repository
            .list_scene_plan_stale(&amendment.project_id, branch_id, Some("stale"))
            .await?;
        let active: BTreeMap<(i32, i32, i32), &crate::sqlite::records::StoredScenePlanStale> =
            active_rows
                .iter()
                .map(|row| ((row.book_number, row.chapter_number, row.scene_order), row))
                .collect();

        let mut failing: BTreeSet<(i32, i32, i32)> = BTreeSet::new();
        for (coordinates, scene_findings) in &by_scene {
            failing.insert(*coordinates);
            let codes: Vec<String> = scene_findings.iter().map(|f| f.code.clone()).collect();
            let reason = crate::format::truncate_at_chars(
                &scene_findings
                    .iter()
                    .map(|finding| finding.message.clone())
                    .collect::<Vec<_>>()
                    .join("; "),
                500,
            );

            // A repeat cascade reuses this Scene's still-staged reconciliation;
            // only a decided/superseded/missing one is restaged, so a Scene never
            // accumulates duplicate proposals.
            let existing_staged = match active.get(coordinates) {
                Some(row) => match row.reconciliation_amendment_id.as_deref() {
                    Some(id) => self
                        .repository
                        .read_plan_amendment_public(id)
                        .await?
                        .filter(|candidate| candidate.status == "staged"),
                    None => None,
                },
                None => None,
            };
            let desired = self.build_reconciliation_proposal(
                amendment,
                *coordinates,
                scene_findings,
                scene_lookup.get(coordinates).copied(),
            )?;
            // Reuse a still-staged proposal only when its payload still repairs
            // exactly what the chain now requires; otherwise supersede it and
            // stage the corrected repair (never two staged repairs for a Scene).
            let reconciliation_id = match (existing_staged, desired) {
                (Some(existing), Some((payload, _))) if existing.payload == payload => {
                    Some(existing.id)
                }
                (Some(existing), desired) => {
                    self.repository
                        .supersede_plan_amendment(&existing.id)
                        .await?;
                    match desired {
                        Some((payload, rationale)) => Some(
                            self.stage_reconciliation_payload(
                                amendment,
                                branch_id,
                                *coordinates,
                                payload,
                                rationale,
                            )
                            .await?,
                        ),
                        None => None,
                    }
                }
                (None, Some((payload, rationale))) => Some(
                    self.stage_reconciliation_payload(
                        amendment,
                        branch_id,
                        *coordinates,
                        payload,
                        rationale,
                    )
                    .await?,
                ),
                (None, None) => None,
            };
            if let Some(id) = reconciliation_id.as_ref()
                && !outcome.reconciliation_amendment_ids.contains(id)
            {
                outcome.reconciliation_amendment_ids.push(id.clone());
            }
            self.repository
                .mark_scene_plan_stale(MarkScenePlanStaleParams {
                    project_id: amendment.project_id.clone(),
                    branch_id: branch_id.to_string(),
                    book_number: coordinates.0,
                    chapter_number: coordinates.1,
                    scene_order: coordinates.2,
                    reason,
                    finding_codes: codes,
                    triggering_amendment_id: amendment.id.clone(),
                    reconciliation_amendment_id: reconciliation_id,
                })
                .await?;
            outcome.marked_stale += 1;
        }

        // ── Clear marks whose Scene now passes (incl. the repaired Scene) ────
        for (coordinates, row) in &active {
            if failing.contains(coordinates)
                || !scope.contains(coordinates.0, coordinates.1, coordinates.2)
            {
                continue;
            }
            if self
                .repository
                .clear_scene_plan_stale(
                    &amendment.project_id,
                    branch_id,
                    coordinates.0,
                    coordinates.1,
                    coordinates.2,
                )
                .await?
            {
                outcome.cleared_stale += 1;
                // A still-staged repair for a Scene that now passes is moot.
                if let Some(id) = row.reconciliation_amendment_id.as_deref() {
                    self.repository.supersede_plan_amendment(id).await?;
                }
            }
        }

        Ok(outcome)
    }

    /// Build the `scene_replace` reconciliation payload for one failing Scene.
    ///
    /// The payload repairs the value plan only (opening Charges follow the new
    /// previous closing Charge) and repeats the current summary/purpose/cast so
    /// the class's existing required-key contract still holds. When the expected
    /// opening Charge would equal the Scene's closing Charge, no single-Scene
    /// repair can keep it a Value Turn, so no proposal is built and the stale
    /// mark stays proposal-less (manual replan) instead of staging a no-op.
    fn build_reconciliation_proposal(
        &self,
        amendment: &StoredPlanAmendment,
        coordinates: (i32, i32, i32),
        findings: &[CascadeSceneFinding],
        scene: Option<&StoredPlannedScene>,
    ) -> Result<Option<(serde_json::Value, String)>> {
        let Some(scene) = scene else {
            return Ok(None);
        };
        let repairs: BTreeMap<String, spindle_core::charge::Charge> = findings
            .iter()
            .filter_map(|finding| Some((finding.axis.clone()?, finding.expected_opening?)))
            .collect();
        if repairs.is_empty() {
            return Ok(None);
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
            return Ok(None);
        }
        // A repaired primary axis that opens and closes at the same Charge is a
        // non-event, so the mechanical repair cannot be staged honestly.
        if primary
            .as_ref()
            .is_some_and(|axis| axis.opening == axis.closing)
        {
            return Ok(None);
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
        let object = payload
            .as_object_mut()
            .context("reconciliation payload must be an object")?;
        if let Some(axis) = primary {
            object.insert(
                "primary_value_axis".to_string(),
                serde_json::to_value(axis)?,
            );
        }
        if !secondary.is_empty() {
            object.insert(
                "secondary_value_axes".to_string(),
                serde_json::to_value(secondary)?,
            );
        }
        Ok(Some((payload, rationale)))
    }

    /// Stage one already-built reconciliation payload through the normal ADR
    /// 0003 staging path.
    async fn stage_reconciliation_payload(
        &self,
        amendment: &StoredPlanAmendment,
        branch_id: &str,
        coordinates: (i32, i32, i32),
        payload: serde_json::Value,
        rationale: String,
    ) -> Result<String> {
        let staged = self
            .repository
            .stage_plan_amendment(StagePlanAmendmentParams {
                project_id: amendment.project_id.clone(),
                branch_id: branch_id.to_string(),
                source_chapter: amendment.source_chapter,
                book_number: coordinates.0,
                authoring_run_id: None,
                amendment_class: "scene_replace".to_string(),
                target_chapter: Some(coordinates.1),
                payload,
                rationale,
                confidence: "high".to_string(),
            })
            .await?;
        Ok(staged.id)
    }
}

/// Every Scene's closing Charge per axis, in story order. A change anywhere in
/// this vector means the chapter's outgoing value chain changed.
fn closing_signature(
    scenes: &[StoredPlannedScene],
) -> Vec<(i32, Vec<(String, spindle_core::charge::Charge)>)> {
    let mut ordered: Vec<&StoredPlannedScene> = scenes.iter().collect();
    ordered.sort_by_key(|scene| scene.scene_order);
    ordered
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
        return (true, after.get(shared).map(|(order, _)| *order));
    }
    (false, None)
}

/// The planned-Scene identity a Scene placement resolves to, used to notice that
/// a Key Fact / promise payoff binding now names a different (or no) planned
/// Scene.
fn bound_scene_summary(plan: &ChapterPlan, scene_order: i32) -> Option<&str> {
    plan.scenes
        .iter()
        .find(|scene| scene.scene_order == scene_order)
        .map(|scene| scene.summary.as_str())
}

/// Detect a change in any accepted Key Fact bound to the target chapter.
///
/// No current amendment class edits a Key Fact row, so the only expressible
/// change is the planned Scene the fact is bound to: dropping/replacing/
/// renumbering that Scene changes the binding without touching the fact. The
/// hook is deliberately keyed on the binding, not on a fabricated row snapshot:
/// a future class that edits a Key Fact must extend this comparison with the
/// before/after fact rows it actually writes.
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
        let before = bound_scene_summary(prior, order);
        let after = bound_scene_summary(post, order);
        let binding_changed = before != after;
        if binding_changed {
            changed = Some(changed.map_or(order, |current: i32| current.min(order)));
        }
    }
    changed
}

/// Detect a change in a Narrative Promise payoff that lands in the target
/// chapter: the planned Scene it resolves to, or the kind of the Beat at its
/// planned payoff Turning Beat position. No current class edits a promise row,
/// so the expressible part is again the plan-shape binding; `target_act` is a
/// promise-row field and therefore not changed by any current class.
fn promise_payoff_change(
    prior: &ChapterPlan,
    post: &ChapterPlan,
    promises: &[crate::sqlite::records::NarrativePromise],
    book_number: i32,
    chapter_number: i32,
) -> Option<i32> {
    let mut changed: Option<i32> = None;
    for promise in promises {
        let Some(payoff) = promise.planned_payoff.as_ref() else {
            continue;
        };
        if payoff.book_number != book_number || payoff.chapter_number != chapter_number {
            continue;
        }
        let Some(order) = payoff.scene_order else {
            continue;
        };
        let before = bound_scene_summary(prior, order);
        let after = bound_scene_summary(post, order);
        let before_beat = prior
            .scenes
            .iter()
            .find(|scene| scene.scene_order == order)
            .and_then(|scene| {
                promise
                    .planned_payoff_turning_beat
                    .and_then(|position| scene.beats.get(position.saturating_sub(1) as usize))
                    .map(|beat| beat.kind)
            });
        let after_beat = post
            .scenes
            .iter()
            .find(|scene| scene.scene_order == order)
            .and_then(|scene| {
                promise
                    .planned_payoff_turning_beat
                    .and_then(|position| scene.beats.get(position.saturating_sub(1) as usize))
                    .map(|beat| beat.kind)
            });
        if before != after || before_beat != after_beat {
            changed = Some(changed.map_or(order, |current: i32| current.min(order)));
        }
    }
    changed
}
