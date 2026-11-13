//! McKee amendment-cascade stale marks (issue #34, migration V0062).
//!
//! Accepting a plan amendment that changes a planned Scene's closing or opening
//! Charge (or the plan binding a Key Fact / Narrative Promise payoff relies on)
//! re-runs the value-chain and promise checks over the later planned, undrafted
//! Scenes. A Scene that no longer passes is marked here so the author can see
//! it; the repair is a normal staged `plan_amendment` (ADR 0003) referenced by
//! `reconciliation_amendment_id`, never a parallel amendment table.
//!
//! One row per Scene: a repeat cascade updates the existing mark instead of
//! inserting a duplicate, and clearing keeps the row as history. Non-McKee
//! projects never write this table.
//!
//! Mark + stage + supersede + clear are applied in ONE writer transaction
//! ([`Repository::apply_scene_plan_stale_batch`]) so a failure can never leave a
//! staged repair without its mark, or a cleared mark next to a live proposal.

use super::*;
use crate::sqlite::records::{SCENE_PLAN_STALE_COLUMNS, StoredScenePlanStale};
use crate::sqlite::repository::plans::{
    insert_plan_amendment_in_conn, supersede_staged_amendment_in_conn,
};

/// Parameters for marking (or re-marking) one planned Scene stale.
#[derive(Debug, Clone)]
pub struct MarkScenePlanStaleParams {
    pub project_id: String,
    pub branch_id: String,
    pub book_number: i32,
    pub chapter_number: i32,
    pub scene_order: i32,
    pub reason: String,
    pub finding_codes: Vec<String>,
    pub triggering_amendment_id: String,
    pub reconciliation_amendment_id: Option<String>,
}

/// One Scene's mark write inside the atomic cascade batch.
#[derive(Debug, Clone)]
pub struct ScenePlanStaleMarkWrite {
    /// The mark to upsert. Its `reconciliation_amendment_id` is used when no
    /// `stage` is supplied (a reused still-staged repair).
    pub mark: MarkScenePlanStaleParams,
    /// Stage this reconciliation in the same transaction; the mark links to the
    /// newly minted id.
    pub stage: Option<StagePlanAmendmentParams>,
    /// Still-staged proposals superseded for this Scene (a replaced repair).
    pub supersede: Vec<(String, String)>,
}

/// One Scene's clear write inside the atomic cascade batch.
#[derive(Debug, Clone)]
pub struct ScenePlanStaleClearWrite {
    pub book_number: i32,
    pub chapter_number: i32,
    pub scene_order: i32,
    /// A still-staged repair that became moot when the Scene started passing.
    pub supersede_amendment_id: Option<String>,
    pub reason: String,
}

/// The whole cascade write set. Everything is committed in one transaction.
#[derive(Debug, Clone)]
pub struct ScenePlanStaleBatch {
    pub project_id: String,
    pub branch_id: String,
    /// The operator who applied the triggering amendment (ADR 0003 D2 audit).
    pub decided_by: String,
    pub marks: Vec<ScenePlanStaleMarkWrite>,
    pub clears: Vec<ScenePlanStaleClearWrite>,
}

fn upsert_mark_in_conn(
    conn: &rusqlite::Connection,
    params: &MarkScenePlanStaleParams,
    reconciliation_amendment_id: Option<&str>,
    now: i64,
) -> rusqlite::Result<()> {
    let codes_json = serde_json::to_string(&params.finding_codes).map_err(|error| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error,
        )))
    })?;
    conn.execute(
        "INSERT INTO scene_plan_stale \
         (id, project_id, branch_id, book_number, chapter_number, scene_order, \
          status, reason, finding_codes, triggering_amendment_id, \
          reconciliation_amendment_id, created_at, updated_at, cleared_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'stale', ?7, ?8, ?9, ?10, ?11, ?11, NULL) \
         ON CONFLICT (project_id, branch_id, book_number, chapter_number, scene_order) \
         DO UPDATE SET status = 'stale', reason = excluded.reason, \
            finding_codes = excluded.finding_codes, \
            triggering_amendment_id = excluded.triggering_amendment_id, \
            reconciliation_amendment_id = excluded.reconciliation_amendment_id, \
            updated_at = excluded.updated_at, cleared_at = NULL",
        rusqlite::params![
            mint_id("scene_plan_stale"),
            &params.project_id,
            &params.branch_id,
            params.book_number,
            params.chapter_number,
            params.scene_order,
            &params.reason,
            &codes_json,
            &params.triggering_amendment_id,
            reconciliation_amendment_id,
            now,
        ],
    )?;
    Ok(())
}

