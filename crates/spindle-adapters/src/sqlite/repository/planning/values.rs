//! McKee value-chain validation over the planned Scenes of a project branch.
//!
//! The artifact's own `schedule.sequences` are checked at every level for a
//! McKee-structured project (a non-empty list outside Chapter is the blocking
//! `sequence_wrong_level`). The value-chain pass itself runs for Chapter and
//! Scene artifacts. It builds the ordered planned-Scene list a decision would
//! leave behind — accepted chapter plans visible from the branch lineage, the
//! staged ancestors in the same approved closure, and the current artifact's
//! `plan_chapter` operations — and evaluates the Sequence grouping and the
//! scene value plan.
//!
//! **Precedence.** A later plan for the same `(book_number, chapter_number)`
//! replaces the earlier one: accepted plans first, then staged ancestors in
//! creation order, then the current artifact. Sequences merge by
//! `sequence_key` instead (see [`spindle_core::sequences`]), because a later
//! Chapter artifact may refine a Sequence that an earlier chapter opened: the
//! later entry overrides the scalar fields and unions `member_scenes`.
//!
//! All blocking findings here are stage-recordable: the proposal stages with the
//! findings visible and acceptance refuses until they are repaired.

use super::*;
use spindle_core::beats::{BeatKind, SceneBeatError, scene_beat_errors};
use spindle_core::models::PlannedScene;
use spindle_core::planning::{PlannedAct, PlannedSequence, PlanningRef, PlanningStoryPlacement};
use spindle_core::scene_value::{
    SceneDilemma, SceneGap, SceneValueAxis, proposal_character_reference, proposal_output_ref,
    scene_value_errors,
};
use spindle_core::sequences::{merge_planned_sequences, placement_is_member, same_scene_position};
use std::collections::{BTreeMap, BTreeSet};

/// One structured Beat as the promise gates read it: kind, whether its Gap is
/// information-dependent, and the promise it pays off (a planning reference
/// before resolution, a record id after).
#[derive(Debug, Clone)]
pub(super) struct SceneGateBeat {
    pub kind: BeatKind,
    pub information_dependent: bool,
    pub promise: Option<PlanningRef>,
}

/// One planned Scene as the promise / Key Fact gates read it: its placement,
/// its Beats, its Turning Beat, and where to point a finding.
#[derive(Debug, Clone)]
pub(super) struct SceneGateEntry {
    pub book_number: i32,
    pub chapter_number: i32,
    pub scene_order: i32,
    pub turning_beat: Option<u32>,
    pub beats: Vec<SceneGateBeat>,
    pub path: String,
    pub repair_hint: String,
}

impl SceneGateEntry {
    pub(super) fn placement(&self) -> PlanningStoryPlacement {
        PlanningStoryPlacement {
            book_number: self.book_number,
            chapter_number: self.chapter_number,
            scene_order: Some(self.scene_order),
            note: None,
        }
    }
}

/// One planned Scene with the value fields the chain rules read, plus the
/// finding path and repair hint for the plan that owns it.
#[derive(Debug, Clone)]
pub(super) struct SceneValueEntry {
    book_number: i32,
    chapter_number: i32,
    scene_order: i32,
    beats: Vec<SceneGateBeat>,
    turning_beat: Option<u32>,
    primary: Option<SceneValueAxis>,
    secondary: Vec<SceneValueAxis>,
    gap: Option<SceneGap>,
    inaction_cost: Option<String>,
    risk: Option<i32>,
    dilemma: Option<SceneDilemma>,
    path: String,
    repair_hint: String,
}

impl SceneValueEntry {
    fn from_planned_scene(
        book_number: i32,
        chapter_number: i32,
        scene: &PlannedScene,
        path: String,
        repair_hint: String,
    ) -> Self {
        Self {
            book_number,
            chapter_number,
            scene_order: scene.scene_order,
            beats: scene
                .beats
                .iter()
                .map(|beat| SceneGateBeat {
                    kind: beat.kind,
                    information_dependent: beat.information_dependent,
                    promise: beat.promise_id.clone().map(PlanningRef::ExistingRecordId),
                })
                .collect(),
            turning_beat: scene.turning_beat,
            primary: scene.primary_value_axis.clone(),
            secondary: scene.secondary_value_axes.clone(),
            gap: scene.gap.clone(),
            inaction_cost: scene.inaction_cost.clone(),
            risk: scene.risk,
            dilemma: scene.dilemma.clone(),
            path,
            repair_hint,
        }
    }

    fn from_planning_scene(
        book_number: i32,
        chapter_number: i32,
        scene: &PlanningPlanChapterSceneInput,
        path: String,
        repair_hint: String,
    ) -> Self {
        Self {
            book_number,
            chapter_number,
            scene_order: scene.scene_order,
            beats: scene
                .beats
                .iter()
                .map(|beat| SceneGateBeat {
                    kind: beat.kind,
                    information_dependent: beat.information_dependent,
                    promise: beat.promise_id.clone(),
                })
                .collect(),
            turning_beat: scene.turning_beat,
            primary: scene.primary_value_axis.clone(),
            secondary: scene.secondary_value_axes.clone(),
            gap: scene.gap.clone(),
            inaction_cost: scene.inaction_cost.clone(),
            risk: scene.risk,
            dilemma: scene.dilemma.as_ref().map(|dilemma| SceneDilemma {
                character_id: normalized_ref(&dilemma.character_id),
                contradiction: dilemma.contradiction.clone(),
            }),
            path,
            repair_hint,
        }
    }

    fn beat_kinds(&self) -> Vec<BeatKind> {
        self.beats.iter().map(|beat| beat.kind).collect()
    }

    fn placement(&self) -> PlanningStoryPlacement {
        PlanningStoryPlacement {
            book_number: self.book_number,
            chapter_number: self.chapter_number,
            scene_order: Some(self.scene_order),
            note: None,
        }
    }

    /// The Scene's exact story coordinates.
    pub(super) fn coordinates(&self) -> (i32, i32, i32) {
        (self.book_number, self.chapter_number, self.scene_order)
    }

    /// Every axis of the Scene (primary first, then secondaries) with the path
    /// suffix the finding would use for it.
    pub(super) fn axes(&self) -> Vec<(String, &SceneValueAxis)> {
        scene_axes(self)
    }

    /// The JSON-pointer path that locates this Scene's plan.
    pub(super) fn finding_path(&self) -> &str {
        &self.path
    }

