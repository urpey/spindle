//! Character Design persistence: one design per character, with its hidden
//! layer and unconscious desire stored as secret canonical facts.
//!
//! The two secret facts are the single source of truth for those texts; the
//! `character_design` row keeps only their ids. A design is created through the
//! planning `CreateCharacterDesign` operation at the Cast & World level, so the
//! writes run inside the planning application transaction and use a savepoint
//! like the other planning-used repository paths.

use super::*;
use crate::sqlite::row::Timestamp;
use spindle_core::character_design::{
    CharacterDesign, CreateCharacterDesignInput, CreateCharacterDesignOutput,
    HIDDEN_SELF_FACT_PREDICATE, MAX_CHARACTER_DESIGN_CONTRADICTIONS,
    MIN_CHARACTER_DESIGN_CONTRADICTIONS, UNCONSCIOUS_DESIRE_FACT_PREDICATE, contradiction_count,
    valid_contradiction_count,
};

const CHARACTER_DESIGN_COLUMNS: &str = "id, project_id, branch_id, character_id, contradictions, \
     public_self, personal_self, private_self, hidden_self_fact_id, conscious_desire, \
     unconscious_desire_fact_id, pressure_on_protagonist, behavior_cues, created_at, updated_at";

/// Concealment note stored on both design secrets. They are author-only: the
/// context gate always withholds them, so the note documents the intent rather
/// than feeding a `[SECRETS IN PLAY]` envelope.
const DESIGN_SECRET_CONCEALMENT_NOTE: &str = "Author-only Character Design layer; dramatise it through the design's behaviour cues, never state it in drafting context.";

struct CharacterDesignRow {
    id: String,
    project_id: String,
    branch_id: String,
    character_id: String,
    contradictions: String,
    public_self: String,
    personal_self: String,
    private_self: String,
    hidden_self_fact_id: String,
    conscious_desire: String,
    unconscious_desire_fact_id: String,
    pressure_on_protagonist: String,
    behavior_cues: String,
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl TryFrom<&rusqlite::Row<'_>> for CharacterDesignRow {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row::text(row, 0)?,
            project_id: row::text(row, 1)?,
            branch_id: row::text(row, 2)?,
            character_id: row::text(row, 3)?,
            contradictions: row::text(row, 4)?,
            public_self: row::text(row, 5)?,
            personal_self: row::text(row, 6)?,
            private_self: row::text(row, 7)?,
            hidden_self_fact_id: row::text(row, 8)?,
            conscious_desire: row::text(row, 9)?,
            unconscious_desire_fact_id: row::text(row, 10)?,
            pressure_on_protagonist: row::text(row, 11)?,
            behavior_cues: row::text(row, 12)?,
            created_at: row::time(row, 13)?,
            updated_at: row::time(row, 14)?,
        })
    }
}

impl CharacterDesignRow {
    fn into_core(self) -> Result<CharacterDesign> {
        Ok(CharacterDesign {
            character_design_id: self.id,
            project_id: self.project_id,
            branch_id: self.branch_id,
            character_id: self.character_id,
            contradictions: serde_json::from_str(&self.contradictions)
                .context("invalid character design contradictions")?,
            public_self: self.public_self,
            personal_self: self.personal_self,
            private_self: self.private_self,
            hidden_self_fact_id: self.hidden_self_fact_id,
            conscious_desire: self.conscious_desire,
            unconscious_desire_fact_id: self.unconscious_desire_fact_id,
            pressure_on_protagonist: self.pressure_on_protagonist,
            behavior_cues: serde_json::from_str(&self.behavior_cues)
                .context("invalid character design behaviour cues")?,
            created_at: self.created_at.timestamp_micros(),
            updated_at: self.updated_at.timestamp_micros(),
        })
    }
}

