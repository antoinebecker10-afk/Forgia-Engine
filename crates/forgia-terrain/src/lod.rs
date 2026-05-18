//! Terrain LOD — GTA 5-style 3-level chunk detail system.
//!
//! Port direct V1 `forgia-game::terrain::lod` + `lod2_tiles` (~400 LOC fusionnés).
//!
//! | Ring  | Distance       | Contenu                                       |
//! |-------|----------------|-----------------------------------------------|
//! | LOD0  | 0 – `LOD0_MAX` | Full mesh + vegetation + grass                |
//! | LOD1  | `LOD0` – `LOD1`| Mesh seul (no veg, no grass)                  |
//! | LOD2  | `LOD1` – `LOD2`| Mega-tile 128×128m plate, 1 par cluster biome |
//! | Beyond| > `LOD2_MAX`   | Rien (skybox horizon)                         |
//!
//! V2 vertical slice : constantes Rust pures (genome system pas prêt). Hystérèse
//! intégrée pour éviter LOD flip-flop aux frontières.

use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use std::collections::HashMap;

use crate::biomes::BiomeMap;
use crate::chunk::{ChunkManager, TerrainConfig, CHUNK_X};
use crate::generation::heightmap_at;

// ─────────────────────────── Constantes ───────────────────────────

pub const LOD0_MAX_M: f32 = 96.0;
/// Wave 5 phase 2b (story-450) — réduit de 320 → 128m pour matcher
/// `streaming.toml::unload_m` (128). Avant ce fix il y avait un GAP visible
/// entre 128m (fin des chunks loaded) et 320m (début LOD2 tiles) : aucune
/// surface rendue, le skybox/water apparaissait à la place.
/// Maintenant LOD2 mega-tiles démarrent dès 128m → continuité visuelle.
/// Acceptable overdraw 64-128m où LOD2 + chunks loaded coexistent (depth
/// buffer handle Z-fighting, même Y heightmap → pas de flicker).
pub const LOD1_MAX_M: f32 = 128.0;
/// Vision lointaine. V1 = 700m (sea_level=20). V2 étendu à 1500m vu que les
/// mega-tiles sont quasi-gratuites (1 plane unlit par cluster 128m).
pub const LOD2_MAX_M: f32 = 1500.0;
pub const LOD_HYSTERESIS_M: f32 = 16.0;

const CLUSTER_CHUNKS: i32 = 4;
const CHUNK_SIZE_M: f32 = CHUNK_X as f32;
const CLUSTER_SIZE_M: f32 = CLUSTER_CHUNKS as f32 * CHUNK_SIZE_M;
/// Wave 5 phase 1 (story-450) : OBSOLETE depuis HLOD per-vertex heightmap.
/// Le mesh contient maintenant les Y absolus baked → Transform Y = 0.
/// Gardé pour référence historique (V1 = -2.0 sea_level=20, V2 wave 0 = 8.0).
#[allow(dead_code)]
const LOD2_Y_OFFSET_LEGACY: f32 = 8.0;

// ─────────────────────────── ChunkLod (Component) ───────────────────────────

#[derive(Component, Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ChunkLod {
    #[default]
    Lod0,
    Lod1,
    Lod2,
}

// ─────────────────────────── LodStats (Resource) ───────────────────────────

#[derive(Resource, Default)]
pub struct LodStats {
    pub lod0_count: u32,
    pub lod1_count: u32,
    pub lod2_count: u32,
    pub lod2_tile_count: u32,
    pub transitions_last_frame: u32,
}

// ─────────────────────────── Lod2 Mega-Tiles ───────────────────────────

#[derive(Component)]
pub struct Lod2Tile {
    pub cluster_key: (i32, i32),
}

