//! Hex-grid layout primitive (story-578 — toolbox in real use: KayKit hexagonal village).
//!
//! Pure axial-coordinate hex math, no Bevy gameplay deps. A caller (the RPG village) maps each
//! [`Hex`] to a world XZ via [`Hex::to_world`] and spawns a tile / building there. The grid is
//! **pointy-top** with the vertices along **±Z** and the flat edges along **±X** — matching the
//! KayKit Medieval Hexagon tiles (native footprint X = 2.0 flat-to-flat, Z = 2.309 point-to-point).
//!
//! `size` everywhere is the **center-to-vertex** distance (for a regular hex, flat-to-flat =
//! `√3 · size`). For a KayKit tile rendered at scale `S`, pass `size = (2 / √3) · S`.

use bevy::prelude::Vec2;

/// An axial hex coordinate (`q`, `r`). The third cube coord is `s = -q - r`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Hex {
    pub q: i32,
    pub r: i32,
}

impl Hex {
    /// Center of the grid.
    pub const ZERO: Hex = Hex { q: 0, r: 0 };

    /// New axial coordinate.
    pub fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    /// World XZ position of this hex's center, pointy-top, vertices along ±Z.
    /// `size` = center-to-vertex distance of the rendered tile.
    pub fn to_world(self, size: f32) -> Vec2 {
        const SQRT3: f32 = 1.732_050_8;
        let x = size * (SQRT3 * self.q as f32 + SQRT3 * 0.5 * self.r as f32);
        let z = size * (1.5 * self.r as f32);
        Vec2::new(x, z)
    }

    /// Hex (ring) distance between two coordinates.
    pub fn distance(self, other: Hex) -> i32 {
        let dq = self.q - other.q;
        let dr = self.r - other.r;
        (dq.abs() + (dq + dr).abs() + dr.abs()) / 2
    }

    /// Ring index from the center (= `distance(ZERO)`).
    pub fn ring(self) -> i32 {
        self.distance(Hex::ZERO)
    }
}

/// The six axial-neighbour directions (pointy-top order, counter-clockwise).
const DIRECTIONS: [Hex; 6] = [
    Hex { q: 1, r: 0 },
    Hex { q: 1, r: -1 },
    Hex { q: 0, r: -1 },
    Hex { q: -1, r: 0 },
    Hex { q: -1, r: 1 },
    Hex { q: 0, r: 1 },
];

/// All hexes exactly `radius` rings from the center (`6·radius` hexes; `radius 0` → just center).
pub fn hex_ring(radius: i32) -> Vec<Hex> {
    if radius <= 0 {
        return vec![Hex::ZERO];
    }
    let mut out = Vec::with_capacity(6 * radius as usize);
    // Start at the corner `radius` steps in direction 4, then walk the 6 edges.
    let mut h = Hex::new(
        Hex::ZERO.q + DIRECTIONS[4].q * radius,
        Hex::ZERO.r + DIRECTIONS[4].r * radius,
    );
    for dir in DIRECTIONS {
        for _ in 0..radius {
            out.push(h);
            h = Hex::new(h.q + dir.q, h.r + dir.r);
        }
    }
    out
}

/// Center hex + every ring out to `radius` (a filled hexagon), center-first then ring by ring.
/// Total = `1 + 3·radius·(radius+1)` hexes.
pub fn hex_spiral(radius: i32) -> Vec<Hex> {
    let mut out = vec![Hex::ZERO];
    for r in 1..=radius.max(0) {
        out.extend(hex_ring(r));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_at_origin() {
        assert_eq!(Hex::ZERO.to_world(3.0), Vec2::ZERO);
        assert_eq!(Hex::ZERO.ring(), 0);
    }

    #[test]
    fn ring_counts() {
        assert_eq!(hex_ring(0).len(), 1);
        assert_eq!(hex_ring(1).len(), 6);
        assert_eq!(hex_ring(2).len(), 12);
        assert_eq!(hex_ring(3).len(), 18);
    }

    #[test]
    fn spiral_counts() {
        // 1 + 3R(R+1)
        assert_eq!(hex_spiral(0).len(), 1);
        assert_eq!(hex_spiral(1).len(), 7);
        assert_eq!(hex_spiral(2).len(), 19);
        assert_eq!(hex_spiral(3).len(), 37);
        assert_eq!(hex_spiral(4).len(), 61);
    }

    #[test]
    fn ring_hexes_have_correct_distance() {
        for r in 0..=4 {
            for h in hex_ring(r) {
                assert_eq!(h.ring(), r, "hex {h:?} should be on ring {r}");
            }
        }
    }

    #[test]
    fn neighbours_are_distance_one() {
        for d in DIRECTIONS {
            assert_eq!(Hex::ZERO.distance(d), 1);
        }
    }

    #[test]
    fn tessellation_spacing_matches_tile_width() {
        // size = 2/√3 → flat-to-flat = √3·size = 2.0 (native KayKit tile width).
        let size = 2.0 / 3.0_f32.sqrt();
        // Neighbour (1,0) sits exactly one flat-to-flat width away along X.
        let n = Hex::new(1, 0).to_world(size);
        assert!((n.x - 2.0).abs() < 1e-4, "x spacing {} != 2.0", n.x);
        assert!(n.y.abs() < 1e-4, "z should be 0 for (1,0), got {}", n.y);
        // Row neighbour (0,1): offset half a width in X, 1.5·size in Z.
        let row = Hex::new(0, 1).to_world(size);
        assert!((row.x - 1.0).abs() < 1e-4, "row x {} != 1.0", row.x);
        assert!((row.y - 1.5 * size).abs() < 1e-4);
    }

    #[test]
    fn spiral_has_no_duplicates() {
        let s = hex_spiral(4);
        let mut seen = std::collections::HashSet::new();
        for h in &s {
            assert!(seen.insert(*h), "duplicate hex {h:?} in spiral");
        }
    }
}
