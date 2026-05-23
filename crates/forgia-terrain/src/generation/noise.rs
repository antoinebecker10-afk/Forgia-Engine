//! Couche 1 & 1b — Primitives noise et blending multi-couche par biome.
//!
//! Fournit :
//! - Cache thread-local `Perlin` (evite 1156 recreations par chunk)
//! - Domain warping 3 passes (Inigo Quilez — Crimson Desert style)
//! - 5 primitives noise : ridged / billow / cellular / swiss / FBm-warped
//! - [`biome_noise_layered`] qui blende selon la recette [`super::BiomeNoiseLayers`]
//!
//! Toutes les fonctions sont seedees depuis le world seed → fully deterministic.

use noise::{NoiseFn, Perlin};
use std::cell::RefCell;

use super::{BiomeGenomeOverrides, BiomeNoiseLayers};
use crate::biomes::BiomeType;

// Thread-local Perlin cache — avoid recreating 1156× per chunk (512-entry permutation table).
thread_local! {
    static PERLIN_CACHE: RefCell<(u32, Perlin)> = RefCell::new((0, Perlin::new(0)));
}

pub(super) fn cached_perlin(seed: u32) -> Perlin {
    PERLIN_CACHE.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.0 != seed {
            *cached = (seed, Perlin::new(seed));
        }
        cached.1
    })
}

/// Hardcoded default noise recipes per biome.
fn default_noise_layers(biome: BiomeType) -> BiomeNoiseLayers {
    match biome {
        BiomeType::Mountain => BiomeNoiseLayers {
            ridged_weight: 0.4,
            billow_weight: 0.0,
            worley_weight: 0.1,
            ridged_freq_mult: 0.8,
            worley_freq_mult: 2.5,
            slope_amp_factor: 0.7,
            swiss_weight: 0.3,
            swiss_warp: 1.0,
            lacunarity: 2.2,
            persistence: 0.5,
        },
        BiomeType::Volcanic => BiomeNoiseLayers {
            ridged_weight: 0.4,
            billow_weight: 0.0,
            worley_weight: 0.3,
            ridged_freq_mult: 1.0,
            worley_freq_mult: 1.8,
            slope_amp_factor: 0.5,
            swiss_weight: 0.15,
            swiss_warp: 0.6,
            lacunarity: 2.3,
            persistence: 0.45,
        },
        BiomeType::Canyon => BiomeNoiseLayers {
            ridged_weight: 0.5,
            billow_weight: 0.0,
            worley_weight: 0.15,
            ridged_freq_mult: 1.2,
            worley_freq_mult: 3.0,
            slope_amp_factor: 0.8,
            swiss_weight: 0.25,
            swiss_warp: 0.9,
            lacunarity: 2.0,
            persistence: 0.55,
        },
        BiomeType::Desert => BiomeNoiseLayers {
            ridged_weight: 0.0,
            billow_weight: 0.65,
            worley_weight: 0.0,
            ridged_freq_mult: 1.0,
            worley_freq_mult: 1.0,
            slope_amp_factor: 0.15,
            swiss_weight: 0.08,
            swiss_warp: 0.8,
            lacunarity: 2.6,
            persistence: 0.35,
        },
        BiomeType::Plains => BiomeNoiseLayers {
            ridged_weight: 0.0,
            billow_weight: 0.0,
            worley_weight: 0.0,
            ridged_freq_mult: 1.0,
            worley_freq_mult: 1.0,
            slope_amp_factor: 0.1,
            swiss_weight: 0.0,
            swiss_warp: 0.8,
            lacunarity: 2.5,
            persistence: 0.42,
        },
        BiomeType::Forest => BiomeNoiseLayers {
            ridged_weight: 0.12,
            billow_weight: 0.0,
            worley_weight: 0.0,
            ridged_freq_mult: 1.0,
            worley_freq_mult: 1.0,
            slope_amp_factor: 0.25,
            swiss_weight: 0.15,
            swiss_warp: 0.6,
            lacunarity: 2.5,
            persistence: 0.48,
        },
        BiomeType::Swamp => BiomeNoiseLayers {
            ridged_weight: 0.0,
            billow_weight: 0.15,
            worley_weight: 0.0,
            ridged_freq_mult: 1.0,
            worley_freq_mult: 1.0,
            slope_amp_factor: 0.1,
            swiss_weight: 0.0,
            swiss_warp: 0.8,
            lacunarity: 2.5,
            persistence: 0.30,
        },
        BiomeType::Tundra => BiomeNoiseLayers {
            ridged_weight: 0.12,
            billow_weight: 0.0,
            worley_weight: 0.0,
            ridged_freq_mult: 1.0,
            worley_freq_mult: 1.0,
            slope_amp_factor: 0.15,
            swiss_weight: 0.28,
            swiss_warp: 0.9,
            lacunarity: 2.5,
            persistence: 0.38,
        },
        BiomeType::Jungle => BiomeNoiseLayers {
            ridged_weight: 0.22,
            billow_weight: 0.0,
            worley_weight: 0.08,
            ridged_freq_mult: 1.0,
            worley_freq_mult: 2.0,
            slope_amp_factor: 0.55,
            swiss_weight: 0.18,
            swiss_warp: 0.5,
            lacunarity: 2.4,
            persistence: 0.5,
        },
        BiomeType::Savanna => BiomeNoiseLayers {
            ridged_weight: 0.0,
            billow_weight: 0.1,
            worley_weight: 0.0,
            ridged_freq_mult: 1.0,
            worley_freq_mult: 1.0,
            slope_amp_factor: 0.1,
            swiss_weight: 0.0,
            swiss_warp: 0.8,
            lacunarity: 2.6,
            persistence: 0.4,
        },
    }
}

