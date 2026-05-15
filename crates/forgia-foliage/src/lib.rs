//! # forgia-foliage
//!
//! Vegetation placement minimal vertical slice. Per-chunk Poisson-disk seeded
//! tree spawn, density proportional to `BiomeType` (port direct V1 `biome_max_per_chunk`,
//! divisée par 20 pour vertical slice).
//!
//! Trees procéduraux (cylindre tronc + sphère canopée) jusqu'à ce qu'un asset
//! GLB nature soit ajouté à V2 — pattern AAA "ship des shapes d'abord, swap GLB
//! plus tard".
//!
//! Lifecycle :
//! - `Added<ChunkCoord>` → spawn vegetation pour ce chunk
//! - chunk despawné → arbres orphelins despawnés via tracking dans `VegetationManager`
//! - OnExit(Rpg) → cleanup global via `RpgWorldMarker` (côté forgia-rpg)
//!
//! Sensor : `forgia_vegetation.json` (loaded_chunks, total_trees, per-biome).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_core::prelude::*;
use forgia_terrain::{
    sampling::poisson_disk_sample, BiomeMap, BiomeType, ChunkCoord, ChunkLod, PathNetwork,
    TerrainConfig, CHUNK_X, CHUNK_Z,
};
use std::collections::HashMap;

pub mod prelude {
    pub use crate::{biome_max_per_chunk, ForgiaFoliagePlugin, VegetationManager};
}

/// Port direct V1 `forgia-game::terrain::vegetation::types::biome_max_per_chunk`,
/// **divisé par 20** pour vertical slice V2 (au lieu de 60-900 → 3-45 arbres/chunk).
/// W3+ : raccrocher à un genome `vegetation_density` quand le système genome V2 sera prêt.
pub fn biome_max_per_chunk(biome: BiomeType) -> usize {
    match biome {
        BiomeType::Jungle => 45,
        BiomeType::Forest => 35,
        BiomeType::Swamp => 22,
        BiomeType::Plains => 18,
        BiomeType::Savanna => 14,
        BiomeType::Mountain => 11,
        BiomeType::Tundra => 8,
        BiomeType::Canyon => 8,
        BiomeType::Desert => 6,
        BiomeType::Volcanic => 3,
    }
}

/// Espacement min Poisson disk (mètres) par biome.
fn biome_min_spacing(biome: BiomeType) -> f32 {
    match biome {
        BiomeType::Jungle | BiomeType::Forest => 3.5,
        BiomeType::Swamp | BiomeType::Plains | BiomeType::Savanna => 5.0,
        BiomeType::Mountain | BiomeType::Tundra | BiomeType::Canyon => 7.0,
        BiomeType::Desert => 9.0,
        BiomeType::Volcanic => 12.0,
    }
}

/// Couleur trunk + canopy approximative par biome (tint variation cosmétique).
fn biome_tree_colors(biome: BiomeType) -> (Color, Color) {
    let trunk = Color::srgb(0.38, 0.26, 0.18); // brun bois neutre toutes biomes
    let canopy = match biome {
        BiomeType::Jungle => Color::srgb(0.10, 0.45, 0.15),
        BiomeType::Forest => Color::srgb(0.18, 0.42, 0.14),
        BiomeType::Swamp => Color::srgb(0.22, 0.34, 0.16),
        BiomeType::Plains => Color::srgb(0.30, 0.55, 0.20),
        BiomeType::Savanna => Color::srgb(0.55, 0.50, 0.18),
        BiomeType::Mountain => Color::srgb(0.18, 0.32, 0.18),
        BiomeType::Tundra => Color::srgb(0.62, 0.68, 0.70),
        BiomeType::Canyon => Color::srgb(0.42, 0.36, 0.20),
        BiomeType::Desert => Color::srgb(0.45, 0.48, 0.22),
        BiomeType::Volcanic => Color::srgb(0.22, 0.18, 0.15),
    };
    (trunk, canopy)
}

#[derive(Component)]
pub struct VegetationTree;

