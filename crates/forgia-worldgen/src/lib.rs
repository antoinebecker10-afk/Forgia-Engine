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

pub mod cache;
pub mod debug_viz;
pub mod hex;
pub mod house_mesh;
pub mod layout;
pub mod points;
pub mod recipe;
pub mod registry;
pub mod resolve;
pub mod road_mesh;
pub mod seed;
pub mod sensor;
pub mod spawn;
pub mod streaming;

pub use registry::{AssetMeta, AssetRegistry, AssetRegistryFile, AssetRole, ColliderKind};

use bevy::gltf::Gltf;
use bevy::prelude::*;
use forgia_core::prelude::{GameMode, GameSet};

use debug_viz::{sys_layout_gizmos, sys_toggle_viz, sys_worldgen_gizmos, WorldgenDebugViz};
use layout::{generate_parcels, generate_roads, Parcel, RoadNetwork, TensorField};
use points::{generate_hamlet, GroundSampler, Point};
use recipe::{load_city_recipe, load_recipe};
use resolve::resolve_building;
use seed::derive;
use spawn::{sys_spawn_drain, SpawnQueue, WorldgenKit, WorldgenModule, WorldgenStats};

/// Kit GLB (individual modules, looked up by mesh name).
const KIT_PATH: &str = "models/environment/platformer/one_file_assets.glb";
/// Registry RON, read at startup from the workspace root.
const REGISTRY_PATH: &str = "assets/registry/asset_meta.ron";
/// Hamlet recipe TOML, re-read on every generation (hot-reload).
const RECIPE_PATH: &str = "assets/genomes/worldgen/hamlet.toml";
/// City recipe TOML (roads + parcels), re-read on every generation (hot-reload).
const CITY_RECIPE_PATH: &str = "assets/genomes/worldgen/city.toml";
/// Where F9 bakes the resolved city, and F11 reloads it from (workspace root).
const BAKE_CITY_PATH: &str = "worldgen_baked_city.ron";
/// Distance beyond which spawned modules are culled (distant LOD, P6).
const LOD_FAR_M: f32 = 200.0;
/// How often the LOD pass runs (seconds).
const LOD_INTERVAL_SEC: f32 = 0.25;

/// Eye height above feet — to drop the layout to ground level.
const EYE_HEIGHT: f32 = 1.6;
/// How far in front of the camera the generated layout center is placed.
const DEMO_FORWARD_M: f32 = 22.0;
/// Max streamline steps when tracing roads.
const ROAD_MAX_STEPS: u32 = 256;
/// Max recursion depth when subdividing blocks into parcels.
const PARCEL_MAX_DEPTH: u32 = 8;

/// What the demo last generated (so Shift+F12 reloads the right thing).
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum GenMode {
    #[default]
    Hamlet,
    City,
}

/// Where + what the last generation was, so Shift+F12 regenerates it in place (hot-reload).
#[derive(Resource, Default)]
struct LastGen {
    active: bool,
    mode: GenMode,
    origin: Vec3,
    axis_x: Vec3,
    axis_z: Vec3,
}

/// The generated city layout (roads + parcels) + its world transform, for debug viz.
#[derive(Resource, Default)]
pub struct CityLayout {
    pub present: bool,
    pub roads: RoadNetwork,
    pub parcels: Vec<Parcel>,
    pub origin: Vec3,
    pub axis_x: Vec3,
    pub axis_z: Vec3,
}

/// Worldgen plugin: loads the registry + kit, runs the demo trigger, spawn, sensor and viz.
pub struct ForgiaWorldgenPlugin;

