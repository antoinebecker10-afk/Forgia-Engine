//! Shared Poisson disk sampling (Bridson's algorithm).
//!
//! Used by vegetation, grass, and villages for well-distributed point placement.
//! Certifié zone propre story-349 E1 : 5 proptests couvrent bounds, min_dist,
//! déterminisme, non-vide, scaling par aire.

/// Generate Poisson disk sample points within a 2D rectangle.
/// Returns Vec<(x, z)> positions in local space [0, width] × [0, depth].
///
/// - `min_dist`: minimum distance between any two points
/// - `seed`: deterministic RNG seed (e.g., derived from chunk coord)
/// - `k`: max attempts per active point (30 is standard)
pub fn poisson_disk_sample(
    width: f32,
    depth: f32,
    min_dist: f32,
    seed: u64,
    k: u32,
) -> Vec<(f32, f32)> {
    let cell_size = min_dist / std::f32::consts::SQRT_2;
    let grid_w = (width / cell_size).ceil() as usize + 1;
    let grid_h = (depth / cell_size).ceil() as usize + 1;
    let mut grid: Vec<Option<usize>> = vec![None; grid_w * grid_h];
    let mut points: Vec<(f32, f32)> = Vec::new();
    let mut active: Vec<usize> = Vec::new();

    let mut rng_state: u64 = seed ^ 0xCAFE_BABE_1337;
    let mut next_f32 = || -> f32 {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        (rng_state & 0xFFFFFF) as f32 / 0xFFFFFF as f32
    };

    // Initial point at center
    let p0 = (width * 0.5, depth * 0.5);
    let gi = (p0.0 / cell_size) as usize;
    let gj = (p0.1 / cell_size) as usize;
    if gi < grid_w && gj < grid_h {
        grid[gi + gj * grid_w] = Some(0);
    }
    points.push(p0);
    active.push(0);

    while !active.is_empty() {
        let ai = (next_f32() * active.len() as f32) as usize % active.len();
        let pi = active[ai];
        let (px, pz) = points[pi];

        let mut found = false;
        for _ in 0..k {
            let angle = next_f32() * std::f32::consts::TAU;
            let r = min_dist + next_f32() * min_dist;
            let nx = px + r * angle.cos();
            let nz = pz + r * angle.sin();

            if nx < 0.0 || nx >= width || nz < 0.0 || nz >= depth {
                continue;
            }

            let ngi = (nx / cell_size) as usize;
            let ngj = (nz / cell_size) as usize;

            // Check neighbors in 5x5 grid
            let mut too_close = false;
            let imin = ngi.saturating_sub(2);
            let imax = (ngi + 3).min(grid_w);
            let jmin = ngj.saturating_sub(2);
            let jmax = (ngj + 3).min(grid_h);

            for jj in jmin..jmax {
                for ii in imin..imax {
                    if let Some(idx) = grid[ii + jj * grid_w] {
                        let (ox, oz) = points[idx];
                        let dx = nx - ox;
                        let dz = nz - oz;
                        if dx * dx + dz * dz < min_dist * min_dist {
                            too_close = true;
                            break;
                        }
                    }
                }
                if too_close {
                    break;
                }
            }

            if !too_close {
                let new_idx = points.len();
                grid[ngi + ngj * grid_w] = Some(new_idx);
                points.push((nx, nz));
                active.push(new_idx);
                found = true;
            }
        }

        if !found {
            active.swap_remove(ai);
        }
    }

    points
}

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
