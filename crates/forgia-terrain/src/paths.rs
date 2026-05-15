//! Path network — version minimale V2 (vertical slice).
//!
//! V1 forgia-terrain::paths fait 689 LOC : 256×256 grid spatial + MST entre
//! villages + Bezier + biome overrides. Sur-dimensionné tant que les villages
//! ne sont pas portés. Ici on garde uniquement :
//!
//! - `PathNetwork` Resource avec `Vec<PathSample>` (position monde + tangent +
//!   tier) échantillonnés tous `SAMPLE_INTERVAL` mètres
//! - `RoadTier` (Primary / Secondary / Trail) avec half_width
//! - Génération Bezier entre N POI (entry-points structurels demo)

use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoadTier {
    /// Route principale entre POI majeurs : 4 m total.
    Primary,
    /// Sentier secondaire : 2.5 m total.
    Secondary,
    /// Sentier mineur / shortcut : 1.2 m total.
    Trail,
}

impl RoadTier {
    /// Demi-largeur du centre plat de la route (mètres).
    pub fn half_width(self) -> f32 {
        match self {
            Self::Primary => 2.0,
            Self::Secondary => 1.25,
            Self::Trail => 0.6,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PathSample {
    /// Position monde XZ (le Y est échantillonné runtime via heightmap_at).
    pub pos: Vec2,
    /// Tangent unitaire dans le plan XZ.
    pub tangent: Vec2,
    pub tier: RoadTier,
}

#[derive(Resource, Default, Clone)]
pub struct PathNetwork {
    pub samples: Vec<PathSample>,
}

// ─────────────────────────── Génération Bezier ───────────────────────────

/// Intervalle de samples sur la courbe (mètres). Structural visual, pas
/// gameplay → exception `no-hardcode.md` "limites techniques".
const SAMPLE_INTERVAL_M: f32 = 3.0;
const BEZIER_SUBDIVISIONS: u32 = 4;

fn bezier_quadratic(start: Vec2, control: Vec2, end: Vec2, t: f32) -> Vec2 {
    let inv = 1.0 - t;
    inv * inv * start + 2.0 * inv * t * control + t * t * end
}

fn bezier_tangent(start: Vec2, control: Vec2, end: Vec2, t: f32) -> Vec2 {
    let inv = 1.0 - t;
    let raw = 2.0 * inv * (control - start) + 2.0 * t * (end - control);
    let n = raw.length();
    if n > 1e-4 { raw / n } else { Vec2::X }
}

/// Génère un réseau de routes en anneau passant par les `pois` (ordre =
/// poi[0] → poi[1] → … → poi[n-1] → poi[0]).
/// `bezier_warp` contrôle l'arc des segments (0 = ligne droite, 0.3 = courbe).
pub fn build_path_network(pois: &[Vec2], tier: RoadTier, bezier_warp: f32) -> PathNetwork {
    let mut samples: Vec<PathSample> = Vec::new();
    if pois.len() < 2 { return PathNetwork { samples }; }

    let steps_per_segment = 1 << BEZIER_SUBDIVISIONS; // 16 sub-steps
    for i in 0..pois.len() {
        let start = pois[i];
        let end = pois[(i + 1) % pois.len()];
        let mid = (start + end) * 0.5;
        let dir = (end - start).normalize_or_zero();
        let perp = Vec2::new(-dir.y, dir.x);
        let seg_len = (end - start).length();
        let control = mid + perp * (seg_len * bezier_warp);

        let mut last_pos = start;
        let mut acc_dist = 0.0_f32;
        for step in 1..=steps_per_segment {
            let t = step as f32 / steps_per_segment as f32;
            let pos = bezier_quadratic(start, control, end, t);
            let delta = (pos - last_pos).length();
            acc_dist += delta;
            while acc_dist >= SAMPLE_INTERVAL_M {
                let p = bezier_quadratic(start, control, end, t);
                let tg = bezier_tangent(start, control, end, t);
                samples.push(PathSample { pos: p, tangent: tg, tier });
                acc_dist -= SAMPLE_INTERVAL_M;
            }
            last_pos = pos;
        }
    }
    PathNetwork { samples }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_network_when_too_few_pois() {
        let net = build_path_network(&[Vec2::ZERO], RoadTier::Trail, 0.0);
        assert!(net.samples.is_empty());
    }

    #[test]
    fn ring_of_two_pois_produces_samples() {
        let pois = vec![Vec2::ZERO, Vec2::new(60.0, 0.0)];
        let net = build_path_network(&pois, RoadTier::Secondary, 0.2);
        assert!(!net.samples.is_empty(), "should produce samples");
        assert!(net.samples.len() > 20 && net.samples.len() < 60);
        assert_eq!(net.samples[0].tier, RoadTier::Secondary);
        let t = net.samples[0].tangent;
        assert!((t.length() - 1.0).abs() < 0.01);
    }

    #[test]
    fn half_width_monotone() {
        assert!(RoadTier::Primary.half_width() > RoadTier::Secondary.half_width());
        assert!(RoadTier::Secondary.half_width() > RoadTier::Trail.half_width());
    }
}
