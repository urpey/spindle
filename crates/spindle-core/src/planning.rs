//! Initial-planning contracts. These never authorize in-run amendments.
use crate::character_design::CharacterDesignView;
use crate::charge::Charge;
use crate::key_fact::KeyFactKind;
use crate::models::*;
use crate::plot_line::SubplotJob;
use crate::scene_value::{SceneGap, SceneValueAxis};
use crate::story_spine::{ControllingIdea, StorySpine, StorySpineEnding};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum PlanningRef {
    ExistingRecordId(String),
    ProposalRef(String),
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum PlanningLevel {
    Contract,
    CastWorld,
    SeriesArc,
    Book,
    Chapter,
    Scene,
}

impl PlanningLevel {
    pub const fn all() -> [Self; 6] {
        [
            Self::Contract,
            Self::CastWorld,
            Self::SeriesArc,
            Self::Book,
            Self::Chapter,
            Self::Scene,
        ]
    }

    pub const fn route_task(self) -> &'static str {
        match self {
            Self::Contract | Self::CastWorld | Self::SeriesArc => "plan",
            Self::Book | Self::Chapter | Self::Scene => "outline",
        }
    }

    /// The original (v1) prompt for this level.
    ///
    /// A session that pins version 1 keeps this prompt byte-identically; a
    /// session that pins version 2 selects [`Self::prompt_id_for_version`].
    pub const fn prompt_id(self) -> PlanningPromptId {
        match self {
            Self::Contract => PlanningPromptId::PlanningContractV1,
            Self::CastWorld => PlanningPromptId::PlanningSeedV1,
            Self::SeriesArc => PlanningPromptId::PlanningSeriesArcV1,
            Self::Book => PlanningPromptId::OutlineBookV1,
            Self::Chapter => PlanningPromptId::OutlineChaptersV1,
            Self::Scene => PlanningPromptId::OutlineManifestV1,
        }
    }

    /// The prompt for this level at a pinned session prompt version.
    ///
    /// Version 2 is the McKee guidance family; every other version (including
    /// the legacy default 1) is the original family. The version is pinned once
    /// on the session at creation, so a later advance never re-derives it from
    /// a scope that changed in between.
    pub const fn prompt_id_for_version(self, prompt_version: u32) -> PlanningPromptId {
        match (prompt_version >= 2, self) {
            (true, Self::Contract) => PlanningPromptId::PlanningContractV2,
            (true, Self::CastWorld) => PlanningPromptId::PlanningSeedV2,
            (true, Self::SeriesArc) => PlanningPromptId::PlanningSeriesArcV2,
            (true, Self::Book) => PlanningPromptId::OutlineBookV2,
            (true, Self::Chapter) => PlanningPromptId::OutlineChaptersV2,
            (true, Self::Scene) => PlanningPromptId::OutlineManifestV2,
            (false, level) => level.prompt_id(),
        }
    }

    /// Whether this level is a root level.
    ///
    /// Root levels (Contract, CastWorld, SeriesArc, Book) always require a human
    /// decision and can never be delegated, at any policy version.
    pub const fn is_root(self) -> bool {
        matches!(
            self,
            Self::Contract | Self::CastWorld | Self::SeriesArc | Self::Book
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanningPromptId {
    #[serde(rename = "planning_contract_v1")]
    PlanningContractV1,
    #[serde(rename = "planning_seed_v1")]
    PlanningSeedV1,
    #[serde(rename = "planning_series_arc_v1")]
    PlanningSeriesArcV1,
    #[serde(rename = "outline_book_v1")]
    OutlineBookV1,
    #[serde(rename = "outline_chapters_v1")]
    OutlineChaptersV1,
    #[serde(rename = "outline_manifest_v1")]
    OutlineManifestV1,
    #[serde(rename = "planning_repair_v1")]
    PlanningRepairV1,
    #[serde(rename = "planning_contract_v2")]
    PlanningContractV2,
    #[serde(rename = "planning_seed_v2")]
    PlanningSeedV2,
    #[serde(rename = "planning_series_arc_v2")]
    PlanningSeriesArcV2,
    #[serde(rename = "outline_book_v2")]
    OutlineBookV2,
    #[serde(rename = "outline_chapters_v2")]
    OutlineChaptersV2,
    #[serde(rename = "outline_manifest_v2")]
    OutlineManifestV2,
    #[serde(rename = "planning_repair_v2")]
    PlanningRepairV2,
}

impl PlanningPromptId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlanningContractV1 => "planning_contract_v1",
            Self::PlanningSeedV1 => "planning_seed_v1",
            Self::PlanningSeriesArcV1 => "planning_series_arc_v1",
            Self::OutlineBookV1 => "outline_book_v1",
            Self::OutlineChaptersV1 => "outline_chapters_v1",
            Self::OutlineManifestV1 => "outline_manifest_v1",
            Self::PlanningRepairV1 => "planning_repair_v1",
            Self::PlanningContractV2 => "planning_contract_v2",
            Self::PlanningSeedV2 => "planning_seed_v2",
            Self::PlanningSeriesArcV2 => "planning_series_arc_v2",
            Self::OutlineBookV2 => "outline_book_v2",
            Self::OutlineChaptersV2 => "outline_chapters_v2",
            Self::OutlineManifestV2 => "outline_manifest_v2",
            Self::PlanningRepairV2 => "planning_repair_v2",
        }
    }

    /// Whether this prompt belongs to the McKee (`_v2`) version family.
    pub const fn is_mckee_v2(self) -> bool {
        matches!(
            self,
            Self::PlanningContractV2
                | Self::PlanningSeedV2
                | Self::PlanningSeriesArcV2
                | Self::OutlineBookV2
                | Self::OutlineChaptersV2
                | Self::OutlineManifestV2
                | Self::PlanningRepairV2
        )
    }

    /// The repair prompt paired with this prompt version.
    pub const fn repair_prompt_id(self) -> Self {
        if self.is_mckee_v2() {
            Self::PlanningRepairV2
        } else {
            Self::PlanningRepairV1
        }
    }

    /// The version label recorded in model-call usage attribution (`v1`/`v2`).
    pub const fn version_label(self) -> &'static str {
        if self.is_mckee_v2() { "v2" } else { "v1" }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanningTargetRange {
    /// Book number the detailed plan covers (1-based).
    pub book_number: i32,
    /// First chapter in the detailed planning horizon (1-based).
    pub chapter_start: i32,
    /// Last chapter in the detailed planning horizon, inclusive.
    pub chapter_end: i32,
    /// Planned scene count for each chapter in the horizon.
    pub scenes_per_chapter: i32,
}

impl Default for PlanningTargetRange {
    fn default() -> Self {
        Self {
            book_number: 1,
            chapter_start: 1,
            chapter_end: 3,
            scenes_per_chapter: 2,
        }
    }
}

impl PlanningTargetRange {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlannedPromiseAction {
    Plant,
    Advance,
    Payoff,
    Defer,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlannedPromiseEvent {
    pub promise_ref: PlanningRef,
    pub action: PlannedPromiseAction,
    pub placement: PlanningStoryPlacement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlannedPov {
    pub book_number: i32,
    pub chapter_number: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_order: Option<i32>,
    pub pov_character_id: PlanningRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlannedIntensity {
    pub book_number: i32,
    pub chapter_number: i32,
    pub target_intensity: f64,
    #[serde(default)]
    pub recovery_beat: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The Subplot Sequence climax an Act climax cites as its cause.
///
/// The shape is deliberately narrow so it resolves deterministically: the plot
/// line is referenced with the same [`PlanningRef`] slot Sequences use, and the
/// Sequence is named by its `sequence_key`. The key is a **global** merge
/// identity, unique across the merged Sequences of the whole project (not just
/// within one plot line), so a later Chapter artifact's refinement is followed
/// deterministically and a key collision is a data error rather than an
/// ambiguity. Naming the record id instead would pin the cause to one
/// proposal's output.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlannedActCause {
    /// The Subplot whose Sequence climax causes this Act's climax. The field is
    /// named `plot_line_id` on purpose so the generic planning reference
    /// validation treats it as a `plot_line` reference.
    pub plot_line_id: PlanningRef,
    /// The cited Sequence's global merge key.
    pub sequence_key: String,
}

/// One Act in the ordered Act list carried by a writing-level schedule.
///
/// An Act is planning intent (ADR 0006), never a story record. It names the
/// reserved place where a later scene batch must land its climax, the Charge the
/// Spine Axis holds after that climax, and the part of the Central Question the
/// Act answers. The entries are additive schedule metadata; existing artifacts
/// without `acts` deserialize and serialize exactly as before.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlannedAct {
    /// 1-based position in the Act list. The final Act is the highest order.
    #[serde(default)]
    pub order: i32,
    /// The climax scene placeholder this Act reserves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub climax: Option<PlanningStoryPlacement>,
    /// The Charge the Spine Axis holds after this Act's climax.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charge: Option<Charge>,
    /// The part of the Central Question this Act answers.
    #[serde(default)]
    pub central_question_part: String,
    /// True only on the final Act, whose climax answers the Central Question.
    #[serde(default)]
    pub answers_central_question: bool,
    /// Optional Subplot Sequence climax this Act climax cites as its cause.
    /// Additive and skipped when absent, so existing Act JSON is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<PlannedActCause>,
}

/// One Sequence in the chapter-level Sequence list carried by a planning
/// schedule.
///
/// A Sequence groups planned Scenes under exactly one plot line and ends in its
/// strongest Scene. It is planning intent (ADR 0006), never a story record, and
/// its `member_scenes` may span chapters and books. The entries are additive
/// schedule metadata; existing artifacts without `sequences` deserialize and
/// serialize exactly as before.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlannedSequence {
    /// Stable **global** merge key, unique across the merged Sequences of the
    /// project. A later entry with the same key refines this Sequence instead of
    /// duplicating it, but may not move it to another plot line. The key is
    /// pointer-safe: non-empty, bounded length, ASCII alphanumerics plus `-`,
    /// `_` and `:`.
    #[serde(default)]
    pub sequence_key: String,
    /// The one plot line this Sequence belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plot_line_id: Option<PlanningRef>,
    /// The part of its plot line's question this Sequence answers.
    #[serde(default)]
    pub question: String,
    /// The Scene that opens the Sequence, when a separate inciting Scene is
    /// planned. It must not come after the climax.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inciting_scene: Option<PlanningStoryPlacement>,
    /// The reserved climax Scene. It must be one of `member_scenes`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub climax_scene: Option<PlanningStoryPlacement>,
    /// Member planned Scenes in strict story order.
    #[serde(default)]
    pub member_scenes: Vec<PlanningStoryPlacement>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanningSchedule {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promise_events: Vec<PlannedPromiseEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pov_plan: Vec<PlannedPov>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intensity_targets: Vec<PlannedIntensity>,
    /// Ordered Act list for the series arc or book, when the level plans one.
    /// Empty for levels that do not shape Acts and for existing artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acts: Vec<PlannedAct>,
    /// Sequence grouping for the chapter level, when the level plans one.
    /// Empty for other levels and for existing artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sequences: Vec<PlannedSequence>,
}

impl PlanningSchedule {
    pub fn is_empty(&self) -> bool {
        self.promise_events.is_empty()
            && self.pov_plan.is_empty()
            && self.intensity_targets.is_empty()
            && self.acts.is_empty()
            && self.sequences.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanningAuthorQuestion {
    /// Stable identifier for this author question.
    pub id: String,
    /// The creative decision the author needs to make.
    pub question: String,
    /// Optional set of choices the author may select.
    #[serde(default)]
    pub choices: Vec<String>,
    /// Optional recommended choice for the author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningArtifactV1 {
    pub schema_version: u32,
    pub level: PlanningLevel,
    #[serde(default)]
    pub target_range: PlanningTargetRange,
    pub rationale: String,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub author_questions: Vec<PlanningAuthorQuestion>,
    pub operations: Vec<PlanningOperationItem>,
    #[serde(default)]
    pub schedule: PlanningSchedule,
}

impl PlanningArtifactV1 {
    pub fn canonical_json_schema() -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(PlanningArtifactV1))
            .expect("PlanningArtifactV1 schema serializes")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanningFindingSeverity {
    Blocking,
    Advisory,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanningFinding {
    /// Stable machine-readable finding code.
    pub code: String,
    /// Whether this finding blocks acceptance or is advisory only.
    pub severity: PlanningFindingSeverity,
    /// JSON pointer or schema path the finding concerns.
    pub path: String,
    /// Related proposal references or record IDs.
    #[serde(default)]
    pub related_refs: Vec<String>,
    /// What the validator expected at this path.
    pub expected: String,
    /// What the staged artifact actually contained.
    pub actual: String,
    /// Short instruction for repairing the artifact.
    pub repair_hint: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanningCostStatus {
    EstimatedFromConfiguredPrices,
    #[default]
    UnknownUnpriced,
    UnknownMissingProviderUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanningBudget {
    /// Maximum number of route calls across the whole session.
    pub max_model_calls: u32,
    /// Maximum input tokens the session may spend.
    pub max_input_tokens: u64,
    /// Maximum output tokens the session may spend.
    pub max_output_tokens: u64,
    /// Maximum combined input plus output tokens.
    pub max_total_tokens: u64,
    /// Maximum output tokens requested from a single route call.
    pub max_output_tokens_per_call: u32,
    /// Optional hard cost ceiling in US dollars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cost_usd: Option<f64>,
    /// Optional configured input price in US dollars per million tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cost_per_million_usd: Option<f64>,
    /// Optional configured output price in US dollars per million tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_cost_per_million_usd: Option<f64>,
}

impl Default for PlanningBudget {
    fn default() -> Self {
        Self {
            max_model_calls: 12,
            max_input_tokens: 48_000,
            max_output_tokens: 24_000,
            max_total_tokens: 72_000,
            max_output_tokens_per_call: 2_000,
            max_cost_usd: None,
            input_cost_per_million_usd: None,
            output_cost_per_million_usd: None,
        }
    }
}

impl PlanningBudget {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanningBudgetUsage {
    /// Route calls already dispatched for this session.
    pub model_calls: u32,
    /// Input tokens reported or estimated so far.
    pub input_tokens: u64,
    /// Output tokens reported or estimated so far.
    pub output_tokens: u64,
    /// Combined input plus output tokens.
    pub total_tokens: u64,
    /// Estimated cost when configured prices are available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost_usd: Option<f64>,
    /// Conservative cost reserved for the maximum output of pending calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserved_cost_usd: Option<f64>,
    /// How the cost figure was derived, or why it is unknown.
    #[serde(default)]
    pub cost_status: PlanningCostStatus,
}

impl PlanningBudgetUsage {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanningOutputSource {
    Model,
    Host,
    Fixture,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanningModelCallReceipt {
    pub attempt_number: u32,
    pub route_task: String,
    pub resolved_route: String,
    pub agent_name: String,
    pub adapter_kind: String,
    pub model: String,
    pub prompt_id: PlanningPromptId,
    pub prompt_hash: String,
    pub schema_hash: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub provider_reported_usage: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost_usd: Option<f64>,
    pub cost_status: PlanningCostStatus,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanningGenerationReceipt {
    pub source: PlanningOutputSource,
    pub prompt_id: PlanningPromptId,
    pub prompt_hash: String,
    pub schema_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_task: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_route: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub attempts_used: u32,
    pub repaired: bool,
    #[serde(default)]
    pub calls: Vec<PlanningModelCallReceipt>,
    pub recorded_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanningSessionOutcome {
    Staged,
    NeedsAuthor,
    NeedsRoute,
    ModelOutputRejected,
    BudgetExhausted,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanningRouteProblem {
    MissingRoute {
        route_task: String,
        allow_review_backend: bool,
    },
    LocalStubRejected {
        route_task: String,
        resolved_route: String,
        agent_name: String,
        adapter_kind: String,
        builtin_default: bool,
    },
    MissingApiKey {
        route_task: String,
        resolved_route: String,
        agent_name: String,
        env_var: String,
    },
    RatingNotCovered {
        route_task: String,
        resolved_route: String,
        agent_name: String,
        rating_ceiling: ContentRating,
    },
    DispatchFailed {
        route_task: String,
        resolved_route: String,
        agent_name: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningHostRequest {
    pub session_id: String,
    pub level: PlanningLevel,
    pub route_task: String,
    pub prompt_id: PlanningPromptId,
    pub prompt_hash: String,
    pub schema_hash: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub artifact_schema: serde_json::Value,
    pub parent_proposals: BTreeMap<String, String>,
    pub target_range: PlanningTargetRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_problem: Option<PlanningRouteProblem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prior_findings: Vec<PlanningFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningHostSubmission {
    pub source: PlanningOutputSource,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdvancePlanningInput {
    pub session_id: String,
    pub scope: Option<PlanningScope>,
    pub level: Option<PlanningLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parents: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_range: Option<PlanningTargetRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_review_backend: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_ceiling: Option<ContentRating>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<PlanningBudget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_submission: Option<PlanningHostSubmission>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningAdvanceReport {
    pub attempt_id: String,
    pub session_id: String,
    pub level: PlanningLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_key: Option<String>,
    pub outcome: PlanningSessionOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<PlanningProposal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_receipt: Option<PlanningGenerationReceipt>,
    #[serde(default)]
    pub findings: Vec<PlanningFinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub author_questions: Vec<PlanningAuthorQuestion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_request: Option<PlanningHostRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_problem: Option<PlanningRouteProblem>,
    pub usage: PlanningBudgetUsage,
    pub budget: PlanningBudget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanningGate {
    #[default]
    Human,
    DelegateIfValid,
    Hold,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanningPovPolicy {
    #[serde(default)]
    pub single_pov: Option<PlanningRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_roster: Vec<PlanningRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub desired_shares: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share_tolerance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_absence_chapters: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_consecutive_chapters: Option<i32>,
    #[serde(default)]
    pub hard_constraint: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanningPacingPolicy {
    #[serde(default)]
    pub hard_constraint: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiver_reason: Option<String>,
}

/// Stable identity of the McKee gate profile. Delegated decisions made under
/// the profile are audited as `policy:mckee-default:1`.
pub const MCKEE_POLICY_ID: &str = "mckee-default";
/// Version of the McKee gate profile (see [`MCKEE_POLICY_ID`]).
pub const MCKEE_POLICY_VERSION: u32 = 1;

/// An empty map means human gates at every level. Root levels remain human-gated;
/// Chapter and Scene levels may opt into `delegate_if_valid` or `hold` once upstream
/// roots are accepted and a human explicitly ratifies the policy version.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanningPolicy {
    #[serde(default)]
    pub gates: BTreeMap<PlanningLevel, PlanningGate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_advisory_findings: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pov_policy: Option<PlanningPovPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pacing_policy: Option<PlanningPacingPolicy>,
}

impl PlanningPolicy {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The McKee gate profile: human at Contract, CastWorld, SeriesArc and Book;
    /// `delegate_if_valid` at Chapter and Scene.
    ///
    /// The profile is **never** applied automatically: every session starts
    /// all-human (ADR 0004), and an author activates this profile with an
    /// explicit `planning_advance(action = set_policy)` override. It carries
    /// the stable identity [`MCKEE_POLICY_ID`] / [`MCKEE_POLICY_VERSION`], so a
    /// policy-actor acceptance is recorded as `policy:mckee-default:1`. No
    /// advisory cap applies: a delegated level is accepted when hard checks
    /// pass and no blocking finding or unresolved author question remains, and
    /// the apply-time gate still refuses anything else.
    pub fn mckee_default() -> Self {
        Self {
            gates: BTreeMap::from([
                (PlanningLevel::Contract, PlanningGate::Human),
                (PlanningLevel::CastWorld, PlanningGate::Human),
                (PlanningLevel::SeriesArc, PlanningGate::Human),
                (PlanningLevel::Book, PlanningGate::Human),
                (PlanningLevel::Chapter, PlanningGate::DelegateIfValid),
                (PlanningLevel::Scene, PlanningGate::DelegateIfValid),
            ]),
            policy_id: Some(MCKEE_POLICY_ID.to_string()),
            policy_version: Some(MCKEE_POLICY_VERSION),
            max_advisory_findings: None,
            pov_policy: None,
            pacing_policy: None,
        }
    }

    /// Whether this policy carries the exact McKee profile identity (id and
    /// version).
    ///
    /// Gate overrides do not change the identity: an author may return Chapter
    /// or Scene to `human` and the session is still a McKee-profile session.
    /// Use [`Self::has_mckee_policy_id`] for the reserved-identity check, which
    /// covers every version of the profile.
    pub fn is_mckee_default(&self) -> bool {
        self.policy_id.as_deref() == Some(MCKEE_POLICY_ID)
            && self.policy_version == Some(MCKEE_POLICY_VERSION)
    }

    /// Whether this policy claims the reserved `mckee-default` identity at any
    /// version.
    ///
    /// The identity is reserved for McKee contexts for every version, not only
    /// the shipped `mckee-default:1` tuple, so a non-McKee session can never
    /// borrow the audited `policy:mckee-default:<version>` actor.
    pub fn has_mckee_policy_id(&self) -> bool {
        self.policy_id.as_deref() == Some(MCKEE_POLICY_ID)
    }

    pub fn delegated_actor(&self) -> Option<String> {
        let policy_id = self.policy_id.as_deref()?.trim();
        let version = self.policy_version?;
        if policy_id.is_empty() || version == 0 {
            return None;
        }
        Some(format!("policy:{policy_id}:{version}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetPlanningPolicyInput {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<PlanningScope>,
    pub policy: PlanningPolicy,
    pub actor: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanningScope {
    pub project_id: String,
    pub branch_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BeginPlanningInput {
    pub premise: String,
    pub scope: Option<PlanningScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningSession {
    pub id: String,
    pub premise: String,
    pub intent_hash: String,
    pub scope: Option<PlanningScope>,
    pub policy: PlanningPolicy,
    /// Prompt version pinned when the session was created: 2 for a McKee
    /// context (an unattached new-book session or an attached project carrying
    /// the McKee marker), 1 for an attached non-McKee project.
    ///
    /// Sessions created before this field existed have `None` and resolve
    /// version 1 byte-identically. The stored value is never re-derived, so a
    /// session does not flip prompt versions when it attaches or advances.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_version: Option<u32>,
    #[serde(default, skip_serializing_if = "PlanningTargetRange::is_default")]
    pub horizon: PlanningTargetRange,
    #[serde(default, skip_serializing_if = "PlanningBudget::is_default")]
    pub budget: PlanningBudget,
    #[serde(default, skip_serializing_if = "PlanningBudgetUsage::is_default")]
    pub usage: PlanningBudgetUsage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_ceiling: Option<ContentRating>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_review_backend: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub projection_stale: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_outcome: Option<PlanningSessionOutcome>,
    pub created_at: i64,
}

impl PlanningSession {
    /// The prompt version pinned at creation; legacy sessions resolve version 1.
    pub fn prompt_version_or_v1(&self) -> u32 {
        self.prompt_version.unwrap_or(1)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanningPacingMetrics {
    /// Number of scene intensity targets evaluated.
    pub evaluated_items: usize,
    /// Mean absolute deviation from the accepted pacing curve.
    pub mean_absolute_deviation: f64,
    /// Longest run of scenes that stayed outside the advisory band.
    pub longest_sustained_deviation: usize,
    /// Chapter count for each POV character reference.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pov_chapter_counts: BTreeMap<String, usize>,
    /// Share of chapters for each POV character reference.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pov_shares: BTreeMap<String, f64>,
    /// Longest consecutive chapter run for each POV character.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pov_max_consecutive_chapters: BTreeMap<String, i32>,
    /// Longest chapter absence for each POV character.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pov_max_absence_chapters: BTreeMap<String, i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningReadinessInput {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<PlanningScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_range: Option<PlanningTargetRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mining_policy: Option<AuthoringMiningPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_policy: Option<AuthoringCheckpointPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningReadinessReport {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<PlanningScope>,
    pub target_range: PlanningTargetRange,
    pub ready_to_draft: bool,
    pub planner_checks_passed: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stale_projection: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<PlanningFinding>,
    pub pacing_metrics: PlanningPacingMetrics,
    pub prepare_run: AuthoringPrepareRunOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "operation",
    content = "input",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum PlanningOperation {
    CreateProject(PlanningCreateProjectInput),
    CreateCharacter(Box<PlanningCreateCharacterInput>),
    CreateCharacterDesign(Box<PlanningCreateCharacterDesignInput>),
    CreateLocation(PlanningCreateLocationInput),
    CreateWorldRule(PlanningCreateWorldRuleInput),
    CreateRelationship(PlanningCreateRelationshipInput),
    CreatePlotLine(PlanningCreatePlotLineInput),
    CreateConflict(PlanningCreateConflictInput),
    CreateTheme(PlanningCreateThemeInput),
    CreateMotif(PlanningCreateMotifInput),
    CreateNarrativePromise(PlanningCreateNarrativePromiseInput),
    CreateKeyFact(PlanningCreateKeyFactInput),
    CreateCharacterArc(PlanningCreateCharacterArcInput),
    CreatePacingConfig(PlanningCreatePacingConfigInput),
    CreatePacingCurve(PlanningCreatePacingCurveInput),
    SetArcPacingConstraints(PlanningSetArcPacingConstraintsInput),
    CreateBook(PlanningCreateBookInput),
    CreateChapter(PlanningCreateChapterInput),
    SetBookOutline(PlanningSetBookOutlineInput),
    SetChapterOutline(PlanningSetChapterOutlineInput),
    PlanChapter(PlanningPlanChapterInput),
    UpdateReaderContract(PlanningReaderContract),
    CreateStorySpine(PlanningCreateStorySpineInput),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateProjectInput {
    pub name: String,
    pub project_type: String,
    pub genre: String,
    pub reader_contract: PlanningReaderContract,
}

/// Stage the series Story Spine or one book sub-spine at the contract level.
///
/// The four meaning fields are defaulted so an incomplete spine can be staged
/// with blocking findings; a proposal carrying such findings cannot be accepted.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateStorySpineInput {
    /// `None` creates the series spine; `Some` creates that book's sub-spine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_number: Option<i32>,
    /// Required for a book sub-spine; must name the series spine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_spine_id: Option<PlanningRef>,
    /// The question the inciting incident raises and the final climax answers.
    #[serde(default)]
    pub central_question: String,
    /// The claim the story proves: a value plus its cause.
    #[serde(default)]
    pub controlling_idea: ControllingIdea,
    /// The opposing idea the story argues against.
    #[serde(default)]
    pub counter_idea: String,
    /// The Value Axis the whole spine turns on.
    #[serde(default)]
    pub spine_axis: String,
    /// Whether the ending is idealist, pessimist, or ironic.
    pub ending_type: StorySpineEnding,
    /// Where the inciting incident is placed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inciting_incident: Option<PlanningStoryPlacement>,
    /// Where the final climax is placed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_climax: Option<PlanningStoryPlacement>,
    /// Themes this spine references; it never replaces them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub theme_ids: Vec<PlanningRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateCharacterInput {
    pub name: String,
    pub summary: String,
    pub role: String,
    pub realm: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub voice_profile: PlanningCharacterVoiceProfileData,
    #[serde(default)]
    pub emotional_profile: PlanningCharacterEmotionalProfileData,
    pub initial_state: Option<PlanningCharacterStatePatch>,
}

/// Stage a Character Design for a character introduced at the Cast & World
/// level. All text fields are defaulted so an incomplete design can be staged
/// with blocking findings instead of disappearing as a deserialization error.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateCharacterDesignInput {
    /// The character this design belongs to (an existing id or a proposal ref).
    pub character_id: PlanningRef,
    /// Two to five contradictions that give the character dimension.
    #[serde(default)]
    pub contradictions: Vec<String>,
    /// The self the character shows the world.
    #[serde(default)]
    pub public_self: String,
    /// The self the character shares with intimates.
    #[serde(default)]
    pub personal_self: String,
    /// The self the character keeps to themselves.
    #[serde(default)]
    pub private_self: String,
    /// The self the character hides even from themselves; stored as a secret
    /// canonical fact and never exposed by non-planning reads.
    #[serde(default)]
    pub hidden_self: String,
    /// What the character wants on the page.
    #[serde(default)]
    pub conscious_desire: String,
    /// What the character wants beneath the page; stored as a secret canonical
    /// fact and never exposed by non-planning reads.
    #[serde(default)]
    pub unconscious_desire: String,
    /// The pressure this character puts on the protagonist.
    #[serde(default)]
    pub pressure_on_protagonist: String,
    /// Planner-written observable behaviour cues derived from the hidden layer
    /// and the unconscious desire.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub behavior_cues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateLocationInput {
    pub name: String,
    #[serde(default, alias = "type")]
    pub kind: String,
    pub realm: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub initial_state: PlanningWorldStateInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateWorldRuleInput {
    pub rule_name: String,
    pub rule_type: String,
    pub description: String,
    #[serde(default)]
    pub scan_pattern: Option<String>,
    #[serde(default)]
    pub relevance_tags: Vec<String>,
    pub established_in: Option<PlanningEstablishedIn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateRelationshipInput {
    pub character_a_id: PlanningRef,
    pub character_b_id: PlanningRef,
    pub relationship_type: String,
    pub initial_trust: i32,
    pub initial_tension: i32,
    #[serde(default)]
    pub dynamics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreatePlotLineInput {
    pub name: String,
    pub plot_type: String,
    pub summary: String,
    pub status: Option<String>,
    #[serde(default)]
    pub convergence_points: Vec<PlanningStoryPlacement>,
    #[serde(default)]
    pub connected_conflict_ids: Vec<PlanningRef>,
    #[serde(default)]
    pub connected_theme_ids: Vec<PlanningRef>,
    /// The Value Axis this plot line turns; a Main Plot carries the Spine Axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_axis: Option<String>,
    /// The one structural job a Subplot performs toward the Controlling Idea.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subplot_job: Option<SubplotJob>,
    /// Whether this is the project's one Main Plot. False for every Subplot.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_main_plot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateConflictInput {
    pub name: String,
    pub conflict_type: String,
    pub stakes: String,
    #[serde(default)]
    pub escalation_stages: Vec<String>,
    pub expected_total_cycles: Option<i32>,
    #[serde(default)]
    pub try_fail_cycles: Vec<PlanningTryFailCycleStep>,
    #[serde(default)]
    pub stated_consequences: Vec<PlanningStatedConsequence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateThemeInput {
    pub theme_statement: String,
    pub thesis_antithesis: String,
    pub introduction_point: Option<PlanningStoryPlacement>,
    pub resolution_point: Option<PlanningStoryPlacement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateMotifInput {
    pub name: String,
    pub description: String,
    pub max_uses_per_chapter: Option<i32>,
    #[serde(default)]
    pub connected_theme_ids: Vec<PlanningRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateNarrativePromiseInput {
    pub promise_type: String,
    pub description: String,
    pub planted_at: PlanningStoryPlacement,
    pub planned_payoff: Option<PlanningStoryPlacement>,
    /// The 1-based position in the payoff Scene's `beats` where the payoff
    /// lands, ideally its Turning Beat (the same numbering as the Scene's
    /// `turning_beat`). Requires `planned_payoff`: a Beat position without the
    /// Scene that owns it is a stage-recordable blocking finding, and the
    /// direct write refuses it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub planned_payoff_turning_beat: Option<i32>,
    /// The 1-based order of the Act this promise must pay off in. Required for a
    /// long-range promise planned at the Series-Arc or Book level; it must name
    /// an Act in the accepted/merged Act list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub target_act: Option<i32>,
    #[serde(default)]
    pub notes: Vec<String>,
}

/// Stage one turning canon fact decided during planning (a Key Fact).
///
/// Only the closed [`KeyFactKind`] set is accepted, so a non-turning fact type
/// is rejected at deserialization and can never enter the Key Fact path. The
/// operation wraps the existing planned-and-pending canonical fact registration
/// (`scene_id: None`, placed by `placement`); the Scene it names may still be
/// unplanned. Acceptance returns the canonical fact id, which is bound to the
/// dramatising scene later with `bind_canonical_fact_to_scene` once the scene
/// row exists. A `revealed_secret` Key Fact is registered as a secret fact with
/// its subject character in the initial circle of trust.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateKeyFactInput {
    /// The closed turning type of this Key Fact.
    pub kind: KeyFactKind,
    /// Subject table of the fact's subject (for example `character`,
    /// `location` or `project`). Required together with `subject_id`; the
    /// existing canonical-fact subject model decides the allowed tables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_table: Option<String>,
    /// Subject record id (a record id or a planning reference). Required
    /// together with `subject_table`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<PlanningRef>,
    /// Unique key identifying the fact; it becomes the canonical fact's
    /// predicate.
    pub key: String,
    /// The canonical value text.
    pub value: String,
    /// Optional context about where/how the fact is established.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// The planned Scene that must dramatise the fact. `scene_order` is
    /// required; the Scene may be planned later (planned-and-pending).
    pub placement: PlanningStoryPlacement,
    /// Optional reference to the planned Scene's chapter plan (a
    /// `plan_chapter` output proposal reference or an existing `chapter_plan`
    /// record id). When present it must resolve and agree with `placement`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_scene_id: Option<PlanningRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateCharacterArcInput {
    pub character_id: PlanningRef,
    pub arc_type: String,
    pub starting_state: String,
    pub ending_state: String,
    #[serde(default)]
    pub milestones: Vec<PlanningCharacterArcMilestone>,
    pub thematic_purpose: String,
    #[serde(default)]
    pub connected_theme_ids: Vec<PlanningRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreatePacingConfigInput {
    pub total_planned_books: i32,
    pub avg_chapters_per_book: i32,
    pub avg_scenes_per_chapter: i32,
    pub tension_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreatePacingCurveInput {
    pub book_number: i32,
    pub act_breakpoints: BTreeMap<String, f64>,
    pub scene_type_density: BTreeMap<String, f64>,
    #[serde(default)]
    pub intensity_points: Vec<PlanningIntensityPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningSetArcPacingConstraintsInput {
    pub character_arc_id: PlanningRef,
    pub per_book_budget: BTreeMap<String, f64>,
    pub max_progress_per_chapter: Option<f64>,
    pub milestone_spacing: Option<i32>,
    pub sprint_allowance: Option<i32>,
    pub regression_budget: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateBookInput {
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCreateChapterInput {
    pub book_number: Option<i32>,
    pub book_id: Option<PlanningRef>,
    pub chapter_number: Option<i32>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningSetBookOutlineInput {
    #[serde(default)]
    pub book_number: i32,
    #[serde(default)]
    pub book_id: Option<PlanningRef>,
    #[serde(default)]
    pub format: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningSetChapterOutlineInput {
    #[serde(default)]
    pub book_number: i32,
    #[serde(default)]
    pub chapter_number: i32,
    #[serde(default, alias = "chapter_id")]
    pub entity_id: Option<PlanningRef>,
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub beats: Vec<PlanningChapterOutlineBeat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningPlanChapterInput {
    pub book_number: i32,
    pub chapter_number: i32,
    pub pov_character_id: Option<PlanningRef>,
    pub synopsis: String,
    #[serde(default)]
    pub target_theme_ids: Vec<PlanningRef>,
    #[serde(default)]
    pub target_conflict_ids: Vec<PlanningRef>,
    #[serde(default)]
    pub target_plot_line_ids: Vec<PlanningRef>,
    #[serde(default)]
    pub scenes: Vec<PlanningPlanChapterSceneInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PlanningPlanChapterSceneInput {
    pub scene_order: i32,
    pub summary: String,
    #[serde(default)]
    pub beat_structure: Vec<String>,
    /// The Scene's structured Beats in story order (McKee projects).
    ///
    /// The stored [`crate::models::PlannedScene`] carries the same Beats with
    /// `actor_character_id` resolved to a record id; this planning twin keeps
    /// the proposal reference so a Scene can act a character created in the
    /// same batch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beats: Vec<PlanningBeat>,
    /// 1-based position in `beats` of the Scene's Turning Beat.
    ///
    /// `0` is never a valid position; the generated schema advertises the
    /// 1-based minimum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub turning_beat: Option<u32>,
    #[serde(default)]
    pub character_ids: Vec<PlanningRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_id: Option<PlanningRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_rating: Option<ContentRating>,
    pub purpose: String,
    #[serde(default)]
    pub research_required: Option<bool>,
    #[serde(default)]
    pub research_tags: Vec<String>,
    #[serde(default)]
    pub explicit_query: Option<String>,
    /// Primary Value Axis the Scene turns, with opening and closing Charge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_value_axis: Option<SceneValueAxis>,
    /// Up to two secondary Value Axes the Scene also turns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secondary_value_axes: Vec<SceneValueAxis>,
    /// The opposition the Scene's action meets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opposition: Option<String>,
    /// The Gap the Scene designs (expected versus actual result).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<SceneGap>,
    /// The question the Scene answers for its Sequence or plot line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_question: Option<String>,
    /// What a character loses by doing nothing in this Scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inaction_cost: Option<String>,
    /// Planned risk level from 1 to 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<i32>,
    /// The dilemma and the tested character contradiction, when planned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dilemma: Option<PlanningSceneDilemma>,
}

/// A planned Scene dilemma whose tested character is a planning reference.
///
/// The stored scene resolves `character_id` to a record id; a planning artifact
/// may point at an earlier `create_character` operation or an existing
/// character visible on the session branch.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanningSceneDilemma {
    /// Character whose Character Design contradiction the dilemma tests.
    pub character_id: PlanningRef,
    /// The contradiction text, matched trim-exact against the design.
    pub contradiction: String,
}

/// A structured Beat inside a planning artifact.
///
/// The stored form is [`crate::beats::PlannedBeat`]: the two differ only in the
/// `actor_character_id`, which is a planning reference here and a resolved
/// record id there.
/// Every other field shares the McKee contract, including the optional
/// free-text Swain label (a label, never a structural kind).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanningBeat {
    /// Structural kind of this Beat.
    pub kind: crate::beats::BeatKind,
    /// Character who takes this Beat, as a planning reference.
    ///
    /// Spelled out rather than named `actor` because planning payloads resolve
    /// `*_id` keys generically; a bare `actor` key would collide with the
    /// human decision actor and any future operation field of that name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_character_id: Option<PlanningRef>,
    /// The tactic the actor uses to get something.
    pub tactic: String,
    /// What is said out loud on this Beat (empty for a silent Beat).
    pub said: String,
    /// What the actor actually wants — the subtext behind the tactic.
    pub wanted: String,
    /// What the actor expects this attempt to bring versus what the world
    /// actually returns.
    pub gap: SceneGap,
    /// Whether this Beat's Gap depends on information the reader must already
    /// have been promised. Only the Scene's Turning Beat is judged, and it must
    /// name the earlier-planted promise it pays off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub information_dependent: bool,
    /// The promise this information-dependent Beat depends on, as a planning
    /// reference (resolved to the narrative-promise record id on apply).
    ///
    /// Spelled out with the `_id` suffix because planning payloads resolve
    /// `*_id` keys generically; a bare `promise` key would not resolve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promise_id: Option<PlanningRef>,
    /// Optional Swain category label. Never a structural kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swain_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PlanningCharacterVoiceProfileData {
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub vocabulary: Vec<String>,
    #[serde(default)]
    pub sentence_structure: Vec<String>,
    #[serde(default)]
    pub tics: Vec<String>,
    #[serde(default)]
    pub forbidden_words: Vec<String>,
    #[serde(default)]
    pub example_lines: Vec<String>,
    #[serde(default)]
    pub established_in_scene_id: Option<PlanningRef>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PlanningWorldStateInput {
    pub controlling_faction: Option<PlanningRef>,
    pub status: Option<String>,
    pub prosperity: Option<String>,
    pub stability: Option<String>,
    pub threat_level: Option<String>,
    #[serde(default)]
    pub sensory_details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PlanningChapterOutlineBeat {
    pub order: i32,
    pub summary: String,
    pub scene_id: Option<PlanningRef>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningOperationItem {
    pub output_ref: String,
    pub operation: PlanningOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StagePlanningInput {
    pub session_id: String,
    pub scope: Option<PlanningScope>,
    pub intent_hash: String,
    pub schema_version: u32,
    pub level: PlanningLevel,
    /// Exact hashes of all external proposal dependencies.
    pub parents: BTreeMap<String, String>,
    pub operations: Vec<PlanningOperationItem>,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_range: Option<PlanningTargetRange>,
    #[serde(default, skip_serializing_if = "PlanningSchedule::is_empty")]
    pub schedule: PlanningSchedule,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub author_questions: Vec<PlanningAuthorQuestion>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanningProposalStatus {
    Staged,
    Applied,
    Rejected,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlanningProposal {
    pub id: String,
    pub artifact: StagePlanningInput,
    pub payload_hash: String,
    pub status: PlanningProposalStatus,
    pub policy_snapshot: PlanningPolicy,
    /// Existing records/targets captured at staging or approved parent-ID resolution;
    /// checked again under the writer gate.
    #[serde(default)]
    pub base_state: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_receipt: Option<PlanningGenerationReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<PlanningFinding>,
    pub decided_by: Option<String>,
    pub decision_note: Option<String>,
    pub decided_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanningDecision {
    Accept,
    Reject,
    Supersede,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecidePlanningInput {
    pub session_id: String,
    pub scope: Option<PlanningScope>,
    pub expected_hashes: BTreeMap<String, String>,
    pub decision: PlanningDecision,
    pub actor: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningReaderContract {
    pub promise: String,
    #[serde(default)]
    pub style_notes: Vec<String>,
    #[serde(default)]
    pub boundaries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCharacterStatePatch {
    #[serde(default)]
    pub emotional_state: BTreeMap<String, serde_json::Value>,
    pub goals: Option<Vec<String>>,
    pub status: Option<Vec<String>>,
    pub notes: Option<Vec<String>>,
    pub source_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PlanningCharacterEmotionalProfileData {
    #[serde(default)]
    pub base_emotions: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub suppressed: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub defense_mechanisms: Vec<String>,
    pub flex_range: Option<PlanningFlexRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningFlexRange {
    pub low: Option<String>,
    pub high: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningEstablishedIn {
    pub book_number: i32,
    pub chapter_number: i32,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanningStoryPlacement {
    pub book_number: i32,
    pub chapter_number: i32,
    pub scene_order: Option<i32>,
    pub note: Option<String>,
}

impl PlanningStoryPlacement {
    pub fn has_positive_coordinates(&self) -> bool {
        self.book_number >= 1
            && self.chapter_number >= 1
            && self.scene_order.is_none_or(|scene| scene >= 1)
    }

    /// Compares two story placements lexicographically by `(book_number, chapter_number, scene_order)`.
    /// When both placements are in the same `(book_number, chapter_number)` and one has `scene_order: None`
    /// (chapter-wide), they compare equal for window boundary checks unless strict scene ordering is requested.
    pub fn cmp_story_order(&self, other: &Self) -> std::cmp::Ordering {
        match (
            self.book_number.cmp(&other.book_number),
            self.chapter_number.cmp(&other.chapter_number),
        ) {
            (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => {
                match (self.scene_order, other.scene_order) {
                    (Some(left), Some(right)) => left.cmp(&right),
                    _ => std::cmp::Ordering::Equal,
                }
            }
            (std::cmp::Ordering::Equal, chapter_ord) => chapter_ord,
            (book_ord, _) => book_ord,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningCharacterArcMilestone {
    pub label: String,
    pub placement: Option<PlanningStoryPlacement>,
    pub description: String,
    #[serde(default)]
    pub unlocks: Vec<String>,
    #[serde(default)]
    pub reached_at: Option<PlanningStoryPlacement>,
    /// Planned change-or-reveal kind; absent means unclassified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ArcMilestoneKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningStatedConsequence {
    pub description: String,
    pub stated_at: Option<PlanningStoryPlacement>,
    pub must_demonstrate_by: Option<String>,
    pub delivered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningTryFailCycleStep {
    pub attempt_order: i32,
    pub label: String,
    pub outcome: String,
    pub cost: Option<String>,
    pub revelation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningIntensityPoint {
    pub position: f64,
    pub intensity: f64,
}

/// Session settings that can be changed without generating a planning level.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigurePlanningSessionInput {
    /// Planning session to update.
    pub session_id: String,
    /// Project and branch the session is bound to; omit before the project root is accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<PlanningScope>,
    /// Detailed chapter horizon to store on the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon: Option<PlanningTargetRange>,
    /// Token and call ceiling to store on the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<PlanningBudget>,
    /// Highest content rating the stored plan may request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_ceiling: Option<ContentRating>,
    /// Whether a configured review agent may be used as a planning backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_review_backend: Option<bool>,
}

/// Gate selected for every planning level. Root levels cannot be delegated.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanningGatePolicy {
    /// Gate for the Contract level; this root requires human review.
    #[serde(default)]
    pub contract: PlanningGate,
    /// Gate for the CastWorld level; this root requires human review.
    #[serde(default)]
    pub cast_world: PlanningGate,
    /// Gate for the SeriesArc level; this root requires human review.
    #[serde(default)]
    pub series_arc: PlanningGate,
    /// Gate for the Book level; this root requires human review.
    #[serde(default)]
    pub book: PlanningGate,
    /// Gate for Chapter-level proposals.
    #[serde(default)]
    pub chapter: PlanningGate,
    /// Gate for Scene-level proposals.
    #[serde(default)]
    pub scene: PlanningGate,
}

impl PlanningGatePolicy {
    /// Build a full gate policy from a sparse level map.
    pub fn from_gates(gates: &BTreeMap<PlanningLevel, PlanningGate>) -> Self {
        let gate = |level| gates.get(&level).copied().unwrap_or_default();
        Self {
            contract: gate(PlanningLevel::Contract),
            cast_world: gate(PlanningLevel::CastWorld),
            series_arc: gate(PlanningLevel::SeriesArc),
            book: gate(PlanningLevel::Book),
            chapter: gate(PlanningLevel::Chapter),
            scene: gate(PlanningLevel::Scene),
        }
    }

    /// Materialize the complete level map stored on the planning session.
    pub fn to_gates(self) -> BTreeMap<PlanningLevel, PlanningGate> {
        BTreeMap::from([
            (PlanningLevel::Contract, self.contract),
            (PlanningLevel::CastWorld, self.cast_world),
            (PlanningLevel::SeriesArc, self.series_arc),
            (PlanningLevel::Book, self.book),
            (PlanningLevel::Chapter, self.chapter),
            (PlanningLevel::Scene, self.scene),
        ])
    }

    /// Gate configured for one level.
    pub fn gate_for(self, level: PlanningLevel) -> PlanningGate {
        match level {
            PlanningLevel::Contract => self.contract,
            PlanningLevel::CastWorld => self.cast_world,
            PlanningLevel::SeriesArc => self.series_arc,
            PlanningLevel::Book => self.book,
            PlanningLevel::Chapter => self.chapter,
            PlanningLevel::Scene => self.scene,
        }
    }
}

/// Explicit gate and policy settings for a planning session. Every level stays
/// human-gated unless the author names a delegated gate here.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanningPolicyOverride {
    /// Gate to apply at each planning level; omitted levels keep human review.
    #[serde(default)]
    pub gates: PlanningGatePolicy,
    /// Stable identifier for an explicitly delegated policy version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<String>,
    /// Monotonic policy version recorded with delegated decisions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<u32>,
    /// Maximum advisory findings allowed for delegated chapter or scene acceptance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_advisory_findings: Option<usize>,
}

/// Start a new planning session or resume an existing one.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningBeginInput {
    /// One- to three-sentence premise or intent for a new planning session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premise: Option<String>,
    /// Existing planning session to resume instead of starting a new one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Existing project to bind; omit for a brand-new book.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Branch to bind; defaults to the project's active branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    /// Detailed chapter horizon to generate; defaults to three chapters with two scenes each.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon: Option<PlanningTargetRange>,
    /// Token and call ceiling for route generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<PlanningBudget>,
    /// Highest content rating the plan may request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_ceiling: Option<ContentRating>,
    /// Allow a configured review agent as a planning backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_review_backend: Option<bool>,
    /// Gate policy override; omission keeps every level human-gated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<PlanningPolicyOverride>,
    /// Human author recorded as ratifying the policy override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_actor: Option<String>,
    /// Note explaining why the policy override is authorized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_note: Option<String>,
}

/// Compact audit summary of a planning policy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanningPolicySummary {
    /// Gate configured for each planning level; the default is human review.
    #[serde(default)]
    pub gates: PlanningGatePolicy,
    /// Identifier of the explicitly delegated policy version, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<String>,
    /// Version of the explicitly delegated policy, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<u32>,
    /// Maximum advisory findings allowed for delegation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_advisory_findings: Option<usize>,
    /// Whether the session carries an explicit POV policy.
    #[serde(default)]
    pub has_pov_policy: bool,
    /// Whether the session carries an explicit pacing policy.
    #[serde(default)]
    pub has_pacing_policy: bool,
}

/// Compact planning session state suitable for reconnect and review.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningSessionSummary {
    /// Stable planning session identifier.
    pub session_id: String,
    /// Short preview of the author's premise.
    pub premise_preview: String,
    /// Hash of the exact premise intent used by staging.
    pub intent_hash: String,
    /// Bound project identifier, once a project root is accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Bound branch identifier, once a project root is accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    /// First planning level that has no staged or accepted proposal.
    pub next_level: PlanningLevel,
    /// Current gate and policy summary.
    pub policy: PlanningPolicySummary,
    /// Detailed chapter horizon stored on the session.
    pub horizon: PlanningTargetRange,
    /// Token and call ceiling stored on the session.
    pub budget: PlanningBudget,
    /// Usage recorded so far.
    pub usage: PlanningBudgetUsage,
    /// Highest content rating the plan may request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_ceiling: Option<ContentRating>,
    /// Whether a configured review agent may be used as a planning backend.
    #[serde(default)]
    pub allow_review_backend: bool,
    /// Whether an accepted amendment made the planning projection stale.
    #[serde(default)]
    pub projection_stale: bool,
    /// Explanation recorded when the projection became stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_reason: Option<String>,
    /// Outcome of the most recent generation attempt, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_outcome: Option<PlanningSessionOutcome>,
    /// Session creation time in microseconds since the Unix epoch.
    pub created_at: i64,
}

/// One typed operation inside a staged proposal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningOperationSummary {
    /// Proposal-local output reference used by dependent operations.
    pub output_ref: String,
    /// Typed operation payload exactly as staged.
    pub operation: serde_json::Value,
}

/// Review summary of one staged or decided proposal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningProposalSummary {
    /// Stable proposal identifier.
    pub proposal_id: String,
    /// Planning level this proposal covers.
    pub level: PlanningLevel,
    /// Current proposal status.
    pub status: PlanningProposalStatus,
    /// Exact hash required to record a decision for this proposal.
    pub payload_hash: String,
    /// Creative rationale supplied with the artifact.
    pub rationale: String,
    /// Typed operations included in the proposal.
    #[serde(default)]
    pub operations: Vec<PlanningOperationSummary>,
    /// Promise, POV and intensity schedule supplied with the artifact.
    #[serde(default)]
    pub schedule: serde_json::Value,
    /// Assumptions the plan relies on.
    #[serde(default)]
    pub assumptions: Vec<String>,
    /// Questions the author still needs to answer.
    #[serde(default)]
    pub author_questions: Vec<PlanningAuthorQuestion>,
    /// Validation findings recorded at staging or decision time.
    #[serde(default)]
    pub findings: Vec<PlanningFinding>,
    /// Actor who recorded the decision, when decided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<String>,
    /// Note recorded with the decision, when decided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_note: Option<String>,
    /// Proposal creation time in microseconds since the Unix epoch.
    pub created_at: i64,
}

/// Result of starting or resuming a planning session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningBeginOutput {
    /// Compact session state after the call.
    pub session: PlanningSessionSummary,
    /// Whether an existing session was resumed instead of created.
    pub resumed: bool,
    /// The next safe action for the host or author.
    pub next_action: String,
    /// Short human-readable status message.
    pub message: String,
}

/// One planning action a host may request without silent drafting.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanningAdvanceAction {
    /// Generate the next or requested planning level through the configured route.
    #[default]
    Generate,
    /// Stage a host-authored structured artifact when no route is available.
    StageHostArtifact,
    /// Retry the most recent planning level after a rejected or malformed attempt.
    RepairLastProposal,
    /// Recompute planner checks and the authoring prepare report without generating.
    RecomputeReadiness,
    /// Update the session gate policy without generating.
    SetPolicy,
}

/// Generate, submit, retry, recheck or repolicy one planning level.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningAdvanceInput {
    /// Planning session to advance.
    pub session_id: String,
    /// Project the session is bound to; omit to use the stored scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Branch the session is bound to; defaults to the project's active branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    /// Action to perform; defaults to generating the next level.
    #[serde(default)]
    pub action: PlanningAdvanceAction,
    /// Planning level to generate; defaults to the first unfilled level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<PlanningLevel>,
    /// Exact accepted or staged parent proposal hashes; defaults to active ancestors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parents: Option<BTreeMap<String, String>>,
    /// Detailed chapter horizon for this generation call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_range: Option<PlanningTargetRange>,
    /// Allow a configured review agent as a planning backend for this call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_review_backend: Option<bool>,
    /// Highest content rating the generated plan may request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_ceiling: Option<ContentRating>,
    /// Token and call ceiling for this call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<PlanningBudget>,
    /// Host-authored artifact payload used by the stage_host_artifact action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_artifact: Option<serde_json::Value>,
    /// Caller-supplied idempotency key that prevents duplicate generation attempts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_key: Option<String>,
    /// Gate policy override used by the set_policy action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<PlanningPolicyOverride>,
    /// Human author recorded as ratifying a policy change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_actor: Option<String>,
    /// Note explaining why a policy change is authorized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_note: Option<String>,
    /// Canon-mining policy applied when checking authoring readiness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mining_policy: Option<AuthoringMiningPolicy>,
    /// Checkpoint policy applied when checking authoring readiness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_policy: Option<AuthoringCheckpointPolicy>,
}

/// Host generation request returned when no planning route can run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningHostRequestSummary {
    /// Planning level the host must generate.
    pub level: PlanningLevel,
    /// Logical route task the host is replacing.
    pub route_task: String,
    /// Versioned prompt identifier the host must use.
    pub prompt_id: PlanningPromptId,
    /// Hash of the exact prompt text.
    pub prompt_hash: String,
    /// Hash of the exact artifact schema.
    pub schema_hash: String,
    /// System instructions for the host model.
    pub system_prompt: String,
    /// User prompt containing intent, context and constraints.
    pub user_prompt: String,
    /// JSON Schema the host artifact must satisfy.
    pub artifact_schema: serde_json::Value,
    /// Exact parent proposal hashes the artifact must reference.
    #[serde(default)]
    pub parent_proposals: BTreeMap<String, String>,
    /// Detailed chapter horizon the artifact must cover.
    pub target_range: PlanningTargetRange,
    /// Short explanation of why the configured route could not run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_problem: Option<String>,
    /// Findings from an earlier rejected attempt that must be repaired.
    #[serde(default)]
    pub prior_findings: Vec<PlanningFinding>,
}

/// Compact outcome of one generation or submission attempt.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningAdvanceSummary {
    /// Durable attempt identifier for retry and audit.
    pub attempt_id: String,
    /// Planning level attempted.
    pub level: PlanningLevel,
    /// Honest session outcome for the attempt.
    pub outcome: PlanningSessionOutcome,
    /// Staged proposal produced by the attempt, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<PlanningProposalSummary>,
    /// Where the artifact came from, when recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PlanningOutputSource>,
    /// Number of route attempts consumed, including repair.
    #[serde(default)]
    pub attempts_used: u32,
    /// Whether the single bounded repair pass was used.
    #[serde(default)]
    pub repaired: bool,
    /// Validation findings from the attempt.
    #[serde(default)]
    pub findings: Vec<PlanningFinding>,
    /// Questions the author must answer before acceptance.
    #[serde(default)]
    pub author_questions: Vec<PlanningAuthorQuestion>,
    /// Host generation request used when no route could run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_request: Option<PlanningHostRequestSummary>,
    /// Usage recorded by the attempt.
    pub usage: PlanningBudgetUsage,
    /// Ceiling in force for the attempt.
    pub budget: PlanningBudget,
    /// Optional human-readable status message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Result of advancing, rechecking or repolicing a planning session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningAdvanceOutput {
    /// Compact session state after the call.
    pub session: PlanningSessionSummary,
    /// Action performed by the call.
    pub action: PlanningAdvanceAction,
    /// Generation result, when the action generated or submitted an artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advance: Option<PlanningAdvanceSummary>,
    /// Readiness result, when the action recomputed or checked it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<PlanningReadinessSummary>,
    /// The next safe action for the host or author.
    pub next_action: String,
}

/// One chapter in the authoring prepare report.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningPrepareChapterSummary {
    /// Chapter number checked.
    pub chapter_number: i32,
    /// Whether this chapter is ready for drafting.
    pub ready: bool,
    /// Items this chapter still needs.
    #[serde(default)]
    pub missing_items: Vec<String>,
}

/// Compact authoring prepare report for the planned range.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningPrepareRunSummary {
    /// Project whose chapters were checked.
    pub project_id: String,
    /// Book number checked.
    pub book_number: i32,
    /// First chapter checked.
    pub start_chapter: i32,
    /// Last chapter checked, inclusive.
    pub end_chapter: i32,
    /// Whether the whole range is ready for drafting.
    pub ready_to_draft: bool,
    /// Items still required before drafting can start.
    #[serde(default)]
    pub missing_requirements: Vec<String>,
    /// Per-chapter readiness.
    #[serde(default)]
    pub chapters: Vec<PlanningPrepareChapterSummary>,
}

/// Planner checks plus the real authoring prepare result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningReadinessSummary {
    /// Chapter range checked.
    pub target_range: PlanningTargetRange,
    /// Whether the accepted plan can start drafting.
    pub ready_to_draft: bool,
    /// Whether the deterministic planner checks passed.
    pub planner_checks_passed: bool,
    /// Whether a stale projection blocks readiness.
    pub stale_projection: bool,
    /// Blocking and advisory planner findings.
    #[serde(default)]
    pub findings: Vec<PlanningFinding>,
    /// POV and pacing measurements for the checked range.
    pub pacing_metrics: PlanningPacingMetrics,
    /// Real authoring prepare report for the same range.
    pub prepare_run: PlanningPrepareRunSummary,
}

/// Read compact planning status for one session or for reconnect discovery.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningStatusInput {
    /// Existing planning session to inspect; omit to discover active sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Existing project whose sessions should be listed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Branch whose sessions should be listed; defaults to the project's active branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    /// Chapter range to check instead of the session's stored horizon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_range: Option<PlanningTargetRange>,
    /// Whether to include planner checks and the authoring prepare report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_readiness: Option<bool>,
}

/// Compact planning status, staged proposals and reconnect guidance.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningStatusOutput {
    /// Selected session detail, when a session was found or selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<PlanningSessionSummary>,
    /// Sessions matching the requested project, or recent sessions when no project is given.
    #[serde(default)]
    pub sessions: Vec<PlanningSessionSummary>,
    /// Effective gate for the selected session's next level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<PlanningGate>,
    /// Proposals staged or decided in the selected session.
    #[serde(default)]
    pub proposals: Vec<PlanningProposalSummary>,
    /// Planner findings currently visible for the selected session.
    #[serde(default)]
    pub findings: Vec<PlanningFinding>,
    /// Persisted Story Spines for the selected session's project, when attached.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub story_spines: Vec<StorySpine>,
    /// Persisted Character Designs for the selected session's project, when
    /// attached. Planning reads are full reads: each view carries the hidden
    /// layer and unconscious desire text from its secret canonical facts.
    /// Skipped when empty so existing sessions' output is unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub character_designs: Vec<CharacterDesignView>,
    /// Readiness report for the selected session, when its project is attached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<PlanningReadinessSummary>,
    /// The next safe action for the host or author.
    pub next_action: String,
    /// How to reconnect to this work in a later session.
    pub reconnect_hint: String,
}

/// One decision action for staged planning proposals.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanningDecideAction {
    /// Record an accept, reject or supersede decision for staged proposals.
    #[default]
    Decide,
    /// Supersede staged proposals so a replacement can be staged at the same level.
    Revise,
    /// Check readiness and prepare the authoring run without changing proposals.
    PrepareAuthoringRun,
}

/// Decide staged planning proposals or prepare the authoring run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningDecideInput {
    /// Planning session whose proposals are being decided.
    pub session_id: String,
    /// Project the session is bound to; omit to use the stored scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Branch the session is bound to; defaults to the project's active branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    /// Whether to decide proposals or prepare the authoring run.
    #[serde(default)]
    pub action: PlanningDecideAction,
    /// Decision to record when the action is decide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<PlanningDecision>,
    /// Exact proposal IDs and payload hashes the decision covers.
    #[serde(default)]
    pub expected_hashes: BTreeMap<String, String>,
    /// Human author recorded as making the decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// Note explaining the decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Chapter range to prepare instead of the session's stored horizon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_range: Option<PlanningTargetRange>,
    /// Canon-mining policy applied when preparing the authoring run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mining_policy: Option<AuthoringMiningPolicy>,
    /// Checkpoint policy applied when preparing the authoring run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_policy: Option<AuthoringCheckpointPolicy>,
}

/// Result of a planning decision or authoring preparation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningDecideOutput {
    /// Compact session state after the call.
    pub session: PlanningSessionSummary,
    /// Action performed by the call.
    pub action: PlanningDecideAction,
    /// Proposals affected by the decision.
    #[serde(default)]
    pub decided: Vec<PlanningProposalSummary>,
    /// Readiness report produced by the call, when checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<PlanningReadinessSummary>,
    /// The next safe action for the host or author.
    pub next_action: String,
    /// Short human-readable status message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use serde_json::{Value, json};

    fn rejects_extra<T: DeserializeOwned>(mut value: Value) {
        assert!(
            serde_json::from_value::<T>(value.clone()).is_ok(),
            "valid nested fixture must parse"
        );
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected_field".into(), json!(true));
        assert!(
            serde_json::from_value::<T>(value).is_err(),
            "nested unknown fields must fail closed"
        );
    }

    #[test]
    fn prompt_versions_resolve_by_pinned_version_and_repair_pairs() {
        let v1 = [
            PlanningPromptId::PlanningContractV1,
            PlanningPromptId::PlanningSeedV1,
            PlanningPromptId::PlanningSeriesArcV1,
            PlanningPromptId::OutlineBookV1,
            PlanningPromptId::OutlineChaptersV1,
            PlanningPromptId::OutlineManifestV1,
        ];
        let v2 = [
            PlanningPromptId::PlanningContractV2,
            PlanningPromptId::PlanningSeedV2,
            PlanningPromptId::PlanningSeriesArcV2,
            PlanningPromptId::OutlineBookV2,
            PlanningPromptId::OutlineChaptersV2,
            PlanningPromptId::OutlineManifestV2,
        ];
        for (level, (old, new)) in PlanningLevel::all().into_iter().zip(v1.into_iter().zip(v2)) {
            assert_eq!(level.prompt_id(), old);
            assert_eq!(level.prompt_id_for_version(0), old);
            assert_eq!(level.prompt_id_for_version(1), old);
            assert_eq!(level.prompt_id_for_version(2), new);
            assert_eq!(level.prompt_id_for_version(3), new);
            assert!(!old.is_mckee_v2());
            assert!(new.is_mckee_v2());
            assert_eq!(old.version_label(), "v1");
            assert_eq!(new.version_label(), "v2");
            assert_eq!(old.repair_prompt_id(), PlanningPromptId::PlanningRepairV1);
            assert_eq!(new.repair_prompt_id(), PlanningPromptId::PlanningRepairV2);
            assert_eq!(PlanningPromptId::PlanningRepairV1.version_label(), "v1");
            assert_eq!(PlanningPromptId::PlanningRepairV2.version_label(), "v2");
            assert!(!PlanningPromptId::PlanningRepairV1.is_mckee_v2());
            assert!(PlanningPromptId::PlanningRepairV2.is_mckee_v2());
        }
        // Root levels are exactly the always-human levels.
        for level in [PlanningLevel::Chapter, PlanningLevel::Scene] {
            assert!(!level.is_root(), "{level:?}");
        }
        for level in [
            PlanningLevel::Contract,
            PlanningLevel::CastWorld,
            PlanningLevel::SeriesArc,
            PlanningLevel::Book,
        ] {
            assert!(level.is_root(), "{level:?}");
        }
        // Wire names are stable JSON-safe strings in both directions.
        for id in [
            PlanningPromptId::PlanningContractV1,
            PlanningPromptId::PlanningSeedV1,
            PlanningPromptId::PlanningSeriesArcV1,
            PlanningPromptId::OutlineBookV1,
            PlanningPromptId::OutlineChaptersV1,
            PlanningPromptId::OutlineManifestV1,
            PlanningPromptId::PlanningRepairV1,
            PlanningPromptId::PlanningContractV2,
            PlanningPromptId::PlanningSeedV2,
            PlanningPromptId::PlanningSeriesArcV2,
            PlanningPromptId::OutlineBookV2,
            PlanningPromptId::OutlineChaptersV2,
            PlanningPromptId::OutlineManifestV2,
            PlanningPromptId::PlanningRepairV2,
        ] {
            let wire = serde_json::to_value(id).unwrap();
            assert_eq!(wire, json!(id.as_str()));
        }
        assert_eq!(
            serde_json::from_value::<PlanningPromptId>(json!("outline_manifest_v2")).unwrap(),
            PlanningPromptId::OutlineManifestV2
        );
    }

    /// The pinned session version is the only input to prompt selection, and a
    /// session without one (created before the field existed) resolves v1.
    #[test]
    fn session_prompt_version_is_pinned_and_legacy_sessions_resolve_v1() {
        let mut session: PlanningSession = serde_json::from_value(json!({
            "id": "planning_session:legacy",
            "premise": "A legacy session",
            "intent_hash": "hash",
            "scope": null,
            "policy": {},
            "created_at": 1
        }))
        .unwrap();
        assert_eq!(session.prompt_version, None);
        assert_eq!(session.prompt_version_or_v1(), 1);
        for level in PlanningLevel::all() {
            assert_eq!(
                level.prompt_id_for_version(session.prompt_version_or_v1()),
                level.prompt_id()
            );
        }
        session.prompt_version = Some(2);
        assert_eq!(session.prompt_version_or_v1(), 2);
        for level in PlanningLevel::all() {
            assert!(
                level
                    .prompt_id_for_version(session.prompt_version_or_v1())
                    .is_mckee_v2()
            );
        }
        // `None` is skipped on the wire so legacy JSON stays byte-identical.
        session.prompt_version = None;
        let wire = serde_json::to_value(&session).unwrap();
        assert!(wire.get("prompt_version").is_none(), "{wire}");
    }

    #[test]
    fn mckee_default_policy_keeps_roots_human_and_delegates_detail() {
        let policy = PlanningPolicy::mckee_default();
        assert!(policy.is_mckee_default());
        assert!(policy.has_mckee_policy_id());
        assert_eq!(
            policy.delegated_actor().as_deref(),
            Some("policy:mckee-default:1")
        );
        assert_eq!(MCKEE_POLICY_ID, "mckee-default");
        assert_eq!(MCKEE_POLICY_VERSION, 1);
        for level in [
            PlanningLevel::Contract,
            PlanningLevel::CastWorld,
            PlanningLevel::SeriesArc,
            PlanningLevel::Book,
        ] {
            assert_eq!(policy.gates[&level], PlanningGate::Human, "{level:?}");
        }
        for level in [PlanningLevel::Chapter, PlanningLevel::Scene] {
            assert_eq!(
                policy.gates[&level],
                PlanningGate::DelegateIfValid,
                "{level:?}"
            );
        }
        assert_eq!(policy.max_advisory_findings, None);
        // The all-human default is a different policy with no delegated actor:
        // the profile is never applied automatically.
        assert!(PlanningPolicy::default().gates.is_empty());
        assert_eq!(PlanningPolicy::default().delegated_actor(), None);
        assert!(!PlanningPolicy::default().is_mckee_default());
        assert!(!PlanningPolicy::default().has_mckee_policy_id());
        // Identity survives an author override back to human detail gates.
        let overridden = PlanningPolicy {
            gates: PlanningGatePolicy::default().to_gates(),
            ..policy.clone()
        };
        assert!(overridden.is_mckee_default());
        assert!(overridden.has_mckee_policy_id());
        // The reserved identity covers every version, not only the v1 tuple.
        let other_version = PlanningPolicy {
            policy_version: Some(9),
            ..policy.clone()
        };
        assert!(!other_version.is_mckee_default());
        assert!(other_version.has_mckee_policy_id());
    }

    #[test]
    fn nested_planning_contracts_are_strict() {
        rejects_extra::<PlanningReaderContract>(json!({"promise":"Discovery"}));
        rejects_extra::<PlanningCharacterStatePatch>(json!({}));
        rejects_extra::<PlanningCharacterEmotionalProfileData>(json!({}));
        rejects_extra::<PlanningFlexRange>(json!({}));
        rejects_extra::<PlanningEstablishedIn>(json!({"book_number":1,"chapter_number":1}));
        rejects_extra::<PlanningStoryPlacement>(json!({"book_number":1,"chapter_number":1}));
        rejects_extra::<PlanningCharacterArcMilestone>(
            json!({"label":"Trust","description":"Accept help"}),
        );
        rejects_extra::<PlanningStatedConsequence>(
            json!({"description":"Exposure","delivered":false}),
        );
        rejects_extra::<PlanningTryFailCycleStep>(
            json!({"attempt_order":1,"label":"Ask","outcome":"Refused"}),
        );
        rejects_extra::<PlanningIntensityPoint>(json!({"position":0.0,"intensity":0.5}));
        rejects_extra::<PlanningCharacterVoiceProfileData>(json!({}));
        rejects_extra::<PlanningWorldStateInput>(json!({}));
        rejects_extra::<PlanningChapterOutlineBeat>(
            json!({"order":1,"summary":"Clue","status":"planned"}),
        );
        rejects_extra::<PlanningPlanChapterSceneInput>(
            json!({"scene_order":1,"summary":"Clue","purpose":"Discovery"}),
        );
        rejects_extra::<PlanningBeat>(json!({
            "kind":"action",
            "tactic":"Ask for the log",
            "said":"Show me the tide log.",
            "wanted":"A clean record",
            "gap":{"expected":"The log clears her","actual":"A page is missing"}
        }));
        rejects_extra::<PlanningTargetRange>(
            json!({"book_number":1,"chapter_start":1,"chapter_end":3,"scenes_per_chapter":2}),
        );
        rejects_extra::<PlannedPromiseEvent>(json!({
            "promise_ref":{"kind":"proposal_ref","value":"arc:promise"},
            "action":"plant",
            "placement":{"book_number":1,"chapter_number":1}
        }));
        rejects_extra::<PlannedPov>(json!({
            "book_number":1,
            "chapter_number":1,
            "pov_character_id":{"kind":"proposal_ref","value":"cast:ada"}
        }));
        rejects_extra::<PlannedIntensity>(
            json!({"book_number":1,"chapter_number":1,"target_intensity":0.5}),
        );
        rejects_extra::<PlannedAct>(json!({
            "order":1,
            "climax":{"book_number":1,"chapter_number":1,"scene_order":2},
            "charge":{"sign":"negative","depth":"contrary"},
            "central_question_part":"The lie surfaces",
            "answers_central_question":false
        }));
        rejects_extra::<PlanningSchedule>(json!({}));
        rejects_extra::<PlanningAuthorQuestion>(json!({"id":"q1","question":"Which tone?"}));
        rejects_extra::<PlanningArtifactV1>(json!({
            "schema_version":1,
            "level":"contract",
            "rationale":"Establish contract",
            "operations":[{"output_ref":"contract:project","operation":{"operation":"update_reader_contract","input":{"promise":"Discovery"}}}]
        }));
        rejects_extra::<PlanningFinding>(json!({
            "code":"ref_unresolved",
            "severity":"blocking",
            "path":"/operations/0",
            "expected":"character",
            "actual":"missing",
            "repair_hint":"Use a valid character reference"
        }));
        rejects_extra::<PlanningBudget>(json!({
            "max_model_calls":12,
            "max_input_tokens":48000,
            "max_output_tokens":24000,
            "max_total_tokens":72000,
            "max_output_tokens_per_call":2000
        }));
        rejects_extra::<PlanningBudgetUsage>(json!({
            "model_calls":0,
            "input_tokens":0,
            "output_tokens":0,
            "total_tokens":0
        }));
        rejects_extra::<PlanningHostSubmission>(json!({"source":"host","payload":{}}));
        rejects_extra::<PlanningPovPolicy>(json!({}));
        rejects_extra::<PlanningPacingPolicy>(json!({}));
        rejects_extra::<PlanningPolicy>(json!({"gates":{}}));
        rejects_extra::<SetPlanningPolicyInput>(json!({
            "session_id":"session-1",
            "policy":{"gates":{}},
            "actor":"author:alice",
            "note":"Accept policy"
        }));
        rejects_extra::<PlanningPacingMetrics>(json!({
            "evaluated_items":0,
            "mean_absolute_deviation":0.0,
            "longest_sustained_deviation":0
        }));
        rejects_extra::<PlanningReadinessInput>(json!({"session_id":"session-1"}));
    }

    fn rejects_at(mut operation: Value, path: &str) {
        assert!(
            serde_json::from_value::<PlanningOperation>(operation.clone()).is_ok(),
            "valid operation fixture must parse"
        );
        operation
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected_field".into(), json!(true));
        assert!(
            serde_json::from_value::<PlanningOperation>(operation).is_err(),
            "unknown nested operation input must fail closed"
        );
    }

    #[test]
    fn operation_tree_rejects_nested_unknowns_before_hashing() {
        rejects_at(
            json!({"operation":"update_reader_contract","input":{"promise":"Discovery"}}),
            "/input",
        );
        rejects_at(
            json!({"operation":"create_project","input":{"name":"Harbor","project_type":"novel","genre":"mystery","reader_contract":{"promise":"Discovery"}}}),
            "/input/reader_contract",
        );
        let character = json!({"operation":"create_character","input":{"name":"Ada","summary":"Seeker","role":"lead","voice_profile":{},"emotional_profile":{"flex_range":{}},"initial_state":{}}});
        for path in [
            "/input/voice_profile",
            "/input/emotional_profile",
            "/input/emotional_profile/flex_range",
            "/input/initial_state",
        ] {
            rejects_at(character.clone(), path);
        }
        let arc = json!({"operation":"create_character_arc","input":{"character_id":{"kind":"proposal_ref","value":"cast:ada"},"arc_type":"growth","starting_state":"Alone","ending_state":"Trusting","milestones":[{"label":"Ask","description":"Seek help","placement":{"book_number":1,"chapter_number":2}}],"thematic_purpose":"Trust"}});
        for path in ["/input/milestones/0", "/input/milestones/0/placement"] {
            rejects_at(arc.clone(), path);
        }
        let conflict = json!({"operation":"create_conflict","input":{"name":"Heir","conflict_type":"mystery","stakes":"Home","try_fail_cycles":[{"attempt_order":1,"label":"Ask","outcome":"Refused"}],"stated_consequences":[{"description":"Exposure","delivered":false,"stated_at":{"book_number":1,"chapter_number":1}}]}});
        for path in [
            "/input/try_fail_cycles/0",
            "/input/stated_consequences/0",
            "/input/stated_consequences/0/stated_at",
        ] {
            rejects_at(conflict.clone(), path);
        }
    }

    #[test]
    fn structured_beats_are_additive_and_legacy_string_beats_still_parse() {
        // Every project can still deserialize a plan that only carries the
        // legacy `beat_structure` strings.
        let legacy: PlanningPlanChapterSceneInput = serde_json::from_value(json!({
            "scene_order": 1,
            "summary": "Ada follows the ledger",
            "purpose": "Discover the forged seal",
            "beat_structure": ["clue", "choice"]
        }))
        .unwrap();
        assert_eq!(legacy.beat_structure, vec!["clue", "choice"]);
        assert!(legacy.beats.is_empty());
        assert_eq!(legacy.turning_beat, None);
        let json = serde_json::to_value(&legacy).unwrap();
        assert!(json.get("beats").is_none());
        assert!(json.get("turning_beat").is_none());

        // A structured Scene carries the typed Beats plus the 1-based Turning
        // Beat position and round-trips through the planning artifact.
        let structured: PlanningPlanChapterSceneInput = serde_json::from_value(json!({
            "scene_order": 1,
            "summary": "Ada follows the ledger",
            "purpose": "Discover the forged seal",
            "beats": [
                {"kind":"action","actor_character_id":{"kind":"proposal_ref","value":"cast:ada"},"tactic":"Ask for the log","said":"Show me the tide log.","wanted":"A clean record","gap":{"expected":"The log clears her","actual":"A page is missing"}},
                {"kind":"reaction","actor_character_id":{"kind":"proposal_ref","value":"cast:ben"},"tactic":"Stall","said":"Sealed until the assizes.","wanted":"Time","gap":{"expected":"Procedure buys a day","actual":"Ada offers her signet"}},
                {"kind":"turn","actor_character_id":{"kind":"proposal_ref","value":"cast:ada"},"tactic":"Wager the signet","said":"Take the signet as surety.","wanted":"Ben's testimony","gap":{"expected":"Ben will testify","actual":"Ben asks for the signet first"},"swain_label":"conflict"}
            ],
            "turning_beat": 3
        }))
        .unwrap();
        assert_eq!(structured.beats.len(), 3);
        assert_eq!(structured.beats[2].kind, crate::beats::BeatKind::Turn);
        assert_eq!(structured.turning_beat, Some(3));
        assert_eq!(structured.beats[2].swain_label.as_deref(), Some("conflict"));
        let json = serde_json::to_value(&structured).unwrap();
        assert_eq!(json["beats"][0]["kind"], "action");
        assert_eq!(
            json["beats"][0]["actor_character_id"],
            json!({"kind":"proposal_ref","value":"cast:ada"})
        );
        assert!(json["beats"][0].get("swain_label").is_none());
    }

    #[test]
    fn key_fact_operation_accepts_only_the_closed_turning_set() {
        let valid = json!({
            "operation": "create_key_fact",
            "input": {
                "kind": "revealed_secret",
                "subject_table": "character",
                "subject_id": {"kind":"proposal_ref","value":"cast:ada"},
                "key": "Ada:forged_manifest",
                "value": "Ada signed the forged manifest",
                "context": "The reveal the mystery promised",
                "placement": {"book_number":1,"chapter_number":1,"scene_order":2},
                "planned_scene_id": {"kind":"proposal_ref","value":"plan:1"}
            }
        });
        let operation: PlanningOperation = serde_json::from_value(valid.clone()).unwrap();
        let PlanningOperation::CreateKeyFact(key_fact) = operation else {
            panic!("create_key_fact must parse as its own operation");
        };
        assert_eq!(key_fact.kind, KeyFactKind::RevealedSecret);
        assert!(key_fact.kind.is_secret_reveal());
        assert_eq!(key_fact.placement.scene_order, Some(2));

        // A non-turning fact type is not a Key Fact type: the operation fails
        // closed at deserialization, before any staging or hashing.
        for non_turning in ["stat_change", "pull_result", "ability_gained"] {
            let mut bad = valid.clone();
            bad["input"]["kind"] = json!(non_turning);
            assert!(
                serde_json::from_value::<PlanningOperation>(bad).is_err(),
                "'{non_turning}' must not be accepted by create_key_fact"
            );
        }
        // Unknown nested fields still fail closed.
        let mut extra = valid;
        extra["input"]["unexpected_field"] = json!(true);
        assert!(serde_json::from_value::<PlanningOperation>(extra).is_err());
    }

    #[test]
    fn promise_payoff_turning_beat_and_target_act_are_additive() {
        // A pre-#28 promise payload keeps its exact shape: neither new key is
        // serialized when unset.
        let legacy: PlanningCreateNarrativePromiseInput = serde_json::from_value(json!({
            "promise_type":"mystery",
            "description":"Who forged the seal?",
            "planted_at":{"book_number":1,"chapter_number":1},
            "planned_payoff":{"book_number":1,"chapter_number":3}
        }))
        .unwrap();
        assert_eq!(legacy.planned_payoff_turning_beat, None);
        assert_eq!(legacy.target_act, None);
        let json = serde_json::to_value(&legacy).unwrap();
        assert!(json.get("planned_payoff_turning_beat").is_none());
        assert!(json.get("target_act").is_none());

        let planned: PlanningCreateNarrativePromiseInput = serde_json::from_value(json!({
            "promise_type":"mystery",
            "description":"Who forged the seal?",
            "planted_at":{"book_number":1,"chapter_number":1,"scene_order":1},
            "planned_payoff":{"book_number":2,"chapter_number":1,"scene_order":2},
            "planned_payoff_turning_beat":3,
            "target_act":2
        }))
        .unwrap();
        assert_eq!(planned.planned_payoff_turning_beat, Some(3));
        assert_eq!(planned.target_act, Some(2));
        let json = serde_json::to_value(&planned).unwrap();
        assert_eq!(json["planned_payoff_turning_beat"], 3);
        assert_eq!(json["target_act"], 2);

        // The same fields are additive on the direct write input, and the
        // direct input's unknown-field rule is unchanged.
        let direct: CreateNarrativePromiseInput = serde_json::from_value(json!({
            "project_id":"project:test",
            "promise_type":"mystery",
            "description":"Who forged the seal?",
            "planted_at":{"book_number":1,"chapter_number":1},
            "planned_payoff":null
        }))
        .unwrap();
        assert_eq!(direct.target_act, None);
        let json = serde_json::to_value(&direct).unwrap();
        assert!(json.get("target_act").is_none());
    }
}

/// A committed operation receipt. IDs include the operation's named output and
/// root aliases `<output>:book`, `<output>:chapter`, `<output>:branch`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlanningOperationReceipt {
    pub proposal_id: String,
    pub operation_index: usize,
    pub input_hash: String,
    pub returned_ids: BTreeMap<String, String>,
    pub actor: String,
    pub decision_note: String,
    pub result: Option<PlanningAppliedOutput>,
    pub prior_state: serde_json::Value,
    pub outcome: PlanningOperationOutcome,
    pub error: Option<String>,
    pub recorded_at: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanningOperationOutcome {
    Applied,
    Failed,
    NotReached,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlanningApplyReport {
    pub attempt_id: String,
    /// Completion of this application batch, never a drafting-readiness verdict.
    pub complete: bool,
    pub proposals: Vec<PlanningProposal>,
    pub operations: Vec<PlanningOperationReceipt>,
    pub error: Option<String>,
    pub pending_search_refreshes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", content = "output", rename_all = "snake_case")]
pub enum PlanningAppliedOutput {
    CreateProject(Box<CreateProjectOutput>),
    CreateCharacter(Box<CreateCharacterOutput>),
    CreateLocation(Box<CreateLocationOutput>),
    CreateWorldRule(Box<CreateWorldRuleOutput>),
    CreateRelationship(Box<CreateRelationshipOutput>),
    CreatePlotLine(Box<CreatePlotLineOutput>),
    CreateConflict(Box<CreateConflictOutput>),
    CreateTheme(Box<CreateThemeOutput>),
    CreateMotif(Box<CreateMotifOutput>),
    CreateNarrativePromise(Box<CreateNarrativePromiseOutput>),
    CreateKeyFact(Box<RegisterCanonicalFactOutput>),
    CreateCharacterArc(Box<CreateCharacterArcOutput>),
    CreatePacingConfig(Box<CreatePacingConfigOutput>),
    CreatePacingCurve(Box<CreatePacingCurveOutput>),
    SetArcPacingConstraints(Box<SetArcPacingConstraintsOutput>),
    CreateBook(Box<CreateBookOutput>),
    CreateChapter(Box<CreateChapterOutput>),
    SetBookOutline(Box<SetBookOutlineOutput>),
    SetChapterOutline(Box<SetChapterOutlineOutput>),
    PlanChapter(Box<PlanChapterOutput>),
    UpdateReaderContract(Box<UpdateEntityOutput>),
    CreateStorySpine(Box<crate::story_spine::CreateStorySpineOutput>),
    CreateCharacterDesign(Box<crate::character_design::CreateCharacterDesignOutput>),
}
