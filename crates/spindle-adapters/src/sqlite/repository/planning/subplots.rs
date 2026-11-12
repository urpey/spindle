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
use spindle_core::charge::Charge;
use spindle_core::planning::{PlannedAct, PlannedSequence, PlanningRef, PlanningStoryPlacement};
use spindle_core::scene_value::SceneValueAxis;
use spindle_core::subplot::{
    DORMANT_CHAPTER_RUN, act_range_for_scene, act_story_ranges, chapters_are_consecutive,
    climax_within_convergence_points, closing_supports_job, deepest_charge_rank,
    job_requires_closing_sign,
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

    validate_convergence_points(input, &subplots, sequences, &receipts, findings);
    validate_subplot_axes(conn, scope, &candidates, scenes, acts, findings)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Convergence points: the blocking rule and the missing-point advisory
// ---------------------------------------------------------------------------

fn validate_convergence_points(
    input: &StagePlanningInput,
    subplots: &[&PlotLineStructureCandidate],
    sequences: &[PlannedSequence],
    receipts: &std::collections::BTreeMap<String, String>,
    findings: &mut Vec<PlanningFinding>,
) {
    for subplot in subplots {
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
    let ranges = act_story_ranges(acts);

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
            validate_depth_in_acts(subplot, value_axis, main_axis, &turns, &ranges, findings);
        }
    }
    Ok(())
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
    let planned_chapters: Vec<(i32, i32)> = scenes
        .iter()
        .map(|scene| (scene.coordinates().0, scene.coordinates().1))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let turned_chapters: BTreeSet<(i32, i32)> = axis_turns
        .iter()
        .map(|turn| (turn.scene.coordinates().0, turn.scene.coordinates().1))
        .collect();

    let mut run = 0usize;
    let mut run_start: Option<(i32, i32)> = None;
    let mut previous: Option<(i32, i32)> = None;
    for chapter in planned_chapters {
        if turned_chapters.contains(&chapter) {
            run = 0;
            run_start = None;
        } else {
            let consecutive =
                previous.is_some_and(|previous| chapters_are_consecutive(previous, chapter));
            if consecutive && run > 0 {
                run += 1;
            } else {
                run = 1;
                run_start = Some(chapter);
            }
            if run == DORMANT_CHAPTER_RUN {
                let start = run_start.unwrap_or(chapter);
                let Some(last_scene) = last_scene_of_chapter(scenes, chapter) else {
                    previous = Some(chapter);
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
                        start.0, start.1, chapter.1, DORMANT_CHAPTER_RUN
                    ),
                    "Give the Subplot a Value Turn inside this run (a planned Scene on its axis with a different opening and closing Charge), or move an existing turn into the window; a Subplot that stops moving is forgotten.",
                ));
            }
        }
        previous = Some(chapter);
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
/// N-1's climax up to and including Act N's climax. For each Act, compare the
/// deepest closing Charge rank on the Subplot's Value Axis with the deepest
/// closing rank on the Main Plot's axis in the same Act. The advisory fires
/// only when both axes turn in that Act and the Subplot is deeper; a Scene after
/// the last Act's climax belongs to no Act and is never compared.
fn validate_depth_in_acts(
    subplot: &PlotLineStructureCandidate,
    value_axis: &str,
    main_axis: &str,
    turns: &[AxisTurn],
    ranges: &[spindle_core::subplot::ActStoryRange],
    findings: &mut Vec<PlanningFinding>,
) {
    if ranges.is_empty() {
        return;
    }
    let mut subplot_by_act: BTreeMap<i32, Vec<&AxisTurn>> = BTreeMap::new();
    let mut main_by_act: BTreeMap<i32, Vec<&Charge>> = BTreeMap::new();
    for turn in turns {
        let placement = scene_placement(turn.scene);
        let Some(range) = act_range_for_scene(ranges, &placement) else {
            continue;
        };
        if turn.axis.axis.trim() == value_axis {
            subplot_by_act.entry(range.order).or_default().push(turn);
        } else if turn.axis.axis.trim() == main_axis {
            main_by_act
                .entry(range.order)
                .or_default()
                .push(&turn.axis.closing);
        }
    }
    for (order, subplot_turns) in subplot_by_act {
        let Some(subplot_deepest) =
            deepest_charge_rank(subplot_turns.iter().map(|turn| &turn.axis.closing))
        else {
            continue;
        };
        let Some(main_deepest) = main_by_act
            .get(&order)
            .and_then(|charges| deepest_charge_rank(charges.iter().copied()))
        else {
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

/// Validate the optional Act-cause reference at Series-Arc/Book acceptance.
///
/// An Act may cite a Subplot Sequence climax as the cause of its own climax.
/// The reference must resolve against the merged sources:
///
/// * the plot line exists and is a Subplot (never the Main Plot);
/// * a Sequence with the cited `sequence_key` exists and belongs to that
///   Subplot;
/// * the cited Sequence has an exact climax placement at or before one of the
///   Subplot's Convergence Points;
/// * the cited Sequence climax is at or before the Act's own climax (cause
///   precedes effect).
///
/// Every failure is a stage-recordable Blocking finding: the proposal stages
/// with the finding visible and acceptance is refused. Acts inherited from an
/// accepted artifact are re-checked too, so a later Sequence refinement that
/// moves a cited climax cannot silently invalidate an accepted cause.
pub(super) fn validate_mckee_act_causes(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    scope: &Option<PlanningScope>,
    findings: &mut Vec<PlanningFinding>,
) -> Result<()> {
    let Some(scope) = scope else {
        return Ok(());
    };
    if !project_is_mckee_in_conn(conn, &scope.project_id)? {
        return Ok(());
    }
    if !matches!(input.level, PlanningLevel::SeriesArc | PlanningLevel::Book) {
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

    for act in &acts {
        let Some(cause) = act.cause.as_ref() else {
            continue;
        };
        let base = match own_index_by_order.get(&act.order) {
            Some(index) => format!("/schedule/acts/{index}/cause"),
            None => format!("/merged_acts/{}/cause", act.order),
        };
        let Some(subplot) = resolve_plot_line(&candidates, &cause.plot_line_id, &receipts) else {
            findings.push(planning_finding(
                "act_cause_unresolved_subplot_sequence",
                PlanningFindingSeverity::Blocking,
                format!("{base}/plot_line_id"),
                vec![describe_ref(&cause.plot_line_id), act.order.to_string()],
                "a visible plot line that is a Subplot",
                "the plot line reference does not resolve",
                "Point the cause at a plot line that exists on the session branch (an existing record id or an active parent proposal ref).",
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
                "Name the cited Sequence's schedule-local key; without one the cause cannot resolve.",
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
                        "Sequence '{key}' belonging to Subplot '{}'",
                        subplot.related_ref
                    ),
                    format!(
                        "Sequence '{key}' belongs to plot line '{}'",
                        sequence_plot_line.related_ref
                    ),
                    "Cite a Sequence of the Subplot named by plot_line_id, or change plot_line_id to the Sequence's plot line.",
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

/// The first candidate whose record id / output ref (or receipt-resolved id)
/// matches the reference.
fn resolve_plot_line<'a>(
    candidates: &'a [PlotLineStructureCandidate],
    reference: &PlanningRef,
    receipts: &std::collections::BTreeMap<String, String>,
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
