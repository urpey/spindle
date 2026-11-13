-- =============================================================================
-- V0062: McKee amendment cascade — stale planned Scenes.
--
-- Accepting a plan amendment that changes a planned Scene's closing Charge (or
-- the plan binding a Key Fact / a Narrative Promise payoff relies on) re-runs
-- the McKee value-chain and promise/Key-Fact checks over the later planned,
-- undrafted Scenes. A Scene that no longer passes is marked stale here so the
-- author can see it without re-staging a planning artifact.
--
-- One row per (project, branch, book, chapter, scene_order): a repeat cascade
-- updates the existing row instead of piling up duplicates, and clearing a mark
-- keeps the row (status = 'cleared') so the history is readable. The staged
-- reconciliation proposal is a normal `plan_amendment` row (ADR 0003), linked by
-- `reconciliation_amendment_id` — no parallel amendment table.
--
-- Only McKee projects ever write this table; a non-McKee project is untouched
-- and `list_plan_amendments` keeps its exact previous output.
-- =============================================================================

CREATE TABLE scene_plan_stale (
    id                          TEXT    PRIMARY KEY NOT NULL
                                        CHECK (id LIKE 'scene_plan_stale:%'),
    project_id                  TEXT    NOT NULL REFERENCES project(id)      ON DELETE CASCADE,
    branch_id                   TEXT    NOT NULL REFERENCES bible_branch(id) ON DELETE CASCADE,
    book_number                 INTEGER NOT NULL,
    chapter_number              INTEGER NOT NULL,
    scene_order                 INTEGER NOT NULL CHECK (scene_order >= 1),
    status                      TEXT    NOT NULL DEFAULT 'stale'
                                        CHECK (status IN ('stale', 'cleared')),
    -- Why the Scene no longer passes, in ids/summaries-only prose.
    reason                      TEXT    NOT NULL,
    -- JSON array of the grading planning-finding codes that failed.
    finding_codes               TEXT    NOT NULL CHECK (json_valid(finding_codes)),
    -- The accepted amendment whose apply triggered this mark.
    triggering_amendment_id     TEXT    NOT NULL REFERENCES plan_amendment(id) ON DELETE CASCADE,
    -- The staged `scene_replace` reconciliation proposal for this Scene, when a
    -- mechanical repair is expressible. NULL = manual replan required.
    reconciliation_amendment_id TEXT             REFERENCES plan_amendment(id) ON DELETE SET NULL,
    created_at                  INTEGER NOT NULL,
    updated_at                  INTEGER NOT NULL,
    cleared_at                  INTEGER,
    UNIQUE (project_id, branch_id, book_number, chapter_number, scene_order)
);

-- Drives `list_plan_amendments`'s active stale view per branch.
CREATE INDEX idx_scene_plan_stale_project_branch_status
    ON scene_plan_stale(project_id, branch_id, status);
