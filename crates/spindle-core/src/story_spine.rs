//! Story Spine contracts: one series spine and optional per-book sub-spines.
//!
//! A Story Spine is planning intent, not realized canon. It is created through
//! the contract planning level and read back through planning/entity reads.

use crate::models::StoryPlacement;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The ending Charge the final climax is designed to prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StorySpineEnding {
    Idealist,
    Pessimist,
    Ironic,
}

/// A Controlling Idea is a claim, written as a value and the cause that proves
/// it at the climax. Empty strings are valid wire values so an incomplete staged
/// spine can be reported as a blocking planning finding instead of disappearing
/// as a deserialization error.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ControllingIdea {
    /// The value the climax proves (for example "trust").
    #[serde(default)]
    pub value: String,
    /// The cause that proves the value at the climax.
    #[serde(default)]
    pub cause: String,
}

/// Resolved create input used by the service/repository write path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateStorySpineInput {
    /// Project that owns the spine.
    pub project_id: String,
    /// `None` creates the one series spine; `Some` creates a book sub-spine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_spine_id: Option<String>,
    /// Required for a book sub-spine, absent for the series spine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_number: Option<i32>,
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
    pub inciting_incident: Option<StoryPlacement>,
    /// Where the final climax is placed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_climax: Option<StoryPlacement>,
    /// Themes this spine references; it never replaces them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub theme_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateStorySpineOutput {
    /// Story Spine record id (`story_spine:...`).
    pub story_spine_id: String,
}

/// Persisted Story Spine read model. `series_spine_id` is `None` for the one
/// series spine and references it for a per-book sub-spine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StorySpine {
    /// Story Spine record id (`story_spine:...`).
    pub story_spine_id: String,
    /// Project that owns the spine.
    pub project_id: String,
    /// Series spine this book sub-spine references; absent on the series spine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_spine_id: Option<String>,
    /// Book this sub-spine covers; absent on the series spine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_number: Option<i32>,
    /// The question the inciting incident raises and the final climax answers.
    pub central_question: String,
    /// The claim the story proves: a value plus its cause.
    pub controlling_idea: ControllingIdea,
    /// The opposing idea the story argues against.
    pub counter_idea: String,
    /// The Value Axis the whole spine turns on.
    pub spine_axis: String,
    /// Whether the ending is idealist, pessimist, or ironic.
    pub ending_type: StorySpineEnding,
    /// Where the inciting incident is placed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inciting_incident: Option<StoryPlacement>,
    /// Where the final climax is placed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_climax: Option<StoryPlacement>,
    /// Themes this spine references; it never replaces them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub theme_ids: Vec<String>,
    /// Creation time in microseconds since the Unix epoch.
    pub created_at: i64,
    /// Last update time in microseconds since the Unix epoch.
    pub updated_at: i64,
}

impl StorySpine {
    pub fn is_series_spine(&self) -> bool {
        self.series_spine_id.is_none()
    }
}