#[derive(Resource, Default)]
pub struct Lod2TileManager {
    pub tiles: HashMap<(i32, i32), Entity>,
    /// Wave 5 phase 1 : ce field n'est plus utilisé (mesh per-cluster avec
    /// Y heightmap baked). Gardé pour backward compat — wave 5 phase 2
    /// pourra l'utiliser pour cache des meshes baked offline (HLOD UE5 pattern).
    #[allow(dead_code)]
    mesh: Option<Handle<Mesh>>,
    material_cache: HashMap<u8, Handle<StandardMaterial>>,
    /// Wave 5 phase 2c : mesh imposter shared (cone simple) pour tree silhouettes
    /// au loin. 1 mesh handle global, instancié per-tree as cluster children.
    tree_imposter_mesh: Option<Handle<Mesh>>,
    /// Materials per biome pour tree silhouette darken (~0.4× biome color).
    tree_material_cache: HashMap<u8, Handle<StandardMaterial>>,
    /// Wave 5 phase 2d : sphere mesh shared pour rock silhouettes.
    rock_imposter_mesh: Option<Handle<Mesh>>,
    /// Material shared rock gris foncé (1 handle global).
    rock_material: Option<Handle<StandardMaterial>>,
}

impl Lod2TileManager {
    pub fn despawn_all(&mut self, commands: &mut Commands) {
        for (_, entity) in self.tiles.drain() {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn(); // wave 2.5 pattern
            }
        }
        self.mesh = None;
        self.material_cache.clear();
    }
}

fn cluster_key_from_world(wx: f32, wz: f32) -> (i32, i32) {
    (
        (wx / CLUSTER_SIZE_M).floor() as i32,
        (wz / CLUSTER_SIZE_M).floor() as i32,
    )
}

fn cluster_world_center(key: (i32, i32)) -> Vec2 {
    Vec2::new(
        key.0 as f32 * CLUSTER_SIZE_M + CLUSTER_SIZE_M * 0.5,
        key.1 as f32 * CLUSTER_SIZE_M + CLUSTER_SIZE_M * 0.5,
    )
}

/// L'offset d'échantillonnage RPG (map_size/2) doit être ajouté pour aligner
/// le biome lookup avec le mesh visible. Exposé sous forme de Resource par
/// forgia-rpg. Fallback Vec2::ZERO si absent.
#[derive(Resource, Clone, Copy, Default)]
pub struct LodSampleOffset {
    pub x: f32,
    pub z: f32,
}

// ─────────────────────────── Systems ───────────────────────────

/// Assigne `ChunkLod` à chaque chunk loaded selon la distance au joueur.
/// Runs every 15 frames (LOD transitions ne nécessitent pas précision/frame).
pub fn update_chunk_lod(
    mut commands: Commands,
    chunk_mgr: Res<ChunkManager>,
    q_chunk_lod: Query<&ChunkLod>,
    player_q: Query<&Transform>,
    mut stats: ResMut<LodStats>,
    mut frame_counter: Local<u32>,
) {
    *frame_counter += 1;
    if !frame_counter.is_multiple_of(15) { return; }

    let Some(player_tf) = player_q.iter().next() else { return };
    let player_pos = player_tf.translation;

    let lod0_sq = LOD0_MAX_M * LOD0_MAX_M;
    let lod1_sq = LOD1_MAX_M * LOD1_MAX_M;
    let lod1_back_sq = (LOD1_MAX_M + LOD_HYSTERESIS_M).powi(2);
    let lod0_back_sq = (LOD0_MAX_M + LOD_HYSTERESIS_M).powi(2);

    stats.lod0_count = 0;
    stats.lod1_count = 0;
    stats.lod2_count = 0;
    stats.transitions_last_frame = 0;

    let coords: Vec<_> = chunk_mgr
        .loaded_entities
        .iter()
        .map(|(c, e)| (*c, *e))
        .collect();

    for (coord, entity) in coords {
        let chunk_world = coord.world_center();
        let dx = chunk_world.x - player_pos.x;
        let dz = chunk_world.z - player_pos.z;
        let dist_sq = dx * dx + dz * dz;

        let current_lod = q_chunk_lod.get(entity).copied().unwrap_or(ChunkLod::Lod0);

        let target_lod = match current_lod {
            ChunkLod::Lod0 => {
                if dist_sq > lod1_sq { ChunkLod::Lod2 }
                else if dist_sq > lod0_sq { ChunkLod::Lod1 }
                else { ChunkLod::Lod0 }
            }
            ChunkLod::Lod1 => {
                if dist_sq > lod1_back_sq { ChunkLod::Lod2 }
                else if dist_sq < lod0_sq { ChunkLod::Lod0 }
                else { ChunkLod::Lod1 }
            }
            ChunkLod::Lod2 => {
                if dist_sq < lod0_back_sq { ChunkLod::Lod0 }
                else if dist_sq < lod1_back_sq { ChunkLod::Lod1 }
                else { ChunkLod::Lod2 }
            }
        };

        if target_lod != current_lod {
            stats.transitions_last_frame += 1;
            commands.entity(entity).insert(target_lod);
        }

        match target_lod {
            ChunkLod::Lod0 => stats.lod0_count += 1,
            ChunkLod::Lod1 => stats.lod1_count += 1,
            ChunkLod::Lod2 => stats.lod2_count += 1,
        }
    }
}

