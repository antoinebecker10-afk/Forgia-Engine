//! Parcels — blocks subdivided into convex lots (story-578 P3).
//!
//! The city is tiled into blocks (the cells between major roads); each block is recursively
//! **subdivided along its longest side** until lots reach a target area. Blocks are rectangles,
//! so every lot stays convex — the simple, robust core of recursive block subdivision (the
//! classic parcel technique). Grammar-driven footprints land in P5. Pure 2D geometry, no ECS.

use bevy::math::Vec2;

/// A convex lot: its polygon (rectangle corners, CCW) and centroid.
#[derive(Clone, Debug)]
pub struct Parcel {
    pub polygon: Vec<Vec2>,
    pub center: Vec2,
}

/// Tile `[min, max]` into `block_size` blocks, inset each by `inset` (road margin) and
/// subdivide it into lots of ~`target_area`. Deterministic.
pub fn generate_parcels(
    min: Vec2,
    max: Vec2,
    block_size: f32,
    inset: f32,
    target_area: f32,
    max_depth: u32,
) -> Vec<Parcel> {
    let bs = block_size.max(2.0);
    let mut out = Vec::new();
    let mut y = min.y;
    while y + bs <= max.y + 1e-3 {
        let mut x = min.x;
        while x + bs <= max.x + 1e-3 {
            let a = Vec2::new(x + inset, y + inset);
            let b = Vec2::new(x + bs - inset, y + inset);
            let c = Vec2::new(x + bs - inset, y + bs - inset);
            let d = Vec2::new(x + inset, y + bs - inset);
            subdivide_rect([a, b, c, d], target_area.max(1.0), max_depth, &mut out);
            x += bs;
        }
        y += bs;
    }
    out
}

/// Recursively split a rectangle (4 CCW corners) along its longest side until each lot's area
/// is `<= target_area` (or `max_depth` is reached). Always produces convex rectangles.
pub fn subdivide_rect(corners: [Vec2; 4], target_area: f32, max_depth: u32, out: &mut Vec<Parcel>) {
    let area = rect_area(&corners);
    if area <= target_area || max_depth == 0 {
        out.push(Parcel {
            center: centroid(&corners),
            polygon: corners.to_vec(),
        });
        return;
    }
    let e01 = corners[0].distance(corners[1]); // edge 0→1
    let e12 = corners[1].distance(corners[2]); // edge 1→2
    if e01 >= e12 {
        // Split across edges 0→1 and 3→2 (halve the longer pair).
        let m01 = corners[0].midpoint(corners[1]);
        let m32 = corners[3].midpoint(corners[2]);
        subdivide_rect(
            [corners[0], m01, m32, corners[3]],
            target_area,
            max_depth - 1,
            out,
        );
        subdivide_rect(
            [m01, corners[1], corners[2], m32],
            target_area,
            max_depth - 1,
            out,
        );
    } else {
        // Split across edges 1→2 and 0→3.
        let m12 = corners[1].midpoint(corners[2]);
        let m03 = corners[0].midpoint(corners[3]);
        subdivide_rect(
            [corners[0], corners[1], m12, m03],
            target_area,
            max_depth - 1,
            out,
        );
        subdivide_rect(
            [m03, m12, corners[2], corners[3]],
            target_area,
            max_depth - 1,
            out,
        );
    }
}

/// Area of a rectangle given as 4 ordered corners.
fn rect_area(c: &[Vec2; 4]) -> f32 {
    c[0].distance(c[1]) * c[1].distance(c[2])
}

/// Centroid of a set of points.
fn centroid(pts: &[Vec2]) -> Vec2 {
    if pts.is_empty() {
        return Vec2::ZERO;
    }
    let sum: Vec2 = pts.iter().copied().sum();
    sum / pts.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subdivision_conserves_area_and_bounds_lot_size() {
        let block = [
            Vec2::new(0.0, 0.0),
            Vec2::new(24.0, 0.0),
            Vec2::new(24.0, 24.0),
            Vec2::new(0.0, 24.0),
        ];
        let mut out = Vec::new();
        subdivide_rect(block, 100.0, 8, &mut out);
        assert!(out.len() > 1, "576 m² block should split for 100 m² target");
        let total: f32 = out.iter().map(|p| rect_area(&to4(&p.polygon))).sum();
        assert!((total - 576.0).abs() < 1.0, "lots tile the block: {total}");
        for p in &out {
            assert!(
                rect_area(&to4(&p.polygon)) <= 100.0 + 1e-2,
                "lot under target area"
            );
            assert_eq!(p.polygon.len(), 4, "lots stay rectangular");
        }
    }

    #[test]
    fn parcels_are_deterministic() {
        let min = Vec2::splat(-30.0);
        let max = Vec2::splat(30.0);
        let a = generate_parcels(min, max, 30.0, 3.0, 120.0, 8);
        let b = generate_parcels(min, max, 30.0, 3.0, 120.0, 8);
        assert!(!a.is_empty());
        assert_eq!(a.len(), b.len());
        for (pa, pb) in a.iter().zip(&b) {
            assert!(pa.center.distance(pb.center) < 1e-4);
        }
    }

    #[test]
    fn smaller_target_makes_more_lots() {
        let min = Vec2::splat(-30.0);
        let max = Vec2::splat(30.0);
        let coarse = generate_parcels(min, max, 30.0, 3.0, 300.0, 8).len();
        let fine = generate_parcels(min, max, 30.0, 3.0, 80.0, 8).len();
        assert!(
            fine > coarse,
            "smaller target → more parcels ({fine} vs {coarse})"
        );
    }

    fn to4(poly: &[Vec2]) -> [Vec2; 4] {
        [poly[0], poly[1], poly[2], poly[3]]
    }
}
