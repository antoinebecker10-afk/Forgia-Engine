//! forgia-juice-camera-shake — Trauma-based camera shake.
//!
//! Pattern : Squirrel Eiserloh GDC 2016 "Math for Game Programmers: Juicing your
//! Cameras with Math". Trauma 0..1, offset = max_amp × trauma² × noise.
//!
//! ## Design AAA-safe 2024-2025 (research veille 2026-05-18)
//!
//! - **Rotation only** — pas de translation pour éviter motion sickness +
//!   accumulation drift (translation += per frame sans reset = camera dérive)
//! - **Upward-biased pitch** (recoil illusion), pas circulaire random
//! - **Décroissance exponentielle rapide** (~125ms full decay) — pas 500ms+
//! - **Magnitude conservatrice** : 0.3-1.5° par tir selon arme (AAA range)
//! - Slider 0-100% via [`CameraShakeIntensity`] Resource (XAG-117 accessibility)
//!
//! ## Ordering critique
//!
//! `apply_shake` DOIT run APRÈS le système qui set la rotation cam (mouse_look
//! dans forgia-player). Sinon le shake est écrasé avant render.
//! → utiliser un SystemSet ordering ou explicit `.after(...)`.

use bevy::prelude::*;

/// Component attaché à la caméra. Trauma s'accumule via `ShakeImpulse` puis decay.
#[derive(Component, Debug, Clone, Copy)]
pub struct CameraShake {
    /// Trauma 0..1. Squared pour l'offset (low trauma = invisible).
    pub trauma: f32,
    /// Trauma decay par seconde. 8.0 = ~125ms full decay = AAA standard.
    pub decay: f32,
    /// Amplitude max rotation en radians à trauma = 1. Default 0.026 rad ≈ 1.5°.
    pub max_rotation: f32,
    /// Seed pour noise déterministe.
    pub seed: u32,
}

impl Default for CameraShake {
    fn default() -> Self {
        Self {
            trauma: 0.0,
            decay: 8.0,            // ~125ms full decay (AAA 80-150ms range)
            max_rotation: 0.026,   // ~1.5° max amplitude
            seed: 0,
        }
    }
}

/// Resource accessibilité — slider 0-100% (XAG-117).
/// `apply_shake` multiplie l'amplitude effective par ce facteur.
#[derive(Resource, Debug, Clone, Copy)]
pub struct CameraShakeIntensity(pub f32);

impl Default for CameraShakeIntensity {
    fn default() -> Self {
        Self(1.0) // 100% par défaut
    }
}

/// Message émis par les systèmes gameplay (fire weapon, explosion, damage...).
/// Ajoute du trauma à toutes les caméras avec `CameraShake`.
#[derive(Message, Debug, Clone, Copy)]
pub struct ShakeImpulse {
    /// Quantité de trauma ajoutée (0..1). Typiquement 0.04-0.22 selon arme.
    pub trauma: f32,
}

pub struct ForgiaJuiceCameraShakePlugin;

impl Plugin for ForgiaJuiceCameraShakePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShakeImpulse>()
            .init_resource::<CameraShakeIntensity>()
            .add_systems(Update, (consume_impulses, apply_shake).chain());
    }
}

fn consume_impulses(
    mut events: MessageReader<ShakeImpulse>,
    mut cams: Query<&mut CameraShake>,
) {
    for ev in events.read() {
        for mut s in &mut cams {
            s.trauma = (s.trauma + ev.trauma).clamp(0.0, 1.0);
        }
    }
}

/// Hash-based deterministic noise pour offset reproductible.
fn hash_noise(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x27d4_eb2d).wrapping_add(0x9e37_79b9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x21f0_aaad);
    x ^= x >> 15;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Applique l'offset rotation au Transform de la cam (rotation-only, pas de translation).
///
/// IMPORTANT : ce système DOIT run après que la rotation de base soit set par
/// mouse_look. Le shake est multiplié par-dessus, et mouse_look reset à chaque
/// frame → pas d'accumulation drift.
///
/// Bias : pitch légèrement amplifié pour effet "recoil up" (pas circulaire random).
pub fn apply_shake(
    time: Res<Time>,
    intensity_res: Res<CameraShakeIntensity>,
    mut q: Query<(&mut CameraShake, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let t_now = (time.elapsed_secs() * 60.0) as u32;
    let user_intensity = intensity_res.0.clamp(0.0, 1.0);

    for (mut shake, mut xf) in &mut q {
        if shake.trauma <= 0.001 {
            shake.trauma = 0.0;
            continue;
        }
        // intensity = trauma² × user_slider — Squirrel pattern + accessibility.
        let intensity = shake.trauma * shake.trauma * user_intensity;
        shake.seed = shake.seed.wrapping_add(1);

        let n_pitch = hash_noise(shake.seed.wrapping_add(t_now));
        let n_yaw = hash_noise(shake.seed.wrapping_add(t_now ^ 0x55));
        let n_roll = hash_noise(shake.seed.wrapping_add(t_now ^ 0xAA));

        // Upward bias : pitch toujours skewé négatif (camera look UP) pour
        // effet recoil naturel — pas full random qui donne nausée (Apex anti-pattern).
        let pitch_skewed = -n_pitch.abs() * 0.7 + n_pitch * 0.3;

        let pitch_amp = pitch_skewed * shake.max_rotation * intensity;
        let yaw_amp = n_yaw * shake.max_rotation * 0.4 * intensity; // yaw 40% du pitch
        let roll_amp = n_roll * shake.max_rotation * 0.3 * intensity; // roll subtil

        // Multiplicatif : applique en local sur la rotation déjà set par mouse_look.
        xf.rotation = xf.rotation
            * Quat::from_rotation_x(pitch_amp)
            * Quat::from_rotation_y(yaw_amp)
            * Quat::from_rotation_z(roll_amp);

        shake.trauma = (shake.trauma - shake.decay * dt).max(0.0);
    }
}

pub mod prelude {
    pub use crate::{
        CameraShake, CameraShakeIntensity, ForgiaJuiceCameraShakePlugin, ShakeImpulse,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaJuiceCameraShakePlugin;
    }

    #[test]
    fn camera_shake_default_sane() {
        let s = CameraShake::default();
        assert_eq!(s.trauma, 0.0);
        assert!(s.decay >= 4.0 && s.decay <= 16.0, "decay AAA range 60-250ms");
        // max_rotation < 0.05 rad (~3°) hard upper bound anti-nausée.
        assert!(s.max_rotation > 0.0 && s.max_rotation < 0.05);
    }

    #[test]
    fn intensity_default_100_percent() {
        let i = CameraShakeIntensity::default();
        assert!((i.0 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hash_noise_in_range() {
        for seed in [1u32, 42, 12345, u32::MAX / 2] {
            let v = hash_noise(seed);
            assert!((-1.0..=1.0).contains(&v));
        }
    }
}
