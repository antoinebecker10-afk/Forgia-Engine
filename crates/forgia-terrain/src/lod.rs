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

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, ComputedColliderShape, RigidBody};
use std::collections::HashMap;

use crate::biomes::BiomeMap;
use crate::chunk::{ChunkCoord, ChunkManager, TerrainConfig, CHUNK_X};
use crate::flatten::FlattenZones;
use crate::generation::heightmap_at;
use crate::terrain_material::TerrainSharedMaterial;

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
/// Story-454 : hystérèse de despawn sur le LOD2 inner ring (128m).
/// Un cluster déjà spawn ne se despawn que si player approche à dist < 128 - 16 = 112m.
/// Évite le flicker 2 Hz quand player marche le long de la frontière LOD2.
pub const LOD2_INNER_HYSTERESIS_M: f32 = 16.0;
/// Story-577 v3 — biais de profondeur (skirt) appliqué au LOD2 sous les chunks.
/// Depuis le fix extent-aware, les tiles LOD2 chevauchent les chunks (~38–160m) ;
/// au même Y → z-fighting (« la texture change selon l'angle caméra »). On descend
/// le tile LOD2 de ce biais → dans la zone de recouvrement le chunk gagne toujours
/// le depth-test (pas de flicker) ; au-delà (trou comblé + lointain) le LOD2 est
/// 2m sous la hauteur vraie = imperceptible à 144m+ et angle rasant. Mesh + arbres/
/// rochers (enfants du tile) descendent ENSEMBLE → arbres restent posés sur le LOD2.
pub const LOD2_DEPTH_BIAS_M: f32 = 2.0;

const CLUSTER_CHUNKS: i32 = 4;
const CHUNK_SIZE_M: f32 = CHUNK_X as f32;
const CLUSTER_SIZE_M: f32 = CLUSTER_CHUNKS as f32 * CHUNK_SIZE_M;
/// Demi-diagonale d'un cluster LOD2 (128m) : distance centre→coin = (S/2)·√2 ≈ 90.5m.
/// Sert à rendre l'inclusion/exclusion LOD2 sensible à l'ÉTENDUE du cluster, pas
/// seulement à son centre. Story-577 : tester le centre laissait un trou annulaire
/// (~144–181m en diagonale) car un cluster centré < `LOD1_MAX_M` mais dont le coin
/// dépasse `LOD1_MAX_M` était exclu du LOD2 alors que les chunks (⌀ ≈ view_m) ne
/// l'atteignaient pas → ni chunk ni LOD2 = skybox à travers le sol.
const CLUSTER_HALF_DIAG_M: f32 = CLUSTER_SIZE_M * std::f32::consts::FRAC_1_SQRT_2;
/// Wave 5 phase 2g : UV tile reps pour LOD2 textured mesh. Matche la densité
/// chunks LOD0/LOD1 où 1 tile texture = 1 chunk de 32m. LOD2 = 128m donc
/// 4 reps de texture pour visual continuity.
const UV_TILE_REPS: f32 = CLUSTER_CHUNKS as f32;
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
    /// Story-453 : points d'échantillonnage LOD0 vs LOD2 pour CHK-3 asymmetry.
    /// 12 points fixes en ring autour de (0,0). Mis à jour 1Hz par
    /// `sys_update_lod_sample_points`. Exporté dans `forgia_terrain_lod.json`.
    pub sample_points: Vec<LodSamplePoint>,
    /// Story-577 : couverture chunk/LOD2 par anneau autour du player (détection
    /// trou annulaire). Rempli 1Hz par `sys_update_lod_coverage`, exporté dans le
    /// sensor. Un `gap > 0` = points sans chunk NI LOD2 = sol manquant visible.
    pub coverage_rings: Vec<LodCoverageRing>,
}

#[derive(Clone, Copy, Debug)]
pub struct LodSamplePoint {
    pub x: f32,
    pub z: f32,
    pub lod0_y: f32,
    pub lod2_y: f32,
    pub sea_level: f32,
}