    fn gate_entry(&self) -> SceneGateEntry {
        SceneGateEntry {
            book_number: self.book_number,
            chapter_number: self.chapter_number,
            scene_order: self.scene_order,
            turning_beat: self.turning_beat,
            beats: self.beats.clone(),
            path: self.path.clone(),
            repair_hint: self.repair_hint.clone(),
        }
    }
}

/// A Character Design contradiction set the value plan may test.
struct DesignContradictions {
    key: String,
    contradictions: Vec<String>,
    path: String,
}

/// Value-chain gate entry point. Records findings and never changes data.
pub(super) fn validate_mckee_value_chain(
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

    // The artifact's own Sequence list is checked at every level: a non-empty
    // list on a Series-Arc, Book, CastWorld or Contract artifact is wrong-level
    // metadata, never silently accepted and never merged. Only Chapter/Scene
    // artifacts continue into the planned-Scene value-chain pass.
    validate_own_sequences(input, findings);
    if !matches!(input.level, PlanningLevel::Chapter | PlanningLevel::Scene) {
        return Ok(());
    }

    let sequences = evaluated_sequences_in_conn(conn, input, scope)?;
    let scenes = evaluated_scene_values_in_conn(conn, input, scope)?;
    let acts = evaluated_mckee_acts_in_conn(conn, input, scope)?;
    // Subplot structure is checked against the same merged sources the value
    // chain reads: merged Sequences, the merged plot lines, the merged Acts and
    // this planned-Scene list. It runs even when no Scene is planned yet, so a
    // Chapter artifact that only groups Scenes into Sequences still cannot
    // place a Subplot Sequence climax after every Convergence Point.
    super::subplots::validate_mckee_subplot_structure(
        conn, input, scope, &scenes, &sequences, &acts, findings,
    )?;
    if scenes.is_empty() {
        return Ok(());
    }
    let designs = evaluated_design_contradictions_in_conn(conn, input, scope)?;
    let receipts = apply::refs_in_conn(conn, &input.session_id)?;

    validate_scene_value_shapes(&scenes, findings);
    validate_scene_beats(&scenes, findings);
    validate_non_events(&scenes, findings);
    validate_value_chain(&scenes, findings);
    validate_sequence_membership(&scenes, &sequences, findings);
    validate_inaction_costs(&scenes, findings);
    validate_climax_dilemmas(&scenes, &sequences, &acts, findings);
    validate_dilemma_contradictions(&scenes, &designs, &receipts, findings);
    validate_same_direction_runs(&scenes, findings);
    validate_sequence_risk(&scenes, &sequences, findings);
    validate_chapter_endings(&scenes, findings);
    validate_untested_contradictions(&scenes, &designs, &receipts, findings);
    Ok(())
}

// ---------------------------------------------------------------------------
// Structural validation of the artifact's own Sequence list
// ---------------------------------------------------------------------------