/// Domain warping Inigo Quilez — 3 passes (AAA standard).
pub(super) fn domain_warp_2d(
    x: f64,
    z: f64,
    perlin: &Perlin,
    freq: f64,
    warp_strength: f64,
) -> f64 {
    let ws = warp_strength;

    // Pass 1: initial deformation (large-scale continental warp)
    let qx = perlin.get([x * freq + 5.2, z * freq + 1.3]);
    let qz = perlin.get([x * freq + 9.1, z * freq + 4.7]);

    // Pass 2: deformation of the deformation (mid-scale organic shapes)
    let rx = perlin.get([(x + ws * qx) * freq + 1.7, (z + ws * qz) * freq + 9.2]);
    let rz = perlin.get([(x + ws * qx) * freq + 8.3, (z + ws * qz) * freq + 2.8]);

    // Pass 3: fine-scale warp (breaks remaining regularity — Crimson Desert style)
    let ws2 = ws * 0.6;
    let sx = perlin.get([(x + ws * rx) * freq + 3.4, (z + ws * rz) * freq + 7.1]);
    let sz = perlin.get([(x + ws * rx) * freq + 6.8, (z + ws * rz) * freq + 0.9]);

    perlin.get([
        (x + ws * rx + ws2 * sx) * freq,
        (z + ws * rz + ws2 * sz) * freq,
    ])
}

/// Ridged multifractal noise — sharp peaks and mountain ridges.
fn ridged_noise_2d(
    x: f64,
    z: f64,
    perlin: &Perlin,
    base_freq: f64,
    octaves: usize,
    lacunarity: f64,
    seed: u32,
) -> f64 {
    let ox = 50000.0 + f64::from(seed) * 0.01;
    let oz = 50000.0 + f64::from(seed) * 0.013;
    let mut freq = base_freq;
    let mut result = 0.0;
    let mut weight = 1.0_f64;
    let mut amp_sum = 0.0_f64;

    for _ in 0..octaves {
        let signal = perlin.get([(x + ox) * freq, (z + oz) * freq]);
        let signal = 1.0 - signal.abs();
        let signal = signal * signal * weight;
        weight = (signal * 2.0).clamp(0.0, 1.0);
        result += signal;
        amp_sum += 1.0;
        freq *= lacunarity;
    }

    if amp_sum > 0.0 {
        result / amp_sum * 2.0 - 1.0
    } else {
        0.0
    }
}

/// Billow noise — smooth rounded dunes and soft hills.
#[allow(clippy::too_many_arguments)]
fn billow_noise_2d(
    x: f64,
    z: f64,
    perlin: &Perlin,
    base_freq: f64,
    octaves: usize,
    lacunarity: f64,
    persistence: f64,
    seed: u32,
) -> f64 {
    let ox = 80000.0 + f64::from(seed) * 0.017;
    let oz = 80000.0 + f64::from(seed) * 0.019;
    let mut freq = base_freq;
    let mut amp = 1.0_f64;
    let mut result = 0.0;
    let mut amp_sum = 0.0_f64;

    for _ in 0..octaves {
        let signal = perlin.get([(x + ox) * freq, (z + oz) * freq]).abs();
        result += signal * amp;
        amp_sum += amp;
        freq *= lacunarity;
        amp *= persistence;
    }

    if amp_sum > 0.0 {
        (result / amp_sum) * 2.0 - 1.0
    } else {
        0.0
    }
}

