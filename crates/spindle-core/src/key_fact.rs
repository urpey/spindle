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
//!
//! **Predicate namespace.** A Key Fact persists as a `canonical_fact` row like
//! every other fact, so it needs a durable marker that survives restarts and
//! needs no new table. The marker is the reserved predicate namespace
//! `key_fact.<kind>:<author key>`: the `create_key_fact` apply path writes it,
//! and writer-facing read paths use it to recognise a due Key Fact. The author's
//! own `key` is preserved verbatim after the namespace, and
//! [`canonical_fact_display_predicate`] strips the namespace from writer-facing
//! text. This mirrors the Character Design predicate namespace in
//! [`crate::character_design`]: the reserved prefix is the marker, so ordinary
//! mine-then-ratify facts are untouched and are never mistaken for Key Facts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Reserved predicate prefix that marks a canonical fact as a Key Fact.
pub const KEY_FACT_PREDICATE_PREFIX: &str = "key_fact.";

/// The canonical-fact predicate of a Key Fact: `key_fact.<kind>:<author key>`.
///
/// The author key is kept verbatim (it may itself contain `:`), so
/// [`key_fact_display_predicate`] can recover it exactly.
pub fn key_fact_predicate(kind: KeyFactKind, key: &str) -> String {
    format!(
        "{KEY_FACT_PREDICATE_PREFIX}{}:{}",
        kind.as_str(),
        key.trim()
    )
}

/// Whether a canonical-fact predicate belongs to the reserved Key Fact
/// namespace. Ordinary facts never match.
pub fn is_key_fact_predicate(predicate: &str) -> bool {
    predicate.starts_with(KEY_FACT_PREDICATE_PREFIX)
}

/// The Key Fact kind encoded in a reserved predicate, when it parses.
pub fn key_fact_kind_from_predicate(predicate: &str) -> Option<KeyFactKind> {
    let rest = predicate.strip_prefix(KEY_FACT_PREDICATE_PREFIX)?;
    let kind = rest.split_once(':').map_or(rest, |(kind, _)| kind);
    KeyFactKind::ALL
        .into_iter()
        .find(|candidate| candidate.as_str() == kind)
}

/// The author-facing predicate of a Key Fact: the author's `key` without the
/// reserved namespace, or the predicate unchanged when it is not a well-formed
/// Key Fact predicate.
pub fn key_fact_display_predicate(predicate: &str) -> &str {
    let Some(rest) = predicate.strip_prefix(KEY_FACT_PREDICATE_PREFIX) else {
        return predicate;
    };
    match rest.split_once(':') {
        Some((_, key)) if !key.trim().is_empty() => key,
        _ => predicate,
    }
}

/// The predicate to show in writer-facing text: Key Facts lose the reserved
/// namespace so the author's key is what the writer sees; every other predicate
/// is returned unchanged.
pub fn canonical_fact_display_predicate(predicate: &str) -> &str {
    if is_key_fact_predicate(predicate) {
        key_fact_display_predicate(predicate)
    } else {
        predicate
    }
}

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

    #[test]
    fn key_fact_predicates_use_the_reserved_namespace_and_keep_the_author_key() {
        let predicate = key_fact_predicate(KeyFactKind::ItemGained, "Mara:lantern_key");
        assert_eq!(predicate, "key_fact.item_gained:Mara:lantern_key");
        assert!(is_key_fact_predicate(&predicate));
        assert_eq!(
            key_fact_kind_from_predicate(&predicate),
            Some(KeyFactKind::ItemGained)
        );
        // The author key may itself contain ':' and is preserved verbatim.
        assert_eq!(key_fact_display_predicate(&predicate), "Mara:lantern_key");
        assert_eq!(
            canonical_fact_display_predicate(&predicate),
            "Mara:lantern_key"
        );
        // Whitespace around the author key is not part of the persisted key.
        assert_eq!(
            key_fact_predicate(KeyFactKind::Death, "  Kael falls  "),
            "key_fact.death:Kael falls"
        );
    }

    #[test]
    fn ordinary_facts_are_never_key_facts() {
        for predicate in [
            "Mara:lantern_key",
            "character_design.hidden_self",
            "keyfacts",
        ] {
            assert!(!is_key_fact_predicate(predicate), "{predicate}");
            assert_eq!(
                canonical_fact_display_predicate(predicate),
                predicate,
                "an ordinary predicate is untouched"
            );
        }
        // A malformed reserved predicate stays visible rather than losing its
        // text: the writer must never see an empty predicate.
        assert_eq!(
            canonical_fact_display_predicate("key_fact.death"),
            "key_fact.death"
        );
        assert_eq!(
            canonical_fact_display_predicate("key_fact.death:"),
            "key_fact.death:"
        );
        assert_eq!(key_fact_kind_from_predicate("key_fact.nope:x"), None);
    }
}
