//! Droplet erosion (Houdini-style) — particle-based hydraulic + thermal erosion.
//!
//! Simulates water droplets flowing downhill (Hans Theobald Beyer / Lague algo)
//! and talus/scree falling from steep slopes. Produces natural ravines, drainage
//! networks, sediment fans, and cliff-base scree.
//!
//! Pipeline order (dans `chunk_sdf`) :
//! 1. `droplet_erosion_params` : water-flow shaping, ~N droplets per chunk
//! 2. `thermal_erosion` : talus stabilisation, `talus_angle` param

/// Genome-driven hydraulic erosion parameters.
/// All fields map to genes in `erosion_advanced.toml`.
#[derive(Clone, Debug)]
pub struct HydroErosionParams {
    /// Droplet inertia: 0=follow gradient exactly, 1=keep old direction.
    pub inertia: f32,
    /// Sediment capacity multiplier.
    pub capacity_mult: f32,
    /// How fast sediment deposits when capacity exceeded.
    pub deposit_speed: f32,
    /// How fast terrain erodes when capacity available.
    pub erode_speed: f32,
    /// Water evaporation per step.
    pub evaporate: f32,
    /// Gravity influence on speed.
    pub gravity: f32,
    /// Max lifetime per droplet (steps).
    pub max_steps: usize,
    /// Minimum slope to prevent flat-area explosion.
    pub min_slope: f32,
}

impl Default for HydroErosionParams {
    fn default() -> Self {
        Self {
            inertia: 0.05,
            capacity_mult: 4.0,
            deposit_speed: 0.3,
            erode_speed: 0.3,
            evaporate: 0.01,
            gravity: 4.0,
            max_steps: 64,
            min_slope: 0.01,
        }
    }
}

/// Hydraulic droplet erosion with default params — kept for API compatibility.
/// Quick one-shot erosion wrapper for tests. Pipeline uses `droplet_erosion_params` directly.
#[cfg(test)]
pub(super) fn droplet_erosion(
    heights: &mut [f32],
    w: usize, d: usize,
    num_droplets: usize,
    seed: u32,
) {
    droplet_erosion_params(heights, w, d, num_droplets, seed, &HydroErosionParams::default());
}

