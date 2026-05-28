//! Category 1 — System : perf macro, sensor health, memory, entities, watchdog.

use super::{fmt_opt, fmt_opt_f64, fmt_opt_str, DebugCategory};
use crate::snapshot::SensorSnapshot;
use bevy_egui::egui;

pub struct SystemCategory;

impl DebugCategory for SystemCategory {
    fn draw(&self, ui: &mut egui::Ui, snap: &SensorSnapshot) {
        ui.label(format!("FPS:           {}", fmt_opt_f64(snap.perf.fps, 1)));
        ui.label(format!(
            "frame_ms:      {}",
            fmt_opt_f64(snap.perf.frame_ms, 2)
        ));
        ui.label(format!("ram_mb:        {}", fmt_opt_f64(snap.system.ram_mb, 1)));
        ui.label(format!(
            "entities:      {}",
            fmt_opt(snap.system.entities_total)
        ));
        ui.label(format!(
            "lag_30s:       {}",
            fmt_opt(snap.system.lag_events_last_30s)
        ));
        ui.label(format!(
            "wdog_emerg_s:  {}",
            fmt_opt_f64(snap.system.watchdog_seconds_in_emergency, 1)
        ));
        ui.separator();
        ui.label(format!(
            "sensors:       {} / {}",
            fmt_opt(snap.system.sensors_stale),
            fmt_opt(snap.system.sensors_total)
        ));
        ui.label(format!(
            "health sev:    {}",
            fmt_opt_str(snap.system.health_severity.as_deref())
        ));
        ui.separator();
        ui.label("recent_alerts:");
        if snap.system.recent_alerts.is_empty() {
            ui.label("  (none)");
        } else {
            for a in &snap.system.recent_alerts {
                ui.label(format!("  - {a}"));
            }
        }
    }
}
