//! Typed value-axis Charge: a sign plus McKee's four-step depth spectrum.
//!
//! The type is closed on purpose. A positive Charge is only the positive step;
//! every deeper step is a negative value state. Constructing the pair any other
//! way is a validation error, not a value to interpret later.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::fmt;

/// The direction a Value Axis currently holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChargeSign {
    Positive,
    Negative,
}

/// McKee's four-step value spectrum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChargeDepth {
    Positive,
    Contrary,
    Contradictory,
    NegationOfTheNegation,
}

impl fmt::Display for ChargeSign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Positive => f.write_str("positive"),
            Self::Negative => f.write_str("negative"),
        }
    }
}

impl fmt::Display for ChargeDepth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Positive => f.write_str("positive"),
            Self::Contrary => f.write_str("contrary"),
            Self::Contradictory => f.write_str("contradictory"),
            Self::NegationOfTheNegation => f.write_str("negation_of_the_negation"),
        }
    }
}

/// A Value Axis Charge at one moment.
///
/// The fields are private so a caller cannot assemble an invalid pair; use
/// [`Charge::new`] or deserialize one of the four valid representations:
///
/// ```json
/// {"sign":"positive","depth":"positive"}
/// {"sign":"negative","depth":"contrary"}
/// {"sign":"negative","depth":"contradictory"}
/// {"sign":"negative","depth":"negation_of_the_negation"}
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Charge {
    sign: ChargeSign,
    depth: ChargeDepth,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeRepr {
    sign: ChargeSign,
    depth: ChargeDepth,
}

impl<'de> Deserialize<'de> for Charge {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let repr = ChargeRepr::deserialize(deserializer)?;
        Charge::new(repr.sign, repr.depth).map_err(de::Error::custom)
    }
}

impl Charge {
    /// Build a valid Charge, or explain why the sign/depth pair is impossible.
    pub fn new(sign: ChargeSign, depth: ChargeDepth) -> Result<Self, ChargeError> {
        match (sign, depth) {
            (ChargeSign::Positive, ChargeDepth::Positive)
            | (
                ChargeSign::Negative,
                ChargeDepth::Contrary
                | ChargeDepth::Contradictory
                | ChargeDepth::NegationOfTheNegation,
            ) => Ok(Self { sign, depth }),
            (ChargeSign::Positive, _) => Err(ChargeError::PositiveSignRequiresPositiveDepth),
            (ChargeSign::Negative, ChargeDepth::Positive) => {
                Err(ChargeError::NegativeSignCannotBePositiveDepth)
            }
        }
    }

    pub const fn sign(self) -> ChargeSign {
        self.sign
    }

    pub const fn depth(self) -> ChargeDepth {
        self.depth
    }

    /// Validate an already-built Charge. Kept for callers that received one
    /// through a boundary which does not deserialize through serde.
    pub fn validate(self) -> Result<Self, ChargeError> {
        Self::new(self.sign, self.depth)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeError {
    PositiveSignRequiresPositiveDepth,
    NegativeSignCannotBePositiveDepth,
}

impl fmt::Display for ChargeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PositiveSignRequiresPositiveDepth => {
                write!(f, "charge depth positive requires sign positive")
            }
            Self::NegativeSignCannotBePositiveDepth => {
                write!(f, "charge sign negative requires a depth below positive")
            }
        }
    }
}

impl std::error::Error for ChargeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_charge_pairs_round_trip_as_closed_json() {
        for (sign, depth, text) in [
            (
                "positive",
                "positive",
                r#"{"sign":"positive","depth":"positive"}"#,
            ),
            (
                "negative",
                "contrary",
                r#"{"sign":"negative","depth":"contrary"}"#,
            ),
            (
                "negative",
                "contradictory",
                r#"{"sign":"negative","depth":"contradictory"}"#,
            ),
            (
                "negative",
                "negation_of_the_negation",
                r#"{"sign":"negative","depth":"negation_of_the_negation"}"#,
            ),
        ] {
            let charge: Charge = serde_json::from_str(text).unwrap();
            assert_eq!(charge.sign().to_string(), sign);
            assert_eq!(charge.depth().to_string(), depth);
            assert_eq!(serde_json::to_string(&charge).unwrap(), text);
        }
    }

    #[test]
    fn invalid_charge_pairs_are_rejected_on_construction_and_deserialization() {
        assert!(
            Charge::new(ChargeSign::Positive, ChargeDepth::Contrary).is_err(),
            "a positive sign may only hold the positive depth"
        );
        assert!(
            Charge::new(ChargeSign::Negative, ChargeDepth::Positive).is_err(),
            "the positive depth may only hold a positive sign"
        );
        for value in [
            json!({"sign":"positive","depth":"contrary"}),
            json!({"sign":"positive","depth":"contradictory"}),
            json!({"sign":"positive","depth":"negation_of_the_negation"}),
            json!({"sign":"negative","depth":"positive"}),
        ] {
            assert!(
                serde_json::from_value::<Charge>(value).is_err(),
                "invalid depth/sign pair must not deserialize"
            );
        }
    }

    #[test]
    fn unknown_or_incomplete_charge_shapes_fail_closed() {
        for text in [
            r#"{"sign":"positive"}"#,
            r#"{"depth":"positive"}"#,
            r#"{"sign":"positive","depth":"positive","extra":true}"#,
            r#"{"sign":"sideways","depth":"positive"}"#,
        ] {
            assert!(serde_json::from_str::<Charge>(text).is_err());
        }
    }
}
