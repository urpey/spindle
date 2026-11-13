//! Atomic persistence of a Scene's realized report consequences.
//!
//! A realized report ([`spindle_core::realized`]) is written in the same
//! transaction as the prose. This module owns that transaction-local work: the
//! report column edit shared by both persist paths, and the Narrative Promise
//! status history the report's confirmed events imply.
//!
//! Promise status is system-written from the report on a McKee project. An
//! unusable event (unknown promise, another project, a branch the Scene does not
//! inherit, a Scene before the planting placement) fails the transaction, so the
//! Scene row and every status row roll back together. Re-saving a Scene replaces
//! its report: a promise event the previous report advanced but the new one no
//! longer claims is corrected by an append-only status event that restores the
//! status the promise held before this Scene's first event. Events the new report
//! still claims are left untouched, so an unchanged re-save is a no-op.
//!
//! Everything here is gated on `project.mckee_structured`; a non-McKee save
//! writes its report column (additively) but never touches promise status.

use super::*;
use spindle_core::models::{PromiseStatusEvent, UpdatePromiseStatusInput};
use spindle_core::realized::RealizedReport;
use std::collections::{BTreeMap, BTreeSet};

/// Serialize a realized report for the `scene.realized_report` column.
///
/// Both persist paths call this, so the wire shape cannot drift between them.
pub(super) fn realized_report_json(report: Option<&RealizedReport>) -> Result<Option<String>> {
    Ok(report.map(serde_json::to_string).transpose()?)
}

/// Write a Scene's realized report column inside the caller's transaction.
pub(super) fn set_scene_realized_report_in_conn(
    conn: &rusqlite::Connection,
    scene_id: &str,
    report_json: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE scene SET realized_report = ?1 WHERE id = ?2",
        rusqlite::params![report_json, scene_id],
    )?;
    Ok(())
}

/// Run one realized step inside a `rusqlite` transaction while preserving its
/// `anyhow` error for the caller.
///
/// The transaction closure can only return `rusqlite::Error`, and the pool's
/// `with_sql_context` would make the generic SQL wrapper the outermost message.
/// A user-facing save error (an unknown promise, a promise the Scene precedes)
/// must stay readable, so the original error is parked in `slot` and re-raised
/// after the transaction has rolled back.
pub(super) fn capture_transaction_error(
    slot: &std::sync::Mutex<Option<anyhow::Error>>,
    result: Result<()>,
) -> rusqlite::Result<()> {
    result.map_err(|error| {
        if let Ok(mut slot) = slot.lock() {
            *slot = Some(error);
        }
        rusqlite::Error::InvalidQuery
    })
}

fn promise_in_conn(
    conn: &rusqlite::Connection,
    promise_id: &str,
) -> Result<Option<NarrativePromise>> {
    let sql = format!("SELECT {NARRATIVE_PROMISE_COLUMNS} FROM narrative_promise WHERE id = ?1");
    let mut stmt = conn.prepare_cached(&sql)?;
    Ok(stmt
        .query_row([promise_id], |row| NarrativePromise::try_from(row))
        .optional_inner()?)
}

fn project_promises_in_conn(
    conn: &rusqlite::Connection,
    project_id: &str,
) -> Result<Vec<NarrativePromise>> {
    let sql =
        format!("SELECT {NARRATIVE_PROMISE_COLUMNS} FROM narrative_promise WHERE project_id = ?1");
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt
        .query_map([project_id], |row| NarrativePromise::try_from(row))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// The status a Scene's first recorded event found the promise in.
fn pre_scene_status(events: &[&PromiseStatusEvent]) -> Option<String> {
    events.first().map(|event| event.previous_status.clone())
}

/// Historical events this Scene wrote, in append order.
fn scene_events<'a>(promise: &'a NarrativePromise, scene_id: &str) -> Vec<&'a PromiseStatusEvent> {
    promise
        .status_history
        .iter()
        .filter(|event| event.source_scene_id.as_deref() == Some(scene_id))
        .collect()
}

