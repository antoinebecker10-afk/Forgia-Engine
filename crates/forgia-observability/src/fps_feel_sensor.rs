//! fps_feel_sensor.rs — Producteur `forgia2_fps_feel.json` (1Hz, cross-mode).
//!
//! Story-528 phase 1 AC7. Expose les métriques FPS feel pour audit accessibilité
//! cartoon famille : taux d'engagement aim assist, usages dash, hit feedbacks.
//!
//! ## Schema JSON
//!
//! ```json
//! {
//!   "id": "fps_feel",
//!   "severity": "ok|warn",
//!   "next_step": "...",
//!   "timestamp_secs": 12.5,
//!   "dash_uses_per_sec": 0.5,
//!   "hit_feedbacks_per_sec": 2.3,
//!   "aim_assist_engagements_per_sec": 0.0,
//!   "dash_uses_total": 8,
//!   "hit_feedbacks_total": 42,
//!   "aim_assist_engagements_total": 0
//! }
//! ```
//!
//! Producteurs des counters : forgia-player (dash), forgia-effects (hitmarker), forgia-fps (aim assist phase 2).
//! Incréments via `FpsFeelMetrics` Resource — write-only, lock-free hot path.

use bevy::prelude::*;
use forgia_core::prelude::FpsFeelMetrics;

/// État interne du sensor : dernier write + snapshot pour calcul rate /sec.
#[derive(Resource, Default)]
pub struct FpsFeelSensorState {
    pub last_write_secs: f32,
    pub last_dash_total: u64,
    pub last_hit_total: u64,
    pub last_aim_total: u64,
}

const SENSOR_PATH: &str = "forgia2_fps_feel.json";

pub fn sys_write_fps_feel_sensor(
    time: Res<Time>,
    metrics: Res<FpsFeelMetrics>,
    mut state: ResMut<FpsFeelSensorState>,
) {
    let now = time.elapsed_secs();
    let dt = now - state.last_write_secs;
    if dt < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let dash_delta = metrics.dash_uses_total.saturating_sub(state.last_dash_total) as f32;
    let hit_delta = metrics.hit_feedbacks_total.saturating_sub(state.last_hit_total) as f32;
    let aim_delta = metrics
        .aim_assist_engagements_total
        .saturating_sub(state.last_aim_total) as f32;

    state.last_dash_total = metrics.dash_uses_total;
    state.last_hit_total = metrics.hit_feedbacks_total;
    state.last_aim_total = metrics.aim_assist_engagements_total;

    let inv_dt = if dt > 0.0 { 1.0 / dt } else { 0.0 };
    let dash_rate = dash_delta * inv_dt;
    let hit_rate = hit_delta * inv_dt;
    let aim_rate = aim_delta * inv_dt;

    // Severity : "warn" si AUCUN feedback et AUCUN dash sur >30s en gameplay actif.
    // Phase 1 : sensor passif, severity=ok par défaut (gating fin par story-528 polish).
    let (severity, next_step) = ("ok", "");

    let json = format!(
        r#"{{"id":"fps_feel","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"dash_uses_per_sec":{:.2},"hit_feedbacks_per_sec":{:.2},"aim_assist_engagements_per_sec":{:.2},"dash_uses_total":{},"hit_feedbacks_total":{},"aim_assist_engagements_total":{}}}"#,
        now,
        dash_rate,
        hit_rate,
        aim_rate,
        metrics.dash_uses_total,
        metrics.hit_feedbacks_total,
        metrics.aim_assist_engagements_total,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[forgia-observability] fps_feel_sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_default_zero() {
        let m = FpsFeelMetrics::default();
        assert_eq!(m.dash_uses_total, 0);
        assert_eq!(m.hit_feedbacks_total, 0);
        assert_eq!(m.aim_assist_engagements_total, 0);
    }

    #[test]
    fn state_default_zero() {
        let s = FpsFeelSensorState::default();
        assert_eq!(s.last_write_secs, 0.0);
        assert_eq!(s.last_dash_total, 0);
    }

    #[test]
    fn saturating_sub_no_overflow() {
        let m = FpsFeelMetrics {
            dash_uses_total: 5,
            ..Default::default()
        };
        let s = FpsFeelSensorState {
            last_dash_total: 10,
            ..Default::default()
        };
        // Counter reset edge case (improbable mais robuste) : pas de panic.
        let delta = m.dash_uses_total.saturating_sub(s.last_dash_total);
        assert_eq!(delta, 0);
    }
}
