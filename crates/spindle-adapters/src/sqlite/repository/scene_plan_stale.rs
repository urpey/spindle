//! McKee amendment-cascade stale marks (issue #34, migration V0062).
//!
//! Accepting a plan amendment that changes a planned Scene's closing Charge (or
//! the plan binding a Key Fact / Narrative Promise payoff relies on) re-runs the
//! value-chain and promise checks over the later planned, undrafted Scenes. A
//! Scene that no longer passes is marked here so the author can see it; the
//! repair is a normal staged `plan_amendment` (ADR 0003) referenced by
//! `reconciliation_amendment_id`, never a parallel amendment table.
//!
//! One row per Scene: a repeat cascade updates the existing mark instead of
//! inserting a duplicate, and clearing keeps the row as history. Non-McKee
//! projects never write this table.

use super::*;
use crate::sqlite::records::{SCENE_PLAN_STALE_COLUMNS, StoredScenePlanStale};

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

impl Repository {
    /// Insert or update the one stale mark for a Scene. A repeat cascade keeps
    /// the row id, refreshes the reason/codes/trigger and moves the status back
    /// to `stale` (clearing `cleared_at`), so a Scene never accumulates duplicate
    /// marks.
    pub async fn mark_scene_plan_stale(
        &self,
        params: MarkScenePlanStaleParams,
    ) -> Result<StoredScenePlanStale> {
        let codes_json = serde_json::to_string(&params.finding_codes)
            .context("serializing stale finding codes")?;
        let project_id = params.project_id.clone();
        let branch_id = params.branch_id.clone();
        let book_number = params.book_number;
        let chapter_number = params.chapter_number;
        let scene_order = params.scene_order;
        let reason = params.reason;
        let triggering_amendment_id = params.triggering_amendment_id;
        let reconciliation_amendment_id = params.reconciliation_amendment_id;
        let id = mint_id("scene_plan_stale");
        let project_id_for_write = project_id.clone();
        let branch_id_for_write = branch_id.clone();
        self.inner
            .pool
            .write(move |conn| {
                let now = timestamp_to_micros(chrono::Utc::now());
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
                        &id,
                        &project_id_for_write,
                        &branch_id_for_write,
                        book_number,
                        chapter_number,
                        scene_order,
                        &reason,
                        &codes_json,
                        &triggering_amendment_id,
                        &reconciliation_amendment_id,
                        now,
                    ],
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
                let affected = conn.execute(
                    "UPDATE scene_plan_stale \
                     SET status='cleared', cleared_at=?1, updated_at=?1 \
                     WHERE project_id=?2 AND branch_id=?3 AND book_number=?4 \
                       AND chapter_number=?5 AND scene_order=?6 AND status='stale'",
                    rusqlite::params![
                        now,
                        &project_id,
                        &branch_id,
                        book_number,
                        chapter_number,
                        scene_order
                    ],
                )?;
                Ok(affected as u64)
            })
            .await?;
        Ok(affected > 0)
    }

    /// Supersede one still-`staged` plan amendment (its reconciliation is moot
    /// because the Scene now passes). Decided rows are history and untouched.
    pub async fn supersede_plan_amendment(&self, id: &str) -> Result<u64> {
        let id = id.to_string();
        self.inner
            .pool
            .write(move |conn| {
                let now = timestamp_to_micros(chrono::Utc::now());
                let affected = conn.execute(
                    "UPDATE plan_amendment SET status='superseded', updated_at=?1 \
                     WHERE id=?2 AND status='staged'",
                    rusqlite::params![now, id],
                )?;
                Ok(affected as u64)
            })
            .await
    }
}
