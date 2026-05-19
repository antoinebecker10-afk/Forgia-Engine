//! # forgia-observability
//!
//! RPG Health Monitor — 6 checks cross-sectoriels (story-452).
//!
//! ## Sensors produits
//!
//! - `forgia_rpg_health.json` — état global + 6 checks, refresh 1Hz
//!
//! ## Checks
//!
//! - CHK-1 : LOD2 desync (lod2_count vs lod2_tile_count)
//! - CHK-2 : Biome luminance Rec709 hors plage
//! - CHK-3 : LOD asymmetry (lod0_y vs lod2_y sur sample_points, story-453)
//! - CHK-4 : Critical assets chargés (handles préchargés OnEnter Rpg, story-453)
//! - CHK-5 : Sensor liveness (stale JSON)
//! - CHK-6 : Health consistency (HP player cohérent)
//!
//! ## Hotkey
//!
//! Shift+F12 : recharge `config/genomes/rpg_monitor.toml` à chaud.

pub mod config;
pub mod state;
pub mod sensor_reader;
pub mod checks;
pub mod exporter;
pub mod asset_handles;
pub mod health_sensor;

pub mod prelude {
    pub use crate::ForgiaObservabilityPlugin;
}

use bevy::prelude::*;
use forgia_core::prelude::*;

use config::{RpgMonitorConfig, sys_reload_config_on_hotkey};
use sensor_reader::sys_read_sensors_1hz;
use checks::sys_run_crosschecks;
use exporter::{sys_write_rpg_health_json, sys_sensor_liveness_watchdog};
use health_sensor::sys_write_health_sensor;
use state::{RpgHealthState, SensorSnapshots, LastWriteTimestamps};

/// Plugin Bevy. Ajouter à l'App via app.add_plugins(ForgiaObservabilityPlugin).
pub struct ForgiaObservabilityPlugin;

impl Plugin for ForgiaObservabilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RpgHealthState>()
            .init_resource::<SensorSnapshots>()
            .init_resource::<LastWriteTimestamps>()
            .insert_resource(RpgMonitorConfig::load_or_default());
        // Story-453 : préchargement critical assets handles OnEnter/OnExit Rpg.
        asset_handles::register(app);
        // Sensor health cross-mode : tourne en tout état (pas de run_if mode-gate).
        app.add_systems(
            Update,
            sys_write_health_sensor.in_set(GameSet::Sensors),
        );
        app.add_systems(
                Update,
                (
                    sys_reload_config_on_hotkey,
                    sys_read_sensors_1hz,
                    sys_run_crosschecks,
                    sys_write_rpg_health_json,
                    sys_sensor_liveness_watchdog,
                )
                    .chain()
                    .in_set(GameSet::Sensors)
                    .run_if(in_state(GameMode::Rpg)),
            );
    }
}
