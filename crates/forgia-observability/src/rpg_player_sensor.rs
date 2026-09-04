//! rpg_player_sensor.rs — Producteur `forgia2_rpg_player.json` (1Hz, RPG mode).
//!
//! Comble blind spot RPG : aujourd'hui forgia2_player_state.json donne pos/vel
//! mais aucun sensor relie player ↔ biome/water. Story-553 (2026-05-28).
//!
//! ## Schema JSON
//!
//! ```json
//! {
//!   "id": "rpg_player",
//!   "severity": "ok|warn",
//!   "next_step": "...",
//!   "timestamp_secs": 12.5,
//!   "player_present": true,
//!   "player_pos": [10.5, 2.0, -5.0],
//!   "biome_current": "Forest",
//!   "sea_level": 4.0,
//!   "depth_below_surface": 2.0,
//!   "is_swimming": true,
//!   "head_above_water": false
//! }
//! ```
//!
//! ## Computations
//!
//! - `biome_current` = `BiomeMap::biome_at(player.x, player.z)` — Voronoi lookup
//! - `is_swimming` = `player.y < sea_level` (V1 mécanique, V2 pas encore gated player)
//! - `depth_below_surface` = `max(0, sea_level - player.y)`
//! - `head_above_water` = `player.y + EYE_HEIGHT > sea_level` (anti-drown UX)
//!
//! Pas de `breath_remaining` — aucun mécanisme V2 ne le mesure (V1 had it).
//! NE PAS fabriquer.

use bevy::prelude::*;
use bevy_water::WaterSettings;
use forgia_player::Player;
use forgia_terrain::BiomeMap;

/// Hauteur des yeux au-dessus du centre body — calibré V1 (1.6m capsule center → 0.5 eye offset).
const EYE_HEIGHT_OFFSET: f32 = 0.5;

#[derive(Resource, Default)]
pub struct RpgPlayerSensorState {
    pub last_write_secs: f32,
}

const SENSOR_PATH: &str = "forgia2_rpg_player.json";

#[allow(clippy::too_many_arguments)]
pub fn sys_write_rpg_player_sensor(
    time: Res<Time>,
    biome_map: Option<Res<BiomeMap>>,
    water: Option<Res<WaterSettings>>,
    q_player: Query<&Transform, With<Player>>,
    mut state: ResMut<RpgPlayerSensorState>,
) {
    let now = time.elapsed_secs();
    if now - state.last_write_secs < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let player_present = q_player.iter().next().is_some();
    let pos = q_player
        .iter()
        .next()
        .map(|tf| tf.translation)
        .unwrap_or(Vec3::ZERO);

    let sea_level = water.as_ref().map(|w| w.height).unwrap_or(f32::NAN);
    let biome = biome_map
        .as_ref()
        .map(|m| m.biome_at(pos.x, pos.z))
        .map(|b| format!("{b:?}"))
        .unwrap_or_else(|| "Unknown".to_string());

    let depth = if sea_level.is_finite() {
        (sea_level - pos.y).max(0.0)
    } else {
        0.0
    };
    let is_swimming = sea_level.is_finite() && pos.y < sea_level && player_present;
    let head_above_water = if sea_level.is_finite() {
        pos.y + EYE_HEIGHT_OFFSET > sea_level
    } else {
        true
    };

    let (severity, next_step) = if !player_present {
        ("ok", "no Player entity — Menu/Pause/AppNotInGame")
    } else if biome_map.is_none() {
        ("warn", "BiomeMap absent — RPG terrain pas généré")
    } else if depth > 50.0 {
        (
            "warn",
            "Player drowning bottom of map (>50m deep) — terrain hole ou fall-through",
        )
    } else if is_swimming && !head_above_water {
        (
            "warn",
            "Player fully underwater — drown imminent (V2 sans mécanique breath)",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"rpg_player","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"player_present":{player_present},"player_pos":[{:.3},{:.3},{:.3}],"biome_current":"{biome}","sea_level":{},"depth_below_surface":{:.3},"is_swimming":{is_swimming},"head_above_water":{head_above_water}}}"#,
        now,
        pos.x,
        pos.y,
        pos.z,
        fmt_f32(sea_level),
        depth,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[forgia-observability] rpg_player_sensor write failed: {e}");
    }
}

fn fmt_f32(v: f32) -> String {
    if v.is_nan() {
        "null".to_string()
    } else {
        format!("{v:.3}")
    }
}
