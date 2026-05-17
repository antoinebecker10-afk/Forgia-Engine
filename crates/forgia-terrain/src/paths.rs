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
    /// Route principale entre POI majeurs : 5 m total.
    Primary,
    /// Sentier secondaire : 3.6 m total.
    Secondary,
    /// Sentier mineur / shortcut : 1.6 m total.
    Trail,
    /// Voie urbaine pavée (sortie de village, place centrale) : 3 m total,
    /// texture PavingStones PBR. story-441 — vague 3 enrichira la transition
    /// vertex blend vers terrain.
    Urban,
}

impl RoadTier {
    /// Demi-largeur du centre plat (mètres). Échelle médiévale crédible.
    pub fn half_width(self) -> f32 {
        match self {
            Self::Primary => 2.5,
            Self::Secondary => 1.8,
            Self::Trail => 0.8,
            Self::Urban => 1.5,
        }
    }

    /// Zone fade vers herbe (mètres). Les bords du ribbon s'étendent au-delà
    /// du centre et leur couleur blend vers la teinte herbe → transition
    /// progressive vs ligne nette.
    pub fn edge_fade(self) -> f32 {
        match self {
            Self::Primary => 0.8,
            Self::Secondary => 0.6,
            Self::Trail => 0.4,
            Self::Urban => 0.5,
        }
    }

