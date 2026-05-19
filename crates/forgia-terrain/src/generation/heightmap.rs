//! Heightmap generation + Couche 4 (micro-roughness) + Feature height.
//!
//! API publique :
//! - [`heightmap_at`] : FBm classique sans biome (backward compat)
//! - [`heightmap_at_gen`] / [`heightmap_at_gen_ext`] / [`heightmap_at_gen_ext_fast`] :
//!   heightmap biome-aware avec genome overrides
//! - [`procedural_sdf_at`] : SDF scalar pour l'Eraser brush
//!
//! Pipeline par point :
//! 1. Multi-noise layer blend via `biome_noise_layered` (Couche 1/1b de noise.rs)
//! 2. Slope-amp mask (optionnel, cher)
//! 3. Redistribution biome (Couche 2 de redistribution.rs)
//! 4. Edge/island falloff
//! 5. TerrainFeature accumulators (MountainRange / Lake / River / Crater / Plateau)
//! 6. Micro-roughness (Couche 4)
//! 7. Clamp max_height + floor

use bevy::prelude::*;
use ::noise::{NoiseFn, Perlin};

use crate::biomes::BiomeType;
use crate::chunk::TerrainConfig;
use crate::map_gen_config::{MapGenConfig, TerrainFeature};

use super::BiomeGenomeOverrides;
use super::noise::{biome_noise_layered, cached_perlin, domain_warp_2d, resolve_noise_layers};
use super::redistribution::redistribute;

/// Evaluate one octave of very-low-frequency Perlin noise for the continental layer.
fn continental_bias(
    x: f32,
    z: f32,
    perlin: &Perlin,
    scale: f32,
    strength: f32,
    max_height: f32,
) -> f32 {
    if strength <= 0.0 { return 0.0; }
    let c = perlin.get([
        f64::from(x) * f64::from(scale),
        f64::from(z) * f64::from(scale),
    ]) as f32;
    (c * 0.5 + 0.5) * strength * max_height
}

/// Multi-octave Perlin noise heightmap (original, backward compatible).
pub fn heightmap_at(x: f32, z: f32, config: &TerrainConfig) -> f32 {
    let perlin = cached_perlin(config.seed);
    let half = config.map_size / 2.0;

    let octaves: [(f64, f32); 4] = [
        (0.003, 1.0),
        (0.008, 0.5),
        (0.025, 0.15),
        (0.060, 0.05),
    ];

    let mut h: f32 = 0.0;
    for &(freq, amp) in &octaves {
        h += perlin.get([f64::from(x) * freq, f64::from(z) * freq]) as f32 * amp;
    }

    h = (h + 1.0) * 0.5;
    h = config.sea_level + h * (config.max_height - config.sea_level);

    // Fix : interpolation vers sea_level au lieu de multiplication vers 0.
    // L'ancienne formule `h *= edge_factor` faisait descendre h sous sea_level
    // partout où edge_factor < 1 → cuvettes systématiques avec floor à 2.0
    // masquant le bug. Pattern industrie : terrain descend vers la mer aux
    // bords, pas vers l'abysse.
    let edge_factor = edge_falloff(x, z, half, 80.0);
    h = config.sea_level + (h - config.sea_level) * edge_factor;

    h.max(2.0)
}

/// Configurable heightmap with MapGenConfig support.
pub fn heightmap_at_gen(
    x: f32, z: f32,
    config: &TerrainConfig,
    gen_config: &MapGenConfig,
    biome: Option<BiomeType>,
) -> f32 {
    heightmap_at_gen_ext(x, z, config, gen_config, biome, None)
}

/// Extended heightmap with genome noise layer overrides.
pub fn heightmap_at_gen_ext(
    x: f32, z: f32,
    config: &TerrainConfig,
    gen_config: &MapGenConfig,
    biome: Option<BiomeType>,
    genome_overrides: Option<&BiomeGenomeOverrides>,
) -> f32 {
    heightmap_at_gen_ext_impl(x, z, config, gen_config, biome, genome_overrides, false)
}

/// Multi-biome blend variant: skips slope-amp (60% cheaper) for neighbor biomes.
pub fn heightmap_at_gen_ext_fast(
    x: f32, z: f32,
    config: &TerrainConfig,
    gen_config: &MapGenConfig,
    biome: Option<BiomeType>,
    genome_overrides: Option<&BiomeGenomeOverrides>,
) -> f32 {
    heightmap_at_gen_ext_impl(x, z, config, gen_config, biome, genome_overrides, true)
}

