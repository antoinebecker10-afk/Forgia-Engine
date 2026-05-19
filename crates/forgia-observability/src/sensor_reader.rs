//! sensor_reader.rs — Lecture 1Hz des sensors JSON depuis disque.
//!
//! sys_read_sensors_1hz : throttlé par Local<f32> accum.
//! Errors silencieuses (1 warn par sensor, pas de panic).

use bevy::prelude::*;
use std::collections::HashSet;

use crate::config::RpgMonitorConfig;
use crate::state::{LastWriteTimestamps, SensorSnapshots};

/// Lit tous les sensors configurés et met à jour SensorSnapshots + LastWriteTimestamps.
/// Throttlé à config.global.tick_interval_secs.
#[allow(clippy::too_many_arguments)]
pub fn sys_read_sensors_1hz(
    time: Res<Time>,
    mut snapshots: ResMut<SensorSnapshots>,
    mut timestamps: ResMut<LastWriteTimestamps>,
    config: Res<RpgMonitorConfig>,
    mut accum: Local<f32>,
    mut warned: Local<HashSet<String>>,
) {
    *accum += time.delta_secs();
    if *accum < config.global.tick_interval_secs {
        return;
    }
    *accum = 0.0;

    for sensor_path in &config.liveness.expected_sensors {
        // Lire le fichier JSON
        let content = match std::fs::read_to_string(sensor_path) {
            Ok(s) => s,
            Err(e) => {
                if !warned.contains(sensor_path) {
                    warn!("[rpg-monitor] Cannot read sensor {sensor_path}: {e}");
                    warned.insert(sensor_path.clone());
                }
                continue;
            }
        };

        // Parser le JSON
        let value: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => {
                // Succès de lecture : retirer de la liste des warned pour future notification
                warned.remove(sensor_path);
                v
            }
            Err(e) => {
                if !warned.contains(sensor_path) {
                    warn!("[rpg-monitor] JSON parse error in {sensor_path}: {e}");
                    warned.insert(sensor_path.clone());
                }
                continue;
            }
        };

        // Mettre à jour le timestamp de modification
        if let Ok(meta) = std::fs::metadata(sensor_path) {
            if let Ok(modified) = meta.modified() {
                timestamps.map.insert(sensor_path.clone(), modified);
            }
        }

        // Dispatcher dans le bon champ de SensorSnapshots
        match sensor_path.as_str() {
            "forgia_terrain_lod.json" => snapshots.terrain_lod = Some(value),
            "forgia_chunks_snapshot.json" => snapshots.chunks_snapshot = Some(value),
            "forgia_combat.json" => snapshots.combat = Some(value),
            "forgia_health.json" => snapshots.health = Some(value),
            "forgia_anim_layer.json" => snapshots.anim_layer = Some(value),
            _ => {
                // Sensor inconnu : ignorer silencieusement (peut être ajouté dans TOML futur)
            }
        }
    }
}
