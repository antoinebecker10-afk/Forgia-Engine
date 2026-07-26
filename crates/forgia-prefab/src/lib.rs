//! # forgia-prefab
//!
//! Spawn data-driven d'entités GLTF avec transform + tag marker.
//!
//! Pattern : `PrefabSpawn` event-like helper construit côté caller (loader),
//! consommé par les helpers `spawn_gltf_prefab`. Aucune duplication entre
//! callers (`forgia-village-loader`, futurs `forgia-quest-objects`, etc.).
//!
//! ## Pourquoi un crate à part
//!
//! Tout caller spawnant un `Scene` GLTF refait à la main :
//! `asset_server.load(GltfAssetLabel::Scene(0).from_asset(path))` +
//! `SceneRoot(handle)` + `Transform`. Ici on centralise pour :
//! - 1 sensor `forgia_prefab.json` qui compte les spawns/erreurs
//! - 1 point pour ajouter futures features (LOD, frustum hint, pooling)
//!
//! ## Hot path
//!
//! Non hot — appelé OnEnter(State) ou via events ponctuels. `cargo expand`
//! garde la closure system minimale.

use bevy::asset::{AssetServer, Handle};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::{Scene, SceneRoot};
use std::sync::atomic::{AtomicU32, Ordering};

/// Plugin. Add via `app.add_plugins(ForgiaPrefabPlugin)`.
pub struct ForgiaPrefabPlugin;

impl Plugin for ForgiaPrefabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PrefabStats>();
        app.add_systems(Update, write_prefab_sensor);
    }
}

/// In-memory counters mirrored to `forgia_prefab.json` 1Hz.
#[derive(Resource, Default)]
pub struct PrefabStats {
    pub total_spawned: AtomicU32,
    pub total_errors: AtomicU32,
    last_write_secs: std::sync::Mutex<f32>,
}

/// Parameters for one prefab spawn. Built by caller, consumed by
/// [`spawn_gltf_prefab`].
///
/// `tag` is a generic marker Component to attach for cleanup grouping
/// (e.g. `VillageMarker`, `QuestObjectMarker`). Provided by caller as `impl Bundle`.
pub struct PrefabSpawn {
    pub asset_path: String,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub name: String,
}

impl PrefabSpawn {
    pub fn new(asset_path: impl Into<String>, translation: Vec3) -> Self {
        Self {
            asset_path: asset_path.into(),
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            name: String::new(),
        }
    }

    pub fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn with_yaw_deg(mut self, yaw_deg: f32) -> Self {
        self.rotation = Quat::from_rotation_y(yaw_deg.to_radians());
        self
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = Vec3::splat(scale);
        self
    }

    /// Non-uniform scale — used by ramparts where walls are stretched on
    /// X axis to fill the polygon side without gap.
    pub fn with_scale_vec3(mut self, scale: Vec3) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

/// Spawn a GLTF Scene prefab and tag it with the supplied marker bundle.
///
/// Returns the [`Entity`] so callers can attach extra Components (Colliders,
/// InteractablePoint, etc.). Increments `PrefabStats::total_spawned`.
///
/// The `tag` parameter is a Bundle (typically a marker Component or a tuple)
/// so each caller defines its own cleanup grouping without coupling.
pub fn spawn_gltf_prefab<B: Bundle>(
    commands: &mut Commands,
    asset_server: &AssetServer,
    stats: &PrefabStats,
    spawn: PrefabSpawn,
    tag: B,
) -> Entity {
    let scene_handle: Handle<Scene> =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(spawn.asset_path.clone()));

    let mut e = commands.spawn((
        tag,
        SceneRoot(scene_handle),
        Transform {
            translation: spawn.translation,
            rotation: spawn.rotation,
            scale: spawn.scale,
        },
    ));
    if !spawn.name.is_empty() {
        e.insert(Name::new(spawn.name));
    }
    stats.total_spawned.fetch_add(1, Ordering::Relaxed);
    e.id()
}

/// Record a failure to spawn (asset path missing/invalid). The caller emits
/// its own log — this only updates the counter.
pub fn record_prefab_error(stats: &PrefabStats) {
    stats.total_errors.fetch_add(1, Ordering::Relaxed);
}

/// Reset session counters — call from a cleanup system (OnExit State) to
/// distinguish sessions in `forgia_prefab.json`. `total_spawned`/`total_errors`
/// reset to 0 ; the next sensor write reflects the new session.
pub fn reset_prefab_stats(stats: &PrefabStats) {
    stats.total_spawned.store(0, Ordering::Relaxed);
    stats.total_errors.store(0, Ordering::Relaxed);
}

const SENSOR_INTERVAL_S: f32 = 1.0;

fn write_prefab_sensor(time: Res<Time>, stats: Res<PrefabStats>) {
    let now = time.elapsed_secs();
    let mut last = match stats.last_write_secs.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if now - *last < SENSOR_INTERVAL_S {
        return;
    }
    *last = now;

    let total = stats.total_spawned.load(Ordering::Relaxed);
    let errs = stats.total_errors.load(Ordering::Relaxed);
    let json = format!(
        "{{\"timestamp_secs\":{:.1},\"total_spawned\":{},\"total_errors\":{}}}",
        now, total, errs
    );
    let _ = forgia_core::sensor_io::enqueue("forgia_prefab.json", json);
}
