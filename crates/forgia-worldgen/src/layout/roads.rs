//! Roads — a road network from a **tensor field** (story-578 P3).
//!
//! A tensor field assigns a *major* road direction to every point of the plane; the *minor*
//! direction is perpendicular. Roads are traced as **streamlines** that follow the field, with
//! a separation rule so they don't overlap — the classic tensor-field road technique
//! (Chen et al. 2008, CityEngine), kept deliberately simple for P3.
//!
//! The field here = a uniform grid orientation blended with an optional *tangential* (ring)
//! component around a center. With `radial_strength = 0` it is a clean grid (roads align with
//! the parcel blocks); raise it for organic ring roads near the center. Pure 2D geometry, no ECS.

use bevy::math::Vec2;
use std::collections::HashSet;

/// Major avenues vs minor streets (purely a viz/role distinction for P3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadKind {
    Major,
    Minor,
}

/// A straight road piece between two points (streamlines are polylines of these).
#[derive(Clone, Copy, Debug)]
pub struct RoadSegment {
    pub a: Vec2,
    pub b: Vec2,
    pub kind: RoadKind,
}

/// The traced road network.
#[derive(Clone, Debug, Default)]
pub struct RoadNetwork {
    pub segments: Vec<RoadSegment>,
}

/// Tensor field providing the major road direction at any point.
#[derive(Clone, Copy, Debug)]
pub struct TensorField {
    /// Grid orientation (radians).
    pub base_angle: f32,
    /// Center of the radial (ring) component.
    pub center: Vec2,
    /// 0 = pure grid; >0 = rings near the center.
    pub radial_strength: f32,
    /// Influence falloff radius of the radial component.
    pub radial_radius: f32,
}

impl TensorField {
    /// Unit major direction at `p` (grid blended with a tangential ring component).
    pub fn major_dir(&self, p: Vec2) -> Vec2 {
        let grid = Vec2::from_angle(self.base_angle);
        if self.radial_strength <= 0.0 {
            return grid;
        }
        let d = p - self.center;
        let dist = d.length();
        if dist < 1e-3 || self.radial_radius < 1e-3 {
            return grid;
        }
        let tangential = Vec2::new(-d.y, d.x) / dist; // perpendicular to radial → ring
        let w = (self.radial_strength
            * (-(dist * dist) / (self.radial_radius * self.radial_radius)).exp())
        .clamp(0.0, 1.0);
        (grid * (1.0 - w) + tangential * w).normalize_or_zero()
    }

    /// Unit minor direction (perpendicular to major).
    pub fn minor_dir(&self, p: Vec2) -> Vec2 {
        let m = self.major_dir(p);
        Vec2::new(-m.y, m.x)
    }
}

/// Trace the road network inside `[min, max]`. Seeds on a grid of `seed_spacing`, streamlines
/// stepped by `step` for up to `max_steps`, with `sep` separation between roads of the same
/// family. Major and minor families cross freely (separate occupancy). Deterministic.
pub fn generate_roads(
    field: &TensorField,
    min: Vec2,
    max: Vec2,
    seed_spacing: f32,
    step: f32,
    max_steps: u32,
    sep: f32,
) -> RoadNetwork {
    let seeds = grid_seeds(min, max, seed_spacing.max(1.0));
    let cell = sep.max(0.5);
    let mut out = Vec::new();
    let mut occ_major = HashSet::new();
    trace_family(field, &seeds, min, max, step, max_steps, cell, false, &mut occ_major, &mut out);
    let mut occ_minor = HashSet::new();
    trace_family(field, &seeds, min, max, step, max_steps, cell, true, &mut occ_minor, &mut out);
    RoadNetwork { segments: out }
}

/// Clean grid streets: one full-length road segment per grid line (vertical + horizontal) at
/// `spacing` intervals. Aligns with the parcel blocks (which tile at the same `spacing`), so the
/// road mesh draws crisp streets between blocks instead of overlapping streamline ribbons.
pub fn road_grid(min: Vec2, max: Vec2, spacing: f32) -> RoadNetwork {
    let s = spacing.max(1.0);
    let mut segments = Vec::new();
    let mut x = min.x;
    while x <= max.x + 1e-3 {
        segments.push(RoadSegment {
            a: Vec2::new(x, min.y),
            b: Vec2::new(x, max.y),
            kind: RoadKind::Minor,
        });
        x += s;
    }
    let mut z = min.y;
    while z <= max.y + 1e-3 {
        segments.push(RoadSegment {
            a: Vec2::new(min.x, z),
            b: Vec2::new(max.x, z),
            kind: RoadKind::Minor,
        });
        z += s;
    }
    RoadNetwork { segments }
}

