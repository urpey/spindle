-- =============================================================================
-- V0057: McKee project marker and Story Spine records.
--
-- * project.mckee_structured is set only by the planning CreateProject apply
--   path. Existing projects and direct create_project calls keep 0.
-- * story_spine stores one series spine per project (series_spine_id NULL) and
--   optional per-book sub-spines that reference it. The table is project-level,
--   not branch-level: the Story Spine is planning intent for the series, shared
--   by the project's branches.
--
-- The partial unique indexes are the database backstop for "one series spine"
-- and "one sub-spine per book"; the service also returns a readable error.
-- =============================================================================

ALTER TABLE project
    ADD COLUMN mckee_structured INTEGER NOT NULL DEFAULT 0
    CHECK (mckee_structured IN (0, 1));

CREATE TABLE story_spine (
    id                     TEXT    PRIMARY KEY NOT NULL
                                   CHECK (id LIKE 'story_spine:%'),
    project_id             TEXT    NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    series_spine_id        TEXT             REFERENCES story_spine(id) ON DELETE CASCADE,
    book_number            INTEGER          CHECK (book_number IS NULL OR book_number > 0),
    central_question       TEXT    NOT NULL DEFAULT '',
    controlling_idea_value TEXT    NOT NULL DEFAULT '',
    controlling_idea_cause TEXT    NOT NULL DEFAULT '',
    counter_idea           TEXT    NOT NULL DEFAULT '',
    spine_axis             TEXT    NOT NULL DEFAULT '',
    ending_type            TEXT    NOT NULL
                                   CHECK (ending_type IN ('idealist', 'pessimist', 'ironic')),
    inciting_incident      TEXT             CHECK (inciting_incident IS NULL OR json_valid(inciting_incident)),
    final_climax           TEXT             CHECK (final_climax IS NULL OR json_valid(final_climax)),
    theme_ids              TEXT    NOT NULL DEFAULT '[]'
                                   CHECK (json_valid(theme_ids) AND json_type(theme_ids) = 'array'),
    created_at             INTEGER NOT NULL,
    updated_at             INTEGER NOT NULL,
    CHECK ((series_spine_id IS NULL AND book_number IS NULL)
        OR (series_spine_id IS NOT NULL AND book_number IS NOT NULL)),
    CHECK (series_spine_id IS NULL OR series_spine_id <> id)
);

CREATE UNIQUE INDEX idx_story_spine_series
    ON story_spine(project_id)
    WHERE series_spine_id IS NULL;

CREATE UNIQUE INDEX idx_story_spine_book
    ON story_spine(project_id, book_number)
    WHERE series_spine_id IS NOT NULL;

CREATE INDEX idx_story_spine_project
    ON story_spine(project_id, created_at);
