//! Resolve — turn a parcel into a **building** via a grammar/template (story-578 P5).
//!
//! A [`BuildingTemplate`] describes how to assemble a footprint into modules: a `base`, a stack
//! of `body` floors, and a `cap` (roof). Modules are stacked in Y using their registry heights so
//! each rests on the one below — the simplest real "footprint → floors → roof" grammar. The floor
//! count is seeded, so buildings vary across parcels while staying reproducible.
//!
//! Pure: no ECS, no ground sampler (the caller passes the footprint's ground Y).

use crate::points::Point;
use crate::recipe::BuildingTemplate;
use crate::registry::{AssetMeta, AssetRegistry, AssetRole};
use crate::seed::SeededRng;
use bevy::prelude::Vec3;

/// Build the module stack for one footprint, centered at world `(world_x, world_z)` with its
/// base resting on `base_y`. Deterministic from `seed`.
pub fn resolve_building(
    template: &BuildingTemplate,
    registry: &AssetRegistry,
    world_x: f32,
    world_z: f32,
    base_y: f32,
    seed: u64,
    scale: f32,
) -> Vec<Point> {
    let mut rng = SeededRng::new(seed);
    let base = pick_role(registry, template.base_role, &mut rng);
    let body = pick_role(registry, template.body_role, &mut rng);
    let cap = pick_role(registry, template.cap_role, &mut rng);

    let lo = template.min_floors;
    let hi = template.max_floors.max(lo);
    let floors = lo + rng.below((hi - lo + 1) as usize) as u32;

    let mut points = Vec::new();
    let mut bottom = base_y;

    if let Some(m) = base {
        let yaw = rng.below(4) as f32 * std::f32::consts::FRAC_PI_2;
        points.push(Point {
            module_id: m.id.clone(),
            ground_pos: Vec3::new(world_x, bottom, world_z),
            yaw,
            scale,
        });
        bottom += m.height() * scale;
    }
    for _ in 0..floors {
        if let Some(m) = body {
            let yaw = rng.below(4) as f32 * std::f32::consts::FRAC_PI_2;
            points.push(Point {
                module_id: m.id.clone(),
                ground_pos: Vec3::new(world_x, bottom, world_z),
                yaw,
                scale,
            });
            bottom += m.height() * scale;
        }
    }
    if let Some(m) = cap {
        let yaw = rng.below(4) as f32 * std::f32::consts::FRAC_PI_2;
        points.push(Point {
            module_id: m.id.clone(),
            ground_pos: Vec3::new(world_x, bottom, world_z),
            yaw,
            scale,
        });
    }
    points
}

/// Pick a sanely-sized module of `role` (deterministic). `None` if the role pool is empty.
fn pick_role<'a>(
    registry: &'a AssetRegistry,
    role: AssetRole,
    rng: &mut SeededRng,
) -> Option<&'a AssetMeta> {
    let mut pool: Vec<&AssetMeta> = registry
        .by_role(role)
        .filter(|m| m.height() <= 16.0)
        .collect();
    pool.sort_by(|a, b| a.id.cmp(&b.id));
    if pool.is_empty() {
        None
    } else {
        Some(pool[rng.below(pool.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> AssetRegistry {
        AssetRegistry::from_ron(include_str!("../../../assets/registry/asset_meta.ron")).unwrap()
    }

    fn template() -> BuildingTemplate {
        BuildingTemplate {
            base_role: AssetRole::Platform,
            body_role: AssetRole::Pillar,
            cap_role: AssetRole::Prop,
            min_floors: 1,
            max_floors: 3,
        }
    }

    #[test]
    fn building_is_deterministic() {
        let reg = registry();
        let t = template();
        let a = resolve_building(&t, &reg, 5.0, -3.0, 2.0, 12345, 0.5);
        let b = resolve_building(&t, &reg, 5.0, -3.0, 2.0, 12345, 0.5);
        assert!(!a.is_empty());
        let ids_a: Vec<&String> = a.iter().map(|p| &p.module_id).collect();
        let ids_b: Vec<&String> = b.iter().map(|p| &p.module_id).collect();
        assert_eq!(ids_a, ids_b);
    }

    #[test]
    fn modules_stack_upward_from_base() {
        let reg = registry();
        let t = template();
        let b = resolve_building(&t, &reg, 0.0, 0.0, 10.0, 7, 0.5);
        assert!(b.len() >= 2, "base + at least one floor");
        assert!(
            (b[0].ground_pos.y - 10.0).abs() < 1e-3,
            "base sits on base_y"
        );
        for w in b.windows(2) {
            assert!(
                w[1].ground_pos.y >= w[0].ground_pos.y,
                "each module is on top of the previous"
            );
        }
        // All share the same footprint XZ.
        for p in &b {
            assert!((p.ground_pos.x - 0.0).abs() < 1e-3 && (p.ground_pos.z - 0.0).abs() < 1e-3);
        }
    }

    #[test]
    fn floor_count_in_range() {
        let reg = registry();
        let t = template(); // base(1) + [1..=3] floors + cap(1) → 3..=5 modules
        for seed in 0..50u64 {
            let n = resolve_building(&t, &reg, 0.0, 0.0, 0.0, seed, 0.5).len();
            assert!(
                (3..=5).contains(&n),
                "module count {n} out of expected range"
            );
        }
    }
}
