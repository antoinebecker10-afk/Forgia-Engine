//! `forgia-worldgen` — procedural worldgen engine for Forgia V2 (cities / villages / maps).
//!
//! Story-578, built in phases (`docs/stories/story-578-worldgen-procgen.md`):
//! - **P0** — asset registry (`registry`): per-module geometry (pivot/AABB/role/collider).
//! - **P1** — points + spawn (`points`, `spawn`, `sensor`, `debug_viz`): instanced, grounded,
//!   collidable spawning of catalogued modules, with a sensor and debug viz.
//!
//! ## Decoupling
//! Worldgen depends on **no gameplay crate and not on `forgia-terrain`**. Ground height is
//! provided by an injected [`GroundSampler`] (default = flat). A terrain-backed game inserts
//! its own sampler. This keeps worldgen reusable by Roguelite / RPG / a future editor.
//!
//! ## Demo (observable proof)
//! - **F7** — generate a small **hamlet** from `hamlet.toml`, centered in front of the camera
//!   (P2: data-driven grid layout). Modules grounded via the P0 pivot.
//! - **Shift+F12** — re-read the recipe and regenerate in place (hot-reload — edit the TOML,
//!   see the new hamlet). Variety comes from the data (seed / grid / roles).
//! - **F8** — toggle AABB debug gizmos.
//! - Sensor: `forgia2_worldgen.json`.

pub mod debug_viz;
pub mod points;
pub mod recipe;
pub mod registry;
pub mod sensor;
pub mod spawn;

pub use registry::{AssetMeta, AssetRegistry, AssetRegistryFile, AssetRole, ColliderKind};

use bevy::gltf::Gltf;
use bevy::prelude::*;
use forgia_core::prelude::GameSet;

use debug_viz::{sys_toggle_viz, sys_worldgen_gizmos, WorldgenDebugViz};
use points::{generate_hamlet, GroundSampler};
use recipe::load_recipe;
use spawn::{sys_spawn_drain, SpawnQueue, WorldgenKit, WorldgenModule, WorldgenStats};

/// Kit GLB (individual modules, looked up by mesh name).
const KIT_PATH: &str = "models/environment/platformer/one_file_assets.glb";
/// Registry RON, read at startup from the workspace root.
const REGISTRY_PATH: &str = "assets/registry/asset_meta.ron";
/// Hamlet recipe TOML, re-read on every generation (hot-reload).
const RECIPE_PATH: &str = "assets/genomes/worldgen/hamlet.toml";

/// Eye height above feet — to drop the hamlet to ground level.
const EYE_HEIGHT: f32 = 1.6;
/// How far in front of the camera the hamlet center is placed.
const DEMO_FORWARD_M: f32 = 22.0;

/// Where the last hamlet was placed, so Shift+F12 can regenerate it in place (hot-reload).
#[derive(Resource, Default)]
struct LastHamletPlacement {
    active: bool,
    origin: Vec3,
    axis_x: Vec3,
    axis_z: Vec3,
}

/// Worldgen plugin: loads the registry + kit, runs the demo trigger, spawn, sensor and viz.
pub struct ForgiaWorldgenPlugin;

impl Plugin for ForgiaWorldgenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GroundSampler>()
            .init_resource::<SpawnQueue>()
            .init_resource::<WorldgenStats>()
            .init_resource::<WorldgenDebugViz>()
            .init_resource::<LastHamletPlacement>()
            .add_systems(Startup, (sys_load_registry, sys_load_kit))
            .add_systems(
                Update,
                (
                    (sys_worldgen_input, sys_spawn_drain).chain(),
                    sys_toggle_viz,
                    sys_worldgen_gizmos,
                ),
            )
            .add_systems(
                Update,
                sensor::sys_write_worldgen_sensor.in_set(GameSet::Sensors),
            );
    }
}

/// Load the asset registry from RON (workspace root) into a resource.
fn sys_load_registry(mut commands: Commands) {
    match std::fs::read_to_string(REGISTRY_PATH) {
        Ok(s) => match AssetRegistry::from_ron(&s) {
            Ok(reg) => {
                info!("[worldgen] registry loaded: {} modules", reg.len());
                commands.insert_resource(reg);
            }
            Err(e) => error!("[worldgen] failed to parse {REGISTRY_PATH}: {e}"),
        },
        Err(e) => error!("[worldgen] failed to read {REGISTRY_PATH}: {e}"),
    }
}

/// Kick off async loading of the kit GLB and build a fallback material.
fn sys_load_kit(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let gltf = asset_server.load::<Gltf>(KIT_PATH);
    let fallback_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.72, 0.75),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.insert_resource(WorldgenKit { gltf, fallback_mat });
}

/// Worldgen demo input:
/// - **F7** — generate a fresh hamlet from the recipe, centered in front of the camera.
/// - **Shift+F12** — re-read the recipe and regenerate the hamlet in place (hot-reload).
///
/// Both re-read `hamlet.toml` from disk, so editing the recipe + pressing the key shows the
/// change immediately. Variety comes from the data (seed / grid / roles), not the code.
#[allow(clippy::too_many_arguments)]
fn sys_worldgen_input(
    keys: Res<ButtonInput<KeyCode>>,
    registry: Option<Res<AssetRegistry>>,
    ground: Res<GroundSampler>,
    mut queue: ResMut<SpawnQueue>,
    mut stats: ResMut<WorldgenStats>,
    mut last: ResMut<LastHamletPlacement>,
    q_cam: Query<&GlobalTransform, With<Camera3d>>,
    q_existing: Query<Entity, With<WorldgenModule>>,
    mut commands: Commands,
) {
    let spawn_new = keys.just_pressed(KeyCode::F7);
    let hot_reload = keys.just_pressed(KeyCode::F12)
        && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
    if !spawn_new && !hot_reload {
        return;
    }
    let Some(registry) = registry else {
        warn!("[worldgen] registry not loaded yet");
        return;
    };

    // Placement: F7 takes a fresh spot in front of the camera; Shift+F12 reuses the last spot.
    let (origin, axis_x, axis_z) = if spawn_new {
        let Some(cam) = q_cam.iter().next() else {
            warn!("[worldgen] F7: no Camera3d found");
            return;
        };
        let fwd = cam.forward();
        let fwd_flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero();
        let right = cam.right();
        let right_flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
        let foot = cam.translation() - Vec3::Y * EYE_HEIGHT;
        let origin = foot + fwd_flat * DEMO_FORWARD_M;
        last.active = true;
        last.origin = origin;
        last.axis_x = right_flat;
        last.axis_z = fwd_flat;
        (origin, right_flat, fwd_flat)
    } else if last.active {
        (last.origin, last.axis_x, last.axis_z)
    } else {
        return; // Shift+F12 with nothing placed yet — nothing to reload.
    };

    // Clear the previous hamlet, then re-read the recipe (hot-reload) and generate.
    for e in &q_existing {
        commands.entity(e).despawn();
    }
    queue.pending.clear();

    let recipe = load_recipe(RECIPE_PATH);
    let cloud = generate_hamlet(&recipe, &registry, &ground, origin, axis_x, axis_z);
    stats.last_row = cloud.points.len() as u32;
    info!(
        "[worldgen] {} hamlet: {} modules (seed {}, {}x{} grid)",
        if spawn_new { "F7" } else { "Shift+F12 reload" },
        cloud.points.len(),
        recipe.seed,
        recipe.grid_cols,
        recipe.grid_rows,
    );
    queue.pending.extend(cloud.points);
}