#[derive(Resource, Default)]
pub struct VegetationManager {
    /// Tracking pour despawn ciblé quand un chunk est déchargé.
    pub chunk_entities: HashMap<ChunkCoord, Vec<Entity>>,
    /// Total cumulé pour sensor.
    pub total_trees: usize,
    /// Distribution par biome (sensor).
    pub per_biome: HashMap<&'static str, usize>,
    /// Caches mesh procéduraux (tronc + canopée, partagés tous arbres).
    pub trunk_mesh: Option<Handle<Mesh>>,
    pub canopy_mesh: Option<Handle<Mesh>>,
    /// Caches material par biome (1 tronc + 1 canopée / biome).
    pub trunk_mats: HashMap<u8, Handle<StandardMaterial>>,
    pub canopy_mats: HashMap<u8, Handle<StandardMaterial>>,
}

pub struct ForgiaFoliagePlugin;

impl Plugin for ForgiaFoliagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VegetationManager>()
            .add_systems(Startup, init_proc_meshes)
            .add_systems(
                Update,
                (
                    populate_new_chunks,
                    despawn_far_lod_vegetation,
                    despawn_unloaded_chunks,
                    write_vegetation_sensor,
                )
                    .chain()
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Rpg)),
            );
    }
}

fn init_proc_meshes(mut veg: ResMut<VegetationManager>, mut meshes: ResMut<Assets<Mesh>>) {
    // Tronc cylindre 0.3m diamètre × 2.5m haut.
    veg.trunk_mesh = Some(meshes.add(Cylinder::new(0.15, 2.5)));
    // Canopée sphère 1.4m diamètre.
    veg.canopy_mesh = Some(meshes.add(Sphere::new(1.4)));
}

