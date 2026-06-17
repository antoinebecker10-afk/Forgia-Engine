//! player_state_sensor.rs — Producteur `forgia2_player_state.json` (1Hz, cross-mode).
//!
//! Track player movement (position xyz + velocity m/s + grounded) ET collision
//! state (KCC contact count + max effective_translation vs requested).
//! "stuck" = velocity_planar < 0.05 m/s pendant > 30 frames consécutives
//! **ET intention de mouvement** (le KCC a demandé un déplacement planaire via
//! `KinematicCharacterController.translation` mais le joueur n'a pas bougé).
//! Sans intention (idle, dialogue, viser sans bouger) → PAS stuck. Corrige le
//! faux positif 2026-06-17 (idle flaggé "critical" à tort). Reste cross-crate
//! light (Rapier déjà importé, aucune dep input).
//!
//! Bloc canonique pour distinguer :
//! - freeze visuel (lag frame, player en mouvement mais render hang)
//! - stuck collision (KCC translate 0 frame après frame, grounded oscille)
//! - immobile attendu (player ne push pas d'input → not stuck)
//!
//! Story-526 — Diagnostic freeze RPG+Roguelite (2026-05-26).

use bevy::prelude::*;
use bevy_rapier3d::prelude::{KinematicCharacterController, KinematicCharacterControllerOutput};
use forgia_player::Player;

const STUCK_VEL_EPSILON: f32 = 0.05; // m/s
const STUCK_FRAMES_THRESHOLD: u32 = 30; // ~0.5s @ 60fps
/// Déplacement planaire demandé/frame (m) au-dessus duquel on considère qu'il y
/// a INTENTION de bouger. En-dessous = idle (pas d'input planaire) → jamais stuck.
const MOVE_INTENT_EPSILON: f32 = 0.001;

#[derive(Resource, Default)]
pub struct PlayerStateAccum {
    pub last_pos: Vec3,
    pub last_pos_recorded: bool,
    pub stuck_frames_consecutive: u32,
    pub stuck_events_session: u32,
    pub last_velocity_planar_m_s: f32,
    pub last_velocity_y_m_s: f32,
    pub last_grounded: bool,
    pub last_kcc_collisions: usize,
    pub last_move_intent: f32,
    pub frames_observed: u64,
}

/// Tick frame — calcule velocity instantanée + accumule stuck frames.
pub fn sys_track_player_state(
    time: Res<Time>,
    mut accum: ResMut<PlayerStateAccum>,
    q_player: Query<
        (
            &Transform,
            Option<&KinematicCharacterControllerOutput>,
            Option<&KinematicCharacterController>,
        ),
        With<Player>,
    >,
) {
    let dt = time.delta_secs().max(1e-6);
    let Ok((tf, kcc_out, kcc)) = q_player.single() else {
        // Pas de player (Menu/Pause/AppNotInGame) — reset stuck counter.
        accum.stuck_frames_consecutive = 0;
        accum.last_pos_recorded = false;
        return;
    };

    let pos = tf.translation;

    let (vel_planar, vel_y) = if accum.last_pos_recorded {
        let delta = pos - accum.last_pos;
        let planar = (delta.x * delta.x + delta.z * delta.z).sqrt() / dt;
        let y = delta.y / dt;
        (planar, y)
    } else {
        (0.0, 0.0)
    };

    accum.last_pos = pos;
    accum.last_pos_recorded = true;
    accum.last_velocity_planar_m_s = vel_planar;
    accum.last_velocity_y_m_s = vel_y;
    accum.frames_observed = accum.frames_observed.saturating_add(1);

    if let Some(out) = kcc_out {
        accum.last_grounded = out.grounded;
        accum.last_kcc_collisions = out.collisions.len();
    }

    // Intention de mouvement = déplacement planaire demandé par le KCC ce frame.
    let move_intent = kcc
        .and_then(|c| c.translation)
        .map(|t| (t.x * t.x + t.z * t.z).sqrt())
        .unwrap_or(0.0);
    accum.last_move_intent = move_intent;

    // Stuck UNIQUEMENT si le joueur VOULAIT bouger (intent) mais n'a pas bougé.
    // Idle / dialogue / viser sans bouger (intent ~0) → reset (jamais stuck).
    if vel_planar < STUCK_VEL_EPSILON && move_intent > MOVE_INTENT_EPSILON {
        accum.stuck_frames_consecutive = accum.stuck_frames_consecutive.saturating_add(1);
        if accum.stuck_frames_consecutive == STUCK_FRAMES_THRESHOLD {
            accum.stuck_events_session = accum.stuck_events_session.saturating_add(1);
        }
    } else {
        accum.stuck_frames_consecutive = 0;
    }
}

pub fn severity_for_player_state(stuck_consecutive: u32, kcc_collisions: usize) -> (&'static str, &'static str) {
    if stuck_consecutive > 300 {
        (
            "critical",
            "stuck_frames_consecutive > 300 (~5s) — player physiquement bloqué (check KCC penetration + collider surrounding)",
        )
    } else if stuck_consecutive > 60 {
        (
            "warn",
            "stuck_frames_consecutive > 60 (~1s) — possible stuck (intentional immobile OR collision deadlock)",
        )
    } else if kcc_collisions > 8 {
        (
            "warn",
            "kcc_collisions > 8 contacts — player entouré, risque push-stuck (raise collider gap or filter)",
        )
    } else {
        ("ok", "")
    }
}

pub fn sys_write_player_state_sensor(
    time: Res<Time>,
    accum: Res<PlayerStateAccum>,
    mut tick_accum: Local<f32>,
) {
    *tick_accum += time.delta_secs();
    if *tick_accum < 1.0 {
        return;
    }
    *tick_accum = 0.0;

    let (severity, next_step) = severity_for_player_state(
        accum.stuck_frames_consecutive,
        accum.last_kcc_collisions,
    );

    let json = format!(
        r#"{{"id":"player_state","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"position":[{:.3},{:.3},{:.3}],"velocity_planar_m_s":{:.3},"velocity_y_m_s":{:.3},"grounded":{},"kcc_collisions":{},"move_intent_planar":{:.4},"stuck_frames_consecutive":{},"stuck_events_session":{},"frames_observed":{}}}"#,
        time.elapsed_secs(),
        accum.last_pos.x,
        accum.last_pos.y,
        accum.last_pos.z,
        accum.last_velocity_planar_m_s,
        accum.last_velocity_y_m_s,
        accum.last_grounded,
        accum.last_kcc_collisions,
        accum.last_move_intent,
        accum.stuck_frames_consecutive,
        accum.stuck_events_session,
        accum.frames_observed,
    );

    if let Err(e) = std::fs::write("forgia2_player_state.json", &json) {
        warn!("[forgia-observability] player_state sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_under_thresholds() {
        assert_eq!(severity_for_player_state(10, 2).0, "ok");
    }

    #[test]
    fn severity_warn_stuck_60_frames() {
        let (sev, next) = severity_for_player_state(120, 0);
        assert_eq!(sev, "warn");
        assert!(next.contains("stuck"));
    }

    #[test]
    fn severity_critical_stuck_300_frames() {
        let (sev, next) = severity_for_player_state(500, 0);
        assert_eq!(sev, "critical");
        assert!(next.contains("bloqué"));
    }

    #[test]
    fn severity_warn_many_kcc_contacts() {
        let (sev, _) = severity_for_player_state(0, 12);
        assert_eq!(sev, "warn");
    }

    #[test]
    fn stuck_epsilon_is_5cm_per_s() {
        assert!((STUCK_VEL_EPSILON - 0.05).abs() < 1e-6);
    }
}
