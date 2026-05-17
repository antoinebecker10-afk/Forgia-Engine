//! # forgia-spline
//!
//! Reusable spline/Bezier utilities for procgen, AI paths, cinematics, terrain.
//!
//! Currently houses the **canonical Bezier polyline sampler** used by :
//! - `forgia-terrain::paths` (road network ribbon mesh)
//! - `forgia-village-generator` (radial roads between buildings)
//!
//! ## Design — pure data
//!
//! No Plugin, no system, no Resource. Just functions + small data types. The
//! caller (Bevy or pure logic) owns the lifecycle. This lets `forgia-spline`
//! be imported by pure crates (`forgia-village-generator`) that themselves
//! don't depend on Bevy ECS — only `bevy::math::Vec2`.
//!
//! ## Sources
//!
//! - Bezier quadratic formula : B(t) = (1-t)²·P₀ + 2(1-t)t·P₁ + t²·P₂
//! - Derivative for tangent  : B'(t) = 2(1-t)(P₁-P₀) + 2t(P₂-P₁)

use bevy::math::Vec2;

// ─────────────────────────────────────────────────────────────────────────────
// Quadratic Bezier — the workhorse for road segments
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate quadratic Bezier at parameter `t` ∈ [0, 1].
pub fn bezier_quadratic(start: Vec2, control: Vec2, end: Vec2, t: f32) -> Vec2 {
    let inv = 1.0 - t;
    inv * inv * start + 2.0 * inv * t * control + t * t * end
}

/// Evaluate **unit tangent** of quadratic Bezier at parameter `t`. Returns
/// `Vec2::X` if the curve is degenerate (zero length) — never NaN.
pub fn bezier_tangent(start: Vec2, control: Vec2, end: Vec2, t: f32) -> Vec2 {
    let inv = 1.0 - t;
    let raw = 2.0 * inv * (control - start) + 2.0 * t * (end - control);
    let n = raw.length();
    if n > 1e-4 {
        raw / n
    } else {
        Vec2::X
    }
}

/// One sample along a polyline : XY position + unit tangent.
#[derive(Clone, Copy, Debug)]
pub struct PolylineSample {
    pub pos: Vec2,
    pub tangent: Vec2,
}

/// Default sample spacing (m) — matches `forgia-terrain::paths`. Caller can
/// override per-call via [`sample_bezier_segment_with_interval`].
pub const DEFAULT_SAMPLE_INTERVAL_M: f32 = 3.0;

/// Default Bezier subdivision power-of-two for parameter discretization.
const BEZIER_SUBDIVISIONS: u32 = 4;

