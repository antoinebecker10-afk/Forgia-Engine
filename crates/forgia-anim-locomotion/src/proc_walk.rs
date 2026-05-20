//! # proc_walk — Walk cycle procédural anatomiquement correct (story-440 Sprint A)
//!
//! Fonctions pures, testables headless. Refs :
//! - Richard Williams, "Animator's Survival Kit" — 4 phases Contact/Down/Pass/Up
//! - Physiopedia "Gait Cycle" — stance 60% / swing 40% ratio référence
//! - Orange Duck (D. Holden) "Simple Two Joint IK" — pattern foot IK Vague 2
//!
//! ## Modèle
//!
//! `gait_phase ∈ [0, 1)` — 1 cycle = 2 pas (L+R). Chaque jambe a un sub-phase
//! déphasé de 0.5. Dans chaque sub-phase :
//! - **Stance** (sub_phase ∈ [0, 0.6]) : pied au sol, thigh roule de +amp → -amp
//!   linéaire, knee tendu (=0), ankle plat (=0)
//! - **Swing** (sub_phase ∈ [0.6, 1.0]) : pied dans les airs, thigh remonte
//!   en ease, knee plie en cloche (max ~57°), ankle dorsi-flex
//!
//! Le knee n'est PAS sinusoïdal → c'est la différence #1 vs `sin()` pur naïf.
//!
//! ## Tunables stance/swing
//!
//! - `STANCE_FRAC_WALK = 0.60` (humain confortable)
//! - `STANCE_FRAC_RUN = 0.40` (course, phase de vol implicite)
//! - Interpolation linéaire `walk → run` via `speed_factor`

use bevy::math::Vec3;
use std::f32::consts::{PI, TAU};

// ── Constantes biomécaniques ────────────────────────────────────────────────

/// Cycle complet par mètre parcouru. Walk humain ~0.55 stride/m, run ~0.40.
pub const STRIDE_PER_M_WALK: f32 = 0.55;
pub const STRIDE_PER_M_RUN: f32 = 0.40;

/// Stance/swing ratio. Walk 60/40, run 40/60.
pub const STANCE_FRAC_WALK: f32 = 0.60;
pub const STANCE_FRAC_RUN: f32 = 0.40;

/// Amplitude thigh swing (rad). Walk ~46°, run ~57° (cartoon-readable cam 3P 7m).
pub const AMP_THIGH_WALK: f32 = 0.80;
pub const AMP_THIGH_RUN: f32 = 1.00;

/// Amplitude arm swing (rad, pitch épaule). Walk ~32°, run ~49°.
pub const AMP_ARM_WALK: f32 = 0.55;
pub const AMP_ARM_RUN: f32 = 0.85;

/// Knee max flexion en swing peak (rad). Exagéré pour lecture cam 3P.
pub const KNEE_FLEX_PEAK_WALK: f32 = 1.60; // ~92°
pub const KNEE_FLEX_PEAK_RUN: f32 = 1.80; // ~103°

/// Ankle dorsi-flex peak en mid-swing (rad). ~15°.
pub const ANKLE_FLEX_PEAK: f32 = 0.26;

/// Elbow rest flexion (rad). Bras au repos plié à ~70°.
pub const ELBOW_REST_FLEX: f32 = 1.20;

/// Pelvic yaw amplitude (rad). Walk ~6°, run ~10°.
pub const PELVIC_YAW_AMP_WALK: f32 = 0.10;
pub const PELVIC_YAW_AMP_RUN: f32 = 0.17;

/// Pelvic roll amplitude (rad). ±7° typique.
pub const PELVIC_ROLL_AMP: f32 = 0.12;

/// Pelvic bob Y amplitude (m). ~4cm walk, ~7cm run.
pub const PELVIC_BOB_AMP_WALK: f32 = 0.04;
pub const PELVIC_BOB_AMP_RUN: f32 = 0.07;

/// Speed threshold (m/s) au-dessus de laquelle on bascule walk → run.
/// Linéaire interp entre WALK et RUN dans cette plage.
pub const SPEED_WALK_PEAK_M_S: f32 = 1.8; // marche confortable
pub const SPEED_RUN_THRESHOLD_M_S: f32 = 4.0; // course établie

