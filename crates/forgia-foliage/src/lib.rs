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
use forgia_asset_registry::{
    target_size_for, AssetCategory, AssetQuery, AssetRegistry, AssetSeason, NeedsAssetCalibrate,
};
use forgia_core::prelude::*;
use forgia_streaming::FoliageCoverageReport;
use forgia_terrain::{
    sampling::poisson_disk_sample, BiomeMap, BiomeType, ChunkCoord, ChunkLod, PathNetwork,
    TerrainConfig, CHUNK_X, CHUNK_Z,
};
use std::collections::HashMap;

pub mod material_override;
pub use material_override::{
    BarkOverrideConfig, BarkTextures, FoliageFallbackDiagnostic, NeedsTrunkOverride,
};

pub mod prelude {
    pub use crate::{
        biome_max_per_chunk, BarkOverrideConfig, BarkTextures, ForgiaFoliagePlugin,
        NeedsTrunkOverride, VegetationManager,
    };
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

/// Couleur trunk + canopy approximative par biome (legacy procedural variants —
/// gardé en cas de fallback si AssetRegistry vide).
#[allow(dead_code)]
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

/// 3 variants d'arbre : standard / tall_thin (pin) / wide_low (broussailleux).
/// `TreeVariant::PARAMS` donne (trunk_radius, trunk_h, canopy_radius, canopy_y).
#[derive(Clone, Copy, Debug)]
pub enum TreeVariant {
    Standard = 0,
    TallThin = 1,
    WideLow = 2,
}

impl TreeVariant {
    pub const COUNT: usize = 3;
    /// (trunk_radius, trunk_half_height, canopy_radius, canopy_local_y)
    pub fn params(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Standard => (0.15, 1.25, 1.4, 1.6),
            Self::TallThin => (0.12, 2.0, 1.0, 2.3), // pin élancé
            Self::WideLow => (0.22, 0.9, 1.8, 1.1),  // touffu bas
        }
    }
    pub fn from_index(i: usize) -> Self {
        match i % Self::COUNT {
            0 => Self::Standard,
            1 => Self::TallThin,
            _ => Self::WideLow,
        }
    }
}

/// Excluded disc (XZ world coords) where no foliage may spawn. Inserted by
/// gameplay caller (e.g. `forgia-rpg` when a village is generated). Removed
/// at cleanup OnExit. Buffer is added to the genome bounding radius for
/// a clean clearing margin around the village edge.
///
/// Pattern : exclusion zones décrit dans `docs/audit-procgen-village-2026-05-17.md` §7.
#[derive(Resource, Clone, Debug)]
pub struct FoliageExclusionDisc {
    /// World XZ center of the disc.
    pub center: Vec2,
    /// Radius in meters. Foliage skipped when `pos.distance(center) < radius`.
    pub radius: f32,
}

#[derive(Resource, Default)]
pub struct VegetationManager {
    /// Tracking pour despawn ciblé quand un chunk est déchargé.
    pub chunk_entities: HashMap<ChunkCoord, Vec<Entity>>,
    /// Total cumulé pour sensor.
    pub total_trees: usize,
    /// Distribution par biome (sensor).
    pub per_biome: HashMap<&'static str, usize>,
    /// Caches mesh procéduraux par variant (3 troncs + 3 canopées, shared).
    pub trunk_meshes: [Option<Handle<Mesh>>; TreeVariant::COUNT],
    pub canopy_meshes: [Option<Handle<Mesh>>; TreeVariant::COUNT],
    /// Caches material par biome (1 tronc + 1 canopée / biome).
    pub trunk_mats: HashMap<u8, Handle<StandardMaterial>>,
    pub canopy_mats: HashMap<u8, Handle<StandardMaterial>>,
}

pub struct ForgiaFoliagePlugin;

