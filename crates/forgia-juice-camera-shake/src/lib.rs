//! forgia-juice-camera-shake — Trauma-based camera shake (Squirrel Eiserloh, GDC 2016).
//!
//! Attach `CameraShake` component to a Camera entity. Emit `ShakeImpulse` to add trauma.
//! Trauma decays exponentially. Offset = max_amp * trauma^2 * noise().

use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy)]
pub struct CameraShake {
    /// Current trauma 0..1. Squared for offset (so low trauma = barely visible).
    pub trauma: f32,
    /// Trauma decay per second.
    pub decay: f32,
    /// Max translation amplitude in meters at trauma = 1.
    pub max_translation: f32,
    /// Max rotation amplitude in radians at trauma = 1.
    pub max_rotation: f32,
    /// Internal seed for pseudo-random offset.
    pub seed: u32,
}

impl Default for CameraShake {
    fn default() -> Self {
        Self {
            trauma: 0.0,
            decay: 1.5,
            max_translation: 0.06,
            max_rotation: 0.04,
            seed: 0,
        }
    }
}

/// Add trauma to all `CameraShake` cameras (clamped to 1.0).
#[derive(Message, Debug, Clone, Copy)]
pub struct ShakeImpulse {
    pub trauma: f32,
}

pub struct ForgiaJuiceCameraShakePlugin;

impl Plugin for ForgiaJuiceCameraShakePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShakeImpulse>()
            .add_systems(Update, (consume_impulses, apply_shake));
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

// Simple deterministic noise (hash-based) — avoids rand dep.
fn hash_noise(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x27d4_eb2du32).wrapping_add(0x9e37_79b9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x21f0_aaad);
    x ^= x >> 15;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn apply_shake(time: Res<Time>, mut q: Query<(&mut CameraShake, &mut Transform)>) {
    let dt = time.delta_secs();
    let t_now = (time.elapsed_secs() * 60.0) as u32;
    for (mut shake, mut xf) in &mut q {
        if shake.trauma <= 0.0 { continue; }
        let intensity = shake.trauma * shake.trauma;
        shake.seed = shake.seed.wrapping_add(1);
        let nx = hash_noise(shake.seed.wrapping_add(t_now));
        let ny = hash_noise(shake.seed.wrapping_add(t_now ^ 0x55));
        let nz = hash_noise(shake.seed.wrapping_add(t_now ^ 0xAA));
        let nr = hash_noise(shake.seed.wrapping_add(t_now ^ 0xFF));

        // Note: this OFFSETS the camera each frame. Caller must ensure the
        // base camera transform is restored by parent FpsCamera logic.
        xf.translation += Vec3::new(nx, ny, nz) * shake.max_translation * intensity;
        xf.rotate_local_z(nr * shake.max_rotation * intensity);

        shake.trauma = (shake.trauma - shake.decay * dt).max(0.0);
    }
}
