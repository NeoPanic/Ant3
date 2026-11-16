//! Preset experience for response-threshold division of labor.
//!
//! Successes lower a preset's response threshold, definite failures raise it;
//! outcome-unknown and cancelled Runs are not experience. The selector reads
//! these statistics as evidence — the threshold model never grants authority.
use crate::{domain::*, store::Store};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

pub use ant3_domain::division::{response_probability, response_threshold, stimulus};

pub(crate) fn record_outcome(
    tx: &Transaction<'_>,
    preset_name: &str,
    preset_version: &str,
    success: bool,
) -> PResult<()> {
    let (s, f) = if success { (1, 0) } else { (0, 1) };
    tx.execute(
        "INSERT INTO preset_experience(preset_name,preset_version,successes,failures) VALUES(?,?,?,?)
         ON CONFLICT(preset_name,preset_version) DO UPDATE SET successes=successes+excluded.successes,failures=failures+excluded.failures",
        params![preset_name, preset_version, s, f],
    )?;
    Ok(())
}

fn stats(tx: &Transaction<'_>, name: &str, version: &str) -> PResult<(u32, u32)> {
    let row: Option<(u32, u32)> = tx
        .query_row(
            "SELECT successes,failures FROM preset_experience WHERE preset_name=? AND preset_version=?",
            params![name, version],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row.unwrap_or((0, 0)))
}

/// Experience evidence for one catalog entry: counts, adaptive threshold.
pub(crate) fn evidence(tx: &Transaction<'_>, name: &str, version: &str) -> PResult<Value> {
    let (successes, failures) = stats(tx, name, version)?;
    Ok(json!({
        "successes": successes,
        "failures": failures,
        "response_threshold": response_threshold(successes, failures),
    }))
}

impl Store {
    /// Operator view of preset experience across the swarm.
    pub fn preset_experience(&self) -> PResult<Vec<Value>> {
        self.transaction(|tx| {
            let mut stmt = tx.prepare(
                "SELECT preset_name,preset_version,successes,failures FROM preset_experience ORDER BY preset_name,preset_version",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, u32>(2)?,
                        r.get::<_, u32>(3)?,
                    ))
                })?
                .map(|row| {
                    let (name, version, successes, failures) = row?;
                    Ok(json!({
                        "preset_name": name,
                        "preset_version": version,
                        "successes": successes,
                        "failures": failures,
                        "response_threshold": response_threshold(successes, failures),
                    }))
                })
                .collect::<PResult<Vec<_>>>()?;
            Ok(rows)
        })
    }
}