impl Plugin for ForgiaFoliagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VegetationManager>()
            .init_resource::<BarkOverrideConfig>()
            .init_resource::<FoliageFallbackDiagnostic>()
            .add_systems(
                Startup,
                (init_proc_meshes, material_override::preload_bark_textures),
            )
            .add_systems(
                Update,
                (
                    populate_new_chunks,
                    material_override::apply_trunk_bark_override,
                    material_override::write_foliage_fallback_sensor,
                    despawn_far_lod_vegetation,
                    despawn_unloaded_chunks,
                    write_vegetation_sensor,
                    write_foliage_coverage_report,
                )
                    .chain()
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Rpg)),
            );
    }
}

fn init_proc_meshes(mut veg: ResMut<VegetationManager>, mut meshes: ResMut<Assets<Mesh>>) {
    for i in 0..TreeVariant::COUNT {
        let (tr, th, cr, _cy) = TreeVariant::from_index(i).params();
        veg.trunk_meshes[i] = Some(meshes.add(Cylinder::new(tr, th * 2.0)));
        veg.canopy_meshes[i] = Some(meshes.add(Sphere::new(cr)));
    }
}

/// Pour chaque chunk présent qui n'a pas encore reçu de vegetation, échantillonne
/// N positions par Poisson disk et spawn arbres procéduraux. On NE dépend PAS de
/// `Added<ChunkCoord>` (timing fragile entre systèmes streamer/foliage dans des
/// plugins différents) — on filtre par contains_key sur le tracking interne.
///
/// Si trunk/canopy meshes pas encore initialisés (cleanup OnExit a remplacé la
/// Resource par défaut), on les ré-initialise lazily.
#[allow(clippy::too_many_arguments)]
fn populate_new_chunks(
    mut commands: Commands,
    mut veg: ResMut<VegetationManager>,
    asset_server: Res<AssetServer>,
    registry: Res<AssetRegistry>,
    cfg: Res<BarkOverrideConfig>,
    biome_map: Option<Res<BiomeMap>>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    rpg_offset: Option<Res<RpgSampleOffset>>,
    path_net: Option<Res<PathNetwork>>,
    excl_disc: Option<Res<FoliageExclusionDisc>>,
    flatten_zones: Option<Res<forgia_terrain::FlattenZones>>,
    q_chunks: Query<(Entity, &ChunkCoord, Option<&ChunkLod>)>,
) {
    let (Some(biome_map), Some(terrain_cfg), Some(rpg_offset)) =
        (biome_map, terrain_cfg, rpg_offset)
    else {
        return;
    };

    // Skip si le registry n'a pas encore scanné (1ère frame avant Startup).
    if registry.is_empty() {
        return;
    }

    for (chunk_entity, coord, lod) in &q_chunks {
        if veg.chunk_entities.contains_key(coord) {
            continue;
        }
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

        let pts = poisson_disk_sample(CHUNK_X as f32, CHUNK_Z as f32, spacing, seed, 30);

        // Query AssetRegistry pour arbres compatibles avec ce biome.
        // Bonus visuel : pour Forest/Plains, on inclut les arbres rouges autumn
        // pour donner une variation visuelle marquée (env. 30% des picks).
        let trees_all: Vec<&forgia_asset_registry::AssetEntry> = registry.query(
            &AssetQuery::new()
                .category(AssetCategory::Tree)
                .biome(biome)
                .alive(),
        );
        let trees_autumn: Vec<&forgia_asset_registry::AssetEntry> = registry.query(
            &AssetQuery::new()
                .category(AssetCategory::Tree)
                .biome(biome)
                .season(AssetSeason::Autumn)
                .alive(),
        );
        if trees_all.is_empty() {
            continue;
        }

        let mut spawned: Vec<Entity> = Vec::with_capacity(target.min(pts.len()));
        for (i, (lx, lz)) in pts.iter().take(target).enumerate() {
            // World pos = origin (coin chunk) + local Poisson.
            let wx = origin.x + lx;
            let wz = origin.z + lz;
            // Échantillon altitude via pipeline V1 (avec offset RPG identique au mesh).
            // Story-447 fix lévitation : foliage Y doit utiliser FlattenZones aussi,
            // sinon les arbres en bordure village (zone falloff) flottent au-dessus
            // du mesh leveled. Pure post-process raw → flatten-sampled.
            let raw_h =
                forgia_terrain::heightmap_at(wx + rpg_offset.x, wz + rpg_offset.z, &terrain_cfg);
            let h = match flatten_zones.as_deref() {
                Some(zones) => zones.sample(wx, wz, raw_h),
                None => raw_h,
            };

            // Skip si sous le sea_level (un poil de marge cosmétique).
            if h < terrain_cfg.sea_level + 0.3 {
                continue;
            }

            // Skip si trop proche d'un PathSample (sentier dégagé). Buffer =
            // road half_width + 4m extra pour clairière nette autour du chemin
            // (user feedback 2026-05-17 PM : "jamais d'asset sur les routes").
            if let Some(ref pn) = path_net {
                let p = Vec2::new(wx, wz);
                let too_close = pn.samples_iter().any(|s| {
                    let buf = s.tier.half_width() + 4.0;
                    p.distance_squared(s.pos) < buf * buf
                });
                if too_close {
                    continue;
                }
            }

            // Skip si à l'intérieur du disque d'exclusion village (gameplay
            // caller insère `FoliageExclusionDisc` quand un village spawne).
            if let Some(ref ed) = excl_disc {
                let p = Vec2::new(wx, wz);
                if p.distance_squared(ed.center) < ed.radius * ed.radius {
                    continue;
                }
            }

            // Hash déterministe (i + chunk_coord) — pool autumn ou pool default.
            let hash = (i as u32)
                .wrapping_add((coord.x as u32).wrapping_mul(31))
                .wrapping_add((coord.z as u32).wrapping_mul(17));
            // 30% chance autumn si dispo (Forest/Plains/Jungle).
            let use_autumn = !trees_autumn.is_empty() && (hash % 100) < 30;
            let pool: &Vec<&forgia_asset_registry::AssetEntry> = if use_autumn {
                &trees_autumn
            } else {
                &trees_all
            };
            let entry = pool[(hash as usize) % pool.len()];

            // Variation visuelle utilisateur (jitter 0.85-1.15) appliqué EN PLUS
            // du scale de calibration auto-mesuré.
            let user_scale =
                0.85 + ((hash.wrapping_mul(2_654_435_761) as f32) / u32::MAX as f32) * 0.3;
            let yaw =
                (hash.wrapping_mul(0x9E37_79B1) as f32 / u32::MAX as f32) * std::f32::consts::TAU;

            let scene_handle: Handle<Scene> = asset_server
                .load(bevy::asset::AssetPath::from(entry.path.clone()).with_label("Scene0"));

            // 2 cas selon que le GLB a déjà été mesuré :
            //   - measured connue → scale immédiat = target / measured × user_scale
            //   - measured None    → spawn avec scale=1.0 + NeedsAssetCalibrate marker.
            //     Le système calibrate_assets ajustera + écrira la mesure dans le TOML.
            let target = target_size_for(entry.category);
            let (initial_scale, needs_calib) = match entry.measured_size_m {
                Some(m) if m > 0.0 => (target / m * user_scale, None),
                _ => (
                    user_scale,
                    Some(NeedsAssetCalibrate {
                        entry_path: entry.path.clone(),
                        target_size_m: target,
                        user_scale,
                    }),
                ),
            };

            let mut ec = commands.spawn((
                SceneRoot(scene_handle),
                Transform {
                    translation: Vec3::new(wx, h, wz),
                    rotation: Quat::from_rotation_y(yaw),
                    scale: Vec3::splat(initial_scale),
                },
                RigidBody::Fixed,
                // Collider générique trunk-like (~3m × 0.3m). Une amélioration
                // future : générer le Collider depuis l'AABB après calibration.
                Collider::cylinder(1.5, 0.3),
                VegetationTree,
                Name::new(format!(
                    "Tree_{}_{}_{}_{i}",
                    entry.species, coord.x, coord.z
                )),
            ));
            if let Some(n) = needs_calib {
                ec.insert(n);
            }
            // Si l'override bark est actif, marquer cet arbre pour traitement trunk.
            if cfg.enabled && entry.category == AssetCategory::Tree {
                ec.insert(NeedsTrunkOverride::default());
            }
            let trunk_entity = ec.id();
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
        if !matches!(lod, ChunkLod::Lod2) {
            continue;
        }
        if let Some(entities) = veg.chunk_entities.remove(coord) {
            let count = entities.len();
            for e in entities {
                // story-450 wave 2.5 : try_despawn (Bevy 0.18) idempotent au
                // flush — pas d'erreur "entity invalid" si command queue
                // contient déjà un despawn de la même entité (concurrent path
                // via despawn_unloaded_chunks ou recursive cascade chunk).
                if let Ok(mut ec) = commands.get_entity(e) {
                    ec.try_despawn();
                }
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
    if removed.is_empty() {
        return;
    }
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
                // story-450 wave 2.5 : try_despawn idempotent — voir commentaire
                // dans despawn_far_lod_vegetation. Race possible quand
                // streaming.toml view_m réduit drastiquement le ring loaded
                // → mass eviction concurrent (38 warns observés t≈09:58:04-23).
                if let Ok(mut ec) = commands.get_entity(e) {
                    ec.try_despawn();
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
    bark_cfg: Option<Res<BarkOverrideConfig>>,
    mut last_write: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last_write < SENSOR_INTERVAL_S {
        return;
    }
    *last_write = now;

    let dist: String = veg
        .per_biome
        .iter()
        .map(|(k, v)| format!("\"{}\":{}", k, v))
        .collect::<Vec<_>>()
        .join(",");
    // story-486 : extension trunk_override stats si BarkOverrideConfig présente.
    let trunk_json = match bark_cfg.as_deref() {
        Some(cfg) => format!(
            ",\"trunk_override\":{{\"enabled\":{},\"overridden\":{},\"fallback\":{},\"not_found\":{}}}",
            cfg.enabled, cfg.overridden, cfg.fallback, cfg.not_found,
        ),
        None => String::new(),
    };
    let json = format!(
        "{{\"timestamp_secs\":{:.1},\"loaded_chunks\":{},\"total_trees\":{},\"per_biome\":{{{}}}{}}}",
        now, veg.chunk_entities.len(), veg.total_trees, dist, trunk_json,
    );
    let _ = std::fs::write("forgia_vegetation.json", json);
}

/// Story-502-B : producteur du `FoliageCoverageReport` côté foliage.
/// Update toutes les frames (sensor côté streaming throttle à 1Hz par défaut).
/// `chunks_loaded` = nombre de chunks éligibles à recevoir foliage (LOD0+LOD1 ;
/// LOD2 exclu car mega-tile plate, populate_new_chunks `continue` dessus).
/// `chunks_with_veg` = nb chunks effectivement passés dans populate (clé dans
/// `veg.chunk_entities`). Le delta = chunks "loaded sans veg" → couvert par
/// le sensor severity du crate streaming.
fn write_foliage_coverage_report(
    veg: Res<VegetationManager>,
    q_chunks: Query<(&ChunkCoord, Option<&ChunkLod>)>,
    mut report: ResMut<FoliageCoverageReport>,
) {
    let mut eligible = 0u32;
    for (_, lod) in &q_chunks {
        let l = lod.copied().unwrap_or(ChunkLod::Lod0);
        if !matches!(l, ChunkLod::Lod2) {
            eligible = eligible.saturating_add(1);
        }
    }
    report.chunks_loaded = eligible;
    report.chunks_with_veg = veg.chunk_entities.len() as u32;
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
