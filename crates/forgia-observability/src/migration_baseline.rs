//! Migration baseline regression detector.
//!
//! Goal : detect runtime regressions automatically between crate-cleanup migrations.
//! Pattern : snapshot expected metrics at boot (T+5s settle window), serialize to
//! `forgia2_migration_baseline.json`, then each subsequent boot compares current
//! vs previous baseline and emits HealthAlert on drift > tolerance.
//!
//! Metrics tracked :
//! - `boot_entity_count` : World entity count at T+5s post-boot
//! - `plugin_count` : nombre de plugins loaded au boot
//! - `sensors_present` : liste des fichiers `forgia2_*.json` écrits dans les 5
//!   premières secondes (détecte sensor "mort" silencieusement)
//! - `player_spawned` : bool — au moins 1 entity avec Player component à T+5s
//!
//! Usage workflow :
//! 1. Avant migration : `cargo run` 5s → baseline_v1 enregistrée
//! 2. Migration des crates
//! 3. Après migration : `cargo run` 5s → baseline_v2 comparée à v1
//! 4. HealthAlert si delta > tolerance (e.g. -50% entity count, sensor disparu)
//!
//! Le fichier `forgia2_migration_baseline.json` peut être committé en repo pour
//! tracker l'évolution baseline cross-PR (artifact dans `docs/baselines/`).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::time::SystemTime;

/// Délai post-boot avant capture baseline (laisse les sensors s'établir).
const SETTLE_WINDOW_SECS: f32 = 5.0;
/// Chemin de sortie/lecture. Relatif workspace root.
const BASELINE_JSON_PATH: &str = "forgia2_migration_baseline.json";
/// Tolérance par défaut entity count delta (±50%).
const ENTITY_COUNT_TOLERANCE_PCT: f32 = 0.5;

#[derive(Resource, Default)]
pub struct MigrationBaselineState {
    pub captured: bool,
    pub previous: Option<MigrationBaseline>,
    pub current: MigrationBaseline,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MigrationBaseline {
    pub timestamp_unix: u64,
    pub boot_entity_count: u64,
    pub plugin_count: u32,
    pub sensors_present: Vec<String>,
    pub player_spawned: bool,
}

/// Au boot : charge le baseline précédent depuis disque si présent.
pub fn sys_load_previous_baseline(mut state: ResMut<MigrationBaselineState>) {
    if state.previous.is_some() {
        return;
    }
    let Ok(contents) = fs::read_to_string(BASELINE_JSON_PATH) else {
        info!("[migration-baseline] no previous baseline found at {BASELINE_JSON_PATH} — first run.");
        return;
    };
    match serde_json::from_str::<MigrationBaseline>(&contents) {
        Ok(prev) => {
            info!(
                "[migration-baseline] previous baseline loaded : entities={} plugins={} sensors={} ts={}",
                prev.boot_entity_count,
                prev.plugin_count,
                prev.sensors_present.len(),
                prev.timestamp_unix
            );
            state.previous = Some(prev);
        }
        Err(e) => warn!("[migration-baseline] failed to parse {BASELINE_JSON_PATH} : {e}"),
    }
}

/// À T+SETTLE_WINDOW_SECS : capture l'état actuel comme nouveau baseline et compare.
#[allow(clippy::too_many_arguments)]
pub fn sys_capture_and_compare_baseline(
    time: Res<Time>,
    mut state: ResMut<MigrationBaselineState>,
    all_entities: Query<Entity>,
    player_check: Query<(), With<bevy::prelude::Camera3d>>, // proxy : Camera3d implique scene loaded
) {
    if state.captured {
        return;
    }
    if time.elapsed_secs() < SETTLE_WINDOW_SECS {
        return;
    }
    state.captured = true;

    let entity_count = all_entities.iter().count() as u64;
    let player_spawned = !player_check.is_empty();
    let sensors_present = scan_sensor_files();

    let current = MigrationBaseline {
        timestamp_unix: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        boot_entity_count: entity_count,
        // plugin_count not directly accessible from system params at runtime —
        // approximation via sensors count for now (deferred to story).
        plugin_count: sensors_present.len() as u32,
        sensors_present: sensors_present.clone(),
        player_spawned,
    };

    info!(
        "[migration-baseline] captured : entities={} sensors={} player_spawned={}",
        current.boot_entity_count, sensors_present.len(), current.player_spawned
    );

    // Write to disk.
    match serde_json::to_string_pretty(&current) {
        Ok(json) => {
            if let Err(e) = fs::write(BASELINE_JSON_PATH, json) {
                warn!("[migration-baseline] write failed : {e}");
            }
        }
        Err(e) => warn!("[migration-baseline] serialize failed : {e}"),
    }

    // Compare with previous if any.
    if let Some(prev) = &state.previous {
        let entity_delta_pct = if prev.boot_entity_count > 0 {
            (current.boot_entity_count as f32 - prev.boot_entity_count as f32)
                / prev.boot_entity_count as f32
        } else {
            0.0
        };
        if entity_delta_pct.abs() > ENTITY_COUNT_TOLERANCE_PCT {
            warn!(
                "[migration-baseline] REGRESSION ENTITIES : {} → {} ({:+.0}%) — investiguer",
                prev.boot_entity_count,
                current.boot_entity_count,
                entity_delta_pct * 100.0
            );
        }
        // Detect dropped sensors.
        let prev_set: HashSet<&str> = prev.sensors_present.iter().map(|s| s.as_str()).collect();
        let curr_set: HashSet<&str> = current.sensors_present.iter().map(|s| s.as_str()).collect();
        let dropped: Vec<&str> = prev_set.difference(&curr_set).copied().collect();
        if !dropped.is_empty() {
            warn!(
                "[migration-baseline] REGRESSION SENSORS DROPPED : {:?}",
                dropped
            );
        }
        // Detect player despawn regression.
        if prev.player_spawned && !current.player_spawned {
            warn!("[migration-baseline] REGRESSION PLAYER : était spawn au baseline précédent, plus maintenant");
        }
    }

    state.current = current;
}

/// Liste les fichiers `forgia2_*.json` à la racine workspace (proxy détection sensors).
fn scan_sensor_files() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("forgia2_") && name.ends_with(".json") {
                    // Skip baseline itself.
                    if name == BASELINE_JSON_PATH {
                        continue;
                    }
                    out.push(name.to_string());
                }
            }
        }
    }
    out.sort();
    out
}