/// Story-450 wave 5 phase 1 — HLOD baked imposters (UE5 World Partition pattern).
///
/// Génère un mesh subdivisé 16×16 quads (17×17 verts) avec Y per-vertex sampled
/// du heightmap. Remplace le plan plat `Plane3d` Y=8 hardcoded par une vraie
/// silhouette terrain au loin.
///
/// Coût : 289 samples heightmap_at par cluster (~30µs sur CPU moderne) × ~420
/// clusters total = ~12ms one-shot, spread sur multiple frames (frame_counter
/// modulo 30 throttle le spawn à 0.5Hz, donc 1-3 tiles/frame).
///
/// Local-space : vertices en (-64..+64, world_y, -64..+64). Transform du tile
/// au (center.x, 0, center.z) → Y absolu vient du mesh, pas du Transform.
fn build_lod2_terrain_mesh(
    cluster_center_xz: Vec2,
    sample_offset: (f32, f32),
    terrain_cfg: &TerrainConfig,
    biome_map: &BiomeMap,
) -> Mesh {
    const SUBDIVS: usize = 16; // 16 quads = 17 verts par côté
    const VERTS_PER_SIDE: usize = SUBDIVS + 1;
    const HALF: f32 = CLUSTER_SIZE_M * 0.5;
    const STEP: f32 = CLUSTER_SIZE_M / SUBDIVS as f32;

    let total_verts = VERTS_PER_SIDE * VERTS_PER_SIDE;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(total_verts);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(total_verts);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(total_verts);
    // Wave 5 phase 2a — per-vertex biome color (Skyrim/Witcher 3 pattern :
    // baked terrain color blend au lieu de material splat per chunk).
    // Vertex colors sont multipliés par base_color = white du material partagé.
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(total_verts);

    // Wave 5 phase 2d : water continuity (bevy_water plane limité au foreground).
    // Si world_y < sea_level → clamp à sea_level + vertex color bleu eau pour
    // surface eau visible au LOD2 ring aussi. Évite "deux eaux différentes"
    // bug visuel (water plane foreground vs LOD2 terrain submergé biome-coloré).
    let water_color_lin = LinearRgba::new(0.28, 0.45, 0.65, 1.0); // bleu marine
    let sea = terrain_cfg.sea_level;

    for j in 0..VERTS_PER_SIDE {
        for i in 0..VERTS_PER_SIDE {
            let local_x = (i as f32) * STEP - HALF;
            let local_z = (j as f32) * STEP - HALF;
            let world_x = cluster_center_xz.x + local_x;
            let world_z = cluster_center_xz.y + local_z;
            let sample_x = world_x + sample_offset.0;
            let sample_z = world_z + sample_offset.1;
            let raw_y = heightmap_at(sample_x, sample_z, terrain_cfg);

            let (world_y, vertex_color) = if raw_y < sea {
                // Submergé : clamp à sea_level + bleu marine.
                ([local_x, sea, local_z], water_color_lin)
            } else {
                // Sur terre : Y heightmap + biome color.
                let biome = biome_map.biome_at(sample_x, sample_z);
                let c = biome.color().to_linear();
                ([local_x, raw_y, local_z], c)
            };
            positions.push(world_y);
            normals.push([0.0, 1.0, 0.0]); // approx — unlit donc OK
            uvs.push([i as f32 / SUBDIVS as f32, j as f32 / SUBDIVS as f32]);
            colors.push([
                vertex_color.red,
                vertex_color.green,
                vertex_color.blue,
                vertex_color.alpha,
            ]);
        }
    }

    // Indices triangles : 2 triangles par quad. Winding CCW vu de +Y (top).
    let mut indices: Vec<u32> = Vec::with_capacity(SUBDIVS * SUBDIVS * 6);
    for j in 0..SUBDIVS {
        for i in 0..SUBDIVS {
            let row = VERTS_PER_SIDE as u32;
            let i32_ = i as u32;
            let j32 = j as u32;
            let a = j32 * row + i32_;
            let b = a + 1;
            let c = a + row;
            let d = c + 1;
            // Top-view CCW : a-c-b et b-c-d
            indices.push(a);
            indices.push(c);
            indices.push(b);
            indices.push(b);
            indices.push(c);
            indices.push(d);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Wave 5 phase 2c — biomes supportant des arbres silhouette au loin.
/// Pattern Skyrim distant trees : silhouette présente mais simplifiée.
fn biome_supports_distant_trees(b: crate::biomes::BiomeType) -> bool {
    use crate::biomes::BiomeType::*;
    matches!(b, Forest | Plains | Jungle | Tundra | Savanna | Swamp)
}

/// Densité d'arbres par cluster LOD2 (128×128m). Bumped 8→24 phase 2d : user
/// feedback "horizon trop vide en assets". Skyrim distant trees ~30/tile.
const LOD2_TREES_PER_CLUSTER: u32 = 24;
/// Densité de rochers par cluster LOD2 (silhouettes complémentaires). Skip
/// si biome ne supporte pas (océan, lava). Pattern Witcher 3 distant rocks.
const LOD2_ROCKS_PER_CLUSTER: u32 = 8;

/// Hash déterministe (cluster_x, cluster_z, seed_offset) → u32 pour
/// scatter positions reproductible per seed.
fn cluster_tree_hash(key: (i32, i32), idx: u32) -> u32 {
    let x = key.0 as u32;
    let z = key.1 as u32;
    let mut h = x.wrapping_mul(0x9E37_79B1);
    h = h.wrapping_add(z.wrapping_mul(0x85EB_CA6B));
    h = h.wrapping_add(idx.wrapping_mul(0xC2B2_AE3D));
    h ^= h >> 16;
    h.wrapping_mul(0x27D4_EB2F)
}

/// Spawn/despawn LOD2 mega-tile planes pour le ring 320–1500m. 1 plane par
/// cluster (4×4 chunks = 128×128m), material per biome (cache shared, 10 max).
///
/// Wave 5 Phase 1 : mesh per-cluster avec Y per-vertex heightmap (silhouette
/// terrain), au lieu du plan plat Y=8 partagé.
#[allow(clippy::too_many_arguments)]
pub fn build_lod2_tiles_system(
    mut commands: Commands,
    mut tile_mgr: ResMut<Lod2TileManager>,
    mut lod_stats: ResMut<LodStats>,
    biome_map: Option<Res<BiomeMap>>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    player_q: Query<&Transform>,
    offset: Option<Res<LodSampleOffset>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut frame_counter: Local<u32>,
) {
    *frame_counter += 1;
    if !frame_counter.is_multiple_of(30) { return; }

    let Some(biome_map) = biome_map else { return };
    let Some(terrain_cfg) = terrain_cfg else { return };
    let Some(player_tf) = player_q.iter().next() else { return };
    let off = offset.map(|r| (r.x, r.z)).unwrap_or((0.0, 0.0));
    let player_pos = player_tf.translation;

    let inner_m = LOD1_MAX_M;
    let outer_m = LOD2_MAX_M;
    if outer_m <= inner_m { return; }

    let inner_sq = inner_m * inner_m;
    let outer_sq = outer_m * outer_m;

    let player_cluster = cluster_key_from_world(player_pos.x, player_pos.z);
    let outer_clusters = (outer_m / CLUSTER_SIZE_M).ceil() as i32 + 1;

    let mut desired: HashMap<(i32, i32), ()> = HashMap::new();
    for dcz in -outer_clusters..=outer_clusters {
        for dcx in -outer_clusters..=outer_clusters {
            let key = (player_cluster.0 + dcx, player_cluster.1 + dcz);
            let center = cluster_world_center(key);
            let dx = center.x - player_pos.x;
            let dz = center.y - player_pos.z;
            let dist_sq = dx * dx + dz * dz;
            if dist_sq >= inner_sq && dist_sq < outer_sq {
                desired.insert(key, ());
            }
        }
    }

    // Wave 5 phase 1+2a : mesh per-cluster (Y heightmap baked + per-vertex
    // biome color). 1 seul material shared (white + vertex_colors enabled) —
    // remplace l'ancien cache par-biome 10 materials.
    const SHARED_MAT_KEY: u8 = 255; // sentinel unique pour shared mat
    let shared_mat = if let Some(h) = tile_mgr.material_cache.get(&SHARED_MAT_KEY) {
        h.clone()
    } else {
        let h = materials.add(StandardMaterial {
            base_color: Color::WHITE, // multiplié par ATTRIBUTE_COLOR per-vertex
            perceptual_roughness: 0.95,
            metallic: 0.0,
            unlit: true,
            ..default()
        });
        tile_mgr.material_cache.insert(SHARED_MAT_KEY, h.clone());
        h
    };

    // Wave 5 phase 2c : tree imposter mesh shared (cone bas-poly).
    // 1 cone primitive instancié N fois par tile, child du tile entity.
    let tree_mesh = tile_mgr
        .tree_imposter_mesh
        .get_or_insert_with(|| meshes.add(Cone::new(1.5, 5.0)))
        .clone();

    // Wave 5 phase 2d : rock imposter (sphere écrasée). Shared mesh + material.
    let rock_mesh = tile_mgr
        .rock_imposter_mesh
        .get_or_insert_with(|| meshes.add(Sphere::new(1.2).mesh().ico(1).unwrap()))
        .clone();
    let rock_mat = tile_mgr
        .rock_material
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.35, 0.32, 0.30), // gris foncé rocky
                perceptual_roughness: 0.95,
                metallic: 0.0,
                unlit: true,
                ..default()
            })
        })
        .clone();

    for &key in desired.keys() {
        if tile_mgr.tiles.contains_key(&key) { continue; }

        let center = cluster_world_center(key);

        // Per-cluster mesh : Y per-vertex heightmap + color per-vertex biome.
        let cluster_mesh =
            build_lod2_terrain_mesh(center, off, &terrain_cfg, &biome_map);
        let mesh_handle = meshes.add(cluster_mesh);

        let tile_entity = commands
            .spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(shared_mat.clone()),
                // Transform Y=0 — le mesh contient déjà les Y absolus heightmap.
                Transform::from_xyz(center.x, 0.0, center.y),
                Lod2Tile { cluster_key: key },
                Name::new(format!("Lod2Tile({},{})", key.0, key.1)),
            ))
            .id();

        // Wave 5 phase 2c : scatter N tree imposters per tile selon biome.
        // Pattern Skyrim distant trees — silhouette présente au loin sans
        // payer le coût des vrais GLB trees du forgia-foliage system.
        for tree_idx in 0..LOD2_TREES_PER_CLUSTER {
            let hash = cluster_tree_hash(key, tree_idx);
            // Position locale dans la tile [-HALF..HALF] avec hash bits.
            let lx = ((hash & 0xFF) as f32 / 255.0 - 0.5) * (CLUSTER_SIZE_M * 0.85);
            let lz = (((hash >> 8) & 0xFF) as f32 / 255.0 - 0.5) * (CLUSTER_SIZE_M * 0.85);
            let world_x = center.x + lx;
            let world_z = center.y + lz;
            let world_y = heightmap_at(world_x + off.0, world_z + off.1, &terrain_cfg);
            // Skip si sous sea_level (pas d'arbres dans l'eau).
            if world_y < terrain_cfg.sea_level + 0.5 {
                continue;
            }
            let biome = biome_map.biome_at(world_x + off.0, world_z + off.1);
            if !biome_supports_distant_trees(biome) {
                continue;
            }
            // Material per biome avec foliage color darken pour silhouette tree.
            let biome_id = biome as u8;
            let tree_mat = if let Some(h) = tile_mgr.tree_material_cache.get(&biome_id) {
                h.clone()
            } else {
                // Darken biome color × 0.4 pour effet "foliage shadowed"
                let c = biome.color().to_linear();
                let darken = LinearRgba::new(c.red * 0.4, c.green * 0.5, c.blue * 0.4, 1.0);
                let h = materials.add(StandardMaterial {
                    base_color: Color::from(darken),
                    perceptual_roughness: 0.95,
                    metallic: 0.0,
                    unlit: true,
                    ..default()
                });
                tile_mgr.tree_material_cache.insert(biome_id, h.clone());
                h
            };
            // Spawn tree comme child du tile (despawn cascade auto).
            commands.entity(tile_entity).with_children(|c| {
                c.spawn((
                    Mesh3d(tree_mesh.clone()),
                    MeshMaterial3d(tree_mat),
                    // Local position relative au tile center (déjà translated
                    // par parent Transform).
                    Transform::from_xyz(lx, world_y + 2.5, lz),
                ));
            });
        }

        // Wave 5 phase 2d : rock imposters scatter, hash offset shifté.
        for rock_idx in 0..LOD2_ROCKS_PER_CLUSTER {
            // Offset 100k pour décorréler du tree hash → distribution indép.
            let hash = cluster_tree_hash(key, rock_idx + 100_000);
            let lx = ((hash & 0xFF) as f32 / 255.0 - 0.5) * (CLUSTER_SIZE_M * 0.9);
            let lz = (((hash >> 8) & 0xFF) as f32 / 255.0 - 0.5) * (CLUSTER_SIZE_M * 0.9);
            let world_x = center.x + lx;
            let world_z = center.y + lz;
            let world_y = heightmap_at(world_x + off.0, world_z + off.1, &terrain_cfg);
            if world_y < terrain_cfg.sea_level + 0.5 {
                continue; // pas de rocher sous l'eau
            }
            let biome = biome_map.biome_at(world_x + off.0, world_z + off.1);
            // Rocks scatter sur tous biomes terrestres (no Volcanic = lava, no Swamp)
            use crate::biomes::BiomeType::*;
            if matches!(biome, Volcanic | Swamp) {
                continue;
            }
            // Scale variation via hash bits — petit/moyen rocher.
            let scale = 0.6 + ((hash >> 16) & 0xFF) as f32 / 255.0 * 1.5;
            commands.entity(tile_entity).with_children(|c| {
                c.spawn((
                    Mesh3d(rock_mesh.clone()),
                    MeshMaterial3d(rock_mat.clone()),
                    Transform::from_xyz(lx, world_y + 0.5, lz)
                        .with_scale(Vec3::splat(scale)),
                ));
            });
        }
    }

    let to_remove: Vec<(i32, i32)> = tile_mgr
        .tiles
        .keys()
        .filter(|k| !desired.contains_key(k))
        .copied()
        .collect();
    for key in to_remove {
        if let Some(entity) = tile_mgr.tiles.remove(&key) {
            if let Ok(mut ec) = commands.get_entity(entity) { ec.try_despawn(); }
        }
    }

    lod_stats.lod2_tile_count = tile_mgr.tiles.len() as u32;
}