/// Cellular/Voronoi noise — distance to nearest cell center.
fn cellular_noise_2d(x: f64, z: f64, seed: u32, frequency: f64) -> f64 {
    let fx = x * frequency;
    let fz = z * frequency;
    let ix = fx.floor() as i32;
    let iz = fz.floor() as i32;

    let mut min_dist = f64::MAX;
    let mut second_dist = f64::MAX;

    for dz in -1..=1_i32 {
        for dx in -1..=1_i32 {
            let cx = ix + dx;
            let cz = iz + dz;
            let h = cell_hash(cx, cz, seed);
            let px = f64::from(cx) + f64::from(h & 0xFFFF) / 65535.0;
            let pz = f64::from(cz) + f64::from((h >> 16) & 0xFFFF) / 65535.0;
            let dist_sq = (fx - px) * (fx - px) + (fz - pz) * (fz - pz);
            if dist_sq < min_dist {
                second_dist = min_dist;
                min_dist = dist_sq;
            } else if dist_sq < second_dist {
                second_dist = dist_sq;
            }
        }
    }

    // F2-F1 pattern: edge distance → produces ridges at cell boundaries
    let f1 = min_dist.sqrt();
    let f2 = second_dist.sqrt();
    (f2 - f1).clamp(0.0, 1.0) * 2.0 - 1.0
}

/// Deterministic hash for cellular noise cell centers.
fn cell_hash(x: i32, z: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(73856093)
        ^ (z as u32).wrapping_mul(19349663)
        ^ seed.wrapping_mul(83492791);
    h = h.wrapping_mul(h.wrapping_shr(16) | 1);
    h ^= h.wrapping_shr(15);
    h
}

/// Swiss turbulence noise — FBm with derivative feedback (de Carpentier).
#[allow(clippy::too_many_arguments)]
fn swiss_noise_2d(
    x: f64,
    z: f64,
    perlin: &Perlin,
    base_freq: f64,
    octaves: usize,
    lacunarity: f64,
    persistence: f64,
    warp_factor: f64,
    seed: u32,
) -> f64 {
    const EPSILON: f64 = 0.01;

    let ox = 120000.0 + f64::from(seed) * 0.023;
    let oz = 120000.0 + f64::from(seed) * 0.029;

    let mut freq = base_freq;
    let mut amp = 1.0_f64;
    let mut result = 0.0_f64;
    let mut amp_sum = 0.0_f64;
    let mut dx_sum = 0.0_f64;
    let mut dz_sum = 0.0_f64;

    for _ in 0..octaves {
        let wx = (x + ox + warp_factor * dx_sum) * freq;
        let wz = (z + oz + warp_factor * dz_sum) * freq;

        let n = perlin.get([wx, wz]);

        let grad_x =
            (perlin.get([wx + EPSILON, wz]) - perlin.get([wx - EPSILON, wz])) / (2.0 * EPSILON);
        let grad_z =
            (perlin.get([wx, wz + EPSILON]) - perlin.get([wx, wz - EPSILON])) / (2.0 * EPSILON);

        let ridge = 1.0 - n.abs();
        let ridge_sq = ridge * ridge;

        dx_sum += grad_x * -ridge * amp;
        dz_sum += grad_z * -ridge * amp;

        result += ridge_sq * amp;
        amp_sum += amp;

        freq *= lacunarity;
        amp *= persistence;
    }

    if amp_sum > 0.0 {
        result / amp_sum * 2.0 - 1.0
    } else {
        0.0
    }
}

/// Multi-layer noise blend for a single point.
#[allow(clippy::too_many_arguments)]
pub(super) fn biome_noise_layered(
    x: f64,
    z: f64,
    perlin: &Perlin,
    base_freq: f64,
    octaves: usize,
    warp: f64,
    layers: &BiomeNoiseLayers,
    seed: u32,
) -> f64 {
    let lac = f64::from(layers.lacunarity);
    let pers = f64::from(layers.persistence);

    // Layer 1: Standard FBm with domain warp (always present as base)
    let mut h_fbm = 0.0_f64;
    {
        let mut freq = base_freq;
        let mut amp = 1.0_f64;
        let mut amp_sum = 0.0_f64;
        for _ in 0..octaves {
            h_fbm += domain_warp_2d(x, z, perlin, freq, warp) * amp;
            amp_sum += amp;
            freq *= lac;
            amp *= pers;
        }
        if amp_sum > 0.0 {
            h_fbm /= amp_sum;
        }
    }

    // Layer 2: Ridged multifractal (if weight > 0)
    let h_ridged = if layers.ridged_weight > 0.001 {
        ridged_noise_2d(
            x,
            z,
            perlin,
            base_freq * f64::from(layers.ridged_freq_mult),
            octaves,
            lac,
            seed,
        )
    } else {
        0.0
    };

    // Layer 3: Billow (if weight > 0)
    let h_billow = if layers.billow_weight > 0.001 {
        billow_noise_2d(x, z, perlin, base_freq, octaves, lac, pers, seed)
    } else {
        0.0
    };

    // Layer 4: Cellular/Worley (if weight > 0)
    let h_worley = if layers.worley_weight > 0.001 {
        cellular_noise_2d(
            x,
            z,
            seed.wrapping_add(33333),
            base_freq * f64::from(layers.worley_freq_mult) * 100.0,
        )
    } else {
        0.0
    };

    // Layer 5: Swiss turbulence (if weight > 0)
    let h_swiss = if layers.swiss_weight > 0.001 {
        swiss_noise_2d(
            x,
            z,
            perlin,
            base_freq,
            octaves,
            lac,
            pers,
            f64::from(layers.swiss_warp),
            seed,
        )
    } else {
        0.0
    };

    let fbm_w = f64::from(
        (1.0 - layers.ridged_weight
            - layers.billow_weight
            - layers.worley_weight
            - layers.swiss_weight)
            .max(0.0),
    );
    let total_w = fbm_w
        + f64::from(layers.ridged_weight)
        + f64::from(layers.billow_weight)
        + f64::from(layers.worley_weight)
        + f64::from(layers.swiss_weight);

    if total_w > 0.001 {
        (h_fbm * fbm_w
            + h_ridged * f64::from(layers.ridged_weight)
            + h_billow * f64::from(layers.billow_weight)
            + h_worley * f64::from(layers.worley_weight)
            + h_swiss * f64::from(layers.swiss_weight))
            / total_w
    } else {
        h_fbm
    }
}

