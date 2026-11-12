//! Key Facts: turning canon decided during planning.
//!
//! A **Key Fact** is a canon fact decided during planning because a turning
//! point depends on it (GLOSSARY): a death, a revealed secret, a relationship
//! turn, or an item or power gained or lost. It is not every planned fact — the
//! ordinary mine-then-ratify canon flow keeps deciding those — and it is bound
//! to the planned Scene that must dramatise it.
//!
//! [`KeyFactKind`] is the closed turning-type set. The planning operation
//! `create_key_fact` accepts nothing outside it, so a non-turning fact type can
//! never be smuggled through the Key Fact path. The operation wraps the
//! existing planned-and-pending canonical fact registration (`scene_id: None`,
//! placed by book/chapter/scene): the Scene may still be unplanned, and the
//! fact is attached to the scene later, once the scene row exists, through
//! `bind_canonical_fact_to_scene`.
//!
//! A `revealed_secret` Key Fact is registered as a **secret** canonical fact:
//! its subject character becomes the initial holder (one `knowledge_fact` row
//! linked by `secret_of_fact_id`) and a concealment note explains the planned
//! reveal. The circle expands only at the dramatised reveal, through
//! `record_knowledge` with `secret_of_fact_id`, never at planning time.
//!
//! Variants deliberately carry no individual doc comments so schemars keeps
//! emitting a plain closed enum instead of a `oneOf` of const objects (the same
//! reason [`crate::plot_line::SubplotJob`] keeps its variants bare).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyFactKind {
    Death,
    RevealedSecret,
    RelationshipTurn,
    ItemGained,
    ItemLost,
    PowerGained,
    PowerLost,
}

impl KeyFactKind {
    /// The closed turning-type set, in wire order.
    pub const ALL: [Self; 7] = [
        Self::Death,
        Self::RevealedSecret,
        Self::RelationshipTurn,
        Self::ItemGained,
        Self::ItemLost,
        Self::PowerGained,
        Self::PowerLost,
    ];

    /// Lower-case wire spelling, matching the serialized form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Death => "death",
            Self::RevealedSecret => "revealed_secret",
            Self::RelationshipTurn => "relationship_turn",
            Self::ItemGained => "item_gained",
            Self::ItemLost => "item_lost",
            Self::PowerGained => "power_gained",
            Self::PowerLost => "power_lost",
        }
    }

    /// Whether this Key Fact reveals a secret the reader must already have been
    /// promised (the deus-ex-machina guard).
    pub const fn is_secret_reveal(self) -> bool {
        matches!(self, Self::RevealedSecret)
    }
}

impl fmt::Display for KeyFactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_fact_kinds_are_a_closed_turning_set() {
        for kind in KeyFactKind::ALL {
            let json = format!("\"{}\"", kind.as_str());
            assert_eq!(
                serde_json::from_str::<KeyFactKind>(&json).unwrap(),
                kind,
                "every listed kind round-trips"
            );
        }
        // The closed set covers exactly the documented turning kinds.
        assert_eq!(KeyFactKind::ALL.len(), 7);
        for non_turning in [
            "stat_change",
            "pull_result",
            "ability_gained",
            "relationship_change",
            "note",
            "",
        ] {
            assert!(
                serde_json::from_str::<KeyFactKind>(&format!("\"{non_turning}\"")).is_err(),
                "'{non_turning}' is not a turning Key Fact type"
            );
        }
        assert!(KeyFactKind::RevealedSecret.is_secret_reveal());
        assert!(!KeyFactKind::Death.is_secret_reveal());
        assert_eq!(KeyFactKind::ItemLost.to_string(), "item_lost");
    }
}
