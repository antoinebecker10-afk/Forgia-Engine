//! assets_load_sensor.rs — Producteur `forgia2_assets.json` (1Hz, cross-mode).
//!
//! Vérifie l'état de chargement de tous les `SceneRoot` + `Mesh3d` du monde.
//! Détecte les Handle::Failed que `asset_server.load()` ne signale PAS au caller
//! (retourne toujours un Handle même si l'asset n'existe pas — `total_spawned`
//! counter trompeur).
//!
//! Origine 2026-05-21 — Règle Fondatrice "Observable" cross-mode. Le compteur
//! `forgia_prefab.json::total_spawned` ne prouve pas que les meshes rendent.
//! Sans ce sensor, "je ne vois rien" runtime = aveugle.
//!
//! Cross-mode : tourne en Update sans gate GameMode, applicable Fps/Rpg/Roguelite.

use bevy::asset::{LoadState, UntypedAssetLoadFailedEvent};
use bevy::prelude::*;
use bevy::scene::SceneRoot;

#[derive(Resource, Default)]
pub struct AssetLoadSensorState {
    pub last_write_secs: f32,
    /// Échecs captés par EVENT (story-595 post-mortem KTX2) : cumulés session,
    /// dédupliqués, cap 20. Couvre les handles JAMAIS spawnés en SceneRoot —
    /// l'angle mort qui a laissé passer 4 armes invisibles avec scene_failed=0
    /// (le viewmodel attend Loaded avant de spawner → échec invisible au scan world).
    pub event_failed_paths: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn sys_write_assets_sensor(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    q_scene: Query<&SceneRoot>,
    q_mesh: Query<&Mesh3d>,
    mut failed_events: MessageReader<UntypedAssetLoadFailedEvent>,
    mut state: ResMut<AssetLoadSensorState>,
) {
    // Drainer AVANT le throttle 1Hz : les messages non lus chaque frame sont perdus.
    for ev in failed_events.read() {
        let p = ev.path.to_string().replace('\\', "/");
        if !state.event_failed_paths.contains(&p) && state.event_failed_paths.len() < 20 {
            warn!("[forgia-observability] asset load FAILED (event): {p}");
            state.event_failed_paths.push(p);
        }
    }

    let now = time.elapsed_secs();
    if now - state.last_write_secs < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let mut scene_loaded = 0u32;
    let mut scene_loading = 0u32;
    let mut scene_failed = 0u32;
    let mut scene_not_loaded = 0u32;
    let mut scene_failed_paths: Vec<String> = Vec::new();

    for scene_root in &q_scene {
        match asset_server.get_load_state(&scene_root.0) {
            Some(LoadState::Loaded) => scene_loaded += 1,
            Some(LoadState::Loading) => scene_loading += 1,
            Some(LoadState::Failed(_)) => {
                scene_failed += 1;
                if scene_failed_paths.len() < 10 {
                    if let Some(path) = asset_server.get_path(scene_root.0.id()) {
                        let p = path.to_string().replace('\\', "/");
                        if !scene_failed_paths.contains(&p) {
                            scene_failed_paths.push(p);
                        }
                    }
                }
            }
            Some(LoadState::NotLoaded) | None => scene_not_loaded += 1,
        }
    }

    let mut mesh_loaded = 0u32;
    let mut mesh_loading = 0u32;
    let mut mesh_failed = 0u32;

    for mesh in &q_mesh {
        match asset_server.get_load_state(&mesh.0) {
            Some(LoadState::Loaded) => mesh_loaded += 1,
            Some(LoadState::Loading) => mesh_loading += 1,
            Some(LoadState::Failed(_)) => mesh_failed += 1,
            _ => {}
        }
    }

    let event_failed = state.event_failed_paths.len() as u32;
    let total_failed = scene_failed + mesh_failed + event_failed;
    let (severity, next_step) = if total_failed > 0 {
        (
            "warn",
            "Asset load failures — check failed_paths + event_failed_paths (echecs hors-world : handle jamais spawne) + verify file exists relative to assets/",
        )
    } else {
        ("ok", "")
    };

    let failed_paths_json: String = scene_failed_paths
        .iter()
        .map(|p| format!("\"{p}\""))
        .collect::<Vec<_>>()
        .join(",");
    let event_failed_json: String = state
        .event_failed_paths
        .iter()
        .map(|p| format!("\"{p}\""))
        .collect::<Vec<_>>()
        .join(",");

    let json = format!(
        r#"{{"id":"assets","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"scene_loaded":{scene_loaded},"scene_loading":{scene_loading},"scene_failed":{scene_failed},"scene_not_loaded":{scene_not_loaded},"mesh_loaded":{mesh_loaded},"mesh_loading":{mesh_loading},"mesh_failed":{mesh_failed},"event_failed":{event_failed},"failed_paths":[{failed_paths_json}],"event_failed_paths":[{event_failed_json}]}}"#,
        now,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_assets.json", json) {
        warn!("[forgia-observability] assets_load_sensor write failed: {e}");
    }
}
