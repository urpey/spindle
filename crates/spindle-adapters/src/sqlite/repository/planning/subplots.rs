//! McKee Subplot structural checks over planned Sequences and Convergence
//! Points.
//!
//! The pass runs at Chapter and Scene acceptance for a McKee-structured project
//! and reads the same merged sources as the other McKee gates:
//!
//! * the merged plot lines a decision would leave behind (accepted
//!   branch-lineage rows, staged-ancestor `create_plot_line` operations and the
//!   current artifact), including each line's `value_axis`, `subplot_job`,
//!   Main Plot flag and Convergence Points;
//! * the merged Sequences (accepted Chapter proposals, staged ancestors, then
//!   the current artifact, refined by `sequence_key`);
//! * the merged Acts (used only for the Act story ranges of the depth
//!   advisory);
//! * the merged planned Scenes and their Value Axes.
//!
//! One rule **blocks**: a Subplot Sequence whose climax lands after every one of
//! that Subplot's Convergence Points. The rest are **advisories**: a Subplot
//! with no Convergence Point, a Subplot Value Axis that goes three Chapters
//! without a turn, a Subplot whose turns do not support its declared job, and a
//! Subplot turning deeper than the Main Plot in the same Act. Advisories never
//! refuse acceptance; a blocking finding stages with the proposal (the code is
//! stage-recordable) and only acceptance is refused.
//!
//! The Act-cause reference is validated here too, because resolving it needs
//! the merged Sequences and plot lines: an Act climax may cite a Subplot
//! Sequence climax as its cause, and the reference must resolve to a Subplot
//! Sequence whose climax is at or before a Convergence Point and at or before
//! the Act's own climax.

use super::values::SceneValueEntry;
use super::*;
use spindle_core::planning::{PlannedAct, PlannedSequence, PlanningRef, PlanningStoryPlacement};
use spindle_core::scene_value::SceneValueAxis;
use spindle_core::subplot::{
    ActStoryRange, DORMANT_CHAPTER_RUN, act_climaxes_strictly_increasing, act_range_for_scene,
    act_story_ranges, climax_within_convergence_points, closing_supports_job, deepest_charge_rank,
    dormant_chapter_runs, job_requires_closing_sign,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// One turn on one Scene's Value Axis, with the finding path suffix it would
/// use. Built once and filtered per Subplot axis.
struct AxisTurn<'a> {
    scene: &'a SceneValueEntry,
    suffix: String,
    axis: &'a SceneValueAxis,
}