/// Sensor `forgia_terrain_lod.json` toutes les 1s (observability-required.md).
pub fn export_lod_sensor_system(
    lod_stats: Res<LodStats>,
    tile_mgr: Res<Lod2TileManager>,
    time: Res<Time>,
    mut last_write: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last_write < 1.0 { return; }
    *last_write = now;

    let json = format!(
        "{{\"timestamp_secs\":{:.1},\"lod0_count\":{},\"lod1_count\":{},\"lod2_count\":{},\"lod2_tile_count\":{},\"transitions_last_frame\":{},\"lod0_max_m\":{:.0},\"lod1_max_m\":{:.0},\"lod2_max_m\":{:.0}}}",
        now,
        lod_stats.lod0_count,
        lod_stats.lod1_count,
        lod_stats.lod2_count,
        tile_mgr.tiles.len(),
        lod_stats.transitions_last_frame,
        LOD0_MAX_M,
        LOD1_MAX_M,
        LOD2_MAX_M,
    );
    let _ = std::fs::write("forgia_terrain_lod.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_ordering_monotone() {
        assert!(LOD0_MAX_M < LOD1_MAX_M);
        assert!(LOD1_MAX_M < LOD2_MAX_M);
    }

    #[test]
    fn cluster_key_at_known_world() {
        // 200m cluster size 128 → floor(200/128) = 1.
        let key = cluster_key_from_world(200.0, -150.0);
        assert_eq!(key.0, 1);
        let center = cluster_world_center(key);
        assert!((center.x - 192.0).abs() < 0.01);
    }
}
