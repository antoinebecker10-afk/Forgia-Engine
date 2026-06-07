//! Points + attributes — the central worldgen data model (story-578 §2.2).
//!
//! We do NOT place modules directly. Layout produces a cloud of cheap `Point`s (transform +
//! attributes); resolution/spawn consume them in bulk. P1 only needs a couple of simple
//! generators (a row, a varied showcase) to prove instanced spawning + grounding.
//!
//! Ground height is provided by an **injected** [`GroundSampler`] so worldgen never depends
//! on `forgia-terrain` or any gameplay crate (decoupling invariant, story-578 §10).

use crate::recipe::HamletRecipe;
use crate::registry::{AssetMeta, AssetRegistry, AssetRole};
use bevy::prelude::*;

/// One module instance to spawn — the atomic unit of the points+attributes model.
#[derive(Clone, Debug)]
pub struct Point {
    /// Module id (registry key == glTF mesh name).
    pub module_id: String,
    /// World position where the module's **bottom** should rest (XZ + ground Y).
    pub ground_pos: Vec3,
    /// Yaw around Y, radians.
    pub yaw: f32,
    /// Uniform scale.
    pub scale: f32,
}

/// A batch of placement points (one generation pass / one chunk).
#[derive(Clone, Debug, Default)]
pub struct PointCloud {
    pub points: Vec<Point>,
}

/// Ground-height sampler, **injected** to keep worldgen decoupled from terrain/gameplay.
///
/// Default = a flat plane at y = 0 (the caller adds a base height via the point origin). A
/// terrain-backed game can `insert_resource(GroundSampler::new(|p| terrain.height_at(p)))`.
#[derive(Resource)]
pub struct GroundSampler(Box<dyn Fn(Vec2) -> f32 + Send + Sync>);

impl Default for GroundSampler {
    fn default() -> Self {
        Self(Box::new(|_| 0.0))
    }
}

impl GroundSampler {
    /// Build a sampler from any height function `(x, z) -> y`.
    pub fn new(f: impl Fn(Vec2) -> f32 + Send + Sync + 'static) -> Self {
        Self(Box::new(f))
    }

    /// Sample the ground height at world XZ.
    #[inline]
    pub fn height(&self, x: f32, z: f32) -> f32 {
        (self.0)(Vec2::new(x, z))
    }
}

/// Lay `count` modules of one `role` in a straight line along `axis` from `origin`, spaced by
/// `spacing`, each grounded via the sampler. Modules are picked round-robin from the role pool
/// in deterministic id order (so the same registry always yields the same row).
pub fn generate_row(
    registry: &AssetRegistry,
    ground: &GroundSampler,
    role: AssetRole,
    count: usize,
    spacing: f32,
    origin: Vec3,
    axis: Vec3,
    scale: f32,
) -> PointCloud {
    let mut pool: Vec<&AssetMeta> = registry.by_role(role).collect();
    pool.sort_by(|a, b| a.id.cmp(&b.id));
    if pool.is_empty() {
        return PointCloud::default();
    }
    let mut points = Vec::with_capacity(count);
    for i in 0..count {
        let meta = pool[i % pool.len()];
        points.push(place(meta, ground, origin, axis, spacing, i, scale));
    }
    PointCloud { points }
}

/// A varied showcase row: up to 2 modules from each of several roles, sanely sized, so the
/// grounding fix is *visible* across very different pivots (a flat platform next to a tall
/// pillar next to a deep-pivot prop — all flush on the same ground line).
pub fn generate_showcase_row(
    registry: &AssetRegistry,
    ground: &GroundSampler,
    origin: Vec3,
    axis: Vec3,
    spacing: f32,
    scale: f32,
) -> PointCloud {
    const ROLES: [AssetRole; 4] = [
        AssetRole::Platform,
        AssetRole::Pillar,
        AssetRole::Wall,
        AssetRole::Prop,
    ];
    let mut chosen: Vec<&AssetMeta> = Vec::new();
    for role in ROLES {
        let mut pool: Vec<&AssetMeta> = registry
            .by_role(role)
            .filter(|m| m.height() >= 1.0 && m.height() <= 12.0)
            .collect();
        pool.sort_by(|a, b| a.id.cmp(&b.id));
        for m in pool.into_iter().take(2) {
            chosen.push(m);
        }
    }
    let mut points = Vec::with_capacity(chosen.len());
    for (i, meta) in chosen.into_iter().enumerate() {
        points.push(place(meta, ground, origin, axis, spacing, i, scale));
    }
    PointCloud { points }
}

/// Place module `meta` at slot `i` along the row.
fn place(
    meta: &AssetMeta,
    ground: &GroundSampler,
    origin: Vec3,
    axis: Vec3,
    spacing: f32,
    i: usize,
    scale: f32,
) -> Point {
    let offset = axis * (i as f32 * spacing);
    let x = origin.x + offset.x;
    let z = origin.z + offset.z;
    let gy = origin.y + ground.height(x, z);
    Point {
        module_id: meta.id.clone(),
        ground_pos: Vec3::new(x, gy, z),
        yaw: 0.0,
        scale,
    }
}

