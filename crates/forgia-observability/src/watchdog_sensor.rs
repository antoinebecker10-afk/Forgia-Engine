//! watchdog_sensor.rs — Producteur `forgia2_watchdog.json` (1Hz, cross-mode).
//!
//! Tick counter en `First` schedule (avant tous GameSets) + lag detection.
//! Lag frame = `delta_secs > 50ms` (proxy stutter 20fps).
//!
//! Note : freeze detection complète "no tick in last 5s" nécessite OS thread
//! externe — HORS scope Session C. Antoine peut détecter manuellement en
//! relisant forgia2_watchdog.json : si `ticks` stagne entre 2 reads = freeze.
//!
//! Story-469 — Vague 5 Phase 5b Session C.

use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct GameTickCounter {
    pub ticks: u64,
    pub last_dt_ms: f32,
    pub total_lag_frames: u32,
    pub consecutive_lag_frames: u32,
}

const LAG_THRESHOLD_MS: f32 = 50.0;

/// Pur — extrait pour tests headless.
pub fn severity_for_watchdog(consecutive_lag: u32) -> (&'static str, &'static str) {
    if consecutive_lag > 30 {
        (
            "critical",
            "consecutive_lag_frames > 30 — sustained freeze (Tracy, GPU profile)",
        )
    } else if consecutive_lag > 10 {
        ("warn", "consecutive_lag_frames > 10 — stutter cluster")
    } else {
        ("ok", "")
    }
}

/// Met à jour les compteurs. Run en `First` schedule pour capturer le vrai dt
/// avant que les GameSets ne perturbent.
pub fn sys_update_tick_counter(time: Res<Time>, mut counter: ResMut<GameTickCounter>) {
    let dt_ms = time.delta_secs() * 1000.0;
    counter.ticks = counter.ticks.saturating_add(1);
    counter.last_dt_ms = dt_ms;
    if dt_ms > LAG_THRESHOLD_MS {
        counter.total_lag_frames = counter.total_lag_frames.saturating_add(1);
        counter.consecutive_lag_frames = counter.consecutive_lag_frames.saturating_add(1);
    } else {
        counter.consecutive_lag_frames = 0;
    }
}

pub fn sys_write_watchdog_sensor(
    time: Res<Time>,
    counter: Res<GameTickCounter>,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (severity, next_step) = severity_for_watchdog(counter.consecutive_lag_frames);

    let json = format!(
        r#"{{"id":"watchdog","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"ticks":{},"last_dt_ms":{:.2},"total_lag_frames":{},"consecutive_lag_frames":{}}}"#,
        time.elapsed_secs(),
        counter.ticks,
        counter.last_dt_ms,
        counter.total_lag_frames,
        counter.consecutive_lag_frames,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_watchdog.json", json) {
        warn!("[forgia-observability] watchdog sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_below_10_consecutive() {
        assert_eq!(severity_for_watchdog(5).0, "ok");
    }

    #[test]
    fn severity_warn_between_10_30() {
        let (sev, next) = severity_for_watchdog(20);
        assert_eq!(sev, "warn");
        assert!(next.contains("stutter"));
    }

    #[test]
    fn severity_critical_above_30() {
        let (sev, next) = severity_for_watchdog(40);
        assert_eq!(sev, "critical");
        assert!(next.contains("freeze"));
    }

    #[test]
    fn tick_counter_default_zero() {
        let c = GameTickCounter::default();
        assert_eq!(c.ticks, 0);
        assert_eq!(c.last_dt_ms, 0.0);
        assert_eq!(c.total_lag_frames, 0);
        assert_eq!(c.consecutive_lag_frames, 0);
    }

    #[test]
    fn lag_threshold_is_50ms() {
        assert_eq!(LAG_THRESHOLD_MS, 50.0);
    }
}