fn clear_mark_in_conn(
    conn: &rusqlite::Connection,
    project_id: &str,
    branch_id: &str,
    book_number: i32,
    chapter_number: i32,
    scene_order: i32,
    now: i64,
) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "UPDATE scene_plan_stale \
         SET status='cleared', cleared_at=?1, updated_at=?1 \
         WHERE project_id=?2 AND branch_id=?3 AND book_number=?4 \
           AND chapter_number=?5 AND scene_order=?6 AND status='stale'",
        rusqlite::params![
            now,
            project_id,
            branch_id,
            book_number,
            chapter_number,
            scene_order
        ],
    )?;
    Ok(affected > 0)
}

impl Repository {
    /// Insert or update the one stale mark for a Scene. A repeat cascade keeps
    /// the row id, refreshes the reason/codes/trigger and moves the status back
    /// to `stale` (clearing `cleared_at`), so a Scene never accumulates duplicate
    /// marks.
    pub async fn mark_scene_plan_stale(
        &self,
        params: MarkScenePlanStaleParams,
    ) -> Result<StoredScenePlanStale> {
        let project_id = params.project_id.clone();
        let branch_id = params.branch_id.clone();
        let book_number = params.book_number;
        let chapter_number = params.chapter_number;
        let scene_order = params.scene_order;
        self.inner
            .pool
            .write(move |conn| {
                let now = timestamp_to_micros(chrono::Utc::now());
                upsert_mark_in_conn(
                    conn,
                    &params,
                    params.reconciliation_amendment_id.as_deref(),
                    now,
                )?;
                Ok(())
            })
            .await?;
        self.read_scene_plan_stale(
            &project_id,
            &branch_id,
            book_number,
            chapter_number,
            scene_order,
        )
        .await?
        .ok_or_else(|| anyhow!("scene_plan_stale vanished after upsert"))
    }

    /// Apply one cascade's mark + stage + supersede + clear set atomically.
    ///
    /// Everything is committed in a single writer transaction: a failing
    /// statement (for example a dangling triggering-amendment reference) rolls
    /// the whole batch back, so there is never an orphan staged repair or a
    /// cleared mark beside a live proposal. Returns the ids of the reconciliation
    /// proposals staged by this batch, in mark order.
    pub async fn apply_scene_plan_stale_batch(
        &self,
        batch: ScenePlanStaleBatch,
    ) -> Result<Vec<String>> {
        self.inner
            .pool
            .write(move |conn| {
                let now = timestamp_to_micros(chrono::Utc::now());
                let tx = conn.transaction()?;
                let mut staged_ids = Vec::new();
                for write in &batch.marks {
                    for (amendment_id, reason) in &write.supersede {
                        supersede_staged_amendment_in_conn(
                            &tx,
                            amendment_id,
                            &batch.decided_by,
                            reason,
                            now,
                        )?;
                    }
                    let new_reconciliation_id = match write.stage.as_ref() {
                        Some(stage) => {
                            let id = insert_plan_amendment_in_conn(&tx, stage, now)?;
                            staged_ids.push(id.clone());
                            Some(id)
                        }
                        None => None,
                    };
                    let reconciliation_id = new_reconciliation_id
                        .as_deref()
                        .or(write.mark.reconciliation_amendment_id.as_deref());
                    upsert_mark_in_conn(&tx, &write.mark, reconciliation_id, now)?;
                }
                for clear in &batch.clears {
                    if let Some(amendment_id) = clear.supersede_amendment_id.as_deref() {
                        supersede_staged_amendment_in_conn(
                            &tx,
                            amendment_id,
                            &batch.decided_by,
                            &clear.reason,
                            now,
                        )?;
                    }
                    clear_mark_in_conn(
                        &tx,
                        &batch.project_id,
                        &batch.branch_id,
                        clear.book_number,
                        clear.chapter_number,
                        clear.scene_order,
                        now,
                    )?;
                }
                tx.commit()?;
                Ok(staged_ids)
            })
            .await
    }

