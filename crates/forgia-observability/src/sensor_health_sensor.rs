//! sensor_health_sensor.rs — Producteur `forgia2_sensor_health.json` (1Hz, meta).
//!
//! Lit les timestamps des 12 forgia2_*.json canoniques (Sessions A+B+C) et
//! expose un résumé CHK-5 canonisé : count present/missing/stale.
//!
//! Story-469 — Vague 5 Phase 5b Session C.

use bevy::prelude::*;
use std::time::SystemTime;

/// 12 sensors canoniques attendus (V5 cible — chunks reportable Session D).
const EXPECTED_SENSORS: &[&str] = &[
    "forgia2_health.json",
    "forgia2_rpg_health.json",
    "forgia2_arena.json",
    "forgia2_combat.json",
    "forgia2_perf.json",
    "forgia2_entities.json",
    "forgia2_memory.json",
    "forgia2_assets.json",
    "forgia2_vram.json",
    "forgia2_lifecycle.json",
    "forgia2_watchdog.json",
    "forgia2_audio.json",
    "forgia2_input.json",
    "forgia2_sensor_health.json",
    "forgia2_sensor_io.json",
];
const STALE_THRESHOLD_SECS: u64 = 10;

/// Pur — extrait pour tests headless.
pub fn severity_for_sensor_health(missing: usize, stale: usize) -> (&'static str, &'static str) {
    if missing >= 3 {
        (
            "critical",
            "≥3 sensors missing — observability degraded (verify plugin wiring)",
        )
    } else if missing > 0 || stale > 0 {
        (
            "warn",
            "1-2 sensors missing/stale — producer may not be tick'd",
        )
    } else {
        ("ok", "")
    }
}

pub fn sys_write_sensor_health(time: Res<Time>, mut accum: Local<f32>) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let now = SystemTime::now();
    let mut stale: Vec<&str> = Vec::new();
    let mut missing: Vec<&str> = Vec::new();
    let mut present = 0u32;

    for path in EXPECTED_SENSORS {
        match std::fs::metadata(path).and_then(|m| m.modified()) {
            Ok(mtime) => {
                let age = now.duration_since(mtime).map(|d| d.as_secs()).unwrap_or(0);
                if age > STALE_THRESHOLD_SECS {
                    stale.push(path);
                }
                present += 1;
            }
            Err(_) => missing.push(path),
        }
    }

    let (severity, next_step) = severity_for_sensor_health(missing.len(), stale.len());

    let missing_json = serde_json::to_string(&missing).unwrap_or_else(|_| "[]".to_string());
    let stale_json = serde_json::to_string(&stale).unwrap_or_else(|_| "[]".to_string());

    let json = format!(
        r#"{{"id":"sensor_health","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"expected":{},"present":{},"missing":{},"stale":{},"missing_paths":{},"stale_paths":{}}}"#,
        time.elapsed_secs(),
        EXPECTED_SENSORS.len(),
        present,
        missing.len(),
        stale.len(),
        missing_json,
        stale_json,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_sensor_health.json", json) {
        warn!("[forgia-observability] sensor_health sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_all_present() {
        assert_eq!(severity_for_sensor_health(0, 0).0, "ok");
    }

    #[test]
    fn severity_warn_one_missing() {
        let (sev, next) = severity_for_sensor_health(1, 0);
        assert_eq!(sev, "warn");
        assert!(next.contains("missing"));
    }

    #[test]
    fn severity_warn_stale_only() {
        assert_eq!(severity_for_sensor_health(0, 2).0, "warn");
    }

    #[test]
    fn severity_critical_three_or_more_missing() {
        let (sev, next) = severity_for_sensor_health(3, 0);
        assert_eq!(sev, "critical");
        assert!(next.contains("degraded"));
    }

    #[test]
    fn expected_sensors_count_is_15() {
        assert_eq!(EXPECTED_SENSORS.len(), 15);
    }
}