    /// Parse a TOML string tier name to enum. Returns Secondary as fallback
    /// for backward compat (called from data-driven TOML).
    pub fn from_tier_name(s: &str) -> Self {
        match s {
            "primary" => Self::Primary,
            "secondary" => Self::Secondary,
            "trail" => Self::Trail,
            "urban" => Self::Urban,
            _ => Self::Secondary,
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

/// Une polyline = série de samples connectés (1 segment Bezier). Le ribbon
/// mesh ne triangulera qu'à l'intérieur d'une polyline → aux coins POI où
/// la tangente flip brutalement, on a deux polylines distinctes plutôt
/// qu'un triangle tordu.
#[derive(Default, Clone, Debug)]
pub struct PathPolyline {
    pub samples: Vec<PathSample>,
}

#[derive(Resource, Default, Clone)]
pub struct PathNetwork {
    /// Une polyline par segment Bezier (entre 2 POIs).
    pub polylines: Vec<PathPolyline>,
}

impl PathNetwork {
    /// Vue plate de tous les samples — pratique pour les queries de distance
    /// (foliage anti-spawn, etc.). Order : segment 1 puis segment 2 etc.
    pub fn samples_iter(&self) -> impl Iterator<Item = &PathSample> {
        self.polylines.iter().flat_map(|p| p.samples.iter())
    }
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
    let mut polylines: Vec<PathPolyline> = Vec::new();
    if pois.len() < 2 { return PathNetwork { polylines }; }

    let steps_per_segment = 1 << BEZIER_SUBDIVISIONS; // 16 sub-steps
    let dt = 1.0 / steps_per_segment as f32;
    for i in 0..pois.len() {
        let start = pois[i];
        let end = pois[(i + 1) % pois.len()];
        let mid = (start + end) * 0.5;
        let dir = (end - start).normalize_or_zero();
        let perp = Vec2::new(-dir.y, dir.x);
        let seg_len = (end - start).length();
        let control = mid + perp * (seg_len * bezier_warp);

        let mut samples: Vec<PathSample> = Vec::new();
        // Premier sample : exactement le départ (assure couture polyline propre).
        samples.push(PathSample {
            pos: start,
            tangent: bezier_tangent(start, control, end, 0.0),
            tier,
        });

        let mut last_pos = start;
        let mut acc_dist = 0.0_f32;
        for step in 1..=steps_per_segment {
            let t = step as f32 / steps_per_segment as f32;
            let pos = bezier_quadratic(start, control, end, t);
            let delta = (pos - last_pos).length();
            if delta < 1e-5 { continue; }
            acc_dist += delta;
            while acc_dist >= SAMPLE_INTERVAL_M {
                let overshoot = acc_dist - SAMPLE_INTERVAL_M;
                let frac_back = (overshoot / delta).clamp(0.0, 1.0);
                let t_sample = t - frac_back * dt;
                let p = bezier_quadratic(start, control, end, t_sample);
                let tg = bezier_tangent(start, control, end, t_sample);
                samples.push(PathSample { pos: p, tangent: tg, tier });
                acc_dist -= SAMPLE_INTERVAL_M;
            }
            last_pos = pos;
        }
        // Dernier sample : exactement l'arrivée.
        samples.push(PathSample {
            pos: end,
            tangent: bezier_tangent(start, control, end, 1.0),
            tier,
        });

        if samples.len() >= 2 {
            polylines.push(PathPolyline { samples });
        }
    }
    PathNetwork { polylines }
}

/// Build a single Bezier polyline between two POIs (open segment, no ring
/// closure). Used by village radial roads where each path is start → end
/// and not a closed loop.
///
/// Returns an empty polyline if `start == end`.
pub fn build_path_segment(start: Vec2, end: Vec2, tier: RoadTier, warp: f32) -> PathPolyline {
    let dir = (end - start).normalize_or_zero();
    if dir == Vec2::ZERO {
        return PathPolyline::default();
    }
    let perp = Vec2::new(-dir.y, dir.x);
    let seg_len = (end - start).length();
    let mid = (start + end) * 0.5;
    let control = mid + perp * (seg_len * warp);

    let steps_per_segment = 1 << BEZIER_SUBDIVISIONS;
    let dt = 1.0 / steps_per_segment as f32;
    let mut samples: Vec<PathSample> = Vec::new();
    samples.push(PathSample {
        pos: start,
        tangent: bezier_tangent(start, control, end, 0.0),
        tier,
    });

    let mut last_pos = start;
    let mut acc_dist = 0.0_f32;
    for step in 1..=steps_per_segment {
        let t = step as f32 / steps_per_segment as f32;
        let pos = bezier_quadratic(start, control, end, t);
        let delta = (pos - last_pos).length();
        if delta < 1e-5 {
            continue;
        }
        acc_dist += delta;
        while acc_dist >= SAMPLE_INTERVAL_M {
            let overshoot = acc_dist - SAMPLE_INTERVAL_M;
            let frac_back = (overshoot / delta).clamp(0.0, 1.0);
            let t_sample = t - frac_back * dt;
            let p = bezier_quadratic(start, control, end, t_sample);
            let tg = bezier_tangent(start, control, end, t_sample);
            samples.push(PathSample { pos: p, tangent: tg, tier });
            acc_dist -= SAMPLE_INTERVAL_M;
        }
        last_pos = pos;
    }
    samples.push(PathSample {
        pos: end,
        tangent: bezier_tangent(start, control, end, 1.0),
        tier,
    });
    PathPolyline { samples }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_network_when_too_few_pois() {
        let net = build_path_network(&[Vec2::ZERO], RoadTier::Trail, 0.0);
        assert!(net.polylines.is_empty());
    }

    #[test]
    fn ring_of_two_pois_produces_polylines() {
        let pois = vec![Vec2::ZERO, Vec2::new(60.0, 0.0)];
        let net = build_path_network(&pois, RoadTier::Secondary, 0.2);
        assert_eq!(net.polylines.len(), 2, "ring of 2 POIs = 2 segments");
        for pl in &net.polylines {
            assert!(pl.samples.len() >= 5, "polyline should have samples");
            let t = pl.samples[0].tangent;
            assert!((t.length() - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn half_width_monotone() {
        assert!(RoadTier::Primary.half_width() > RoadTier::Secondary.half_width());
        assert!(RoadTier::Secondary.half_width() > RoadTier::Trail.half_width());
    }
}
