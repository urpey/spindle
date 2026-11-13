//! Realized reports: what a written Scene actually did.
//!
//! A **Realized Report** is the writer's declaration about the prose that now
//! exists: the realized Value Turn on each planned Value Axis (opening and
//! closing **Charge**, the realized **Gap**, the realized **Turning Beat**), the
//! **Character Effect** on each present major character, the **Narrative
//! Promise** events the Scene actually advanced, and which planned **Key Facts**
//! it dramatised. It is a report, never canon: a fact still enters canon through
//! the continuity package and its ratification classes, and a mismatch with the
//! plan becomes a deterministic verify finding that drives the bounded revise
//! loop.
//!
//! The report is carried by both continuity-carrying save paths
//! (`commit_scene`'s [`crate::models::CommitSceneContinuityPackage`] and
//! `authoring_save_scene_draft`'s structured fields), persisted on the Scene row
//! (V0061) and read back additively. Every field is optional and skipped when
//! unset, so a Scene without a report — and every non-McKee project — serializes
//! byte-identically.
//!
//! ## Comparison rules (used by the deterministic scene verify)
//!
//! - **Charge** comparison is exact: the closed [`crate::charge::Charge`] pair
//!   must equal the planned one, with no tolerance.
//! - **Gap** comparison is by presence, not wording: a planned Gap the report
//!   does not mention is a mismatch, while a reported Gap worded differently is
//!   accepted because the Gap is prose the writer legitimately paraphrases.
//! - **Turning Beat** comparison uses the 1-based Beat position when the report
//!   numbers one; a summary-only reference is accepted as reported and is not
//!   position-compared.
//! - Axis names compare exactly after trimming.

use crate::charge::{Charge, ChargeDepth};
use crate::scene_value::SceneGap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One of the four deterministic realized-report verify rules.
///
/// This is the single source of the rule's `check_type` name and of the repair
/// action its suggestions carry, so the exported constants, the requested-check
/// selection and the finding text cannot drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealizedCheck {
    /// Realized versus planned Value Turn.
    ValueTurn,
    /// A planned Key Fact the report does not list.
    KeyFact,
    /// A planned promise event the report does not list.
    PromiseEvent,
    /// Character Effect rules.
    CharacterEffect,
}

impl RealizedCheck {
    /// Every rule, in wire order.
    pub const ALL: [Self; 4] = [
        Self::ValueTurn,
        Self::KeyFact,
        Self::PromiseEvent,
        Self::CharacterEffect,
    ];

    /// The consistency finding `check_type` and scene-verify check name.
    pub const fn check_type(self) -> &'static str {
        match self {
            Self::ValueTurn => "realized_value_turn",
            Self::KeyFact => "realized_key_fact",
            Self::PromiseEvent => "realized_promise_event",
            Self::CharacterEffect => "character_effect",
        }
    }

    /// The repair action a finding for this rule carries.
    pub const fn suggested_action(self) -> &'static str {
        match self {
            Self::ValueTurn => {
                "revise the scene so it realizes the planned Value Turn, or re-plan the scene \
                 (plan amendment) before drafting"
            }
            Self::KeyFact => {
                "dramatise the planned Key Fact in the scene and list its canonical fact id in \
                 the realized report, or re-plan the Key Fact's placement"
            }
            Self::PromiseEvent => {
                "report the promise event the scene actually delivers, or re-plan the promise \
                 placement"
            }
            Self::CharacterEffect => {
                "report 'change' only where a planned change milestone lands, or re-plan the \
                 character arc milestone"
            }
        }
    }
}

/// Deterministic verify check: realized versus planned Value Turn.
pub const REALIZED_VALUE_TURN_CHECK: &str = RealizedCheck::ValueTurn.check_type();
/// Deterministic verify check: a planned Key Fact the report does not list.
pub const REALIZED_KEY_FACT_CHECK: &str = RealizedCheck::KeyFact.check_type();
/// Deterministic verify check: a planned promise event the report does not list.
pub const REALIZED_PROMISE_EVENT_CHECK: &str = RealizedCheck::PromiseEvent.check_type();
/// Deterministic verify check: Character Effect rules.
pub const CHARACTER_EFFECT_CHECK: &str = RealizedCheck::CharacterEffect.check_type();

/// One realized Value Turn on one planned Value Axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RealizedValueTurn {
    /// Value Axis name, matching a planned primary or secondary axis exactly
    /// after trimming.
    pub axis: String,
    /// Charge the axis holds when the written Scene opens.
    pub opening: Charge,
    /// Charge the axis holds when the written Scene closes.
    pub closing: Charge,
    /// The realized Gap (expected versus actual), when the writer reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<SceneGap>,
    /// 1-based position of the realized Turning Beat in the Scene's Beats,
    /// when the writer numbers it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub turning_beat: Option<u32>,
    /// The realized Turning Beat described in words, when it is not numbered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turning_beat_summary: Option<String>,
}

