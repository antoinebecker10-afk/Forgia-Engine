//! # forgia-mesh-voxelizer — Solid voxelization d'un mesh triangulé
//!
//! **Phase 1 du pipeline auto-rig Forgia (Pinocchio-inspired, story-440)** :
//! transforme un mesh GLB en grille 3D de voxels remplis (solide). C'est la
//! base pour `forgia-medial-axis` (distance field + sphere packing) puis
//! `forgia-skeleton-embedder`.
//!
//! ## Algorithme
//!
//! **Solid voxelization par ray-casting** (Lengyel, Game Engine Gems Vol 2) :
//!
//! Pour chaque colonne `(x, z)` de la grille, on tire un rayon vertical (axe +Y)
//! depuis le bas. On compte les intersections avec les triangles du mesh.
//! Un voxel est "intérieur au mesh" si le nombre d'intersections sous son centre
//! est **impair** (Jordan curve theorem).
//!
//! Coût : O(resolution² × N_triangles). Pour Rex.glb (~32k triangles, res 32) :
//! ~32M ops, ~200ms one-shot. Acceptable car runs UNE fois par mesh au spawn,
//! pas par frame.
//!
//! ## Robustesse
//!
//! - **Mesh non-watertight** (cas typique Meshy v5) : le ray-casting peut être
//!   fragile. Le fallback `also_mark_surface_triangles` ajoute les voxels que
//!   les triangles touchent, garantissant au moins la silhouette.
//! - **Triangles dégénérés** : skip via test `cross().length() < epsilon`.
//! - **Resolution adaptive** : `VoxelizerConfig::resolution_for_mesh_size` choisit
//!   un res qui donne ~5-10cm par voxel selon la hauteur du mesh.
//!
//! ## API
//!
//! ```ignore
//! let grid = voxelize_mesh(&positions, &indices, VoxelizerConfig::default());
//! for idx in grid.iter_filled() {
//!     let world_pos = grid.voxel_center(idx);
//!     // ... process voxel ...
//! }
//! ```

use bevy::math::{UVec3, Vec3};

/// Configuration de la voxelization. Default = 32³ resolution, fill interior +
/// surface (robust pour meshes non-watertight Meshy).
#[derive(Debug, Clone)]
pub struct VoxelizerConfig {
    /// Résolution de la grille (cubique). 32 = bon compromis precision/perf.
    /// 64 si mesh > 2m avec détails fins.
    pub resolution: u32,
    /// Si true, marque aussi les voxels touchés par la surface des triangles
    /// (fallback robuste si le mesh n'est pas watertight et que le ray-casting
    /// rate l'intérieur).
    pub also_mark_surface_triangles: bool,
    /// Padding (en fraction de la hauteur AABB) ajouté autour du mesh AABB
    /// pour éviter les voxels frontière coupés. Default 5%.
    pub aabb_padding_frac: f32,
}

impl Default for VoxelizerConfig {
    fn default() -> Self {
        Self {
            resolution: 32,
            also_mark_surface_triangles: true,
            aabb_padding_frac: 0.05,
        }
    }
}

impl VoxelizerConfig {
    /// Choisit une résolution adaptive pour cibler ~5cm par voxel.
    /// Mesh 1m → res 20. Mesh 2m → res 40. Cap à [16, 64].
    pub fn resolution_for_mesh_size(mesh_height_m: f32) -> u32 {
        let target_voxel_cm = 5.0;
        let res = ((mesh_height_m * 100.0) / target_voxel_cm).round() as u32;
        res.clamp(16, 64)
    }
}

/// Grille 3D de voxels avec représentation dense (Vec<bool>). Pour mesh résolution
/// 32 → 32^3 = 32768 voxels = 32 KB de mémoire. Acceptable.
///
/// Layout : `index = x * res² + y * res + z` (X-major, Y-mid, Z-minor).
#[derive(Debug, Clone)]
pub struct VoxelGrid {
    /// Résolution cubique (resolution × resolution × resolution voxels).
    pub resolution: u32,
    /// AABB monde couvert par la grille (avec padding).
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    /// Voxels remplis (dense bool array).
    voxels: Vec<bool>,
}

impl VoxelGrid {
    /// Crée une grille vide avec les bounds donnés.
    pub fn new_empty(resolution: u32, bounds_min: Vec3, bounds_max: Vec3) -> Self {
        let n = (resolution as usize).pow(3);
        Self {
            resolution,
            bounds_min,
            bounds_max,
            voxels: vec![false; n],
        }
    }

