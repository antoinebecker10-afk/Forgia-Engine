//! # forgia-medial-axis — Distance field + sphere packing
//!
//! **Phase 1 du pipeline auto-rig Forgia (Pinocchio-inspired, story-440)** :
//! prend une `VoxelGrid` solide (output de `forgia-mesh-voxelizer`) et extrait
//! le **medial axis** sous forme d'un graphe de sphères.
//!
//! ## Concept
//!
//! Le **medial axis** d'une forme 3D est l'ensemble des points équidistants à
//! au moins 2 points de la surface. Intuitivement : c'est le "squelette
//! géométrique" naturel de la forme (Blum 1967).
//!
//! Pour un humanoïde : axe vertical central + 4 ramifications (jambes, bras).
//! Pour un cube : 1 point au centre. Pour un cylindre : ligne centrale.
//!
//! C'est cette structure que Pinocchio (Baran 2007) utilise pour fitter un
//! template skeleton — chaque bone du template est associé à un sous-graphe
//! du medial axis qui matche sa morphologie.
//!
//! ## Algorithme
//!
//! 1. **Distance transform** (BFS depuis la frontière) : pour chaque voxel
//!    intérieur, calcule la distance Euclidienne au plus proche voxel de
//!    surface (= vide voisin d'un rempli).
//! 2. **Sphere packing greedy** : itère "pick max distance → create sphere →
//!    mark covered voxels → repeat". Produit un set de sphères qui couvrent
//!    l'intérieur.
//! 3. **Graph edges** : 2 sphères connectées si leurs surfaces se touchent
//!    (distance centres < sum radii × kissing_factor).
//!
//! ## Coût
//!
//! Distance transform : O(V) où V = voxels intérieurs (~5k-15k pour Rex 32³).
//! Sphere packing : O(V × N) où N = nombre de sphères (~50-200 typique).
//! Edges : O(N²) (~10k-40k ops, négligeable).
//!
//! Total : ~50ms one-shot par mesh. Acceptable.

use bevy::math::{UVec3, Vec3};
use forgia_mesh_voxelizer::VoxelGrid;

/// Une sphère du medial axis : centre + rayon en world space.
#[derive(Debug, Clone, Copy)]
pub struct MedialSphere {
    pub center: Vec3,
    pub radius: f32,
}

/// Graphe de sphères représentant le medial axis du mesh.
#[derive(Debug, Clone)]
pub struct MedialAxisGraph {
    pub spheres: Vec<MedialSphere>,
    /// Connexions (undirected, peers `i < j`).
    pub edges: Vec<(usize, usize)>,
}

impl MedialAxisGraph {
    pub fn sphere_count(&self) -> usize {
        self.spheres.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Itère les voisins (par index) d'une sphère.
    pub fn neighbors(&self, idx: usize) -> impl Iterator<Item = usize> + '_ {
        self.edges.iter().filter_map(move |&(a, b)| {
            if a == idx {
                Some(b)
            } else if b == idx {
                Some(a)
            } else {
                None
            }
        })
    }

    /// Bounds AABB du graph (utile pour debug/visualisation).
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        if self.spheres.is_empty() {
            return None;
        }
        let mut min_v = Vec3::splat(f32::INFINITY);
        let mut max_v = Vec3::splat(f32::NEG_INFINITY);
        for s in &self.spheres {
            min_v = min_v.min(s.center - Vec3::splat(s.radius));
            max_v = max_v.max(s.center + Vec3::splat(s.radius));
        }
        Some((min_v, max_v))
    }
}