fn insert_design_fact(
    conn: &rusqlite::Connection,
    project_id: &str,
    branch_id: &str,
    character_id: &str,
    predicate: &str,
    value: &str,
    now: i64,
) -> Result<String> {
    let fact_id = mint_id("canonical_fact");
    conn.execute(
        "INSERT INTO canonical_fact (id, project_id, branch_id, scene_id, source_scene_id, \
         book_number, chapter_number, subject_table, subject_id, predicate, value_kind, \
         value_number, value_text, value_json, unit, aliases, scope, valid_from, valid_until, \
         superseded_by, created_at, updated_at, secret, concealment_note) \
         VALUES (?1, ?2, ?3, NULL, NULL, 1, 1, 'character', ?4, ?5, 'string', NULL, ?6, NULL, \
                 NULL, '[]', 'invariant', NULL, NULL, NULL, ?7, ?7, 1, ?8)",
        rusqlite::params![
            &fact_id,
            project_id,
            branch_id,
            character_id,
            predicate,
            value,
            now,
            DESIGN_SECRET_CONCEALMENT_NOTE,
        ],
    )?;
    // The circle of trust is derived from these rows: one knowledge_fact row
    // linking the character to the secret fact (learned_at NULL = known from
    // the start). `fact` carries the layer label so the two design facts never
    // collide on the (project, branch, character, normalized_fact) key.
    let knowledge_id = mint_id("knowledge_fact");
    let fact_text = format!("{predicate}: {value}");
    let normalized = spindle_core::models::normalize_name(&fact_text);
    conn.execute(
        "INSERT INTO knowledge_fact (id, project_id, branch_id, character_id, fact, \
         normalized_fact, source_summary, learned_at, confidence, tags, reader_visible, \
         source_import_session_id, created_at, updated_at, secret_of_fact_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'character design', NULL, NULL, '[]', 1, NULL, ?7, ?7, ?8)",
        rusqlite::params![
            &knowledge_id,
            project_id,
            branch_id,
            character_id,
            &fact_text,
            &normalized,
            now,
            &fact_id,
        ],
    )?;
    Ok(fact_id)
}

