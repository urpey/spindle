//! Scene-level value planning: the Value Axis a planned Scene turns, its Gap,
//! its Inaction Cost and the dilemma that tests a character contradiction.
//!
//! These types are the additive, serde-defaulted value-plan fields shared by
//! the planning operation input and the stored [`crate::models::PlannedScene`].
//! Every field is optional so a pre-existing plan (and every non-McKee plan)
//! serializes exactly as before.
//!
//! The module also owns the typed write-time validity rules — at most two
//! secondary axes, unique non-empty axis names, `risk` in `1..=5`, and
//! non-blank Gap and dilemma texts — plus the turn-direction helper the
//! advisory same-direction run check uses. The value-chain pass itself lives in
//! the adapters because it reads accepted chapter plans and Character Designs.

use crate::acts::charge_depth_rank;
use crate::charge::Charge;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Maximum number of secondary Value Axes a planned Scene may carry.
pub const MAX_SECONDARY_VALUE_AXES: usize = 2;
/// Lowest allowed planned scene risk.
pub const MIN_SCENE_RISK: i32 = 1;
/// Highest allowed planned scene risk.
pub const MAX_SCENE_RISK: i32 = 5;

/// One Value Axis a planned Scene turns, with the Charge it opens and closes at.
///
/// A Scene's primary axis is required for McKee projects; its secondary axes
/// are optional (at most [`MAX_SECONDARY_VALUE_AXES`]). The axis name is a
/// human quality such as "trust", not a record id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneValueAxis {
    /// Value Axis name, for example "trust" or "freedom".
    pub axis: String,
    /// Charge the axis holds when the Scene opens.
    pub opening: Charge,
    /// Charge the axis holds when the Scene closes.
    pub closing: Charge,
}

impl SceneValueAxis {
    /// Whether the axis moves between opening and closing Charge.
    pub fn turns(&self) -> bool {
        self.opening != self.closing
    }

    /// Direction of the turn on McKee's four-step depth spectrum, or `None`
    /// when the axis does not turn (a flat axis, never a non-event by itself
    /// unless it is the primary axis).
    pub fn turn_direction(&self) -> Option<ValueTurnDirection> {
        let opening = charge_depth_rank(self.opening.depth());
        let closing = charge_depth_rank(self.closing.depth());
        match closing.cmp(&opening) {
            std::cmp::Ordering::Greater => Some(ValueTurnDirection::Deeper),
            std::cmp::Ordering::Less => Some(ValueTurnDirection::Shallower),
            std::cmp::Ordering::Equal => None,
        }
    }
}

/// Direction a Value Axis moves in one Scene, read from the four-step depth
/// ranks (positive = 0, contrary = 1, contradictory = 2, negation = 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueTurnDirection {
    /// The axis moves toward a deeper negative state.
    Deeper,
    /// The axis moves back toward a shallower state.
    Shallower,
}

impl fmt::Display for ValueTurnDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deeper => f.write_str("deeper"),
            Self::Shallower => f.write_str("shallower"),
        }
    }
}

/// The Gap a planned Scene designs: what the character expects an action to
/// bring versus what the world actually returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneGap {
    /// What the acting character expects the action to produce.
    pub expected: String,
    /// What the world actually returns.
    pub actual: String,
}

/// The dilemma a planned Scene puts a character under: the contradiction the
/// choice tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneDilemma {
    /// Character record id (or, in a planning artifact, a proposal reference)
    /// whose Character Design contradiction the dilemma tests.
    pub character_id: String,
    /// The contradiction text; it must match one accepted Character Design
    /// contradiction exactly after trimming.
    pub contradiction: String,
}

/// A typed structural error in a planned Scene's value plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneValueError {
    /// More than [`MAX_SECONDARY_VALUE_AXES`] secondary axes were supplied.
    TooManySecondaryAxes {
        /// Number of secondary axes supplied.
        count: usize,
    },
    /// An axis name is blank. `secondary_index` is `None` for the primary axis.
    EmptyValueAxisName {
        /// Index into `secondary_value_axes`; `None` means the primary axis.
        secondary_index: Option<usize>,
    },
    /// Two axes in the same Scene use the same name.
    DuplicateValueAxisName {
        /// The repeated axis name.
        axis: String,
        /// Index into `secondary_value_axes`; `None` means the primary axis.
        secondary_index: Option<usize>,
    },
    /// `risk` is outside `1..=5`.
    RiskOutOfRange {
        /// The supplied risk.
        risk: i32,
    },
    /// The Gap's expected or actual text is blank.
    BlankGap,
    /// The dilemma names no character or no contradiction.
    BlankDilemma,
}

impl fmt::Display for SceneValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManySecondaryAxes { count } => write!(
                f,
                "{count} secondary Value Axes; at most {MAX_SECONDARY_VALUE_AXES} are allowed"
            ),
            Self::EmptyValueAxisName { .. } => f.write_str("a Value Axis name is blank"),
            Self::DuplicateValueAxisName { axis, .. } => {
                write!(f, "the Value Axis name '{axis}' is used more than once")
            }
            Self::RiskOutOfRange { risk } => write!(
                f,
                "risk {risk} is outside {MIN_SCENE_RISK}..={MAX_SCENE_RISK}"
            ),
            Self::BlankGap => f.write_str("the Gap has a blank expected or actual text"),
            Self::BlankDilemma => f.write_str("the dilemma has a blank character or contradiction"),
        }
    }
}

