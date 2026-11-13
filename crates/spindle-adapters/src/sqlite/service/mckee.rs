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
use spindle_core::drafting::critic::{MCKEE_SCENE_CRITIC_RUBRIC_VERSION, McKeeCriticVerdict};
use spindle_core::models::{ArcMilestoneKind, ConsistencyIssue};
use spindle_core::realized::{
    CharacterEffect, RealizedCheck, RealizedPromiseEventKind, RealizedReport, format_value_turn,
};
use std::collections::BTreeMap;

/// Which deterministic McKee checks a caller asked for. `check_consistency`
/// resolves this from its requested `checks` list, so a caller that asks for one
/// rule never receives another rule's findings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RealizedChecks {
    pub value_turn: bool,
    pub key_fact: bool,
    pub promise_event: bool,
    pub character_effect: bool,
    pub critic_disagreement: bool,
}

impl RealizedChecks {
    /// Resolve the requested check names.
    pub(super) fn from_requested(requested: &std::collections::BTreeSet<String>) -> Self {
        Self {
            value_turn: should_run_check(requested, RealizedCheck::ValueTurn.check_type()),
            key_fact: should_run_check(requested, RealizedCheck::KeyFact.check_type()),
            promise_event: should_run_check(requested, RealizedCheck::PromiseEvent.check_type()),
            character_effect: should_run_check(
                requested,
                RealizedCheck::CharacterEffect.check_type(),
            ),
            critic_disagreement: should_run_check(
                requested,
                RealizedCheck::CriticReportDisagreement.check_type(),
            ),
        }
    }

    pub(super) fn any(self) -> bool {
        self.value_turn
            || self.key_fact
            || self.promise_event
            || self.character_effect
            || self.critic_disagreement
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
    check: RealizedCheck,
    scene_id: &str,
    message: String,
    mut entity_ids: Vec<String>,
) -> ConsistencyIssue {
    entity_ids.insert(0, scene_id.to_string());
    entity_ids.dedup();
    ConsistencyIssue {
        severity: "error".to_string(),
        check_type: check.check_type().to_string(),
        message,
        entity_ids,
        suggested_action: Some(check.suggested_action().to_string()),
    }
}

/// Everything the deterministic realized-report checks need for one Scene.
///
/// Bundled so the call site cannot silently transpose the four collections and
/// so adding a chapter-level rule input does not grow another positional
/// argument. `chapter_scenes` is every Scene of this Scene's chapter on the
/// checked branch (including `scene` itself), because a planned chapter-level
/// `change` milestone is satisfied by a `change` effect anywhere in the chapter.
pub(super) struct RealizedFindingsInputs<'a> {
    pub scene: &'a Scene,
    pub planned_scene: Option<&'a StoredPlannedScene>,
    pub checks: RealizedChecks,
    pub key_facts: &'a [CanonicalFact],
    pub promises: &'a [NarrativePromise],
    pub arcs: &'a [CharacterArc],
    pub character_names: &'a BTreeMap<String, String>,
    pub chapter_scenes: &'a [Scene],
    /// The critic's persisted independent verdict for THIS prose version, when
    /// one exists. `None` is an unavailable pass, never a mismatch.
    pub critic_verdict: Option<&'a McKeeCriticVerdict>,
}

