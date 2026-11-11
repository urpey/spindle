//! Character Design contracts: a character's structural blueprint.
//!
//! A Character Design is planning intent owned by a character, alongside its
//! voice and emotional profiles (see [`crate::planning::PlanningOperation::CreateCharacterDesign`]).
//! It records contradictions, the four layers of self, conscious and
//! unconscious desire, the pressure the character puts on the protagonist, and
//! the behaviour cues a writer can dramatise.
//!
//! **Visibility.** The hidden layer and the unconscious desire are stored as
//! secret canonical facts, so their raw text is read only by planning tools
//! (which see the full [`CharacterDesignView`]). Non-planning reads get the
//! [`CharacterDesignRedactedView`] and never the secret fact ids, values or
//! concealment notes. The facts use the reserved predicate namespace below so
//! the context assembly can recognise them and always withhold them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Predicate of the secret canonical fact that stores the hidden layer.
pub const HIDDEN_SELF_FACT_PREDICATE: &str = "character_design.hidden_self";
/// Predicate of the secret canonical fact that stores the unconscious desire.
pub const UNCONSCIOUS_DESIRE_FACT_PREDICATE: &str = "character_design.unconscious_desire";
/// Minimum number of contradictions a Character Design must carry.
pub const MIN_CHARACTER_DESIGN_CONTRADICTIONS: usize = 2;
/// Maximum number of contradictions a Character Design may carry.
pub const MAX_CHARACTER_DESIGN_CONTRADICTIONS: usize = 5;

/// Whether a canonical-fact predicate belongs to the Character Design secret
/// namespace. These facts are authored for planning only: every non-planning
/// read (subject snapshots and scene context) strips them instead of rendering
/// even the `[SECRETS IN PLAY]` envelope.
pub fn is_character_design_fact(predicate: &str) -> bool {
    predicate == HIDDEN_SELF_FACT_PREDICATE || predicate == UNCONSCIOUS_DESIRE_FACT_PREDICATE
}

/// Count the contradictions that carry text; whitespace-only entries are not
/// contradictions and are ignored by the 2–5 rule everywhere.
pub fn contradiction_count(contradictions: &[String]) -> usize {
    contradictions
        .iter()
        .filter(|contradiction| !contradiction.trim().is_empty())
        .count()
}

/// Whether `count` satisfies the 2–5 contradictions rule.
pub fn valid_contradiction_count(count: usize) -> bool {
    (MIN_CHARACTER_DESIGN_CONTRADICTIONS..=MAX_CHARACTER_DESIGN_CONTRADICTIONS).contains(&count)
}

/// Persisted Character Design. The hidden layer and the unconscious desire are
/// not duplicated here: `hidden_self_fact_id` and
/// `unconscious_desire_fact_id` point at their secret canonical facts, which
/// remain the single source of truth for those texts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CharacterDesign {
    /// Character Design record id (`character_design:...`).
    pub character_design_id: String,
    /// Project that owns the design.
    pub project_id: String,
    /// Branch the character lives on.
    pub branch_id: String,
    /// Character this design belongs to (one design per character).
    pub character_id: String,
    /// Two to five contradictions that give the character dimension.
    pub contradictions: Vec<String>,
    /// The self the character shows the world.
    pub public_self: String,
    /// The self the character shares with intimates.
    pub personal_self: String,
    /// The self the character keeps to themselves.
    pub private_self: String,
    /// The self the character hides even from themselves; stored as a secret
    /// canonical fact, never as plain text on this row.
    pub hidden_self_fact_id: String,
    /// What the character wants on the page.
    pub conscious_desire: String,
    /// What the character wants beneath the page; stored as a secret canonical
    /// fact, never as plain text on this row.
    pub unconscious_desire_fact_id: String,
    /// The pressure this character puts on the protagonist.
    pub pressure_on_protagonist: String,
    /// Planner-written observable behaviour cues derived from the hidden layer
    /// and the unconscious desire. These are the writer-facing rendering of the
    /// two secret fields.
    pub behavior_cues: Vec<String>,
    /// Creation time in microseconds since the Unix epoch.
    pub created_at: i64,
    /// Last update time in microseconds since the Unix epoch.
    pub updated_at: i64,
}

/// Resolved create input used by the service/repository write path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCharacterDesignInput {
    /// Project that owns the design.
    pub project_id: String,
    /// Character this design belongs to.
    pub character_id: String,
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
    /// The self the character hides even from themselves.
    #[serde(default)]
    pub hidden_self: String,
    /// What the character wants on the page.
    #[serde(default)]
    pub conscious_desire: String,
    /// What the character wants beneath the page.
    #[serde(default)]
    pub unconscious_desire: String,
    /// The pressure this character puts on the protagonist.
    #[serde(default)]
    pub pressure_on_protagonist: String,
    /// Planner-written observable behaviour cues.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub behavior_cues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCharacterDesignOutput {
    /// Character Design record id (`character_design:...`).
    pub character_design_id: String,
}

