-- =============================================================================
-- V0063: record why a staged plan amendment was superseded.
--
-- ADR 0003 D2 defines supersede-on-replan as an audit transition, but the row
-- previously carried only `status = 'superseded'` plus updated_at: neither which
-- pass/decision retired it nor why. The amendment cascade (issue #34) supersedes
-- a moot or replaced reconciliation inside the apply transaction, so it must say
-- so (mirroring ADR 0003 D2's decided_at/decided_by audit pair).
--
-- Additive NULLable column: every existing row keeps NULL and serializes exactly
-- as before (`skip_serializing_if` on the read model).
-- =============================================================================

ALTER TABLE plan_amendment ADD COLUMN decision_note TEXT;

-- The cascade reads the still-staged reconciliation for one Scene by id, so the
-- existing status index is sufficient; no new index is needed for this column.
