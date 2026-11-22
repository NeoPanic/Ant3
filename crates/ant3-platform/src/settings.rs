//! Operator-tunable runtime mechanics, persisted in the settings table.
//!
//! Values are read on every use so a change applies to the next sweep, deposit
//! or settlement without a restart. Defaults honour the legacy environment
//! switches (`ANT3_SELF_SELECT`, `ANT3_BOARD_EXPLORATION`) on first boot; once
//! saved, the stored value wins.
use crate::{domain::*, store::Store};
use ant3_domain::energy::{
    ENERGY_ACCEPT_REFUND, ENERGY_COMPLETE_REFUND, ENERGY_DEFAULT, ENERGY_RESERVE,
    ENERGY_UNKNOWN_COST, TOKENS_PER_ENERGY,
};
use ant3_domain::division::{IDLE_BURN_PER_MIN, POOR_PRESET_FAILURES, SELF_SELECT_POOL_MAX};
use ant3_domain::traces::{PERCEPTION_THRESHOLD, TRACE_HALF_LIFE_MS, TRACE_MAX_STRENGTH};
use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Upper bound for the colony energy pool (2^30). Operators may park a large
/// pool for long experiments; the ledger itself stays i64.
pub const MAX_COLONY_BUDGET: i64 = 1_073_741_824;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub traces: TraceSettings,
    pub self_select: SelfSelectSettings,
    pub energy: EnergySettings,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TraceSettings {
    pub half_life_minutes: i64,
    pub max_strength: f64,
    pub perception_threshold: f64,
    pub exploration: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SelfSelectSettings {
    pub enabled: bool,
    pub pool_max: i64,
    pub idle_burn_per_min: i64,
    pub colony_budget: i64,
    pub poor_failures: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnergySettings {
    pub default_budget: i64,
    pub reserve: i64,
    pub unknown_cost: i64,
    pub complete_refund: i64,
    pub accept_refund: i64,
    pub tokens_per_energy: i64,
}

fn env_on(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1")
}

impl Default for TraceSettings {
    fn default() -> Self {
        Self {
            half_life_minutes: TRACE_HALF_LIFE_MS / 60_000,
            max_strength: TRACE_MAX_STRENGTH,
            perception_threshold: PERCEPTION_THRESHOLD,
            exploration: env_on("ANT3_BOARD_EXPLORATION"),
        }
    }
}
impl Default for SelfSelectSettings {
    fn default() -> Self {
        Self {
            enabled: env_on("ANT3_SELF_SELECT"),
            pool_max: SELF_SELECT_POOL_MAX as i64,
            idle_burn_per_min: IDLE_BURN_PER_MIN,
            colony_budget: 200_000,
            poor_failures: POOR_PRESET_FAILURES as i64,
        }
    }
}
impl Default for EnergySettings {
    fn default() -> Self {
        Self {
            default_budget: ENERGY_DEFAULT,
            reserve: ENERGY_RESERVE,
            unknown_cost: ENERGY_UNKNOWN_COST,
            complete_refund: ENERGY_COMPLETE_REFUND,
            accept_refund: ENERGY_ACCEPT_REFUND,
            tokens_per_energy: TOKENS_PER_ENERGY,
        }
    }
}

impl TraceSettings {
    pub fn half_life_ms(&self) -> i64 {
        self.half_life_minutes * 60_000
    }
}

impl Settings {
    pub(crate) fn load(tx: &Transaction<'_>) -> PResult<Self> {
        let mut settings = Self::default();
        let mut stmt = tx.prepare("SELECT key,value FROM settings")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        for (key, body) in rows {
            match key.as_str() {
                "traces" => settings.traces = serde_json::from_str(&body)?,
                "self_select" => settings.self_select = serde_json::from_str(&body)?,
                "energy" => settings.energy = serde_json::from_str(&body)?,
                _ => {}
            }
        }
        settings.validate()?;
        Ok(settings)
    }
    pub(crate) fn save(tx: &Transaction<'_>, settings: &Self) -> PResult<()> {
        settings.validate()?;
        for (key, section) in [
            ("traces", serde_json::to_value(&settings.traces)?),
            ("self_select", serde_json::to_value(&settings.self_select)?),
            ("energy", serde_json::to_value(&settings.energy)?),
        ] {
            tx.execute(
                "INSERT INTO settings(key,value,updated_at) VALUES(?,?,?)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at",
                params![key, section.to_string(), now()],
            )?;
        }
        Ok(())
    }
    pub(crate) fn merged(base: &Self, patch: &Value) -> PResult<Self> {
        let mut current = serde_json::to_value(base)?;
        merge(&mut current, patch);
        serde_json::from_value(current).map_err(|_| fault(400, "invalid_settings"))
    }
    pub fn validate(&self) -> PResult<()> {
        let t = &self.traces;
        if !(1..=1440).contains(&t.half_life_minutes)
            || !(1.0..=10_000.0).contains(&t.max_strength)
            || !(0.0..=t.max_strength).contains(&t.perception_threshold)
        {
            return Err(fault(400, "invalid_settings"));
        }
        let s = &self.self_select;
        if !(1..=64).contains(&s.pool_max)
            || s.idle_burn_per_min != 0
            || !(0..=MAX_COLONY_BUDGET).contains(&s.colony_budget)
            || !(1..=100).contains(&s.poor_failures)
        {
            return Err(fault(400, "invalid_settings"));
        }
        let e = &self.energy;
        if !(1..=ant3_domain::energy::ENERGY_MAX).contains(&e.default_budget)
            || e.reserve != 0
            || !(0..=1_000_000).contains(&e.unknown_cost)
            || !(0..=1_000_000).contains(&e.complete_refund)
            || !(0..=1_000_000).contains(&e.accept_refund)
            || e.tokens_per_energy != 1
        {
            return Err(fault(400, "invalid_settings"));
        }
        Ok(())
    }
    pub fn to_json(&self) -> Value {
        json!({
            "traces": self.traces,
            "self_select": self.self_select,
            "energy": self.energy,
        })
    }
}

fn merge(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                match target.get_mut(key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (target, patch) => {
            *target = patch.clone();
        }
    }
}

impl Store {
    pub fn settings_config(&self) -> PResult<Settings> {
        self.transaction(Settings::load)
    }
    pub fn settings(&self) -> PResult<Value> {
        Ok(self.settings_config()?.to_json())
    }
    pub fn update_settings(&self, patch: Value) -> PResult<Value> {
        self.transaction(|tx| {
            let next = Settings::merged(&Settings::load(tx)?, &patch)?;
            Settings::save(tx, &next)?;
            Ok(next.to_json())
        })
    }
}