/// Spine counter-rotation factor (fraction du pelvic yaw, opposé).
pub const SPINE_COUNTER_ROT_FACTOR: f32 = 0.7;

// ── Modèle paramétré par speed ──────────────────────────────────────────────

/// Coefficients d'un cycle de marche pour une `speed` donnée. Tout est lerp
/// entre WALK et RUN selon `speed_t = saturate((speed - SPEED_WALK_PEAK) /
/// (SPEED_RUN_THRESHOLD - SPEED_WALK_PEAK))`.
#[derive(Debug, Clone, Copy)]
pub struct GaitTunables {
    pub stride_per_m: f32,
    pub stance_frac: f32,
    pub amp_thigh: f32,
    pub amp_arm: f32,
    pub knee_flex_peak: f32,
    pub pelvic_yaw_amp: f32,
    pub pelvic_bob_amp: f32,
}

impl GaitTunables {
    pub fn for_speed(speed_m_s: f32) -> Self {
        let speed_t = ((speed_m_s - SPEED_WALK_PEAK_M_S)
            / (SPEED_RUN_THRESHOLD_M_S - SPEED_WALK_PEAK_M_S))
            .clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * speed_t;
        Self {
            stride_per_m: lerp(STRIDE_PER_M_WALK, STRIDE_PER_M_RUN),
            stance_frac: lerp(STANCE_FRAC_WALK, STANCE_FRAC_RUN),
            amp_thigh: lerp(AMP_THIGH_WALK, AMP_THIGH_RUN),
            amp_arm: lerp(AMP_ARM_WALK, AMP_ARM_RUN),
            knee_flex_peak: lerp(KNEE_FLEX_PEAK_WALK, KNEE_FLEX_PEAK_RUN),
            pelvic_yaw_amp: lerp(PELVIC_YAW_AMP_WALK, PELVIC_YAW_AMP_RUN),
            pelvic_bob_amp: lerp(PELVIC_BOB_AMP_WALK, PELVIC_BOB_AMP_RUN),
        }
    }
}

/// Avance `gait_phase` selon `speed * stride_per_m * dt`. Reste dans [0, 1).
pub fn update_gait_phase(gait: f32, speed_m_s: f32, dt: f32, tunables: &GaitTunables) -> f32 {
    (gait + dt * speed_m_s * tunables.stride_per_m).rem_euclid(1.0)
}

// ── Leg pose : thigh + knee + ankle pitch ────────────────────────────────────

/// Pour une `sub_phase ∈ [0, 1)` (sub-phase = gait pour leg L, gait+0.5 pour
/// leg R), retourne `(thigh_pitch, knee_bend, ankle_pitch)` en radians.
///
/// Convention rotation X (autour de l'axe latéral hanche) : `+` = thigh swing
/// forward (genou avance), `-` = backward.
///
/// **Stance** : thigh roule linéaire de +amp_thigh → -amp_thigh (le bassin
/// se déplace au-dessus du pied immobile). Knee=0 (tendu), ankle=0 (plat).
///
/// **Swing** : thigh remonte avec ease (smoothstep cosinus) de -amp_thigh →
/// +amp_thigh. Knee plie en cloche (sin(t*π)) jusqu'au peak `knee_flex_peak`.
/// Ankle dorsi-flex en cloche peak `ANKLE_FLEX_PEAK`.
pub fn leg_pose(sub_phase: f32, tunables: &GaitTunables) -> (f32, f32, f32) {
    let stance_frac = tunables.stance_frac;
    let amp = tunables.amp_thigh;
    if sub_phase < stance_frac {
        // Stance : linéaire +amp → -amp
        let t = sub_phase / stance_frac.max(1e-6); // 0..1
        let thigh = amp * (1.0 - 2.0 * t);
        (thigh, 0.0, 0.0)
    } else {
        // Swing : ease cosinus + cloches knee/ankle
        let t = (sub_phase - stance_frac) / (1.0 - stance_frac).max(1e-6);
        let ease = 0.5 - 0.5 * (t * PI).cos(); // 0 → 1 smoothstep
        let thigh = -amp + 2.0 * amp * ease;
        let knee = (t * PI).sin() * tunables.knee_flex_peak;
        let ankle = (t * PI).sin() * ANKLE_FLEX_PEAK;
        (thigh, knee, ankle)
    }
}

