//! # aim_assist — Story-528 AC1
//!
//! Aim drag accessibility (cible Roblox kids + casual + femmes 20-35).
//! Lerp camera aim vers la cible la plus proche dans un cône frontal,
//! par un facteur `strength` (0.0 = off, 1.0 = max).
//!
//! Pattern : pas de snap dur (anti aim-bot). On bias progressivement le
//! yaw/pitch du player vers la direction de la nearest target enemy si
//! elle est dans le cône d'engagement. Tuning hot-reload via `fps_tuning.toml`.
//!
//! Cross-mode FPS + Roguelite : filtre les entités `Health + Mortal Without<Player>`,
//! pas couplé à un archetype enemy spécifique.

use bevy::prelude::*;
use forgia_core::prelude::*;
use forgia_damage::{Health, Mortal};
use forgia_player::prelude::*;

/// Tuning aim assist (hot-reload via fps_tuning.toml [aim_assist]).
#[derive(Resource, Debug, Clone, Copy)]
pub struct AimAssistTuning {
    /// Force d'attraction 0.0..1.0. 0.0 = désactivé. Default 0.5.
    pub strength: f32,
    /// Demi-angle max d'engagement (degrés). Au-delà : pas d'effet.
    pub max_angle_deg: f32,
    /// Distance max d'engagement (mètres).
    pub engage_distance_m: f32,
}

impl Default for AimAssistTuning {
    fn default() -> Self {
        Self {
            strength: 0.5,
            max_angle_deg: 5.0,
            engage_distance_m: 50.0,
        }
    }
}

/// System aim assist — applique un bias yaw/pitch vers la nearest target dans
/// le cône d'engagement. Tourne en `GameSet::Camera` après mouse_look.
pub fn aim_assist_system(
    time: Res<Time>,
    tuning: Res<AimAssistTuning>,
    mut metrics: ResMut<FpsFeelMetrics>,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    mut q_player: Query<(&mut Transform, &mut Player), Without<FpsCamera>>,
    mut q_cam_local: Query<&mut Transform, (With<FpsCamera>, Without<Player>)>,
    q_targets: Query<&GlobalTransform, (With<Health>, With<Mortal>, Without<Player>)>,
) {
    if tuning.strength <= 0.0 {
        return;
    }
    let Ok(cam_tf) = q_cam.single() else {
        return;
    };
    let Ok((mut player_tf, mut player)) = q_player.single_mut() else {
        return;
    };

    let cam_pos = cam_tf.translation();
    let cam_forward = cam_tf.forward().as_vec3();
    let max_cos = tuning.max_angle_deg.to_radians().cos();
    let max_dist_sq = tuning.engage_distance_m * tuning.engage_distance_m;

    // Trouve la target la plus proche angulairement dans le cône + distance.
    let mut best: Option<(Vec3, f32)> = None; // (target_pos, dot)
    for tf in &q_targets {
        let to_target = tf.translation() - cam_pos;
        let dist_sq = to_target.length_squared();
        if dist_sq < 0.01 || dist_sq > max_dist_sq {
            continue;
        }
        let dir = to_target / dist_sq.sqrt();
        let dot = dir.dot(cam_forward);
        if dot < max_cos {
            continue;
        }
        match best {
            Some((_, best_dot)) if best_dot >= dot => {}
            _ => best = Some((tf.translation(), dot)),
        }
    }

    let Some((target_pos, _)) = best else {
        return;
    };

    // Compute desired yaw/pitch pour viser target_pos depuis cam_pos.
    let to_target = target_pos - cam_pos;
    let desired_yaw = (-to_target.x).atan2(-to_target.z);
    let horiz_dist = (to_target.x * to_target.x + to_target.z * to_target.z).sqrt();
    let desired_pitch = to_target.y.atan2(horiz_dist.max(0.001));

    // Lerp factor : strength × dt × constante (rad/sec à strength=1).
    let lerp_rate = 4.0; // ~250ms half-life à strength=1
    let alpha = (tuning.strength.clamp(0.0, 1.0) * lerp_rate * time.delta_secs()).min(1.0);

    let yaw_delta = shortest_angle(player.yaw, desired_yaw);
    let pitch_delta = desired_pitch - player.pitch;

    if yaw_delta.abs() < 1e-4 && pitch_delta.abs() < 1e-4 {
        return;
    }

    player.yaw += yaw_delta * alpha;
    player.pitch = (player.pitch + pitch_delta * alpha).clamp(-1.5, 1.5);
    player_tf.rotation = Quat::from_rotation_y(player.yaw);
    if let Ok(mut cam_local) = q_cam_local.single_mut() {
        cam_local.rotation = Quat::from_rotation_x(player.pitch);
    }

    metrics.aim_assist_engagements_total =
        metrics.aim_assist_engagements_total.saturating_add(1);
}

/// Compute le delta angulaire le plus court entre 2 angles (rad).
fn shortest_angle(from: f32, to: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    let mut d = (to - from) % two_pi;
    if d > std::f32::consts::PI {
        d -= two_pi;
    } else if d < -std::f32::consts::PI {
        d += two_pi;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuning_default_strength_05() {
        let t = AimAssistTuning::default();
        assert_eq!(t.strength, 0.5);
        assert_eq!(t.max_angle_deg, 5.0);
        assert_eq!(t.engage_distance_m, 50.0);
    }

    #[test]
    fn shortest_angle_no_wrap() {
        let d = shortest_angle(0.0, 1.0);
        assert!((d - 1.0).abs() < 1e-5);
    }

    #[test]
    fn shortest_angle_wraps_negative() {
        // De 3.0 vers -3.0 : delta court = +0.28 (vers +PI puis wrap), pas -6.0.
        let d = shortest_angle(3.0, -3.0);
        assert!(d > 0.0 && d < std::f32::consts::PI);
    }

    #[test]
    fn shortest_angle_wraps_positive() {
        let d = shortest_angle(-3.0, 3.0);
        assert!(d < 0.0 && d > -std::f32::consts::PI);
    }
}
