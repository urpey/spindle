-- =============================================================================
-- V0061: McKee realized reports on the Scene row.
--
-- A written Scene can carry the writer's realized report: the realized Value
-- Turn per planned Value Axis (opening/closing Charge, Gap, Turning Beat), the
-- Character Effect per present major character, the Narrative Promise events the
-- Scene actually advanced, and the planned Key Facts it dramatised
-- (spindle_core::realized).
--
-- The column is nullable and additive: existing rows keep NULL, every typed
-- read path skips an unset report, and a Scene with no report — and every
-- non-McKee project — serializes byte-identically. The realized report is a
-- report about prose, never canon: its only write-through is a confirmed
-- realized promise event advancing that promise's status (with the source scene
-- recorded in the existing status-history event).
-- =============================================================================

ALTER TABLE scene
    ADD COLUMN realized_report TEXT
    CHECK (realized_report IS NULL OR json_valid(realized_report));