/// Validate and apply a realized report's promise events, then reconcile a
/// re-save, inside the Scene's own transaction.
///
/// `placement` is the saved Scene's placement; the generic status primitive
/// re-reads the Scene row, which is why this must run after the Scene write and
/// inside the same transaction. A non-McKee project returns immediately.
pub(super) fn apply_realized_promise_events_in_conn(
    conn: &rusqlite::Connection,
    project_id: &str,
    branch_id: &str,
    scene_id: &str,
    placement: &StoryPlacement,
    report: Option<&RealizedReport>,
) -> Result<()> {
    if !project_is_mckee_in_conn(conn, project_id)? {
        return Ok(());
    }
    let scene_order = placement.scene_order.ok_or_else(|| {
        anyhow!("a realized promise event needs a scene placement, not a chapter-level one")
    })?;
    let scene_index =
        crate::format::story_index(placement.book_number, placement.chapter_number, scene_order);
    let lineage = resolve_branch_lineage_in_conn(conn, Some(project_id), branch_id)?;

    // Validate every event and keep the LAST status per promise: one Scene can
    // report at most one final event per promise, and that final status is what
    // the report claims.
    let mut desired: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    for event in report
        .map(|report| report.promise_events.as_slice())
        .unwrap_or_default()
    {
        let promise = promise_in_conn(conn, &event.narrative_promise_id)?.ok_or_else(|| {
            anyhow!(
                "realized promise event names unknown narrative promise '{}'",
                event.narrative_promise_id
            )
        })?;
        anyhow::ensure!(
            promise.project_id == project_id,
            "realized promise event names narrative promise '{}' from another project",
            event.narrative_promise_id
        );
        anyhow::ensure!(
            lineage.iter().any(|branch| branch == &promise.branch_id),
            "realized promise event names narrative promise '{}' from a branch the scene does not \
             inherit",
            event.narrative_promise_id
        );
        anyhow::ensure!(
            scene_index >= crate::format::story_index_from_placement(&promise.planted_at),
            "the scene precedes the planting placement of narrative promise '{}'; a promise event \
             cannot precede its planting",
            event.narrative_promise_id
        );
        desired.insert(
            event.narrative_promise_id.clone(),
            (event.event.promise_status().to_string(), event.note.clone()),
        );
    }

    // Apply the report's claimed statuses. A status the Scene already recorded as
    // its latest event is a no-op, which is what makes an unchanged re-save
    // idempotent.
    for (promise_id, (status, note)) in &desired {
        let promise = promise_in_conn(conn, promise_id)?.ok_or_else(|| {
            anyhow!("narrative promise '{promise_id}' disappeared during the save")
        })?;
        let latest = scene_events(&promise, scene_id)
            .last()
            .map(|event| event.status.clone());
        if latest.as_deref() == Some(status.as_str()) {
            continue;
        }
        let mut message =
            format!("system: realized '{status}' event reported with scene {scene_id}");
        if let Some(extra) = note
            .as_deref()
            .map(str::trim)
            .filter(|note| !note.is_empty())
        {
            message.push_str(" — ");
            message.push_str(extra);
        }
        super::Repository::apply_promise_status_in_conn(
            conn,
            &UpdatePromiseStatusInput {
                narrative_promise_id: promise_id.clone(),
                status: status.clone(),
                note: Some(message),
                at: Some(placement.clone()),
                source_scene_id: Some(scene_id.to_string()),
                replaces_event_id: None,
            },
        )?;
    }

    // Reconcile the promises this Scene previously drove but the new report no
    // longer claims. The corrective event is append-only: the original event
    // stays as history and the promise returns to the status it held before this
    // Scene's first event. Keying on the Scene's latest recorded status makes a
    // repeated removal a no-op.
    for promise in project_promises_in_conn(conn, project_id)? {
        if !lineage.iter().any(|branch| branch == &promise.branch_id) {
            continue;
        }
        if desired.contains_key(&promise.id) {
            continue;
        }
        let events = scene_events(&promise, scene_id);
        let Some(target) = pre_scene_status(&events) else {
            continue;
        };
        let latest = events.last().map(|event| event.status.clone());
        if latest.as_deref() == Some(target.as_str()) {
            continue;
        }
        let dropped: BTreeSet<&str> = events.iter().map(|event| event.status.as_str()).collect();
        let dropped = dropped.into_iter().collect::<Vec<_>>().join(", ");
        super::Repository::apply_promise_status_in_conn(
            conn,
            &UpdatePromiseStatusInput {
                narrative_promise_id: promise.id.clone(),
                status: target.clone(),
                note: Some(format!(
                    "system: re-saved scene {scene_id} no longer reports the realized event(s) \
                     '{dropped}'; restored '{target}'"
                )),
                at: Some(placement.clone()),
                source_scene_id: Some(scene_id.to_string()),
                replaces_event_id: None,
            },
        )?;
    }
    Ok(())
}
