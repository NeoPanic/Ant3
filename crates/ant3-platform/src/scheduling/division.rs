//! Preset experience for response-threshold division of labor.
//!
//! Successes lower a preset's response threshold, definite failures raise it;
//! outcome-unknown and cancelled Runs are not experience. The selector reads
//! these statistics as evidence — the threshold model never grants authority.
use crate::{domain::*, store::Store};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

pub use ant3_domain::division::{cost_adjusted_threshold, response_probability, response_threshold, stimulus};

pub(crate) fn record_outcome(
    tx: &Transaction<'_>,
    preset_name: &str,
    preset_version: &str,
    success: bool,
    energy: i64,
) -> PResult<()> {
    let (s, f) = if success { (1, 0) } else { (0, 1) };
    tx.execute(
        "INSERT INTO preset_experience(preset_name,preset_version,successes,failures,energy_spent) VALUES(?,?,?,?,?)
         ON CONFLICT(preset_name,preset_version) DO UPDATE SET successes=successes+excluded.successes,failures=failures+excluded.failures,energy_spent=energy_spent+excluded.energy_spent",
        params![preset_name, preset_version, s, f, energy.max(0)],
    )?;
    Ok(())
}

fn stats(tx: &Transaction<'_>, name: &str, version: &str) -> PResult<(u32, u32, i64)> {
    let row: Option<(u32, u32, i64)> = tx
        .query_row(
            "SELECT successes,failures,energy_spent FROM preset_experience WHERE preset_name=? AND preset_version=?",
            params![name, version],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    Ok(row.unwrap_or((0, 0, 0)))
}

/// Mean energy per success relative to the reservation unit; neutral 1.0
/// until a preset has both successes and recorded spend.
fn efficiency(successes: u32, energy_spent: i64) -> f64 {
    if successes == 0 || energy_spent <= 0 {
        return 1.0;
    }
    (energy_spent as f64 / successes as f64) / crate::energy::ENERGY_RESERVE as f64
}

/// Experience evidence for one catalog entry: counts, adaptive threshold.
pub(crate) fn evidence(tx: &Transaction<'_>, name: &str, version: &str) -> PResult<Value> {
    let (successes, failures, energy_spent) = stats(tx, name, version)?;
    let base = response_threshold(successes, failures);
    let eff = efficiency(successes, energy_spent);
    Ok(json!({
        "successes": successes,
        "failures": failures,
        "energy_spent": energy_spent,
        "energy_per_success": if successes>0 { Some(energy_spent / successes as i64) } else { None },
        "cost_efficiency": ant3_domain::traces::quantize_strength(eff),
        "response_threshold": cost_adjusted_threshold(base, eff),
    }))
}

impl Store {
    /// Operator view of preset experience across the swarm.
    pub fn preset_experience(&self) -> PResult<Vec<Value>> {
        self.transaction(|tx| {
            let mut stmt = tx.prepare(
                "SELECT preset_name,preset_version,successes,failures,energy_spent FROM preset_experience ORDER BY preset_name,preset_version",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, u32>(2)?,
                        r.get::<_, u32>(3)?,
                        r.get::<_, i64>(4)?,
                    ))
                })?
                .map(|row| {
                    let (name, version, successes, failures, energy_spent) = row?;
                    let eff = efficiency(successes, energy_spent);
                    Ok(json!({
                        "preset_name": name,
                        "preset_version": version,
                        "successes": successes,
                        "failures": failures,
                        "energy_spent": energy_spent,
                        "cost_efficiency": ant3_domain::traces::quantize_strength(eff),
                        "response_threshold": cost_adjusted_threshold(response_threshold(successes, failures), eff),
                    }))
                })
                .collect::<PResult<Vec<_>>>()?;
            Ok(rows)
        })
    }
}