/// Pour chaque chunk présent qui n'a pas encore reçu de vegetation, échantillonne
/// N positions par Poisson disk et spawn arbres procéduraux. On NE dépend PAS de
/// `Added<ChunkCoord>` (timing fragile entre systèmes streamer/foliage dans des
/// plugins différents) — on filtre par contains_key sur le tracking interne.
///
/// Si trunk/canopy meshes pas encore initialisés (cleanup OnExit a remplacé la
/// Resource par défaut), on les ré-initialise lazily.
fn populate_new_chunks(
    mut commands: Commands,
    mut veg: ResMut<VegetationManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    biome_map: Option<Res<BiomeMap>>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    rpg_offset: Option<Res<RpgSampleOffset>>,
    path_net: Option<Res<PathNetwork>>,
    q_chunks: Query<(Entity, &ChunkCoord, Option<&ChunkLod>)>,
) {
    let (Some(biome_map), Some(terrain_cfg), Some(rpg_offset)) =
        (biome_map, terrain_cfg, rpg_offset) else { return };

    // Lazy init des meshes procéduraux (idempotent, survit aux resets OnExit).
    if veg.trunk_mesh.is_none() {
        veg.trunk_mesh = Some(meshes.add(Cylinder::new(0.15, 2.5)));
    }
    if veg.canopy_mesh.is_none() {
        veg.canopy_mesh = Some(meshes.add(Sphere::new(1.4)));
    }
    let trunk_mesh = veg.trunk_mesh.clone().unwrap();
    let canopy_mesh = veg.canopy_mesh.clone().unwrap();

    for (chunk_entity, coord, lod) in &q_chunks {
        if veg.chunk_entities.contains_key(coord) { continue; }
        let lod_val = lod.copied().unwrap_or(ChunkLod::Lod0);
        // Vegetation sur LOD0 (full) + LOD1 (clairsemé ×0.2). LOD2 = pas d'arbres
        // (mega-tiles plates, distance > 320 m). Pattern AAA mid-distance fade-out.
        let density_factor = match lod_val {
            ChunkLod::Lod0 => 1.0_f32,
            ChunkLod::Lod1 => 0.2,
            ChunkLod::Lod2 => continue,
        };

        let origin = coord.world_origin();
        let center = coord.world_center();
        let sample_x = center.x + rpg_offset.x;
        let sample_z = center.z + rpg_offset.z;
        let biome = biome_map.biome_at(sample_x, sample_z);

        let target = ((biome_max_per_chunk(biome) as f32) * density_factor).round() as usize;
        // LOD1 : espacement Poisson augmenté pour distribution naturelle (sinon
        // les ×0.2 premiers points s'agglutinent dans un coin).
        let spacing = biome_min_spacing(biome) / density_factor.sqrt();
        let seed = derive_chunk_seed(coord, terrain_cfg.seed);

        let pts = poisson_disk_sample(
            CHUNK_X as f32,
            CHUNK_Z as f32,
            spacing,
            seed,
            30,
        );

        // Tronc/canopée material lazily cachés par biome.
        let (trunk_color, canopy_color) = biome_tree_colors(biome);
        let trunk_mat = veg
            .trunk_mats
            .entry(biome as u8)
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: trunk_color,
                    perceptual_roughness: 0.88,
                    ..default()
                })
            })
            .clone();
        let canopy_mat = veg
            .canopy_mats
            .entry(biome as u8)
            .or_insert_with(|| {
                // Boost ×1.8 + roughness 0.65 + léger emissive : sans ces fixes
                // la sphère canopy reçoit toutes les ombres et apparaît noire
                // au sunset (couleurs biome déjà sombres × 0 IBL × roughness 0.9).
                let lin = canopy_color.to_linear();
                let boosted = Color::linear_rgb(
                    (lin.red * 1.8).min(1.0),
                    (lin.green * 1.8).min(1.0),
                    (lin.blue * 1.8).min(1.0),
                );
                let emiss = Color::linear_rgb(lin.red * 0.25, lin.green * 0.25, lin.blue * 0.25);
                materials.add(StandardMaterial {
                    base_color: boosted,
                    emissive: emiss.to_linear(),
                    perceptual_roughness: 0.65,
                    ..default()
                })
            })
            .clone();

        let mut spawned: Vec<Entity> = Vec::with_capacity(target.min(pts.len()));
        for (i, (lx, lz)) in pts.iter().take(target).enumerate() {
            // World pos = origin (coin chunk) + local Poisson.
            let wx = origin.x + lx;
            let wz = origin.z + lz;
            // Échantillon altitude via pipeline V1 (avec offset RPG identique au mesh).
            let h = forgia_terrain::heightmap_at(wx + rpg_offset.x, wz + rpg_offset.z, &terrain_cfg);

            // Skip si sous le sea_level (un poil de marge cosmétique).
            if h < terrain_cfg.sea_level + 0.3 { continue; }

            // Skip si trop proche d'un PathSample (sentier dégagé). Buffer =
            // road half_width + 3m extra pour clairière nette autour du chemin.
            if let Some(ref pn) = path_net {
                let p = Vec2::new(wx, wz);
                let too_close = pn.samples_iter().any(|s| {
                    let buf = s.tier.half_width() + 3.0;
                    p.distance_squared(s.pos) < buf * buf
                });
                if too_close { continue; }
            }

            // Variation de taille déterministe par index.
            let scale = 0.85 + ((i as u32).wrapping_mul(2_654_435_761) as f32 / u32::MAX as f32) * 0.45;

            // Tronc + canopée parent-enfant pour atomic transform.
            let trunk_entity = commands
                .spawn((
                    Mesh3d(trunk_mesh.clone()),
                    MeshMaterial3d(trunk_mat.clone()),
                    Transform::from_xyz(wx, h + 1.25 * scale, wz).with_scale(Vec3::splat(scale)),
                    RigidBody::Fixed,
                    Collider::cylinder(1.25 * scale, 0.18 * scale),
                    VegetationTree,
                    Name::new(format!("Tree_{:?}_{}_{}_{i}", biome, coord.x, coord.z)),
                ))
                .with_children(|p| {
                    p.spawn((
                        Mesh3d(canopy_mesh.clone()),
                        MeshMaterial3d(canopy_mat.clone()),
                        Transform::from_xyz(0.0, 1.6, 0.0),
                    ));
                })
                .id();
            spawned.push(trunk_entity);
        }

        let count = spawned.len();
        veg.total_trees += count;
        *veg.per_biome.entry(biome.as_str()).or_insert(0) += count;
        veg.chunk_entities.insert(*coord, spawned);

        // Attache les arbres comme enfants logiques du chunk pour cleanup auto si
        // la hiérarchie est despawnée (ex. cleanup RpgWorldMarker côté forgia-rpg).
        // ⚠ chunk_entity n'est PAS marqueur de cleanup ici — c'est le marker RPG sur
        // l'entité chunk qui propage le despawn aux enfants Bevy. On utilise donc
        // `add_children` (Bevy 0.18 hierarchy) plutôt qu'un Resource opaque.
        let _ = chunk_entity; // évite warn unused (lifecycle géré côté despawn_unloaded_chunks)
    }
}

