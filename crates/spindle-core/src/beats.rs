//! Structured Scene Beats: the action, reaction and turn a Scene is built from.
//!
//! A **Beat** is one exchange of action and reaction inside a Scene; in dialogue
//! it is one line used as a tactic to get something (GLOSSARY). McKee's closed
//! set of Beat kinds is [`BeatKind::Action`], [`BeatKind::Reaction`] and
//! [`BeatKind::Turn`]; Swain's categories (goal, conflict, disaster, …) are
//! **optional labels** on a Beat ([`PlannedBeat::swain_label`]), never a
//! structural kind.
//!
//! A planned Scene names its **Turning Beat** with a scene-level
//! `turning_beat`: the 1-based position of the Beat in the Scene's ordered
//! `beats` list. The
//! Turning Beat is the Beat whose Gap turns the Scene's primary Value Axis, so
//! it must be of kind [`BeatKind::Turn`] and there must be exactly one Turn Beat
//! in the Scene.
//!
//! This module owns the typed contract and the write-time shape rules. It holds
//! the resolved form (`actor_character_id` as a record id) shared by the model
//! DTOs and the stored chapter plan; the planning artifact carries the same
//! contract with proposal references and lives in
//! [`crate::planning::PlanningBeat`]. The planning validator (adapters) maps
//! these rules to stage-recordable Blocking findings.
//!
//! `beat_structure: Vec<String>` predates this module and stays exactly as it
//! was: every project can still deserialize it, but for a McKee project the
//! structured Beats are authoritative and the strings are never a substitute.

use crate::scene_value::SceneGap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The closed set of structural Beat kinds.
///
/// Variants deliberately carry no individual doc comments. Per-variant doc
/// comments make schemars emit the closed set as a `oneOf` of const objects,
/// and several clients render that shape poorly. Keeping the variants bare lets
/// the generated schedule stay a plain enum, so every host and transport sees
/// the closed set as one legible list of allowed values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BeatKind {
    Action,
    Reaction,
    Turn,
}

impl BeatKind {
    /// Whether this Beat kind is the Turning Beat kind.
    pub const fn is_turn(self) -> bool {
        matches!(self, Self::Turn)
    }

    /// Lower-case wire spelling, matching the serialized form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Reaction => "reaction",
            Self::Turn => "turn",
        }
    }
}

impl fmt::Display for BeatKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One planned Beat of a Scene, with the actor resolved to a record id.
///
/// Every Beat is an attempt: the actor uses `tactic` to get `wanted`, says
/// `said`, and the world answers with a `gap` between `expected` and `actual`.
/// A Beat with no spoken line uses an empty (or action-only) `said`; the tactic,
/// the want and the Gap are what make it a Beat rather than a summary line.
///
/// `swain_label` is free text only. Swain categories never gain structural
/// meaning, enum membership or validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlannedBeat {
    /// Structural kind of this Beat.
    pub kind: BeatKind,
    /// Character record id that takes this Beat, when the Beat has an actor.
    ///
    /// Spelled out rather than named `actor` because planning payloads are
    /// resolved generically: a bare `actor` key would collide with the human
    /// decision actor and any future operation field of that name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_character_id: Option<String>,
    /// The tactic the actor uses to get something.
    pub tactic: String,
    /// What is said out loud on this Beat (empty for a silent Beat).
    pub said: String,
    /// What the actor actually wants — the subtext behind the tactic.
    pub wanted: String,
    /// What the actor expects this attempt to bring versus what the world
    /// actually returns.
    pub gap: SceneGap,
    /// Optional Swain category label. Never a structural kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swain_label: Option<String>,
}

/// A typed structural error in a Scene's structured Beats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneBeatError {
    /// The Scene has no structured Beats at all.
    NoBeats,
    /// The Scene lists Beats but names no Turning Beat.
    MissingTurningBeat,
    /// `turning_beat` is not the 1-based position of a Beat in `beats`.
    TurningBeatOutOfRange {
        /// The supplied 1-based position.
        turning_beat: u32,
        /// How many Beats the Scene has.
        beats: usize,
    },
    /// The referenced Beat is not of kind [`BeatKind::Turn`].
    TurningBeatNotTurn {
        /// The kind of the referenced Beat.
        kind: BeatKind,
    },
    /// More than one Beat is of kind [`BeatKind::Turn`], so the reference is
    /// ambiguous.
    MultipleTurnBeats {
        /// How many Turn Beats the Scene has.
        count: usize,
    },
}

impl fmt::Display for SceneBeatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoBeats => f.write_str("the Scene has no structured Beats"),
            Self::MissingTurningBeat => f.write_str("the Scene names no Turning Beat"),
            Self::TurningBeatOutOfRange {
                turning_beat,
                beats,
            } => write!(
                f,
                "turning_beat {turning_beat} is not a 1-based position in {beats} Beat(s)"
            ),
            Self::TurningBeatNotTurn { kind } => write!(
                f,
                "the referenced Turning Beat is of kind '{kind}', not 'turn'"
            ),
            Self::MultipleTurnBeats { count } => {
                write!(
                    f,
                    "the Scene has {count} Turn Beats; exactly one is allowed"
                )
            }
        }
    }
}

impl std::error::Error for SceneBeatError {}