/// Hydraulic droplet erosion with genome-driven parameters (Beyer/Lague algorithm).
pub(super) fn droplet_erosion_params(
    heights: &mut [f32],
    w: usize, d: usize,
    num_droplets: usize,
    seed: u32,
    params: &HydroErosionParams,
) {

    let mut rng = u64::from(seed) ^ 0xABCD_EF01;
    let next_f32 = |state: &mut u64| -> f32 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state & 0xFFFFFF) as f32 / 0xFFFFFF as f32
    };

    for _ in 0..num_droplets {
        let mut pos_x = 1.0 + next_f32(&mut rng) * (w as f32 - 3.0);
        let mut pos_z = 1.0 + next_f32(&mut rng) * (d as f32 - 3.0);
        let mut dir_x = 0.0_f32;
        let mut dir_z = 0.0_f32;
        let mut speed = 1.0_f32;
        let mut water = 1.0_f32;
        let mut sediment = 0.0_f32;

        for _ in 0..params.max_steps {
            let ix = pos_x as usize;
            let iz = pos_z as usize;

            if ix < 1 || ix >= w - 2 || iz < 1 || iz >= d - 2 {
                break;
            }

            let idx = ix + w * iz;

            let fx = pos_x - ix as f32;
            let fz = pos_z - iz as f32;
            let h00 = heights[idx];
            let h10 = heights[idx + 1];
            let h01 = heights[idx + w];
            let h11 = heights[idx + 1 + w];

            let grad_x = (h10 - h00) * (1.0 - fz) + (h11 - h01) * fz;
            let grad_z = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;

            dir_x = dir_x * params.inertia - grad_x * (1.0 - params.inertia);
            dir_z = dir_z * params.inertia - grad_z * (1.0 - params.inertia);

            let dir_len = (dir_x * dir_x + dir_z * dir_z).sqrt();
            if dir_len < 0.0001 {
                dir_x = next_f32(&mut rng) - 0.5;
                dir_z = next_f32(&mut rng) - 0.5;
                let len = (dir_x * dir_x + dir_z * dir_z).sqrt().max(0.001);
                dir_x /= len;
                dir_z /= len;
            } else {
                dir_x /= dir_len;
                dir_z /= dir_len;
            }

            let new_x = pos_x + dir_x;
            let new_z = pos_z + dir_z;

            let nix = new_x as usize;
            let niz = new_z as usize;
            if nix < 1 || nix >= w - 2 || niz < 1 || niz >= d - 2 {
                break;
            }

            let nfx = new_x - nix as f32;
            let nfz = new_z - niz as f32;
            let nidx = nix + w * niz;
            let new_h = heights[nidx] * (1.0 - nfx) * (1.0 - nfz)
                      + heights[nidx + 1] * nfx * (1.0 - nfz)
                      + heights[nidx + w] * (1.0 - nfx) * nfz
                      + heights[nidx + 1 + w] * nfx * nfz;
            let old_h = h00 * (1.0 - fx) * (1.0 - fz)
                      + h10 * fx * (1.0 - fz)
                      + h01 * (1.0 - fx) * fz
                      + h11 * fx * fz;

            let height_diff = new_h - old_h;

            let capacity = (-height_diff).max(params.min_slope) * speed * water * params.capacity_mult;

            if sediment > capacity || height_diff > 0.0 {
                let deposit = if height_diff > 0.0 {
                    height_diff.min(sediment)
                } else {
                    (sediment - capacity) * params.deposit_speed
                };
                sediment -= deposit;
                heights[idx]         += deposit * (1.0 - fx) * (1.0 - fz);
                heights[idx + 1]     += deposit * fx * (1.0 - fz);
                heights[idx + w]     += deposit * (1.0 - fx) * fz;
                heights[idx + 1 + w] += deposit * fx * fz;
            } else {
                let erode = ((capacity - sediment) * params.erode_speed).min(-height_diff);
                sediment += erode;
                heights[idx]         -= erode * (1.0 - fx) * (1.0 - fz);
                heights[idx + 1]     -= erode * fx * (1.0 - fz);
                heights[idx + w]     -= erode * (1.0 - fx) * fz;
                heights[idx + 1 + w] -= erode * fx * fz;
            }

            speed = (speed * speed + height_diff * params.gravity).abs().sqrt();
            water *= 1.0 - params.evaporate;
            pos_x = new_x;
            pos_z = new_z;

            if water < 0.01 { break; }
        }
    }
}