/// Every deterministic McKee realized-report finding for one written Scene.
///
/// Pure over already-read rows: the caller has the branch-scoped Scene, its
/// planned Scene (when the chapter has a plan), the branch's Key Facts and
/// promises, the project's character arcs, a character-name map and the
/// chapter's Scenes (for the chapter-level change rule). Non-McKee projects
/// never call this.
pub(super) fn realized_findings_for_scene(
    inputs: RealizedFindingsInputs<'_>,
) -> Vec<ConsistencyIssue> {
    let mut issues = Vec::new();
    let scene = inputs.scene;
    let book = scene.book_number;
    let chapter = scene.chapter_number;
    let order = scene.scene_order;
    let scene_ref = format!("book {book}, chapter {chapter} scene {order}");
    let report = scene.realized_report.as_ref();

    if inputs.checks.value_turn {
        realized_value_turn_findings(&mut issues, scene, inputs.planned_scene, report, &scene_ref);
    }
    if inputs.checks.key_fact {
        realized_key_fact_findings(&mut issues, scene, report, inputs.key_facts, &scene_ref);
    }
    if inputs.checks.promise_event {
        realized_promise_event_findings(&mut issues, scene, report, inputs.promises, &scene_ref);
    }
    if inputs.checks.character_effect {
        realized_character_effect_findings(
            &mut issues,
            scene,
            report,
            inputs.arcs,
            inputs.character_names,
            inputs.chapter_scenes,
            &scene_ref,
        );
    }
    if inputs.checks.critic_disagreement {
        critic_report_disagreement_findings(
            &mut issues,
            scene,
            inputs.planned_scene,
            report,
            inputs.promises,
            inputs.critic_verdict,
            &scene_ref,
        );
    }
    issues
}

/// Every promise event a Scene is planned to deliver, as
/// `(event, description)`.
///
/// Shared by the deterministic promise-event rule and the critic-disagreement
/// rule so the two cannot disagree about what "planned" means.
fn planned_promise_events<'a>(
    scene: &Scene,
    promises: &'a [NarrativePromise],
) -> Vec<(RealizedPromiseEventKind, &'a NarrativePromise)> {
    let mut expected = Vec::new();
    for promise in promises
        .iter()
        .filter(|promise| promise.archived_at.is_none())
    {
        if placement_names_scene(
            &promise.planted_at,
            scene.book_number,
            scene.chapter_number,
            scene.scene_order,
        ) {
            expected.push((RealizedPromiseEventKind::Setup, promise));
        }
        if promise.planned_payoff.as_ref().is_some_and(|payoff| {
            placement_names_scene(
                payoff,
                scene.book_number,
                scene.chapter_number,
                scene.scene_order,
            )
        }) {
            expected.push((RealizedPromiseEventKind::Payoff, promise));
        }
    }
    expected
}

