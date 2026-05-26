//! Sensor `forgia_hud_ammo.json` — état HUD ammo per weapon.
//!
//! Écrit `sensor_period_secs` (genome-driven, default 1Hz). Distinct du
//! `forgia_hitscan.json::ammo_state` (qui reste pour combat diagnostic) :
//! ce sensor est focused HUD (active weapon, low_active flag, last render perf).

use bevy::prelude::*;
use forgia_combat::weapons::EquippedWeapons;
use std::fs;

use super::tuning::AmmoHudTuning;

#[derive(Resource, Default)]
pub struct AmmoHudSensor {
    pub last_write_secs: f32,
    /// Durée du dernier draw (ms) — sentinel performance.
    pub last_render_ms: f32,
    /// Nombre d'AmmoChanged events reçus session (cumulatif).
    pub events_total: u32,
}

/// Écrit `forgia_hud_ammo.json` à fréquence `tuning.sensor_period_secs`.
pub fn write_ammo_hud_sensor(
    time: Res<Time>,
    tuning: Res<AmmoHudTuning>,
    equipped: Res<EquippedWeapons>,
    mut sensor: ResMut<AmmoHudSensor>,
) {
    let now = time.elapsed_secs();
    if now - sensor.last_write_secs < tuning.sensor_period_secs.max(0.1) {
        return;
    }
    sensor.last_write_secs = now;

    let mut slots_json = String::with_capacity(equipped.slots.len() * 120);
    let mut first = true;
    let active = equipped.current;
    let mut low_active = false;
    for (weapon, slot) in equipped.iter_slots() {
        if !first {
            slots_json.push(',');
        }
        first = false;
        if weapon == active {
            low_active = slot.is_low();
        }
        slots_json.push_str(&format!(
            r#"{{"weapon":"{:?}","is_current":{},"current_mag":{},"reserve":{},"mag_size":{},"is_low":{},"reloading":{},"reload_progress":{:.3}}}"#,
            weapon,
            weapon == active,
            slot.current_mag,
            slot.reserve,
            slot.config.mag_size,
            slot.is_low(),
            slot.reload_state.is_reloading(),
            slot.reload_state.progress(slot.config.reload_time_secs),
        ));
    }

    let json = format!(
        r#"{{"timestamp_secs":{:.2},"active_weapon":"{:?}","low_ammo_active":{},"events_total":{},"last_render_ms":{:.3},"slots":[{}]}}"#,
        now, active, low_active, sensor.events_total, sensor.last_render_ms, slots_json
    );

    let _ = fs::write("forgia_hud_ammo.json", json);
}