fn validate_own_sequences(input: &StagePlanningInput, findings: &mut Vec<PlanningFinding>) {
    if input.schedule.sequences.is_empty() {
        return;
    }
    if input.level != PlanningLevel::Chapter {
        findings.push(planning_finding(
            "sequence_wrong_level",
            PlanningFindingSeverity::Blocking,
            "/schedule/sequences".to_string(),
            Vec::new(),
            "no schedule.sequences at levels other than Chapter",
            format!(
                "{} Sequence(s) on a {level:?} artifact",
                input.schedule.sequences.len(),
                level = input.level
            ),
            "Stage Sequences at the Chapter level; they are chapter-level schedule metadata.",
        ));
        return;
    }
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for (index, sequence) in input.schedule.sequences.iter().enumerate() {
        let base = format!("/schedule/sequences/{index}");
        let key = sequence.sequence_key.trim();
        if key.is_empty() {
            findings.push(planning_finding(
                "sequence_key_missing",
                PlanningFindingSeverity::Blocking,
                format!("{base}/sequence_key"),
                Vec::new(),
                "a non-empty stable sequence_key",
                "empty",
                "Give the Sequence a stable key (for example 'seq:opening'); later plans merge by it.",
            ));
        } else if let Some(previous) = seen.insert(key.to_string(), index) {
            findings.push(planning_finding(
                "sequence_key_duplicate",
                PlanningFindingSeverity::Blocking,
                format!("{base}/sequence_key"),
                Vec::new(),
                "a sequence_key unique within the artifact",
                format!("'{key}' is also used by sequence {previous}"),
                "Merge the two entries into one Sequence; a key identifies exactly one Sequence.",
            ));
        }
        if !key.is_empty()
            && !spindle_core::sequences::sequence_key_is_valid(&sequence.sequence_key)
        {
            findings.push(planning_finding(
                "sequence_key_invalid",
                PlanningFindingSeverity::Blocking,
                format!("{base}/sequence_key"),
                Vec::new(),
                "a non-empty key of at most 128 ASCII characters from alphanumerics, '-', '_' or ':'",
                format!("{:?}", sequence.sequence_key),
                "Use a pointer-safe key such as 'seq:opening': it is the global merge identity and appears in finding locators, so whitespace, '/' and '~' are rejected.",
            ));
        }
        if sequence.plot_line_id.is_none() {
            findings.push(planning_finding(
                "sequence_missing_plot_line",
                PlanningFindingSeverity::Blocking,
                format!("{base}/plot_line_id"),
                Vec::new(),
                "exactly one plot line reference",
                "no plot line",
                "Name the one plot line this Sequence belongs to.",
            ));
        }
        if sequence.question.trim().is_empty() {
            findings.push(planning_finding(
                "sequence_missing_question",
                PlanningFindingSeverity::Blocking,
                format!("{base}/question"),
                Vec::new(),
                "the part of the plot line's question this Sequence answers",
                "empty",
                "State the question this Sequence answers before accepting it.",
            ));
        }
        if sequence.member_scenes.is_empty() {
            findings.push(planning_finding(
                "sequence_members_empty",
                PlanningFindingSeverity::Blocking,
                format!("{base}/member_scenes"),
                Vec::new(),
                "at least one member planned Scene",
                "no members",
                "List the planned Scenes this Sequence groups, in story order.",
            ));
        }
        for (member_index, member) in sequence.member_scenes.iter().enumerate() {
            if !member.has_positive_coordinates() || member.scene_order.is_none() {
                findings.push(planning_finding(
                    "sequence_invalid_placement",
                    PlanningFindingSeverity::Blocking,
                    format!("{base}/member_scenes/{member_index}"),
                    Vec::new(),
                    "positive book_number, chapter_number and scene_order",
                    format!(
                        "book={}, chapter={}, scene_order={:?}",
                        member.book_number, member.chapter_number, member.scene_order
                    ),
                    "Use 1-based positive coordinates with an exact scene_order for every member.",
                ));
            }
        }
        if !sequence.member_scenes.is_empty()
            && !spindle_core::sequences::members_strictly_ordered(&sequence.member_scenes)
        {
            findings.push(planning_finding(
                "sequence_members_unordered",
                PlanningFindingSeverity::Blocking,
                format!("{base}/member_scenes"),
                Vec::new(),
                "members in strict (book_number, chapter_number, scene_order) order",
                format!(
                    "{:?}",
                    sequence
                        .member_scenes
                        .iter()
                        .map(|m| (m.book_number, m.chapter_number, m.scene_order))
                        .collect::<Vec<_>>()
                ),
                "Order member_scenes by story position; a Sequence may cross chapters but not reorder them.",
            ));
        }
        match sequence.inciting_scene.as_ref() {
            None => findings.push(planning_finding(
                "sequence_missing_inciting",
                PlanningFindingSeverity::Blocking,
                format!("{base}/inciting_scene"),
                Vec::new(),
                "the Scene that opens the Sequence",
                "no inciting Scene",
                "Name the Scene that opens the Sequence; it must be at or before the climax.",
            )),
            Some(inciting) => {
                if !inciting.has_positive_coordinates() || inciting.scene_order.is_none() {
                    findings.push(planning_finding(
                        "sequence_invalid_placement",
                        PlanningFindingSeverity::Blocking,
                        format!("{base}/inciting_scene"),
                        Vec::new(),
                        "positive book_number, chapter_number and scene_order",
                        format!(
                            "book={}, chapter={}, scene_order={:?}",
                            inciting.book_number, inciting.chapter_number, inciting.scene_order
                        ),
                        "Use 1-based positive coordinates with an exact scene_order for the inciting Scene.",
                    ));
                }
            }
        }
        match sequence.climax_scene.as_ref() {
            None => findings.push(planning_finding(
                "sequence_missing_climax",
                PlanningFindingSeverity::Blocking,
                format!("{base}/climax_scene"),
                Vec::new(),
                "the Sequence climax Scene placement",
                "no climax",
                "Name the strongest Scene the Sequence ends on; a dilemma is required there.",
            )),
            Some(climax) => {
                if !climax.has_positive_coordinates() || climax.scene_order.is_none() {
                    findings.push(planning_finding(
                        "sequence_invalid_placement",
                        PlanningFindingSeverity::Blocking,
                        format!("{base}/climax_scene"),
                        Vec::new(),
                        "positive book_number, chapter_number and scene_order",
                        format!(
                            "book={}, chapter={}, scene_order={:?}",
                            climax.book_number, climax.chapter_number, climax.scene_order
                        ),
                        "Use 1-based positive coordinates with an exact scene_order for the climax.",
                    ));
                }
                if !placement_is_member(climax, &sequence.member_scenes) {
                    findings.push(planning_finding(
                        "sequence_climax_not_member",
                        PlanningFindingSeverity::Blocking,
                        format!("{base}/climax_scene"),
                        Vec::new(),
                        "a climax that is one of member_scenes",
                        "the climax is not a member of this Sequence",
                        "Add the climax to member_scenes, or move the climax to a member Scene.",
                    ));
                }
                let climax_is_last = sequence
                    .member_scenes
                    .last()
                    .is_some_and(|last| same_scene_position(last, climax));
                if !sequence.member_scenes.is_empty() && !climax_is_last {
                    findings.push(planning_finding(
                        "sequence_climax_not_last",
                        PlanningFindingSeverity::Blocking,
                        format!("{base}/climax_scene"),
                        Vec::new(),
                        "the climax as the last member in story order",
                        format!(
                            "the climax is book {} chapter {} scene {:?}, not the last member",
                            climax.book_number, climax.chapter_number, climax.scene_order
                        ),
                        "Make the strongest Scene the last member of the Sequence; a Sequence ends on its climax.",
                    ));
                }
            }
        }
        if let (Some(inciting), Some(climax)) = (
            sequence.inciting_scene.as_ref(),
            sequence.climax_scene.as_ref(),
        ) && inciting.cmp_story_order(climax) == std::cmp::Ordering::Greater
        {
            findings.push(planning_finding(
                "sequence_inciting_after_climax",
                PlanningFindingSeverity::Blocking,
                format!("{base}/inciting_scene"),
                Vec::new(),
                "an inciting Scene at or before the climax",
                format!(
                    "inciting book {} chapter {} comes after climax book {} chapter {}",
                    inciting.book_number,
                    inciting.chapter_number,
                    climax.book_number,
                    climax.chapter_number
                ),
                "Move the inciting Scene to or before the climax.",
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluated sets
// ---------------------------------------------------------------------------

/// Merge applied, staged-ancestor and current-artifact Sequences by key.
pub(super) fn evaluated_sequences_in_conn(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    scope: &PlanningScope,
) -> Result<Vec<PlannedSequence>> {
    let mut evaluated =
        accepted_planning_sequences_in_conn(conn, &scope.project_id, &scope.branch_id)?;
    for ancestor in collect_staged_ancestor_proposals_in_conn(conn, input)? {
        if ancestor.artifact.level != PlanningLevel::Chapter {
            continue;
        }
        evaluated = merge_planned_sequences(&evaluated, &ancestor.artifact.schedule.sequences);
    }
    if input.level == PlanningLevel::Chapter {
        evaluated = merge_planned_sequences(&evaluated, &input.schedule.sequences);
    }
    Ok(evaluated)
}

/// The accepted chapter plans visible from the branch lineage. A nearer branch
/// shadows a farther one for the same chapter.
fn accepted_chapter_plan_scenes_in_conn(
    conn: &rusqlite::Connection,
    project_id: &str,
    branch_id: &str,
) -> Result<Vec<(i32, i32, Vec<PlannedScene>)>> {
    let lineage = resolve_branch_lineage_in_conn(conn, Some(project_id), branch_id)?;
    let mut by_chapter: BTreeMap<(i32, i32), Vec<PlannedScene>> = BTreeMap::new();
    let mut stmt = conn.prepare(
        "SELECT book_number, chapter_number, scenes FROM chapter_plan \
         WHERE project_id=?1 AND branch_id=?2",
    )?;
    for lineage_branch in lineage {
        let rows = stmt
            .query_map(rusqlite::params![project_id, lineage_branch], |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (book_number, chapter_number, scenes_json) in rows {
            if by_chapter.contains_key(&(book_number, chapter_number)) {
                continue;
            }
            let scenes: Vec<PlannedScene> = serde_json::from_str(&scenes_json)?;
            by_chapter.insert((book_number, chapter_number), scenes);
        }
    }
    Ok(by_chapter
        .into_iter()
        .map(|((book, chapter), scenes)| (book, chapter, scenes))
        .collect())
}

/// Build the ordered planned-Scene list: accepted chapter plans, then staged
/// ancestors (creation order), then the current artifact; a later plan for the
/// same chapter replaces the earlier one.
fn evaluated_scene_values_in_conn(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    scope: &PlanningScope,
) -> Result<Vec<SceneValueEntry>> {
    let mut by_chapter: BTreeMap<(i32, i32), Vec<SceneValueEntry>> = BTreeMap::new();
    for (book_number, chapter_number, scenes) in
        accepted_chapter_plan_scenes_in_conn(conn, &scope.project_id, &scope.branch_id)?
    {
        let repair_hint = accepted_repair_hint(book_number, chapter_number);
        by_chapter.insert(
            (book_number, chapter_number),
            scenes
                .iter()
                .map(|scene| {
                    SceneValueEntry::from_planned_scene(
                        book_number,
                        chapter_number,
                        scene,
                        format!(
                            "/accepted_scenes/{book_number}/{chapter_number}/{}",
                            scene.scene_order
                        ),
                        repair_hint.clone(),
                    )
                })
                .collect(),
        );
    }
    for ancestor in collect_staged_ancestor_proposals_in_conn(conn, input)? {
        if !matches!(
            ancestor.artifact.level,
            PlanningLevel::Chapter | PlanningLevel::Scene
        ) {
            continue;
        }
        for item in &ancestor.artifact.operations {
            let PlanningOperation::PlanChapter(plan) = &item.operation else {
                continue;
            };
            let repair_hint = format!(
                "Repair the staged plan for book {} chapter {} (proposal {}).",
                plan.book_number, plan.chapter_number, ancestor.id
            );
            by_chapter.insert(
                (plan.book_number, plan.chapter_number),
                plan.scenes
                    .iter()
                    .map(|scene| {
                        SceneValueEntry::from_planning_scene(
                            plan.book_number,
                            plan.chapter_number,
                            scene,
                            format!(
                                "/staged_scenes/{}/{}/{}",
                                plan.book_number, plan.chapter_number, scene.scene_order
                            ),
                            repair_hint.clone(),
                        )
                    })
                    .collect(),
            );
        }
    }
    for (index, item) in input.operations.iter().enumerate() {
        let PlanningOperation::PlanChapter(plan) = &item.operation else {
            continue;
        };
        by_chapter.insert(
            (plan.book_number, plan.chapter_number),
            plan.scenes
                .iter()
                .enumerate()
                .map(|(scene_index, scene)| {
                    SceneValueEntry::from_planning_scene(
                        plan.book_number,
                        plan.chapter_number,
                        scene,
                        format!("/operations/{index}/operation/input/scenes/{scene_index}"),
                        "Repair this scene batch.".to_string(),
                    )
                })
                .collect(),
        );
    }
    let mut scenes: Vec<SceneValueEntry> = by_chapter.into_values().flatten().collect();
    scenes.sort_by(|left, right| {
        (left.book_number, left.chapter_number, left.scene_order).cmp(&(
            right.book_number,
            right.chapter_number,
            right.scene_order,
        ))
    });
    Ok(scenes)
}

/// The same evaluated planned-Scene list, projected to the fields the promise
/// and Key Fact gates read. It is built from one merge, so the value chain and
/// the promise gates can never disagree about which plan is in force.
pub(super) fn evaluated_gate_scenes_in_conn(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    scope: &PlanningScope,
) -> Result<Vec<SceneGateEntry>> {
    Ok(evaluated_scene_values_in_conn(conn, input, scope)?
        .iter()
        .map(SceneValueEntry::gate_entry)
        .collect())
}

fn accepted_repair_hint(book_number: i32, chapter_number: i32) -> String {
    format!(
        "Stage a replacement Chapter or Scene plan for book {book_number} chapter {chapter_number}; \
         the accepted chapter plan is the plan to repair."
    )
}

/// The Character Design contradictions the planned range may test: accepted
/// rows plus designs declared by staged CastWorld ancestors (a batch acceptance
/// must be able to reference the design it is about to create).
fn evaluated_design_contradictions_in_conn(
    conn: &rusqlite::Connection,
    input: &StagePlanningInput,
    scope: &PlanningScope,
) -> Result<Vec<DesignContradictions>> {
    let mut designs: BTreeMap<String, DesignContradictions> = BTreeMap::new();
    let mut stmt = conn
        .prepare("SELECT character_id, contradictions FROM character_design WHERE project_id=?1")?;
    let rows = stmt
        .query_map([&scope.project_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (character_id, contradictions_json) in rows {
        let contradictions: Vec<String> = serde_json::from_str(&contradictions_json)?;
        designs.insert(
            character_id.clone(),
            DesignContradictions {
                path: format!("/character_designs/{character_id}"),
                key: character_id,
                contradictions,
            },
        );
    }
    for ancestor in collect_staged_ancestor_proposals_in_conn(conn, input)? {
        if ancestor.artifact.level != PlanningLevel::CastWorld {
            continue;
        }
        for (index, item) in ancestor.artifact.operations.iter().enumerate() {
            let PlanningOperation::CreateCharacterDesign(design) = &item.operation else {
                continue;
            };
            let key = match &design.character_id {
                PlanningRef::ProposalRef(reference) => proposal_character_reference(reference),
                PlanningRef::ExistingRecordId(id) => id.clone(),
            };
            designs.insert(
                key.clone(),
                DesignContradictions {
                    key,
                    contradictions: design.contradictions.clone(),
                    path: format!("/staged_character_designs/{}/{}", ancestor.id, index),
                },
            );
        }
    }
    Ok(designs.into_values().collect())
}

/// Normalize a planning reference to the key used by the design lookup.
fn normalized_ref(reference: &PlanningRef) -> String {
    match reference {
        PlanningRef::ExistingRecordId(id) => id.clone(),
        PlanningRef::ProposalRef(key) => proposal_character_reference(key),
    }
}

/// Resolve a scene dilemma's character key: an applied character reference is
/// rewritten to the record id via the session receipts, so accepted designs
/// (keyed by record id) and staged designs (keyed by proposal reference) are
/// both findable.
fn resolved_dilemma_key(dilemma: &SceneDilemma, receipts: &BTreeMap<String, String>) -> String {
    if let Some(reference) = proposal_output_ref(&dilemma.character_id)
        && let Some(record_id) = receipts.get(reference)
    {
        return record_id.clone();
    }
    dilemma.character_id.clone()
}

/// JSON-pointer path suffix for one axis of a Scene.
fn axis_suffix(primary: bool, secondary_index: usize) -> String {
    if primary {
        "primary_value_axis".to_string()
    } else {
        format!("secondary_value_axes/{secondary_index}")
    }
}

/// All axes of a Scene with their path suffix, primary first.
fn scene_axes(scene: &SceneValueEntry) -> Vec<(String, &SceneValueAxis)> {
    let mut axes = Vec::new();
    if let Some(primary) = scene.primary.as_ref() {
        axes.push((axis_suffix(true, 0), primary));
    }
    for (index, secondary) in scene.secondary.iter().enumerate() {
        axes.push((axis_suffix(false, index), secondary));
    }
    axes
}

// ---------------------------------------------------------------------------
// Blocking rules
// ---------------------------------------------------------------------------

fn validate_scene_value_shapes(scenes: &[SceneValueEntry], findings: &mut Vec<PlanningFinding>) {
    for scene in scenes {
        if scene.primary.is_none() {
            findings.push(planning_finding(
                "scene_missing_primary_value_axis",
                PlanningFindingSeverity::Blocking,
                format!("{}/primary_value_axis", scene.path),
                Vec::new(),
                "one primary Value Axis with an opening and closing Charge",
                "no primary Value Axis",
                format!(
                    "{} Give the Scene a primary Value Axis; a Scene with no axis cannot turn anything.",
                    scene.repair_hint
                ),
            ));
        }
        let dilemma_text = scene.dilemma.as_ref().map(|dilemma| {
            (
                dilemma.character_id.as_str(),
                dilemma.contradiction.as_str(),
            )
        });
        // One pass over the typed shape rules; each error maps to exactly one
        // finding, so no shape is reported twice.
        for error in scene_value_errors(
            scene.primary.as_ref(),
            &scene.secondary,
            scene.risk,
            scene.gap.as_ref(),
            dilemma_text,
        ) {
            use spindle_core::scene_value::SceneValueError;
            let (code, path, expected) = match &error {
                SceneValueError::TooManySecondaryAxes { .. } => (
                    "scene_too_many_secondary_axes",
                    format!("{}/secondary_value_axes", scene.path),
                    "at most two secondary Value Axes",
                ),
                SceneValueError::EmptyValueAxisName { secondary_index } => (
                    "scene_empty_value_axis_name",
                    format!(
                        "{}/{}/axis",
                        scene.path,
                        axis_suffix(secondary_index.is_none(), secondary_index.unwrap_or(0))
                    ),
                    "a non-empty Value Axis name",
                ),
                SceneValueError::DuplicateValueAxisName {
                    secondary_index, ..
                } => (
                    "scene_duplicate_value_axis",
                    format!(
                        "{}/{}/axis",
                        scene.path,
                        axis_suffix(secondary_index.is_none(), secondary_index.unwrap_or(0))
                    ),
                    "unique Value Axis names within the Scene",
                ),
                SceneValueError::RiskOutOfRange { .. } => (
                    "scene_invalid_risk",
                    format!("{}/risk", scene.path),
                    "risk in 1..=5",
                ),
                SceneValueError::BlankGap => (
                    "scene_invalid_gap",
                    format!("{}/gap", scene.path),
                    "a Gap with non-blank expected and actual text",
                ),
                SceneValueError::BlankDilemma => (
                    "scene_invalid_dilemma",
                    format!("{}/dilemma", scene.path),
                    "a dilemma with a character and a contradiction",
                ),
            };
            findings.push(planning_finding(
                code,
                PlanningFindingSeverity::Blocking,
                path,
                Vec::new(),
                expected.to_string(),
                error.to_string(),
                format!("{} {error}.", scene.repair_hint),
            ));
        }
    }
}

/// Structured Beat rules for a McKee Scene: Beats are required, exactly one is
/// of kind `turn`, and `turning_beat` points at it.
///
/// The Turning Beat is a 1-based position in `beats`; a Scene therefore names no
/// Beat order of its own. `beat_structure` strings are ignored here: they stay
/// readable for every project, but on a McKee project they are never a
/// substitute for structured Beats.
fn validate_scene_beats(scenes: &[SceneValueEntry], findings: &mut Vec<PlanningFinding>) {
    for scene in scenes {
        for error in scene_beat_errors(&scene.beat_kinds(), scene.turning_beat) {
            let (code, path, expected) = match &error {
                SceneBeatError::NoBeats => (
                    "scene_beats_missing",
                    format!("{}/beats", scene.path),
                    "at least one structured Beat",
                ),
                SceneBeatError::MissingTurningBeat => (
                    "scene_missing_turning_beat",
                    format!("{}/turning_beat", scene.path),
                    "the 1-based position of the Scene's Turning Beat",
                ),
                SceneBeatError::TurningBeatOutOfRange { .. } => (
                    "scene_turning_beat_out_of_range",
                    format!("{}/turning_beat", scene.path),
                    "a 1-based position of an existing Beat",
                ),
                SceneBeatError::TurningBeatNotTurn { .. } => (
                    "scene_turning_beat_not_turn",
                    format!("{}/turning_beat", scene.path),
                    "a Turning Beat of kind 'turn'",
                ),
                SceneBeatError::MultipleTurnBeats { .. } => (
                    "scene_multiple_turn_beats",
                    format!("{}/beats", scene.path),
                    "exactly one Beat of kind 'turn'",
                ),
            };
            findings.push(planning_finding(
                code,
                PlanningFindingSeverity::Blocking,
                path,
                Vec::new(),
                expected.to_string(),
                error.to_string(),
                format!(
                    "{} Design the Scene's Beats as action/reaction and exactly one turn, then point turning_beat at the turn.",
                    scene.repair_hint
                ),
            ));
        }
    }
}

fn validate_non_events(scenes: &[SceneValueEntry], findings: &mut Vec<PlanningFinding>) {
    for scene in scenes {
        let Some(primary) = scene.primary.as_ref() else {
            continue;
        };
        if primary.turns() {
            continue;
        }
        findings.push(planning_finding(
            "scene_non_event",
            PlanningFindingSeverity::Blocking,
            format!("{}/primary_value_axis", scene.path),
            Vec::new(),
            "a primary axis whose opening and closing Charge differ",
            format!(
                "the primary axis '{}' opens and closes at {} / {}",
                primary.axis,
                primary.opening.sign(),
                primary.opening.depth()
            ),
            format!(
                "{} Turn the primary Value Axis: a Scene that changes nothing is a non-event.",
                scene.repair_hint
            ),
        ));
    }
}

/// One broken value-chain link: the Scene whose opening Charge no longer
/// continues the axis from the preceding Scene's closing Charge.
struct ValueChainBreak {
    scene_index: usize,
    previous_index: usize,
    axis: String,
    expected: spindle_core::charge::Charge,
    actual: spindle_core::charge::Charge,
    path: String,
}

/// The pure broken-chain walk. It is shared by the planning gate (mapped into
/// stage-recordable findings) and the McKee amendment cascade (mapped into
/// actionable repairs), so the two can never disagree about the chain rule.
fn value_chain_breaks(scenes: &[SceneValueEntry]) -> Vec<ValueChainBreak> {
    // axis name -> (closing Charge left by the preceding occurrence, scene index)
    let mut last_closing: BTreeMap<String, (spindle_core::charge::Charge, usize)> = BTreeMap::new();
    let mut breaks = Vec::new();
    for (index, scene) in scenes.iter().enumerate() {
        for (suffix, axis) in scene_axes(scene) {
            let name = axis.axis.trim().to_string();
            if name.is_empty() {
                continue;
            }
            if let Some((expected, previous_index)) = last_closing.get(&name)
                && *expected != axis.opening
            {
                breaks.push(ValueChainBreak {
                    scene_index: index,
                    previous_index: *previous_index,
                    axis: name.clone(),
                    expected: *expected,
                    actual: axis.opening,
                    path: format!("{}/{suffix}/opening", scene.path),
                });
            }
            last_closing.insert(name, (axis.closing, index));
        }
    }
    breaks
}

fn validate_value_chain(scenes: &[SceneValueEntry], findings: &mut Vec<PlanningFinding>) {
    for chain_break in value_chain_breaks(scenes) {
        let scene = &scenes[chain_break.scene_index];
        let previous = &scenes[chain_break.previous_index];
        findings.push(planning_finding(
            "value_chain_broken",
            PlanningFindingSeverity::Blocking,
            chain_break.path,
            Vec::new(),
            format!(
                "opening Charge {} / {} — the closing Charge left on axis '{}' by book {} chapter {} scene {}",
                chain_break.expected.sign(),
                chain_break.expected.depth(),
                chain_break.axis,
                previous.book_number,
                previous.chapter_number,
                previous.scene_order
            ),
            format!(
                "opening Charge {} / {}",
                chain_break.actual.sign(),
                chain_break.actual.depth()
            ),
            format!(
                "{} Continue axis '{}' from its last closing Charge, or change that earlier closing Charge first.",
                scene.repair_hint, chain_break.axis
            ),
        ));
    }
}

/// One blocking finding the amendment cascade reports against one planned Scene
/// (issue #34). `axis`/`expected_opening` are populated for a broken value-chain
/// link so a `scene_replace` reconciliation can repair the opening Charge with
/// the exact Charge the chain requires.
#[derive(Debug, Clone)]
pub(crate) struct CascadeSceneFinding {
    pub book_number: i32,
    pub chapter_number: i32,
    pub scene_order: i32,
    pub code: String,
    pub message: String,
    pub axis: Option<String>,
    pub expected_opening: Option<spindle_core::charge::Charge>,
}

/// Re-run the SAME value-plan rules the planning gate uses (scene value shapes,
/// non-events and the value chain) over the accepted plans of the whole branch,
/// and return the blocking findings per Scene.
///
/// It is deliberately the shared rule functions, not a copy: a cascade can
/// never accept something the planning gate would reject, or vice versa. The
/// caller decides which Scenes are in scope (later planned, undrafted only).
pub(crate) fn cascade_value_findings(
    scenes: &[(i32, i32, spindle_core::models::PlannedScene)],
) -> Vec<CascadeSceneFinding> {
    let entries: Vec<SceneValueEntry> = scenes
        .iter()
        .map(|(book_number, chapter_number, scene)| {
            SceneValueEntry::from_planned_scene(
                *book_number,
                *chapter_number,
                scene,
                format!(
                    "/accepted_scenes/{book_number}/{chapter_number}/{}",
                    scene.scene_order
                ),
                format!(
                    "Stage a replacement Chapter or Scene plan for book {book_number} chapter \
                     {chapter_number}."
                ),
            )
        })
        .collect();

    // A finding's path starts with the owning Scene's base path; mapping through
    // the base path keeps the rule output unchanged (the planning gate sees the
    // exact same shape).
    let mut bases: Vec<(String, usize)> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.path.clone(), index))
        .collect();
    // Longest first so a chapter/order prefix can never shadow a longer one.
    bases.sort_by_key(|left| std::cmp::Reverse(left.0.len()));
    let owner = |path: &str| {
        bases.iter().find_map(|(base, index)| {
            (path == base || path.starts_with(&format!("{base}/"))).then_some(*index)
        })
    };

    let mut findings = Vec::new();
    validate_scene_value_shapes(&entries, &mut findings);
    validate_non_events(&entries, &mut findings);
    let breaks = value_chain_breaks(&entries);
    let mut break_details: BTreeMap<String, (String, spindle_core::charge::Charge)> = breaks
        .into_iter()
        .map(|chain_break| (chain_break.path, (chain_break.axis, chain_break.expected)))
        .collect();
    validate_value_chain(&entries, &mut findings);

    findings
        .into_iter()
        .filter(|finding| finding.severity == PlanningFindingSeverity::Blocking)
        .filter_map(|finding| {
            let index = owner(&finding.path)?;
            let entry = &entries[index];
            let (axis, expected_opening) = match break_details.remove(&finding.path) {
                Some((axis, expected)) => (Some(axis), Some(expected)),
                None => (None, None),
            };
            let code = finding.code.clone();
            Some(CascadeSceneFinding {
                book_number: entry.book_number,
                chapter_number: entry.chapter_number,
                scene_order: entry.scene_order,
                code: code.clone(),
                message: format!(
                    "{}: expected {}, got {} ({})",
                    finding.path, finding.expected, finding.actual, code
                ),
                axis,
                expected_opening,
            })
        })
        .collect()
}

fn validate_sequence_membership(
    scenes: &[SceneValueEntry],
    sequences: &[PlannedSequence],
    findings: &mut Vec<PlanningFinding>,
) {
    let members: Vec<&PlanningStoryPlacement> = sequences
        .iter()
        .flat_map(|sequence| sequence.member_scenes.iter())
        .collect();
    for scene in scenes {
        let placement = scene.placement();
        if members
            .iter()
            .any(|member| same_scene_position(member, &placement))
        {
            continue;
        }
        findings.push(planning_finding(
            "scene_no_sequence",
            PlanningFindingSeverity::Blocking,
            format!("{}/scene_order", scene.path),
            Vec::new(),
            "membership in one Sequence's member_scenes",
            "no Sequence lists this planned Scene",
            format!(
                "{} Add the Scene to a Sequence (a Chapter-level schedule.sequences entry) so every Scene sits in the structure.",
                scene.repair_hint
            ),
        ));
    }
}

fn validate_inaction_costs(scenes: &[SceneValueEntry], findings: &mut Vec<PlanningFinding>) {
    for scene in scenes {
        if scene
            .inaction_cost
            .as_deref()
            .is_some_and(|cost| !cost.trim().is_empty())
        {
            continue;
        }
        findings.push(planning_finding(
            "scene_missing_inaction_cost",
            PlanningFindingSeverity::Blocking,
            format!("{}/inaction_cost", scene.path),
            Vec::new(),
            "what a character loses by doing nothing in this Scene",
            "empty or missing Inaction Cost",
            format!(
                "{} State the Inaction Cost; a Scene a character could survive by doing nothing has too little pressure.",
                scene.repair_hint
            ),
        ));
    }
}

fn validate_climax_dilemmas(
    scenes: &[SceneValueEntry],
    sequences: &[PlannedSequence],
    acts: &[PlannedAct],
    findings: &mut Vec<PlanningFinding>,
) {
    let sequence_climaxes: BTreeSet<(i32, i32, i32)> = sequences
        .iter()
        .filter_map(|sequence| sequence.climax_scene.as_ref())
        .filter_map(|climax| {
            climax
                .scene_order
                .map(|order| (climax.book_number, climax.chapter_number, order))
        })
        .collect();
    let act_climaxes: BTreeSet<(i32, i32, i32)> = acts
        .iter()
        .filter_map(|act| act.climax.as_ref())
        .filter_map(|climax| {
            climax
                .scene_order
                .map(|order| (climax.book_number, climax.chapter_number, order))
        })
        .collect();
    for scene in scenes {
        let position = (scene.book_number, scene.chapter_number, scene.scene_order);
        let mut reasons = Vec::new();
        if sequence_climaxes.contains(&position) {
            reasons.push("Sequence climax");
        }
        if act_climaxes.contains(&position) {
            reasons.push("Act climax");
        }
        if reasons.is_empty() || scene.dilemma.is_some() {
            continue;
        }
        let reason = reasons.join(" and ");
        findings.push(planning_finding(
            "scene_climax_missing_dilemma",
            PlanningFindingSeverity::Blocking,
            format!("{}/dilemma", scene.path),
            Vec::new(),
            format!("a dilemma on this {reason} Scene"),
            "no dilemma",
            format!(
                "{} Add a dilemma naming the tested character and contradiction on the {reason} Scene.",
                scene.repair_hint
            ),
        ));
    }
}

fn validate_dilemma_contradictions(
    scenes: &[SceneValueEntry],
    designs: &[DesignContradictions],
    receipts: &BTreeMap<String, String>,
    findings: &mut Vec<PlanningFinding>,
) {
    let by_key: BTreeMap<&str, &DesignContradictions> = designs
        .iter()
        .map(|design| (design.key.as_str(), design))
        .collect();
    for scene in scenes {
        let Some(dilemma) = scene.dilemma.as_ref() else {
            continue;
        };
        let contradiction = dilemma.contradiction.trim();
        if contradiction.is_empty() {
            continue;
        }
        let key = resolved_dilemma_key(dilemma, receipts);
        let Some(design) = by_key.get(key.as_str()) else {
            findings.push(planning_finding(
                "scene_dilemma_unknown_contradiction",
                PlanningFindingSeverity::Blocking,
                format!("{}/dilemma/contradiction", scene.path),
                vec![key.clone()],
                "a Character Design for the tested character",
                format!("no accepted Character Design for '{key}'"),
                format!(
                    "{} Name a character with an accepted Character Design and one of its contradictions.",
                    scene.repair_hint
                ),
            ));
            continue;
        };
        let matches = design
            .contradictions
            .iter()
            .any(|candidate| candidate.trim() == contradiction);
        if matches {
            continue;
        }
        findings.push(planning_finding(
            "scene_dilemma_unknown_contradiction",
            PlanningFindingSeverity::Blocking,
            format!("{}/dilemma/contradiction", scene.path),
            vec![key.clone()],
            format!(
                "one of the contradictions designed for '{key}': {}",
                design
                    .contradictions
                    .iter()
                    .map(|c| format!("'{c}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            format!("'{contradiction}'"),
            format!(
                "{} Use a contradiction that exists in that Character Design (trim-exact match).",
                scene.repair_hint
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// Advisory rules
// ---------------------------------------------------------------------------

fn validate_same_direction_runs(scenes: &[SceneValueEntry], findings: &mut Vec<PlanningFinding>) {
    let mut runs: BTreeMap<String, (spindle_core::scene_value::ValueTurnDirection, usize)> =
        BTreeMap::new();
    for scene in scenes {
        let mut present: BTreeSet<String> = BTreeSet::new();
        for (suffix, axis) in scene_axes(scene) {
            let name = axis.axis.trim().to_string();
            if name.is_empty() {
                continue;
            }
            present.insert(name.clone());
            let Some(direction) = axis.turn_direction() else {
                runs.remove(&name);
                continue;
            };
            let entry = runs.entry(name.clone()).or_insert((direction, 0));
            if entry.0 == direction {
                entry.1 += 1;
            } else {
                *entry = (direction, 1);
            }
            if entry.1 == 3 {
                findings.push(planning_finding(
                    "value_axis_same_direction_run",
                    PlanningFindingSeverity::Advisory,
                    format!("{}/{suffix}", scene.path),
                    Vec::new(),
                    "a direction change before the third consecutive same-direction turn",
                    format!("axis '{name}' turns {direction} in 3 consecutive planned Scenes"),
                    format!(
                        "{} Vary the direction or depth of axis '{name}' before diminishing returns; this is advisory.",
                        scene.repair_hint
                    ),
                ));
            }
        }
        runs.retain(|name, _| present.contains(name));
    }
}

fn validate_sequence_risk(
    scenes: &[SceneValueEntry],
    sequences: &[PlannedSequence],
    findings: &mut Vec<PlanningFinding>,
) {
    let by_position: BTreeMap<(i32, i32, i32), &SceneValueEntry> = scenes
        .iter()
        .map(|scene| {
            (
                (scene.book_number, scene.chapter_number, scene.scene_order),
                scene,
            )
        })
        .collect();
    for sequence in sequences {
        let mut ordered: Vec<&SceneValueEntry> = sequence
            .member_scenes
            .iter()
            .filter_map(|member| {
                member.scene_order.and_then(|order| {
                    by_position
                        .get(&(member.book_number, member.chapter_number, order))
                        .copied()
                })
            })
            .collect();
        ordered.sort_by(|left, right| {
            (left.book_number, left.chapter_number, left.scene_order).cmp(&(
                right.book_number,
                right.chapter_number,
                right.scene_order,
            ))
        });
        let mut previous: Option<&SceneValueEntry> = None;
        for scene in ordered {
            if let (Some(previous_scene), Some(previous_risk), Some(risk)) =
                (previous, previous.and_then(|entry| entry.risk), scene.risk)
                && risk < previous_risk
            {
                findings.push(planning_finding(
                    "sequence_risk_drop",
                    PlanningFindingSeverity::Advisory,
                    format!("{}/risk", scene.path),
                    vec![sequence.sequence_key.clone()],
                    format!(
                        "risk at or above {} from the previous Scene in Sequence '{}'",
                        previous_risk, sequence.sequence_key
                    ),
                    format!(
                        "risk {risk} after risk {previous_risk} (book {} chapter {} scene {})",
                        previous_scene.book_number,
                        previous_scene.chapter_number,
                        previous_scene.scene_order
                    ),
                    format!(
                        "{} Raise the risk or reorder the Sequence so it escalates; this is advisory.",
                        scene.repair_hint
                    ),
                ));
            }
            previous = Some(scene);
        }
    }
}

/// A chapter "lands on a Value Turn" when its last Scene leaves the primary
/// Value Axis turned relative to the Charge the chapter's **first** Scene opened
/// on that axis. Two conditions emit the advisory:
///
/// 1. the literal flat-primary case — the last Scene's primary axis opens and
///    closes at the same Charge (which also records the blocking non-event), and
/// 2. the net-zero chapter — the last Scene's primary closing Charge equals the
///    first Scene's primary opening Charge, so the chapter returns the axis to
///    where it started even though individual Scenes turn.
fn validate_chapter_endings(scenes: &[SceneValueEntry], findings: &mut Vec<PlanningFinding>) {
    let mut first_by_chapter: BTreeMap<(i32, i32), &SceneValueEntry> = BTreeMap::new();
    let mut last_by_chapter: BTreeMap<(i32, i32), &SceneValueEntry> = BTreeMap::new();
    for scene in scenes {
        let key = (scene.book_number, scene.chapter_number);
        first_by_chapter.entry(key).or_insert(scene);
        match last_by_chapter.get(&key) {
            Some(previous) if previous.scene_order >= scene.scene_order => {}
            _ => {
                last_by_chapter.insert(key, scene);
            }
        }
    }
    for ((book_number, chapter_number), scene) in last_by_chapter {
        let first = first_by_chapter[&(book_number, chapter_number)];
        let flat_primary = scene.primary.as_ref().is_none_or(|axis| !axis.turns());
        let net_zero = match (first.primary.as_ref(), scene.primary.as_ref()) {
            (Some(first_axis), Some(last_axis))
                if !first_axis.axis.trim().is_empty()
                    && first_axis.axis.trim() == last_axis.axis.trim() =>
            {
                last_axis.closing == first_axis.opening
            }
            _ => false,
        };
        if !flat_primary && !net_zero {
            continue;
        }
        let actual = if flat_primary {
            match scene.primary.as_ref() {
                Some(axis) => format!(
                    "book {book_number} chapter {chapter_number} scene {} leaves primary axis '{}' at {} / {} where it opened",
                    scene.scene_order,
                    axis.axis,
                    axis.opening.sign(),
                    axis.opening.depth()
                ),
                None => format!(
                    "book {book_number} chapter {chapter_number} scene {} has no primary Value Axis",
                    scene.scene_order
                ),
            }
        } else {
            let last_axis = scene.primary.as_ref().expect("net zero implies axes");
            format!(
                "book {book_number} chapter {chapter_number} closes primary axis '{}' at {} / {}, the Charge scene {} opened with (net-zero chapter)",
                last_axis.axis,
                last_axis.closing.sign(),
                last_axis.closing.depth(),
                first.scene_order
            )
        };
        findings.push(planning_finding(
            "chapter_ending_no_value_turn",
            PlanningFindingSeverity::Advisory,
            format!("{}/primary_value_axis", scene.path),
            Vec::new(),
            "a chapter whose last Scene leaves its primary Value Axis turned relative to the chapter's first Scene",
            actual,
            format!(
                "{} End the chapter on a Value Turn so the ending pulls the reader on; this is advisory.",
                scene.repair_hint
            ),
        ));
    }
}

fn validate_untested_contradictions(
    scenes: &[SceneValueEntry],
    designs: &[DesignContradictions],
    receipts: &BTreeMap<String, String>,
    findings: &mut Vec<PlanningFinding>,
) {
    let matches_design = |dilemma: &SceneDilemma, design_key: &str| -> bool {
        if dilemma.character_id == design_key {
            return true;
        }
        proposal_output_ref(&dilemma.character_id)
            .and_then(|reference| receipts.get(reference))
            .is_some_and(|record_id| record_id == design_key)
    };
    for design in designs {
        for (index, contradiction) in design.contradictions.iter().enumerate() {
            let text = contradiction.trim();
            if text.is_empty() {
                continue;
            }
            let tested = scenes.iter().any(|scene| {
                scene.dilemma.as_ref().is_some_and(|dilemma| {
                    matches_design(dilemma, &design.key) && dilemma.contradiction.trim() == text
                })
            });
            if tested {
                continue;
            }
            findings.push(planning_finding(
                "character_contradiction_untested",
                PlanningFindingSeverity::Advisory,
                format!("{}/contradictions/{index}", design.path),
                vec![design.key.clone()],
                "a planned dilemma that tests this character contradiction",
                format!("'{text}' is never named by a dilemma in the planned range"),
                "Name this contradiction in a planned Scene dilemma, or accept that the design's depth stays unused; this is advisory.",
            ));
        }
    }
}