/// Thermal erosion — material falls from steep slopes to lower neighbors.
/// Creates natural talus/scree at cliff bases (Houdini: heightfield erode thermal).
pub(super) fn thermal_erosion(
    heights: &mut [f32],
    w: usize, d: usize,
    passes: usize,
    talus_angle: f32,
) {
    for _ in 0..passes {
        let snapshot: Vec<f32> = heights.to_vec();
        for z in 1..d - 1 {
            for x in 1..w - 1 {
                let idx = x + w * z;
                let center = snapshot[idx];

                let neighbors = [idx - w, idx + w, idx - 1, idx + 1];
                let mut total_excess = 0.0_f32;
                let mut excess_count = 0;

                for &nidx in &neighbors {
                    let diff = center - snapshot[nidx];
                    if diff > talus_angle {
                        total_excess += diff - talus_angle;
                        excess_count += 1;
                    }
                }

                if excess_count > 0 {
                    let transfer = total_excess * 0.25;
                    heights[idx] -= transfer;
                    for &nidx in &neighbors {
                        let diff = center - snapshot[nidx];
                        if diff > talus_angle {
                            let share = (diff - talus_angle) / total_excess;
                            let nx = nidx % w;
                            let nz = nidx / w;
                            if nx > 0 && nx < w - 1 && nz > 0 && nz < d - 1 {
                                heights[nidx] += transfer * share;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hydro_erosion_params_default_sane() {
        let p = HydroErosionParams::default();
        assert!(p.inertia >= 0.0 && p.inertia <= 1.0);
        assert!(p.capacity_mult > 0.0);
        assert!(p.deposit_speed > 0.0 && p.erode_speed > 0.0);
        assert!(p.evaporate >= 0.0 && p.evaporate <= 0.1);
        assert!(p.gravity > 0.0);
        assert!(p.max_steps > 0);
        assert!(p.min_slope > 0.0, "min_slope == 0 cause flat-area explosion");
    }

    #[test]
    fn droplet_erosion_on_flat_terrain_stays_bounded() {
        let w = 8;
        let d = 8;
        let mut h = vec![50.0_f32; w * d];
        droplet_erosion(&mut h, w, d, 20, 42);
        for v in &h {
            assert!(v.is_finite(), "flat-terrain droplet produced NaN");
            assert!((*v - 50.0).abs() < 10.0, "flat-terrain droplet drifted too far: {v}");
        }
    }

    #[test]
    fn thermal_erosion_high_talus_is_noop() {
        let w = 5;
        let d = 5;
        let original: Vec<f32> = (0..(w * d)).map(|i| (i as f32) * 10.0).collect();
        let mut h = original.clone();
        thermal_erosion(&mut h, w, d, 3, 1e6);
        assert_eq!(h, original, "thermal erosion with huge talus should be no-op");
    }

    #[test]
    fn thermal_erosion_flattens_isolated_peak() {
        let w = 5;
        let d = 5;
        let mut h = vec![10.0_f32; w * d];
        h[12] = 100.0;
        let original_center = h[12];
        thermal_erosion(&mut h, w, d, 10, 1.0);
        assert!(h[12] < original_center, "peak should erode : {} -> {}", original_center, h[12]);
    }

    fn extract_padding_ring(h: &[f32], w: usize, d: usize) -> Vec<f32> {
        let mut r = Vec::new();
        for x in 0..w { r.push(h[x]); r.push(h[x + w * (d - 1)]); }
        for z in 1..d - 1 { r.push(h[w * z]); r.push(h[w - 1 + w * z]); }
        r
    }

    #[test]
    fn thermal_erosion_never_touches_padding_ring() {
        let w = 7;
        let d = 7;
        let mut h = vec![10.0_f32; w * d];
        h[3 * w + 3] = 200.0;
        h[2 * w + 2] = 1.0;
        h[4 * w + 4] = 1.0;
        let original_ring = extract_padding_ring(&h, w, d);
        thermal_erosion(&mut h, w, d, 5, 0.5);
        let new_ring = extract_padding_ring(&h, w, d);
        for (i, (o, n)) in original_ring.iter().zip(new_ring.iter()).enumerate() {
            assert_eq!(o, n, "ring[{i}] changed by thermal_erosion: {o} -> {n}");
        }
    }

    #[test]
    fn droplet_erosion_never_touches_padding_ring() {
        let w = 10;
        let d = 10;
        let mut h: Vec<f32> = (0..(w * d)).map(|i| {
            let x = i % w;
            let z = i / w;
            (x + z) as f32 * 20.0
        }).collect();
        let original_ring = extract_padding_ring(&h, w, d);
        droplet_erosion(&mut h, w, d, 200, 12345);
        let new_ring = extract_padding_ring(&h, w, d);
        for (i, (o, n)) in original_ring.iter().zip(new_ring.iter()).enumerate() {
            assert_eq!(o, n, "ring[{i}] changed by droplet_erosion: {o} -> {n}");
        }
    }
}
