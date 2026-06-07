//! Industry-standard RPG heightmap-grid mesher (W1 vertical slice).
//!
//! Pattern AAA (Skyrim / Witcher 3 / Horizon / RDR2) :
//! - 1 quad-grid mesh par chunk, vertices alignés sur une grille régulière
//! - Hauteur échantillonnée via `generation::heightmap_at(x, z, &TerrainConfig)`
//! - Vertex colors par biome via `BiomeMap::biome_at` + altitude tint
//! - `Collider::heightfield` rapier3d natif (pas de trimesh CPU-heavy)
//!
//! Pas de Surface Nets, pas de voxel — cavernes/overhangs seront des meshes
//! séparés posés dessus quand le besoin viendra (V2+).

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::biomes::{BiomeMap, BiomeType};
use crate::chunk::{ChunkCoord, TerrainConfig, CHUNK_X, CHUNK_Z};

/// Vertices per chunk axis (33×33 = 32 quads = 1m resolution).
pub const VERTS_PER_AXIS: u32 = CHUNK_X + 1;

/// UV tiles per chunk. 1 tile = 2m monde (CHUNK_X = 32m / 16). Sans tiling,
/// la grass.jpg serait étirée sur 32×32m → motif de stries continues.
/// Le sampler du material doit être en `AddressMode::Repeat` pour matcher.
pub const TEXTURE_TILES_PER_CHUNK: f32 = 4.0;

/// Output : Mesh GPU + heights row-major (nrows = ncols = VERTS_PER_AXIS) pour Collider.
pub struct ChunkMeshData {
    pub mesh: Mesh,
    pub heights: Vec<f32>,
    pub min_y: f32,
    pub max_y: f32,
}