/// Sample a quadratic Bezier segment uniformly at `interval_m` arc-length
/// spacing. Always emits the **exact start** and **exact end** as the first
/// and last samples (so segments couture neatly).
///
/// Returns an empty Vec if `start == end` or `interval_m <= 0`.
pub fn sample_bezier_segment_with_interval(
    start: Vec2,
    control: Vec2,
    end: Vec2,
    interval_m: f32,
) -> Vec<PolylineSample> {
    if interval_m <= 0.0 || (end - start).length() < 1e-5 {
        return Vec::new();
    }
    let steps_per_segment: u32 = 1 << BEZIER_SUBDIVISIONS;
    let dt = 1.0 / steps_per_segment as f32;

    let mut samples: Vec<PolylineSample> = Vec::with_capacity(16);
    samples.push(PolylineSample {
        pos: start,
        tangent: bezier_tangent(start, control, end, 0.0),
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
        while acc_dist >= interval_m {
            let overshoot = acc_dist - interval_m;
            let frac_back = (overshoot / delta).clamp(0.0, 1.0);
            let t_sample = t - frac_back * dt;
            samples.push(PolylineSample {
                pos: bezier_quadratic(start, control, end, t_sample),
                tangent: bezier_tangent(start, control, end, t_sample),
            });
            acc_dist -= interval_m;
        }
        last_pos = pos;
    }

    samples.push(PolylineSample {
        pos: end,
        tangent: bezier_tangent(start, control, end, 1.0),
    });
    samples
}

/// Convenience wrapper with [`DEFAULT_SAMPLE_INTERVAL_M`].
pub fn sample_bezier_segment(start: Vec2, control: Vec2, end: Vec2) -> Vec<PolylineSample> {
    sample_bezier_segment_with_interval(start, control, end, DEFAULT_SAMPLE_INTERVAL_M)
}

/// Build a curved segment between two POIs by deriving an automatic control
/// point perpendicular to the chord at midpoint, offset by `warp × chord_len`.
///
/// `warp = 0` → straight line. `warp = 0.25` → mild curve (forgia-terrain default).
pub fn arc_control_from_chord(start: Vec2, end: Vec2, warp: f32) -> Vec2 {
    let dir = (end - start).normalize_or_zero();
    let perp = Vec2::new(-dir.y, dir.x);
    let mid = (start + end) * 0.5;
    let seg_len = (end - start).length();
    mid + perp * (seg_len * warp)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_endpoints_exact() {
        let s = Vec2::new(0.0, 0.0);
        let c = Vec2::new(5.0, 5.0);
        let e = Vec2::new(10.0, 0.0);
        assert_eq!(bezier_quadratic(s, c, e, 0.0), s);
        assert_eq!(bezier_quadratic(s, c, e, 1.0), e);
    }

    #[test]
    fn bezier_midpoint_matches_formula() {
        let s = Vec2::ZERO;
        let c = Vec2::new(0.0, 10.0);
        let e = Vec2::new(10.0, 0.0);
        let m = bezier_quadratic(s, c, e, 0.5);
        // B(0.5) = 0.25 P0 + 0.5 P1 + 0.25 P2 = (2.5, 5.0)
        assert!((m.x - 2.5).abs() < 1e-4);
        assert!((m.y - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tangent_is_unit_or_x_fallback() {
        let s = Vec2::ZERO;
        let c = Vec2::new(5.0, 5.0);
        let e = Vec2::new(10.0, 0.0);
        let t = bezier_tangent(s, c, e, 0.5);
        assert!((t.length() - 1.0).abs() < 1e-4);
        // Degenerate case : start == end == control.
        let degen = bezier_tangent(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO, 0.5);
        assert_eq!(degen, Vec2::X);
    }

    #[test]
    fn sample_starts_and_ends_at_endpoints() {
        let s = Vec2::new(0.0, 0.0);
        let e = Vec2::new(60.0, 0.0);
        let c = arc_control_from_chord(s, e, 0.0); // straight
        let samples = sample_bezier_segment(s, c, e);
        assert!(samples.len() >= 2);
        assert_eq!(samples.first().unwrap().pos, s);
        assert_eq!(samples.last().unwrap().pos, e);
    }

    #[test]
    fn sample_count_matches_interval() {
        // Straight chord 60m, interval 3m → 21 samples (0..20 inclusive).
        // Allow ±1 slack for floating accumulation.
        let s = Vec2::ZERO;
        let e = Vec2::new(60.0, 0.0);
        let c = arc_control_from_chord(s, e, 0.0);
        let samples = sample_bezier_segment_with_interval(s, c, e, 3.0);
        assert!(
            samples.len() >= 20 && samples.len() <= 22,
            "expected ~21 samples, got {}",
            samples.len()
        );
    }

    #[test]
    fn arc_control_warp_zero_is_midpoint() {
        let s = Vec2::new(0.0, 0.0);
        let e = Vec2::new(10.0, 0.0);
        let c = arc_control_from_chord(s, e, 0.0);
        assert_eq!(c, Vec2::new(5.0, 0.0));
    }

    #[test]
    fn arc_control_warp_perp_correct() {
        // Chord along +X, warp 1.0 → control perpendicular offset = chord length.
        let s = Vec2::ZERO;
        let e = Vec2::new(10.0, 0.0);
        let c = arc_control_from_chord(s, e, 1.0);
        // perp for +X is (0, +1) — control should be at (5, 10) since seg_len=10.
        assert!((c.x - 5.0).abs() < 1e-4);
        assert!((c.y - 10.0).abs() < 1e-4);
    }

    #[test]
    fn sample_empty_for_zero_interval() {
        let samples = sample_bezier_segment_with_interval(
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::new(10.0, 0.0),
            0.0,
        );
        assert!(samples.is_empty());
    }

    #[test]
    fn sample_empty_for_zero_chord() {
        let samples = sample_bezier_segment(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO);
        assert!(samples.is_empty());
    }
}
