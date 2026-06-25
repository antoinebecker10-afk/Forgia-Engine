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

pub mod asset_handles;
pub mod checks;
pub mod config;
pub mod exporter;
pub mod forgia2_aggregator;
pub mod health_sensor;
pub mod roguelite_health;
pub mod sensor_reader;
pub mod state;
// Story-467 V5 Session B — perf / entities / memory producers
pub mod entities_sensor;
pub mod memory_sensor;
pub mod perf_sensor;
// Story-469 V5 Session C — lifecycle / watchdog / audio / input / sensor_health
pub mod audio_sensor;
pub mod input_sensor;
pub mod lifecycle_sensor;
pub mod sensor_health_sensor;
pub mod watchdog_sensor;
// Story-520+ regression detector cross-migration. Snapshot baseline T+5s →
// compare cross-runs → HealthAlert on drift (entity count / sensor missing /
// player despawn).
pub mod migration_baseline;
pub mod player_state_sensor;
pub mod lag_events_sensor;
// Story-549 (suite session 2026-05-28) — Rapier blind spot.
pub mod physics_sensor;
// Story-553 — RPG player ↔ biome/water linkage.
pub mod rpg_player_sensor;
// Story-554 — RPG quests state.
pub mod quests_sensor;
// Story-555 — RPG inventory + LOCK-INV-1 audit.
pub mod inventory_sensor;
// Story-556 — NPCs + dialogue active state.
pub mod npcs_sensor;
// Story-528 phase 1 — FPS feel (dash uses, hit feedbacks, aim assist engagements).
pub mod fps_feel_sensor;
// Story-529 Phase 1 — boons foundation (catalogue + active + tag unlocks).
pub mod boons_sensor;
pub mod render_sensor;

pub mod prelude {
    pub use crate::ForgiaObservabilityPlugin;
}

use bevy::diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use forgia_core::prelude::*;

use checks::sys_run_crosschecks;
use config::{sys_reload_config_on_hotkey, RpgMonitorConfig};
use exporter::{sys_sensor_liveness_watchdog, sys_write_rpg_health_json};
use forgia2_aggregator::{sys_write_forgia2_aggregates, Forgia2AggregatorState};
use health_sensor::sys_write_health_sensor;
use sensor_reader::sys_read_sensors_1hz;
use state::{LastWriteTimestamps, RpgHealthState, SensorSnapshots};

/// Plugin Bevy. Ajouter à l'App via app.add_plugins(ForgiaObservabilityPlugin).
pub struct ForgiaObservabilityPlugin;