/// Full Character Design read model for planning tools. It carries the hidden
/// layer and the unconscious desire texts resolved from their secret facts;
/// their fact ids stay internal. Never return this shape from a non-planning
/// read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CharacterDesignView {
    /// Character Design record id (`character_design:...`).
    pub character_design_id: String,
    /// Character this design belongs to.
    pub character_id: String,
    /// Two to five contradictions that give the character dimension.
    pub contradictions: Vec<String>,
    /// The self the character shows the world.
    pub public_self: String,
    /// The self the character shares with intimates.
    pub personal_self: String,
    /// The self the character keeps to themselves.
    pub private_self: String,
    /// The self the character hides even from themselves.
    pub hidden_self: String,
    /// What the character wants on the page.
    pub conscious_desire: String,
    /// What the character wants beneath the page.
    pub unconscious_desire: String,
    /// The pressure this character puts on the protagonist.
    pub pressure_on_protagonist: String,
    /// Planner-written observable behaviour cues.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub behavior_cues: Vec<String>,
}

/// Redacted Character Design slice for non-planning reads (for example
/// `get_character_snapshot`). It deliberately has no hidden-layer or
/// unconscious-desire fields, no secret fact ids and no concealment notes, so
/// there is nothing to leak when it serializes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CharacterDesignRedactedView {
    /// Character Design record id (`character_design:...`).
    pub character_design_id: String,
    /// Character this design belongs to.
    pub character_id: String,
    /// Two to five contradictions that give the character dimension.
    pub contradictions: Vec<String>,
    /// The self the character shows the world.
    pub public_self: String,
    /// The self the character shares with intimates.
    pub personal_self: String,
    /// The self the character keeps to themselves.
    pub private_self: String,
    /// What the character wants on the page.
    pub conscious_desire: String,
    /// The pressure this character puts on the protagonist.
    pub pressure_on_protagonist: String,
    /// Observable behaviour cues the writer should dramatise instead of naming
    /// the hidden layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub behavior_cues: Vec<String>,
}

impl CharacterDesignRedactedView {
    /// Project the public half of a persisted design. The hidden and
    /// unconscious texts are not reachable from this constructor, so a
    /// redacted read cannot leak them by construction.
    pub fn from_design(design: &CharacterDesign) -> Self {
        Self {
            character_design_id: design.character_design_id.clone(),
            character_id: design.character_id.clone(),
            contradictions: design.contradictions.clone(),
            public_self: design.public_self.clone(),
            personal_self: design.personal_self.clone(),
            private_self: design.private_self.clone(),
            conscious_desire: design.conscious_desire.clone(),
            pressure_on_protagonist: design.pressure_on_protagonist.clone(),
            behavior_cues: design.behavior_cues.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_secret_predicate_namespace_is_closed() {
        assert!(is_character_design_fact(HIDDEN_SELF_FACT_PREDICATE));
        assert!(is_character_design_fact(UNCONSCIOUS_DESIRE_FACT_PREDICATE));
        for other in [
            "",
            "character_design",
            "character_design.public_self",
            "character_design.hidden_self ",
            "hidden_self",
            "secret:character_design.hidden_self",
        ] {
            assert!(!is_character_design_fact(other), "{other:?}");
        }
    }

    #[test]
    fn contradiction_rule_ignores_blank_entries() {
        let text = |values: &[&str]| values.iter().map(|v| v.to_string()).collect::<Vec<_>>();
        assert_eq!(contradiction_count(&text(&["generous", " "])), 1);
        assert!(!valid_contradiction_count(contradiction_count(&text(&[
            "generous"
        ]))));
        assert!(valid_contradiction_count(contradiction_count(&text(&[
            "generous", "ruthless"
        ]))));
        assert!(valid_contradiction_count(contradiction_count(&text(&[
            "a", "b", "c", "d", "e"
        ]))));
        assert!(!valid_contradiction_count(contradiction_count(&text(&[
            "a", "b", "c", "d", "e", "f"
        ]))));
    }

    #[test]
    fn the_redacted_view_has_no_hidden_layer_surface() {
        let design = CharacterDesign {
            character_design_id: "character_design:1".into(),
            project_id: "project:1".into(),
            branch_id: "bible_branch:1".into(),
            character_id: "character:1".into(),
            contradictions: vec!["kind".into(), "cold".into()],
            public_self: "Harbour archivist".into(),
            personal_self: "Loyal to her sister".into(),
            private_self: "Fears the water".into(),
            hidden_self_fact_id: "canonical_fact:hidden".into(),
            conscious_desire: "Find the heir".into(),
            unconscious_desire_fact_id: "canonical_fact:unconscious".into(),
            pressure_on_protagonist: "Makes the protagonist choose".into(),
            behavior_cues: vec!["checks the tide twice before speaking".into()],
            created_at: 1,
            updated_at: 2,
        };
        let redacted = CharacterDesignRedactedView::from_design(&design);
        let json = serde_json::to_value(&redacted).unwrap();
        let text = json.to_string();
        assert!(!text.contains("canonical_fact:hidden"));
        assert!(!text.contains("canonical_fact:unconscious"));
        assert!(!text.contains("hidden_self"));
        assert!(!text.contains("unconscious_desire"));
        assert_eq!(json["public_self"], "Harbour archivist");
        assert_eq!(json["contradictions"][0], "kind");
    }
}
