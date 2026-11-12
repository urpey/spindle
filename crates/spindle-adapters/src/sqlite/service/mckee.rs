//! McKee realized reports: deterministic verify rules and system-written
//! Narrative Promise status.
//!
//! The writer reports what a written Scene actually did
//! ([`spindle_core::realized`]). This module owns the two server-side
//! consequences of that report, both deterministic and model-free:
//!
//! 1. **Scene verify** (`realized_value_turn`, `realized_key_fact`,
//!    `realized_promise_event`, `character_effect`) joins the existing
//!    scene-verify set. Each finding is attributed to the Scene and uses the
//!    existing consistency-finding shape. The comparison tolerances are
//!    documented on [`spindle_core::realized`]: exact Charge match, Gap by
//!    presence, Turning Beat by 1-based position when reported.
//! 2. **Promise status** is advanced by the system after a confirmed realized
//!    event, with the Scene recorded as the source scene, instead of the agent
//!    calling `update_promise_status` by hand. The report is pre-validated
//!    before the prose write so an unusable event fails the save before it
//!    writes anything, and applied afterwards so a Scene that never committed
//!    never advances a promise.
//!
//! Everything is gated on `project.mckee_structured`: a non-McKee project gets
//! no findings and no status write, so its behaviour is unchanged.

use super::*;
use crate::sqlite::json_records::StoredPlannedScene;
use crate::sqlite::records::{CanonicalFact, CharacterArc, NarrativePromise, Scene};
use spindle_core::models::{ArcMilestoneKind, ConsistencyIssue, StoryPlacement};
use spindle_core::realized::{
    CHARACTER_EFFECT_CHECK, CharacterEffect, REALIZED_KEY_FACT_CHECK, REALIZED_PROMISE_EVENT_CHECK,
    REALIZED_VALUE_TURN_CHECK, RealizedPromiseEventKind, RealizedReport, format_value_turn,
};
use std::collections::BTreeMap;

/// Which of the four deterministic McKee realized-report checks a caller asked
/// for. `check_consistency` resolves this from its requested `checks` list, so a
/// caller that asks for one rule never receives another rule's findings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RealizedChecks {
    pub value_turn: bool,
    pub key_fact: bool,
    pub promise_event: bool,
    pub character_effect: bool,
}

impl RealizedChecks {
    /// Resolve the requested check names.
    pub(super) fn from_requested(requested: &std::collections::BTreeSet<String>) -> Self {
        Self {
            value_turn: should_run_check(requested, REALIZED_VALUE_TURN_CHECK),
            key_fact: should_run_check(requested, REALIZED_KEY_FACT_CHECK),
            promise_event: should_run_check(requested, REALIZED_PROMISE_EVENT_CHECK),
            character_effect: should_run_check(requested, CHARACTER_EFFECT_CHECK),
        }
    }

    pub(super) fn any(self) -> bool {
        self.value_turn || self.key_fact || self.promise_event || self.character_effect
    }
}

/// Whether a persisted placement names exactly this Scene.
///
/// A chapter-level placement (`scene_order: None`) is a different thing from a
/// Scene placement: it is accepted here only when the caller opts in, because a
/// chapter-wide promise plant or arc milestone is not pinned to one Scene and
/// must not be demanded in every Scene of the chapter. Key Facts keep their own
/// chapter-wide-due rule in [`key_fact_is_due_in_scene`], matching the drafting
/// packet's constraint.
fn placement_names_scene(
    placement: &crate::sqlite::records::StoredStoryPlacement,
    book_number: i32,
    chapter_number: i32,
    scene_order: i32,
) -> bool {
    placement.book_number == book_number
        && placement.chapter_number == chapter_number
        && placement.scene_order == Some(scene_order)
}

