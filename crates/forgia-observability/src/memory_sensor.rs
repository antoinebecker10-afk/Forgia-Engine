//! memory_sensor.rs — Producteur `forgia2_memory.json` (1Hz, cross-mode).
//!
//! Lit RAM process via `sysinfo` sur un worker dédié (refresh cooldownné 5s).
//! Le thread de jeu relit seulement un entier atomique : l'appel Windows ne peut
//! donc plus provoquer de hitch. VRAM = stub `"N/A"` honnête — wgpu 0.18 n'expose pas
//! d'API memory budget cross-backend (Vulkan/DX12/Metal divergents).
//!
//! Severity heuristic :
//! - `warn`     : RAM > 4096 MB
//! - `critical` : RAM > 8192 MB
//!
//! Story-467 — Vague 5 Phase 5b Session B.

use bevy::prelude::*;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{sync_channel, SyncSender},
    Arc,
};
use std::thread;
use sysinfo::System;

pub struct MemSensorState {
    request_tx: Option<SyncSender<()>>,
    cached_ram_bytes: Arc<AtomicU64>,
    last_request_secs: f32,
}

impl Default for MemSensorState {
    fn default() -> Self {
        Self {
            request_tx: None,
            cached_ram_bytes: Arc::new(AtomicU64::new(0)),
            last_request_secs: f32::NEG_INFINITY,
        }
    }
}

/// Démarre un worker à capacité 1 : une mesure déjà en attente n'est jamais
/// empilée. Le PID est capturé ici (et non dans le worker), sinon `sysinfo`
/// mesurerait le thread worker au lieu du processus jeu.
fn start_memory_probe() -> Option<(SyncSender<()>, Arc<AtomicU64>)> {
    let pid = sysinfo::get_current_pid().ok()?;
    let (request_tx, request_rx) = sync_channel(1);
    let cached_ram_bytes = Arc::new(AtomicU64::new(0));
    let cached_for_worker = Arc::clone(&cached_ram_bytes);
    thread::Builder::new()
        .name("forgia-memory-probe".to_string())
        .spawn(move || {
            let mut system = System::new();
            while request_rx.recv().is_ok() {
                system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                let bytes = system
                    .process(pid)
                    .map(|process| process.memory())
                    .unwrap_or(0);
                cached_for_worker.store(bytes, Ordering::Relaxed);
            }
        })
        .ok()?;
    Some((request_tx, cached_ram_bytes))
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
    if state.request_tx.is_none() {
        if let Some((request_tx, cached_ram_bytes)) = start_memory_probe() {
            state.request_tx = Some(request_tx);
            state.cached_ram_bytes = cached_ram_bytes;
        }
    }
    if now - state.last_request_secs > 5.0 {
        if let Some(request_tx) = &state.request_tx {
            // `try_send` est essentiel : le thread jeu ne patiente jamais sur
            // l'API Windows, même si une mesure précédente est encore en cours.
            let _ = request_tx.try_send(());
        }
        state.last_request_secs = now;
    }

    let ram_bytes = state.cached_ram_bytes.load(Ordering::Relaxed);
    let ram_mb = ram_bytes as f64 / 1024.0 / 1024.0;
    let (severity, next_step) = severity_for_memory(ram_mb);

    let json = format!(
        r#"{{"id":"memory","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"ram_bytes":{},"ram_mb":{:.1},"vram_status":"N/A — wgpu adapter telemetry custom needed"}}"#,
        time.elapsed_secs(),
        ram_bytes,
        ram_mb,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_memory.json", json) {
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
