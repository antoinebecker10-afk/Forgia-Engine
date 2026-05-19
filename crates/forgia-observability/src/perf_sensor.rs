//! perf_sensor.rs — Producteur `forgia2_perf.json` (1Hz, cross-mode).
//!
//! Lit `FrameTimeDiagnosticsPlugin` (Bevy 0.18) :
//! - `avg_ms` : moyenne ring-buffer (~120 samples)
//! - `min_ms` / `max_ms` : fold sur `values()` (la struct `Diagnostic` n'expose
//!   pas de min/max natifs — voir bevy-specialist research 2026-05-19)
//! - `fps_smoothed` : EMA depuis le diagnostic dédié FPS
//!
//! Severity heuristic :
//! - `warn`     : avg > 25 ms (~40 FPS)
//! - `critical` : avg > 50 ms (~20 FPS)
//!
//! Story-467 — Vague 5 Phase 5b Session B.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

/// Pur — extrait pour tests headless sans App Bevy.
pub fn severity_for_perf(avg_ms: f64) -> (&'static str, &'static str) {
    if avg_ms > 50.0 {
        (
            "critical",
            "frame_time avg > 50ms — investigate hot systems (Tracy, forgia2_entities)",
        )
    } else if avg_ms > 25.0 {
        ("warn", "frame_time avg > 25ms — perf budget approaching")
    } else {
        ("ok", "")
    }
}

pub fn sys_write_perf_sensor(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (avg_ms, min_ms, max_ms, samples) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .map(|ft| {
            let avg = ft.average().unwrap_or(0.0);
            let (mn, mx, n) = ft.values().fold(
                (f64::MAX, 0.0_f64, 0usize),
                |(mn, mx, n), v| (mn.min(*v), mx.max(*v), n + 1),
            );
            (avg, if n > 0 { mn } else { 0.0 }, mx, n)
        })
        .unwrap_or((0.0, 0.0, 0.0, 0));

    let fps_smoothed = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);

    let (severity, next_step) = severity_for_perf(avg_ms);

    let json = format!(
        r#"{{"id":"perf","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"frame_time_avg_ms":{:.3},"frame_time_min_ms":{:.3},"frame_time_max_ms":{:.3},"fps_smoothed":{:.1},"samples":{}}}"#,
        time.elapsed_secs(),
        avg_ms,
        min_ms,
        max_ms,
        fps_smoothed,
        samples,
    );

    if let Err(e) = std::fs::write("forgia2_perf.json", &json) {
        warn!("[forgia-observability] perf sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_below_25ms() {
        let (sev, next) = severity_for_perf(16.6);
        assert_eq!(sev, "ok");
        assert!(next.is_empty());
    }

    #[test]
    fn severity_warn_25_to_50ms() {
        let (sev, next) = severity_for_perf(33.3);
        assert_eq!(sev, "warn");
        assert!(next.contains("perf budget"));
    }

    #[test]
    fn severity_critical_above_50ms() {
        let (sev, next) = severity_for_perf(60.0);
        assert_eq!(sev, "critical");
        assert!(next.contains("Tracy"));
    }

    #[test]
    fn severity_boundary_25_is_ok() {
        assert_eq!(severity_for_perf(25.0).0, "ok");
        assert_eq!(severity_for_perf(25.001).0, "warn");
    }
}