/// Whether a planned Key Fact is due in a Scene.
///
/// This is the one due rule: the reserved `key_fact.<kind>:<author key>`
/// predicate namespace marks a Key Fact, a Character Design secret is
/// author-only and never due, a superseded fact is history, the placement must
/// be this Scene (or chapter-wide, `scene_order: None`) and the fact must not
/// have expired before it. `prepare_scene`'s drafting constraints and the
/// deterministic verify check both call this, so the packet and the check cannot
/// drift.
pub(super) fn key_fact_is_due_in_scene(
    fact: &CanonicalFact,
    book_number: i32,
    chapter_number: i32,
    scene_order: i32,
) -> bool {
    if !spindle_core::key_fact::is_key_fact_predicate(&fact.predicate)
        || crate::format::is_planning_only_secret_fact(fact)
        || fact.superseded_by.is_some()
    {
        return false;
    }
    let Some(from) = fact.valid_from.as_ref() else {
        return false;
    };
    if from.book_number != book_number
        || from.chapter_number != chapter_number
        || !(from.scene_order.is_none() || from.scene_order == Some(scene_order))
    {
        return false;
    }
    fact.valid_until.as_ref().is_none_or(|until| {
        crate::format::story_index_from_placement(until)
            >= crate::format::story_index(book_number, chapter_number, scene_order)
    })
}

/// Shorten a description for a bounded finding message.
fn preview_text(text: &str) -> String {
    let trimmed = text.trim();
    const MAX: usize = 80;
    if trimmed.chars().count() <= MAX {
        return trimmed.to_string();
    }
    let head: String = trimmed.chars().take(MAX).collect();
    format!("{head}…")
}

fn realized_issue(
    check_type: &str,
    scene_id: &str,
    message: String,
    mut entity_ids: Vec<String>,
) -> ConsistencyIssue {
    entity_ids.insert(0, scene_id.to_string());
    entity_ids.dedup();
    ConsistencyIssue {
        severity: "error".to_string(),
        check_type: check_type.to_string(),
        message,
        entity_ids,
        suggested_action: Some(match check_type {
            REALIZED_VALUE_TURN_CHECK => {
                "revise the scene so it realizes the planned Value Turn, or re-plan the scene \
                 (plan amendment) before drafting"
                    .to_string()
            }
            REALIZED_KEY_FACT_CHECK => {
                "dramatise the planned Key Fact in the scene and list its canonical fact id in \
                 the realized report, or re-plan the Key Fact's placement"
                    .to_string()
            }
            REALIZED_PROMISE_EVENT_CHECK => {
                "report the promise event the scene actually delivers, or re-plan the promise \
                 placement"
                    .to_string()
            }
            _ => "report 'change' only where a planned change milestone lands, or re-plan the \
                 character arc milestone"
                .to_string(),
        }),
    }
}

/// Every deterministic McKee realized-report finding for one written Scene.
///
/// Pure over already-read rows: the caller has the branch-scoped Scene, its
/// planned Scene (when the chapter has a plan), the branch's Key Facts and
/// promises, the project's character arcs and a character-name map. Non-McKee
/// projects never call this.
#[allow(clippy::too_many_arguments)]
pub(super) fn realized_findings_for_scene(
    scene: &Scene,
    planned_scene: Option<&StoredPlannedScene>,
    checks: RealizedChecks,
    key_facts: &[CanonicalFact],
    promises: &[NarrativePromise],
    arcs: &[CharacterArc],
    character_names: &BTreeMap<String, String>,
) -> Vec<ConsistencyIssue> {
    let mut issues = Vec::new();
    let book = scene.book_number;
    let chapter = scene.chapter_number;
    let order = scene.scene_order;
    let scene_ref = format!("book {book}, chapter {chapter} scene {order}");
    let report = scene.realized_report.as_ref();

    if checks.value_turn {
        realized_value_turn_findings(&mut issues, scene, planned_scene, report, &scene_ref);
    }
    if checks.key_fact {
        realized_key_fact_findings(&mut issues, scene, report, key_facts, &scene_ref);
    }
    if checks.promise_event {
        realized_promise_event_findings(&mut issues, scene, report, promises, &scene_ref);
    }
    if checks.character_effect {
        realized_character_effect_findings(
            &mut issues,
            scene,
            report,
            arcs,
            character_names,
            &scene_ref,
        );
    }
    issues
}

