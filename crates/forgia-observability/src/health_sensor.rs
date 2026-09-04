//! health_sensor.rs — Producteur forgia_health.json (1Hz, cross-mode).
//!
//! Agrégateur minimal : lit RpgHealthState si présent (mode RPG),
//! sinon écrit un snapshot ok par défaut (mode FPS, menu, etc.).
//!
//! Ce sensor est DISTINCT de forgia_rpg_health.json (détail complet 6 checks RPG-only).
//! forgia_health.json = probe cross-mode minimaliste pour CHK-5 liveness.

use bevy::prelude::*;

use crate::state::{RpgHealthState, Severity};

/// Système sensor 1Hz — écrit forgia_health.json.
pub fn sys_write_health_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    rpg_state: Option<Res<RpgHealthState>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (severity_str, source_str, next_step_str, checks_count) = if let Some(ref state) = rpg_state
    {
        let sev = match state.last_severity {
            Severity::Ok => "ok",
            Severity::Warn => "warn",
            Severity::Critical => "critical",
        };
        let next = if state.last_next_step.is_empty() {
            ""
        } else {
            state.last_next_step.as_str()
        };
        (sev, "rpg_health", next, state.checks.len() as u32)
    } else {
        ("ok", "fps_default", "", 0u32)
    };

    let json = format!(
        r#"{{
  "id": "health",
  "severity": "{}",
  "next_step": "{}",
  "timestamp_secs": {:.1},
  "overall_severity_source": "{}",
  "checks_count": {}
}}"#,
        severity_str,
        next_step_str,
        time.elapsed_secs(),
        source_str,
        checks_count,
    );

    // Vague 5 Phase 5b Session A — Renamé forgia_health.json → forgia2_health.json
    // (sensor canonique Phase 5 ARCHITECTURE.md §9).
    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_health.json", json) {
        warn!("[forgia-observability] health sensor write failed: {e}");
    }
}
