//! # forgia-village-loader
//!
//! Plugin Bevy : charge un village (TOML data-driven, [`forgia_village_kit::VillageDef`])
//! et spawne ramparts + buildings via [`forgia_prefab`]. Calcule également les
//! endpoints des routes radiales (le caller construit le mesh ribbon via
//! `forgia-terrain::build_path_network`).
//!
//! ## Flow d'utilisation
//!
//! 1. Caller insère `LoadVillageRequest { toml_path }` (Resource).
//! 2. Système [`process_village_request`] le consomme : load TOML, spawn entités,
//!    insère `VillageLoadResult` + retire la request.
//! 3. Caller lit `VillageLoadResult` pour positionner Player + construire les
//!    paths via son propre pipeline terrain.
//! 4. OnExit cleanup : entités taggées [`VillageMarker`] sont despawnées par
//!    le système de cleanup du caller (pattern V2, pas centralisé ici).
//!
//! ## Pourquoi pas spawn Player ici
//!
//! Le Player vit dans `forgia-player`, son spawn est géré par le mode (RPG/FPS).
//! Le loader publie seulement la position cible. Pas de couplage cross-domain.

use bevy::prelude::*;
use bevy_rapier3d::prelude::{AsyncSceneCollider, ComputedColliderShape, RigidBody};
use forgia_genome_village::VillageGenome;
use forgia_prefab::{reset_prefab_stats, spawn_gltf_prefab, PrefabSpawn, PrefabStats};
use forgia_terrain::{heightmap_at, TerrainConfig};
use forgia_village_generator::generate as generate_village;
use forgia_village_kit::{
    compute_rampart_pieces, rampart_wall_stretch, KitResolver, RampartPieceKind, VillageDef,
};
use std::sync::atomic::{AtomicU32, Ordering};

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

pub struct ForgiaVillageLoaderPlugin;

impl Plugin for ForgiaVillageLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VillageStats>();
        app.add_systems(
            Update,
            (
                process_village_genome_request,
                process_village_request,
                write_village_sensor,
            )
                .chain(),
        );
    }
}

/// Story-442 — preferred input path : load a procgen village from a genome
/// TOML, run the generator, spawn entities. Replaces the hardcoded-positions
/// `LoadVillageRequest` flow for new code (kept for backward compat).
#[derive(Resource, Clone)]
pub struct LoadVillageGenomeRequest {
    pub genome_path: String,
    pub terrain_sample_offset: Vec2,
    /// World XZ position where the village's local-origin maps to.
    /// Caller (game mode) decides where the village sits in the world ;
    /// generator stays world-agnostic. No hardcoded `(16, 16)` in loader.
    pub world_center: Vec2,
}

/// Insert this Resource to trigger village load on next frame.
#[derive(Resource, Clone)]
pub struct LoadVillageRequest {
    pub toml_path: String,
    /// Sample offset to apply when querying [`heightmap_at`] (the world coord
    /// origin for terrain sampling — `forgia-rpg` uses `sample_offset()`).
    pub terrain_sample_offset: Vec2,
}

/// Component marker — attached to every entity spawned by the village loader.
/// Caller cleanup is responsible for despawning these OnExit(State).
#[derive(Component)]
pub struct VillageMarker;

/// Inserted as Resource after a successful load — read by caller to position
/// Player and build path meshes.
#[derive(Resource, Clone, Debug)]
pub struct VillageLoadResult {
    pub village_id: String,
    pub center: Vec2,
    /// World-space spawn position (with Y = terrain + offset).
    pub spawn_position: Vec3,
    pub spawn_yaw_deg: f32,
    /// Pairs `(start_xz_world, end_xz_world, tier)` for each road segment.
    pub roads: Vec<RoadSegment>,
    pub buildings_loaded: u32,
    pub ramparts_pieces: u32,
    /// Bounding radius (m) from the genome — used by callers to set up
    /// foliage exclusion zones around the village.
    pub bounding_radius: f32,
}