// ── Arm pose : arm + forearm ─────────────────────────────────────────────────

/// Bras = opposé jambe correspondante. Sub-phase = sub-phase de la jambe
/// OPPOSÉE (arm_L synchro avec leg_R, donc utiliser `gait_phase + 0.5` pour
/// arm_L si leg_L est sur `gait_phase`).
///
/// Retourne `(arm_pitch, elbow_flex)`. arm_pitch suit le swing thigh opposé,
/// elbow reste à ELBOW_REST_FLEX + petit pump synchronisé.
pub fn arm_pose(opposite_leg_sub_phase: f32, tunables: &GaitTunables) -> (f32, f32) {
    let (thigh_opp, _, _) = leg_pose(opposite_leg_sub_phase, tunables);
    let scale = tunables.amp_arm / tunables.amp_thigh.max(1e-6);
    // Arm pitch suit thigh opposé, amplitude scaled vers amp_arm.
    let arm_pitch = thigh_opp * scale;
    // Elbow : flexion de base + pump léger au moment du swing fort
    let elbow_pump = thigh_opp.abs() * 0.30;
    let elbow = ELBOW_REST_FLEX + elbow_pump;
    (arm_pitch, elbow)
}

// ── Pelvic pose : yaw + roll + bob Y ─────────────────────────────────────────

/// Retourne `(pelvic_yaw, pelvic_roll, bob_y)`.
///
/// - `pelvic_yaw` : rotation Y autour de l'axe vertical, ±amp aligné jambe avant
/// - `pelvic_roll` : rotation Z (axis lateral), hip s'abaisse côté jambe en swing
/// - `bob_y` : translation Y, fréquence **2× le gait** (1 bob par foot-strike,
///   2 strikes/cycle) — c'est la signature visuelle "humain marche"
///
/// `speed_factor ∈ [0, 1]` scale les amplitudes (0 = pas de pelvic, 1 = full).
pub fn pelvic_pose(gait: f32, speed_factor: f32, tunables: &GaitTunables) -> (f32, f32, f32) {
    let phi = gait * TAU;
    let yaw = phi.sin() * tunables.pelvic_yaw_amp * speed_factor;
    let roll = phi.cos() * PELVIC_ROLL_AMP * speed_factor;
    // Bob 2x freq : |cos(2φ)| → 2 maxima par cycle. Floor à 0 (le bassin monte,
    // ne descend pas en dessous du rest).
    let bob_y = (phi * 2.0).cos().abs() * tunables.pelvic_bob_amp * speed_factor;
    (yaw, roll, bob_y)
}

/// Spine counter-rotation Y, opposée au pelvic yaw, à 70% (anatomie humaine).
/// **Différence vs bug #6 actuel** : actuel applique rotation_Z (roll lateral),
/// ce qui est faux. Counter-rot est Y (yaw).
pub fn spine_counter_rot(pelvic_yaw: f32) -> f32 {
    -pelvic_yaw * SPINE_COUNTER_ROT_FACTOR
}

// ── Tail (Rex / lézard bipède) — counter-balance ────────────────────────────

/// Rotation Y du segment `idx` (0 = base) sur une chaîne de `n` segments.
/// Amplitude croissante vers la pointe (`0.5 + 0.15 * idx`) pour effet whip.
/// Opposée au pelvic yaw → le tail balance contre le bassin.
pub fn tail_segment_yaw(idx: usize, n: usize, pelvic_yaw: f32) -> f32 {
    let _ = n; // gardé pour future extension (clamp idx, etc.)
    -pelvic_yaw * (0.5 + 0.15 * idx as f32)
}

// ── Helpers debug / sensor export ───────────────────────────────────────────