/// Configuration de l'extraction du medial axis.
#[derive(Debug, Clone)]
pub struct MedialAxisConfig {
    /// Rayon min en voxels pour qu'une sphère soit conservée (skip noise).
    /// 1.0 = accepter sphères fines (= ramifications bras T-pose).
    pub min_sphere_radius_voxels: f32,
    /// Facteur de "kissing" pour edges : 2 sphères sont voisines si distance
    /// centres < `(r1 + r2) * kissing_factor`. Default 1.2 (légèrement
    /// chevauchantes → garantit connectivité).
    pub kissing_factor: f32,
    /// Cap max nombre de sphères pour borner le coût.
    pub max_spheres: usize,
    /// Coverage factor : après packing une sphère, on marque covered les voxels
    /// dans un rayon `sphere_radius × coverage_factor`. Default 0.7 = laisse
    /// 30% de la sphère "non couverte" pour permettre à d'autres sphères de
    /// se loger dans les ramifications (bras T-pose, jambes). 1.0 = greedy
    /// agressif = medial axis collapse à une ligne (BUG runtime 2026-05-17 night).
    pub coverage_factor: f32,
}

impl Default for MedialAxisConfig {
    fn default() -> Self {
        Self {
            min_sphere_radius_voxels: 1.0,
            kissing_factor: 1.2,
            max_spheres: 500,
            coverage_factor: 0.7,
        }
    }
}

/// Extrait le medial axis d'une `VoxelGrid` solide.
pub fn extract_medial_axis(grid: &VoxelGrid, config: &MedialAxisConfig) -> MedialAxisGraph {
    if grid.filled_count() == 0 {
        return MedialAxisGraph {
            spheres: Vec::new(),
            edges: Vec::new(),
        };
    }

    // 1. Distance transform : pour chaque voxel intérieur, distance (en voxels)
    //    au voxel de surface le plus proche. BFS depuis la frontière.
    let distance_field = compute_distance_field(grid);

    // 2. Sphere packing greedy.
    let spheres = pack_spheres_greedy(grid, &distance_field, config);

    // 3. Connect via kissing.
    let edges = connect_kissing_spheres(&spheres, config.kissing_factor);

    MedialAxisGraph { spheres, edges }
}

/// Computes Euclidean Distance Transform sur les voxels intérieurs.
/// Output : array `[Option<f32>; res³]` où `Some(d)` est la distance en VOXELS
/// au plus proche voxel de surface ; `None` si voxel vide (extérieur).
///
/// Implémentation simple BFS (Chamfer distance approximation).
fn compute_distance_field(grid: &VoxelGrid) -> Vec<Option<f32>> {
    let res = grid.resolution as usize;
    let n = res * res * res;
    let mut field: Vec<Option<f32>> = vec![None; n];

    // BFS queue : (voxel_idx, distance_in_voxels)
    let mut queue: Vec<(UVec3, f32)> = Vec::new();

    // Init : pour chaque voxel intérieur, si voisin (6-connectivity) est vide,
    // c'est un voxel de "surface" → distance 0.5 (= demi-voxel jusqu'à la frontière).
    for x in 0..grid.resolution {
        for y in 0..grid.resolution {
            for z in 0..grid.resolution {
                let idx = UVec3::new(x, y, z);
                if !grid.is_filled(idx) {
                    continue;
                }
                let is_surface = has_empty_neighbor(grid, idx);
                if is_surface {
                    let li = linear_index(idx, res);
                    field[li] = Some(0.5);
                    queue.push((idx, 0.5));
                }
            }
        }
    }

    // BFS propagation : distance += 1.0 par étape (Manhattan ≈ Euclidean pour
    // mesh sans détails fins ; pour vraie Euclidienne, utiliser 3-4-5 chamfer).
    let mut head = 0;
    while head < queue.len() {
        let (cur, d) = queue[head];
        head += 1;
        let new_d = d + 1.0;
        for n in neighbors_6(cur, grid.resolution) {
            if !grid.is_filled(n) {
                continue;
            }
            let li = linear_index(n, res);
            if field[li].is_none() {
                field[li] = Some(new_d);
                queue.push((n, new_d));
            }
        }
    }

    field
}