impl Plugin for ForgiaObservabilityPlugin {
    fn build(&self, app: &mut App) {
        // Story-467 — Diagnostics plugins required by perf_sensor + entities_sensor.
        // Bevy 0.18 ignore double-add silencieusement, idempotent.
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        if !app.is_plugin_added::<EntityCountDiagnosticsPlugin>() {
            app.add_plugins(EntityCountDiagnosticsPlugin::default());
        }

        app.init_resource::<RpgHealthState>()
            .init_resource::<SensorSnapshots>()
            .init_resource::<LastWriteTimestamps>()
            .init_resource::<Forgia2AggregatorState>()
            .init_resource::<lifecycle_sensor::LifecycleCounter>()
            .init_resource::<watchdog_sensor::GameTickCounter>()
            .init_resource::<migration_baseline::MigrationBaselineState>()
            .init_resource::<player_state_sensor::PlayerStateAccum>()
            .init_resource::<lag_events_sensor::LagEventsRing>()
            .init_resource::<physics_sensor::PhysicsSensorState>()
            .init_resource::<rpg_player_sensor::RpgPlayerSensorState>()
            .init_resource::<quests_sensor::QuestsSensorState>()
            .init_resource::<inventory_sensor::InventorySensorState>()
            .init_resource::<npcs_sensor::NpcsSensorState>()
            .init_resource::<fps_feel_sensor::FpsFeelSensorState>()
            .init_resource::<boons_sensor::BoonsSensorState>()
            .insert_resource(RpgMonitorConfig::load_or_default());

        // Migration baseline : Startup load previous, Update capture+compare at T+5s.
        app.add_systems(Startup, migration_baseline::sys_load_previous_baseline);
        app.add_systems(
            Update,
            migration_baseline::sys_capture_and_compare_baseline.in_set(GameSet::Sensors),
        );

        // Story-469 V5 Session C — Observers lifecycle (Bevy 0.18 syntax On<Add, C>).
        app.add_observer(lifecycle_sensor::obs_player_added);
        app.add_observer(lifecycle_sensor::obs_player_removed);
        app.add_observer(lifecycle_sensor::obs_target_cube_added);
        app.add_observer(lifecycle_sensor::obs_target_cube_removed);
        app.add_observer(lifecycle_sensor::obs_nameplate_inserted);
        app.add_observer(lifecycle_sensor::obs_arena_bot_added);
        app.add_observer(lifecycle_sensor::obs_arena_bot_removed);

        // Story-469 — watchdog tick counter en First (avant tous GameSets).
        // Story-526 — lag_events ring buffer aussi en First (capture dt précis).
        app.add_systems(
            First,
            (
                watchdog_sensor::sys_update_tick_counter,
                lag_events_sensor::sys_track_lag_events,
            ),
        );
        // Story-453 : préchargement critical assets handles OnEnter/OnExit Rpg.
        asset_handles::register(app);
        // Sensor health cross-mode : tourne en tout état (pas de run_if mode-gate).
        app.add_systems(Update, sys_write_health_sensor.in_set(GameSet::Sensors));
        // Story-467 V5 Session B — perf / entities / memory producers cross-mode.
        app.add_systems(
            Update,
            (
                perf_sensor::sys_write_perf_sensor,
                entities_sensor::sys_write_entities_sensor,
                memory_sensor::sys_write_memory_sensor,
            )
                .in_set(GameSet::Sensors),
        );
        // Story-469 V5 Session C — lifecycle / watchdog / audio / input / sensor_health.
        // Story-526 — player_state (movement + KCC collision) + lag_events writer.
        app.add_systems(
            Update,
            (
                lifecycle_sensor::sys_write_lifecycle_sensor,
                watchdog_sensor::sys_write_watchdog_sensor,
                audio_sensor::sys_write_audio_sensor,
                input_sensor::sys_track_input_accum,
                sensor_health_sensor::sys_write_sensor_health,
                player_state_sensor::sys_track_player_state,
                player_state_sensor::sys_write_player_state_sensor,
                lag_events_sensor::sys_write_lag_events_sensor,
            )
                .in_set(GameSet::Sensors),
        );
        // Tuple Bevy limit — physics_sensor (story-549) + rpg_player (story-553)
        // + quests (story-554) en groupe séparé.
        app.add_systems(
            Update,
            (
                physics_sensor::sys_write_physics_sensor,
                rpg_player_sensor::sys_write_rpg_player_sensor,
                quests_sensor::sys_write_quests_sensor,
                inventory_sensor::sys_write_inventory_sensor,
                npcs_sensor::sys_write_npcs_sensor,
                fps_feel_sensor::sys_write_fps_feel_sensor,
                boons_sensor::sys_write_boons_sensor,
            )
                .in_set(GameSet::Sensors),
        );
        // Audit 2026-06-17 — capteur de rendu cross-mode (forgia2_render.json) :
        // diagnostic direct écran brun / monde invisible (mesh3d_total vs visible
        // + état par caméra). Comble le trou d'observabilité du rendu.
        app.add_systems(
            Update,
            render_sensor::sys_write_render_sensor.in_set(GameSet::Sensors),
        );
        // story-621 (Phase 0.6 A) — checks santé Roguelite (RGL-1 écran vide /
        // RGL-2 vague figée) qui peuplent RpgHealthState. Gaté Roguelite : les
        // checks RPG (CHK-*) sont gatés Rpg → aucun conflit d'écriture.
        app.add_systems(
            Update,
            roguelite_health::sys_roguelite_health
                .in_set(GameSet::Sensors)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // Story-465 — forgia2 aggregator Tier 1 : combat + arena.
        // V7 M1 (story-470) : gate étendu Fps OU Roguelite — V7 réutilise le firing
        // path FPS (forgia-fps emit hitscan/ammo/screen_flash/etc. en mode Roguelite
        // aussi). Visibilité combat obligatoire dans le 3e jeu.
        app.add_systems(
            Update,
            sys_write_forgia2_aggregates
                .in_set(GameSet::Sensors)
                .run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite))),
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
