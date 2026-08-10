//! Streaming — an endless procedural city loaded/unloaded around the player (story-578 P4).
//!
//! **Decoupled**: worldgen owns its own chunk grid (no `forgia-terrain` dependency); the player
//! position comes from the active `Camera3d`, the ground from the injected `GroundSampler`. This
//! mirrors terrain's chunk concept (matching `chunk_size`) without coupling to it — a future
//! adapter can sync the two grids.
//!
//! **Reproducible**: each chunk is generated in isolation from `chunk_seed(world_seed, cx, cy)`
//! (see [`crate::seed`]), so the same `world_seed` always yields the same city, and chunks load
//! independently as the player moves. Toggle with **F5** (dev tool).

use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::points::{generate_chunk, ChunkConfig, GroundSampler};
use crate::recipe::{load_stream_recipe, StreamRecipe};
use crate::registry::AssetRegistry;
use crate::spawn::{spawn_module, WorldgenKit};

const STREAM_RECIPE_PATH: &str = "assets/genomes/worldgen/stream.toml";
/// Max chunks generated per frame (anti-spike — spreads loading over frames).
const CHUNK_BUDGET_PER_FRAME: usize = 1;

/// Streaming state: config + which chunks are currently loaded (chunk coord → its entities).
#[derive(Resource, Default)]
pub struct CityStreaming {
    pub enabled: bool,
    recipe: StreamRecipe,
    active: HashMap<IVec2, Vec<Entity>>,
}

impl CityStreaming {
    /// Number of currently-loaded chunks (for the sensor).
    pub fn active_chunks(&self) -> usize {
        self.active.len()
    }
}

/// **F5** toggles streaming (dev tool — F10 is taken by the collider debug). Turning it on
/// re-reads `stream.toml` (hot-reload); turning it off unloads every chunk.
pub fn sys_toggle_streaming(
    keys: Res<ButtonInput<KeyCode>>,
    mut streaming: ResMut<CityStreaming>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::F5) {
        return;
    }
    streaming.enabled = !streaming.enabled;
    if streaming.enabled {
        streaming.recipe = load_stream_recipe(STREAM_RECIPE_PATH);
        info!(
            "[worldgen] streaming ON (seed {}, chunk {} m, radius {})",
            streaming.recipe.world_seed, streaming.recipe.chunk_size, streaming.recipe.view_radius
        );
    } else {
        for (_, entities) in streaming.active.drain() {
            for e in entities {
                commands.entity(e).despawn();
            }
        }
        info!("[worldgen] streaming OFF");
    }
}

/// Load chunks within `view_radius` of the player and unload the rest. Generation is budgeted
/// (nearest chunks first) and deterministic per chunk coordinate.
#[allow(clippy::too_many_arguments)]
pub fn sys_stream_city(
    mut streaming: ResMut<CityStreaming>,
    registry: Option<Res<AssetRegistry>>,
    ground: Res<GroundSampler>,
    kit: Option<Res<WorldgenKit>>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
    q_cam: Query<&GlobalTransform, With<Camera3d>>,
    mut commands: Commands,
) {
    if !streaming.enabled {
        return;
    }
    let (Some(registry), Some(kit)) = (registry, kit) else {
        return;
    };
    let Some(gltf) = gltfs.get(&kit.gltf) else {
        return; // kit GLB not loaded yet
    };
    let Some(cam) = q_cam.iter().next() else {
        return;
    };

    // Snapshot config (avoids interleaving borrows of `streaming` with its mutations below).
    let cs = streaming.recipe.chunk_size.max(2.0);
    let r = streaming.recipe.view_radius as i32;
    let world_seed = streaming.recipe.world_seed;
    let cfg = ChunkConfig {
        chunk_size: cs,
        buildings_per_chunk: streaming.recipe.buildings_per_chunk,
        building_role: streaming.recipe.building_role,
        scale: streaming.recipe.scale,
    };

    let pos = cam.translation();
    let ccx = (pos.x / cs).floor() as i32;
    let ccy = (pos.z / cs).floor() as i32;

    // Chunks we want loaded (Chebyshev radius square).
    let mut desired = HashSet::new();
    for dy in -r..=r {
        for dx in -r..=r {
            desired.insert(IVec2::new(ccx + dx, ccy + dy));
        }
    }

    // Unload chunks that left the view.
    let to_remove: Vec<IVec2> = streaming
        .active
        .keys()
        .filter(|c| !desired.contains(*c))
        .copied()
        .collect();
    for c in to_remove {
        if let Some(entities) = streaming.active.remove(&c) {
            for e in entities {
                commands.entity(e).despawn();
            }
        }
    }

    // Load missing chunks, nearest first, up to the per-frame budget.
    let mut missing: Vec<IVec2> = desired
        .iter()
        .filter(|c| !streaming.active.contains_key(*c))
        .copied()
        .collect();
    missing.sort_by_key(|c| (c.x - ccx).pow(2) + (c.y - ccy).pow(2));

    let mut budget = CHUNK_BUDGET_PER_FRAME;
    for c in missing {
        if budget == 0 {
            break;
        }
        budget -= 1;
        let cloud = generate_chunk(world_seed, c.x, c.y, &cfg, &registry, &ground);
        let mut entities = Vec::with_capacity(cloud.points.len());
        for point in &cloud.points {
            let Some(meta) = registry.get(&point.module_id) else {
                continue;
            };
            let Some(gm) = gltf.named_meshes.get(point.module_id.as_str()) else {
                continue;
            };
            let Some(gltf_mesh) = gltf_meshes.get(gm) else {
                continue; // sub-asset not ready — skip this chunk's building for now
            };
            entities.push(spawn_module(
                &mut commands,
                &kit,
                &meshes,
                meta,
                gltf_mesh,
                point,
            ));
        }
        streaming.active.insert(c, entities);
    }
}
