//! McKee scene-critic rubric, structured verdicts and the fixed-output parser.
//!
//! ## Independence (ADR 0005)
//!
//! The critic judges the *prose* against the **plan**. It is never given the
//! writer's realized report, because a critic that receives the writer's
//! declared verdict can only agree with it. The structured verdict says what the
//! prose actually did on the two dimensions where a deterministic rule cannot
//! reach: the realized **Value Turn** and the realized **Narrative Promise**
//! events. The writer's report is compared with that independent verdict by the
//! deterministic `critic_report_disagreement` scene-verify check; a difference
//! is a finding.
//!
//! ## Rubric versions
//!
//! [`SCENE_CRITIC_RUBRIC_VERSION`] (`scene-critic-v1`) is the rubric every
//! existing project keeps, byte-identically. [`MCKEE_SCENE_CRITIC_RUBRIC_VERSION`]
//! (`scene-critic-mckee-v1`) adds the four McKee dimensions: Value Turn
//! realized, Gap lands, exposition used as ammunition rather than explanation,
//! and over-obvious foreshadowing. Only a McKee-structured project selects it.
//!
//! ## Exhaustive reality
//!
//! Every field is optional where a caller may legitimately have nothing to say,
//! and the whole verdict is absent when the critic produced no usable output.
//! A missing verdict is **not** a mismatch: it is an unavailable pass, and the
//! verify check stays silent rather than inventing a disagreement.

use crate::model_output::extract_trailing_json_object;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The rubric every non-McKee project keeps, byte-identically.
pub const SCENE_CRITIC_RUBRIC_VERSION: &str = "scene-critic-v1";
/// The McKee rubric: the base scene-critic rubric plus four McKee dimensions.
pub const MCKEE_SCENE_CRITIC_RUBRIC_VERSION: &str = "scene-critic-mckee-v1";

/// The marker the critic is asked to put before its structured verdict JSON.
pub const MCKEE_CRITIC_VERDICT_MARKER: &str = "CRITIC VERDICT";

/// One of the four McKee craft dimensions the critic rubric adds:
/// `value_turn_realized` (did the written Scene turn the planned Value Axis),
/// `gap_lands` (did the character's expectation break against reality),
/// `exposition_as_ammunition` (is exposition used in conflict, not explained to
/// the reader) and `over_obvious_foreshadowing` (is a setup so obvious that the
/// reader sees the payoff coming).
///
/// The variants deliberately carry no individual doc comments: schemars would
/// emit one `oneOf` const object per variant, which the Gemini schema sanitiser
/// degrades to a plain `object`, so the closed set would vanish from any schema
/// used as input. The same reason `CharacterEffect` does it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum McKeeCriticDimension {
    ValueTurnRealized,
    GapLands,
    ExpositionAsAmmunition,
    OverObviousForeshadowing,
}

impl McKeeCriticDimension {
    /// Every McKee dimension, in wire order.
    pub const ALL: [Self; 4] = [
        Self::ValueTurnRealized,
        Self::GapLands,
        Self::ExpositionAsAmmunition,
        Self::OverObviousForeshadowing,
    ];

    /// Lower-case wire spelling, matching the serialized form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ValueTurnRealized => "value_turn_realized",
            Self::GapLands => "gap_lands",
            Self::ExpositionAsAmmunition => "exposition_as_ammunition",
            Self::OverObviousForeshadowing => "over_obvious_foreshadowing",
        }
    }
}

impl std::fmt::Display for McKeeCriticDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The rubric version a project selects.
pub const fn scene_critic_rubric_version(mckee_structured: bool) -> &'static str {
    if mckee_structured {
        MCKEE_SCENE_CRITIC_RUBRIC_VERSION
    } else {
        SCENE_CRITIC_RUBRIC_VERSION
    }
}

/// A 1..=5 diagnostic score plus the critic's reason for one McKee dimension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McKeeCriticDimensionScore {
    /// Which McKee dimension this score is for.
    pub dimension: McKeeCriticDimension,
    /// Diagnostic score, `1..=5`. Higher is better.
    #[schemars(range(min = 1, max = 5))]
    pub score_1_to_5: u8,
    /// Why the critic scored it that way.
    pub rationale: String,
}