fn grid_seeds(min: Vec2, max: Vec2, spacing: f32) -> Vec<Vec2> {
    let mut seeds = Vec::new();
    let mut y = min.y;
    while y <= max.y {
        let mut x = min.x;
        while x <= max.x {
            seeds.push(Vec2::new(x, y));
            x += spacing;
        }
        y += spacing;
    }
    seeds
}

#[inline]
fn cell_key(p: Vec2, cell: f32) -> (i32, i32) {
    ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32)
}

#[allow(clippy::too_many_arguments)]
fn trace_family(
    field: &TensorField,
    seeds: &[Vec2],
    min: Vec2,
    max: Vec2,
    step: f32,
    max_steps: u32,
    cell: f32,
    minor: bool,
    occ: &mut HashSet<(i32, i32)>,
    out: &mut Vec<RoadSegment>,
) {
    let kind = if minor { RoadKind::Minor } else { RoadKind::Major };
    for &seed in seeds {
        if occ.contains(&cell_key(seed, cell)) {
            continue; // this area is already served by a road of this family
        }
        for sign in [1.0f32, -1.0] {
            let mut p = seed;
            for _ in 0..max_steps {
                let dir = if minor {
                    field.minor_dir(p)
                } else {
                    field.major_dir(p)
                };
                if dir.length_squared() < 1e-6 {
                    break;
                }
                let np = p + dir * sign * step;
                if np.x < min.x || np.x > max.x || np.y < min.y || np.y > max.y {
                    break;
                }
                let k = cell_key(np, cell);
                if k != cell_key(p, cell) && occ.contains(&k) {
                    break; // hit another road of this family → stop (separation)
                }
                out.push(RoadSegment { a: p, b: np, kind });
                occ.insert(k);
                p = np;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_field() -> TensorField {
        TensorField {
            base_angle: 0.0,
            center: Vec2::ZERO,
            radial_strength: 0.0,
            radial_radius: 50.0,
        }
    }

    #[test]
    fn roads_are_deterministic_and_in_bounds() {
        let f = grid_field();
        let min = Vec2::splat(-60.0);
        let max = Vec2::splat(60.0);
        let a = generate_roads(&f, min, max, 30.0, 4.0, 64, 6.0);
        let b = generate_roads(&f, min, max, 30.0, 4.0, 64, 6.0);
        assert!(!a.segments.is_empty(), "should trace some roads");
        assert_eq!(a.segments.len(), b.segments.len(), "deterministic");
        for s in &a.segments {
            for p in [s.a, s.b] {
                assert!(p.x >= min.x - 4.1 && p.x <= max.x + 4.1, "x in bounds: {p:?}");
                assert!(p.y >= min.y - 4.1 && p.y <= max.y + 4.1, "y in bounds: {p:?}");
            }
        }
        assert!(a.segments.iter().any(|s| s.kind == RoadKind::Major));
        assert!(a.segments.iter().any(|s| s.kind == RoadKind::Minor));
    }

    #[test]
    fn radial_field_bends_direction() {
        let f = TensorField {
            base_angle: 0.0,
            center: Vec2::ZERO,
            radial_strength: 1.0,
            radial_radius: 40.0,
        };
        // On the +X axis the tangential component is along Y, so it rotates the major dir
        // away from pure +X near the center; far away the field relaxes back to the grid.
        let near = f.major_dir(Vec2::new(10.0, 0.0));
        let far = f.major_dir(Vec2::new(1000.0, 0.0));
        assert!((far.x - 1.0).abs() < 0.05, "far from center → grid (+X)");
        assert!(near.x.abs() < 0.9, "near center → bent away from pure +X");
        assert!(near.y.abs() > 0.1, "near center → gained a perpendicular component");
    }
}