fn heightmap_at_gen_ext_impl(
    x: f32, z: f32,
    config: &TerrainConfig,
    gen_config: &MapGenConfig,
    biome: Option<BiomeType>,
    genome_overrides: Option<&BiomeGenomeOverrides>,
    skip_slope_amp: bool,
) -> f32 {
    let _span = info_span!("terrain_heightmap_gen", x, z).entered();
    let perlin = cached_perlin(config.seed);
    let half = config.map_size / 2.0;
    let map_size = config.map_size;

    let base_freq = f64::from(gen_config.base_frequency);
    let num_octaves = gen_config.octaves.clamp(1, 8);

    // Resolve per-biome warp strength
    let warp = f64::from(if let Some(b) = biome {
        if let Some(ovr) = genome_overrides {
            ovr.warp_strength[(b as u8 as usize).min(9)]
                .unwrap_or(gen_config.warp_strength)
        } else {
            gen_config.warp_strength
        }
    } else {
        gen_config.warp_strength
    });

    // Couche 1: Multi-noise layer blend per biome
    let mut h: f32 = if let Some(b) = biome {
        let layers = resolve_noise_layers(b, genome_overrides);

        let raw = biome_noise_layered(
            f64::from(x), f64::from(z),
            &perlin,
            base_freq,
            num_octaves as usize,
            warp,
            &layers,
            config.seed,
        ) as f32;

        // Slope-dependent amplitude mask
        if !skip_slope_amp && layers.slope_amp_factor > 0.001 {
            let dx_h = biome_noise_layered(
                f64::from(x) + 1.0, f64::from(z), &perlin, base_freq,
                num_octaves as usize, warp, &layers, config.seed,
            ) as f32;
            let dz_h = biome_noise_layered(
                f64::from(x), f64::from(z) + 1.0, &perlin, base_freq,
                num_octaves as usize, warp, &layers, config.seed,
            ) as f32;
            let slope = ((dx_h - raw).powi(2) + (dz_h - raw).powi(2)).sqrt();
            let slope_mask = (slope * 5.0).clamp(0.2, 1.0);
            let factor = layers.slope_amp_factor;
            raw * (1.0 - factor + factor * slope_mask)
        } else {
            raw
        }
    } else {
        // No biome: standard FBm with domain warp (backward compat)
        let mut val = 0.0f32;
        let mut freq = base_freq;
        let mut amp = 1.0f32;
        for _ in 0..num_octaves {
            val += domain_warp_2d(f64::from(x), f64::from(z), &perlin, freq, warp) as f32 * amp;
            freq *= 2.5;
            amp *= 0.45;
        }
        val
    };

    // Couche 2: Redistribution per biome
    if let Some(b) = biome {
        h = redistribute(h, b);
    }

    h = (h + 1.0) * 0.5;
    h = config.sea_level + h * (config.max_height - config.sea_level);

    // GAP 1: Per-biome height multiplier
    if let Some(b) = biome {
        let mult = genome_overrides
            .and_then(|o| o.height_mult[(b as u8 as usize).min(9)])
            .unwrap_or(1.0);
        if (mult - 1.0).abs() > f32::EPSILON {
            h = config.sea_level + (h - config.sea_level) * mult;
        }
    }

    // GAP 2: Continental bias
    h += continental_bias(
        x, z, &perlin,
        gen_config.continental_scale,
        gen_config.continental_strength,
        config.max_height,
    );

    // Island mode: circular falloff from map center.
    if gen_config.island_mode {
        if let Some(params) = genome_overrides.and_then(|o| o.island_mask.as_ref()) {
            let mask = super::island_mask_at(x, z, params);
            let above = h - config.sea_level;
            h = config.sea_level + above * mask;
        } else {
            let cx = half;
            let cz = half;
            let dist = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
            let max_radius = half * 0.85;
            let shore_start = max_radius * 0.6;
            if dist > shore_start {
                let t = ((dist - shore_start) / (max_radius - shore_start)).clamp(0.0, 1.0);
                h *= 1.0 - t * t * (3.0 - 2.0 * t);
            }
        }
    } else {
        h *= edge_falloff(x, z, half, 80.0);
    }

    // Apply terrain features
    for feature in &gen_config.features {
        h += feature_height(x, z, feature, map_size);
    }

    // Couche 4: Micro-roughness (per-biome amplitude from genome).
    let micro_amp = biome
        .and_then(|b| genome_overrides.and_then(|o| o.micro_roughness_amp[(b as u8 as usize).min(9)]))
        .unwrap_or(0.35);
    h += micro_roughness(x, z, config.seed, micro_amp);

    h = h.min(config.max_height);

    // Coastal band smoothing
    let sea_level = config.sea_level;
    let coastal_band = 25.0;
    let coastal_top = sea_level + coastal_band;
    if h > sea_level && h < coastal_top {
        let h_above = h - sea_level;
        let t = h_above / coastal_band;
        let smooth = t * t * (3.0 - 2.0 * t);
        let blended = sea_level + smooth * coastal_band;
        let beach_bias = if t < 0.3 { (0.3 - t) * 0.7 } else { 0.0 };
        h = blended - beach_bias * (h_above);
    }

    if gen_config.island_mode {
        h.max(-2.0)
    } else {
        h.max(2.0)
    }
}

