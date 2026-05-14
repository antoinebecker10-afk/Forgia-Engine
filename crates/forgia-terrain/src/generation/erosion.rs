//! Couche 3 — Erosion hydraulique cellule-par-cellule.
//!
//! Simule un ecoulement simplifie sur la heightmap :
//! - `valley_carve` : creuse les bas-fonds et lisse les cuvettes
//! - `erode_heightmap_variable` : diffusion outflow → in neighbour cells,
//!   avec rate variable par cellule (pour supporter les transitions biomes)
//! - `slope_limit_variable` : clamp two-pass forward+backward
//! - `save_padding_ring` / `restore_padding_ring` : preservent la bordure
//!   deterministe pour les chunks voisins (seamless meshing)

use crate::biomes::BiomeType;
use super::BiomeGenomeOverrides;

/// Valley carving pass: deepens low areas by following steepest-descent gradients.
/// Reduces micro-roughness in valley floors for smoother ground.
pub(super) fn valley_carve(heights: &mut [f32], w: usize, d: usize, sea_level: f32) {
    let snapshot: Vec<f32> = heights.to_vec();
    let valley_threshold = sea_level + 5.0;

    // Skip padding ring — only modify internal voxels to preserve chunk boundary continuity.
    // Padding is READ for neighbor lookups but never WRITTEN.
    for z in 1..d - 1 {
        for x in 1..w - 1 {
            let idx = x + w * z;
            let center = snapshot[idx];

            // All 4 neighbors guaranteed valid (we skip padding ring)
            let nn = snapshot[idx - w];
            let ns = snapshot[idx + w];
            let nw = snapshot[idx - 1];
            let ne = snapshot[idx + 1];

            // Near valley floor: reduce micro-roughness (smooth towards neighbors)
            if center < valley_threshold {
                let avg = (nn + ns + nw + ne) * 0.25;
                let blend = ((valley_threshold - center) / 5.0).clamp(0.0, 0.5);
                heights[idx] = center * (1.0 - blend) + avg * blend;
            }

            // Steepest-descent carving: deepen channels where gradient is strong
            // AAA: 5x more aggressive for visible drainage networks
            let min_neighbor = nn.min(ns).min(nw).min(ne);
            let drop = center - min_neighbor;
            if drop > 1.5 {
                heights[idx] -= (drop - 1.5) * 0.12; // was 0.03 threshold 2.0
            }
        }
    }
}

/// Per-biome erosion parameters (passes, rate).
/// Reads from genome overrides if available, falls back to hardcoded defaults.
pub(super) fn erosion_params(biome: BiomeType, overrides: Option<&BiomeGenomeOverrides>) -> (usize, f32) {
    if let Some(ovr) = overrides {
        if let Some(params) = ovr.erosion[(biome as u8 as usize).min(9)] {
            return params;
        }
    }
    // AAA erosion: more passes + higher rates for dramatic valleys/drainage
    match biome {
        BiomeType::Plains   => (2, 0.06),  // gentle rolling drainage (was 1, 0.04)
        BiomeType::Forest   => (2, 0.08),  // forest gulches (was 1, 0.05)
        BiomeType::Desert   => (1, 0.03),  // wind-smoothed (was 1, 0.02)
        BiomeType::Mountain => (3, 0.10),  // deep V-cuts on slopes (was 1, 0.03)
        BiomeType::Swamp    => (2, 0.08),  // water channels (was 1, 0.06)
        BiomeType::Tundra   => (1, 0.02),  // glacial, still flat (was 1, 0.01)
        BiomeType::Savanna  => (1, 0.04),  // dry washes (was 1, 0.02)
        BiomeType::Jungle   => (3, 0.12),  // deep ravines (was 2, 0.07)
        BiomeType::Volcanic => (2, 0.05),  // lava channels (was 1, 0.01)
        BiomeType::Canyon   => (4, 0.15),  // deep incision, the most eroded (was 2, 0.08)
    }
}

/// Per-biome maximum slope for slope limiting.
/// Reads from genome overrides (BiomeSpec::slope_max) if available, falls back to hardcoded defaults.
pub(super) fn slope_max_for_biome(biome: BiomeType, overrides: Option<&BiomeGenomeOverrides>) -> f32 {
    if let Some(ovr) = overrides {
        if let Some(v) = ovr.slope_max[(biome as u8 as usize).min(9)] {
            return v;
        }
    }
    match biome {
        BiomeType::Mountain => 3.0,  // steep cliffs allowed (was 2.2)
        BiomeType::Canyon => 3.2,    // sheer canyon walls
        BiomeType::Volcanic => 2.8,  // caldera rim steepness
        BiomeType::Jungle => 2.4,    // ravine walls
        _ => 2.0,                    // was 1.8
    }
}