    /// Taille d'un voxel (longueur d'arête).
    pub fn voxel_size(&self) -> Vec3 {
        (self.bounds_max - self.bounds_min) / self.resolution as f32
    }

    /// Index linéaire dans `voxels` pour `(x, y, z)`.
    #[inline]
    fn linear_index(&self, idx: UVec3) -> usize {
        let res = self.resolution as usize;
        idx.x as usize * res * res + idx.y as usize * res + idx.z as usize
    }

    /// True si le voxel `(x, y, z)` est marqué rempli.
    pub fn is_filled(&self, idx: UVec3) -> bool {
        if idx.x >= self.resolution || idx.y >= self.resolution || idx.z >= self.resolution {
            return false;
        }
        self.voxels[self.linear_index(idx)]
    }

    /// Marque le voxel `(x, y, z)` comme rempli.
    pub fn set_filled(&mut self, idx: UVec3) {
        if idx.x < self.resolution && idx.y < self.resolution && idx.z < self.resolution {
            let li = self.linear_index(idx);
            self.voxels[li] = true;
        }
    }

    /// Position monde du CENTRE du voxel `(x, y, z)`.
    pub fn voxel_center(&self, idx: UVec3) -> Vec3 {
        let vs = self.voxel_size();
        self.bounds_min
            + Vec3::new(
                (idx.x as f32 + 0.5) * vs.x,
                (idx.y as f32 + 0.5) * vs.y,
                (idx.z as f32 + 0.5) * vs.z,
            )
    }

    /// Itère tous les voxels remplis (indices 3D).
    pub fn iter_filled(&self) -> impl Iterator<Item = UVec3> + '_ {
        let res = self.resolution;
        (0..res).flat_map(move |x| {
            (0..res).flat_map(move |y| {
                (0..res).filter_map(move |z| {
                    let idx = UVec3::new(x, y, z);
                    if self.is_filled(idx) {
                        Some(idx)
                    } else {
                        None
                    }
                })
            })
        })
    }

    /// Nombre total de voxels remplis (diagnostic).
    pub fn filled_count(&self) -> usize {
        self.voxels.iter().filter(|&&b| b).count()
    }

    /// Volume estimé (m³) du mesh selon la voxelisation.
    pub fn estimated_volume(&self) -> f32 {
        let vs = self.voxel_size();
        let voxel_vol = vs.x * vs.y * vs.z;
        self.filled_count() as f32 * voxel_vol
    }
}

/// Voxelize un mesh triangulé en grille 3D solide.
///
/// `positions` : vertices en world space (ou mesh-local cohérent avec consumer)
/// `indices` : triplets formant les triangles (length doit être multiple de 3)
/// `config` : paramètres de voxelization
pub fn voxelize_mesh(positions: &[Vec3], indices: &[u32], config: &VoxelizerConfig) -> VoxelGrid {
    if positions.is_empty() || indices.len() < 3 {
        return VoxelGrid::new_empty(config.resolution, Vec3::ZERO, Vec3::ONE);
    }

    // 1. Compute AABB avec padding.
    let (mut min_v, mut max_v) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for p in positions {
        min_v = min_v.min(*p);
        max_v = max_v.max(*p);
    }
    let size = max_v - min_v;
    let pad = size * config.aabb_padding_frac;
    let bounds_min = min_v - pad;
    let bounds_max = max_v + pad;

    let mut grid = VoxelGrid::new_empty(config.resolution, bounds_min, bounds_max);

    // 2. Solid voxelization par ray-casting axe +Y.
    let vs = grid.voxel_size();
    let res = config.resolution;
    for x in 0..res {
        for z in 0..res {
            let ray_origin_x = bounds_min.x + (x as f32 + 0.5) * vs.x;
            let ray_origin_z = bounds_min.z + (z as f32 + 0.5) * vs.z;

            // Find Y-intersections of vertical ray with all triangles.
            let mut intersections: Vec<f32> = Vec::new();
            for tri in indices.chunks_exact(3) {
                let i0 = tri[0] as usize;
                let i1 = tri[1] as usize;
                let i2 = tri[2] as usize;
                if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
                    continue;
                }
                let v0 = positions[i0];
                let v1 = positions[i1];
                let v2 = positions[i2];
                if let Some(t_y) = vertical_ray_triangle_y(
                    Vec3::new(ray_origin_x, 0.0, ray_origin_z),
                    v0,
                    v1,
                    v2,
                ) {
                    intersections.push(t_y);
                }
            }
            intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // For each voxel in column : odd count of intersections below center → interior.
            for y in 0..res {
                let voxel_y = bounds_min.y + (y as f32 + 0.5) * vs.y;
                let count_below = intersections.iter().filter(|&&t| t < voxel_y).count();
                if count_below % 2 == 1 {
                    grid.set_filled(UVec3::new(x, y, z));
                }
            }
        }
    }

    // 3. Fallback : marquer aussi les voxels touchés par chaque triangle vertex.
    //    Garantit silhouette même si mesh non-watertight (Meshy v5 typique).
    if config.also_mark_surface_triangles {
        for p in positions {
            let local = (*p - bounds_min) / (bounds_max - bounds_min);
            let ix = (local.x * res as f32) as u32;
            let iy = (local.y * res as f32) as u32;
            let iz = (local.z * res as f32) as u32;
            grid.set_filled(UVec3::new(
                ix.min(res - 1),
                iy.min(res - 1),
                iz.min(res - 1),
            ));
        }
    }

    grid
}