fn realized_value_turn_findings(
    issues: &mut Vec<ConsistencyIssue>,
    scene: &Scene,
    planned_scene: Option<&StoredPlannedScene>,
    report: Option<&RealizedReport>,
    scene_ref: &str,
) {
    let planned_axes = planned_scene
        .map(|planned| {
            planned
                .primary_value_axis
                .iter()
                .chain(planned.secondary_value_axes.iter())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if planned_axes.is_empty() {
        return;
    }
    let Some(report) = report else {
        issues.push(realized_issue(
            REALIZED_VALUE_TURN_CHECK,
            &scene.id,
            format!(
                "{scene_ref} has a planned Value Turn but no realized report; report the realized \
                 opening and closing Charge for every planned Value Axis"
            ),
            Vec::new(),
        ));
        return;
    };
    for axis in &planned_axes {
        let name = axis.axis.trim();
        let Some(realized) = report.value_turn(name) else {
            issues.push(realized_issue(
                REALIZED_VALUE_TURN_CHECK,
                &scene.id,
                format!(
                    "{scene_ref} plans the Value Axis '{name}' but the realized report does not \
                     report a realized Value Turn on it"
                ),
                Vec::new(),
            ));
            continue;
        };
        if realized.opening != axis.opening || realized.closing != axis.closing {
            issues.push(realized_issue(
                REALIZED_VALUE_TURN_CHECK,
                &scene.id,
                format!(
                    "{scene_ref} realizes '{name}' as {} but the plan is {} (Charge comparison is \
                     exact)",
                    format_value_turn(realized.opening, realized.closing),
                    format_value_turn(axis.opening, axis.closing)
                ),
                Vec::new(),
            ));
        }
    }
    // The Gap and the Turning Beat are scene-level plan fields, so they are
    // compared once per Scene rather than once per axis.
    let primary_axis = planned_scene.and_then(|planned| planned.primary_value_axis.as_ref());
    let primary_realized = primary_axis.and_then(|axis| report.value_turn(&axis.axis));
    if let Some(planned_gap) = planned_scene.and_then(|planned| planned.gap.as_ref())
        && !report.value_turns.iter().any(|turn| turn.gap.is_some())
    {
        issues.push(realized_issue(
            REALIZED_VALUE_TURN_CHECK,
            &scene.id,
            format!(
                "{scene_ref} plans a Gap (expected '{}', actual '{}') but the realized report \
                 reports none; Gap comparison is by presence, not wording",
                preview_text(&planned_gap.expected),
                preview_text(&planned_gap.actual)
            ),
            Vec::new(),
        ));
    }
    if let Some(planned_beat) = planned_scene.and_then(|planned| planned.turning_beat)
        && let Some(realized) = primary_realized
        && let Some(realized_beat) = realized.turning_beat
        && planned_beat != realized_beat
    {
        issues.push(realized_issue(
            REALIZED_VALUE_TURN_CHECK,
            &scene.id,
            format!(
                "{scene_ref} realizes the Turning Beat at beat {realized_beat} but the plan \
                 places it at beat {planned_beat}"
            ),
            Vec::new(),
        ));
    }
    for realized in &report.value_turns {
        let name = realized.axis.trim();
        if !planned_axes.iter().any(|axis| axis.axis.trim() == name) {
            issues.push(realized_issue(
                REALIZED_VALUE_TURN_CHECK,
                &scene.id,
                format!(
                    "{scene_ref} reports a realized Value Turn on '{name}', which is not one of \
                     the scene's planned Value Axes"
                ),
                Vec::new(),
            ));
        }
    }
}

fn realized_key_fact_findings(
    issues: &mut Vec<ConsistencyIssue>,
    scene: &Scene,
    report: Option<&RealizedReport>,
    key_facts: &[CanonicalFact],
    scene_ref: &str,
) {
    for fact in key_facts.iter().filter(|fact| {
        key_fact_is_due_in_scene(
            fact,
            scene.book_number,
            scene.chapter_number,
            scene.scene_order,
        )
    }) {
        if report.is_some_and(|report| report.reports_key_fact(&fact.id)) {
            continue;
        }
        issues.push(realized_issue(
            REALIZED_KEY_FACT_CHECK,
            &scene.id,
            format!(
                "{scene_ref} has the planned Key Fact '{}' due but the realized report does not \
                 list its id in key_fact_ids",
                spindle_core::key_fact::key_fact_display_predicate(&fact.predicate)
            ),
            vec![fact.id.clone()],
        ));
    }
}

fn realized_promise_event_findings(
    issues: &mut Vec<ConsistencyIssue>,
    scene: &Scene,
    report: Option<&RealizedReport>,
    promises: &[NarrativePromise],
    scene_ref: &str,
) {
    for promise in promises
        .iter()
        .filter(|promise| promise.archived_at.is_none())
    {
        let mut expected: Vec<(RealizedPromiseEventKind, &str)> = Vec::new();
        if placement_names_scene(
            &promise.planted_at,
            scene.book_number,
            scene.chapter_number,
            scene.scene_order,
        ) {
            expected.push((RealizedPromiseEventKind::Setup, "plant"));
        }
        if promise.planned_payoff.as_ref().is_some_and(|payoff| {
            placement_names_scene(
                payoff,
                scene.book_number,
                scene.chapter_number,
                scene.scene_order,
            )
        }) {
            expected.push((RealizedPromiseEventKind::Payoff, "pay off"));
        }
        for (kind, verb) in expected {
            if report.is_some_and(|report| report.reports_promise_event(&promise.id, kind)) {
                continue;
            }
            issues.push(realized_issue(
                REALIZED_PROMISE_EVENT_CHECK,
                &scene.id,
                format!(
                    "{scene_ref} is planned to {verb} the narrative promise '{}' but the realized \
                     report does not report a '{kind}' event for it",
                    preview_text(&promise.description)
                ),
                vec![promise.id.clone()],
            ));
        }
    }
}

fn realized_character_effect_findings(
    issues: &mut Vec<ConsistencyIssue>,
    scene: &Scene,
    report: Option<&RealizedReport>,
    arcs: &[CharacterArc],
    character_names: &BTreeMap<String, String>,
    scene_ref: &str,
) {
    // Planned change milestones at this Scene, grouped per character so one
    // finding names every planned change the Scene owes that character.
    let mut planned_change: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for arc in arcs.iter().filter(|arc| arc.archived_at.is_none()) {
        for milestone in &arc.milestones {
            if milestone.kind != Some(ArcMilestoneKind::Change) {
                continue;
            }
            let Some(placement) = milestone.placement.as_ref() else {
                continue;
            };
            if !placement_names_scene(
                placement,
                scene.book_number,
                scene.chapter_number,
                scene.scene_order,
            ) {
                continue;
            }
            planned_change
                .entry(arc.character_id.clone())
                .or_default()
                .push(milestone.label.clone());
        }
    }
    let name_of = |character_id: &str| {
        character_names
            .get(character_id)
            .cloned()
            .unwrap_or_else(|| character_id.to_string())
    };
    for (character_id, labels) in &planned_change {
        let name = name_of(character_id);
        match report.and_then(|report| report.character_effect(character_id)) {
            Some(CharacterEffect::Change) => {}
            Some(other) => issues.push(realized_issue(
                CHARACTER_EFFECT_CHECK,
                &scene.id,
                format!(
                    "{scene_ref} plans the change milestone(s) {} for {name}, but the realized \
                     report records '{other}'; a planned change must be reported as 'change' and \
                     is never downgraded to a reveal",
                    labels
                        .iter()
                        .map(|label| format!("'{label}'"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                vec![character_id.clone()],
            )),
            None => issues.push(realized_issue(
                CHARACTER_EFFECT_CHECK,
                &scene.id,
                format!(
                    "{scene_ref} plans the change milestone(s) {} for {name} but the realized \
                     report records no Character Effect for them",
                    labels
                        .iter()
                        .map(|label| format!("'{label}'"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                vec![character_id.clone()],
            )),
        }
    }
    for effect in report
        .map(|report| report.character_effects.as_slice())
        .unwrap_or_default()
    {
        if effect.effect != CharacterEffect::Change {
            continue;
        }
        if planned_change.contains_key(&effect.character_id) {
            continue;
        }
        issues.push(realized_issue(
            CHARACTER_EFFECT_CHECK,
            &scene.id,
            format!(
                "{scene_ref} reports the Character Effect 'change' for {}, but no planned arc \
                 milestone of kind change is placed at this scene; 'change' is accepted only at a \
                 planned change milestone",
                name_of(&effect.character_id)
            ),
            vec![effect.character_id.clone()],
        ));
    }
}

impl SqliteSpindleService {
    /// Pre-validate a realized report's promise events before the prose write.
    ///
    /// A report that names an unknown promise, a promise on another branch, or a
    /// Scene that precedes the promise's planting placement is rejected before
    /// anything is written, so the save stays all-or-nothing. Non-McKee projects
    /// are untouched: their report never advances a promise.
    pub(super) async fn validate_realized_promise_events(
        &self,
        project_id: &str,
        branch_id: &str,
        placement: StoryPlacement,
        report: Option<&RealizedReport>,
    ) -> Result<()> {
        let Some(report) = report else {
            return Ok(());
        };
        if report.promise_events.is_empty() {
            return Ok(());
        }
        let project = self.repository.get_project(project_id).await?;
        if !project.mckee_structured {
            return Ok(());
        }
        let lineage = self
            .repository
            .resolve_branch_lineage(project_id, branch_id)
            .await?;
        let scene_order = placement
            .scene_order
            .ok_or_else(|| anyhow::anyhow!("a realized promise event needs a scene placement"))?;
        let scene_index = crate::format::story_index(
            placement.book_number,
            placement.chapter_number,
            scene_order,
        );
        for event in &report.promise_events {
            let promise = self
                .repository
                .get_narrative_promise(&event.narrative_promise_id)
                .await
                .with_context(|| {
                    format!(
                        "realized promise event names unknown narrative promise '{}'",
                        event.narrative_promise_id
                    )
                })?;
            anyhow::ensure!(
                promise.project_id == project_id,
                "realized promise event names narrative promise '{}' from another project",
                event.narrative_promise_id
            );
            anyhow::ensure!(
                lineage.iter().any(|branch| branch == &promise.branch_id),
                "realized promise event names narrative promise '{}' from a branch the scene does \
                 not inherit",
                event.narrative_promise_id
            );
            anyhow::ensure!(
                scene_index >= crate::format::story_index_from_placement(&promise.planted_at),
                "the scene precedes the planting placement of narrative promise '{}'; a promise \
                 event cannot precede its planting",
                event.narrative_promise_id
            );
        }
        Ok(())
    }

    /// Advance Narrative Promise status from a confirmed realized report.
    ///
    /// Runs after the Scene write commits, so a Scene that never committed never
    /// advances a promise. Each event maps to one status
    /// (`spindle_core::realized::RealizedPromiseEventKind::promise_status`) and
    /// records the Scene as the source scene through the existing promise
    /// status-history event. Re-saving the same Scene is idempotent: an event
    /// this Scene already recorded is not appended twice. Non-McKee projects are
    /// untouched and the agent-facing `update_promise_status` keeps working.
    pub(super) async fn apply_realized_promise_events(
        &self,
        project_id: &str,
        scene_id: &str,
        report: Option<&RealizedReport>,
    ) -> Result<()> {
        let Some(report) = report else {
            return Ok(());
        };
        if report.promise_events.is_empty() {
            return Ok(());
        }
        let project = self.repository.get_project(project_id).await?;
        if !project.mckee_structured {
            return Ok(());
        }
        for event in &report.promise_events {
            let status = event.event.promise_status();
            let promise = self
                .repository
                .get_narrative_promise(&event.narrative_promise_id)
                .await
                .with_context(|| {
                    format!(
                        "realized promise event names unknown narrative promise '{}'",
                        event.narrative_promise_id
                    )
                })?;
            if promise.status_history.iter().any(|recorded| {
                recorded.source_scene_id.as_deref() == Some(scene_id) && recorded.status == status
            }) {
                continue;
            }
            let mut note = format!(
                "system: realized '{}' event reported with scene {scene_id}",
                event.event
            );
            if let Some(extra) = event
                .note
                .as_deref()
                .map(str::trim)
                .filter(|note| !note.is_empty())
            {
                note.push_str(" — ");
                note.push_str(extra);
            }
            self.update_promise_status(spindle_core::models::UpdatePromiseStatusInput {
                narrative_promise_id: event.narrative_promise_id.clone(),
                status: status.to_string(),
                note: Some(note),
                at: None,
                source_scene_id: Some(scene_id.to_string()),
                replaces_event_id: None,
            })
            .await?;
        }
        Ok(())
    }
}