/// Compute the procedural SDF value at a single world voxel position.
pub fn procedural_sdf_at(
    wx: f32, wy: f32, wz: f32,
    config: &TerrainConfig,
    gen_config: Option<&MapGenConfig>,
    biome: Option<BiomeType>,
) -> f32 {
    let height = match gen_config {
        Some(gc) => heightmap_at_gen(wx, wz, config, gc, biome),
        None => heightmap_at(wx, wz, config),
    };
    wy - height
}

/// Smooth falloff near map edges (0 at edge, 1 in interior).
fn edge_falloff(x: f32, z: f32, half: f32, fade_dist: f32) -> f32 {
    // Forgia convention : world centré sur (0,0), bords à ±half (cf ChunkCoord
    // dans chunk.rs:33). Le code initial assumait [0, map_size] ce qui
    // faisait croire que le spawn world (19,19) était au coin (edge_factor
    // ≈ 0.14 → terrain écrasé sous sea_level partout autour du spawn).
    let dx = (half - x.abs()).max(0.0);
    let dz = (half - z.abs()).max(0.0);
    let d = dx.min(dz);
    if d < fade_dist {
        let t = d / fade_dist;
        t * t * (3.0 - 2.0 * t)
    } else {
        1.0
    }
}

/// Multi-octave micro-roughness — breaks mathematical regularity at multiple scales.
pub(super) fn micro_roughness(x: f32, z: f32, seed: u32, amp: f32) -> f32 {
    let micro = Perlin::new(seed.wrapping_add(77777));
    let xd = f64::from(x);
    let zd = f64::from(z);
    let o1 = micro.get([xd * 0.5, zd * 0.5]) as f32 * 0.5;
    let o2 = micro.get([xd * 1.4 + 333.3, zd * 1.4 + 777.7]) as f32 * 0.3;
    (o1 + o2) * amp
}