/// Story-577 : couverture terrain à un rayon donné autour du player. Pour `samples`
/// angles répartis sur le cercle, compte combien de points sont couverts par un
/// chunk loaded, par un LOD2 tile, ou par RIEN (`gap`). Un `gap > 0` localise le
/// trou annulaire (chunks ⌀ ≈ view_m ne se rejoignent pas avec les LOD2 tiles).
#[derive(Clone, Copy, Debug)]
pub struct LodCoverageRing {
    pub radius_m: f32,
    pub samples: u32,
    pub chunk_covered: u32,
    pub lod2_covered: u32,
    pub gap: u32,
}

/// Simule la Y produite par `build_lod2_terrain_mesh` à un point (x, z).
/// Doit rester en sync avec le mesh builder : si une logique de clamp / offset
/// est réintroduite côté LOD2, mettre à jour cette fonction → CHK-3 détectera
/// instantanément l'asymétrie.
///
/// Actuellement (post-story-450 phase 2g + fix water clamp removal) : raw heightmap.
pub fn simulate_lod2_y_at(x: f32, z: f32, terrain_cfg: &TerrainConfig) -> f32 {
    heightmap_at(x, z, terrain_cfg)
}

/// 12 positions cardinales fixes à 3 rings (64m, 128m, 256m) autour de (0,0).
/// Permet un échantillonnage déterministe indépendant de la position joueur.
const SAMPLE_POINTS_XZ: [(f32, f32); 12] = [
    // Ring 64m
    (64.0, 0.0),
    (-64.0, 0.0),
    (0.0, 64.0),
    (0.0, -64.0),
    // Ring 128m (LOD0/LOD2 transition)
    (128.0, 0.0),
    (-128.0, 0.0),
    (0.0, 128.0),
    (0.0, -128.0),
    // Ring 256m
    (181.0, 181.0),
    (-181.0, -181.0),
    (181.0, -181.0),
    (-181.0, 181.0),
];

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
    /// Wave 5 phase 2c+2e : OBSOLETE — remplacé par SceneRoot kaykit-forest GLBs.
    #[allow(dead_code)]
    tree_imposter_mesh: Option<Handle<Mesh>>,
    /// Wave 5 phase 2c+2e : OBSOLETE — kaykit-forest materials baked dans GLB.
    #[allow(dead_code)]
    tree_material_cache: HashMap<u8, Handle<StandardMaterial>>,
    /// Wave 5 phase 2d+2e : OBSOLETE — remplacé par SceneRoot kaykit-forest rocks.
    #[allow(dead_code)]
    rock_imposter_mesh: Option<Handle<Mesh>>,
    /// Wave 5 phase 2d+2e : OBSOLETE — kaykit-forest rocks ont leur material.
    #[allow(dead_code)]
    rock_material: Option<Handle<StandardMaterial>>,
    /// Wave 5 phase 2e : Scene handles kaykit-forest trees (3 variants).
    /// Loaded une fois, instanciés N × per tile via SceneRoot — auto-instancing
    /// Bevy si même Handle<Scene> partagé.
    tree_scenes: Vec<Handle<Scene>>,
    /// Wave 5 phase 2e : Scene handles kaykit-forest rocks (3 variants).
    rock_scenes: Vec<Handle<Scene>>,
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
    if !frame_counter.is_multiple_of(15) {
        return;
    }

    let Some(player_tf) = player_q.iter().next() else {
        return;
    };
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
                if dist_sq > lod1_sq {
                    ChunkLod::Lod2
                } else if dist_sq > lod0_sq {
                    ChunkLod::Lod1
                } else {
                    ChunkLod::Lod0
                }
            }
            ChunkLod::Lod1 => {
                if dist_sq > lod1_back_sq {
                    ChunkLod::Lod2
                } else if dist_sq < lod0_sq {
                    ChunkLod::Lod0
                } else {
                    ChunkLod::Lod1
                }
            }
            ChunkLod::Lod2 => {
                if dist_sq < lod0_back_sq {
                    ChunkLod::Lod0
                } else if dist_sq < lod1_back_sq {
                    ChunkLod::Lod1
                } else {
                    ChunkLod::Lod2
                }
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
    flatten_zones: Option<&FlattenZones>,
) -> Mesh {
    const SUBDIVS: usize = 16; // 16 quads = 17 verts par côté
    const VERTS_PER_SIDE: usize = SUBDIVS + 1;
    const HALF: f32 = CLUSTER_SIZE_M * 0.5;
    const STEP: f32 = CLUSTER_SIZE_M / SUBDIVS as f32;

    let total_verts = VERTS_PER_SIDE * VERTS_PER_SIDE;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(total_verts);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(total_verts);
    // Wave 5 phase 2a — per-vertex biome color (Skyrim/Witcher 3 pattern).
    // Audit 2026-06-05 : la couleur est désormais calculée en PASS 2 (après
    // compute_normals) via `blend_biome_color` — rock/snow tint IDENTIQUE au LOD0.
    // Avant : `biome.color()` brut + normales plates [0,1,0] → lointain sombre et
    // sans roche/neige = perçu comme une "couche" distincte vs le proche LOD0.

    // Wave 5 phase 2d retiré 2026-05-18 : le clamp Y=sea_level + bleu marine
    // créait des "phantom water patches" au LOD2 ring là où le heightmap dip
    // légèrement sous sea_level mais que LOD0 (vrai chunk mesh) montre du sol
    // sec (heightmap ondulé). Quand le joueur s'approche < 128m → LOD2
    // despawn → patch d'eau disparaît, sol sec apparaît = bug visuel évident.
    // LOD0 (`meshing_heightmap.rs`) n'a JAMAIS clampé sea_level → asymétrie.
    // Fix : LOD2 utilise raw_y + biome color partout, comme LOD0/LOD1.
    // Trade-off accepté : bevy_water plane peut être coupé en bout de mega-
    // tiles 1500m (cohérent avec V1). Story-451 Phase 2h pour water mesh
    // séparé si besoin futur d'horizon water > 1500m.

    for j in 0..VERTS_PER_SIDE {
        for i in 0..VERTS_PER_SIDE {
            let local_x = (i as f32) * STEP - HALF;
            let local_z = (j as f32) * STEP - HALF;
            let world_x = cluster_center_xz.x + local_x;
            let world_z = cluster_center_xz.y + local_z;
            let sample_x = world_x + sample_offset.0;
            let sample_z = world_z + sample_offset.1;
            let raw_y = heightmap_at(sample_x, sample_z, terrain_cfg);
            // Story-577 v2 : applique le FlattenZones du village (coords MONDE sans
            // offset, comme build_chunk_mesh:84). Indispensable depuis que le fix
            // extent-aware fait chevaucher les tiles LOD2 sur la zone du spawn : sans
            // ça, le mesh LOD2 brut (montagne) recouvrait la ville aplanie.
            let y = match flatten_zones {
                Some(fz) => fz.sample(world_x, world_z, raw_y),
                None => raw_y,
            };

            positions.push([local_x, y, local_z]);
            // Wave 5 phase 2g : UV × UV_TILE_REPS pour densité texture cohérente
            // avec chunks (chunks 32m = 1 rep, LOD2 128m = 4 reps).
            uvs.push([
                (i as f32 / SUBDIVS as f32) * UV_TILE_REPS,
                (j as f32 / SUBDIVS as f32) * UV_TILE_REPS,
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    // Audit 2026-06-05 : VRAIES normales (smooth, mesh indexé) comme LOD0
    // (meshing_heightmap.rs:118 `compute_normals`) au lieu de [0,1,0] plat → le
    // lointain reçoit la lumière correctement (fini la "couche sombre" sous
    // soleil bas qui le faisait paraître une couche distincte du proche).
    mesh.compute_normals();

    // Pass 2 — couleur biome rock(pente)/snow(altitude) IDENTIQUE au LOD0, via la
    // pente extraite des normales fraîchement calculées.
    let normals = mesh
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .and_then(|a| a.as_float3())
        .map(|n| n.to_vec())
        .unwrap_or_default();
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(total_verts);
    for (idx, pos) in positions.iter().enumerate() {
        let sample_x = cluster_center_xz.x + pos[0] + sample_offset.0;
        let sample_z = cluster_center_xz.y + pos[2] + sample_offset.1;
        let slope = normals
            .get(idx)
            .map(|n| (1.0 - n[1].clamp(0.0, 1.0)).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        // Story-577 polish : couleur blendée (cohérent avec les chunks LOD0/LOD1).
        colors.push(crate::meshing_heightmap::blended_vertex_color(
            biome_map, sample_x, sample_z, pos[1], slope, terrain_cfg,
        ));
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
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
    asset_server: Res<AssetServer>,
    mut tile_mgr: ResMut<Lod2TileManager>,
    mut lod_stats: ResMut<LodStats>,
    biome_map: Option<Res<BiomeMap>>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    terrain_shared_mat: Option<Res<TerrainSharedMaterial>>,
    flatten_zones: Option<Res<FlattenZones>>,
    player_q: Query<&Transform>,
    offset: Option<Res<LodSampleOffset>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut frame_counter: Local<u32>,
) {
    *frame_counter += 1;
    if !frame_counter.is_multiple_of(30) {
        return;
    }

    let Some(biome_map) = biome_map else { return };
    let Some(terrain_cfg) = terrain_cfg else {
        return;
    };
    let Some(terrain_shared_mat) = terrain_shared_mat else {
        return;
    };
    let Some(player_tf) = player_q.iter().next() else {
        return;
    };
    let off = offset.map(|r| (r.x, r.z)).unwrap_or((0.0, 0.0));
    let player_pos = player_tf.translation;

    let inner_m = LOD1_MAX_M;
    let outer_m = LOD2_MAX_M;
    if outer_m <= inner_m {
        return;
    }

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
            // Extent-aware (fix story-577 trou annulaire) : on dessine le LOD2 dès que
            // le COIN du cluster dépasse `inner_m` (chunks ⌀ ≈ view_m). Tester le seul
            // centre laissait la bande inner_m..(centre+90m) sans chunk NI LOD2.
            // Recouvrement chunk/LOD2 bénin (même Y heightmap → depth buffer gère le Z).
            let center_dist = dist_sq.sqrt();
            if center_dist + CLUSTER_HALF_DIAG_M >= inner_m && dist_sq < outer_sq {
                desired.insert(key, ());
            }
        }
    }

    // Wave 5 phase 2g : RÉUTILISE TerrainSharedMaterial (PBR + grass diff/normal/
    // roughness textures). Continuité texture LOD0/LOD1/LOD2 — finis les "step"
    // visibles entre foreground textured chunks et LOD2 unlit colored.
    // Vertex colors (biome blend) ×= texture × base_color (white) → texture
    // tinted par biome. Water clamp (vertex=blue) → texture tinted bleu.
    let shared_mat = terrain_shared_mat.0.clone();
    // Ancien cache unlit retiré : materials Res<…> conservé pour build_lod2_terrain_mesh
    // côté tree/rock cache si besoin futur.
    let _ = &mut materials;

    // Wave 5 phase 2e : LOAD vrais GLBs kaykit-forest une fois et cache.
    // Variants pour diversité visuelle (hash bits choisit lequel).
    // Paths relatifs assets/ — kebab-case stable post-rename story-449 wave 5.
    if tile_mgr.tree_scenes.is_empty() {
        const TREE_PATHS: &[&str] = &[
            "models-v1/packs/kaykit-forest/Assets/gltf/Tree_1_A_Color1.gltf",
            "models-v1/packs/kaykit-forest/Assets/gltf/Tree_2_B_Color1.gltf",
            "models-v1/packs/kaykit-forest/Assets/gltf/Tree_1_C_Color1.gltf",
        ];
        for p in TREE_PATHS {
            let h: Handle<Scene> =
                asset_server.load(bevy::asset::AssetPath::from(*p).with_label("Scene0"));
            tile_mgr.tree_scenes.push(h);
        }
    }
    if tile_mgr.rock_scenes.is_empty() {
        const ROCK_PATHS: &[&str] = &[
            "models-v1/packs/kaykit-forest/Assets/gltf/Rock_1_A_Color1.gltf",
            "models-v1/packs/kaykit-forest/Assets/gltf/Rock_1_D_Color1.gltf",
            "models-v1/packs/kaykit-forest/Assets/gltf/Rock_1_G_Color1.gltf",
        ];
        for p in ROCK_PATHS {
            let h: Handle<Scene> =
                asset_server.load(bevy::asset::AssetPath::from(*p).with_label("Scene0"));
            tile_mgr.rock_scenes.push(h);
        }
    }
    // Note : `materials` + `meshes` ResMut conservés pour build_lod2_terrain_mesh.
    let _ = (&materials, &meshes);

    for &key in desired.keys() {
        if tile_mgr.tiles.contains_key(&key) {
            continue;
        }

        let center = cluster_world_center(key);

        // Per-cluster mesh : Y per-vertex heightmap (+ flatten village) + color biome.
        let cluster_mesh =
            build_lod2_terrain_mesh(center, off, &terrain_cfg, &biome_map, flatten_zones.as_deref());
        // B2 (story-587) : collider trimesh sur le mesh LOD2 → le sol lointain
        // (128–1500m) devient collisionnable. Avant, seuls les chunks LOD0/LOD1
        // portaient un `Collider::heightfield` → chute à travers le terrain dès qu'on
        // sortait du ring (téléport, knockback, projectile rapide). Le trimesh épouse
        // exactement le mesh visuel. Pas de `CollisionGroups` (cohérent avec le
        // heightfield LOD0 qui n'en a pas — l'harmonisation des groupes = cleanup à part).
        let lod2_collider =
            Collider::from_bevy_mesh(&cluster_mesh, &ComputedColliderShape::TriMesh(default()));
        let mesh_handle = meshes.add(cluster_mesh);

        let mut tile_ec = commands.spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(shared_mat.clone()),
            // Y = -biais skirt (story-577 v3) : le mesh porte les Y absolus, on
            // descend tout le tile (mesh + arbres enfants) sous les chunks → le
            // chunk gagne le depth-test dans le recouvrement (fin du z-fighting).
            Transform::from_xyz(center.x, -LOD2_DEPTH_BIAS_M, center.y),
            Lod2Tile { cluster_key: key },
            Name::new(format!("Lod2Tile({},{})", key.0, key.1)),
        ));
        if let Some(col) = lod2_collider {
            tile_ec.insert((RigidBody::Fixed, col));
        }
        let tile_entity = tile_ec.id();
        tile_mgr.tiles.insert(key, tile_entity);

        // Polish story-577 : imposteurs (arbres/rochers) UNIQUEMENT pour les clusters
        // ENTIÈREMENT au-delà de la couverture chunk (point le plus proche >= inner_m).
        // Depuis le fix extent-aware, les clusters de bord chevauchent la zone chunk
        // (real foliage + village y vivent) → scatter dessus = clutter au spawn. Le
        // MESH LOD2 couvre toujours (gap fermé), seuls les imposteurs sont sautés.
        let cdx = center.x - player_pos.x;
        let cdz = center.y - player_pos.z;
        if (cdx * cdx + cdz * cdz).sqrt() - CLUSTER_HALF_DIAG_M < inner_m {
            continue;
        }

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
            let world_y = {
                let raw = heightmap_at(world_x + off.0, world_z + off.1, &terrain_cfg);
                match flatten_zones.as_deref() {
                    Some(fz) => fz.sample(world_x, world_z, raw),
                    None => raw,
                }
            };
            // Skip si sous sea_level (pas d'arbres dans l'eau).
            if world_y < terrain_cfg.sea_level + 0.5 {
                continue;
            }
            let biome = biome_map.biome_at(world_x + off.0, world_z + off.1);
            if !biome_supports_distant_trees(biome) {
                continue;
            }
            // Wave 5 phase 2e : pick tree variant via hash bits (3 variants
            // kaykit-forest). Auto-instancing Bevy = même draw call par variant.
            let variant_idx = ((hash >> 24) as usize) % tile_mgr.tree_scenes.len();
            let scene = tile_mgr.tree_scenes[variant_idx].clone();
            // Scale jitter 0.85-1.4 — varieté naturelle de la forêt.
            let scale = 0.85 + ((hash >> 12) & 0xFF) as f32 / 255.0 * 0.55;
            let yaw = ((hash >> 4) as f32 / u32::MAX as f32) * std::f32::consts::TAU;
            commands.entity(tile_entity).with_children(|c| {
                c.spawn((
                    SceneRoot(scene),
                    Transform {
                        translation: Vec3::new(lx, world_y, lz),
                        rotation: Quat::from_rotation_y(yaw),
                        scale: Vec3::splat(scale),
                    },
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
            let world_y = {
                let raw = heightmap_at(world_x + off.0, world_z + off.1, &terrain_cfg);
                match flatten_zones.as_deref() {
                    Some(fz) => fz.sample(world_x, world_z, raw),
                    None => raw,
                }
            };
            if world_y < terrain_cfg.sea_level + 0.5 {
                continue; // pas de rocher sous l'eau
            }
            let biome = biome_map.biome_at(world_x + off.0, world_z + off.1);
            // Rocks scatter sur tous biomes terrestres (no Volcanic = lava, no Swamp)
            use crate::biomes::BiomeType::*;
            if matches!(biome, Volcanic | Swamp) {
                continue;
            }
            // Wave 5 phase 2e : pick rock variant via hash bits (3 variants
            // kaykit-forest). Scale jitter pour silhouette diversifiée.
            let variant_idx = ((hash >> 24) as usize) % tile_mgr.rock_scenes.len();
            let scene = tile_mgr.rock_scenes[variant_idx].clone();
            let scale = 0.6 + ((hash >> 16) & 0xFF) as f32 / 255.0 * 1.5;
            let yaw = ((hash >> 4) as f32 / u32::MAX as f32) * std::f32::consts::TAU;
            commands.entity(tile_entity).with_children(|c| {
                c.spawn((
                    SceneRoot(scene),
                    Transform {
                        translation: Vec3::new(lx, world_y, lz),
                        rotation: Quat::from_rotation_y(yaw),
                        scale: Vec3::splat(scale),
                    },
                ));
            });
        }
    }

    // Story-454 fix : hystérèse inner ring (anti-flicker à la frontière 128m).
    // Sans cette marge, un cluster à dist ~128m oscillait spawn/despawn 2×/sec
    // quand le joueur marchait perpendiculairement (tick 30 frames). Pattern
    // UE5 World Partition : asymétrie load/unload thresholds.
    // - Spawn :   dist >= inner_m            (déjà géré dans desired)
    // - Despawn : dist <  inner_m - LOD2_INNER_HYSTERESIS_M  OU dist > outer_m
    let inner_despawn = (inner_m - LOD2_INNER_HYSTERESIS_M).max(0.0);
    let to_remove: Vec<(i32, i32)> = tile_mgr
        .tiles
        .keys()
        .filter(|k| {
            let center = cluster_world_center(**k);
            let dx = center.x - player_pos.x;
            let dz = center.y - player_pos.z;
            let dist_sq = dx * dx + dz * dz;
            let center_dist = dist_sq.sqrt();
            // Symétrique au spawn (story-577) : despawn UNIQUEMENT si le cluster est
            // ENTIÈREMENT sous l'inner (coin inclus + marge hystérèse) ou hors outer.
            // Sinon on retirerait un tile dont un coin couvre encore le trou.
            (center_dist + CLUSTER_HALF_DIAG_M) < inner_despawn || dist_sq >= outer_sq
        })
        .copied()
        .collect();
    for key in to_remove {
        if let Some(entity) = tile_mgr.tiles.remove(&key) {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
        }
    }

    lod_stats.lod2_tile_count = tile_mgr.tiles.len() as u32;
}

/// Story-577 — sonde de couverture terrain par anneau (1Hz). Pour chaque rayon
/// autour du player, échantillonne `ANGLES` directions et compte les points couverts
/// par un chunk loaded, par un LOD2 tile, ou par RIEN (`gap`). Un `gap > 0` =
/// trou visible (skybox/eau à travers le sol) = root cause du « sol qui ne s'affiche
/// pas au loin ». Garde-fou non-régression du fix extent-aware ci-dessus.
///
/// Mirror du pattern player `Query<&Transform>` + `.iter().next()` des autres
/// systèmes LOD → la couverture est mesurée par rapport au MÊME point que le LOD2.
pub fn sys_update_lod_coverage(
    chunk_mgr: Option<Res<ChunkManager>>,
    tile_mgr: Res<Lod2TileManager>,
    player_q: Query<&Transform>,
    mut lod_stats: ResMut<LodStats>,
    time: Res<Time>,
    mut last_write: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last_write < 1.0 {
        return;
    }
    *last_write = now;

    let Some(chunk_mgr) = chunk_mgr else { return };
    let Some(player_tf) = player_q.iter().next() else {
        return;
    };
    let player_pos = player_tf.translation;

    // Rayons sondés : encadrent la transition chunk→LOD2 (view_m ≈ LOD1_MAX_M = 128m)
    // où le trou annulaire apparaissait (~144–181m, pire en diagonale).
    const RADII: [f32; 7] = [96.0, 120.0, 140.0, 160.0, 180.0, 220.0, 300.0];
    const ANGLES: u32 = 16;

    let mut rings = Vec::with_capacity(RADII.len());
    for &r in &RADII {
        let mut chunk_covered = 0u32;
        let mut lod2_covered = 0u32;
        let mut gap = 0u32;
        for a in 0..ANGLES {
            let theta = a as f32 / ANGLES as f32 * std::f32::consts::TAU;
            let wx = player_pos.x + r * theta.cos();
            let wz = player_pos.z + r * theta.sin();
            let chunk = chunk_mgr
                .loaded_entities
                .contains_key(&ChunkCoord::from_world(Vec3::new(wx, 0.0, wz)));
            let lod2 = tile_mgr.tiles.contains_key(&cluster_key_from_world(wx, wz));
            if chunk {
                chunk_covered += 1;
            }
            if lod2 {
                lod2_covered += 1;
            }
            if !chunk && !lod2 {
                gap += 1;
            }
        }
        rings.push(LodCoverageRing {
            radius_m: r,
            samples: ANGLES,
            chunk_covered,
            lod2_covered,
            gap,
        });
    }
    lod_stats.coverage_rings = rings;
}

/// Sensor `forgia_terrain_lod.json` toutes les 1s (observability-required.md).
pub fn export_lod_sensor_system(
    lod_stats: Res<LodStats>,
    tile_mgr: Res<Lod2TileManager>,
    time: Res<Time>,
    mut last_write: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last_write < 1.0 {
        return;
    }
    *last_write = now;

    // Story-453 : serialize sample_points (LOD0 vs LOD2 dual reading).
    let sp_json = if lod_stats.sample_points.is_empty() {
        "[]".to_string()
    } else {
        let parts: Vec<String> = lod_stats
            .sample_points
            .iter()
            .map(|p| {
                format!(
            "{{\"x\":{:.1},\"z\":{:.1},\"lod0_y\":{:.3},\"lod2_y\":{:.3},\"sea_level\":{:.2}}}",
            p.x, p.z, p.lod0_y, p.lod2_y, p.sea_level
        )
            })
            .collect();
        format!("[{}]", parts.join(","))
    };

    // Story-577 : couverture par anneau + résumé pire-trou (gap fraction max).
    let cov_json = if lod_stats.coverage_rings.is_empty() {
        "[]".to_string()
    } else {
        let parts: Vec<String> = lod_stats
            .coverage_rings
            .iter()
            .map(|c| {
                format!(
                    "{{\"r\":{:.0},\"samples\":{},\"chunk\":{},\"lod2\":{},\"gap\":{}}}",
                    c.radius_m, c.samples, c.chunk_covered, c.lod2_covered, c.gap
                )
            })
            .collect();
        format!("[{}]", parts.join(","))
    };
    let (worst_gap_r, max_gap_frac) =
        lod_stats
            .coverage_rings
            .iter()
            .fold((0.0f32, 0.0f32), |(wr, wf), c| {
                let frac = if c.samples > 0 {
                    c.gap as f32 / c.samples as f32
                } else {
                    0.0
                };
                if frac > wf {
                    (c.radius_m, frac)
                } else {
                    (wr, wf)
                }
            });

    let json = format!(
        "{{\"timestamp_secs\":{:.1},\"lod0_count\":{},\"lod1_count\":{},\"lod2_count\":{},\"lod2_tile_count\":{},\"transitions_last_frame\":{},\"lod0_max_m\":{:.0},\"lod1_max_m\":{:.0},\"lod2_max_m\":{:.0},\"sample_points\":{},\"coverage\":{},\"worst_gap_r\":{:.0},\"max_gap_frac\":{:.3}}}",
        now,
        lod_stats.lod0_count,
        lod_stats.lod1_count,
        lod_stats.lod2_count,
        tile_mgr.tiles.len(),
        lod_stats.transitions_last_frame,
        LOD0_MAX_M,
        LOD1_MAX_M,
        LOD2_MAX_M,
        sp_json,
        cov_json,
        worst_gap_r,
        max_gap_frac,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia_terrain_lod.json", json);
}

/// Story-453 : remplit `LodStats.sample_points` à 1Hz pour CHK-3 LOD asymmetry.
/// Compare lod0_y (heightmap canonique) vs lod2_y (simulation mesh LOD2).
/// Si la fonction `simulate_lod2_y_at` diverge de `heightmap_at` (ex: futur
/// re-introduction d'un clamp sea_level), CHK-3 alertera.
pub fn sys_update_lod_sample_points(
    terrain_cfg: Option<Res<TerrainConfig>>,
    mut lod_stats: ResMut<LodStats>,
    time: Res<Time>,
    mut last_write: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last_write < 1.0 {
        return;
    }
    *last_write = now;

    let Some(cfg) = terrain_cfg else { return };
    let sea = cfg.sea_level;

    if lod_stats.sample_points.len() != SAMPLE_POINTS_XZ.len() {
        lod_stats.sample_points.clear();
        lod_stats.sample_points.resize(
            SAMPLE_POINTS_XZ.len(),
            LodSamplePoint {
                x: 0.0,
                z: 0.0,
                lod0_y: 0.0,
                lod2_y: 0.0,
                sea_level: sea,
            },
        );
    }
    for (i, &(x, z)) in SAMPLE_POINTS_XZ.iter().enumerate() {
        let lod0_y = heightmap_at(x, z, &cfg);
        let lod2_y = simulate_lod2_y_at(x, z, &cfg);
        lod_stats.sample_points[i] = LodSamplePoint {
            x,
            z,
            lod0_y,
            lod2_y,
            sea_level: sea,
        };
    }
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
