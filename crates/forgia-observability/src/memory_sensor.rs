//! memory_sensor.rs — Producteur `forgia2_memory.json` (1Hz, cross-mode).
//!
//! Lit RAM process via `sysinfo` (refresh cooldownné 5s pour absorber ~2ms
//! coût Windows API). VRAM = stub `"N/A"` honnête — wgpu 0.18 n'expose pas
//! d'API memory budget cross-backend (Vulkan/DX12/Metal divergents).
//!
//! Severity heuristic :
//! - `warn`     : RAM > 4096 MB
//! - `critical` : RAM > 8192 MB
//!
//! Story-467 — Vague 5 Phase 5b Session B.

use bevy::prelude::*;
use sysinfo::System;

#[derive(Default)]
pub struct MemSensorState {
    system: Option<System>,
    last_refresh_secs: f32,
    cached_ram_bytes: u64,
}

/// Pur — extrait pour tests headless.
pub fn severity_for_memory(ram_mb: f64) -> (&'static str, &'static str) {
    if ram_mb > 8192.0 {
        (
            "critical",
            "RAM > 8GB — investigate leaks (forgia2_entities, sensor_health)",
        )
    } else if ram_mb > 4096.0 {
        ("warn", "RAM > 4GB — approach budget")
    } else {
        ("ok", "")
    }
}

pub fn sys_write_memory_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut state: Local<MemSensorState>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let now = time.elapsed_secs();
    if state.system.is_none() || now - state.last_refresh_secs > 5.0 {
        let sys = state.system.get_or_insert_with(System::new);
        if let Ok(pid) = sysinfo::get_current_pid() {
            // Story-592 (M0.5, audit 2026-06-10 P1) : rafraîchir UNIQUEMENT notre
            // process. `ProcessesToUpdate::All` énumérait tous les process Windows
            // sur le thread de jeu → stutter métronome période 5,01 s mesuré
            // (forgia2_lag_events.json, spikes 30-50 ms). Some(&[pid]) ≈ ~2 ms.
            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            state.cached_ram_bytes = sys.process(pid).map(|p| p.memory()).unwrap_or(0);
        }
        state.last_refresh_secs = now;
    }

    let ram_bytes = state.cached_ram_bytes;
    let ram_mb = ram_bytes as f64 / 1024.0 / 1024.0;
    let (severity, next_step) = severity_for_memory(ram_mb);

    let json = format!(
        r#"{{"id":"memory","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"ram_bytes":{},"ram_mb":{:.1},"vram_status":"N/A — wgpu adapter telemetry custom needed"}}"#,
        time.elapsed_secs(),
        ram_bytes,
        ram_mb,
    );

    if let Err(e) = std::fs::write("forgia2_memory.json", &json) {
        warn!("[forgia-observability] memory sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_below_4gb() {
        let (sev, next) = severity_for_memory(1024.0);
        assert_eq!(sev, "ok");
        assert!(next.is_empty());
    }

    #[test]
    fn severity_warn_between_4_8gb() {
        let (sev, next) = severity_for_memory(5120.0);
        assert_eq!(sev, "warn");
        assert!(next.contains("approach budget"));
    }

    #[test]
    fn severity_critical_above_8gb() {
        let (sev, next) = severity_for_memory(10240.0);
        assert_eq!(sev, "critical");
        assert!(next.contains("leaks"));
    }

    #[test]
    fn severity_boundary_4096_is_ok() {
        assert_eq!(severity_for_memory(4096.0).0, "ok");
        assert_eq!(severity_for_memory(4096.001).0, "warn");
    }
}