/// Resolve noise layers for a biome: genome override > hardcoded default.
pub(super) fn resolve_noise_layers(
    biome: BiomeType,
    overrides: Option<&BiomeGenomeOverrides>,
) -> BiomeNoiseLayers {
    if let Some(ovr) = overrides {
        if let Some(layers) = &ovr.noise_layers[(biome as u8 as usize).min(9)] {
            return layers.clone();
        }
    }
    default_noise_layers(biome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_perlin_is_stable_for_same_seed() {
        let p1 = cached_perlin(42);
        let p2 = cached_perlin(42);
        assert_eq!(p1.get([0.1, 0.2]), p2.get([0.1, 0.2]));
        assert_eq!(p1.get([1.5, 3.7]), p2.get([1.5, 3.7]));
    }

    #[test]
    fn cached_perlin_rebuilds_on_seed_change() {
        let p1 = cached_perlin(42);
        let p2 = cached_perlin(100);
        assert_ne!(p1.get([0.1, 0.2]), p2.get([0.1, 0.2]));
    }

    #[test]
    fn domain_warp_produces_finite_values() {
        let p = Perlin::new(42);
        for (x, z, freq, warp) in [
            (0.0, 0.0, 0.001, 0.0),
            (1000.0, -500.0, 0.01, 2.0),
            (-1e6, 1e6, 0.001, 5.0),
        ] {
            let v = domain_warp_2d(x, z, &p, freq, warp);
            assert!(
                v.is_finite(),
                "domain_warp({x}, {z}, freq={freq}, warp={warp}) = {v}"
            );
        }
    }

    #[test]
    fn biome_noise_layered_covers_all_biomes_finite() {
        let p = Perlin::new(123);
        let biomes = [
            BiomeType::Plains,
            BiomeType::Forest,
            BiomeType::Desert,
            BiomeType::Mountain,
            BiomeType::Swamp,
            BiomeType::Tundra,
            BiomeType::Savanna,
            BiomeType::Jungle,
            BiomeType::Volcanic,
            BiomeType::Canyon,
        ];
        for b in biomes {
            let layers = default_noise_layers(b);
            let v = biome_noise_layered(50.0, 50.0, &p, 0.01, 4, 1.0, &layers, 123);
            assert!(v.is_finite(), "biome_noise_layered({b:?}) = {v}");
            assert!(
                (-3.0..=3.0).contains(&v),
                "biome_noise_layered({b:?}) = {v} outside [-3, 3] bounds"
            );
        }
    }

    #[test]
    fn resolve_noise_layers_default_when_no_override() {
        for b in [BiomeType::Plains, BiomeType::Mountain, BiomeType::Canyon] {
            let layers = resolve_noise_layers(b, None);
            let expected = default_noise_layers(b);
            assert_eq!(layers.ridged_weight, expected.ridged_weight);
            assert_eq!(layers.lacunarity, expected.lacunarity);
        }
    }

    #[test]
    fn cell_hash_is_deterministic() {
        let h1 = cell_hash(10, -5, 42);
        let h2 = cell_hash(10, -5, 42);
        assert_eq!(h1, h2);
        let h3 = cell_hash(10, -5, 43);
        assert_ne!(h1, h3, "different seed must change hash");
    }
}
