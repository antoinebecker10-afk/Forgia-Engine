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
use std::collections::HashMap;

use crate::biomes::BiomeMap;
use crate::chunk::{ChunkManager, CHUNK_X};

// ─────────────────────────── Constantes ───────────────────────────

pub const LOD0_MAX_M: f32 = 96.0;
pub const LOD1_MAX_M: f32 = 320.0;
/// Vision lointaine. V1 = 700m (sea_level=20). V2 étendu à 1500m vu que les
/// mega-tiles sont quasi-gratuites (1 plane unlit par cluster 128m).
pub const LOD2_MAX_M: f32 = 1500.0;
pub const LOD_HYSTERESIS_M: f32 = 16.0;

const CLUSTER_CHUNKS: i32 = 4;
const CHUNK_SIZE_M: f32 = CHUNK_X as f32;
const CLUSTER_SIZE_M: f32 = CLUSTER_CHUNKS as f32 * CHUNK_SIZE_M;
/// V1 = -2.0 (sea_level=20, tiles sunk sous le terrain low). V2 sea_level=4
/// avec heights 2-28 → tile à -2 cachée. Y=8 = entre sea_level et mid-range,
/// couvre les plats, sommets ressortent, pas de z-fight (LOD1 termine 320m
/// loin de la tile à 320-1500m).
const LOD2_Y_OFFSET: f32 = 8.0;

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
    mesh: Option<Handle<Mesh>>,
    material_cache: HashMap<u8, Handle<StandardMaterial>>,
}

impl Lod2TileManager {
    pub fn despawn_all(&mut self, commands: &mut Commands) {
        for (_, entity) in self.tiles.drain() {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.despawn();
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

/// Spawn/despawn LOD2 mega-tile planes pour le ring 320–700m. 1 plane par cluster
/// (4×4 chunks = 128×128m), material per biome (cache shared, 10 max).
#[allow(clippy::too_many_arguments)]
pub fn build_lod2_tiles_system(
    mut commands: Commands,
    mut tile_mgr: ResMut<Lod2TileManager>,
    mut lod_stats: ResMut<LodStats>,
    biome_map: Option<Res<BiomeMap>>,
    player_q: Query<&Transform>,
    offset: Option<Res<LodSampleOffset>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut frame_counter: Local<u32>,
) {
    *frame_counter += 1;
    if !frame_counter.is_multiple_of(30) { return; }

    let Some(biome_map) = biome_map else { return };
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

    let mesh_handle = tile_mgr
        .mesh
        .get_or_insert_with(|| meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(CLUSTER_SIZE_M * 0.5))))
        .clone();

    for &key in desired.keys() {
        if tile_mgr.tiles.contains_key(&key) { continue; }

        let center = cluster_world_center(key);
        let biome = biome_map.biome_at(center.x + off.0, center.y + off.1);
        let biome_id = biome as u8;

        let mat_handle = if let Some(h) = tile_mgr.material_cache.get(&biome_id) {
            h.clone()
        } else {
            let h = materials.add(StandardMaterial {
                base_color: biome.color(),
                perceptual_roughness: 0.95,
                metallic: 0.0,
                unlit: true,
                ..default()
            });
            tile_mgr.material_cache.insert(biome_id, h.clone());
            h
        };

        let tile_entity = commands
            .spawn((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(mat_handle),
                Transform::from_xyz(center.x, LOD2_Y_OFFSET, center.y),
                Lod2Tile { cluster_key: key },
                Name::new(format!("Lod2Tile({},{})", key.0, key.1)),
            ))
            .id();

        tile_mgr.tiles.insert(key, tile_entity);
    }

    let to_remove: Vec<(i32, i32)> = tile_mgr
        .tiles
        .keys()
        .filter(|k| !desired.contains_key(k))
        .copied()
        .collect();
    for key in to_remove {
        if let Some(entity) = tile_mgr.tiles.remove(&key) {
            if let Ok(mut ec) = commands.get_entity(entity) { ec.despawn(); }
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