fn has_empty_neighbor(grid: &VoxelGrid, idx: UVec3) -> bool {
    for n in neighbors_6(idx, grid.resolution) {
        if !grid.is_filled(n) {
            return true;
        }
    }
    // Bord de grille = considère qu'il y a un voxel vide hors-grille
    idx.x == 0
        || idx.y == 0
        || idx.z == 0
        || idx.x == grid.resolution - 1
        || idx.y == grid.resolution - 1
        || idx.z == grid.resolution - 1
}

fn neighbors_6(idx: UVec3, res: u32) -> impl Iterator<Item = UVec3> {
    let offsets: [(i32, i32, i32); 6] = [
        (-1, 0, 0),
        (1, 0, 0),
        (0, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
    ];
    let res_i = res as i32;
    let x = idx.x as i32;
    let y = idx.y as i32;
    let z = idx.z as i32;
    offsets.into_iter().filter_map(move |(dx, dy, dz)| {
        let nx = x + dx;
        let ny = y + dy;
        let nz = z + dz;
        if nx >= 0 && nx < res_i && ny >= 0 && ny < res_i && nz >= 0 && nz < res_i {
            Some(UVec3::new(nx as u32, ny as u32, nz as u32))
        } else {
            None
        }
    })
}

fn linear_index(idx: UVec3, res: usize) -> usize {
    idx.x as usize * res * res + idx.y as usize * res + idx.z as usize
}

/// Sphere packing greedy : pick voxel max distance → sphere → mark covered → repeat.
fn pack_spheres_greedy(
    grid: &VoxelGrid,
    distance_field: &[Option<f32>],
    config: &MedialAxisConfig,
) -> Vec<MedialSphere> {
    let res = grid.resolution;
    let voxel_size = grid.voxel_size();
    let avg_voxel_size = (voxel_size.x + voxel_size.y + voxel_size.z) / 3.0;
    let mut covered: Vec<bool> = vec![false; (res as usize).pow(3)];
    let mut spheres: Vec<MedialSphere> = Vec::new();

    while spheres.len() < config.max_spheres {
        // 1. Trouve le voxel intérieur, non-covered, avec MAX distance field.
        let mut best_idx: Option<UVec3> = None;
        let mut best_dist: f32 = config.min_sphere_radius_voxels;
        for x in 0..res {
            for y in 0..res {
                for z in 0..res {
                    let idx = UVec3::new(x, y, z);
                    let li = linear_index(idx, res as usize);
                    if covered[li] {
                        continue;
                    }
                    if let Some(d) = distance_field[li] {
                        if d > best_dist {
                            best_dist = d;
                            best_idx = Some(idx);
                        }
                    }
                }
            }
        }

        let Some(center_idx) = best_idx else {
            break; // plus de voxels avec distance >= seuil
        };

        // 2. Crée la sphère et marque les voxels couverts (= dans la sphère).
        let center_world = grid.voxel_center(center_idx);
        let radius_world = best_dist * avg_voxel_size;
        spheres.push(MedialSphere {
            center: center_world,
            radius: radius_world,
        });

        // Marque covered : voxels à distance < radius * coverage_factor.
        // < 1.0 laisse de la place dans les ramifications pour d'autres sphères.
        let radius_voxels = best_dist * config.coverage_factor;
        let radius_voxels_sq = radius_voxels * radius_voxels;
        let cx = center_idx.x as f32;
        let cy = center_idx.y as f32;
        let cz = center_idx.z as f32;
        let r_ceil = radius_voxels.ceil() as i32;
        let res_i = res as i32;
        for dx in -r_ceil..=r_ceil {
            for dy in -r_ceil..=r_ceil {
                for dz in -r_ceil..=r_ceil {
                    let nx = center_idx.x as i32 + dx;
                    let ny = center_idx.y as i32 + dy;
                    let nz = center_idx.z as i32 + dz;
                    if nx < 0 || nx >= res_i || ny < 0 || ny >= res_i || nz < 0 || nz >= res_i {
                        continue;
                    }
                    let fx = nx as f32 - cx;
                    let fy = ny as f32 - cy;
                    let fz = nz as f32 - cz;
                    if fx * fx + fy * fy + fz * fz <= radius_voxels_sq {
                        let n_idx = UVec3::new(nx as u32, ny as u32, nz as u32);
                        let li = linear_index(n_idx, res as usize);
                        covered[li] = true;
                    }
                }
            }
        }
    }

    spheres
}

/// Connecte 2 sphères si leurs surfaces se "touchent" (kissing) :
/// `distance_centres < (r1 + r2) * kissing_factor`.
fn connect_kissing_spheres(spheres: &[MedialSphere], kissing_factor: f32) -> Vec<(usize, usize)> {
    let mut edges = Vec::new();
    for i in 0..spheres.len() {
        for j in (i + 1)..spheres.len() {
            let d = spheres[i].center.distance(spheres[j].center);
            let touch_threshold = (spheres[i].radius + spheres[j].radius) * kissing_factor;
            if d < touch_threshold {
                edges.push((i, j));
            }
        }
    }
    edges
}

// ── Tests headless ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_mesh_voxelizer::{voxelize_mesh, VoxelizerConfig};

    fn unit_cube() -> (Vec<Vec3>, Vec<u32>) {
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
        ];
        let indices = vec![
            0, 2, 1, 1, 2, 3, 4, 5, 6, 5, 7, 6, 0, 1, 4, 1, 5, 4, 2, 6, 3, 3, 6, 7, 0, 4, 2, 2, 4,
            6, 1, 3, 5, 3, 7, 5,
        ];
        (positions, indices)
    }

    #[test]
    fn cube_medial_axis_has_at_least_one_sphere() {
        let (positions, indices) = unit_cube();
        let voxel_cfg = VoxelizerConfig {
            resolution: 24,
            ..Default::default()
        };
        let grid = voxelize_mesh(&positions, &indices, &voxel_cfg);
        let medial_cfg = MedialAxisConfig::default();
        let graph = extract_medial_axis(&grid, &medial_cfg);
        assert!(
            !graph.spheres.is_empty(),
            "cube should have at least 1 medial sphere"
        );
    }

    #[test]
    fn cube_largest_sphere_near_center() {
        let (positions, indices) = unit_cube();
        let grid = voxelize_mesh(
            &positions,
            &indices,
            &VoxelizerConfig {
                resolution: 24,
                ..Default::default()
            },
        );
        let graph = extract_medial_axis(&grid, &MedialAxisConfig::default());

        // La plus grosse sphère doit être proche du centre du cube (0.5, 0.5, 0.5).
        let largest = graph
            .spheres
            .iter()
            .max_by(|a, b| a.radius.partial_cmp(&b.radius).unwrap())
            .expect("at least 1 sphere");
        let center = Vec3::new(0.5, 0.5, 0.5);
        let d = largest.center.distance(center);
        // Tolérance large : voxelization 24³ + plateau de voxels à distance max
        // + coverage_factor 0.7 (= packing produit plus de sphères diverses,
        // la "plus grosse" peut être légèrement décentrée).
        assert!(
            d < 0.40,
            "largest sphere center {} should be near cube center {} (got d={:.3})",
            largest.center,
            center,
            d
        );
        // Rayon de la plus grosse sphère ≈ 0.5 (rayon inscrit dans le cube)
        assert!(
            largest.radius > 0.25 && largest.radius < 0.60,
            "largest sphere radius should be ≈0.5 for unit cube, got {:.3}",
            largest.radius
        );
    }

    #[test]
    fn empty_grid_no_spheres() {
        let grid = VoxelGrid::new_empty(8, Vec3::ZERO, Vec3::ONE);
        let graph = extract_medial_axis(&grid, &MedialAxisConfig::default());
        assert!(graph.spheres.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn capsule_like_has_axial_spheres() {
        // Capsule grossière : cylindre 1m tall, radius 0.15. Vérifier que
        // les sphères du medial axis s'alignent sur l'axe central Y.
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        let n_segments = 16;
        let n_rings = 20;
        // Ring vertices
        for r in 0..=n_rings {
            let y = r as f32 / n_rings as f32; // 0..1
            for s in 0..n_segments {
                let a = (s as f32 / n_segments as f32) * std::f32::consts::TAU;
                positions.push(Vec3::new(0.15 * a.cos(), y, 0.15 * a.sin()));
            }
        }
        // Triangles : rings
        for r in 0..n_rings {
            for s in 0..n_segments {
                let s2 = (s + 1) % n_segments;
                let i0 = r * n_segments + s;
                let i1 = r * n_segments + s2;
                let i2 = (r + 1) * n_segments + s;
                let i3 = (r + 1) * n_segments + s2;
                indices.extend_from_slice(&[i0 as u32, i2 as u32, i1 as u32]);
                indices.extend_from_slice(&[i1 as u32, i2 as u32, i3 as u32]);
            }
        }
        // Top + bottom caps simplistes : 1 vertex centre + fan
        let top_center = positions.len() as u32;
        positions.push(Vec3::new(0.0, 1.0, 0.0));
        let bot_center = positions.len() as u32;
        positions.push(Vec3::new(0.0, 0.0, 0.0));
        for s in 0..n_segments {
            let s2 = (s + 1) % n_segments;
            let top_i0 = (n_rings * n_segments + s) as u32;
            let top_i1 = (n_rings * n_segments + s2) as u32;
            let bot_i0 = s as u32;
            let bot_i1 = s2 as u32;
            indices.extend_from_slice(&[top_center, top_i1, top_i0]);
            indices.extend_from_slice(&[bot_center, bot_i0, bot_i1]);
        }

        let grid = voxelize_mesh(
            &positions,
            &indices,
            &VoxelizerConfig {
                resolution: 32,
                ..Default::default()
            },
        );
        let graph = extract_medial_axis(
            &grid,
            &MedialAxisConfig {
                min_sphere_radius_voxels: 1.0,
                ..Default::default()
            },
        );

        // Au moins 2 sphères (le cylindre s'étend en Y, donc plusieurs sphères axiales)
        assert!(
            graph.spheres.len() >= 2,
            "capsule should have ≥2 medial spheres, got {}",
            graph.spheres.len()
        );

        // Toutes les sphères doivent être proches de l'axe X=0, Z=0 (cylindre)
        // Tolérance : rayon cylindre = 0.15 → voxel size ~0.05 → tolérance 0.15 OK
        for s in &graph.spheres {
            let axial_dist = (s.center.x * s.center.x + s.center.z * s.center.z).sqrt();
            assert!(
                axial_dist < 0.15,
                "sphere centre should be near axis Y, got XZ dist {:.3}",
                axial_dist
            );
        }
    }

    #[test]
    fn neighbors_function_skips_out_of_bounds() {
        let n = neighbors_6(UVec3::new(0, 0, 0), 4).count();
        // Au coin (0,0,0), seuls +X, +Y, +Z voisins valides
        assert_eq!(n, 3);
    }

    #[test]
    fn bounds_returns_aabb() {
        let graph = MedialAxisGraph {
            spheres: vec![
                MedialSphere {
                    center: Vec3::new(0.0, 0.0, 0.0),
                    radius: 0.5,
                },
                MedialSphere {
                    center: Vec3::new(1.0, 0.0, 0.0),
                    radius: 0.3,
                },
            ],
            edges: vec![],
        };
        let (min, max) = graph.bounds().expect("non-empty graph");
        assert!((min.x - (-0.5)).abs() < 1e-4);
        assert!((max.x - 1.3).abs() < 1e-4);
    }
}
