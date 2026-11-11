//! Main Plot / Subplot structure carried by a plot line.
//!
//! A Main Plot is the one plot line whose Value Axis is the Story Spine's
//! Spine Axis. A Subplot is any other plot line and carries exactly one
//! structural job toward the Controlling Idea. These are planning intent, not
//! realized canon; the fields are additive so existing plot lines keep their
//! previous wire shape.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The one structural job a Subplot performs toward the Controlling Idea:
/// contradict it, resonate with it, set up the Main Plot, or complicate it.
///
/// Variants deliberately carry no individual doc comments. Per-variant doc
/// comments make schemars emit the closed set as a `oneOf` of const objects,
/// and several clients render that shape poorly. Keeping the variants bare
/// lets the generated schema stay a plain enum, so every host and transport
/// sees the closed set as one legible list of allowed values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubplotJob {
    Contradict,
    Resonate,
    SetUp,
    Complicate,
}

impl SubplotJob {
    /// The closed set, in wire/DB order.
    pub const ALL: [SubplotJob; 4] = [
        SubplotJob::Contradict,
        SubplotJob::Resonate,
        SubplotJob::SetUp,
        SubplotJob::Complicate,
    ];

    /// Stable snake_case spelling used by JSON, planning findings and SQLite.
    pub const fn as_str(self) -> &'static str {
        match self {
            SubplotJob::Contradict => "contradict",
            SubplotJob::Resonate => "resonate",
            SubplotJob::SetUp => "set_up",
            SubplotJob::Complicate => "complicate",
        }
    }

    /// Parse a stored/wire spelling; `None` for anything outside the closed set.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "contradict" => Some(SubplotJob::Contradict),
            "resonate" => Some(SubplotJob::Resonate),
            "set_up" => Some(SubplotJob::SetUp),
            "complicate" => Some(SubplotJob::Complicate),
            _ => None,
        }
    }
}

impl fmt::Display for SubplotJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The Value Axis comparison used by the Main Plot gate: trim only, then exact
/// match. A Value Axis keeps the author's spelling and capitalisation; only
/// accidental surrounding whitespace is ignored.
pub fn value_axis_matches(main_plot_axis: Option<&str>, spine_axis: &str) -> bool {
    main_plot_axis.map(str::trim) == Some(spine_axis.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subplot_jobs_round_trip_as_closed_snake_case_strings() {
        for job in SubplotJob::ALL {
            let text = serde_json::to_string(&job).unwrap();
            assert_eq!(text, format!("\"{}\"", job.as_str()));
            assert_eq!(SubplotJob::parse(job.as_str()), Some(job));
            assert_eq!(format!("{job}"), job.as_str());
            assert_eq!(serde_json::from_str::<SubplotJob>(&text).unwrap(), job);
        }
        for unknown in ["", "setup", "set-up", "Contradict", "side_plot"] {
            assert_eq!(SubplotJob::parse(unknown), None);
            assert!(serde_json::from_str::<SubplotJob>(&format!("\"{unknown}\"")).is_err());
        }
    }

    #[test]
    fn value_axis_comparison_trims_but_stays_exact() {
        assert!(value_axis_matches(Some(" trust "), "trust"));
        assert!(value_axis_matches(Some("trust"), " trust\t"));
        assert!(value_axis_matches(Some(""), ""));
        assert!(!value_axis_matches(None, "trust"));
        assert!(!value_axis_matches(Some("Trust"), "trust"));
        assert!(!value_axis_matches(Some("trust "), "betrayal"));
        assert!(!value_axis_matches(Some("trust/betrayal"), "trust"));
    }
}