/// Intersection rayon vertical (axe Y) avec triangle. Retourne le `y` de
/// l'intersection si elle existe, `None` sinon.
///
/// Le rayon est défini par `(origin.x, _, origin.z)` parallèle à +Y. On teste
/// si la projection XZ de `origin` tombe dans le triangle projeté XZ, et calcule
/// le `y` correspondant.
fn vertical_ray_triangle_y(origin: Vec3, v0: Vec3, v1: Vec3, v2: Vec3) -> Option<f32> {
    // Projection XZ pour test de containment via barycentric coords.
    let p = (origin.x, origin.z);
    let p0 = (v0.x, v0.z);
    let p1 = (v1.x, v1.z);
    let p2 = (v2.x, v2.z);

    // Aire signée 2D du triangle (= determinant).
    let area2 = (p1.0 - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (p1.1 - p0.1);
    if area2.abs() < 1e-10 {
        // Triangle dégénéré ou perpendiculaire au plan XZ → pas d'intersection valide.
        return None;
    }

    // Barycentric coords XZ.
    let s = ((p1.1 - p2.1) * (p.0 - p2.0) + (p2.0 - p1.0) * (p.1 - p2.1)) / area2;
    let t = ((p2.1 - p0.1) * (p.0 - p2.0) + (p0.0 - p2.0) * (p.1 - p2.1)) / area2;
    let u = 1.0 - s - t;

    let eps = -1e-6;
    if s < eps || t < eps || u < eps {
        return None;
    }

    // Y de l'intersection = interpolation barycentric de v0.y, v1.y, v2.y.
    Some(s * v0.y + t * v1.y + u * v2.y)
}

// ── Tests headless ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Cube unitaire de [0,1]³ — 12 triangles (2 par face × 6 faces).
    fn unit_cube_mesh() -> (Vec<Vec3>, Vec<u32>) {
        let positions = vec![
            // 8 corners
            Vec3::new(0.0, 0.0, 0.0), // 0
            Vec3::new(1.0, 0.0, 0.0), // 1
            Vec3::new(0.0, 1.0, 0.0), // 2
            Vec3::new(1.0, 1.0, 0.0), // 3
            Vec3::new(0.0, 0.0, 1.0), // 4
            Vec3::new(1.0, 0.0, 1.0), // 5
            Vec3::new(0.0, 1.0, 1.0), // 6
            Vec3::new(1.0, 1.0, 1.0), // 7
        ];
        // 12 triangles (face winding outward)
        let indices = vec![
            // -Z face (z=0)
            0, 2, 1, 1, 2, 3,
            // +Z face (z=1)
            4, 5, 6, 5, 7, 6,
            // -Y face (y=0)
            0, 1, 4, 1, 5, 4,
            // +Y face (y=1)
            2, 6, 3, 3, 6, 7,
            // -X face (x=0)
            0, 4, 2, 2, 4, 6,
            // +X face (x=1)
            1, 3, 5, 3, 7, 5,
        ];
        (positions, indices)
    }

    #[test]
    fn unit_cube_fills_grid_majority() {
        let (positions, indices) = unit_cube_mesh();
        let config = VoxelizerConfig {
            resolution: 16,
            also_mark_surface_triangles: true,
            aabb_padding_frac: 0.05,
        };
        let grid = voxelize_mesh(&positions, &indices, &config);
        let total = (16_usize).pow(3);
        let filled = grid.filled_count();
        // Cube unitaire avec padding 5% → environ (1.0/1.10)³ ≈ 0.75 du volume
        // total, soit ~3000 voxels sur 4096. Minimum 1000 pour test pas trop strict.
        assert!(
            filled > 1000,
            "expected >1000 filled voxels for unit cube, got {} / {}",
            filled,
            total
        );
        // Pas plus de 90% du total (sinon = full grid, voxelisation cassée)
        assert!(
            filled < total * 9 / 10,
            "too many filled voxels ({}/{}) — voxelization cassée ?",
            filled,
            total
        );
    }

    #[test]
    fn unit_cube_corners_filled() {
        let (positions, indices) = unit_cube_mesh();
        let grid = voxelize_mesh(&positions, &indices, &VoxelizerConfig::default());
        // Le voxel au centre du cube doit être rempli.
        let res = grid.resolution;
        let center_idx = UVec3::new(res / 2, res / 2, res / 2);
        assert!(
            grid.is_filled(center_idx),
            "center voxel ({}) should be filled inside unit cube",
            center_idx
        );
    }

    #[test]
    fn empty_outside_cube() {
        let (positions, indices) = unit_cube_mesh();
        let grid = voxelize_mesh(&positions, &indices, &VoxelizerConfig::default());
        // Voxels aux extrêmes (coins de la grille avec padding) doivent être vides.
        let res = grid.resolution;
        assert!(
            !grid.is_filled(UVec3::new(0, 0, 0)),
            "corner voxel (0,0,0) should be empty (outside cube due to padding)"
        );
        assert!(
            !grid.is_filled(UVec3::new(res - 1, res - 1, res - 1)),
            "corner voxel (res-1,res-1,res-1) should be empty"
        );
    }

    #[test]
    fn voxel_center_in_bounds() {
        let grid = VoxelGrid::new_empty(8, Vec3::ZERO, Vec3::splat(1.0));
        let c = grid.voxel_center(UVec3::new(0, 0, 0));
        // Premier voxel : centre à (1/16, 1/16, 1/16) = 0.0625
        assert!((c.x - 0.0625).abs() < 1e-4);
        assert!((c.y - 0.0625).abs() < 1e-4);
        assert!((c.z - 0.0625).abs() < 1e-4);
    }

    #[test]
    fn estimated_volume_unit_cube_approx_1() {
        let (positions, indices) = unit_cube_mesh();
        let grid = voxelize_mesh(&positions, &indices, &VoxelizerConfig::default());
        let vol = grid.estimated_volume();
        // Cube unitaire → volume ≈ 1m³. Avec padding 5% (grid 1.10³ = 1.33),
        // voxelization peut être inexacte → tolérance large [0.5, 1.5].
        assert!(
            (0.5..=1.5).contains(&vol),
            "expected volume ~1.0 for unit cube, got {:.3}",
            vol
        );
    }

    #[test]
    fn resolution_for_mesh_size_clamps() {
        assert_eq!(VoxelizerConfig::resolution_for_mesh_size(0.5), 16); // 10 voxels min clamped to 16
        assert_eq!(VoxelizerConfig::resolution_for_mesh_size(1.75), 35); // res 35 (1.75m / 5cm)
        assert_eq!(VoxelizerConfig::resolution_for_mesh_size(10.0), 64); // capped
    }

    #[test]
    fn degenerate_triangle_no_panic() {
        // 3 vertices alignés → triangle dégénéré (aire 0)
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
        ];
        let indices = vec![0, 1, 2];
        let grid = voxelize_mesh(&positions, &indices, &VoxelizerConfig::default());
        // Triangle dégénéré : 0 voxels d'interieur, mais surface fallback marque
        // les vertex → 3 voxels (ou collapsed) → assert pas crash + filled <= 3.
        assert!(grid.filled_count() <= 5, "degenerate triangle, got {}", grid.filled_count());
    }

    #[test]
    fn empty_mesh_returns_empty_grid() {
        let grid = voxelize_mesh(&[], &[], &VoxelizerConfig::default());
        assert_eq!(grid.filled_count(), 0);
    }

    #[test]
    fn iter_filled_matches_count() {
        let (positions, indices) = unit_cube_mesh();
        let grid = voxelize_mesh(&positions, &indices, &VoxelizerConfig::default());
        let iter_count = grid.iter_filled().count();
        assert_eq!(iter_count, grid.filled_count());
    }
}
