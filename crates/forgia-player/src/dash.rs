//! # dash — "Pas de l'Apprenti" (Story-528 AC3)
//!
//! Bond rapide 4m en 0.25s déclenché par double-tap Espace.
//! 2 charges max, recharge 1.5s/charge. Voiceline *"Hop !"* à chaque usage.
//!
//! Pattern : pendant la fenêtre dash, le horizontal du player_movement est
//! overridé par une vélocité forcée (range/duration). Le vertical reste sous
//! contrôle normal (gravité, jump). Pas de double-tap sur la touche Jump :
//! détecteur dédié `DashTapDetector` qui regarde just_pressed(Jump) avec
//! cooldown <= DOUBLE_TAP_WINDOW.
//!
//! Voiceline : émise via `info!()` log (audio path TBD V2, cf. ROADMAP_ROGUELITE
//! Tier 3 TTS). L'event `DashUsedEvent` est canon pour brancher l'audio plus tard.

use bevy::prelude::*;
use bevy_rapier3d::prelude::KinematicCharacterController;
use forgia_core::prelude::*;
use forgia_input::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::Player;

const DOUBLE_TAP_WINDOW_SECS: f32 = 0.25;
const VOICELINE_HOP: &str = "Hop !";

/// Tuning hot-reloadable du dash. Valeurs default cohérentes story-528 AC3.
#[derive(Resource, Debug, Clone, Copy)]
pub struct DashTuning {
    pub range_m: f32,
    pub duration_secs: f32,
    pub cooldown_per_charge_secs: f32,
    pub max_charges: u8,
}

impl Default for DashTuning {
    fn default() -> Self {
        Self {
            range_m: 4.0,
            duration_secs: 0.25,
            cooldown_per_charge_secs: 1.5,
            max_charges: 2,
        }
    }
}

/// État runtime des charges + dash actif. Inséré au spawn Player.
#[derive(Component, Debug, Clone, Copy)]
pub struct DashState {
    pub charges: u8,
    /// Temps cumulé écoulé depuis le dernier usage (>= cooldown → +1 charge).
    pub recharge_timer_secs: f32,
    /// Temps restant dans la fenêtre de dash actif (secs). 0.0 = inactif.
    pub active_remaining_secs: f32,
    /// Direction monde XZ figée au déclenchement (yaw player).
    pub direction_xz: Vec3,
}

impl DashState {
    pub fn new(max_charges: u8) -> Self {
        Self {
            charges: max_charges,
            recharge_timer_secs: 0.0,
            active_remaining_secs: 0.0,
            direction_xz: Vec3::ZERO,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_remaining_secs > 0.0
    }
}

/// Detector double-tap Jump : retient le timestamp du dernier just_pressed Jump.
#[derive(Resource, Default)]
pub struct DashTapDetector {
    pub last_jump_secs: f32,
}

/// Event émis à chaque dash déclenché — wired par observability + audio futur.
#[derive(Message, Debug, Clone, Copy)]
pub struct DashUsedEvent;

/// System input : détecte le double-tap Jump et amorce un dash si charges dispo.
pub fn dash_input_system(
    time: Res<Time>,
    tuning: Res<DashTuning>,
    mut detector: ResMut<DashTapDetector>,
    mut metrics: ResMut<FpsFeelMetrics>,
    mut events: MessageWriter<DashUsedEvent>,
    mut q: Query<(&mut DashState, &ActionState<PlayerAction>, &Transform), With<Player>>,
) {
    let Ok((mut dash, action, tf)) = q.single_mut() else {
        return;
    };
    if !action.just_pressed(&PlayerAction::Jump) {
        return;
    }
    let now = time.elapsed_secs();
    let dt_since_last = now - detector.last_jump_secs;
    detector.last_jump_secs = now;

    if dt_since_last > DOUBLE_TAP_WINDOW_SECS {
        return;
    }
    if dash.charges == 0 || dash.is_active() {
        return;
    }
    // Direction = forward player projeté XZ. Si player static face caméra, c'est forward.
    let mut dir = tf.forward().as_vec3();
    dir.y = 0.0;
    let dir = dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return;
    }

    dash.charges = dash.charges.saturating_sub(1);
    dash.active_remaining_secs = tuning.duration_secs;
    dash.direction_xz = dir;
    dash.recharge_timer_secs = 0.0;
    detector.last_jump_secs = 0.0; // empêche triple-tap de re-déclencher.

    metrics.dash_uses_total = metrics.dash_uses_total.saturating_add(1);
    events.write(DashUsedEvent);
    info!("[forgia-player::dash] {VOICELINE_HOP} (charges restantes: {})", dash.charges);
}

/// System motion : pendant la fenêtre active, override la translation horizontal
/// du KCC. Le player_movement applique son propre move_vec ; on additionne
/// l'impulse dash en pre-pass. Ici : on remplace si dash actif.
///
/// Ordonné AVANT player_movement dans GameSet::Movement.
pub fn dash_motion_system(
    time: Res<Time>,
    tuning: Res<DashTuning>,
    mut q: Query<(&mut DashState, &mut KinematicCharacterController), With<Player>>,
) {
    let Ok((mut dash, mut kcc)) = q.single_mut() else {
        return;
    };
    if !dash.is_active() {
        return;
    }
    let dt = time.delta_secs();
    dash.active_remaining_secs = (dash.active_remaining_secs - dt).max(0.0);
    let speed = tuning.range_m / tuning.duration_secs;
    let step = dash.direction_xz * speed * dt;
    // Préserve la composante verticale potentiellement déjà écrite (rarement
    // applicable car dash_motion tourne avant player_movement). Fallback safe :
    // si translation None, on initialise au step horizontal.
    let prior_y = kcc.translation.map(|t| t.y).unwrap_or(0.0);
    kcc.translation = Some(Vec3::new(step.x, prior_y, step.z));
}

/// System recharge : incrémente charges quand >= cooldown_per_charge_secs s'est
/// écoulé depuis le dernier usage ET charges < max.
pub fn dash_recharge_system(time: Res<Time>, tuning: Res<DashTuning>, mut q: Query<&mut DashState, With<Player>>) {
    let Ok(mut dash) = q.single_mut() else {
        return;
    };
    if dash.charges >= tuning.max_charges {
        dash.recharge_timer_secs = 0.0;
        return;
    }
    dash.recharge_timer_secs += time.delta_secs();
    if dash.recharge_timer_secs >= tuning.cooldown_per_charge_secs {
        dash.charges = (dash.charges + 1).min(tuning.max_charges);
        dash.recharge_timer_secs = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuning_default_matches_story_528() {
        let t = DashTuning::default();
        assert_eq!(t.range_m, 4.0);
        assert_eq!(t.duration_secs, 0.25);
        assert_eq!(t.cooldown_per_charge_secs, 1.5);
        assert_eq!(t.max_charges, 2);
    }

    #[test]
    fn dash_state_new_starts_full_charges() {
        let s = DashState::new(2);
        assert_eq!(s.charges, 2);
        assert!(!s.is_active());
    }

    #[test]
    fn dash_state_is_active_iff_remaining_positive() {
        let mut s = DashState::new(2);
        assert!(!s.is_active());
        s.active_remaining_secs = 0.1;
        assert!(s.is_active());
        s.active_remaining_secs = 0.0;
        assert!(!s.is_active());
    }

    #[test]
    fn dash_speed_matches_range_over_duration() {
        let t = DashTuning::default();
        let expected_speed = t.range_m / t.duration_secs; // 16 m/s
        assert!((expected_speed - 16.0).abs() < 0.001);
    }
}