/// Compute height contribution of a single terrain feature at (x, z).
pub(super) fn feature_height(x: f32, z: f32, feature: &TerrainFeature, map_size: f32) -> f32 {
    match feature {
        TerrainFeature::MountainRange { center, direction, width, height } => {
            let cx = center[0] * map_size;
            let cz = center[1] * map_size;
            let dir_len = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt().max(0.001);
            let nx = -direction[1] / dir_len;
            let nz = direction[0] / dir_len;
            let px = x - cx;
            let pz = z - cz;
            let perp_dist = (px * nx + pz * nz).abs();
            let w = width * map_size;
            let outer = w * 1.3;
            if perp_dist < outer {
                let sigma = w * 0.55;
                *height * (-perp_dist * perp_dist / (2.0 * sigma * sigma)).exp()
            } else {
                0.0
            }
        }

        TerrainFeature::Lake { center, radius, depth } => {
            let cx = center[0] * map_size;
            let cz = center[1] * map_size;
            let r = radius * map_size;
            let dist = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
            if dist < r {
                let t = 1.0 - dist / r;
                -depth * t * t
            } else {
                0.0
            }
        }

        TerrainFeature::River { start, end, width, depth } => {
            let sx = start[0] * map_size;
            let sz = start[1] * map_size;
            let ex = end[0] * map_size;
            let ez = end[1] * map_size;
            let dx = ex - sx;
            let dz = ez - sz;
            let len_sq = dx * dx + dz * dz;
            if len_sq < 0.001 { return 0.0; }
            let t = ((x - sx) * dx + (z - sz) * dz) / len_sq;
            let t = t.clamp(0.0, 1.0);
            let closest_x = sx + t * dx;
            let closest_z = sz + t * dz;
            let dist = ((x - closest_x).powi(2) + (z - closest_z).powi(2)).sqrt();
            let w = width * map_size;
            if dist < w {
                let norm = dist / w;
                let channel = norm * norm * (3.0 - 2.0 * norm);
                -depth * (1.0 - channel * 0.7)
            } else if dist < w * 2.8 {
                let bank_norm = (dist - w) / (w * 1.8);
                let bank_peak = (bank_norm * 2.0 - bank_norm * bank_norm).clamp(0.0, 1.0);
                depth * 0.06 * bank_peak
            } else {
                0.0
            }
        }

        TerrainFeature::Crater { center, radius, rim_height, depth } => {
            let cx = center[0] * map_size;
            let cz = center[1] * map_size;
            let r = radius * map_size;
            let dist = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
            if dist < r {
                let norm = dist / r;
                let rim = rim_height * (-((norm - 0.80) * 6.0).powi(2)).exp();
                let t = 1.0 - norm;
                let crater = -depth * t * t * (3.0 - 2.0 * t);
                rim + crater
            } else if dist < r * 1.5 {
                let t = 1.0 - (dist - r) / (r * 0.5);
                let t = t.clamp(0.0, 1.0);
                rim_height * t * t * (3.0 - 2.0 * t) * 0.4
            } else {
                0.0
            }
        }

        TerrainFeature::Plateau { center, radius, height } => {
            let cx = center[0] * map_size;
            let cz = center[1] * map_size;
            let r = radius * map_size;
            let dist = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
            if dist < r * 0.7 {
                *height
            } else if dist < r {
                let t = 1.0 - (dist - r * 0.7) / (r * 0.3);
                height * t * t * (3.0 - 2.0 * t)
            } else {
                0.0
            }
        }

        TerrainFeature::LavaPool { .. } => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> TerrainConfig {
        TerrainConfig::default()
    }

    #[test]
    fn edge_falloff_zero_at_edge_one_far_inside() {
        // Convention centrée (0,0) avec bords à ±half (cf chunk.rs).
        let half = 2048.0;
        // Au bord : z = ±half → dz = 0 → falloff = 0
        let v0 = edge_falloff(0.0, half, half, 80.0);
        assert!(v0 <= 1e-5, "edge should be ~0, got {v0}");
        let v_neg = edge_falloff(0.0, -half, half, 80.0);
        assert!(v_neg <= 1e-5, "negative edge should also be ~0, got {v_neg}");
        // Au centre (0,0) : dx = dz = half → loin du fade → 1.0
        let v1 = edge_falloff(0.0, 0.0, half, 80.0);
        assert!((v1 - 1.0).abs() < 1e-5, "center should be ~1, got {v1}");
        // Spawn typique world (19, 19) doit être > 0.99 (presque centre)
        let v_spawn = edge_falloff(19.0, 19.0, half, 80.0);
        assert!(v_spawn > 0.99, "spawn (19,19) should be near-1, got {v_spawn}");
    }

    #[test]
    fn heightmap_at_never_below_floor() {
        let cfg = test_config();
        for (x, z) in [(0.0, 0.0), (100.0, 100.0), (-1000.0, 2000.0), (4096.0, 0.0)] {
            let h = heightmap_at(x, z, &cfg);
            assert!(h.is_finite(), "heightmap_at({x}, {z}) = {h}");
            assert!(h >= 2.0, "heightmap_at({x}, {z}) = {h} < 2.0 floor");
        }
    }

    #[test]
    fn procedural_sdf_at_sign_matches_below_above() {
        let cfg = test_config();
        let h = heightmap_at(100.0, 100.0, &cfg);
        let below = procedural_sdf_at(100.0, h - 5.0, 100.0, &cfg, None, None);
        let above = procedural_sdf_at(100.0, h + 5.0, 100.0, &cfg, None, None);
        assert!(below < 0.0, "below terrain should be solid (SDF < 0), got {below}");
        assert!(above > 0.0, "above terrain should be air (SDF > 0), got {above}");
    }

    #[test]
    fn micro_roughness_zero_amp_returns_zero() {
        assert_eq!(micro_roughness(100.0, 100.0, 42, 0.0), 0.0);
        assert_eq!(micro_roughness(-500.0, 1e6, 999, 0.0), 0.0);
    }

    #[test]
    fn micro_roughness_stays_bounded() {
        for amp in [0.1, 0.35, 1.0] {
            let v = micro_roughness(123.0, 456.0, 42, amp);
            assert!(v.abs() < 1.0 * amp, "micro {v} exceeds {amp}");
        }
    }
}
