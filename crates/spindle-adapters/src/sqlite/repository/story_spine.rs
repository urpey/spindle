use super::*;
use crate::sqlite::row::Timestamp;
use spindle_core::story_spine::{CreateStorySpineInput, CreateStorySpineOutput, StorySpine};

pub const STORY_SPINE_COLUMNS: &str = "id, project_id, series_spine_id, book_number, central_question, \
     controlling_idea_value, controlling_idea_cause, counter_idea, spine_axis, ending_type, \
     inciting_incident, final_climax, theme_ids, created_at, updated_at";

struct StorySpineRow {
    id: String,
    project_id: String,
    series_spine_id: Option<String>,
    book_number: Option<i32>,
    central_question: String,
    controlling_idea_value: String,
    controlling_idea_cause: String,
    counter_idea: String,
    spine_axis: String,
    ending_type: String,
    inciting_incident: Option<String>,
    final_climax: Option<String>,
    theme_ids: String,
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl TryFrom<&rusqlite::Row<'_>> for StorySpineRow {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row::text(row, 0)?,
            project_id: row::text(row, 1)?,
            series_spine_id: row::opt_text(row, 2)?,
            book_number: row.get::<_, Option<i32>>(3)?,
            central_question: row::text(row, 4)?,
            controlling_idea_value: row::text(row, 5)?,
            controlling_idea_cause: row::text(row, 6)?,
            counter_idea: row::text(row, 7)?,
            spine_axis: row::text(row, 8)?,
            ending_type: row::text(row, 9)?,
            inciting_incident: row::opt_text(row, 10)?,
            final_climax: row::opt_text(row, 11)?,
            theme_ids: row::text(row, 12)?,
            created_at: row::time(row, 13)?,
            updated_at: row::time(row, 14)?,
        })
    }
}

fn parse_ending(value: &str) -> Result<spindle_core::story_spine::StorySpineEnding> {
    match value {
        "idealist" => Ok(spindle_core::story_spine::StorySpineEnding::Idealist),
        "pessimist" => Ok(spindle_core::story_spine::StorySpineEnding::Pessimist),
        "ironic" => Ok(spindle_core::story_spine::StorySpineEnding::Ironic),
        other => anyhow::bail!("unknown story spine ending type: {other}"),
    }
}

fn ending_str(ending: spindle_core::story_spine::StorySpineEnding) -> &'static str {
    use spindle_core::story_spine::StorySpineEnding::*;
    match ending {
        Idealist => "idealist",
        Pessimist => "pessimist",
        Ironic => "ironic",
    }
}

fn parse_placement(value: Option<&str>, field: &str) -> Result<Option<StoryPlacement>> {
    value
        .map(|json| {
            serde_json::from_str(json)
                .with_context(|| format!("invalid story spine {field} placement"))
        })
        .transpose()
}

impl StorySpineRow {
    fn into_core(self) -> Result<StorySpine> {
        Ok(StorySpine {
            story_spine_id: self.id,
            project_id: self.project_id,
            series_spine_id: self.series_spine_id,
            book_number: self.book_number,
            central_question: self.central_question,
            controlling_idea: spindle_core::story_spine::ControllingIdea {
                value: self.controlling_idea_value,
                cause: self.controlling_idea_cause,
            },
            counter_idea: self.counter_idea,
            spine_axis: self.spine_axis,
            ending_type: parse_ending(&self.ending_type)?,
            inciting_incident: parse_placement(
                self.inciting_incident.as_deref(),
                "inciting incident",
            )?,
            final_climax: parse_placement(self.final_climax.as_deref(), "final climax")?,
            theme_ids: serde_json::from_str(&self.theme_ids)
                .context("invalid story spine theme ids")?,
            created_at: self.created_at.timestamp_micros(),
            updated_at: self.updated_at.timestamp_micros(),
        })
    }
}

fn nonempty(field: &str, value: &str) -> Result<()> {
    anyhow::ensure!(
        !value.trim().is_empty(),
        "story spine {field} must be nonempty"
    );
    Ok(())
}

fn validate_placement(
    field: &str,
    placement: &Option<StoryPlacement>,
    compare_after: Option<&StoryPlacement>,
) -> Result<()> {
    let Some(placement) = placement else {
        return Ok(());
    };
    anyhow::ensure!(
        placement.book_number >= 1
            && placement.chapter_number >= 1
            && placement.scene_order.is_none_or(|scene| scene >= 1),
        "story spine {field} placement must use positive book, chapter, and scene coordinates"
    );
    if let Some(earlier) = compare_after {
        anyhow::ensure!(
            (
                placement.book_number,
                placement.chapter_number,
                placement.scene_order.unwrap_or(0)
            ) >= (
                earlier.book_number,
                earlier.chapter_number,
                earlier.scene_order.unwrap_or(0),
            ),
            "story spine final climax must not precede the inciting incident"
        );
    }
    Ok(())
}