#[derive(Clone, Debug)]
pub struct RoadSegment {
    pub start: Vec2,
    pub end: Vec2,
    pub tier: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Stats — mirrored to forgia_village.json 1Hz
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct VillageStats {
    pub buildings_loaded: AtomicU32,
    pub ramparts_pieces: AtomicU32,
    pub roads_count: AtomicU32,
    pub missing_assets: AtomicU32,
    /// 0=pending (request inserted, not yet processed), 1=loaded (success),
    /// 2=error (TOML missing/parse failed). Serialized to `forgia_village.json`
    /// to distinguish "still loading" from "failed silently". (BUG-441-05)
    pub status: std::sync::atomic::AtomicU8,
    last_write_secs: std::sync::Mutex<f32>,
    village_id: std::sync::Mutex<Option<String>>,
    spawn_pos: std::sync::Mutex<Option<Vec3>>,
}

/// Loader status — mapped to `VillageStats::status` AtomicU8.
#[repr(u8)]
pub enum VillageStatus {
    /// No request inserted, no village currently loaded — idle steady state.
    Idle = 0,
    /// Request inserted, but waiting on upstream dependency (TerrainConfig).
    Pending = 1,
    /// Village loaded successfully (entities spawned, paths ready for caller).
    Loaded = 2,
    /// TOML missing or parse failed — see logs for next-step.
    Error = 3,
}

// ─────────────────────────────────────────────────────────────────────────────
// Core system — process the request
// ─────────────────────────────────────────────────────────────────────────────

const PLAYER_SPAWN_HEIGHT_OFFSET: f32 = 2.0;
const BUILDING_Y_OFFSET: f32 = 0.0; // KayKit pivots at floor — no fudge.
const RAMPART_Y_OFFSET: f32 = 0.0;

fn process_village_request(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    request: Option<Res<LoadVillageRequest>>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    stats: Res<VillageStats>,
    prefab_stats: Res<PrefabStats>,
    mut warned_no_terrain: Local<bool>,
) {
    // Gating: must have both request and terrain config (terrain plugin Startup).
    let Some(request) = request else { return };
    // Request present — at minimum we're Pending until TerrainConfig arrives.
    stats.status.store(VillageStatus::Pending as u8, Ordering::Relaxed);
    let Some(terrain_cfg) = terrain_cfg else {
        // BUG-441-01 fix : signaler une seule fois si terrain absent malgré
        // request présente. Si jamais ça reste comme ça > 5s, Antoine voit le
        // log + next-step au lieu d'un silence ambigu.
        if !*warned_no_terrain {
            warn!(
                target: "forgia_village_loader",
                "[village] request '{}' en attente — TerrainConfig absente — next: vérifier que ForgiaTerrainPlugin est ajouté + OnEnter(Rpg) insère TerrainConfig",
                request.toml_path
            );
            *warned_no_terrain = true;
        }
        return;
    };
    *warned_no_terrain = false;

    // Parse TOML.
    let def = match VillageDef::load_from_path(&request.toml_path) {
        Ok(d) => d,
        Err(e) => {
            error!(
                target: "forgia_village_loader",
                "[village] FAILED to load '{}' — {} — next: verifier path TOML ou syntaxe",
                request.toml_path, e
            );
            stats.missing_assets.fetch_add(1, Ordering::Relaxed);
            stats.status.store(VillageStatus::Error as u8, Ordering::Relaxed);
            // Consume request so we don't spam.
            commands.remove_resource::<LoadVillageRequest>();
            return;
        }
    };

    let resolver = KitResolver;
    let center = Vec2::from(def.meta.center);

    // ── Ramparts ─────────────────────────────────────────────────────────
    // Walls are chained N per hexagon side ; the stretch factor ensures each
    // chunk fills its allocated portion exactly (no gap, no overshoot).
    let rampart_pieces = compute_rampart_pieces(&def.ramparts, def.meta.unit_scale);
    let stretch = rampart_wall_stretch(&def.ramparts, def.meta.unit_scale);
    let wall_scale = Vec3::new(
        def.meta.unit_scale * stretch, // X stretched to chunk length
        def.meta.unit_scale,           // Y unchanged
        def.meta.unit_scale,           // Z unchanged
    );
    let mut ramparts_count = 0u32;
    for piece in &rampart_pieces {
        let piece_name = match piece.kind {
            RampartPieceKind::Straight => "wall_straight",
            RampartPieceKind::Gate => "wall_straight_gate",
        };
        let path = match resolver.wall_path(&def.meta.kit, piece_name) {
            Ok(p) => p,
            Err(e) => {
                warn!(target: "forgia_village_loader", "[village] skip rampart piece '{piece_name}' — {e}");
                stats.missing_assets.fetch_add(1, Ordering::Relaxed);
                forgia_prefab::record_prefab_error(&prefab_stats);
                continue;
            }
        };
        let local = Vec2::from(piece.position);
        let world_xz = center + local;
        let y_terrain = heightmap_at(
            world_xz.x + request.terrain_sample_offset.x,
            world_xz.y + request.terrain_sample_offset.y,
            &terrain_cfg,
        );
        let e = spawn_gltf_prefab(
            &mut commands,
            &asset_server,
            &prefab_stats,
            PrefabSpawn::new(path, Vec3::new(world_xz.x, y_terrain + def.ramparts.y_offset + RAMPART_Y_OFFSET, world_xz.y))
                .with_yaw_deg(piece.yaw_deg)
                .with_scale_vec3(wall_scale)
                .with_name(format!("Rampart_{piece_name}_{}", ramparts_count)),
            VillageMarker,
        );
        // Static collider auto-derived from the loaded scene meshes.
        commands.entity(e).insert((
            RigidBody::Fixed,
            AsyncSceneCollider {
                shape: Some(ComputedColliderShape::TriMesh(default())),
                ..default()
            },
        ));
        ramparts_count += 1;
    }

    // ── Buildings ────────────────────────────────────────────────────────
    let mut buildings_count = 0u32;
    for (idx, b) in def.buildings.iter().enumerate() {
        let path = match resolver.building_path(&def.meta.kit, &b.piece, b.color.as_deref()) {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    target: "forgia_village_loader",
                    "[village] skip building '{}' — {} — next: vérifier kit/piece/color dans TOML",
                    b.piece, e
                );
                stats.missing_assets.fetch_add(1, Ordering::Relaxed);
                forgia_prefab::record_prefab_error(&prefab_stats);
                continue;
            }
        };
        let world_xz = center + Vec2::from(b.position);
        let y_terrain = heightmap_at(
            world_xz.x + request.terrain_sample_offset.x,
            world_xz.y + request.terrain_sample_offset.y,
            &terrain_cfg,
        );
        let e = spawn_gltf_prefab(
            &mut commands,
            &asset_server,
            &prefab_stats,
            PrefabSpawn::new(
                path,
                Vec3::new(world_xz.x, y_terrain + BUILDING_Y_OFFSET, world_xz.y),
            )
            .with_yaw_deg(b.yaw_deg)
            .with_scale(b.scale * def.meta.unit_scale)
            .with_name(b.label.clone().unwrap_or_else(|| format!("{}_{idx}", b.piece))),
            VillageMarker,
        );
        commands.entity(e).insert((
            RigidBody::Fixed,
            AsyncSceneCollider {
                shape: Some(ComputedColliderShape::TriMesh(default())),
                ..default()
            },
        ));
        buildings_count += 1;
    }

    // ── Roads — compute endpoints, caller builds the ribbon mesh ─────────
    let radius = def.ramparts.radius;
    let apothem = radius * (std::f32::consts::PI / 6.0).cos();
    let roads: Vec<RoadSegment> = def
        .roads
        .radial
        .iter()
        .map(|r| {
            let rad = r.direction_deg.to_radians();
            let dir = Vec2::new(rad.sin(), rad.cos());
            // Start just outside the rampart wall (apothem + 1m clearance).
            let start = center + dir * (apothem + 1.0);
            let end = start + dir * r.length;
            RoadSegment {
                start,
                end,
                tier: r.tier.clone(),
            }
        })
        .collect();
    let roads_count_u = roads.len() as u32;

    // ── Spawn world position ─────────────────────────────────────────────
    let spawn_local = Vec2::from(def.spawn.player_position);
    let spawn_world_xz = center + spawn_local;
    let spawn_y_terrain = heightmap_at(
        spawn_world_xz.x + request.terrain_sample_offset.x,
        spawn_world_xz.y + request.terrain_sample_offset.y,
        &terrain_cfg,
    );
    let spawn_position = Vec3::new(
        spawn_world_xz.x,
        spawn_y_terrain + PLAYER_SPAWN_HEIGHT_OFFSET,
        spawn_world_xz.y,
    );

    // Update stats + emit result.
    stats.buildings_loaded.store(buildings_count, Ordering::Relaxed);
    stats.ramparts_pieces.store(ramparts_count, Ordering::Relaxed);
    stats.roads_count.store(roads_count_u, Ordering::Relaxed);
    if let Ok(mut lock) = stats.village_id.lock() {
        *lock = Some(def.meta.id.clone());
    }
    if let Ok(mut lock) = stats.spawn_pos.lock() {
        *lock = Some(spawn_position);
    }

    stats.status.store(VillageStatus::Loaded as u8, Ordering::Relaxed);

    info!(
        target: "forgia_village_loader",
        "[village] loaded '{}' — {} buildings, {} ramparts, {} roads, spawn=({:.1},{:.1},{:.1})",
        def.meta.id, buildings_count, ramparts_count, roads_count_u,
        spawn_position.x, spawn_position.y, spawn_position.z
    );

    commands.insert_resource(VillageLoadResult {
        village_id: def.meta.id,
        center,
        spawn_position,
        spawn_yaw_deg: def.spawn.player_yaw_deg,
        roads,
        buildings_loaded: buildings_count,
        ramparts_pieces: ramparts_count,
        // Legacy TOML path : ramparts radius approximate, fallback 18m.
        bounding_radius: def.ramparts.radius.max(18.0),
    });
    commands.remove_resource::<LoadVillageRequest>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Sensor — forgia_village.json 1Hz