/// What a written Scene did to one character.
///
/// The closed set is `reinforce` (the Scene reinforced what the character
/// already is), `reveal` (the Scene revealed something about the character),
/// `contradict` (the Scene put the character's contradiction under pressure),
/// `change` (the Scene changed the character, earned only at a planned change
/// milestone) and `none` (the Scene left the character as they were).
///
/// The variants deliberately carry no individual doc comments: schemars would
/// emit one `oneOf` const object per variant, which the Gemini schema sanitiser
/// degrades to a plain `object`, so the closed set would vanish from input
/// schemas. Keeping the variants bare emits one closed string enum instead (the
/// same reason `SubplotJob` does it). A planned `change` is never downgraded to
/// `reveal` (GLOSSARY: Character Effect).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CharacterEffect {
    Reinforce,
    Reveal,
    Contradict,
    Change,
    None,
}

impl CharacterEffect {
    /// The closed character-effect set, in wire order.
    pub const ALL: [Self; 5] = [
        Self::Reinforce,
        Self::Reveal,
        Self::Contradict,
        Self::Change,
        Self::None,
    ];

    /// Lower-case wire spelling, matching the serialized form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reinforce => "reinforce",
            Self::Reveal => "reveal",
            Self::Contradict => "contradict",
            Self::Change => "change",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for CharacterEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The Character Effect reported for one present major character.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RealizedCharacterEffect {
    /// Character record id the effect applies to.
    pub character_id: String,
    /// What the Scene did to the character.
    pub effect: CharacterEffect,
    /// Optional note explaining the effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A Narrative Promise event a written Scene actually advanced.
///
/// The closed set is `setup` (the promise was planted), `reinforcement`,
/// `payoff`, `subversion` and `abandonment`.
///
/// The variants deliberately carry no individual doc comments, for the same
/// schema reason as [`CharacterEffect`]: a bare fieldless enum emits one closed
/// string enum and survives the Gemini schema sanitiser, while per-variant docs
/// degrade to a plain `object` in input schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RealizedPromiseEventKind {
    Setup,
    Reinforcement,
    Payoff,
    Subversion,
    Abandonment,
}

impl RealizedPromiseEventKind {
    /// The closed realized promise-event set, in wire order.
    pub const ALL: [Self; 5] = [
        Self::Setup,
        Self::Reinforcement,
        Self::Payoff,
        Self::Subversion,
        Self::Abandonment,
    ];

    /// Lower-case wire spelling, matching the serialized form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Reinforcement => "reinforcement",
            Self::Payoff => "payoff",
            Self::Subversion => "subversion",
            Self::Abandonment => "abandonment",
        }
    }

    /// The Narrative Promise status this confirmed event advances to.
    ///
    /// This is the one mapping the system uses; the status vocabulary is the
    /// repository's existing lower-case one (`planted`, `paid_off`, …).
    pub const fn promise_status(self) -> &'static str {
        match self {
            Self::Setup => "planted",
            Self::Reinforcement => "reinforced",
            Self::Payoff => "paid_off",
            Self::Subversion => "subverted",
            Self::Abandonment => "abandoned",
        }
    }
}

impl std::fmt::Display for RealizedPromiseEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One Narrative Promise event a written Scene actually advanced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RealizedPromiseEvent {
    /// Narrative promise record id.
    pub narrative_promise_id: String,
    /// The event kind the Scene realized.
    pub event: RealizedPromiseEventKind,
    /// Optional note carried onto the promise status event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The realized report of one written Scene.
///
/// Empty sections are skipped, so a Scene with no report and every non-McKee
/// project keep their exact previous serialization.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RealizedReport {
    /// The realized Value Turn on each planned Value Axis.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value_turns: Vec<RealizedValueTurn>,
    /// The Character Effect on each present major character.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub character_effects: Vec<RealizedCharacterEffect>,
    /// The Narrative Promise events this Scene actually advanced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promise_events: Vec<RealizedPromiseEvent>,
    /// Canonical-fact record ids of the planned Key Facts this Scene
    /// dramatised.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_fact_ids: Vec<String>,
}

impl RealizedReport {
    /// True when the report declares nothing at all.
    pub fn is_empty(&self) -> bool {
        self.value_turns.is_empty()
            && self.character_effects.is_empty()
            && self.promise_events.is_empty()
            && self.key_fact_ids.is_empty()
    }

    /// The realized turn reported for one axis, comparing trimmed names.
    pub fn value_turn(&self, axis: &str) -> Option<&RealizedValueTurn> {
        let axis = axis.trim();
        self.value_turns
            .iter()
            .find(|turn| turn.axis.trim() == axis)
    }