impl Repository {
    pub async fn create_story_spine(
        &self,
        input: &CreateStorySpineInput,
    ) -> Result<CreateStorySpineOutput> {
        nonempty("central question", &input.central_question)?;
        nonempty("controlling idea value", &input.controlling_idea.value)?;
        nonempty("controlling idea cause", &input.controlling_idea.cause)?;
        nonempty("spine axis", &input.spine_axis)?;
        validate_placement("inciting incident", &input.inciting_incident, None)?;
        validate_placement(
            "final climax",
            &input.final_climax,
            input.inciting_incident.as_ref(),
        )?;
        let project_id = input.project_id.clone();
        let series_spine_id = input.series_spine_id.clone();
        let book_number = input.book_number;
        let central_question = input.central_question.clone();
        let controlling_idea_value = input.controlling_idea.value.clone();
        let controlling_idea_cause = input.controlling_idea.cause.clone();
        let counter_idea = input.counter_idea.clone();
        let spine_axis = input.spine_axis.clone();
        let ending_type = ending_str(input.ending_type).to_string();
        let inciting_incident = input
            .inciting_incident
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let final_climax = input
            .final_climax
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let theme_ids = serde_json::to_string(&input.theme_ids)?;
        let theme_ids_for_check = input.theme_ids.clone();
        let story_spine_id = mint_id("story_spine");
        let now = timestamp_to_micros(chrono::Utc::now());

        self.inner.pool.write(move |conn| {
            (|| -> Result<CreateStorySpineOutput> {
                let tx = conn.savepoint()?;
                let mckee: i64 = tx.query_row(
                    "SELECT mckee_structured FROM project WHERE id=?1",
                    [&project_id],
                    |row| row.get(0),
                )?;
                anyhow::ensure!(
                    mckee == 1,
                    "story spine requires a McKee-structured project"
                );

                match &series_spine_id {
                    None => {
                        anyhow::ensure!(
                            book_number.is_none(),
                            "a series story spine cannot name a book"
                        );
                        let series_exists: bool = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM story_spine \
                             WHERE project_id=?1 AND series_spine_id IS NULL)",
                            [&project_id],
                            |row| row.get(0),
                        )?;
                        anyhow::ensure!(
                            !series_exists,
                            "project already has a series story spine"
                        );
                    }
                    Some(series_id) => {
                        let book_number = book_number.context(
                            "a book story spine requires book_number and series_spine_id",
                        )?;
                        anyhow::ensure!(book_number >= 1, "book_number must be positive");
                        let (parent_project, parent_series): (String, Option<String>) = tx
                            .query_row(
                                "SELECT project_id, series_spine_id FROM story_spine WHERE id=?1",
                                [series_id],
                                |row| Ok((row.get(0)?, row.get(1)?)),
                            )
                            .context("series story spine not found")?;
                        anyhow::ensure!(
                            parent_project == project_id,
                            "series story spine belongs to another project"
                        );
                        anyhow::ensure!(
                            parent_series.is_none(),
                            "a book sub-spine must reference the series spine, not another sub-spine"
                        );
                        let duplicate: bool = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM story_spine \
                             WHERE project_id=?1 AND book_number=?2 AND series_spine_id IS NOT NULL)",
                            rusqlite::params![&project_id, book_number],
                            |row| row.get(0),
                        )?;
                        anyhow::ensure!(
                            !duplicate,
                            "book {book_number} already has a story sub-spine"
                        );
                    }
                }

                for theme_id in &theme_ids_for_check {
                    let visible: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM theme WHERE id=?1 AND project_id=?2)",
                        rusqlite::params![theme_id, &project_id],
                        |row| row.get(0),
                    )?;
                    anyhow::ensure!(
                        visible,
                        "story spine theme {theme_id} is not visible in the project"
                    );
                }

                tx.execute(
                    "INSERT INTO story_spine \
                     (id, project_id, series_spine_id, book_number, central_question, \
                      controlling_idea_value, controlling_idea_cause, counter_idea, spine_axis, \
                      ending_type, inciting_incident, final_climax, theme_ids, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
                    rusqlite::params![
                        &story_spine_id,
                        &project_id,
                        &series_spine_id,
                        book_number,
                        &central_question,
                        &controlling_idea_value,
                        &controlling_idea_cause,
                        &counter_idea,
                        &spine_axis,
                        &ending_type,
                        &inciting_incident,
                        &final_climax,
                        &theme_ids,
                        now,
                    ],
                )?;
                tx.commit()?;
                Ok(CreateStorySpineOutput {
                    story_spine_id: story_spine_id.clone(),
                })
            })()
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))
        })
        .await
    }

    pub async fn get_story_spine(&self, project_id: &str, id: &str) -> Result<StorySpine> {
        self.get_project(project_id).await?;
        let project_id = project_id.to_string();
        let id = id.to_string();
        let row = self
            .inner
            .pool
            .read(move |conn| {
                let sql = format!(
                    "SELECT {STORY_SPINE_COLUMNS} FROM story_spine WHERE id=?1 AND project_id=?2"
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                stmt.query_row([&id, &project_id], |row| StorySpineRow::try_from(row))
                    .optional_inner()
            })
            .await?
            .context("story spine not found")?;
        row.into_core()
    }

    pub async fn list_story_spines(&self, project_id: &str) -> Result<Vec<StorySpine>> {
        self.get_project(project_id).await?;
        let project_id = project_id.to_string();
        let rows = self
            .inner
            .pool
            .read(move |conn| {
                let sql = format!(
                    "SELECT {STORY_SPINE_COLUMNS} FROM story_spine WHERE project_id=?1 \
                     ORDER BY CASE WHEN series_spine_id IS NULL THEN 0 ELSE 1 END, \
                              book_number, created_at"
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                stmt.query_map([&project_id], |row| StorySpineRow::try_from(row))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .await?;
        rows.into_iter()
            .map(StorySpineRow::into_core)
            .collect::<Result<Vec<_>>>()
    }

    pub async fn is_mckee_project(&self, project_id: &str) -> Result<bool> {
        let project_id = project_id.to_string();
        let value: i64 = self
            .inner
            .pool
            .read(move |conn| {
                conn.query_row(
                    "SELECT mckee_structured FROM project WHERE id=?1",
                    [&project_id],
                    |row| row.get(0),
                )
            })
            .await?;
        Ok(value == 1)
    }
}
