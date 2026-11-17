//! Root energy pools: atomic reservation, usage settlement, verified refunds.
//!
//! Dispatch reserves energy; settlement bills actual usage (unknown usage is
//! billed conservatively); acceptance and completion refund energy. A root
//! whose available energy cannot cover a reservation is starved: it stops
//! being offered and its pending tasks are culled with an explicit reason.
use crate::{
    domain::*,
    settings::Settings,
    store::{Store, event},
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

pub use ant3_domain::energy::*;

pub(crate) fn init(tx: &Transaction<'_>, root: &str, budget: i64) -> PResult<()> {
    tx.execute(
        "INSERT OR IGNORE INTO board_energy(root_id,budget) VALUES(?,?)",
        params![root, budget],
    )?;
    Ok(())
}

fn balance(tx: &Transaction<'_>, root: &str) -> PResult<(i64, i64, i64, i64)> {
    tx.query_row(
        "SELECT budget,reserved,spent,refunded FROM board_energy WHERE root_id=?",
        [root],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .optional()?
    .ok_or_else(|| fault(404, "energy_pool_not_found"))
}

pub(crate) fn available(tx: &Transaction<'_>, root: &str) -> PResult<i64> {
    let (budget, reserved, spent, refunded) = balance(tx, root)?;
    Ok(budget + refunded - spent - reserved)
}

fn ledger(
    tx: &Transaction<'_>,
    root: &str,
    run: Option<&str>,
    kind: &str,
    amount: i64,
) -> PResult<()> {
    let (budget, reserved, spent, refunded) = balance(tx, root)?;
    tx.execute(
        "INSERT INTO energy_ledger(root_id,run_id,kind,amount,balance,created_at) VALUES(?,?,?,?,?,?)",
        params![root, run, kind, amount, budget + refunded - spent - reserved, now()],
    )?;
    event(
        tx,
        "board.energy_posted",
        root,
        json!({"run_id":run,"kind":kind,"amount":amount,"available":budget + refunded - spent - reserved}),
    )
}

/// Reserve energy for a dispatched Run. Fails when the pool is starved.
pub(crate) fn reserve(tx: &Transaction<'_>, root: &str, run: &str) -> PResult<()> {
    let reserve = Settings::load(tx)?.energy.reserve;
    if available(tx, root)? < reserve {
        return Err(fault(409, "energy_exhausted"));
    }
    tx.execute(
        "UPDATE board_energy SET reserved=reserved+? WHERE root_id=?",
        params![reserve, root],
    )?;
    ledger(tx, root, Some(run), "reserve", reserve)
}

/// Settle a finished Run: release the reservation and bill actual usage.
/// Unknown usage bills conservatively. Idempotent per Run.
pub(crate) fn settle(tx: &Transaction<'_>, run: &str, report: &Value) -> PResult<()> {
    let root: Option<String> = tx
        .query_row(
            "SELECT root_id FROM board_agents WHERE run_id=?",
            [run],
            |r| r.get(0),
        )
        .optional()?;
    let root: Option<String> = match root {
        Some(root) => Some(root),
        None => tx
            .query_row(
                "SELECT t.root_id FROM board_verifications v JOIN board_tasks t ON t.id=v.task_id WHERE v.run_id=?",
                [run],
                |r| r.get::<_, String>(0),
            )
            .optional()?,
    };
    let Some(root) = root else { return Ok(()) };
    let already: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM energy_ledger WHERE run_id=? AND kind IN ('settle','settle_unknown'))",
        [run],
        |r| r.get(0),
    )?;
    if already {
        return Ok(());
    }
    let reserved: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM energy_ledger WHERE run_id=? AND kind='reserve')",
        [run],
        |r| r.get(0),
    )?;
    let settings = Settings::load(tx)?.energy;
    if reserved {
        tx.execute(
            "UPDATE board_energy SET reserved=MAX(reserved-?,0) WHERE root_id=?",
            params![settings.reserve, root],
        )?;
    }
    let usage = &report["usage"];
    let (cost, kind) = if usage.is_object() {
        let input = usage["input"].as_u64().unwrap_or(0);
        let output = usage["output"].as_u64().unwrap_or(0);
        let cached = usage["cached_input"].as_u64();
        (usage_cost_with(input, output, cached, settings.tokens_per_energy), "settle")
    } else {
        (settings.unknown_cost, "settle_unknown")
    };
    tx.execute(
        "UPDATE board_energy SET spent=spent+? WHERE root_id=?",
        params![cost, root],
    )?;
    ledger(tx, &root, Some(run), kind, cost)?;
    cull_if_starved(tx, &root)
}

/// Verified success earns energy back.
pub(crate) fn refund(tx: &Transaction<'_>, root: &str, run: Option<&str>, kind: &str, amount: i64) -> PResult<()> {
    tx.execute(
        "UPDATE board_energy SET refunded=refunded+? WHERE root_id=?",
        params![amount, root],
    )?;
    ledger(tx, root, run, kind, amount)
}

/// A starved root stops being offered: pending unassigned tasks are culled
/// with an explicit reason; running work settles normally (already reserved).
pub(crate) fn cull_if_starved(tx: &Transaction<'_>, root: &str) -> PResult<()> {
    if available(tx, root)? >= Settings::load(tx)?.energy.reserve {
        return Ok(());
    }
    let mut stmt = tx.prepare(
        "SELECT id FROM board_tasks WHERE root_id=? AND state='pending' AND route_state='unassigned'",
    )?;
    let ids = stmt
        .query_map([root], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    for id in ids {
        tx.execute(
            "UPDATE board_tasks SET state='failed',error='energy_exhausted',finished_at=? WHERE id=?",
            params![now(), id],
        )?;
        event(tx, "board.task_settled", &id, json!({"state":"failed","error":"energy_exhausted"}))?;
    }
    Ok(())
}

impl Store {
    /// Cull pending work on a starved root (also runs after each settlement).
    pub fn cull_starved(&self, root: &str) -> PResult<()> {
        self.transaction(|tx| cull_if_starved(tx, root))
    }
    /// Operator view of a root's energy pool and ledger.
    pub fn board_energy(&self, root: &str) -> PResult<Value> {
        self.transaction(|tx| {
            let (budget, reserved, spent, refunded) = balance(tx, root)?;
            let mut stmt = tx.prepare(
                "SELECT json_object('run_id',run_id,'kind',kind,'amount',amount,'balance',balance,'created_at',created_at) FROM energy_ledger WHERE root_id=? ORDER BY seq DESC LIMIT 100",
            )?;
            let entries = stmt
                .query_map([root], |r| r.get::<_, String>(0))?
                .map(|r| Ok(serde_json::from_str::<Value>(&r?)?))
                .collect::<PResult<Vec<_>>>()?;
            Ok(json!({
                "root_id": root,
                "budget": budget,
                "reserved": reserved,
                "spent": spent,
                "refunded": refunded,
                "available": budget + refunded - spent - reserved,
                "reserve_per_run": Settings::load(tx)?.energy.reserve,
                "ledger": entries,
            }))
        })
    }
}