impl std::error::Error for SceneValueError {}

/// Typed structural validity of one planned Scene's value fields.
///
/// This is the shared write-time rule: a direct `plan_chapter` write rejects
/// the same field shapes the planning validator records. A missing primary axis
/// is not a typed error here (non-McKee plans have no value plan); the McKee
/// value-chain pass reports it as a planning requirement instead.
pub fn scene_value_errors(
    primary: Option<&SceneValueAxis>,
    secondary: &[SceneValueAxis],
    risk: Option<i32>,
    gap: Option<&SceneGap>,
    dilemma: Option<(&str, &str)>,
) -> Vec<SceneValueError> {
    let mut errors = Vec::new();
    if secondary.len() > MAX_SECONDARY_VALUE_AXES {
        errors.push(SceneValueError::TooManySecondaryAxes {
            count: secondary.len(),
        });
    }
    let mut seen: Vec<&str> = Vec::new();
    let labeled = primary.map(|axis| (None, axis)).into_iter().chain(
        secondary
            .iter()
            .enumerate()
            .map(|(index, axis)| (Some(index), axis)),
    );
    for (secondary_index, axis) in labeled {
        let name = axis.axis.trim();
        if name.is_empty() {
            errors.push(SceneValueError::EmptyValueAxisName { secondary_index });
            continue;
        }
        if seen.contains(&name) {
            errors.push(SceneValueError::DuplicateValueAxisName {
                axis: name.to_string(),
                secondary_index,
            });
        } else {
            seen.push(name);
        }
    }
    if let Some(risk) = risk
        && !(MIN_SCENE_RISK..=MAX_SCENE_RISK).contains(&risk)
    {
        errors.push(SceneValueError::RiskOutOfRange { risk });
    }
    if let Some(gap) = gap
        && (gap.expected.trim().is_empty() || gap.actual.trim().is_empty())
    {
        errors.push(SceneValueError::BlankGap);
    }
    if let Some((character_id, contradiction)) = dilemma
        && (character_id.trim().is_empty() || contradiction.trim().is_empty())
    {
        errors.push(SceneValueError::BlankDilemma);
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charge::{ChargeDepth, ChargeSign};

    fn charge(sign: ChargeSign, depth: ChargeDepth) -> Charge {
        Charge::new(sign, depth).unwrap()
    }

    fn axis(name: &str, opening: Charge, closing: Charge) -> SceneValueAxis {
        SceneValueAxis {
            axis: name.into(),
            opening,
            closing,
        }
    }

    #[test]
    fn turn_direction_follows_the_four_step_depth_ranks() {
        let deeper = axis(
            "trust",
            charge(ChargeSign::Positive, ChargeDepth::Positive),
            charge(ChargeSign::Negative, ChargeDepth::Contradictory),
        );
        assert!(deeper.turns());
        assert_eq!(deeper.turn_direction(), Some(ValueTurnDirection::Deeper));
        let shallower = axis(
            "trust",
            charge(ChargeSign::Negative, ChargeDepth::Contradictory),
            charge(ChargeSign::Negative, ChargeDepth::Contrary),
        );
        assert_eq!(
            shallower.turn_direction(),
            Some(ValueTurnDirection::Shallower)
        );
        let flat = axis(
            "trust",
            charge(ChargeSign::Negative, ChargeDepth::Contrary),
            charge(ChargeSign::Negative, ChargeDepth::Contrary),
        );
        assert!(!flat.turns());
        assert_eq!(flat.turn_direction(), None);
    }

    #[test]
    fn typed_value_errors_cover_axes_risk_gap_and_dilemma() {
        let positive = charge(ChargeSign::Positive, ChargeDepth::Positive);
        let contrary = charge(ChargeSign::Negative, ChargeDepth::Contrary);
        let bad_axes = vec![
            axis("", positive, contrary),
            axis("trust", positive, contrary),
            axis("trust", positive, contrary),
        ];
        let errors = scene_value_errors(
            Some(&axis("truth", positive, contrary)),
            &bad_axes,
            Some(9),
            Some(&SceneGap {
                expected: "a welcome".into(),
                actual: "  ".into(),
            }),
            Some(("character:1", " ")),
        );
        assert!(errors.contains(&SceneValueError::TooManySecondaryAxes { count: 3 }));
        assert!(errors.contains(&SceneValueError::EmptyValueAxisName {
            secondary_index: Some(0)
        }));
        assert!(errors.contains(&SceneValueError::DuplicateValueAxisName {
            axis: "trust".into(),
            secondary_index: Some(2),
        }));
        assert!(errors.contains(&SceneValueError::RiskOutOfRange { risk: 9 }));
        assert!(errors.contains(&SceneValueError::BlankGap));
        assert!(errors.contains(&SceneValueError::BlankDilemma));
    }

    #[test]
    fn a_clean_value_plan_has_no_typed_errors() {
        let positive = charge(ChargeSign::Positive, ChargeDepth::Positive);
        let contrary = charge(ChargeSign::Negative, ChargeDepth::Contrary);
        let errors = scene_value_errors(
            Some(&axis("truth", positive, contrary)),
            &[axis("trust", positive, contrary)],
            Some(3),
            Some(&SceneGap {
                expected: "a welcome".into(),
                actual: "a refusal".into(),
            }),
            Some(("character:1", "generous")),
        );
        assert!(errors.is_empty(), "{errors:?}");
    }
}