/// Build a quad-grid mesh for one chunk.
///
/// - `coord` : où placer le mesh dans l'espace monde rendu (Transform).
/// - `sample_offset` : décalage **logique** appliqué aux coordonnées passées
///   au pipeline noise. Permet de centrer l'échantillonnage sur `map_size/2`
///   (zone interne sans edge_falloff) même quand le joueur est près de (0,0,0).
pub fn build_chunk_mesh(
    coord: ChunkCoord,
    sample_offset: Vec2,
    config: &TerrainConfig,
    biome_map: &BiomeMap,
    flatten_zones: Option<&crate::flatten::FlattenZones>,
) -> ChunkMeshData {
    let verts_axis = VERTS_PER_AXIS;
    let n_verts = (verts_axis * verts_axis) as usize;
    let origin = coord.world_origin();

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n_verts);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n_verts);
    // `heights` est consommé par `Collider::heightfield` qui attend du **column-major**
    // (parry3d `DMatrix::from_vec(nrows, ncols, heights)` ; row=Z, col=X).
    // On pré-alloue + on écrit à l'index `ix * stride + iz` pour respecter cette
    // convention, sans déranger l'ordre row-major du mesh (indices triangulaires).
    let mut heights: Vec<f32> = vec![0.0; n_verts];
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    // Pass 1 — positions + heights échantillonnés depuis le pipeline noise.
    // Coords mesh locales CENTRÉES sur l'origine du chunk (`lx ∈ [-CHUNK_X/2, +CHUNK_X/2]`),
    // pour que `Collider::heightfield` (centré sur Transform) corresponde au mesh visible.
    let half_x = CHUNK_X as f32 * 0.5;
    let half_z = CHUNK_Z as f32 * 0.5;
    let stride = verts_axis as usize;
    for iz in 0..verts_axis {
        for ix in 0..verts_axis {
            let lx = ix as f32 - half_x; // -CHUNK_X/2 .. +CHUNK_X/2
            let lz = iz as f32 - half_z;
            // World coord = chunk center + local + (sample shift vers intérieur monde)
            let wx = origin.x + half_x + lx;
            let wz = origin.z + half_z + lz;
            // heightmap_at est biome-aware via le global WORLD_BIOME_MAP →
            // cohérent avec foliage/village/spawn (mêmes hauteurs biome-aware).
            let raw_h =
                crate::generation::heightmap_at(wx + sample_offset.x, wz + sample_offset.y, config);
            // Story-447 — apply village flatten zones post-process. World-space XZ
            // = chunk origin + local (not offset by sample_offset, since FlattenZones
            // are inserted in world-RPG-space matching village_world_center).
            let h = match flatten_zones {
                Some(z) => z.sample(wx, wz, raw_h),
                None => raw_h,
            };
            positions.push([lx, h, lz]);
            uvs.push([
                (ix as f32) / CHUNK_X as f32 * TEXTURE_TILES_PER_CHUNK,
                (iz as f32) / CHUNK_Z as f32 * TEXTURE_TILES_PER_CHUNK,
            ]);
            // Column-major : col=ix (X), row=iz (Z) → index = row + col*nrows.
            heights[(ix as usize) * stride + (iz as usize)] = h;
            min_y = min_y.min(h);
            max_y = max_y.max(h);
        }
    }

    // Indices (quad-pair tri-list, CCW front-facing up).
    let stride = verts_axis;
    let n_quads = CHUNK_X * CHUNK_Z;
    let mut indices: Vec<u32> = Vec::with_capacity((n_quads * 6) as usize);
    for iz in 0..CHUNK_Z {
        for ix in 0..CHUNK_X {
            let i = iz * stride + ix;
            let i_right = i + 1;
            let i_down = i + stride;
            let i_diag = i_down + 1;
            indices.extend_from_slice(&[i, i_down, i_right, i_right, i_down, i_diag]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();

    // Pass 2 — vertex colors : biome base color × altitude/slope tint.
    let normals = mesh
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .and_then(|a| a.as_float3())
        .map(|n| n.to_vec())
        .unwrap_or_default();
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(n_verts);
    for (i, pos) in positions.iter().enumerate() {
        let wx = origin.x + half_x + pos[0] + sample_offset.x;
        let wz = origin.z + half_z + pos[2] + sample_offset.y;
        let slope = if i < normals.len() {
            (1.0 - normals[i][1].clamp(0.0, 1.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // Story-577 polish : couleur blendée par biomes voisins (fin du seam Voronoi).
        colors.push(blended_vertex_color(biome_map, wx, wz, pos[1], slope, config));
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

    ChunkMeshData {
        mesh,
        heights,
        min_y,
        max_y,
    }
}

/// Couleur biome avec teinte roche (pente) + neige (altitude). `pub(crate)` :
/// réutilisé par `lod.rs::build_lod2_terrain_mesh` pour que le LOD2 lointain ait
/// EXACTEMENT le même rendu que le LOD0 proche (audit 2026-06-05).
pub(crate) fn blend_biome_color(
    biome: BiomeType,
    h: f32,
    slope: f32,
    config: &TerrainConfig,
) -> [f32; 4] {
    let base = biome.linear_rgba();
    // Mix vers la roche grise sur les pentes raides (cliffs naturels).
    let rock = [0.45, 0.42, 0.38, 1.0];
    let rock_t = (slope * 1.6).clamp(0.0, 1.0);
    // Mix vers blanc en altitude (neige sommets).
    let snow_start = config.max_height * 0.75;
    let snow_end = config.max_height * 0.95;
    let snow_t = ((h - snow_start) / (snow_end - snow_start)).clamp(0.0, 1.0);
    let snow = [0.92, 0.94, 0.97, 1.0];

    let lerp = |a: [f32; 4], b: [f32; 4], t: f32| -> [f32; 4] {
        [
            a[0] * (1.0 - t) + b[0] * t,
            a[1] * (1.0 - t) + b[1] * t,
            a[2] * (1.0 - t) + b[2] * t,
            1.0,
        ]
    };
    let with_rock = lerp(base, rock, rock_t);
    lerp(with_rock, snow, snow_t)
}

/// Couleur de sol BLENDÉE par poids des biomes voisins (smoothstep, même infra
/// `biome_weights_at` que le relief) → transition de teinte douce au lieu du seam
/// Voronoi dur (`biome_at`) sur un relief pourtant lissé. Story-577 polish.
/// `wx`/`wz` = coords d'échantillonnage (avec sample_offset, comme `biome_at`).
/// Coût ≈ inchangé : on remplace un `biome_at` O(seeds) par un `biome_weights_at`
/// O(seeds). `h`/`slope` identiques pour chaque biome au vertex (tint altitude/pente).
pub(crate) fn blended_vertex_color(
    biome_map: &BiomeMap,
    wx: f32,
    wz: f32,
    h: f32,
    slope: f32,
    config: &TerrainConfig,
) -> [f32; 4] {
    let blend = biome_map.biome_weights_at(
        wx,
        wz,
        crate::biomes::MAX_BLEND_BIOMES,
        crate::biomes::BIOME_COLOR_BLEND_RADIUS_M,
    );
    if blend.count == 0 {
        return blend_biome_color(biome_map.biome_at(wx, wz), h, slope, config);
    }
    let mut col = [0.0f32; 4];
    for k in 0..blend.count {
        let (biome, w) = blend.biomes[k];
        let c = blend_biome_color(biome, h, slope, config);
        col[0] += c[0] * w;
        col[1] += c[1] * w;
        col[2] += c[2] * w;
        col[3] += c[3] * w;
    }
    col[3] = 1.0;
    col
}

/// Spawn the chunk entity : Mesh + Material + Collider::heightfield + ChunkCoord.
///
/// Le mesh et le collider sont **centrés** sur l'entité → on positionne le
/// Transform sur le centre du chunk (`world_origin + (CHUNK_X/2, 0, CHUNK_Z/2)`)
/// pour que collider et visuel coïncident exactement.
pub fn spawn_chunk_entity(
    commands: &mut Commands,
    coord: ChunkCoord,
    mesh_data: ChunkMeshData,
    mesh_handle: Handle<Mesh>,
    material: Handle<StandardMaterial>,
) -> Entity {
    let origin = coord.world_origin();
    let center_x = origin.x + CHUNK_X as f32 * 0.5;
    let center_z = origin.z + CHUNK_Z as f32 * 0.5;
    let nrows = VERTS_PER_AXIS as usize;
    let ncols = nrows;
    let extent_x = CHUNK_X as f32;
    let extent_z = CHUNK_Z as f32;

    commands
        .spawn((
            coord,
            Mesh3d(mesh_handle),
            MeshMaterial3d(material),
            Transform::from_xyz(center_x, 0.0, center_z),
            RigidBody::Fixed,
            Collider::heightfield(
                mesh_data.heights,
                nrows,
                ncols,
                Vec3::new(extent_x, 1.0, extent_z),
            ),
            Name::new(format!("TerrainChunk({},{})", coord.x, coord.z)),
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_has_expected_vertex_count() {
        let cfg = TerrainConfig::default();
        let bm = BiomeMap { seeds: vec![] };
        let data = build_chunk_mesh(ChunkCoord::new(0, 0), Vec2::ZERO, &cfg, &bm, None);
        let expected = (VERTS_PER_AXIS * VERTS_PER_AXIS) as usize;
        assert_eq!(data.heights.len(), expected);
        let positions = data.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(positions.len(), expected);
    }

    #[test]
    fn min_max_consistent_with_heights() {
        let cfg = TerrainConfig::default();
        let bm = BiomeMap { seeds: vec![] };
        let data = build_chunk_mesh(ChunkCoord::new(1, 1), Vec2::ZERO, &cfg, &bm, None);
        assert!(data.min_y <= data.max_y);
        let observed_min = data.heights.iter().cloned().fold(f32::INFINITY, f32::min);
        let observed_max = data
            .heights
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(data.min_y, observed_min);
        assert_eq!(data.max_y, observed_max);
    }

    /// Régression : pour chaque vertex (ix, iz), le `position.y` du mesh doit
    /// être strictement égal à la valeur `heights[ix*stride + iz]` (column-major)
    /// que `Collider::heightfield` consommera. Garantit que mesh visible et
    /// surface de collision rapier coïncident à 0 ulp près.
    #[test]
    fn heights_column_major_matches_mesh_y() {
        let cfg = TerrainConfig::default();
        let bm = BiomeMap { seeds: vec![] };
        let data = build_chunk_mesh(ChunkCoord::new(2, -3), Vec2::splat(1024.0), &cfg, &bm, None);

        let stride = VERTS_PER_AXIS as usize;
        let positions = data
            .mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|a| a.as_float3())
            .expect("mesh has positions");

        for iz in 0..stride {
            for ix in 0..stride {
                // Mesh row-major : positions[iz * stride + ix].y
                let mesh_y = positions[iz * stride + ix][1];
                // Collider column-major : heights[ix * stride + iz]
                let collider_y = data.heights[ix * stride + iz];
                assert_eq!(
                    mesh_y, collider_y,
                    "Mesh / collider mismatch at (ix={}, iz={})",
                    ix, iz,
                );
            }
        }
    }
}
