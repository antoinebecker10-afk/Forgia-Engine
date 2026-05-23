//! Couche 2 — Redistribution : transforme la distribution noise Perlin brute
//! `[-1, 1]` en forme biome-specifique (ridges, terraces, dunes, collines, ...).
//!
//! L'operation est pure (pas de state). Chaque biome a sa fonction de
//! redistribution dediee, traduite depuis le concept art : Desert = billow,
//! Mountain = ridged, Swamp/Canyon = terraced, Tundra = flat, etc.

use crate::biomes::BiomeType;

/// S-curve contrast enhancement — pushes values toward 0 and 1, creating
/// distinct elevation bands (foothills vs peaks, valleys vs plateaus).
/// Strength 0.0 = no effect, 1.0 = full sigmoid.
#[inline]
fn s_curve(n01: f32, strength: f32) -> f32 {
    if strength < 0.01 {
        return n01;
    }
    // Hermite S-curve: 3t² - 2t³, applied iteratively for stronger effect
    let s = n01 * n01 * (3.0 - 2.0 * n01);
    n01 + (s - n01) * strength
}

/// Redistribution of noise values per biome.
/// Transforms the raw Perlin distribution [-1, 1] into biome-specific shapes.
/// Now with S-curve contrast for dramatic elevation banding (Crimson Desert style).
pub(crate) fn redistribute(n: f32, biome: BiomeType) -> f32 {
    match biome {
        // RIDGED: mountain crests at noise zero-crossings + foothills via s-curve.
        BiomeType::Mountain => {
            let ridged = 1.0 - n.abs();
            let shaped = ridged.powf(1.15);
            let n01 = shaped;
            s_curve(n01, 0.30) * 2.0 - 1.0
        }

        // BILLOW: dunes with flat interdune areas + crests
        BiomeType::Desert => {
            let n01 = n.abs();
            s_curve(n01, 0.3) * 0.8
        }

        // TERRACED: geological steps with flat plateaus
        BiomeType::Swamp => {
            let n01 = (n + 1.0) * 0.5;
            let steps = 2.0_f32;
            let terraced = (n01 * steps).floor() / steps;
            let frac = (n01 * steps).fract();
            let t = frac * frac * (3.0 - 2.0 * frac);
            (terraced + t / steps) * 2.0 - 1.0
        }

        // FLAT: compressed tundra, glacial smoothing
        BiomeType::Tundra => {
            let t = (n + 1.0) * 0.5;
            let compressed = t * t * (3.0 - 2.0 * t);
            compressed * 0.6
        }

        // ROLLING_FLAT: gentle savanna with occasional kopjes
        BiomeType::Savanna => {
            let n01 = (n + 1.0) * 0.5;
            let shaped = n01.powf(0.8);
            s_curve(shaped, 0.2) * 2.0 - 1.0
        }

        // STEEP RAVINES: tropical jungle with deep V-cuts
        BiomeType::Jungle => {
            let n01 = (n + 1.0) * 0.5;
            let shaped = n01.powf(1.5);
            s_curve(shaped, 0.38) * 2.0 - 1.0
        }

        // VOLCANIC: sharp ridges with lava-flow valleys
        BiomeType::Volcanic => {
            let ridged = (1.0 - n.abs()).powf(1.2);
            let n01 = ridged;
            s_curve(n01, 0.4) * 2.0 - 1.0
        }

        // CANYON: stratified terraces (badlands style), gentler transitions
        BiomeType::Canyon => {
            let n01 = (n + 1.0) * 0.5;
            let steps = 4.0_f32;
            let terraced = (n01 * steps).floor() / steps;
            let frac = (n01 * steps).fract();
            let t = frac * frac * (3.0 - 2.0 * frac);
            let base = (terraced + t / steps) * 2.0 - 1.0;
            let n01_final = (base + 1.0) * 0.5;
            s_curve(n01_final, 0.25) * 2.0 - 1.0
        }

        // ROLLING HILLS: natural with gentle S-curve for valley→hill contrast
        BiomeType::Plains => {
            let n01 = (n + 1.0) * 0.5;
            let shaped = n01.powf(1.2);
            s_curve(shaped, 0.25) * 2.0 - 1.0
        }
        BiomeType::Forest => {
            let n01 = (n + 1.0) * 0.5;
            let shaped = n01.powf(1.4);
            s_curve(shaped, 0.35) * 2.0 - 1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_curve_strength_zero_is_identity() {
        for v in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
            assert!(
                (s_curve(v, 0.0) - v).abs() < 1e-6,
                "s_curve({v}, 0.0) should equal {v}"
            );
        }
    }

    #[test]
    fn s_curve_preserves_endpoints() {
        assert!((s_curve(0.0, 1.0) - 0.0).abs() < 1e-6);
        assert!((s_curve(1.0, 1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn redistribute_is_finite_for_all_biomes() {
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
            for n in [-1.0_f32, -0.5, -0.01, 0.0, 0.01, 0.5, 1.0] {
                let out = redistribute(n, b);
                assert!(
                    out.is_finite(),
                    "redistribute({n}, {b:?}) = {out} (not finite)"
                );
            }
        }
    }

    #[test]
    fn redistribute_stays_near_unit_range() {
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
            for n in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
                let out = redistribute(n, b);
                assert!(
                    (-1.1..=1.1).contains(&out),
                    "redistribute({n}, {b:?}) = {out} outside ~[-1, 1]"
                );
            }
        }
    }

    #[test]
    fn redistribute_mountain_peaks_at_noise_zero() {
        let peak = redistribute(0.0, BiomeType::Mountain);
        let valley_pos = redistribute(1.0, BiomeType::Mountain);
        let valley_neg = redistribute(-1.0, BiomeType::Mountain);
        assert!(
            peak > valley_pos,
            "peak({peak}) doit dépasser valley+({valley_pos})"
        );
        assert!(
            peak > valley_neg,
            "peak({peak}) doit dépasser valley-({valley_neg})"
        );
        assert!(peak > 0.5, "ridged peak attendu > 0.5, obtenu {peak}");
    }

    #[test]
    fn redistribute_mountain_monotone_descent() {
        let samples: Vec<f32> = [0.0_f32, 0.25, 0.5, 0.75, 1.0]
            .iter()
            .map(|&n| redistribute(n, BiomeType::Mountain))
            .collect();
        for w in samples.windows(2) {
            assert!(w[0] >= w[1] - 1e-5, "Mountain non-monotone : {w:?}");
        }
    }
}