/// The independent-judgement rule (ADR 0005): compare the critic's persisted
/// verdict with the writer's realized report.
///
/// A finding fires only when both sides make a definite claim and those claims
/// disagree. A missing verdict, a stale/foreign rubric, a report that is absent
/// or that does not report a planned axis, and a Scene with nothing planned are
/// all **unavailable passes** — no finding is invented.
fn critic_report_disagreement_findings(
    issues: &mut Vec<ConsistencyIssue>,
    scene: &Scene,
    planned_scene: Option<&StoredPlannedScene>,
    report: Option<&RealizedReport>,
    promises: &[NarrativePromise],
    verdict: Option<&McKeeCriticVerdict>,
    scene_ref: &str,
) {
    let Some(verdict) = verdict else {
        return;
    };
    // The verdict must be the McKee rubric and must satisfy its own closed
    // contract; anything else is unavailable, not a disagreement.
    if verdict.rubric_version != MCKEE_SCENE_CRITIC_RUBRIC_VERSION || verdict.validate().is_err() {
        return;
    }
    let Some(report) = report else {
        return;
    };
    let note_suffix = |notes: Option<&str>| {
        notes
            .map(str::trim)
            .filter(|notes| !notes.is_empty())
            .map(|notes| format!("; critic notes: '{}'", preview_text(notes)))
            .unwrap_or_default()
    };

    let planned_axes = planned_scene
        .map(|planned| {
            planned
                .primary_value_axis
                .iter()
                .chain(planned.secondary_value_axes.iter())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !planned_axes.is_empty() {
        let mut complete = true;
        let mut writer_matches = true;
        for axis in &planned_axes {
            match report.value_turn(axis.axis.trim()) {
                Some(realized) => {
                    if realized.opening != axis.opening || realized.closing != axis.closing {
                        writer_matches = false;
                    }
                }
                None => complete = false,
            }
        }
        // Only compare when the report makes a definite claim on every planned
        // axis; a missing axis is the `realized_value_turn` rule's finding.
        if complete && writer_matches != verdict.value_turn.is_agreement() {
            let planned = planned_axes
                .iter()
                .map(|axis| {
                    format!(
                        "'{}' {}",
                        axis.axis.trim(),
                        format_value_turn(axis.opening, axis.closing)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let message = if writer_matches {
                format!(
                    "{scene_ref}: the critic judged that the prose does not realize the planned \
                     Value Turn ({planned}) but the writer's realized report matches the plan. \
                     The critic judges independently of the writer's report (ADR 0005); revise \
                     the scene so the prose turns the planned value, or re-plan it{}",
                    note_suffix(verdict.value_turn.notes.as_deref())
                )
            } else {
                format!(
                    "{scene_ref}: the critic judged that the prose realizes the planned Value \
                     Turn ({planned}) but the writer's realized report records a different turn. \
                     The critic judges independently of the writer's report (ADR 0005); revise \
                     the scene or re-plan it{}",
                    note_suffix(verdict.value_turn.notes.as_deref())
                )
            };
            issues.push(realized_issue(
                RealizedCheck::CriticReportDisagreement,
                &scene.id,
                message,
                Vec::new(),
            ));
        }
    }

    let expected = planned_promise_events(scene, promises);
    if !expected.is_empty() {
        let writer_reports_all = expected
            .iter()
            .all(|(kind, promise)| report.reports_promise_event(&promise.id, *kind));
        if writer_reports_all != verdict.promise_events.is_agreement() {
            let planned = expected
                .iter()
                .map(|(kind, promise)| {
                    format!("'{kind}' on '{}'", preview_text(&promise.description))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let message = if writer_reports_all {
                format!(
                    "{scene_ref}: the critic judged that the prose does not deliver the planned \
                     Narrative Promise event(s) ({planned}) but the writer's realized report \
                     lists every one of them. The critic judges independently of the writer's \
                     report (ADR 0005); revise the scene or re-plan the promise placement{}",
                    note_suffix(verdict.promise_events.notes.as_deref())
                )
            } else {
                format!(
                    "{scene_ref}: the critic judged that the prose delivers the planned Narrative \
                     Promise event(s) ({planned}) but the writer's realized report does not list \
                     every one of them. The critic judges independently of the writer's report \
                     (ADR 0005); revise the scene or re-plan the promise placement{}",
                    note_suffix(verdict.promise_events.notes.as_deref())
                )
            };
            issues.push(realized_issue(
                RealizedCheck::CriticReportDisagreement,
                &scene.id,
                message,
                Vec::new(),
            ));
        }
    }
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
            RealizedCheck::ValueTurn,
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
                RealizedCheck::ValueTurn,
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
                RealizedCheck::ValueTurn,
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
            RealizedCheck::ValueTurn,
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
            RealizedCheck::ValueTurn,
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
                RealizedCheck::ValueTurn,
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
            RealizedCheck::KeyFact,
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
    for (kind, promise) in planned_promise_events(scene, promises) {
        let verb = match kind {
            RealizedPromiseEventKind::Setup => "plant",
            _ => "pay off",
        };
        if report.is_some_and(|report| report.reports_promise_event(&promise.id, kind)) {
            continue;
        }
        issues.push(realized_issue(
            RealizedCheck::PromiseEvent,
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

fn realized_character_effect_findings(
    issues: &mut Vec<ConsistencyIssue>,
    scene: &Scene,
    report: Option<&RealizedReport>,
    arcs: &[CharacterArc],
    character_names: &BTreeMap<String, String>,
    chapter_scenes: &[Scene],
    scene_ref: &str,
) {
    // Planned change milestones are grouped per character so one finding names
    // every planned change the Scene owes that character. A milestone is
    // either pinned to this Scene (`scene_order: Some`) or chapter-level
    // (`scene_order: None`, the placement shape planning accepts).
    let mut planned_at_scene: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut planned_in_chapter: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for arc in arcs.iter().filter(|arc| arc.archived_at.is_none()) {
        for milestone in &arc.milestones {
            if milestone.kind != Some(ArcMilestoneKind::Change) {
                continue;
            }
            let Some(placement) = milestone.placement.as_ref() else {
                continue;
            };
            if placement.book_number != scene.book_number
                || placement.chapter_number != scene.chapter_number
            {
                continue;
            }
            let planned = match placement.scene_order {
                Some(order) if order == scene.scene_order => &mut planned_at_scene,
                Some(_) => continue,
                None => &mut planned_in_chapter,
            };
            planned
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
    let labels_of = |labels: &[String]| {
        labels
            .iter()
            .map(|label| format!("'{label}'"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    // A milestone placed at this Scene must be realized here as `change`; a
    // downgrade to `reveal` (or no effect) is the classic character jump.
    for (character_id, labels) in &planned_at_scene {
        let name = name_of(character_id);
        match report.and_then(|report| report.character_effect(character_id)) {
            Some(CharacterEffect::Change) => {}
            Some(other) => issues.push(realized_issue(
                RealizedCheck::CharacterEffect,
                &scene.id,
                format!(
                    "{scene_ref} plans the change milestone(s) {} for {name}, but the realized \
                     report records '{other}'; a planned change must be reported as 'change' and \
                     is never downgraded to a reveal",
                    labels_of(labels)
                ),
                vec![character_id.clone()],
            )),
            None => issues.push(realized_issue(
                RealizedCheck::CharacterEffect,
                &scene.id,
                format!(
                    "{scene_ref} plans the change milestone(s) {} for {name} but the realized \
                     report records no Character Effect for them",
                    labels_of(labels)
                ),
                vec![character_id.clone()],
            )),
        }
    }
    // A chapter-level milestone is satisfied by a `change` effect in ANY Scene
    // of the chapter, so this Scene is not demanded to carry one. The one
    // reportable mismatch is a Scene that claims only `reveal` while no Scene
    // in the chapter reports the planned `change`: attribute that finding to
    // the Scene making the reveal claim.
    for (character_id, labels) in &planned_in_chapter {
        if planned_at_scene.contains_key(character_id) {
            continue;
        }
        if report.and_then(|report| report.character_effect(character_id))
            != Some(CharacterEffect::Reveal)
        {
            continue;
        }
        let change_elsewhere = chapter_scenes.iter().any(|sibling| {
            sibling.id != scene.id
                && sibling
                    .realized_report
                    .as_ref()
                    .and_then(|report| report.character_effect(character_id))
                    == Some(CharacterEffect::Change)
        });
        if change_elsewhere {
            continue;
        }
        issues.push(realized_issue(
            RealizedCheck::CharacterEffect,
            &scene.id,
            format!(
                "{scene_ref} plans the chapter-level change milestone(s) {} for {}, but the \
                 realized report records only 'reveal' and no scene in this chapter reports \
                 'change'; a planned change is never downgraded to a reveal",
                labels_of(labels),
                name_of(character_id)
            ),
            vec![character_id.clone()],
        ));
    }
    // A reported `change` needs a planned change milestone either at this Scene
    // or at chapter level in this chapter.
    for effect in report
        .map(|report| report.character_effects.as_slice())
        .unwrap_or_default()
    {
        if effect.effect != CharacterEffect::Change {
            continue;
        }
        if planned_at_scene.contains_key(&effect.character_id)
            || planned_in_chapter.contains_key(&effect.character_id)
        {
            continue;
        }
        issues.push(realized_issue(
            RealizedCheck::CharacterEffect,
            &scene.id,
            format!(
                "{scene_ref} reports the Character Effect 'change' for {}, but no planned arc \
                 milestone of kind change is placed at this scene or at chapter level in this \
                 chapter; 'change' is accepted only at a planned change milestone",
                name_of(&effect.character_id)
            ),
            vec![effect.character_id.clone()],
        ));
    }
}