/// The critic's structured judgement on one dimension: agree or mismatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CriticVerdictKind {
    Agree,
    Mismatch,
}

impl CriticVerdictKind {
    /// The closed verdict set, in wire order.
    pub const ALL: [Self; 2] = [Self::Agree, Self::Mismatch];

    /// Lower-case wire spelling, matching the serialized form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Agree => "agree",
            Self::Mismatch => "mismatch",
        }
    }
}

/// Where a persisted critic verdict came from. Only `route` may gate
/// independence; `host` is the `critic_output` offline/test seam (which also
/// carries fixed fixtures) and is ignored by gating.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CriticVerdictSource {
    #[default]
    Route,
    Host,
}

impl CriticVerdictSource {
    /// The wire spelling, matching the serialized form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Route => "route",
            Self::Host => "host",
        }
    }

    /// True for the only source that may gate independence.
    pub const fn is_route(self) -> bool {
        matches!(self, Self::Route)
    }

    /// `serde` skip predicate: a route-produced verdict is the historical
    /// default, so its marker is never serialized (existing payloads and route
    /// rounds keep their exact bytes). A host verdict is serialized.
    pub fn skip_if_route(value: &Option<Self>) -> bool {
        value.is_none_or(|source| source.is_route())
    }
}

/// One structured critic judgement with optional notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CriticJudgement {
    /// Agree or mismatch.
    pub verdict: CriticVerdictKind,
    /// Why, in the critic's own words. Never prose from the Scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl CriticJudgement {
    /// True when the critic judged that the prose realizes the plan.
    pub fn is_agreement(&self) -> bool {
        self.verdict == CriticVerdictKind::Agree
    }
}

/// The critic's independent, structured verdict on a written Scene.
///
/// The two judgements are the ones a deterministic rule cannot make: whether
/// the prose turned the planned **Value Turn** and whether it delivered the
/// planned **Narrative Promise** events. The four dimension scores are the
/// rubric's diagnostic context; they never drive a finding on their own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McKeeCriticVerdict {
    /// Rubric the critic answered under; must be the McKee rubric.
    pub rubric_version: String,
    /// The crit's judgement on the realized Value Turn.
    pub value_turn: CriticJudgement,
    /// The critic's judgement on the realized Narrative Promise events.
    pub promise_events: CriticJudgement,
    /// One score per [`McKeeCriticDimension`], all four required.
    pub dimensions: Vec<McKeeCriticDimensionScore>,
}

