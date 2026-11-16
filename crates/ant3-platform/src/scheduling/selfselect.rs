//! Self-selection: idle peers claim work themselves; no central classifier.
//!
//! A wandering pool of ready instances (composition reproduced from verified
//! experience) scans open tasks. Each idle peer responds to a task with the
//! classic threshold probability against its own cost-adjusted threshold;
//! responding peers race to claim atomically. The control plane enforces
//! only physics: capacity, energy reservation, atomicity. Idle wandering
//! burns energy from a colony pool; sustained-failure presets are retired.
use crate::{
    domain::*,
    store::{Store, event},
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

pub use ant3_domain::division::{IDLE_BURN_PER_MIN, POOR_PRESET_FAILURES, SELF_SELECT_POOL_MAX, pool_targets, responds};

/// Colony energy pool for idle wandering (root_id NULL ledger rows).
const COLONY_BUDGET: i64 = 200_000;

fn enabled() -> bool {
    std::env::var("ANT3_SELF_SELECT").is_ok_and(|v| v == "1")
}

fn experience(tx: &Transaction<'_>) -> PResult<Vec<(String, u32, u32)>> {
    // Only presets whose model protocol can actually run are eligible; the
    // demo protocol is test-only and handled by the caller's fixture path.
    let mut stmt = tx.prepare(
        "SELECT p.name,coalesce(e.successes,0),coalesce(e.failures,0) FROM presets p LEFT JOIN preset_experience e ON e.preset_name=p.name AND e.preset_version=p.version ORDER BY p.name,p.version",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?, r.get::<_, u32>(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Maintain the wandering pool toward reproduction targets. Returns the
/// number of standing idle instances after maintenance.
fn maintain_pool(tx: &Transaction<'_>) -> PResult<usize> {
    let exp = experience(tx)?;
    let targets = pool_targets(&exp, SELF_SELECT_POOL_MAX);
    // Retire idle instances of presets no longer targeted.
    let mut stmt = tx.prepare(
        "SELECT i.id,i.preset_name FROM instances i WHERE i.state='ready' AND EXISTS(SELECT 1 FROM wander_stock w JOIN fleet_slots s ON s.instance_id=i.id WHERE w.fleet_id=s.fleet_id)",
    )?;
    let idle = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    for (agent, preset) in &idle {
        let target = targets.iter().find(|(n, _)| n == preset).map(|(_, n)| *n).unwrap_or(0);
        let standing: usize = tx.query_row(
            "SELECT count(*) FROM instances i WHERE i.preset_name=? AND i.state='ready' AND EXISTS(SELECT 1 FROM wander_stock w JOIN fleet_slots s ON s.instance_id=i.id WHERE w.fleet_id=s.fleet_id)",
            [preset],
            |r| r.get(0),
        )?;
        if standing > target {
            crate::lifecycle::drain(tx, agent, false)?;
            event(tx, "swarm.peer_retired", agent, json!({"preset_name":preset,"reason":"reproduction_target"}))?;
        }
    }
    // Reproduce toward targets with fresh instances.
    for (preset, target) in &targets {
        let standing: usize = tx.query_row(
            "SELECT count(*) FROM instances i WHERE i.preset_name=? AND i.state IN ('ready','busy') AND EXISTS(SELECT 1 FROM wander_stock w JOIN fleet_slots s ON s.instance_id=i.id WHERE w.fleet_id=s.fleet_id)",
            [preset],
            |r| r.get(0),
        )?;
        if standing >= *target {
            continue;
        }
        let version: Option<String> = tx.query_row(
            "SELECT version FROM presets WHERE name=? ORDER BY created_at DESC LIMIT 1",
            [preset],
            |r| r.get(0),
        ).optional()?;
        let Some(version) = version else { continue };
        let quota = crate::fleets::quota_used(tx)?;
        if quota >= crate::limits::INSTANCES {
            break;
        }
        tx.execute_batch("SAVEPOINT wander_spawn;")?;
        let result = crate::fleets::create_in(
            tx,
            &crate::fleets::FleetPlan {
                preset_name: preset.clone(),
                preset_version: version,
                count: 1,
                mode: "finite".into(),
                ttl_seconds: None,
                parameters: Default::default(),
            },
            &format!("wander-{}-{}", preset, uuid::Uuid::new_v4()),
        );
        match result {
            Ok(fleet) => {
                tx.execute_batch("RELEASE wander_spawn;")?;
                // Tag the fleet as wandering stock so maintenance can find it.
                tx.execute(
                    "INSERT OR IGNORE INTO wander_stock(fleet_id,preset_name,created_at) VALUES(?,?,?)",
                    params![fleet["fleet_id"].as_str().unwrap(), preset, now()],
                )?;
                event(tx, "swarm.peer_spawned", fleet["fleet_id"].as_str().unwrap(), json!({"preset_name":preset,"target":target}))?;
            }
            Err(_) => {
                tx.execute_batch("ROLLBACK TO wander_spawn; RELEASE wander_spawn;")?;
            }
        }
    }
    let standing: usize = tx.query_row(
        "SELECT count(*) FROM instances i WHERE i.state='ready' AND EXISTS(SELECT 1 FROM wander_stock w JOIN fleet_slots s ON s.instance_id=i.id WHERE w.fleet_id=s.fleet_id)",
        [],
        |r| r.get(0),
    )?;
    Ok(standing)
}

/// One self-selection sweep: maintain the pool, burn idle energy, and let
/// each idle peer respond to open tasks by its own threshold.
pub(crate) fn sweep(tx: &Transaction<'_>) -> PResult<()> {
    sweep_with(tx, enabled())
}

/// Explicit-mode sweep (tests and operator overrides pass the flag directly).
pub(crate) fn sweep_with(tx: &Transaction<'_>, on: bool) -> PResult<()> {
    if !on {
        return Ok(());
    }
    let standing = maintain_pool(tx)?;
    burn_idle(tx, standing)?;
    // Open tasks: pending, unassigned, funded, cooled — same physics as routing.
    let mut stmt = tx.prepare(
        "SELECT id,root_id FROM board_tasks WHERE state='pending' AND route_state='unassigned' ORDER BY created_at,rowid LIMIT 32",
    )?;
    let open = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    if open.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare(
        "SELECT i.id,i.preset_name,i.preset_version FROM instances i WHERE i.state='ready' AND EXISTS(SELECT 1 FROM wander_stock w JOIN fleet_slots s ON s.instance_id=i.id WHERE w.fleet_id=s.fleet_id) ORDER BY i.created_at,i.rowid",
    )?;
    let idle = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    let seed = now() as u64;
    let mut rng = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut draw = || {
        rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        z as f64 / u64::MAX as f64
    };
    for (task_id, root) in &open {
        if crate::energy::available(tx, root)? < crate::energy::ENERGY_RESERVE {
            continue;
        }
        if !crate::board::cooled(tx, task_id)? {
            continue;
        }
        let strength = crate::traces::trace_view(tx, root, task_id)?
            .and_then(|v| v["strength"].as_f64())
            .unwrap_or(0.0);
        let crowd: usize = tx.query_row(
            "SELECT count(*) FROM board_agents a JOIN runs r ON r.id=a.run_id WHERE a.root_id=? AND r.state IN ('queued','running')",
            [root],
            |r| r.get(0),
        )?;
        let stim = crate::division::stimulus(strength, crowd);
        for (agent, preset, version) in &idle {
            // Still idle? A previous claim in this sweep may have taken it.
            let state: String = tx.query_row(
                "SELECT state FROM instances WHERE id=?",
                [agent],
                |r| r.get(0),
            )?;
            if state != "ready" {
                continue;
            }
            let threshold = crate::division::evidence(tx, preset, version)?["response_threshold"]
                .as_f64()
                .unwrap_or(0.5);
            if !responds(stim.max(0.2), threshold, draw()) {
                continue;
            }
            match claim_open(tx, Claim { task_id, root, agent, preset, version, stimulus: stim, threshold }) {
                Ok(()) => break,
                Err(e) if e.1 == "board_task_already_claimed" => break,
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

struct Claim<'a> {
    task_id: &'a str,
    root: &'a str,
    agent: &'a str,
    preset: &'a str,
    version: &'a str,
    stimulus: f64,
    threshold: f64,
}
fn claim_open(tx: &Transaction<'_>, c: Claim<'_>) -> PResult<()> {
    let Claim { task_id, root, agent, preset, version, stimulus, threshold } = c;
    let run = id("run");
    let input = format!(
        "Self-selected shared task board work. You responded to task {task_id} by your own response threshold; no controller assigned it. Read platform.board.read, then atomically claim it with platform.board.claim before acting. You are a peer Agent with no authority over other identities. Split only when independent work is useful; report progress; your final output is the durable contribution."
    );
    // Insert the Run first: board_tasks.run_id references it. The task UPDATE
    // below is the atomic claim gate; a lost race rolls the Run back.
    tx.execute(
        "INSERT INTO runs(id,instance_id,state,input,created_at) VALUES(?,?,'queued',?,?)",
        params![run, agent, input, now()],
    )?;
    let changed = tx.execute(
        "UPDATE board_tasks SET state='running',route_state='self-selected',instance_id=?,run_id=?,preset_name=?,preset_version=?,route_reason=? WHERE id=? AND state='pending' AND route_state='unassigned'",
        params![agent, run, preset, version, format!("self-selected: stimulus {stimulus:.3} vs threshold {threshold:.3}"), task_id],
    )?;
    if changed != 1 {
        tx.execute("DELETE FROM runs WHERE id=?", [&run])?;
        return Err(fault(409, "board_task_already_claimed"));
    }
    tx.execute("UPDATE instances SET state='busy' WHERE id=?", [agent])?;
    tx.execute(
        "INSERT INTO board_agents(instance_id,root_id,task_id,run_id,created_at) VALUES(?,?,?,?,?)",
        params![agent, root, task_id, run, now()],
    )?;
    crate::energy::reserve(tx, root, &run)?;
    event(tx, "swarm.self_selected", task_id, json!({"instance_id":agent,"preset_name":preset,"run_id":run,"stimulus":stimulus,"threshold":threshold}))?;
    event(tx, "run.queued", &run, json!({"instance_id":agent,"board_task_id":task_id,"self_selected":true}))?;
    Ok(())
}

/// Idle wandering burns colony energy; a starved colony retires its pool.
fn burn_idle(tx: &Transaction<'_>, standing: usize) -> PResult<()> {
    if standing == 0 {
        return Ok(());
    }
    let last: Option<i64> = tx.query_row(
        "SELECT max(created_at) FROM energy_ledger WHERE root_id='colony' AND kind='idle_burn'",
        [],
        |r| r.get::<_, Option<i64>>(0),
    )?;
    let at = now();
    let elapsed = at - last.unwrap_or(at);
    if elapsed < 60_000 {
        return Ok(());
    }
    let minutes = elapsed / 60_000;
    let cost = (standing as i64) * IDLE_BURN_PER_MIN * minutes;
    let spent: i64 = tx.query_row(
        "SELECT coalesce(sum(amount),0) FROM energy_ledger WHERE root_id='colony' AND kind='idle_burn'",
        [],
        |r| r.get(0),
    )?;
    if spent + cost > COLONY_BUDGET {
        // Colony starved: retire all wandering stock.
        let mut stmt = tx.prepare(
            "SELECT i.id FROM instances i WHERE i.state='ready' AND EXISTS(SELECT 1 FROM wander_stock w JOIN fleet_slots s ON s.instance_id=i.id WHERE w.fleet_id=s.fleet_id)",
        )?;
        let agents = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        for agent in agents {
            crate::lifecycle::drain(tx, &agent, false)?;
        }
        event(tx, "swarm.colony_starved", "colony", json!({"budget":COLONY_BUDGET,"spent":spent}))?;
        return Ok(());
    }
    tx.execute(
        "INSERT INTO energy_ledger(root_id,run_id,kind,amount,balance,created_at) VALUES('colony',NULL,'idle_burn',?,?,?)",
        params![cost, COLONY_BUDGET - spent - cost, at],
    )?;
    event(tx, "board.energy_posted", "colony", json!({"kind":"idle_burn","amount":cost,"standing":standing,"available":COLONY_BUDGET - spent - cost}))?;
    Ok(())
}

impl Store {
    /// Run one self-selection sweep with an explicit mode flag.
    pub fn self_select_sweep(&self, on: bool) -> PResult<()> {
        self.board_transaction(|tx| sweep_with(tx, on))
    }
    /// Operator view of the wandering pool and colony energy.
    pub fn swarm_pool(&self) -> PResult<Value> {
        self.transaction(|tx| {
            let mut stmt = tx.prepare(
                "SELECT json_object('instance_id',i.id,'preset_name',i.preset_name,'preset_version',i.preset_version,'state',i.state) FROM instances i WHERE i.state IN ('ready','busy') AND EXISTS(SELECT 1 FROM wander_stock w JOIN fleet_slots s ON s.instance_id=i.id WHERE w.fleet_id=s.fleet_id) ORDER BY i.created_at",
            )?;
            let peers = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .map(|r| Ok(serde_json::from_str::<Value>(&r?)?))
                .collect::<PResult<Vec<_>>>()?;
            let spent: i64 = tx.query_row(
                "SELECT coalesce(sum(amount),0) FROM energy_ledger WHERE root_id='colony' AND kind='idle_burn'",
                [],
                |r| r.get(0),
            )?;
            Ok(json!({
                "enabled": enabled(),
                "peers": peers,
                "pool_max": SELF_SELECT_POOL_MAX,
                "colony_budget": COLONY_BUDGET,
                "colony_spent": spent,
                "idle_burn_per_min": IDLE_BURN_PER_MIN,
            }))
        })
    }
}
