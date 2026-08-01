//! Shared Poisson disk sampling (Bridson's algorithm).
//!
//! Used by vegetation, grass, and villages for well-distributed point placement.
//! Certifié zone propre story-349 E1 : 5 proptests couvrent bounds, min_dist,
//! déterminisme, non-vide, scaling par aire.

// Story-674 — l'algorithme vit désormais dans `forgia-core::layout` : il a un
// SECOND consommateur (l'aménagement des arènes roguelite), et une crate gameplay
// n'a pas à dépendre de `forgia-terrain` pour une fonction de maths pure.
// Le chemin public `forgia_terrain::sampling::poisson_disk_sample` est conservé —
// `forgia-foliage` et les proptests ci-dessous n'ont pas bougé.
pub use forgia_core::layout::poisson_disk_sample;

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Reasonable area bounds. Wider ranges blow up proptest runtime for N² pair checks.
    fn arb_dim() -> impl Strategy<Value = f32> {
        16.0f32..128.0f32
    }

    /// Minimum distance must be small vs area (else no points fit) but not zero (div/0).
    fn arb_min_dist() -> impl Strategy<Value = f32> {
        0.5f32..8.0f32
    }

    proptest! {
        /// Every sampled point lies within [0, width] × [0, depth].
        #[test]
        fn prop_poisson_points_within_bounds(
            w in arb_dim(),
            d in arb_dim(),
            md in arb_min_dist(),
            seed: u64,
        ) {
            let pts = poisson_disk_sample(w, d, md, seed, 30);
            for (x, z) in &pts {
                prop_assert!(*x >= 0.0 && *x <= w, "x {x} outside [0, {w}]");
                prop_assert!(*z >= 0.0 && *z <= d, "z {z} outside [0, {d}]");
            }
        }

        #[test]
        fn prop_poisson_min_distance_respected(
            w in arb_dim(),
            d in arb_dim(),
            md in arb_min_dist(),
            seed: u64,
        ) {
            let pts = poisson_disk_sample(w, d, md, seed, 30);
            let n = pts.len().min(40);
            for i in 0..n {
                for j in (i+1)..n {
                    let dx = pts[i].0 - pts[j].0;
                    let dz = pts[i].1 - pts[j].1;
                    let dist2 = dx * dx + dz * dz;
                    prop_assert!(
                        dist2 >= md * md - 1e-3,
                        "points {:?} and {:?} are {:.3} apart, expected >= {}",
                        pts[i], pts[j], dist2.sqrt(), md
                    );
                }
            }
        }

        #[test]
        fn prop_poisson_deterministic(
            w in arb_dim(),
            d in arb_dim(),
            md in arb_min_dist(),
            seed: u64,
        ) {
            let a = poisson_disk_sample(w, d, md, seed, 30);
            let b = poisson_disk_sample(w, d, md, seed, 30);
            prop_assert_eq!(a.len(), b.len(), "length differs");
            if let (Some(p0a), Some(p0b)) = (a.first(), b.first()) {
                prop_assert_eq!(p0a, p0b, "first point differs");
            }
        }

        #[test]
        fn prop_poisson_nonempty(
            w in arb_dim(),
            d in arb_dim(),
            md in arb_min_dist(),
            seed: u64,
        ) {
            let pts = poisson_disk_sample(w, d, md, seed, 30);
            prop_assert!(!pts.is_empty(), "poisson should always place at least the initial seed point");
        }

        #[test]
        fn prop_poisson_density_scales_with_area(
            w in (20.0f32..60.0f32),
            d in (20.0f32..60.0f32),
            md in (1.0f32..4.0f32),
            seed: u64,
        ) {
            let small = poisson_disk_sample(w, d, md, seed, 30);
            let large = poisson_disk_sample(w * 2.0, d, md, seed, 30);
            let ratio = large.len() as f32 / small.len().max(1) as f32;
            prop_assert!(
                (0.5..=4.0).contains(&ratio),
                "doubling width scaled count by {ratio:.2}x, expected ~2x (tolerance [0.5, 4.0])"
            );
        }
    }
}