// ─────────────────────────────────────────────────────────────────────────────

const SENSOR_INTERVAL_S: f32 = 1.0;

fn write_village_sensor(time: Res<Time>, stats: Res<VillageStats>) {
    let now = time.elapsed_secs();
    let mut last = match stats.last_write_secs.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if now - *last < SENSOR_INTERVAL_S {
        return;
    }
    *last = now;

    let village_id = stats
        .village_id
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| "<none>".to_string());
    let spawn_pos = stats
        .spawn_pos
        .lock()
        .ok()
        .and_then(|g| *g)
        .unwrap_or(Vec3::ZERO);
    let buildings = stats.buildings_loaded.load(Ordering::Relaxed);
    let ramparts = stats.ramparts_pieces.load(Ordering::Relaxed);
    let roads = stats.roads_count.load(Ordering::Relaxed);
    let missing = stats.missing_assets.load(Ordering::Relaxed);
    let status = match stats.status.load(Ordering::Relaxed) {
        0 => "idle",
        1 => "pending",
        2 => "loaded",
        3 => "error",
        _ => "unknown",
    };

    let json = format!(
        "{{\"timestamp_secs\":{:.1},\"status\":\"{}\",\"village_id\":\"{}\",\"buildings_loaded\":{},\"ramparts_pieces\":{},\"roads_count\":{},\"spawn_position\":[{:.2},{:.2},{:.2}],\"missing_assets\":{}}}",
        now, status, village_id, buildings, ramparts, roads,
        spawn_pos.x, spawn_pos.y, spawn_pos.z, missing
    );
    let _ = std::fs::write("forgia_village.json", json);
}