/// Quand un chunk passe en LOD2 (>320 m, mega-tile plate), despawn ses arbres.
/// LOD0↔LOD1 : on garde les arbres en place (la densité spawned à LOD1 est déjà
/// clairsemée ×0.2, pas la peine de les re-thinner aux transitions).
fn despawn_far_lod_vegetation(
    mut commands: Commands,
    mut veg: ResMut<VegetationManager>,
    q_far_chunks: Query<(&ChunkCoord, &ChunkLod), Changed<ChunkLod>>,
) {
    for (coord, lod) in &q_far_chunks {
        if !matches!(lod, ChunkLod::Lod2) { continue; }
        if let Some(entities) = veg.chunk_entities.remove(coord) {
            let count = entities.len();
            for e in entities {
                if let Ok(mut ec) = commands.get_entity(e) { ec.despawn(); }
            }
            veg.total_trees = veg.total_trees.saturating_sub(count);
        }
    }
}

/// Quand un `ChunkCoord` disparaît (despawné par `stream_chunks_around_player`),
/// despawn les arbres associés. Détection via RemovedComponents<ChunkCoord>.
fn despawn_unloaded_chunks(
    mut commands: Commands,
    mut veg: ResMut<VegetationManager>,
    mut removed: RemovedComponents<ChunkCoord>,
    chunks_alive: Query<&ChunkCoord>,
) {
    if removed.is_empty() { return; }
    // Rebuild set des chunks vivants → ce qui manque dans veg.chunk_entities est mort.
    let alive: std::collections::HashSet<ChunkCoord> = chunks_alive.iter().copied().collect();
    let to_remove: Vec<ChunkCoord> = veg
        .chunk_entities
        .keys()
        .filter(|c| !alive.contains(c))
        .copied()
        .collect();
    for coord in to_remove {
        if let Some(entities) = veg.chunk_entities.remove(&coord) {
            let count = entities.len();
            for e in entities {
                if let Ok(mut ec) = commands.get_entity(e) {
                    ec.despawn();
                }
            }
            veg.total_trees = veg.total_trees.saturating_sub(count);
        }
    }
    // Drain l'event pour ne pas re-traiter
    removed.clear();
}

/// Hash 32-bit déterministe (chunk.x, chunk.z, seed) → u64 pour Poisson disk.
fn derive_chunk_seed(coord: &ChunkCoord, world_seed: u32) -> u64 {
    let x = u64::from(coord.x as u32);
    let z = u64::from(coord.z as u32);
    let s = u64::from(world_seed);
    let mut h: u64 = s ^ (x.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    h ^= z.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 33;
    h.wrapping_mul(0xFF51_AFD7_ED55_8CCD)
}

/// `RpgSampleOffset` Resource exposée par forgia-rpg pour aligner les samples
/// (heightmap + biome) avec le décalage `(map_size/2, map_size/2)`.
#[derive(Resource, Clone, Copy)]
pub struct RpgSampleOffset {
    pub x: f32,
    pub z: f32,
}

const SENSOR_INTERVAL_S: f32 = 1.0;

fn write_vegetation_sensor(
    time: Res<Time>,
    veg: Res<VegetationManager>,
    mut last_write: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last_write < SENSOR_INTERVAL_S { return; }
    *last_write = now;

    let dist: String = veg
        .per_biome
        .iter()
        .map(|(k, v)| format!("\"{}\":{}", k, v))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"timestamp_secs\":{:.1},\"loaded_chunks\":{},\"total_trees\":{},\"per_biome\":{{{}}}}}",
        now, veg.chunk_entities.len(), veg.total_trees, dist,
    );
    let _ = std::fs::write("forgia_vegetation.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_monotone_jungle_top_volcanic_bottom() {
        assert!(biome_max_per_chunk(BiomeType::Jungle) > biome_max_per_chunk(BiomeType::Forest));
        assert!(biome_max_per_chunk(BiomeType::Forest) > biome_max_per_chunk(BiomeType::Plains));
        assert!(biome_max_per_chunk(BiomeType::Volcanic) <= biome_max_per_chunk(BiomeType::Desert));
    }

    #[test]
    fn chunk_seed_deterministic() {
        let c = ChunkCoord::new(3, -7);
        let s1 = derive_chunk_seed(&c, 1337);
        let s2 = derive_chunk_seed(&c, 1337);
        assert_eq!(s1, s2);
    }

    #[test]
    fn chunk_seed_changes_with_coord() {
        let s1 = derive_chunk_seed(&ChunkCoord::new(0, 0), 1337);
        let s2 = derive_chunk_seed(&ChunkCoord::new(1, 0), 1337);
        assert_ne!(s1, s2);
    }
}