impl Repository {
    /// Insert one Character Design plus its two secret canonical facts. The
    /// caller (a planning apply step) owns the surrounding transaction; this
    /// method nests through a savepoint so a failed design write rolls back
    /// only its own rows.
    pub async fn create_character_design(
        &self,
        input: &CreateCharacterDesignInput,
    ) -> Result<CreateCharacterDesignOutput> {
        let count = contradiction_count(&input.contradictions);
        anyhow::ensure!(
            valid_contradiction_count(count),
            "a Character Design requires {}..={} contradictions (got {count})",
            MIN_CHARACTER_DESIGN_CONTRADICTIONS,
            MAX_CHARACTER_DESIGN_CONTRADICTIONS
        );
        let project_id = input.project_id.clone();
        let character_id = input.character_id.clone();
        let contradictions = serde_json::to_string(&input.contradictions)?;
        let public_self = input.public_self.clone();
        let personal_self = input.personal_self.clone();
        let private_self = input.private_self.clone();
        let hidden_self = input.hidden_self.clone();
        let conscious_desire = input.conscious_desire.clone();
        let unconscious_desire = input.unconscious_desire.clone();
        let pressure_on_protagonist = input.pressure_on_protagonist.clone();
        let behavior_cues = serde_json::to_string(&input.behavior_cues)?;
        let design_id = mint_id("character_design");
        let design_id_out = design_id.clone();

        self.inner
            .pool
            .write(move |conn| {
                (|| -> Result<()> {
                    let tx = conn.savepoint()?;
                    let (character_project, branch_id): (String, String) = tx
                        .query_row(
                            "SELECT project_id, branch_id FROM character WHERE id=?1",
                            [&character_id],
                            |row| Ok((row.get(0)?, row.get(1)?)),
                        )
                        .context("character design character not found")?;
                    anyhow::ensure!(
                        character_project == project_id,
                        "character {character_id} does not belong to project {project_id}"
                    );
                    let mckee: i64 = tx.query_row(
                        "SELECT mckee_structured FROM project WHERE id=?1",
                        [&project_id],
                        |row| row.get(0),
                    )?;
                    anyhow::ensure!(
                        mckee == 1,
                        "character design requires a McKee-structured project"
                    );
                    let existing: Option<String> = tx
                        .query_row(
                            "SELECT id FROM character_design WHERE character_id=?1",
                            [&character_id],
                            |row| row.get(0),
                        )
                        .optional_inner()?;
                    anyhow::ensure!(
                        existing.is_none(),
                        "character {character_id} already has a character design; an accepted design is frozen"
                    );
                    let now = timestamp_to_micros(chrono::Utc::now());
                    let hidden_fact_id = insert_design_fact(
                        &tx,
                        &project_id,
                        &branch_id,
                        &character_id,
                        HIDDEN_SELF_FACT_PREDICATE,
                        &hidden_self,
                        now,
                    )?;
                    let unconscious_fact_id = insert_design_fact(
                        &tx,
                        &project_id,
                        &branch_id,
                        &character_id,
                        UNCONSCIOUS_DESIRE_FACT_PREDICATE,
                        &unconscious_desire,
                        now,
                    )?;
                    tx.execute(
                        "INSERT INTO character_design (id, project_id, branch_id, character_id, \
                         contradictions, public_self, personal_self, private_self, \
                         hidden_self_fact_id, conscious_desire, unconscious_desire_fact_id, \
                         pressure_on_protagonist, behavior_cues, created_at, updated_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
                        rusqlite::params![
                            &design_id,
                            &project_id,
                            &branch_id,
                            &character_id,
                            &contradictions,
                            &public_self,
                            &personal_self,
                            &private_self,
                            &hidden_fact_id,
                            &conscious_desire,
                            &unconscious_fact_id,
                            &pressure_on_protagonist,
                            &behavior_cues,
                            now,
                        ],
                    )?;
                    tx.commit()?;
                    Ok(())
                })()
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))
            })
            .await?;
        Ok(CreateCharacterDesignOutput {
            character_design_id: design_id_out,
        })
    }

    pub async fn get_character_design(
        &self,
        project_id: &str,
        character_design_id: &str,
    ) -> Result<CharacterDesign> {
        let project_id = project_id.to_string();
        let character_design_id = character_design_id.to_string();
        self.inner
            .pool
            .read(move |conn| {
                let sql = format!(
                    "SELECT {CHARACTER_DESIGN_COLUMNS} FROM character_design \
                     WHERE project_id=?1 AND id=?2"
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                stmt.query_row(rusqlite::params![project_id, character_design_id], |row| {
                    CharacterDesignRow::try_from(row)
                })
                .optional_inner()
            })
            .await?
            .context("character design not found")?
            .into_core()
    }

    pub async fn get_character_design_for_character(
        &self,
        project_id: &str,
        character_id: &str,
    ) -> Result<Option<CharacterDesign>> {
        let project_id = project_id.to_string();
        let character_id = character_id.to_string();
        let row = self
            .inner
            .pool
            .read(move |conn| {
                let sql = format!(
                    "SELECT {CHARACTER_DESIGN_COLUMNS} FROM character_design \
                     WHERE project_id=?1 AND character_id=?2"
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                stmt.query_row(rusqlite::params![project_id, character_id], |row| {
                    CharacterDesignRow::try_from(row)
                })
                .optional_inner()
            })
            .await?;
        row.map(CharacterDesignRow::into_core).transpose()
    }

    pub async fn list_character_designs(&self, project_id: &str) -> Result<Vec<CharacterDesign>> {
        let project_id = project_id.to_string();
        let rows = self
            .inner
            .pool
            .read(move |conn| {
                let sql = format!(
                    "SELECT {CHARACTER_DESIGN_COLUMNS} FROM character_design \
                     WHERE project_id=?1 ORDER BY created_at, id"
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                stmt.query_map([project_id], |row| CharacterDesignRow::try_from(row))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .await?;
        rows.into_iter()
            .map(CharacterDesignRow::into_core)
            .collect::<Result<Vec<_>>>()
    }
}