// ─────────────────────────────────────────────────────────────────────────────
// Story-442 — genome-driven path (preferred over LoadVillageRequest)
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn process_village_genome_request(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    request: Option<Res<LoadVillageGenomeRequest>>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    stats: Res<VillageStats>,
    prefab_stats: Res<PrefabStats>,
    mut warned_no_terrain: Local<bool>,
) {
    let Some(request) = request else { return };
    stats.status.store(VillageStatus::Pending as u8, Ordering::Relaxed);
    let Some(terrain_cfg) = terrain_cfg else {
        if !*warned_no_terrain {
            warn!(
                target: "forgia_village_loader",
                "[village-genome] request '{}' en attente — TerrainConfig absente — next: vérifier ForgiaTerrainPlugin",
                request.genome_path
            );
            *warned_no_terrain = true;
        }
        return;
    };
    *warned_no_terrain = false;

    // 1. Load genome TOML.
    let genome = match VillageGenome::load_from_path(&request.genome_path) {
        Ok(g) => g,
        Err(e) => {
            error!(
                target: "forgia_village_loader",
                "[village-genome] FAILED to load genome '{}' — {} — next: vérifier path/syntaxe TOML",
                request.genome_path, e
            );
            stats.missing_assets.fetch_add(1, Ordering::Relaxed);
            stats.status.store(VillageStatus::Error as u8, Ordering::Relaxed);
            commands.remove_resource::<LoadVillageGenomeRequest>();
            return;
        }
    };

    // Safety net : seed = 0 is suspicious uninitialized.
    if genome.meta.seed == 0 {
        warn!(
            target: "forgia_village_loader",
            "[village-genome] genome '{}' has seed=0 (uninitialized?) — next: set explicit seed in TOML",
            genome.meta.id
        );
    }

    // 2. Generate procedural graph.
    let (graph, gen_stats) = match generate_village(&genome) {
        Ok(t) => t,
        Err(e) => {
            error!(
                target: "forgia_village_loader",
                "[village-genome] generator failed for '{}' — {} — next: vérifier parameters genome (bounding_radius, density, target_count)",
                genome.meta.id, e
            );
            stats.status.store(VillageStatus::Error as u8, Ordering::Relaxed);
            commands.remove_resource::<LoadVillageGenomeRequest>();
            return;
        }
    };

    // 3. Spawn buildings from graph nodes.
    let resolver = KitResolver;
    // World center supplied by caller via the request — no hardcode here.
    let world_center = request.world_center;

    let mut buildings_count = 0u32;
    for (node, b) in graph.buildings() {
        let path = match resolver.building_path(&genome.kit_mix.kit, &b.piece, b.color.as_deref()) {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    target: "forgia_village_loader",
                    "[village-genome] skip building '{}' — {} — next: vérifier piece dans kit",
                    b.piece, e
                );
                stats.missing_assets.fetch_add(1, Ordering::Relaxed);
                forgia_prefab::record_prefab_error(&prefab_stats);
                continue;
            }
        };
        let world_xz = world_center + node.position;
        let y = heightmap_at(
            world_xz.x + request.terrain_sample_offset.x,
            world_xz.y + request.terrain_sample_offset.y,
            &terrain_cfg,
        );
        let scale = b.scale.unwrap_or(1.0) * genome.scale.unit_scale;
        let e = spawn_gltf_prefab(
            &mut commands,
            &asset_server,
            &prefab_stats,
            PrefabSpawn::new(path, Vec3::new(world_xz.x, y, world_xz.y))
                .with_yaw_deg(b.yaw_deg)
                .with_scale(scale)
                .with_name(b.label.clone().unwrap_or_else(|| b.piece.clone())),
            VillageMarker,
        );
        commands.entity(e).insert((
            RigidBody::Fixed,
            AsyncSceneCollider {
                shape: Some(ComputedColliderShape::TriMesh(default())),
                ..default()
            },
        ));
        buildings_count += 1;
    }

    // 4. Compute road segments from graph edges.
    let roads: Vec<RoadSegment> = graph
        .edges
        .iter()
        .filter_map(|edge| {
            let from_pos = graph.nodes.get(edge.from)?.position;
            let to_pos = graph.nodes.get(edge.to)?.position;
            Some(RoadSegment {
                start: world_center + from_pos,
                end: world_center + to_pos,
                tier: edge.tier.clone(),
            })
        })
        .collect();
    let roads_count = roads.len() as u32;

    // 5. Spawn position : from genome [spawn] section (data-driven, no hardcode).
    // Safety net : if terrain Y at chosen point is below sea_level, elevate
    // the player above sea_level (avoids "spawn under water" when the village
    // genome's chosen world_center happens to land on low terrain).
    let spawn_local = Vec2::from(genome.spawn.local_offset);
    let spawn_world_xz = world_center + spawn_local;
    let spawn_y_terrain = heightmap_at(
        spawn_world_xz.x + request.terrain_sample_offset.x,
        spawn_world_xz.y + request.terrain_sample_offset.y,
        &terrain_cfg,
    );
    let safe_y = spawn_y_terrain.max(terrain_cfg.sea_level + 1.0);
    let spawn_position = Vec3::new(spawn_world_xz.x, safe_y + 2.0, spawn_world_xz.y);
    if spawn_y_terrain < terrain_cfg.sea_level {
        warn!(
            target: "forgia_village_loader",
            "[village-genome] terrain Y at spawn ({:.2}) below sea_level ({:.2}) — player elevated to {:.2} — next: ajuster world_center du request vers un terrain plus haut OU baisser sea_level dans terrain config",
            spawn_y_terrain, terrain_cfg.sea_level, safe_y + 2.0
        );
    }

    // 6. Update stats + publish result.
    stats.buildings_loaded.store(buildings_count, Ordering::Relaxed);
    stats.ramparts_pieces.store(0, Ordering::Relaxed); // V1 procgen has no ramparts
    stats.roads_count.store(roads_count, Ordering::Relaxed);
    if let Ok(mut lock) = stats.village_id.lock() {
        *lock = Some(genome.meta.id.clone());
    }
    if let Ok(mut lock) = stats.spawn_pos.lock() {
        *lock = Some(spawn_position);
    }
    stats.status.store(VillageStatus::Loaded as u8, Ordering::Relaxed);

    info!(
        target: "forgia_village_loader",
        "[village-genome] generated '{}' (seed={}) — {} buildings, {} roads, retries={}, gen_stats={:?}",
        genome.meta.id, genome.meta.seed, buildings_count, roads_count, gen_stats.poisson_retries, gen_stats.layout_type
    );

    commands.insert_resource(VillageLoadResult {
        village_id: genome.meta.id,
        center: world_center,
        spawn_position,
        spawn_yaw_deg: genome.spawn.yaw_deg,
        roads,
        buildings_loaded: buildings_count,
        ramparts_pieces: 0,
        bounding_radius: genome.scale.bounding_radius,
    });
    commands.remove_resource::<LoadVillageGenomeRequest>();

    let _ = gen_stats; // touch to silence unused warning if optimized away
}

