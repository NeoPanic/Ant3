//! Board-scoped pheromone traces: bounded deposits, lazy evaporation.
//!
//! Verified events (completion, human acceptance, peer reuse) deposit fixed
//! weights; Agents may add low-bandwidth signals under a per-Run quota.
//! Strength is always evaporated to `now` before reinforcement, so stale
//! trails cannot be amplified by fresh deposits.
use crate::{
    board::{active_scope, receipt, replay},
    domain::*,
    store::{Store, event},
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use ant3_domain::traces::*;
pub use ant3_domain::traces::TraceInput;

/// Evaporate the current strength to `now`, reinforce, upsert and emit the
/// deposit event. Does not write the ledger; callers own deduplication.
fn apply(
    tx: &Transaction<'_>,
    root: &str,
    key: &str,
    kind: &str,
    amount: f64,
    reason: &str,
    source_run: Option<&str>,
) -> PResult<()> {
    let at = now();
    let existing: Option<(f64, i64)> = tx
        .query_row(
            "SELECT strength,updated_at FROM board_traces WHERE root_id=? AND key=?",
            params![root, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let base = existing
        .map(|(strength, updated_at)| effective_strength(strength, updated_at, at))
        .unwrap_or(0.0);
    let strength = reinforce(base, amount);
    tx.execute(
        "INSERT INTO board_traces(root_id,key,kind,strength,deposits,created_at,updated_at) VALUES(?,?,?,?,1,?,?)
         ON CONFLICT(root_id,key) DO UPDATE SET strength=excluded.strength,deposits=deposits+1,updated_at=excluded.updated_at",
        params![root, key, kind, strength, at, at],
    )?;
    event(
        tx,
        "board.trace_deposited",
        root,
        json!({"key":key,"kind":kind,"amount":amount,"strength":strength,"reason":reason,"source_run_id":source_run}),
    )
}

/// Deposit into a trace. `reason` is `agent`, `completion` or `accepted`.
/// Capacity is enforced on the agent path only; verified-event deposits on
/// task keys must never fail because agents filled the field.
pub(crate) fn deposit(
    tx: &Transaction<'_>,
    root: &str,
    key: &str,
    kind: &str,
    amount: f64,
    reason: &str,
    source_run: Option<&str>,
) -> PResult<()> {
    tx.execute(
        "INSERT INTO board_trace_deposits(root_id,key,amount,reason,source_run_id,created_at) VALUES(?,?,?,?,?,?)",
        params![root, key, amount, reason, source_run, now()],
    )?;
    apply(tx, root, key, kind, amount, reason, source_run)
}

/// Once-per-Run deposit for `reuse` and `recruitment`, deduplicated by a
/// partial unique index on the ledger. Returns whether a deposit happened.
fn deposit_once(
    tx: &Transaction<'_>,
    root: &str,
    key: &str,
    run: &str,
    reason: &str,
    amount: f64,
    kind: Option<&str>,
) -> PResult<bool> {
    let inserted = tx.execute(
        "INSERT OR IGNORE INTO board_trace_deposits(root_id,key,amount,reason,source_run_id,created_at) VALUES(?,?,?,?,?,?)",
        params![root, key, amount, reason, run, now()],
    )?;
    if inserted == 0 {
        return Ok(false);
    }
    let kind = match kind {
        Some(kind) => kind.to_owned(),
        None => tx.query_row(
            "SELECT kind FROM board_traces WHERE root_id=? AND key=?",
            params![root, key],
            |r| r.get(0),
        )?,
    };
    apply(tx, root, key, &kind, amount, reason, Some(run))?;
    Ok(true)
}

/// Reuse deposit: reading a peer's settled result reinforces that trail once
/// per Run.
pub(crate) fn deposit_reuse_once(
    tx: &Transaction<'_>,
    root: &str,
    key: &str,
    run: &str,
) -> PResult<bool> {
    deposit_once(
        tx,
        root,
        key,
        run,
        "reuse",
        TRACE_REUSE_AMOUNT,
        Some(RESULT_TRACE_KIND),
    )
}

/// Recruitment deposit: a successful Run amplifies the signals it left
/// behind, once per trail per Run.
pub(crate) fn deposit_recruitment_once(
    tx: &Transaction<'_>,
    root: &str,
    key: &str,
    run: &str,
) -> PResult<bool> {
    deposit_once(tx, root, key, run, "recruitment", TRACE_RECRUITMENT_AMOUNT, None)
}

pub(crate) fn trace_view(tx: &Transaction<'_>, root: &str, key: &str) -> PResult<Option<Value>> {
    let row: Option<(String, f64, i64, i64, i64)> = tx
        .query_row(
            "SELECT kind,strength,deposits,created_at,updated_at FROM board_traces WHERE root_id=? AND key=?",
            params![root, key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    Ok(row.map(|(kind, strength, deposits, created_at, updated_at)| {
        json!({
            "key": key,
            "kind": kind,
            "strength": effective_strength(strength, updated_at, now()),
            "raw_strength": strength,
            "deposits": deposits,
            "created_at": created_at,
            "updated_at": updated_at,
        })
    }))
}

fn traces_in(tx: &Transaction<'_>, root: &str) -> PResult<Value> {
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM board_tasks WHERE id=?)",
        [root],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(fault(404, "board_task_not_found"));
    }
    let mut stmt = tx.prepare(
        "SELECT key,kind,strength,deposits,created_at,updated_at FROM board_traces WHERE root_id=?",
    )?;
    let at = now();
    let mut entries = stmt
        .query_map([root], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?
        .map(|row| {
            let (key, kind, strength, deposits, created_at, updated_at) = row?;
            Ok(json!({
                "key": key,
                "kind": kind,
                "strength": effective_strength(strength, updated_at, at),
                "raw_strength": strength,
                "deposits": deposits,
                "created_at": created_at,
                "updated_at": updated_at,
            }))
        })
        .collect::<PResult<Vec<_>>>()?;
    entries.sort_by(|a, b| {
        b["strength"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&a["strength"].as_f64().unwrap_or(0.0))
    });
    Ok(json!({
        "root_id": root,
        "traces": entries,
        "half_life_ms": TRACE_HALF_LIFE_MS,
        "max_strength": TRACE_MAX_STRENGTH,
    }))
}

impl Store {
    /// Operator view of a root task's trace field.
    pub fn board_traces(&self, root: &str) -> PResult<Value> {
        self.board_transaction(|tx| traces_in(tx, root))
    }
    /// Agent view, scoped to the Run's root task.
    pub fn read_board_traces(&self, run: &str) -> PResult<Value> {
        self.board_transaction(|tx| {
            let scope = active_scope(tx, run)?;
            traces_in(tx, scope["root_id"].as_str().unwrap())
        })
    }
    /// Bounded agent signal deposit with idempotent retries.
    pub fn deposit_board_trace(
        &self,
        run: &str,
        input: &TraceInput,
        key: &str,
    ) -> PResult<Value> {
        input.validate()?;
        if key.is_empty() || key.len() > 160 {
            return Err(fault(400, "invalid_command_key"));
        }
        self.board_transaction(|tx| {
            let scope = active_scope(tx, run)?;
            let root = scope["root_id"].as_str().unwrap();
            let command_scope = format!("trace:{run}");
            let fingerprint = serde_json::to_string(input)?;
            if let Some(value) = replay(tx, &command_scope, key, &fingerprint)? {
                return Ok(value);
            }
            let used: usize = tx.query_row(
                "SELECT count(*) FROM board_commands WHERE scope=?",
                [&command_scope],
                |r| r.get(0),
            )?;
            if used >= TRACE_DEPOSITS_PER_RUN {
                return Err(fault(409, "board_trace_quota"));
            }
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM board_traces WHERE root_id=? AND key=?)",
                params![root, input.key],
                |r| r.get(0),
            )?;
            if !exists {
                let count: usize = tx.query_row(
                    "SELECT count(*) FROM board_traces WHERE root_id=?",
                    [root],
                    |r| r.get(0),
                )?;
                if count >= TRACES_PER_ROOT {
                    return Err(fault(409, "board_trace_capacity"));
                }
            }
            deposit(
                tx,
                root,
                &input.key,
                &input.kind,
                TRACE_AGENT_AMOUNT,
                "agent",
                Some(run),
            )?;
            let response = trace_view(tx, root, &input.key)?
                .ok_or_else(|| fault(500, "board_trace_missing"))?;
            receipt(tx, &command_scope, key, &fingerprint, response)
        })
    }
}