/// Save the 1-cell padding ring around the heightmap buffer.
/// Used to restore deterministic boundary values after local erosion.
pub(super) fn save_padding_ring(heights: &[f32], w: usize, d: usize) -> Vec<(usize, f32)> {
    let mut ring = Vec::with_capacity(2 * (w + d));
    for x in 0..w {
        // Top row (pz=0) and bottom row (pz=d-1)
        ring.push((x, heights[x]));
        ring.push((x + w * (d - 1), heights[x + w * (d - 1)]));
    }
    for z in 1..d - 1 {
        // Left col (px=0) and right col (px=w-1)
        ring.push((w * z, heights[w * z]));
        ring.push((w - 1 + w * z, heights[w - 1 + w * z]));
    }
    ring
}

/// Restore the padding ring saved by save_padding_ring.
pub(super) fn restore_padding_ring(heights: &mut [f32], _w: usize, _d: usize, ring: &[(usize, f32)]) {
    for &(idx, val) in ring {
        heights[idx] = val;
    }
}

/// Erosion with per-cell rates — eliminates seams at biome chunk boundaries.
pub(super) fn erode_heightmap_variable(
    heights: &mut [f32],
    w: usize, d: usize,
    passes: usize,
    rates: &[f32],
) {
    for _ in 0..passes {
        let snapshot: Vec<f32> = heights.to_vec();

        // Skip padding ring (x=0, x=w-1, z=0, z=d-1) — only erode internal voxels.
        // Padding must stay deterministic for seamless chunk boundaries.
        for z in 1..d - 1 {
            for x in 1..w - 1 {
                let idx = x + w * z;
                let center = snapshot[idx];
                let rate = rates[idx];

                let mut outflow = 0.0_f32;
                let mut neighbor_data: [(usize, f32); 4] = [(0, 0.0); 4];

                let n = snapshot[idx - w]; neighbor_data[0] = (idx - w, n); if n < center { outflow += center - n; }
                let s = snapshot[idx + w]; neighbor_data[1] = (idx + w, s); if s < center { outflow += center - s; }
                let ww = snapshot[idx - 1]; neighbor_data[2] = (idx - 1, ww); if ww < center { outflow += center - ww; }
                let e = snapshot[idx + 1]; neighbor_data[3] = (idx + 1, e); if e < center { outflow += center - e; }

                if outflow > 0.01 {
                    heights[idx] -= outflow * rate;

                    for &(nidx, nval) in &neighbor_data {
                        if nval < center {
                            let share = (center - nval) / outflow;
                            // Only deposit to internal voxels, not padding
                            let nx = nidx % w;
                            let nz = nidx / w;
                            if nx > 0 && nx < w - 1 && nz > 0 && nz < d - 1 {
                                heights[nidx] += outflow * rate * share * 0.5;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Slope limiting with per-cell max slope — eliminates seams at biome chunk boundaries.
/// Skips padding ring to preserve deterministic chunk boundaries.
pub(super) fn slope_limit_variable(heights: &mut [f32], w: usize, d: usize, max_slopes: &[f32], passes: usize) {
    for _ in 0..passes {
        // Forward pass — skip padding (start at 1, end at w-2/d-2)
        for z in 1..d - 1 {
            for x in 1..w - 1 {
                let idx = x + w * z;
                let ms = max_slopes[idx];
                let cap = heights[idx - 1].min(heights[idx - w]) + ms;
                if heights[idx] > cap {
                    heights[idx] = cap;
                }
            }
        }
        // Backward pass — skip padding
        for z in (1..d - 1).rev() {
            for x in (1..w - 1).rev() {
                let idx = x + w * z;
                let ms = max_slopes[idx];
                let cap = heights[idx + 1].min(heights[idx + w]) + ms;
                if heights[idx] > cap {
                    heights[idx] = cap;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erosion_params_default_for_canyon_is_most_eroded() {
        let (passes, rate) = erosion_params(BiomeType::Canyon, None);
        assert!(passes >= 4, "Canyon should have the most erosion passes");
        assert!(rate >= 0.10, "Canyon should have highest rate");
    }

    #[test]
    fn slope_max_descends_cliffs_allowed_higher() {
        // Mountain/Canyon/Volcanic/Jungle allow steeper slopes than default 2.0
        assert!(slope_max_for_biome(BiomeType::Mountain, None) > slope_max_for_biome(BiomeType::Plains, None));
        assert!(slope_max_for_biome(BiomeType::Canyon, None) > slope_max_for_biome(BiomeType::Forest, None));
    }

    /// Padding ring round-trip : save + mutate interior + restore should
    /// preserve the ring exactly. Guaranty pour le seamless meshing.
    #[test]
    fn padding_ring_round_trip_preserves_border() {
        let w = 5;
        let d = 5;
        let original: Vec<f32> = (0..(w * d)).map(|i| i as f32).collect();
        let mut h = original.clone();

        let ring = save_padding_ring(&h, w, d);
        // Zap everything (including ring).
        for v in h.iter_mut() { *v = -999.0; }
        // Restore ring : centre reste zapped, bord revient.
        restore_padding_ring(&mut h, w, d, &ring);

        // Top/bottom rows
        for x in 0..w {
            assert_eq!(h[x], original[x], "top row px={x} differs");
            assert_eq!(h[x + w * (d - 1)], original[x + w * (d - 1)],
                       "bottom row px={x} differs");
        }
        // Left/right cols
        for z in 1..d - 1 {
            assert_eq!(h[w * z], original[w * z], "left col pz={z} differs");
            assert_eq!(h[w - 1 + w * z], original[w - 1 + w * z],
                       "right col pz={z} differs");
        }
    }

    /// Extract the 1-cell padding ring of a height buffer as a flat Vec.
    fn extract_padding_ring(h: &[f32], w: usize, d: usize) -> Vec<f32> {
        let mut r = Vec::new();
        for x in 0..w { r.push(h[x]); r.push(h[x + w * (d - 1)]); }
        for z in 1..d - 1 { r.push(h[w * z]); r.push(h[w - 1 + w * z]); }
        r
    }

    #[test]
    fn erosion_never_touches_padding_ring() {
        let w = 5;
        let d = 5;
        let mut h: Vec<f32> = (0..(w * d)).map(|i| (i as f32) * 10.0).collect();
        let original_ring = extract_padding_ring(&h, w, d);

        let rates = vec![0.5_f32; w * d];
        erode_heightmap_variable(&mut h, w, d, 3, &rates);

        let new_ring = extract_padding_ring(&h, w, d);
        for (i, (o, n)) in original_ring.iter().zip(new_ring.iter()).enumerate() {
            assert_eq!(o, n, "ring[{i}] changed by erosion: {o} -> {n}");
        }
    }

    #[test]
    fn valley_carve_never_touches_padding_ring() {
        let w = 7;
        let d = 7;
        let mut h = vec![20.0_f32; w * d];
        h[3 * w + 3] = 80.0;
        for z in 1..d - 1 {
            for x in 1..w - 1 {
                if (x as i32 - 3).abs() + (z as i32 - 3).abs() == 3 {
                    h[x + w * z] = 2.0;
                }
            }
        }
        let original_ring = extract_padding_ring(&h, w, d);
        valley_carve(&mut h, w, d, 5.0);
        let new_ring = extract_padding_ring(&h, w, d);
        for (i, (o, n)) in original_ring.iter().zip(new_ring.iter()).enumerate() {
            assert_eq!(o, n, "ring[{i}] changed by valley_carve: {o} -> {n}");
        }
    }

    #[test]
    fn slope_limit_variable_never_touches_padding_ring() {
        let w = 6;
        let d = 6;
        let mut h: Vec<f32> = (0..(w * d)).map(|i| (i as f32) * 5.0).collect();
        let original_ring = extract_padding_ring(&h, w, d);
        let max_slopes = vec![0.0_f32; w * d];
        slope_limit_variable(&mut h, w, d, &max_slopes, 4);
        let new_ring = extract_padding_ring(&h, w, d);
        for (i, (o, n)) in original_ring.iter().zip(new_ring.iter()).enumerate() {
            assert_eq!(o, n, "ring[{i}] changed by slope_limit: {o} -> {n}");
        }
    }

    #[test]
    fn valley_carve_never_raises_above_original() {
        let w = 5;
        let d = 5;
        let mut h = vec![20.0_f32; w * d];
        h[12] = 50.0;
        let original_center = h[12];
        valley_carve(&mut h, w, d, 5.0);
        assert!(h[12] <= original_center,
                "valley_carve raised central peak from {original_center} to {}", h[12]);
    }

    #[test]
    fn slope_limit_zero_flattens_interior() {
        let w = 5;
        let d = 5;
        let mut h: Vec<f32> = (0..(w * d)).map(|i| i as f32).collect();
        let max_slopes = vec![0.0_f32; w * d];
        slope_limit_variable(&mut h, w, d, &max_slopes, 5);
        for z in 1..d - 1 {
            for x in 1..w - 1 {
                let idx = x + w * z;
                let cap = h[idx - 1].min(h[idx - w]);
                assert!(h[idx] <= cap + 1e-5,
                        "cell {idx} = {} > neighbour cap {cap}", h[idx]);
            }
        }
    }
}
