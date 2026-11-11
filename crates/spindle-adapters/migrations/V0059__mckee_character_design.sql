-- =============================================================================
-- V0059: McKee Character Design.
--
-- * character_design is one structural blueprint per character (unique
--   character_id): contradictions, the four layers of self, conscious desire,
--   pressure on the protagonist and planner-written behaviour cues.
-- * The hidden layer and the unconscious desire are NOT stored as plain text:
--   hidden_self_fact_id / unconscious_desire_fact_id reference their secret
--   canonical_fact rows (V0023 secrecy columns), which remain the single
--   source of truth for those texts. The facts are planned-and-pending and
--   linked to the character through knowledge_fact.secret_of_fact_id, so the
--   existing circle-of-trust mechanism owns their audience.
-- * The table is additive: existing projects keep no rows, and every read
--   path that does not know about the table is unchanged. Arc milestone kinds
--   (change / reveal) need no column: milestones are a JSON array on
--   character_arc.
-- =============================================================================

CREATE TABLE character_design (
    id                           TEXT    PRIMARY KEY NOT NULL
                                         CHECK (id LIKE 'character_design:%'),
    project_id                   TEXT    NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    branch_id                    TEXT    NOT NULL REFERENCES bible_branch(id) ON DELETE CASCADE,
    character_id                 TEXT    NOT NULL REFERENCES character(id) ON DELETE CASCADE,
    contradictions               TEXT    NOT NULL DEFAULT '[]'
                                         CHECK (json_valid(contradictions)
                                                AND json_type(contradictions) = 'array'),
    public_self                  TEXT    NOT NULL DEFAULT '',
    personal_self                TEXT    NOT NULL DEFAULT '',
    private_self                 TEXT    NOT NULL DEFAULT '',
    hidden_self_fact_id          TEXT    NOT NULL
                                         REFERENCES canonical_fact(id) ON DELETE CASCADE,
    conscious_desire             TEXT    NOT NULL DEFAULT '',
    unconscious_desire_fact_id   TEXT    NOT NULL
                                         REFERENCES canonical_fact(id) ON DELETE CASCADE,
    pressure_on_protagonist      TEXT    NOT NULL DEFAULT '',
    behavior_cues                TEXT    NOT NULL DEFAULT '[]'
                                         CHECK (json_valid(behavior_cues)
                                                AND json_type(behavior_cues) = 'array'),
    created_at                   INTEGER NOT NULL,
    updated_at                   INTEGER NOT NULL
);

-- One Character Design per character: the database backstop for the service's
-- readable duplicate error.
CREATE UNIQUE INDEX idx_character_design_character
    ON character_design(character_id);

CREATE INDEX idx_character_design_project
    ON character_design(project_id, created_at);