impl Plugin for ForgiaWorldgenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GroundSampler>()
            .init_resource::<SpawnQueue>()
            .init_resource::<WorldgenStats>()
            .init_resource::<CityLayout>()
            .init_resource::<streaming::CityStreaming>()
            .add_systems(Startup, (sys_load_registry, sys_load_kit))
            // Always available, ALL modes — the authoring runtime: drain the spawn queue (so
            // authored content reaches the world: Roguelite demo AND the RPG village), distant
            // LOD, and the sensor. NOT mode-gated — cheap no-ops when the queue is empty.
            .add_systems(
                Update,
                (sys_spawn_drain, sys_worldgen_lod).in_set(GameSet::Effects),
            )
            .add_systems(
                Update,
                sensor::sys_write_worldgen_sensor.in_set(GameSet::Sensors),
            );

        // Dev tools — the key-triggered demos / visualization (F7/F9/F11/F8/F5 + Shift+F12).
        // Behind the `dev-tools` feature (default on); a ship build that disables the feature
        // skips registration, so players can't trigger them. Gated to Roguelite.
        if cfg!(feature = "dev-tools") {
            app.init_resource::<WorldgenDebugViz>()
                .init_resource::<LastGen>()
                .add_systems(
                    Update,
                    (
                        sys_worldgen_input,
                        sys_toggle_viz,
                        sys_worldgen_gizmos,
                        sys_layout_gizmos,
                        (streaming::sys_toggle_streaming, streaming::sys_stream_city).chain(),
                    )
                        .run_if(in_state(GameMode::Roguelite)),
                );
        }
    }
}