    /// A stale mark by Scene coordinates, or `None`.
    pub async fn read_scene_plan_stale(
        &self,
        project_id: &str,
        branch_id: &str,
        book_number: i32,
        chapter_number: i32,
        scene_order: i32,
    ) -> Result<Option<StoredScenePlanStale>> {
        let project_id = project_id.to_string();
        let branch_id = branch_id.to_string();
        self.inner
            .pool
            .read(move |conn| {
                let sql = format!(
                    "SELECT {SCENE_PLAN_STALE_COLUMNS} FROM scene_plan_stale \
                     WHERE project_id=?1 AND branch_id=?2 AND book_number=?3 \
                       AND chapter_number=?4 AND scene_order=?5"
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                stmt.query_row(
                    rusqlite::params![
                        &project_id,
                        &branch_id,
                        book_number,
                        chapter_number,
                        scene_order
                    ],
                    |r| StoredScenePlanStale::try_from(r),
                )
                .optional_inner()
            })
            .await
    }

    /// Stale marks on one branch, optionally narrowed to a status
    /// (`stale` | `cleared`). Deterministic `(book, chapter, scene_order)` order.
    pub async fn list_scene_plan_stale(
        &self,
        project_id: &str,
        branch_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<StoredScenePlanStale>> {
        let project_id = project_id.to_string();
        let branch_id = branch_id.to_string();
        let status = status.map(str::to_string);
        self.inner
            .pool
            .read(move |conn| {
                let mut clauses = String::from("project_id=?1 AND branch_id=?2");
                let mut binds: Vec<&dyn rusqlite::ToSql> = vec![&project_id, &branch_id];
                if let Some(status) = status.as_ref() {
                    clauses.push_str(&format!(" AND status=?{}", binds.len() + 1));
                    binds.push(status);
                }
                let sql = format!(
                    "SELECT {SCENE_PLAN_STALE_COLUMNS} FROM scene_plan_stale \
                     WHERE {clauses} ORDER BY book_number, chapter_number, scene_order"
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                let rows = stmt
                    .query_map(binds.as_slice(), |r| StoredScenePlanStale::try_from(r))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .await
    }

    /// The stale marks whose reconciliation proposal is the given amendment (a
    /// reconciliation apply must re-validate its own Scene).
    pub async fn list_scene_plan_stale_for_reconciliation(
        &self,
        project_id: &str,
        branch_id: &str,
        amendment_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<StoredScenePlanStale>> {
        let project_id = project_id.to_string();
        let branch_id = branch_id.to_string();
        let amendment_id = amendment_id.to_string();
        let status = status.map(str::to_string);
        self.inner
            .pool
            .read(move |conn| {
                let mut clauses = String::from(
                    "project_id=?1 AND branch_id=?2 AND reconciliation_amendment_id=?3",
                );
                let mut binds: Vec<&dyn rusqlite::ToSql> =
                    vec![&project_id, &branch_id, &amendment_id];
                if let Some(status) = status.as_ref() {
                    clauses.push_str(&format!(" AND status=?{}", binds.len() + 1));
                    binds.push(status);
                }
                let sql = format!(
                    "SELECT {SCENE_PLAN_STALE_COLUMNS} FROM scene_plan_stale \
                     WHERE {clauses} ORDER BY book_number, chapter_number, scene_order"
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                let rows = stmt
                    .query_map(binds.as_slice(), |r| StoredScenePlanStale::try_from(r))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .await
    }

    /// Clear a stale mark (status `cleared`, `cleared_at` stamped). Returns
    /// whether an active mark existed. History is retained, never deleted.
    pub async fn clear_scene_plan_stale(
        &self,
        project_id: &str,
        branch_id: &str,
        book_number: i32,
        chapter_number: i32,
        scene_order: i32,
    ) -> Result<bool> {
        let project_id = project_id.to_string();
        let branch_id = branch_id.to_string();
        let affected = self
            .inner
            .pool
            .write(move |conn| {
                let now = timestamp_to_micros(chrono::Utc::now());
                clear_mark_in_conn(
                    conn,
                    &project_id,
                    &branch_id,
                    book_number,
                    chapter_number,
                    scene_order,
                    now,
                )
            })
            .await?;
        Ok(affected)
    }
}
