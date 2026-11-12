-- =============================================================================
-- V0060: McKee Narrative Promise payoff structure.
--
-- * planned_payoff_turning_beat is the 1-based Beat position in the planned
--   payoff Scene where the payoff lands (the same numbering as the Scene's
--   `turning_beat`). It is meaningful only together with planned_payoff.
-- * target_act is the 1-based order of the Act a long-range promise must pay
--   off in. Long-range promises are planned at the Series-Arc or Book level and
--   the planning validator requires it to name an Act in the accepted/merged
--   Act list; Chapter/Scene-level (short-range) placements may omit it.
--
-- Both columns are nullable and additive: existing rows keep NULL, every typed
-- read/serialization path skips unset values, and existing projects therefore
-- serialize byte-identically. Key Facts need no table of their own: a Key Fact
-- is a planned-and-pending canonical_fact row (scene_id NULL) placed by its
-- book/chapter/scene placement and bound later with
-- bind_canonical_fact_to_scene.
-- =============================================================================

ALTER TABLE narrative_promise
    ADD COLUMN planned_payoff_turning_beat INTEGER
    CHECK (planned_payoff_turning_beat IS NULL OR planned_payoff_turning_beat >= 1);

ALTER TABLE narrative_promise
    ADD COLUMN target_act INTEGER
    CHECK (target_act IS NULL OR target_act >= 1);