/// Subplot structure gate entry point. Records findings and never changes data.
pub(super) fn validate_mckee_subplot_structure(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    scope: &PlanningScope,
    scenes: &[SceneValueEntry],
    sequences: &[PlannedSequence],
    acts: &[PlannedAct],
    findings: &mut Vec<PlanningFinding>,
) -> Result<()> {
    if !project_is_mckee_in_conn(conn, &scope.project_id)? {
        return Ok(());
    }
    let candidates = evaluated_plot_line_candidates_in_conn(conn, input, scope)?;
    if candidates.is_empty() {
        return Ok(());
    }
    let receipts = apply::refs_in_conn(conn, &input.session_id)?;
    let subplots: Vec<&PlotLineStructureCandidate> = candidates
        .iter()
        .filter(|item| !item.is_main_plot)
        .collect();

    validate_sequence_identity(conn, input, scope, &candidates, &receipts, findings)?;
    validate_convergence_points(
        input,
        &subplots,
        &candidates,
        sequences,
        &receipts,
        findings,
    );
    validate_subplot_axes(conn, scope, &candidates, scenes, acts, findings)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Convergence points: the blocking rule and the missing-point advisory
// ---------------------------------------------------------------------------

/// Advisory: a merged Subplot plans no Convergence Point.
///
/// Series-Arc/Book staging calls this over the merged plot-line set so a
/// Subplot authored without a Convergence Point is visible where it is authored,
/// not only in a later Chapter batch. Chapter/Scene staging calls it through
/// [`validate_convergence_points`]; the advisory re-reports on later stagings
/// because a visible advisory is the point.
pub(super) fn validate_subplot_convergence_advisory(
    candidates: &[PlotLineStructureCandidate],
    findings: &mut Vec<PlanningFinding>,
) {
    for subplot in candidates.iter().filter(|item| !item.is_main_plot) {
        if !subplot.convergence_points.is_empty() {
            continue;
        }
        findings.push(planning_finding(
            "subplot_without_convergence_point",
            PlanningFindingSeverity::Advisory,
            subplot.field_path("convergence_points"),
            vec![subplot.related_ref.clone()],
            "at least one Convergence Point where this Subplot feeds the Main Plot",
            "no Convergence Point",
            "Place a Convergence Point at the Scene where this Subplot feeds the Main Plot; without one its Sequence climaxes cannot land at or before a convergence and the line risks staying a parallel story.",
        ));
    }
}

fn validate_convergence_points(
    input: &StagePlanningInput,
    subplots: &[&PlotLineStructureCandidate],
    candidates: &[PlotLineStructureCandidate],
    sequences: &[PlannedSequence],
    receipts: &BTreeMap<String, String>,
    findings: &mut Vec<PlanningFinding>,
) {
    validate_subplot_convergence_advisory(candidates, findings);

    for sequence in sequences {
        let key = sequence.sequence_key.trim();
        if key.is_empty() {
            // The Sequence structural gate already reports the missing key.
            continue;
        }
        let Some(reference) = sequence.plot_line_id.as_ref() else {
            continue;
        };
        let Some(subplot) = subplots
            .iter()
            .copied()
            .find(|candidate| candidate.matches(reference, receipts))
        else {
            // An unresolved plot line reference is the existing reference
            // validation's finding, not this gate's.
            continue;
        };
        if subplot.convergence_points.is_empty() {
            // Covered by the advisory above: without a Convergence Point the
            // blocking rule has nothing to measure against.
            continue;
        }
        let Some(climax) = sequence.climax_scene.as_ref() else {
            // The Sequence structural gate reports the missing climax.
            continue;
        };
        if climax.scene_order.is_none()
            || climax_within_convergence_points(climax, &subplot.convergence_points)
        {
            continue;
        }
        findings.push(planning_finding(
            "subplot_sequence_climax_after_convergence_point",
            PlanningFindingSeverity::Blocking,
            sequence_climax_path(input, key),
            vec![sequence.sequence_key.clone(), subplot.related_ref.clone()],
            format!(
                "a Sequence climax at or before one of this Subplot's Convergence Points ({})",
                describe_placements(&subplot.convergence_points)
            ),
            format!(
                "the Sequence climax at book {} chapter {} scene {} is after every Convergence Point",
                climax.book_number,
                climax.chapter_number,
                climax.scene_order.unwrap_or_default()
            ),
            "Move the Sequence climax to or before a Convergence Point, or add a later Convergence Point: a Subplot Sequence must land where the Subplot feeds the Main Plot.",
        ));
    }
}

/// The path of the Sequence entry a staging author can repair. A Sequence the
/// current artifact declares or refines points into `schedule.sequences`; an
/// inherited Sequence points at the merged-sequence set by key.
fn sequence_climax_path(input: &StagePlanningInput, key: &str) -> String {
    if let Some(index) = input
        .schedule
        .sequences
        .iter()
        .position(|sequence| sequence.sequence_key.trim() == key)
    {
        return format!("/schedule/sequences/{index}/climax_scene");
    }
    format!("/merged_sequences/{key}/climax_scene")
}

/// Enforce the global `sequence_key` identity across refinements.
///
/// A Sequence is merged by `sequence_key` alone: reusing a key means refining
/// that Sequence, so its `plot_line_id` may not move to another plot line. The
/// comparison resolves both references through the merged plot-line candidates,
/// so a proposal ref and the record id it became are the same plot line. An
/// unresolved reference is left to the generic reference validation.
fn validate_sequence_identity(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    scope: &PlanningScope,
    candidates: &[PlotLineStructureCandidate],
    receipts: &BTreeMap<String, String>,
    findings: &mut Vec<PlanningFinding>,
) -> Result<()> {
    if input.level != PlanningLevel::Chapter || input.schedule.sequences.is_empty() {
        return Ok(());
    }
    let mut base = accepted_planning_sequences_in_conn(conn, &scope.project_id, &scope.branch_id)?;
    for ancestor in collect_staged_ancestor_proposals_in_conn(conn, input)? {
        if ancestor.artifact.level != PlanningLevel::Chapter {
            continue;
        }
        base = spindle_core::sequences::merge_planned_sequences(
            &base,
            &ancestor.artifact.schedule.sequences,
        );
    }
    for (index, sequence) in input.schedule.sequences.iter().enumerate() {
        let Some(previous) = base
            .iter()
            .find(|previous| previous.sequence_key == sequence.sequence_key)
        else {
            continue;
        };
        let (Some(previous_ref), Some(current_ref)) = (
            previous.plot_line_id.as_ref(),
            sequence.plot_line_id.as_ref(),
        ) else {
            continue;
        };
        let (Some(previous_line), Some(current_line)) = (
            resolve_plot_line(candidates, previous_ref, receipts),
            resolve_plot_line(candidates, current_ref, receipts),
        ) else {
            continue;
        };
        if previous_line.related_ref == current_line.related_ref {
            continue;
        }
        findings.push(planning_finding(
            "sequence_plot_line_changed",
            PlanningFindingSeverity::Blocking,
            format!("/schedule/sequences/{index}/plot_line_id"),
            vec![
                sequence.sequence_key.clone(),
                previous_line.related_ref.clone(),
                current_line.related_ref.clone(),
            ],
            format!(
                "Sequence '{}' keeps the plot line it was first staged under ('{}')",
                sequence.sequence_key, previous_line.related_ref
            ),
            format!(
                "the refinement moves it to plot line '{}'",
                current_line.related_ref
            ),
            "A sequence_key is a global identity: keep plot_line_id unchanged when refining this Sequence, or give the different Subplot's Sequence a new key.",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Axis advisories: dormancy, job support and depth against the Main Plot
// ---------------------------------------------------------------------------

fn validate_subplot_axes(
    conn: &rusqlite::Connection,
    scope: &PlanningScope,
    candidates: &[PlotLineStructureCandidate],
    scenes: &[SceneValueEntry],
    acts: &[PlannedAct],
    findings: &mut Vec<PlanningFinding>,
) -> Result<()> {
    if scenes.is_empty() {
        return Ok(());
    }
    let main_axis = main_plot_axis(conn, scope, candidates)?;
    let mut turns: Vec<AxisTurn> = Vec::new();
    for scene in scenes {
        for (suffix, axis) in scene.axes() {
            if axis.axis.trim().is_empty() || !axis.turns() {
                continue;
            }
            turns.push(AxisTurn {
                scene,
                suffix,
                axis,
            });
        }
    }
    // Act ranges are only meaningful when the merged Acts' exact climaxes rise
    // in global story order. The Act-list gate proves only that they rise
    // inside one book; a later-order Act in an earlier book would make one
    // Act's range empty and let another Act's turns be read as its own. Skip
    // the depth comparison rather than attribute turns to the wrong Act.
    let ranges = if act_climaxes_strictly_increasing(acts) {
        act_story_ranges(acts)
    } else {
        Vec::new()
    };
    // The Main Plot rank is tracked independently of the per-Subplot axis
    // lookup: a Subplot may legally share the Main Plot's Value Axis, and then
    // both maps read the same turns and the ranks are equal (never a false
    // deeper-than-main finding).
    let main_deepest_by_act = main_axis
        .as_deref()
        .map(|axis| deepest_rank_by_act(&turns, axis, &ranges))
        .unwrap_or_default();

    for subplot in candidates.iter().filter(|item| !item.is_main_plot) {
        let Some(value_axis) = subplot
            .value_axis
            .as_deref()
            .map(str::trim)
            .filter(|axis| !axis.is_empty())
        else {
            // Without a Value Axis there is no axis to watch or compare.
            continue;
        };
        let axis_turns: Vec<&AxisTurn> = turns
            .iter()
            .filter(|turn| turn.axis.axis.trim() == value_axis)
            .collect();

        validate_dormancy(subplot, value_axis, scenes, &axis_turns, findings);
        validate_job_support(subplot, value_axis, &axis_turns, findings);
        if let Some(main_axis) = main_axis.as_deref() {
            validate_depth_in_acts(
                subplot,
                value_axis,
                main_axis,
                &turns,
                &ranges,
                &main_deepest_by_act,
                findings,
            );
        }
    }
    Ok(())
}

/// The deepest closing Charge rank per Act for one axis. See
/// [`validate_depth_in_acts`].
fn deepest_rank_by_act(
    turns: &[AxisTurn],
    axis: &str,
    ranges: &[ActStoryRange],
) -> BTreeMap<i32, u8> {
    let mut by_act: BTreeMap<i32, u8> = BTreeMap::new();
    for turn in turns {
        if turn.axis.axis.trim() != axis {
            continue;
        }
        let Some(range) = act_range_for_scene(ranges, &scene_placement(turn.scene)) else {
            continue;
        };
        let rank = spindle_core::acts::charge_depth_rank(turn.axis.closing.depth());
        by_act
            .entry(range.order)
            .and_modify(|deepest| *deepest = (*deepest).max(rank))
            .or_insert(rank);
    }
    by_act
}

/// The Main Plot's Value Axis: the merged Main Plot's `value_axis`, falling back
/// to the accepted series Spine Axis when the row carries none. The two are
/// required to match by the Main Plot gate, so either is the axis the depth
/// advisory compares against.
fn main_plot_axis(
    conn: &rusqlite::Connection,
    scope: &PlanningScope,
    candidates: &[PlotLineStructureCandidate],
) -> Result<Option<String>> {
    if let Some(axis) = candidates
        .iter()
        .find(|candidate| candidate.is_main_plot)
        .and_then(|candidate| candidate.value_axis.as_deref())
        .map(str::trim)
        .filter(|axis| !axis.is_empty())
    {
        return Ok(Some(axis.to_string()));
    }
    let spine: Option<String> = conn
        .query_row(
            "SELECT spine_axis FROM story_spine WHERE project_id=?1 AND series_spine_id IS NULL",
            [&scope.project_id],
            |row| row.get(0),
        )
        .optional_inner()?;
    Ok(spine
        .map(|axis| axis.trim().to_string())
        .filter(|axis| !axis.is_empty()))
}

/// Advisory: a Subplot Value Axis that goes [`DORMANT_CHAPTER_RUN`] consecutive
/// scanned Chapters without a turn.
///
/// Run definition (documented): scan the distinct `(book_number,
/// chapter_number)` pairs the merged planned Scenes carry, in story order. A
/// Chapter *turns* the axis when any planned Scene in it has any axis entry
/// (primary or secondary) whose name matches the Subplot's Value Axis
/// (trim-exact) and whose opening and closing Charge differ. A run continues
/// only while the next scanned Chapter is the immediately following Chapter in
/// the same book (`n + 1`); a chapter gap or a turning Chapter resets it. The
/// advisory fires once when a run reaches the third Chapter, naming the last
/// planned Scene of the Chapter that completed the run.
fn validate_dormancy(
    subplot: &PlotLineStructureCandidate,
    value_axis: &str,
    scenes: &[SceneValueEntry],
    axis_turns: &[&AxisTurn],
    findings: &mut Vec<PlanningFinding>,
) {
    let planned_chapters: BTreeSet<(i32, i32)> = scenes
        .iter()
        .map(|scene| (scene.coordinates().0, scene.coordinates().1))
        .collect();
    let turned_chapters: BTreeSet<(i32, i32)> = axis_turns
        .iter()
        .map(|turn| (turn.scene.coordinates().0, turn.scene.coordinates().1))
        .collect();

    for run in dormant_chapter_runs(planned_chapters, &turned_chapters) {
        let Some(last_scene) = last_scene_of_chapter(scenes, run.end) else {
            continue;
        };
        findings.push(planning_finding(
            "subplot_axis_dormant",
            PlanningFindingSeverity::Advisory,
            last_scene.finding_path().to_string(),
            vec![subplot.related_ref.clone()],
            format!(
                "a turn on the Subplot Value Axis '{value_axis}' within every {DORMANT_CHAPTER_RUN} Chapters"
            ),
            format!(
                "axis '{value_axis}' does not turn in book {} chapters {}-{} ({} consecutive Chapters)",
                run.start.0, run.start.1, run.end.1, DORMANT_CHAPTER_RUN
            ),
            "Give the Subplot a Value Turn inside this run (a planned Scene on its axis with a different opening and closing Charge), or move an existing turn into the window; a Subplot that stops moving is forgotten.",
        ));
    }
}

fn last_scene_of_chapter(
    scenes: &[SceneValueEntry],
    chapter: (i32, i32),
) -> Option<&SceneValueEntry> {
    scenes
        .iter()
        .filter(|scene| scene.coordinates().0 == chapter.0 && scene.coordinates().1 == chapter.1)
        .max_by_key(|scene| scene.coordinates().2)
}

/// Advisory: a Subplot's turns do not support its declared job.
///
/// The heuristic is [`job_requires_closing_sign`]: `contradict` needs at least
/// one turn on the Subplot's Value Axis ending at a negative Charge, `resonate`
/// needs at least one ending positive, and `set_up` / `complicate` impose no
/// direction requirement. The advisory fires only when the axis does turn at
/// least once but no turn satisfies the requirement; a Subplot with no turns at
/// all is the dormancy advisory's concern.
fn validate_job_support(
    subplot: &PlotLineStructureCandidate,
    value_axis: &str,
    axis_turns: &[&AxisTurn],
    findings: &mut Vec<PlanningFinding>,
) {
    let Some(job) = subplot.subplot_job else {
        // The Main Plot / Subplot gate already blocks a jobless Subplot.
        return;
    };
    let Some(required) = job_requires_closing_sign(job) else {
        return;
    };
    if axis_turns.is_empty()
        || axis_turns
            .iter()
            .any(|turn| closing_supports_job(job, turn.axis.closing))
    {
        return;
    }
    findings.push(planning_finding(
        "subplot_job_unsupported",
        PlanningFindingSeverity::Advisory,
        subplot.field_path("subplot_job"),
        vec![subplot.related_ref.clone()],
        format!(
            "at least one turn on axis '{value_axis}' ending at a {required} Charge, so the Subplot {job}s the Controlling Idea"
        ),
        format!(
            "every turn on axis '{value_axis}' ends at a {} Charge",
            axis_turns
                .iter()
                .map(|turn| turn.axis.closing.sign().to_string())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(" or ")
        ),
        format!(
            "Turn axis '{value_axis}' toward a {required} Charge at least once, or change the Subplot's job; this is advisory."
        ),
    ));
}

/// Advisory: a Subplot turns deeper than the Main Plot inside one Act.
///
/// Act ranges follow [`act_story_ranges`]: Act N covers the Scenes after Act
/// N-1's climax up to and including Act N's climax. The caller only supplies
/// ranges when [`act_climaxes_strictly_increasing`] holds, so turns are never
/// attributed across a book boundary that breaks global story order. For each
/// Act, compare the deepest closing Charge rank on the Subplot's Value Axis with
/// the independently computed deepest rank on the Main Plot's axis in the same
/// Act. The advisory fires only when both axes turn in that Act and the Subplot
/// is deeper; a Scene after the last Act's climax belongs to no Act and is never
/// compared.
///
/// A Subplot may legally share the Main Plot's Value Axis. Both sides then read
/// the same turns and ranks are equal, so the advisory never produces a false
/// positive.
fn validate_depth_in_acts(
    subplot: &PlotLineStructureCandidate,
    value_axis: &str,
    main_axis: &str,
    turns: &[AxisTurn],
    ranges: &[ActStoryRange],
    main_deepest_by_act: &BTreeMap<i32, u8>,
    findings: &mut Vec<PlanningFinding>,
) {
    if ranges.is_empty() {
        return;
    }
    let mut subplot_by_act: BTreeMap<i32, Vec<&AxisTurn>> = BTreeMap::new();
    for turn in turns {
        if turn.axis.axis.trim() != value_axis {
            continue;
        }
        let Some(range) = act_range_for_scene(ranges, &scene_placement(turn.scene)) else {
            continue;
        };
        subplot_by_act.entry(range.order).or_default().push(turn);
    }
    for (order, subplot_turns) in subplot_by_act {
        let Some(subplot_deepest) =
            deepest_charge_rank(subplot_turns.iter().map(|turn| &turn.axis.closing))
        else {
            continue;
        };
        let Some(main_deepest) = main_deepest_by_act.get(&order).copied() else {
            continue;
        };
        if subplot_deepest <= main_deepest {
            continue;
        }
        let Some(deepest) = subplot_turns
            .iter()
            .find(|turn| {
                spindle_core::acts::charge_depth_rank(turn.axis.closing.depth()) == subplot_deepest
            })
            .copied()
        else {
            continue;
        };
        findings.push(planning_finding(
            "subplot_deeper_than_main_plot",
            PlanningFindingSeverity::Advisory,
            format!("{}/{}", deepest.scene.finding_path(), deepest.suffix),
            vec![subplot.related_ref.clone()],
            format!(
                "a Subplot turn on axis '{value_axis}' no deeper than the Main Plot axis '{main_axis}' in Act {order}"
            ),
            format!(
                "axis '{value_axis}' closes at {} / {} (rank {subplot_deepest}) while the Main Plot's deepest turn in Act {order} is rank {main_deepest}",
                deepest.axis.closing.sign(),
                deepest.axis.closing.depth()
            ),
            format!(
                "Keep the Subplot's Value Axis at or above the Main Plot's depth inside Act {order}, or deepen the Main Plot; a Subplot that turns deeper steals the Act."
            ),
        ));
    }
}

/// The exact placement of a planned Scene, for the Act-range comparison.
fn scene_placement(scene: &SceneValueEntry) -> PlanningStoryPlacement {
    let (book_number, chapter_number, scene_order) = scene.coordinates();
    PlanningStoryPlacement {
        book_number,
        chapter_number,
        scene_order: Some(scene_order),
        note: None,
    }
}

// ---------------------------------------------------------------------------
// Act-cause reference
// ---------------------------------------------------------------------------

/// Validate the optional Act-cause reference at every staging level.
///
/// An Act may cite a Subplot Sequence climax as the cause of its own climax.
/// The reference must resolve against the merged sources:
///
/// * the plot line exists and is a Subplot (never the Main Plot);
/// * a Sequence with the cited `sequence_key` exists and belongs to that
///   Subplot (the key is global: a key owned by a different plot line is a data
///   error, not an ambiguous lookup);
/// * the cited Sequence has an exact climax placement at or before one of the
///   Subplot's Convergence Points;
/// * the cited Sequence climax is at or before the Act's own climax (cause
///   precedes effect).
///
/// Series-Arc/Book staging validates the Acts it authors; Chapter/Scene staging
/// re-validates the merged accepted Acts so a Chapter refinement that moves a
/// cited Sequence climax cannot silently invalidate an accepted cause. Every
/// semantic failure is a stage-recordable Blocking finding: the proposal stages
/// with the finding visible and acceptance is refused. A reference that cannot
/// resolve at all is already a hard staging error in the generic reference
/// validation; the unresolved-plot-line branch here remains reachable for a row
/// the reference check accepts but the merged candidate set excludes (an
/// archived plot line), so it stays stage-recordable with a readable code.
pub(super) fn validate_mckee_act_causes(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    scope: &PlanningScope,
    findings: &mut Vec<PlanningFinding>,
) -> Result<()> {
    if !project_is_mckee_in_conn(conn, &scope.project_id)? {
        return Ok(());
    }
    let acts = evaluated_mckee_acts_in_conn(conn, input, scope)?;
    if !acts.iter().any(|act| act.cause.is_some()) {
        return Ok(());
    }
    let candidates = evaluated_plot_line_candidates_in_conn(conn, input, scope)?;
    let sequences = super::values::evaluated_sequences_in_conn(conn, input, scope)?;
    let receipts = apply::refs_in_conn(conn, &input.session_id)?;
    let own_index_by_order: BTreeMap<i32, usize> = input
        .schedule
        .acts
        .iter()
        .enumerate()
        .map(|(index, act)| (act.order, index))
        .collect();
    let accepted_orders: BTreeSet<i32> =
        accepted_planning_acts_in_conn(conn, &scope.project_id, &scope.branch_id)?
            .iter()
            .map(|act| act.order)
            .collect();
    let mut staged_orders: BTreeSet<i32> = BTreeSet::new();
    for ancestor in collect_staged_ancestor_proposals_in_conn(conn, input)? {
        staged_orders.extend(ancestor.artifact.schedule.acts.iter().map(|act| act.order));
    }

    for act in &acts {
        let Some(cause) = act.cause.as_ref() else {
            continue;
        };
        let base = act_cause_path(
            act.order,
            own_index_by_order.get(&act.order).copied(),
            &staged_orders,
            &accepted_orders,
        );
        let Some(subplot) = resolve_plot_line(&candidates, &cause.plot_line_id, &receipts) else {
            findings.push(planning_finding(
                "act_cause_unresolved_subplot_sequence",
                PlanningFindingSeverity::Blocking,
                format!("{base}/plot_line_id"),
                vec![describe_ref(&cause.plot_line_id), act.order.to_string()],
                "a visible, non-archived plot line that is a Subplot",
                "the plot line reference does not resolve to a merged plot line",
                "Point the cause at a plot line that exists on the session branch (an existing record id or an active parent proposal ref) and has not been archived.",
            ));
            continue;
        };
        if subplot.is_main_plot {
            findings.push(planning_finding(
                "act_cause_not_a_subplot",
                PlanningFindingSeverity::Blocking,
                format!("{base}/plot_line_id"),
                vec![subplot.related_ref.clone(), act.order.to_string()],
                "a Subplot plot line (is_main_plot=false)",
                "the cited plot line is the Main Plot",
                "An Act climax's cause must come from a Subplot Sequence, not the Main Plot's own spine; cite the Subplot that drives this Act or clear the cause.",
            ));
            continue;
        }
        let key = cause.sequence_key.trim();
        if key.is_empty() {
            findings.push(planning_finding(
                "act_cause_unresolved_subplot_sequence",
                PlanningFindingSeverity::Blocking,
                format!("{base}/sequence_key"),
                vec![subplot.related_ref.clone(), act.order.to_string()],
                "a non-empty sequence_key naming one of the Subplot's Sequences",
                "empty sequence_key",
                "Name the cited Sequence's global merge key; without one the cause cannot resolve.",
            ));
            continue;
        }
        let Some(sequence) = sequences
            .iter()
            .find(|sequence| sequence.sequence_key.trim() == key)
        else {
            findings.push(planning_finding(
                "act_cause_unresolved_subplot_sequence",
                PlanningFindingSeverity::Blocking,
                format!("{base}/sequence_key"),
                vec![key.to_string(), act.order.to_string()],
                format!("a Sequence with key '{key}' in the merged Sequences"),
                "no Sequence carries that key",
                "Stage the Sequence at the Chapter level (or cite the key of an accepted/staged Sequence).",
            ));
            continue;
        };
        let sequence_plot_line = sequence
            .plot_line_id
            .as_ref()
            .and_then(|reference| resolve_plot_line(&candidates, reference, &receipts));
        match sequence_plot_line {
            None => {
                findings.push(planning_finding(
                    "act_cause_unresolved_subplot_sequence",
                    PlanningFindingSeverity::Blocking,
                    format!("{base}/sequence_key"),
                    vec![key.to_string(), act.order.to_string()],
                    format!("a Sequence whose plot line is '{}'", subplot.related_ref),
                    "the cited Sequence has no resolvable plot line",
                    "Repair the Sequence's plot_line_id, or cite a Sequence that belongs to the Subplot.",
                ));
                continue;
            }
            Some(sequence_plot_line) if sequence_plot_line.related_ref != subplot.related_ref => {
                findings.push(planning_finding(
                    "act_cause_unresolved_subplot_sequence",
                    PlanningFindingSeverity::Blocking,
                    format!("{base}/sequence_key"),
                    vec![key.to_string(), act.order.to_string()],
                    format!(
                        "Sequence key '{key}' to be owned by the Subplot '{}' named in plot_line_id",
                        subplot.related_ref
                    ),
                    format!(
                        "the globally keyed Sequence '{key}' belongs to plot line '{}'",
                        sequence_plot_line.related_ref
                    ),
                    "A sequence_key is globally unique: this is a data error (a key cannot name two plot lines). Cite the cited Subplot's own Sequence key, or repair the Sequence's plot_line_id; do not change plot_line_id to match a colliding key.",
                ));
                continue;
            }
            Some(_) => {}
        }
        let Some(sequence_climax) = sequence.climax_scene.as_ref() else {
            findings.push(planning_finding(
                "act_cause_unresolved_subplot_sequence",
                PlanningFindingSeverity::Blocking,
                format!("{base}/sequence_key"),
                vec![key.to_string(), act.order.to_string()],
                "a Sequence with an exact climax placement",
                "the cited Sequence has no climax",
                "Give the Sequence a climax Scene; an Act cause cites the Sequence's strongest Scene.",
            ));
            continue;
        };
        if sequence_climax.scene_order.is_none() {
            findings.push(planning_finding(
                "act_cause_unresolved_subplot_sequence",
                PlanningFindingSeverity::Blocking,
                format!("{base}/sequence_key"),
                vec![key.to_string(), act.order.to_string()],
                "a Sequence climax with an exact scene_order",
                "the cited Sequence climax is chapter-wide",
                "Give the Sequence climax an exact scene_order so the cause can be ordered against the Act and the Convergence Points.",
            ));
            continue;
        }
        let Some(act_climax) = act.climax.as_ref() else {
            // The Act-list gate reports the missing climax; nothing to order
            // the cause against.
            continue;
        };
        if act_climax.scene_order.is_none() {
            continue;
        }
        if sequence_climax.cmp_story_order(act_climax) == Ordering::Greater {
            findings.push(planning_finding(
                "act_cause_after_act_climax",
                PlanningFindingSeverity::Blocking,
                format!("{base}/sequence_key"),
                vec![key.to_string(), act.order.to_string()],
                format!(
                    "the cited Sequence climax at or before Act {}'s climax (book {} chapter {} scene {})",
                    act.order,
                    act_climax.book_number,
                    act_climax.chapter_number,
                    act_climax.scene_order.unwrap_or_default()
                ),
                format!(
                    "the cited climax at book {} chapter {} scene {} comes after the Act climax",
                    sequence_climax.book_number,
                    sequence_climax.chapter_number,
                    sequence_climax.scene_order.unwrap_or_default()
                ),
                "A cause precedes its effect: move the Sequence climax before the Act climax, or cite a Sequence that lands earlier.",
            ));
            continue;
        }
        if !climax_within_convergence_points(sequence_climax, &subplot.convergence_points) {
            let actual = if subplot.convergence_points.is_empty() {
                format!(
                    "Subplot '{}' has no Convergence Point to land the cited climax",
                    subplot.related_ref
                )
            } else {
                format!(
                    "the cited climax is after every Convergence Point ({})",
                    describe_placements(&subplot.convergence_points)
                )
            };
            findings.push(planning_finding(
                "act_cause_after_convergence_point",
                PlanningFindingSeverity::Blocking,
                format!("{base}/sequence_key"),
                vec![key.to_string(), subplot.related_ref.clone()],
                "the cited Sequence climax at or before one of the Subplot's Convergence Points",
                actual,
                "Move the cited Sequence climax to or before a Convergence Point (or add a later one): the cause must be a place where the Subplot feeds the Main Plot.",
            ));
        }
    }
    Ok(())
}

/// A descriptive locator for an Act's cause: the current artifact's schedule
/// entry when it authors the Act, a staged ancestor's entry when that Act was
/// last authored there, otherwise the accepted Act row.
fn act_cause_path(
    order: i32,
    own_index: Option<usize>,
    staged_orders: &BTreeSet<i32>,
    accepted_orders: &BTreeSet<i32>,
) -> String {
    if let Some(index) = own_index {
        return format!("/schedule/acts/{index}/cause");
    }
    if staged_orders.contains(&order) {
        return format!("/staged_acts/{order}/cause");
    }
    if accepted_orders.contains(&order) {
        return format!("/accepted_acts/{order}/cause");
    }
    // Defensive fallback: every evaluated Act comes from one of the sources
    // above. Keep a stable locator instead of inventing a path.
    format!("/accepted_acts/{order}/cause")
}

/// The first candidate whose record id / output ref (or receipt-resolved id)
/// matches the reference.
fn resolve_plot_line<'a>(
    candidates: &'a [PlotLineStructureCandidate],
    reference: &PlanningRef,
    receipts: &BTreeMap<String, String>,
) -> Option<&'a PlotLineStructureCandidate> {
    candidates
        .iter()
        .find(|candidate| candidate.matches(reference, receipts))
}

fn describe_ref(reference: &PlanningRef) -> String {
    match reference {
        PlanningRef::ExistingRecordId(id) => id.clone(),
        PlanningRef::ProposalRef(key) => key.clone(),
    }
}

fn describe_placements(placements: &[PlanningStoryPlacement]) -> String {
    placements
        .iter()
        .map(|placement| {
            format!(
                "book {} chapter {} scene {}",
                placement.book_number,
                placement.chapter_number,
                placement.scene_order.unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