/// Load the asset registry from RON (workspace root) into a resource.
fn sys_load_registry(mut commands: Commands) {
    // def_io : sur wasm, std::fs échoue — le registre vient du pack embarqué
    // (sinon ERROR rouge à chaque session web, story-695).
    match forgia_core::def_io::read_def_str(REGISTRY_PATH) {
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
    let road_mat = materials.add(road_mesh::road_material());
    let house_mat = materials.add(house_mesh::house_material());
    commands.insert_resource(WorldgenKit {
        gltf,
        fallback_mat,
        road_mat,
        house_mat,
    });
}

/// Worldgen demo input:
/// - **F7** — generate a **hamlet** (grid) from `hamlet.toml`, in front of the camera.
/// - **F9** — generate a **city** (roads + parcels + buildings + landmarks) from `city.toml`,
///   in front of the camera, and **bake** the resolved points to disk.
/// - **F11** — reload the **baked** city instantly (no regeneration — P6 bake/cache).
/// - **Shift+F12** — re-read the recipe and regenerate the last thing in place (hot-reload).
///
/// Generation paths re-read the TOML from disk, so editing the recipe + pressing the key applies
/// the change immediately. Variety comes from the data, not the code.
#[allow(clippy::too_many_arguments)]
fn sys_worldgen_input(
    keys: Res<ButtonInput<KeyCode>>,
    registry: Option<Res<AssetRegistry>>,
    ground: Res<GroundSampler>,
    mut queue: ResMut<SpawnQueue>,
    mut stats: ResMut<WorldgenStats>,
    mut last: ResMut<LastGen>,
    mut city: ResMut<CityLayout>,
    kit: Option<Res<WorldgenKit>>,
    mut meshes: ResMut<Assets<Mesh>>,
    q_cam: Query<&GlobalTransform, With<Camera3d>>,
    q_existing: Query<Entity, With<WorldgenModule>>,
    mut commands: Commands,
) {
    let f7 = keys.just_pressed(KeyCode::F7);
    let f9 = keys.just_pressed(KeyCode::F9);
    let f11 = keys.just_pressed(KeyCode::F11);
    let reload = keys.just_pressed(KeyCode::F12)
        && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
    if !f7 && !f9 && !f11 && !reload {
        return;
    }

    // F11 — reload the baked city instantly (no registry / generation needed).
    if f11 {
        for e in &q_existing {
            commands.entity(e).despawn();
        }
        queue.pending.clear();
        city.present = false;
        match cache::load(BAKE_CITY_PATH) {
            Some(points) => {
                stats.last_row = points.len() as u32;
                info!(
                    "[worldgen] F11: loaded {} baked modules (no regen)",
                    points.len()
                );
                queue.pending.extend(points);
            }
            None => warn!("[worldgen] F11: no baked city — press F9 first"),
        }
        return;
    }

    let Some(registry) = registry else {
        warn!("[worldgen] registry not loaded yet");
        return;
    };

    // Decide mode + placement. F7/F9 take a fresh spot in front of the camera; Shift+F12 reuses.
    let (mode, origin, axis_x, axis_z) = if f7 || f9 {
        let Some(cam) = q_cam.iter().next() else {
            warn!("[worldgen] no Camera3d found");
            return;
        };
        let (origin, right, fwd) = camera_placement(cam);
        let mode = if f9 { GenMode::City } else { GenMode::Hamlet };
        *last = LastGen {
            active: true,
            mode,
            origin,
            axis_x: right,
            axis_z: fwd,
        };
        (mode, origin, right, fwd)
    } else if last.active {
        (last.mode, last.origin, last.axis_x, last.axis_z)
    } else {
        return; // Shift+F12 with nothing placed yet.
    };

    // Clear the previous output.
    for e in &q_existing {
        commands.entity(e).despawn();
    }
    queue.pending.clear();
    city.present = false;

    match mode {
        GenMode::Hamlet => {
            let recipe = load_recipe(RECIPE_PATH);
            let cloud = generate_hamlet(&recipe, &registry, &ground, origin, axis_x, axis_z);
            stats.last_row = cloud.points.len() as u32;
            info!(
                "[worldgen] hamlet: {} modules (seed {}, {}x{})",
                cloud.points.len(),
                recipe.seed,
                recipe.grid_cols,
                recipe.grid_rows
            );
            queue.pending.extend(cloud.points);
        }
        GenMode::City => {
            let recipe = load_city_recipe(CITY_RECIPE_PATH);
            let (points, roads, parcels) =
                build_city(&recipe, &registry, &ground, origin, axis_x, axis_z);
            stats.last_row = points.len() as u32;
            info!(
                "[worldgen] city: {} roads, {} parcels, {} buildings (seed {})",
                roads.segments.len(),
                parcels.len(),
                points.len(),
                recipe.seed
            );
            // P6 bake — write the resolved city so F11 can reload it without regenerating.
            match cache::bake(&points, BAKE_CITY_PATH) {
                Ok(n) => info!("[worldgen] baked {n} modules → {BAKE_CITY_PATH}"),
                Err(e) => warn!("[worldgen] bake failed: {e}"),
            }
            queue.pending.extend(points);
            // P6+ road mesh — a real flat ribbon mesh for the roads (not just gizmos).
            if let Some(kit) = &kit {
                let mesh = road_mesh::build_road_mesh(
                    &roads,
                    origin,
                    axis_x,
                    axis_z,
                    &ground,
                    road_mesh::ROAD_WIDTH_M,
                );
                commands.spawn((
                    WorldgenModule {
                        module_id: "worldgen:roads".to_string(),
                        local_center: Vec3::ZERO,
                        local_half: Vec3::ZERO,
                    },
                    Name::new("worldgen:roads"),
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(kit.road_mat.clone()),
                    Transform::from_translation(origin),
                    Visibility::default(),
                ));
            }
            *city = CityLayout {
                present: true,
                roads,
                parcels,
                origin,
                axis_x,
                axis_z,
            };
        }
    }
}

/// Ground-level placement in front of a camera: `(origin, right_flat, forward_flat)`.
fn camera_placement(cam: &GlobalTransform) -> (Vec3, Vec3, Vec3) {
    let fwd = cam.forward();
    let fwd_flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero();
    let right = cam.right();
    let right_flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
    let foot = cam.translation() - Vec3::Y * EYE_HEIGHT;
    (foot + fwd_flat * DEMO_FORWARD_M, right_flat, fwd_flat)
}

/// Build a city from a recipe: trace roads, subdivide parcels, inject hand-crafted landmarks
/// (reserving nearby parcels), and resolve each remaining parcel into a stacked building via the
/// grammar. Returns the spawn points + the layout. Deterministic. **Authoring API** — call this
/// to generate a city anywhere (push the points to `SpawnQueue` to spawn them).
pub fn build_city(
    recipe: &recipe::CityRecipe,
    registry: &AssetRegistry,
    ground: &GroundSampler,
    origin: Vec3,
    axis_x: Vec3,
    axis_z: Vec3,
) -> (Vec<Point>, RoadNetwork, Vec<Parcel>) {
    let half = recipe.size * 0.5;
    let (min, max) = (Vec2::splat(-half), Vec2::splat(half));
    let field = TensorField {
        base_angle: recipe.base_angle_deg.to_radians(),
        center: Vec2::ZERO,
        radial_strength: recipe.radial_strength,
        radial_radius: recipe.radial_radius,
    };
    let roads = generate_roads(
        &field,
        min,
        max,
        recipe.road_spacing,
        recipe.road_step,
        ROAD_MAX_STEPS,
        recipe.road_sep,
    );
    let parcels = generate_parcels(
        min,
        max,
        recipe.road_spacing,
        recipe.parcel_inset,
        recipe.parcel_target_area,
        PARCEL_MAX_DEPTH,
    );

    let to_world = |p: Vec2| origin + axis_x * p.x + axis_z * p.y;
    let mut points = Vec::new();

    // Hand-crafted landmarks (placed first, exact positions). Reserve their footprint.
    let mut reserved: Vec<Vec2> = Vec::new();
    for lm in &recipe.landmarks {
        if lm.module_id.is_empty() {
            continue;
        }
        if registry.get(&lm.module_id).is_none() {
            warn!(
                "[worldgen] landmark '{}' not in registry — skipped",
                lm.module_id
            );
            continue;
        }
        let local = Vec2::new(lm.x, lm.z);
        reserved.push(local);
        let world = to_world(local);
        let gy = world.y + ground.height(world.x, world.z);
        points.push(Point {
            module_id: lm.module_id.clone(),
            ground_pos: Vec3::new(world.x, gy, world.z),
            yaw: 0.0,
            scale: if lm.scale > 0.0 { lm.scale } else { 1.0 },
        });
    }

    // Procedural buildings on the remaining parcels (grammar-resolved, seeded per parcel).
    let reserve_r = recipe.landmark_reserve_radius.max(0.0);
    for (i, parcel) in parcels.iter().enumerate() {
        if reserved
            .iter()
            .any(|r| parcel.center.distance(*r) < reserve_r)
        {
            continue; // reserved for a landmark
        }
        let world = to_world(parcel.center);
        let base_y = world.y + ground.height(world.x, world.z);
        let seed = derive(recipe.seed, i as u64);
        let building = resolve_building(
            &recipe.building,
            registry,
            world.x,
            world.z,
            base_y,
            seed,
            recipe.building_scale,
        );
        points.extend(building);
    }

    (points, roads, parcels)
}

/// Distant LOD (P6): cull spawned modules beyond `LOD_FAR_M` of the camera (throttled), so a
/// large streamed/baked city keeps the frame budget. Toggles `Visibility` with hysteresis-free
/// change detection (only writes on transition).
fn sys_worldgen_lod(
    time: Res<Time>,
    mut accum: Local<f32>,
    q_cam: Query<&GlobalTransform, With<Camera3d>>,
    mut q_modules: Query<(&GlobalTransform, &mut Visibility), With<WorldgenModule>>,
) {
    *accum += time.delta_secs();
    if *accum < LOD_INTERVAL_SEC {
        return;
    }
    *accum = 0.0;
    let Some(cam) = q_cam.iter().next() else {
        return;
    };
    let cam_pos = cam.translation();
    let far_sq = LOD_FAR_M * LOD_FAR_M;
    for (gt, mut vis) in &mut q_modules {
        let want = if gt.translation().distance_squared(cam_pos) > far_sq {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *vis != want {
            *vis = want;
        }
    }
}