impl McKeeCriticVerdict {
    /// Validate the closed contract: McKee rubric, all four dimensions exactly
    /// once, scores in `1..=5`, non-empty rationales.
    pub fn validate(&self) -> Result<(), String> {
        if self.rubric_version != MCKEE_SCENE_CRITIC_RUBRIC_VERSION {
            return Err(format!(
                "expected rubric_version '{MCKEE_SCENE_CRITIC_RUBRIC_VERSION}', got '{}'",
                self.rubric_version
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for score in &self.dimensions {
            if !seen.insert(score.dimension) {
                return Err(format!("duplicate dimension '{}'", score.dimension));
            }
            if !(1..=5).contains(&score.score_1_to_5) {
                return Err(format!(
                    "dimension '{}' score {} outside 1..=5",
                    score.dimension, score.score_1_to_5
                ));
            }
            if score.rationale.trim().is_empty() {
                return Err(format!(
                    "dimension '{}' requires a non-empty rationale",
                    score.dimension
                ));
            }
        }
        for dimension in McKeeCriticDimension::ALL {
            if !seen.contains(&dimension) {
                return Err(format!("missing required dimension '{dimension}'"));
            }
        }
        Ok(())
    }
}

/// Render the part of the critic prompt that carries the rubric, the plan (never
/// the writer's report) and the output contract.
///
/// `plan_lines` are already-rendered plan facts (axis turns, Gap, Turning Beat);
/// `promise_lines` are the promise events this Scene is planned to deliver.
/// Both are payload, not instructions, so the critic can judge the prose against
/// the plan without being told what the writer claimed.
pub fn render_mckee_critic_prompt(plan_lines: &[String], promise_lines: &[String]) -> String {
    let mut out = String::from(
        "\n\nMCKEE CRITIC RUBRIC (scene-critic-mckee-v1)\n\
         Judge the PROSE against the PLAN below. You have NOT been given the writer's realized \
         report, and you must not assume it: your job is to say what the prose actually did, \
         independently. Score these four McKee dimensions from 1 to 5:\n\
         - value_turn_realized: did the Scene turn the planned primary Value Axis from its \
         planned opening Charge to its planned closing Charge?\n\
         - gap_lands: does the character's expectation break against reality, so the Gap is \
         dramatised rather than summarised?\n\
         - exposition_as_ammunition: is exposition used as ammunition inside conflict, rather \
         than explained to the reader?\n\
         - over_obvious_foreshadowing: is any setup so obvious that the reader sees the payoff \
         coming?\n\n\
         Planned Scene facts (the plan, not a report):\n",
    );
    if plan_lines.is_empty() {
        out.push_str("- (no planned Value Turn for this Scene)\n");
    } else {
        for line in plan_lines {
            out.push_str("- ");
            out.push_str(line.trim());
            out.push('\n');
        }
    }
    out.push_str("\nPlanned Narrative Promise events in this Scene:\n");
    if promise_lines.is_empty() {
        out.push_str("- (none planned)\n");
    } else {
        for line in promise_lines {
            out.push_str("- ");
            out.push_str(line.trim());
            out.push('\n');
        }
    }
    out.push_str(
        "\nAfter your STRENGTHS and CONCERNS sections, append exactly one line starting with \
         'CRITIC VERDICT:' followed by one JSON object and nothing after it. The verdict values \
         are the closed set \"agree\" | \"mismatch\"; use \"agree\" only when the prose genuinely \
         realizes the planned turn / promised event, and \"mismatch\" when it does not. \"notes\" \
         is optional. All four dimensions are required. Example shape (replace the scores and \
         rationales with your own):\n\
         CRITIC VERDICT: {\"rubric_version\":\"scene-critic-mckee-v1\",\
\"value_turn\":{\"verdict\":\"agree\",\"notes\":\"the gate turns trust\"},\
\"promise_events\":{\"verdict\":\"agree\"},\
\"dimensions\":[\
{\"dimension\":\"value_turn_realized\",\"score_1_to_5\":4,\"rationale\":\"why this score\"},\
{\"dimension\":\"gap_lands\",\"score_1_to_5\":3,\"rationale\":\"why this score\"},\
{\"dimension\":\"exposition_as_ammunition\",\"score_1_to_5\":3,\"rationale\":\"why this score\"},\
{\"dimension\":\"over_obvious_foreshadowing\",\"score_1_to_5\":2,\"rationale\":\"why this score\"}\
]}",
    );
    out
}

/// Parse a critic's structured verdict out of fixed output text.
///
/// The parser looks for [`MCKEE_CRITIC_VERDICT_MARKER`] and then takes the last
/// balanced JSON object after it (the same conservative salvage the model-output
/// helper uses), so narration after the marker does not defeat it. A verdict
/// that fails [`McKeeCriticVerdict::validate`], or output with no marker or no
/// balanced object, returns `None`: the check must never manufacture a critic
/// judgement out of malformed output.
pub fn parse_mckee_critic_verdict(output: &str) -> Option<McKeeCriticVerdict> {
    let upper = output.to_ascii_uppercase();
    let marker = upper.find(MCKEE_CRITIC_VERDICT_MARKER)?;
    let tail = &output[marker + MCKEE_CRITIC_VERDICT_MARKER.len()..];
    let json = extract_trailing_json_object(tail)?;
    let verdict: McKeeCriticVerdict = serde_json::from_str(json).ok()?;
    verdict.validate().ok()?;
    Some(verdict)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scores() -> Vec<McKeeCriticDimensionScore> {
        McKeeCriticDimension::ALL
            .iter()
            .map(|dimension| McKeeCriticDimensionScore {
                dimension: *dimension,
                score_1_to_5: 4,
                rationale: format!("{dimension} is realised"),
            })
            .collect()
    }

    fn verdict(
        value_turn: CriticVerdictKind,
        promise_events: CriticVerdictKind,
    ) -> McKeeCriticVerdict {
        McKeeCriticVerdict {
            rubric_version: MCKEE_SCENE_CRITIC_RUBRIC_VERSION.into(),
            value_turn: CriticJudgement {
                verdict: value_turn,
                notes: Some("the gate scene turns trust".into()),
            },
            promise_events: CriticJudgement {
                verdict: promise_events,
                notes: None,
            },
            dimensions: scores(),
        }
    }

    fn rendered(v: &McKeeCriticVerdict) -> String {
        format!(
            "STRENGTHS:\n- concrete\n\nCONCERNS:\n- the ending is thin\n\nCRITIC VERDICT: {}",
            serde_json::to_string(v).unwrap()
        )
    }

    #[test]
    fn rubric_selection_is_mckee_only() {
        assert_eq!(scene_critic_rubric_version(false), "scene-critic-v1");
        assert_eq!(scene_critic_rubric_version(true), "scene-critic-mckee-v1");
        assert_eq!(
            MCKEE_SCENE_CRITIC_RUBRIC_VERSION,
            scene_critic_rubric_version(true)
        );
        assert_eq!(
            SCENE_CRITIC_RUBRIC_VERSION,
            scene_critic_rubric_version(false)
        );
        assert_eq!(McKeeCriticDimension::ALL.len(), 4);
        let names: std::collections::BTreeSet<_> = McKeeCriticDimension::ALL
            .iter()
            .map(|dimension| dimension.as_str())
            .collect();
        assert_eq!(names.len(), 4);
        assert_eq!(CriticVerdictKind::ALL.len(), 2);
    }

    #[test]
    fn a_verdict_round_trips_as_closed_json() {
        let v = verdict(CriticVerdictKind::Agree, CriticVerdictKind::Mismatch);
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!(json["value_turn"]["verdict"], "agree");
        assert_eq!(json["promise_events"]["verdict"], "mismatch");
        assert_eq!(json["dimensions"][0]["dimension"], "value_turn_realized");
        assert!(json["promise_events"].get("notes").is_none());
        let back: McKeeCriticVerdict = serde_json::from_value(json).unwrap();
        assert_eq!(back, v);
        assert!(back.value_turn.is_agreement());
        assert!(!back.promise_events.is_agreement());
    }

    #[test]
    fn agreement_and_disagreement_fixed_outputs_parse() {
        for kind in [CriticVerdictKind::Agree, CriticVerdictKind::Mismatch] {
            let text = rendered(&verdict(kind, kind));
            let parsed = parse_mckee_critic_verdict(&text).expect("fixed output must parse");
            assert_eq!(parsed.value_turn.verdict, kind);
            assert_eq!(parsed.promise_events.verdict, kind);
        }
        // Case-insensitive marker, narration after the object tolerated.
        let text = format!(
            "critic verdict: {} trailing narration.",
            serde_json::to_string(&verdict(CriticVerdictKind::Agree, CriticVerdictKind::Agree))
                .unwrap()
        );
        assert!(parse_mckee_critic_verdict(&text).is_some());
    }

    #[test]
    fn malformed_or_incomplete_output_is_an_unavailable_verdict() {
        assert!(parse_mckee_critic_verdict("STRENGTHS:\n- none").is_none());
        assert!(parse_mckee_critic_verdict("CRITIC VERDICT: {\"rubric_version\":").is_none());
        let mut wrong_rubric = verdict(CriticVerdictKind::Agree, CriticVerdictKind::Agree);
        wrong_rubric.rubric_version = SCENE_CRITIC_RUBRIC_VERSION.into();
        assert!(parse_mckee_critic_verdict(&rendered(&wrong_rubric)).is_none());
        let mut missing_dimension = verdict(CriticVerdictKind::Agree, CriticVerdictKind::Agree);
        missing_dimension.dimensions.pop();
        assert!(parse_mckee_critic_verdict(&rendered(&missing_dimension)).is_none());
        let mut bad_score = verdict(CriticVerdictKind::Agree, CriticVerdictKind::Agree);
        bad_score.dimensions[0].score_1_to_5 = 9;
        assert!(parse_mckee_critic_verdict(&rendered(&bad_score)).is_none());
    }

    #[test]
    fn the_prompt_carries_the_plan_dimensions_and_contract_but_not_a_report() {
        let prompt = render_mckee_critic_prompt(
            &["trust: positive -> contradictory".to_string()],
            &["setup: 'Who forged the seal?'".to_string()],
        );
        for needle in [
            MCKEE_SCENE_CRITIC_RUBRIC_VERSION,
            "value_turn_realized",
            "gap_lands",
            "exposition_as_ammunition",
            "over_obvious_foreshadowing",
            "trust: positive -> contradictory",
            "Who forged the seal?",
            "CRITIC VERDICT",
            "NOT been given the writer's realized report",
        ] {
            assert!(
                prompt.contains(needle),
                "prompt must mention {needle}: {prompt}"
            );
        }
        assert!(!prompt.to_ascii_lowercase().contains("realized report:"));
    }

    #[test]
    fn verdict_source_defaults_to_route_and_skips_its_marker() {
        use crate::models::DualPersonaReviewRound;

        assert_eq!(CriticVerdictSource::default(), CriticVerdictSource::Route);
        assert!(CriticVerdictSource::Route.is_route());
        assert!(!CriticVerdictSource::Host.is_route());
        assert_eq!(CriticVerdictSource::Route.as_str(), "route");
        assert_eq!(CriticVerdictSource::Host.as_str(), "host");

        let round = |source| DualPersonaReviewRound {
            round: 1,
            literary_critic: default_notes(),
            craft_technician: default_notes(),
            genre_reader: default_notes(),
            priority_actions: Vec::new(),
            critic_rubric_version: Some(MCKEE_SCENE_CRITIC_RUBRIC_VERSION.into()),
            critic_verdict: Some(verdict(CriticVerdictKind::Agree, CriticVerdictKind::Agree)),
            critic_verdict_source: source,
        };
        // A route-produced verdict omits the marker entirely, so pre-existing
        // rounds and route payloads keep their exact bytes.
        let route_json = serde_json::to_value(round(Some(CriticVerdictSource::Route))).unwrap();
        assert!(
            route_json.get("critic_verdict_source").is_none(),
            "{route_json}"
        );
        let legacy_json = serde_json::to_value(round(None)).unwrap();
        assert_eq!(legacy_json, route_json);
        // A host-supplied verdict is tagged, and never satisfies gating.
        let host_json = serde_json::to_value(round(Some(CriticVerdictSource::Host))).unwrap();
        assert_eq!(host_json["critic_verdict_source"], "host");
        let host: DualPersonaReviewRound = serde_json::from_value(host_json).unwrap();
        assert!(host.route_critic_verdict().is_none());
        let route: DualPersonaReviewRound = serde_json::from_value(legacy_json).unwrap();
        assert!(route.route_critic_verdict().is_some());
    }

    fn default_notes() -> crate::models::PersonaReviewNotes {
        crate::models::PersonaReviewNotes {
            persona: "literary_critic".into(),
            ..Default::default()
        }
    }

    #[test]
    fn verdict_enums_are_one_closed_string_enum() {
        // Per-variant docs would emit one `oneOf` const object per variant,
        // which the Gemini schema sanitiser turns into a plain `object`; both
        // closed sets must survive as a single string enum.
        for (name, schema, expected) in [
            (
                "mckee_dimension",
                serde_json::to_value(schemars::schema_for!(McKeeCriticDimension)).unwrap(),
                4,
            ),
            (
                "critic_verdict_kind",
                serde_json::to_value(schemars::schema_for!(CriticVerdictKind)).unwrap(),
                2,
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
                    .is_some_and(|values| values.len() == expected),
                "{name} must carry the closed set: {schema}"
            );
        }
    }

    #[test]
    fn every_mckee_dimension_has_a_wire_name_and_a_score_entry() {
        for (dimension, wire) in [
            (
                McKeeCriticDimension::ValueTurnRealized,
                "value_turn_realized",
            ),
            (McKeeCriticDimension::GapLands, "gap_lands"),
            (
                McKeeCriticDimension::ExpositionAsAmmunition,
                "exposition_as_ammunition",
            ),
            (
                McKeeCriticDimension::OverObviousForeshadowing,
                "over_obvious_foreshadowing",
            ),
        ] {
            assert_eq!(dimension.as_str(), wire);
            assert_eq!(dimension.to_string(), wire);
            let parsed: McKeeCriticDimension =
                serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(parsed, dimension);
        }
    }
}