    /// The reported Character Effect for one character, if any.
    pub fn character_effect(&self, character_id: &str) -> Option<CharacterEffect> {
        self.character_effects
            .iter()
            .find(|entry| entry.character_id == character_id)
            .map(|entry| entry.effect)
    }

    /// Whether the report lists one event of `kind` on a promise.
    pub fn reports_promise_event(
        &self,
        narrative_promise_id: &str,
        kind: RealizedPromiseEventKind,
    ) -> bool {
        self.promise_events
            .iter()
            .any(|event| event.narrative_promise_id == narrative_promise_id && event.event == kind)
    }

    /// Whether the report lists a planned Key Fact as dramatised.
    pub fn reports_key_fact(&self, fact_id: &str) -> bool {
        self.key_fact_ids.iter().any(|id| id == fact_id)
    }
}

/// Reader-facing label for a Charge, used in verify findings.
pub fn charge_label(charge: Charge) -> &'static str {
    match charge.depth() {
        ChargeDepth::Positive => "positive",
        ChargeDepth::Contrary => "contrary",
        ChargeDepth::Contradictory => "contradictory",
        ChargeDepth::NegationOfTheNegation => "negation of the negation",
    }
}

/// Render one Value Turn as `opening -> closing`, for verify findings.
pub fn format_value_turn(opening: Charge, closing: Charge) -> String {
    format!("{} -> {}", charge_label(opening), charge_label(closing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charge::{ChargeDepth, ChargeSign};

    fn charge(sign: ChargeSign, depth: ChargeDepth) -> Charge {
        Charge::new(sign, depth).unwrap()
    }

    fn positive() -> Charge {
        charge(ChargeSign::Positive, ChargeDepth::Positive)
    }

    fn contrary() -> Charge {
        charge(ChargeSign::Negative, ChargeDepth::Contrary)
    }

    fn contradictory() -> Charge {
        charge(ChargeSign::Negative, ChargeDepth::Contradictory)
    }

    fn turn(axis: &str, opening: Charge, closing: Charge) -> RealizedValueTurn {
        RealizedValueTurn {
            axis: axis.into(),
            opening,
            closing,
            gap: None,
            turning_beat: None,
            turning_beat_summary: None,
        }
    }

    #[test]
    fn an_empty_report_serializes_as_an_empty_object() {
        let report = RealizedReport::default();
        assert!(report.is_empty());
        assert_eq!(serde_json::to_string(&report).unwrap(), "{}");
    }

    #[test]
    fn a_report_round_trips_as_closed_json() {
        let report = RealizedReport {
            value_turns: vec![RealizedValueTurn {
                gap: Some(SceneGap {
                    expected: "a welcome".into(),
                    actual: "a refusal".into(),
                }),
                turning_beat: Some(3),
                turning_beat_summary: Some("Ben refuses the signet".into()),
                ..turn("trust", positive(), contradictory())
            }],
            character_effects: vec![RealizedCharacterEffect {
                character_id: "character:ada".into(),
                effect: CharacterEffect::Change,
                note: Some("Ada asks for help".into()),
            }],
            promise_events: vec![RealizedPromiseEvent {
                narrative_promise_id: "narrative_promise:seal".into(),
                event: RealizedPromiseEventKind::Payoff,
                note: None,
            }],
            key_fact_ids: vec!["canonical_fact:seal".into()],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json["value_turns"][0]["opening"],
            serde_json::json!({"sign":"positive","depth":"positive"})
        );
        assert_eq!(json["value_turns"][0]["closing"]["depth"], "contradictory");
        assert_eq!(json["character_effects"][0]["effect"], "change");
        assert_eq!(json["promise_events"][0]["event"], "payoff");
        assert_eq!(json["key_fact_ids"][0], "canonical_fact:seal");
        let back: RealizedReport = serde_json::from_value(json).unwrap();
        assert_eq!(back, report);
    }

    #[test]
    fn effect_and_promise_event_sets_are_closed_and_wire_stable() {
        for (effect, wire) in [
            (CharacterEffect::Reinforce, "reinforce"),
            (CharacterEffect::Reveal, "reveal"),
            (CharacterEffect::Contradict, "contradict"),
            (CharacterEffect::Change, "change"),
            (CharacterEffect::None, "none"),
        ] {
            assert_eq!(effect.as_str(), wire);
            assert_eq!(effect.to_string(), wire);
            let parsed: CharacterEffect = serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(parsed, effect);
        }
        assert_eq!(CharacterEffect::ALL.len(), 5);
        for (event, wire, status) in [
            (RealizedPromiseEventKind::Setup, "setup", "planted"),
            (
                RealizedPromiseEventKind::Reinforcement,
                "reinforcement",
                "reinforced",
            ),
            (RealizedPromiseEventKind::Payoff, "payoff", "paid_off"),
            (
                RealizedPromiseEventKind::Subversion,
                "subversion",
                "subverted",
            ),
            (
                RealizedPromiseEventKind::Abandonment,
                "abandonment",
                "abandoned",
            ),
        ] {
            assert_eq!(event.as_str(), wire);
            assert_eq!(event.promise_status(), status);
            let parsed: RealizedPromiseEventKind =
                serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(parsed, event);
        }
        assert_eq!(RealizedPromiseEventKind::ALL.len(), 5);
    }

    #[test]
    fn lookups_trim_axis_names_and_scan_the_whole_report() {
        let report = RealizedReport {
            value_turns: vec![
                turn("trust", positive(), contradictory()),
                turn("freedom", contrary(), contradictory()),
            ],
            character_effects: vec![RealizedCharacterEffect {
                character_id: "character:ada".into(),
                effect: CharacterEffect::Reveal,
                note: None,
            }],
            promise_events: vec![
                RealizedPromiseEvent {
                    narrative_promise_id: "narrative_promise:seal".into(),
                    event: RealizedPromiseEventKind::Setup,
                    note: None,
                },
                RealizedPromiseEvent {
                    narrative_promise_id: "narrative_promise:seal".into(),
                    event: RealizedPromiseEventKind::Payoff,
                    note: None,
                },
            ],
            key_fact_ids: vec!["canonical_fact:seal".into()],
        };
        assert!(report.value_turn("  trust ").is_some());
        assert!(report.value_turn("justice").is_none());
        assert_eq!(
            report.character_effect("character:ada"),
            Some(CharacterEffect::Reveal)
        );
        assert_eq!(report.character_effect("character:ben"), None);
        assert!(
            report
                .reports_promise_event("narrative_promise:seal", RealizedPromiseEventKind::Payoff)
        );
        assert!(!report.reports_promise_event(
            "narrative_promise:seal",
            RealizedPromiseEventKind::Abandonment
        ));
        assert!(report.reports_key_fact("canonical_fact:seal"));
        assert!(!report.reports_key_fact("canonical_fact:other"));
        assert!(!report.is_empty());
    }

    #[test]
    fn charges_render_readable_labels_in_findings() {
        assert_eq!(charge_label(positive()), "positive");
        assert_eq!(
            format_value_turn(positive(), contradictory()),
            "positive -> contradictory"
        );
        assert_eq!(
            format_value_turn(
                contrary(),
                charge(ChargeSign::Negative, ChargeDepth::NegationOfTheNegation)
            ),
            "contrary -> negation of the negation"
        );
    }

    #[test]
    fn effect_and_promise_event_schemas_are_one_closed_string_enum() {
        // Per-variant doc comments would emit one `oneOf` const object per
        // variant, which the Gemini schema sanitiser turns into a plain
        // `object`; the closed set must survive as a single string enum.
        for (name, schema) in [
            (
                "character_effect",
                serde_json::to_value(schemars::schema_for!(CharacterEffect)).unwrap(),
            ),
            (
                "realized_promise_event",
                serde_json::to_value(schemars::schema_for!(RealizedPromiseEventKind)).unwrap(),
            ),
        ] {
            assert_eq!(schema["type"], "string", "{name}: {schema}");
            assert!(
                schema.get("oneOf").is_none(),
                "{name} must not emit per-variant oneOf: {schema}"
            );
            assert!(
                schema["enum"]
                    .as_array()
                    .is_some_and(|values| values.len() == 5),
                "{name} must carry the closed five-value set: {schema}"
            );
        }
    }

    #[test]
    fn check_mapping_matches_the_exported_constants() {
        let consts = [
            REALIZED_VALUE_TURN_CHECK,
            REALIZED_KEY_FACT_CHECK,
            REALIZED_PROMISE_EVENT_CHECK,
            CHARACTER_EFFECT_CHECK,
        ];
        for (check, expected) in RealizedCheck::ALL.iter().zip(consts) {
            assert_eq!(check.check_type(), expected);
            assert!(!check.suggested_action().trim().is_empty());
        }
        let names: std::collections::BTreeSet<_> = RealizedCheck::ALL
            .iter()
            .map(|check| check.check_type())
            .collect();
        assert_eq!(names.len(), RealizedCheck::ALL.len());
        let actions: std::collections::BTreeSet<_> = RealizedCheck::ALL
            .iter()
            .map(|check| check.suggested_action())
            .collect();
        assert_eq!(actions.len(), RealizedCheck::ALL.len());
    }
}