// ─────────────────────────────────────────────────────────────────────────────
// Cleanup helper — caller invokes from OnExit(State).
// ─────────────────────────────────────────────────────────────────────────────

/// Despawn all village entities and clear the load result. Idempotent.
///
/// Bevy 0.18 default: `Commands::entity().despawn()` is recursive.
/// Also resets [`PrefabStats`] so the next session's `forgia_prefab.json`
/// reflects only the new session counts (BUG-441-02).
pub fn cleanup_village(
    mut commands: Commands,
    q: Query<Entity, With<VillageMarker>>,
    stats: Res<VillageStats>,
    prefab_stats: Res<PrefabStats>,
) {
    let n = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<VillageLoadResult>();
    stats.buildings_loaded.store(0, Ordering::Relaxed);
    stats.ramparts_pieces.store(0, Ordering::Relaxed);
    stats.roads_count.store(0, Ordering::Relaxed);
    stats.missing_assets.store(0, Ordering::Relaxed);
    stats.status.store(VillageStatus::Idle as u8, Ordering::Relaxed);
    if let Ok(mut lock) = stats.village_id.lock() {
        *lock = None;
    }
    if let Ok(mut lock) = stats.spawn_pos.lock() {
        *lock = None;
    }
    reset_prefab_stats(&prefab_stats);
    if n > 0 {
        info!(target: "forgia_village_loader", "[village] cleanup despawned {n} entities");
    }
}