/// Tiny deterministic RNG (splitmix64). P2 jitter / selection are reproducible from the
/// recipe seed. P4 will switch to `forgia-rng` for hierarchical world seeds.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `[0, 1)`.
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    /// Uniform signed in `[-1, 1)`.
    fn next_signed(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

fn pick<'a>(pool: &[&'a AssetMeta], rng: &mut Rng) -> Option<&'a AssetMeta> {
    if pool.is_empty() {
        None
    } else {
        Some(pool[(rng.next_u64() as usize) % pool.len()])
    }
}

/// Generate a small hamlet from a [`HamletRecipe`]: a jittered grid of buildings with an
/// optional wall ring. Deterministic from `recipe.seed`. `axis_x`/`axis_z` are the horizontal
/// grid axes in world space (e.g. camera right / forward), `origin` the grid center at ground.
pub fn generate_hamlet(
    recipe: &HamletRecipe,
    registry: &AssetRegistry,
    ground: &GroundSampler,
    origin: Vec3,
    axis_x: Vec3,
    axis_z: Vec3,
) -> PointCloud {
    let cols = recipe.grid_cols.max(1);
    let rows = recipe.grid_rows.max(1);
    let mut rng = Rng::new(recipe.seed);

    // Building pool: the chosen roles, minus oversized pieces (no 50m spires in a hamlet).
    let mut building_pool: Vec<&AssetMeta> = recipe
        .building_roles
        .iter()
        .flat_map(|role| registry.by_role(*role))
        .filter(|m| m.height() <= 16.0)
        .collect();
    building_pool.sort_by(|a, b| a.id.cmp(&b.id));
    let mut border_pool: Vec<&AssetMeta> = registry.by_role(recipe.border_role).collect();
    border_pool.sort_by(|a, b| a.id.cmp(&b.id));

    let mut points = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let cx = (col as f32 - (cols as f32 - 1.0) * 0.5) * recipe.cell_size;
            let cz = (row as f32 - (rows as f32 - 1.0) * 0.5) * recipe.cell_size;
            let is_border = col == 0 || col == cols - 1 || row == 0 || row == rows - 1;

            let meta = if is_border {
                if recipe.border {
                    pick(&border_pool, &mut rng)
                } else {
                    None
                }
            } else if rng.next_f32() < recipe.fill_chance {
                pick(&building_pool, &mut rng)
            } else {
                None
            };
            let Some(meta) = meta else {
                continue;
            };

            let jx = rng.next_signed() * recipe.jitter;
            let jz = rng.next_signed() * recipe.jitter;
            let world = origin + axis_x * (cx + jx) + axis_z * (cz + jz);
            let gy = world.y + ground.height(world.x, world.z);
            let yaw = if recipe.yaw_random {
                (rng.next_u64() % 4) as f32 * std::f32::consts::FRAC_PI_2
            } else {
                0.0
            };
            points.push(Point {
                module_id: meta.id.clone(),
                ground_pos: Vec3::new(world.x, gy, world.z),
                yaw,
                scale: recipe.scale,
            });
        }
    }
    PointCloud { points }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> AssetRegistry {
        AssetRegistry::from_ron(include_str!("../../../assets/registry/asset_meta.ron")).unwrap()
    }

    #[test]
    fn hamlet_is_deterministic() {
        let reg = registry();
        let ground = GroundSampler::default();
        let recipe = HamletRecipe::default();
        let a = generate_hamlet(&recipe, &reg, &ground, Vec3::ZERO, Vec3::X, Vec3::Z);
        let b = generate_hamlet(&recipe, &reg, &ground, Vec3::ZERO, Vec3::X, Vec3::Z);
        assert!(!a.points.is_empty(), "hamlet should produce modules");
        assert_eq!(a.points.len(), b.points.len(), "same seed → same count");
        let ids_a: Vec<&String> = a.points.iter().map(|p| &p.module_id).collect();
        let ids_b: Vec<&String> = b.points.iter().map(|p| &p.module_id).collect();
        assert_eq!(ids_a, ids_b, "same seed → identical module sequence");
    }

    #[test]
    fn different_seed_changes_hamlet() {
        let reg = registry();
        let ground = GroundSampler::default();
        let r1 = HamletRecipe { seed: 1, ..HamletRecipe::default() };
        let r2 = HamletRecipe { seed: 999, ..HamletRecipe::default() };
        let a = generate_hamlet(&r1, &reg, &ground, Vec3::ZERO, Vec3::X, Vec3::Z);
        let b = generate_hamlet(&r2, &reg, &ground, Vec3::ZERO, Vec3::X, Vec3::Z);
        // Border is deterministic regardless of seed, but interior fill differs → sequences differ.
        let ids_a: Vec<&String> = a.points.iter().map(|p| &p.module_id).collect();
        let ids_b: Vec<&String> = b.points.iter().map(|p| &p.module_id).collect();
        assert_ne!(ids_a, ids_b, "different seed should change the hamlet");
    }

    #[test]
    fn hamlet_modules_are_grounded() {
        // Every generated point's ground_pos.y matches the flat sampler (0) at origin 0.
        let reg = registry();
        let ground = GroundSampler::default();
        let recipe = HamletRecipe::default();
        let cloud = generate_hamlet(&recipe, &reg, &ground, Vec3::ZERO, Vec3::X, Vec3::Z);
        for p in &cloud.points {
            assert!(p.ground_pos.y.abs() < 1e-3, "flat ground → y≈0, got {}", p.ground_pos.y);
        }
    }
}
