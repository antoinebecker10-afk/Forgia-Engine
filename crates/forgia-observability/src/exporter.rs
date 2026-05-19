//! exporter.rs — Écrit forgia2_rpg_health.json à 1Hz.
//!
//! sys_write_rpg_health_json : throttlé par Local<f32>.
//! sys_sensor_liveness_watchdog : log warn si stale, 1 fois/minute.
//!
//! Vague 5 Phase 5b Session A — Renamé forgia_rpg_health.json → forgia2_rpg_health.json
//! (sensor canonique Phase 5 ARCHITECTURE.md §9, format {id, severity, next_step}).

use bevy::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

use crate::config::RpgMonitorConfig;
use crate::state::{CheckResult, RpgHealthState, Severity};

const OUTPUT_PATH: &str = "forgia2_rpg_health.json";

// ─────────────────────────── Structures de sérialisation ───────────────────────────

#[derive(Serialize)]
struct HealthJson<'a> {
    timestamp_secs: f32,
    schema_version: u32,
    overall_severity: &'a Severity,
    overall_message: &'a str,
    next_step: &'a str,
    checks: HashMap<&'static str, &'a CheckResult>,
    tick_count: u64,
}

// ─────────────────────────── sys_write_rpg_health_json ───────────────────────────

pub fn sys_write_rpg_health_json(
    state: Res<RpgHealthState>,
    time: Res<Time>,
    config: Res<RpgMonitorConfig>,
    mut last_write: Local<f32>,
    mut warned_write: Local<bool>,
) {
    *last_write += time.delta_secs();
    if *last_write < config.global.tick_interval_secs {
        return;
    }
    *last_write = 0.0;

    // Construire la map checks sérialisable
    let checks_ref: HashMap<&'static str, &CheckResult> = state
        .checks
        .iter()
        .map(|(&k, v)| (k, v))
        .collect();

    let payload = HealthJson {
        timestamp_secs: time.elapsed_secs(),
        schema_version: 1,
        overall_severity: &state.last_severity,
        overall_message: &state.last_message,
        next_step: &state.last_next_step,
        checks: checks_ref,
        tick_count: state.tick_count,
    };

    match serde_json::to_string_pretty(&payload) {
        Ok(json_str) => {
            if let Err(e) = std::fs::write(OUTPUT_PATH, &json_str) {
                if !*warned_write {
                    warn!("[rpg-monitor] Cannot write {OUTPUT_PATH}: {e}");
                    *warned_write = true;
                }
            } else {
                // Reset warned si écriture réussie
                *warned_write = false;
            }
        }
        Err(e) => {
            warn!("[rpg-monitor] JSON serialization error: {e}");
        }
    }
}

// ─────────────────────────── sys_sensor_liveness_watchdog ───────────────────────────

/// Log un health_alert si des sensors sont stale. Throttlé à 1 log/minute.
pub fn sys_sensor_liveness_watchdog(
    state: Res<RpgHealthState>,
    time: Res<Time>,
    mut cooldown: Local<f32>,
) {
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }

    // Vérifier si CHK-5 (sensor liveness) est en warn/critical
    if let Some(chk5) = state.checks.get("chk5_sensor_liveness") {
        if chk5.severity != crate::state::Severity::Ok {
            warn!(
                "[rpg-monitor] HEALTH ALERT: {} — {}",
                chk5.severity,
                chk5.message
            );
            if !chk5.next_step.is_empty() {
                warn!("[rpg-monitor] Next step: {}", chk5.next_step);
            }
            *cooldown = 60.0;
        }
    }

    // Vérifier aussi la sévérité globale
    if state.last_severity == crate::state::Severity::Critical {
        warn!(
            "[rpg-monitor] CRITICAL: {} | next_step: {}",
            state.last_message, state.last_next_step
        );
        *cooldown = 60.0;
    }
}