/// Typed structural validity of one Scene's structured Beats.
///
/// The rule is McKee-only by caller: non-McKee plans may carry structured Beats
/// without any of this being enforced. `kinds` is the Scene's Beat kinds in
/// story order and `turning_beat` is the 1-based Turning Beat position.
///
/// A Scene with no Beats reports only [`SceneBeatError::NoBeats`] — a missing
/// Turning Beat reference is not reported on top of it. Every other rule is
/// independent, so a Scene can report both a missing reference and more than one
/// Turn Beat.
pub fn scene_beat_errors(kinds: &[BeatKind], turning_beat: Option<u32>) -> Vec<SceneBeatError> {
    if kinds.is_empty() {
        return vec![SceneBeatError::NoBeats];
    }
    let mut errors = Vec::new();
    let turn_count = kinds.iter().filter(|kind| kind.is_turn()).count();
    if turn_count > 1 {
        errors.push(SceneBeatError::MultipleTurnBeats { count: turn_count });
    }
    match turning_beat {
        None => errors.push(SceneBeatError::MissingTurningBeat),
        Some(position) => {
            let index = position.checked_sub(1).map(|index| index as usize);
            match index.and_then(|index| kinds.get(index).map(|kind| (index, *kind))) {
                None => errors.push(SceneBeatError::TurningBeatOutOfRange {
                    turning_beat: position,
                    beats: kinds.len(),
                }),
                Some((_, kind)) if !kind.is_turn() => {
                    errors.push(SceneBeatError::TurningBeatNotTurn { kind });
                }
                Some(_) => {}
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gap() -> SceneGap {
        SceneGap {
            expected: "a welcome".into(),
            actual: "a refusal".into(),
        }
    }

    #[test]
    fn beat_kinds_are_a_closed_three_value_set() {
        for kind in [BeatKind::Action, BeatKind::Reaction, BeatKind::Turn] {
            assert_eq!(
                serde_json::from_str::<BeatKind>(&format!("\"{}\"", kind.as_str())).unwrap(),
                kind
            );
        }
        assert!(serde_json::from_str::<BeatKind>("\"goal\"").is_err());
        assert!(BeatKind::Turn.is_turn());
        assert!(!BeatKind::Reaction.is_turn());
    }

    #[test]
    fn a_beat_round_trips_with_an_optional_actor_character_id_and_swain_label() {
        let beat = PlannedBeat {
            kind: BeatKind::Turn,
            actor_character_id: Some("character:ada".into()),
            tactic: "Wager the signet".into(),
            said: "Then take the signet as surety.".into(),
            wanted: "Ben's testimony".into(),
            gap: gap(),
            swain_label: Some("conflict".into()),
        };
        let json = serde_json::to_value(&beat).unwrap();
        assert_eq!(json["actor_character_id"], "character:ada");
        assert_eq!(serde_json::from_value::<PlannedBeat>(json).unwrap(), beat);
        let anonymous = PlannedBeat {
            actor_character_id: None,
            swain_label: None,
            ..beat
        };
        let json = serde_json::to_value(&anonymous).unwrap();
        assert!(json.get("actor_character_id").is_none());
        assert!(json.get("swain_label").is_none());

        // Unknown fields fail closed, so a Swain category can never sneak in as
        // a second structural kind beside the closed `kind` set.
        assert!(
            serde_json::from_value::<PlannedBeat>(serde_json::json!({
                "kind": "turn",
                "unexpected_field": true,
                "tactic": "Ask",
                "said": "",
                "wanted": "Help",
                "gap": {"expected": "a yes", "actual": "a no"}
            }))
            .is_err()
        );
    }

    #[test]
    fn one_turn_beat_with_a_resolving_reference_has_no_errors() {
        let kinds = [BeatKind::Action, BeatKind::Reaction, BeatKind::Turn];
        assert!(scene_beat_errors(&kinds, Some(3)).is_empty());
    }

    #[test]
    fn missing_and_out_of_range_references_are_typed_errors() {
        let kinds = [BeatKind::Action, BeatKind::Turn];
        assert_eq!(
            scene_beat_errors(&kinds, None),
            vec![SceneBeatError::MissingTurningBeat]
        );
        for position in [0, 3] {
            assert_eq!(
                scene_beat_errors(&kinds, Some(position)),
                vec![SceneBeatError::TurningBeatOutOfRange {
                    turning_beat: position,
                    beats: 2
                }]
            );
        }
    }

    #[test]
    fn a_reference_to_a_non_turn_beat_is_a_typed_error() {
        let kinds = [BeatKind::Action, BeatKind::Turn];
        assert_eq!(
            scene_beat_errors(&kinds, Some(1)),
            vec![SceneBeatError::TurningBeatNotTurn {
                kind: BeatKind::Action
            }]
        );
    }

    #[test]
    fn more_than_one_turn_beat_is_ambiguous() {
        let kinds = [BeatKind::Turn, BeatKind::Action, BeatKind::Turn];
        assert_eq!(
            scene_beat_errors(&kinds, Some(1)),
            vec![SceneBeatError::MultipleTurnBeats { count: 2 }]
        );
    }

    #[test]
    fn a_scene_with_no_beats_reports_only_that_error() {
        assert_eq!(scene_beat_errors(&[], None), vec![SceneBeatError::NoBeats]);
    }
}
