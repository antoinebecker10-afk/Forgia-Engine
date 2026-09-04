//! Island mask procedural — Story 350 Cartoon Island MVP V2.
//!
//! Calcule un masque [0..1] qui sculpte une silhouette d'île reconnaissable :
//! cercle déformé (radial × Perlin warp) − baies/péninsules (Worley sur seeds
//! jitterées). Multiplié au heightmap final → côte irrégulière, ocean wrap.
//!
//! Référence : Red Blob Games https://www.redblobgames.com/maps/mapgen/
//!
//! Pure-function, déterministe (seed). Pas de Resource Bevy ici.

use noise::{NoiseFn, Perlin};

/// Paramètres du mask, tous lisibles depuis genome `island_mvp.toml`.
#[derive(Clone, Debug, Copy)]
pub struct IslandMaskParams {
    /// Centre île (m, world space)
    pub center_x: f32,
    pub center_z: f32,
    /// Rayon nominal de l'île (avant warp)
    pub radius: f32,
    /// Pente de la transition côte→océan (1.0 = doux, 5.0 = falaise)
    pub falloff_steepness: f32,
    /// Nombre de baies/péninsules Voronoi (3..24)
    pub voronoi_seeds: u32,
    /// Fréquence du Perlin de déformation côte (~0.0008..0.005)
    pub perlin_freq: f64,
    /// Seed déterministe pour le Worley + Perlin
    pub seed: u32,
}

impl Default for IslandMaskParams {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_z: 0.0,
            radius: 1500.0,
            falloff_steepness: 2.5,
            voronoi_seeds: 12,
            perlin_freq: 0.0015,
            seed: 1337,
        }
    }
}

/// Calcule le mask [0..1] au point (x, z). 1 = pleine île, 0 = océan.
pub fn island_mask_at(x: f32, z: f32, params: &IslandMaskParams) -> f32 {
    let dx = x - params.center_x;
    let dz = z - params.center_z;
    let dist = (dx * dx + dz * dz).sqrt();
    let radius = params.radius.max(1.0);

    // 1. Perlin warp on coast
    let perlin = Perlin::new(params.seed);
    let warp_amp = radius * 0.18;
    let warp = perlin.get([
        f64::from(x) * params.perlin_freq,
        f64::from(z) * params.perlin_freq,
    ]) as f32
        * warp_amp;
    let warped_dist = (dist + warp).max(0.0);

    // 2. Radial falloff (smoothstep avec puissance configurable)
    let t = (warped_dist / radius).clamp(0.0, 1.0);
    let t_pow = t.powf(params.falloff_steepness.max(0.5));
    let radial = 1.0 - smoothstep(t_pow);

    // 3. Worley bays subtractive
    let bays = worley_bays_at(x, z, params, radius);

    (radial - bays * 0.30).clamp(0.0, 1.0)
}

/// Smoothstep cubic Hermite : t² (3 - 2t).
#[inline]
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Worley distance au seed le plus proche, normalisée [0..1] sur le rayon nominal.
fn worley_bays_at(x: f32, z: f32, params: &IslandMaskParams, radius: f32) -> f32 {
    let n = params.voronoi_seeds.clamp(1, 64);
    let mut min_dist_sq = f32::MAX;

    let ring_radius = radius * 0.85;
    let two_pi = std::f32::consts::TAU;

    for i in 0..n {
        let h = hash_u32(params.seed.wrapping_add(i.wrapping_mul(2654435761)));
        let angle_jitter = (h as f32 / u32::MAX as f32) * 0.4 - 0.2;
        let radius_jitter = ((hash_u32(h) as f32 / u32::MAX as f32) * 0.3 - 0.15) * radius;

        let theta = (i as f32 / n as f32) * two_pi + angle_jitter;
        let sx = params.center_x + (ring_radius + radius_jitter) * theta.cos();
        let sz = params.center_z + (ring_radius + radius_jitter) * theta.sin();

        let ddx = x - sx;
        let ddz = z - sz;
        let dsq = ddx * ddx + ddz * ddz;
        if dsq < min_dist_sq {
            min_dist_sq = dsq;
        }
    }

    let bay_radius = radius * 0.30;
    let dist = min_dist_sq.sqrt();
    let proximity = (1.0 - (dist / bay_radius)).clamp(0.0, 1.0);
    proximity * proximity * (3.0 - 2.0 * proximity)
}

/// Hash u32 → u32 déterministe (Wang hash).
#[inline]
fn hash_u32(x: u32) -> u32 {
    let mut x = x ^ 0x9e3779b9;
    x = x.wrapping_add(x << 6);
    x ^= x >> 11;
    x = x.wrapping_add(x << 8);
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn island_mask_center_is_full_land() {
        let p = IslandMaskParams::default();
        let m = island_mask_at(p.center_x, p.center_z, &p);
        assert!(m > 0.5, "center should be full land, got {m}");
    }

    #[test]
    fn island_mask_far_from_island_is_ocean() {
        let p = IslandMaskParams::default();
        let m = island_mask_at(p.center_x + p.radius * 3.0, p.center_z, &p);
        assert_eq!(m, 0.0);
    }

    #[test]
    fn island_mask_deterministic() {
        let p = IslandMaskParams::default();
        let a = island_mask_at(100.0, -200.0, &p);
        let b = island_mask_at(100.0, -200.0, &p);
        assert_eq!(a, b);
    }

    #[test]
    fn island_mask_different_seeds_produce_different_shapes() {
        let p1 = IslandMaskParams {
            seed: 1,
            ..IslandMaskParams::default()
        };
        let p2 = IslandMaskParams {
            seed: 99999,
            ..IslandMaskParams::default()
        };
        let mut total_diff = 0.0_f32;
        let n = 24;
        for i in 0..n {
            let theta = (i as f32 / n as f32) * std::f32::consts::TAU;
            let r = p1.radius * 0.85;
            let x = r * theta.cos();
            let z = r * theta.sin();
            total_diff += (island_mask_at(x, z, &p1) - island_mask_at(x, z, &p2)).abs();
        }
        let avg_diff = total_diff / n as f32;
        assert!(
            avg_diff > 0.01,
            "different seeds should produce visibly different coast shapes (avg_diff={avg_diff:.4})"
        );
    }

    #[test]
    fn island_mask_radius_scales_island() {
        let small = IslandMaskParams {
            radius: 500.0,
            ..IslandMaskParams::default()
        };
        let big = IslandMaskParams {
            radius: 2000.0,
            ..IslandMaskParams::default()
        };
        let m_small = island_mask_at(1000.0, 0.0, &small);
        let m_big = island_mask_at(1000.0, 0.0, &big);
        assert!(m_big > m_small);
    }
}