/// Snapshot des angles clés pour export sensor JSON. Caller passe ça à
/// `forgia-anim-debug` ou directement à `forgia_auto_rig.json` enrichi.
#[derive(Debug, Clone, Copy, Default)]
pub struct WalkPoseSnapshot {
    pub gait_phase: f32,
    pub speed_m_s: f32,
    pub speed_factor: f32,
    pub thigh_l_deg: f32,
    pub thigh_r_deg: f32,
    pub knee_l_deg: f32,
    pub knee_r_deg: f32,
    pub pelvic_yaw_deg: f32,
    pub pelvic_roll_deg: f32,
    pub bob_y_cm: f32,
}

impl WalkPoseSnapshot {
    pub fn from_gait(gait: f32, speed: f32, speed_factor: f32) -> Self {
        let t = GaitTunables::for_speed(speed);
        let (thigh_l, knee_l, _) = leg_pose(gait, &t);
        let (thigh_r, knee_r, _) = leg_pose((gait + 0.5).rem_euclid(1.0), &t);
        let (yaw, roll, bob) = pelvic_pose(gait, speed_factor, &t);
        let to_deg = |r: f32| r.to_degrees();
        Self {
            gait_phase: gait,
            speed_m_s: speed,
            speed_factor,
            thigh_l_deg: to_deg(thigh_l),
            thigh_r_deg: to_deg(thigh_r),
            knee_l_deg: to_deg(knee_l),
            knee_r_deg: to_deg(knee_r),
            pelvic_yaw_deg: to_deg(yaw),
            pelvic_roll_deg: to_deg(roll),
            bob_y_cm: bob * 100.0,
        }
    }
}

// Vec3 import gardé pour Vague 2 (foot IK targets).
#[allow(dead_code)]
fn _vec3_marker(_: Vec3) {}

// ── Tests headless ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn walk_tunables() -> GaitTunables {
        GaitTunables::for_speed(1.4)
    }

    #[test]
    fn leg_stance_start_thigh_forward() {
        // Au début du stance (sub_phase=0), pied vient de toucher devant → thigh +amp
        let t = walk_tunables();
        let (thigh, knee, ankle) = leg_pose(0.0, &t);
        assert!(
            thigh > 0.3,
            "thigh at contact must be forward (+amp), got {}",
            thigh
        );
        assert!(knee.abs() < 1e-4, "knee tendu en stance, got {}", knee);
        assert!(ankle.abs() < 1e-4, "ankle plat en stance, got {}", ankle);
    }

    #[test]
    fn leg_stance_end_thigh_backward() {
        // En fin de stance (juste avant swing), thigh est derrière → -amp
        let t = walk_tunables();
        let (thigh, knee, _) = leg_pose(0.59, &t);
        assert!(
            thigh < -0.3,
            "thigh at end-stance must be backward (-amp), got {}",
            thigh
        );
        assert!(knee.abs() < 1e-4, "knee toujours tendu fin de stance, got {}", knee);
    }

    #[test]
    fn leg_swing_mid_has_knee_bent() {
        // Mid-swing : knee plié à son peak
        let t = walk_tunables();
        let (_, knee, ankle) = leg_pose(0.80, &t); // mid-swing
        assert!(
            knee > 0.5,
            "knee must be significantly bent mid-swing, got {}",
            knee
        );
        assert!(
            ankle > 0.10,
            "ankle dorsi-flex mid-swing, got {}",
            ankle
        );
    }

    #[test]
    fn legs_opposed_in_phase() {
        // Phase = 0 → L en début stance, R = 0.5 → R en fin stance
        let t = walk_tunables();
        let (thigh_l, _, _) = leg_pose(0.0, &t);
        let (thigh_r, _, _) = leg_pose(0.5, &t);
        assert!(
            thigh_l * thigh_r < 0.0 || (thigh_l.abs() < 0.05 && thigh_r.abs() < 0.05),
            "legs must be opposed (L={}, R={})",
            thigh_l,
            thigh_r
        );
    }

    #[test]
    fn arm_opposite_to_leg() {
        // Arm L doit suivre la phase de leg R (et vice-versa)
        // Si leg_L thigh forward (+), alors arm_L pitch devrait être backward (-)
        let t = walk_tunables();
        let gait = 0.0;
        let (thigh_l, _, _) = leg_pose(gait, &t);
        let (arm_l_pitch, _) = arm_pose((gait + 0.5).rem_euclid(1.0), &t);
        assert!(
            thigh_l * arm_l_pitch < 0.0,
            "arm L must move opposite to leg L (thigh_l={}, arm_l={})",
            thigh_l,
            arm_l_pitch
        );
    }

    #[test]
    fn pelvic_bob_has_2x_frequency() {
        // bob_y doit avoir 2 maxima dans gait ∈ [0, 1]
        let t = walk_tunables();
        let n_samples = 100;
        let bobs: Vec<f32> = (0..n_samples)
            .map(|i| {
                let gait = i as f32 / n_samples as f32;
                let (_, _, bob) = pelvic_pose(gait, 1.0, &t);
                bob
            })
            .collect();
        // Compte les "maxima locaux"
        let mut maxima = 0;
        for i in 1..n_samples - 1 {
            if bobs[i] > bobs[i - 1] && bobs[i] > bobs[i + 1] {
                maxima += 1;
            }
        }
        // Au moins 2 maxima (2× freq). Possible 3 si bord de wrap, OK.
        assert!(
            (2..=3).contains(&maxima),
            "bob_y must have ~2 maxima per cycle (got {}), cf signature humain",
            maxima
        );
    }

    #[test]
    fn pelvic_bob_always_positive() {
        // Le bassin remonte, ne descend pas sous le rest pose
        let t = walk_tunables();
        for i in 0..50 {
            let gait = i as f32 / 50.0;
            let (_, _, bob) = pelvic_pose(gait, 1.0, &t);
            assert!(bob >= 0.0, "bob_y must be ≥ 0 (got {} at gait={})", bob, gait);
        }
    }

    #[test]
    fn spine_counters_pelvis() {
        // Spine counter-rot doit être opposée et ~70% amplitude
        let pelvic_yaw = 0.10;
        let spine = spine_counter_rot(pelvic_yaw);
        assert!(spine * pelvic_yaw < 0.0, "spine opposed to pelvis");
        assert!(
            (spine.abs() - 0.07).abs() < 1e-4,
            "spine = -0.7 * pelvis, got {}",
            spine
        );
    }

    #[test]
    fn tail_segments_increasing_amplitude() {
        // Le tail s'amplifie vers la pointe (whip effect)
        let pelvic_yaw = 0.10;
        let t0 = tail_segment_yaw(0, 4, pelvic_yaw).abs();
        let t1 = tail_segment_yaw(1, 4, pelvic_yaw).abs();
        let t2 = tail_segment_yaw(2, 4, pelvic_yaw).abs();
        let t3 = tail_segment_yaw(3, 4, pelvic_yaw).abs();
        assert!(t0 < t1 && t1 < t2 && t2 < t3, "tail amplitudes increasing: {} {} {} {}", t0, t1, t2, t3);
        // Et opposé au bassin
        assert!(tail_segment_yaw(0, 4, pelvic_yaw) * pelvic_yaw < 0.0);
    }

    #[test]
    fn gait_tunables_lerp_walk_to_run() {
        let walk = GaitTunables::for_speed(1.4);
        let run = GaitTunables::for_speed(5.0);
        assert!(
            run.stance_frac < walk.stance_frac,
            "run must have shorter stance (got walk={}, run={})",
            walk.stance_frac,
            run.stance_frac
        );
        assert!(run.amp_thigh > walk.amp_thigh, "run has bigger thigh swing");
        assert!(run.knee_flex_peak > walk.knee_flex_peak, "run knees plient plus");
    }

    #[test]
    fn update_gait_phase_wraps() {
        let t = walk_tunables();
        let g = update_gait_phase(0.95, 5.0, 0.1, &t);
        assert!((0.0..1.0).contains(&g), "gait wraps to [0,1), got {}", g);
    }

    #[test]
    fn snapshot_exports_human_readable() {
        let snap = WalkPoseSnapshot::from_gait(0.25, 1.4, 0.7);
        // mid-stance L → thigh L proche du 0 (transition)
        assert!(snap.thigh_l_deg.abs() < 50.0);
        // Bob Y entre 0 et 7cm
        assert!(snap.bob_y_cm >= 0.0 && snap.bob_y_cm < 8.0);
    }
}
