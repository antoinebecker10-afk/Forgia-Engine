//! # Tactical AI — Phase 2-4 story-456 (2026-05-18)
//!
//! Patterns AAA research sourcés :
//! - **LOS check** ~8 Hz (Halo 2 props poll, Damian Isla GDC 2005)
//! - **Strafing** sin + Perlin-like noise (Doom 2016 imp dodge — anti-prévisibilité)
//! - **Context steering simplifié** 3-ray forward+sides (Andrew Fray Game AI Pro 2 §18)
//! - **Reaction time grace** 350ms warmup post-LOS (humain casual 200-300ms)
//! - **Gunshot alert radius** 25m, +600ms grace si alerted-not-yet-seen
//!
//! Tous params dans `TacticalTuning` Resource (genome-driven via consumer crate).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_combat::prelude::CombatHitEvent;

use crate::{ArenaBot, BotState, BotTarget};

// ─── Tuning Resource (genome-driven) ───────────────────────────────────

#[derive(Resource, Debug, Clone, Copy)]
pub struct TacticalTuning {
    /// Fréquence raycast LOS bot→player (Hz). 8Hz = 7.5 frames @60fps. Anti CPU spam.
    pub los_check_hz: f32,
    /// Grace window post-acquisition LOS avant 1er tir (sec). AAA reaction time.
    pub los_grace_secs: f32,
    /// Amplitude latérale strafe (m). Doom imp dodge style.
    pub strafe_amplitude_m: f32,
    /// Fréquence sinusoid strafe (Hz). 0.9 = période ~1.1s.
    pub strafe_freq_hz: f32,
    /// Poids du noise additif sur strafe (0..1). 0 = pure sin (prévisible), 1 = full noise.
    pub strafe_noise_weight: f32,
    /// Distance raycast obstacle avoidance (m).
    pub local_avoid_dist_m: f32,
    /// Rayon de perception "tir player entendu" — bots dans ce rayon → alerted.
    pub gunshot_alert_radius_m: f32,
    /// Grace window supplémentaire post-alert avant 1er tir (sec). AAA "look around" feel.
    pub gunshot_alert_los_grace_secs: f32,
    /// Durée du flag alerted (forced Chase even out of detect_range).
    pub alert_duration_secs: f32,
    /// Période d'écriture sensor `forgia_bot_ai.json` (sec).
    pub sensor_period_secs: f32,
}

impl Default for TacticalTuning {
    fn default() -> Self {
        Self {
            los_check_hz: 8.0,
            los_grace_secs: 0.35,
            strafe_amplitude_m: 1.8,
            strafe_freq_hz: 0.9,
            strafe_noise_weight: 0.35,
            local_avoid_dist_m: 2.5,
            gunshot_alert_radius_m: 25.0,
            gunshot_alert_los_grace_secs: 0.6,
            alert_duration_secs: 4.0,
            sensor_period_secs: 1.0,
        }
    }
}

// ─── Sensor state ──────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct BotAiSensor {
    pub last_write_secs: f32,
    pub bots_alive: u32,
    pub bots_with_los: u32,
    pub bots_alerted: u32,
    pub bots_chasing: u32,
    pub bots_attacking: u32,
    pub los_checks_session: u32,
    pub alerts_triggered_session: u32,
}

// ─── Phase 2 — LOS check ───────────────────────────────────────────────

/// Raycast bot.shoulder → player.torso à `los_check_hz`. Set `has_los` + `los_grace_left`.
/// Filter exclut le bot lui-même via predicate (anti self-hit).
#[allow(clippy::too_many_arguments)]
pub fn bot_los_check(
    mut bots: Query<(Entity, &mut ArenaBot, &GlobalTransform, &crate::BotShootConfig)>,
    targets: Query<(Entity, &GlobalTransform), With<BotTarget>>,
    rapier: ReadRapierContext,
    tuning: Res<TacticalTuning>,
    time: Res<Time>,
    mut sensor: ResMut<BotAiSensor>,
) {
    let Some((target_entity, target_tf)) = targets.iter().next() else { return };
    let Ok(ctx) = rapier.single() else { return };
    let dt = time.delta_secs();
    let check_interval = 1.0 / tuning.los_check_hz.max(0.1);
    let target_pos = target_tf.translation();

    for (bot_entity, mut bot, bot_tf, config) in &mut bots {
        if bot.state == BotState::Dead {
            continue;
        }
        // Décrémente timer + grace.
        bot.los_check_left -= dt;
        bot.los_grace_left = (bot.los_grace_left - dt).max(0.0);
        bot.alert_left = (bot.alert_left - dt).max(0.0);
        if bot.alert_left <= 0.0 {
            bot.alerted = false;
        }
        if bot.los_check_left > 0.0 {
            continue;
        }
        bot.los_check_left = check_interval;
        sensor.los_checks_session = sensor.los_checks_session.saturating_add(1);

        let origin = bot_tf.translation() + Vec3::Y * config.shoulder_y;
        let aim_at = Vec3::new(
            target_pos.x,
            target_pos.y + config.target_torso_y,
            target_pos.z,
        );
        let to_target = aim_at - origin;
        let dist = to_target.length();
        if dist < 0.5 || dist > config.range {
            bot.has_los = false;
            continue;
        }
        let dir = to_target / dist;
        // Exclure le bot pour anti self-hit ; tout autre hit = obstacle = pas de LOS.
        let predicate = |e: Entity| e != bot_entity;
        let filter = QueryFilter::default().predicate(&predicate);
        let hit = ctx.cast_ray(origin, dir, dist, true, filter);
        let new_los = match hit {
            None => true,
            Some((hit_entity, _)) => hit_entity == target_entity,
        };
        // Transition false → true : démarrer grace window (reaction time AAA).
        if !bot.has_los && new_los {
            let grace = if bot.alerted {
                tuning.los_grace_secs.min(tuning.gunshot_alert_los_grace_secs)
            } else {
                tuning.los_grace_secs
            };
            bot.los_grace_left = grace;
        }
        bot.has_los = new_los;
    }
}

// ─── Phase 4 — Perception alert ────────────────────────────────────────

/// Consume `CombatHitEvent` filter `attacker == player` (= target reçoit damage de player).
/// Tout bot dans `gunshot_alert_radius_m` du player passe alerted=true + alert_left=duration.
/// Bot alerted force le state Chase même hors detect_range (audio-driven AI).
pub fn bot_perception_alert(
    mut events: MessageReader<CombatHitEvent>,
    mut bots: Query<(&mut ArenaBot, &GlobalTransform)>,
    q_target: Query<&GlobalTransform, With<BotTarget>>,
    tuning: Res<TacticalTuning>,
    mut sensor: ResMut<BotAiSensor>,
) {
    let Ok(target_tf) = q_target.single() else {
        // Drain events sinon ils s'accumulent.
        for _ in events.read() {}
        return;
    };
    let player_pos = target_tf.translation();
    for hit in events.read() {
        // Source du bruit = position du player tirant (proxy : si attacker = player,
        // le tir part du player). Pas le hit_world_pos (ça c'est la cible).
        let _ = hit; // dummy : ici on déclenche alert sur n'importe quel tir player.
        // Filter : on alerte sur tous les CombatHitEvent (proxy "player a tiré").
        // Pourrait être affiné via une dedicated WeaponFiredEvent — out of scope phase 4.
        for (mut bot, bot_tf) in &mut bots {
            if bot.state == BotState::Dead {
                continue;
            }
            let d = bot_tf.translation().distance(player_pos);
            if d <= tuning.gunshot_alert_radius_m && !bot.alerted {
                bot.alerted = true;
                bot.alert_left = tuning.alert_duration_secs;
                sensor.alerts_triggered_session =
                    sensor.alerts_triggered_session.saturating_add(1);
            }
        }
    }
}

// ─── Phase 3 — Strafing + obstacle avoidance ──────────────────────────

/// Calcule le vecteur strafe lateral à appliquer en plus du chase forward.
/// sin(phase) bias droite/gauche, modulé par noise xorshift (anti-prévisibilité).
/// Output : direction unit dans plan XZ (perpendiculaire à to_target).
fn compute_strafe_offset(
    bot: &mut ArenaBot,
    to_target_dir: Vec3,
    tuning: &TacticalTuning,
    dt: f32,
) -> Vec3 {
    bot.strafe_phase_rad += dt * tuning.strafe_freq_hz * std::f32::consts::TAU;
    let sin = bot.strafe_phase_rad.sin();
    // xorshift32 → noise [-0.5, 0.5]
    let mut x = bot.strafe_noise_seed.max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    bot.strafe_noise_seed = x;
    let noise = (x as f32 / u32::MAX as f32) - 0.5;
    let bias = sin * (1.0 - tuning.strafe_noise_weight) + noise * 2.0 * tuning.strafe_noise_weight;
    // Right vector perpendiculaire à to_target dans plan XZ.
    let right = Vec3::new(-to_target_dir.z, 0.0, to_target_dir.x).normalize_or_zero();
    right * bias * tuning.strafe_amplitude_m
}

/// Obstacle avoidance context steering simplifié 3-ray (forward, ±45°).
/// Return : direction unit XZ ajustée pour éviter les obstacles. None si tous bloqués.
fn pick_avoid_direction(
    origin: Vec3,
    desired_dir: Vec3,
    rapier: &RapierContext,
    self_entity: Entity,
    max_dist: f32,
) -> Option<Vec3> {
    let predicate = |e: Entity| e != self_entity;
    let filter = QueryFilter::default().predicate(&predicate);
    let cast = |d: Vec3| -> f32 {
        rapier
            .cast_ray(origin + Vec3::Y * 0.5, d.normalize_or_zero(), max_dist, true, filter)
            .map(|(_, t)| t)
            .unwrap_or(max_dist)
    };
    let right = Vec3::new(-desired_dir.z, 0.0, desired_dir.x).normalize_or_zero();
    let dir_fwd = desired_dir;
    let dir_left = (desired_dir - right * 0.7).normalize_or_zero();
    let dir_right = (desired_dir + right * 0.7).normalize_or_zero();
    let fwd = cast(dir_fwd);
    let left = cast(dir_left);
    let right_d = cast(dir_right);
    // Pick le plus dégagé (max distance).
    let best = [(fwd, dir_fwd), (left, dir_left), (right_d, dir_right)]
        .into_iter()
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))?;
    if best.0 < 0.6 {
        None // tous bloqués (< 0.6m = collision imminente)
    } else {
        Some(best.1)
    }
}

/// Override state machine V1 — Chase intelligent : forward + strafe + obstacle avoidance.
/// Run APRÈS `bot_state_machine` original pour appliquer le tactical layer.
#[allow(clippy::too_many_arguments)]
pub fn bot_tactical_movement(
    mut bots: Query<(Entity, &mut ArenaBot, &mut Transform), Without<BotTarget>>,
    targets: Query<&Transform, With<BotTarget>>,
    rapier: ReadRapierContext,
    tuning: Res<TacticalTuning>,
    time: Res<Time>,
) {
    let Some(target_tf) = targets.iter().next() else { return };
    let target_pos = target_tf.translation;
    let Ok(ctx) = rapier.single() else { return };
    let dt = time.delta_secs();

    for (bot_entity, mut bot, mut xf) in &mut bots {
        if bot.state == BotState::Dead {
            continue;
        }
        // Alerted → force Chase si hors detect_range mais dans alert (audio AI).
        let to_target = target_pos - xf.translation;
        let dist = to_target.length();
        let want_chase = matches!(bot.state, BotState::Chase)
            || (bot.alerted && dist > bot.stop_distance);
        if !want_chase || bot.speed < 0.01 || dist < 0.01 {
            continue;
        }
        let fwd_dir = (to_target / dist).with_y(0.0).normalize_or_zero();
        let strafe = compute_strafe_offset(&mut bot, fwd_dir, &tuning, dt);
        let desired = (fwd_dir + strafe.normalize_or_zero() * 0.4).normalize_or_zero();
        // Phase 3 obstacle avoidance.
        let final_dir = pick_avoid_direction(
            xf.translation,
            desired,
            &ctx,
            bot_entity,
            tuning.local_avoid_dist_m,
        )
        .unwrap_or(fwd_dir); // fallback forward simple si tous bloqués
        let step = final_dir * bot.speed * dt;
        xf.translation.x += step.x;
        xf.translation.z += step.z;
        // Y stays at spawn (pas de jump V2).
    }
}

// ─── Sensor `forgia_bot_ai.json` ───────────────────────────────────────

pub fn write_bot_ai_sensor(
    time: Res<Time>,
    tuning: Res<TacticalTuning>,
    bots: Query<&ArenaBot>,
    mut sensor: ResMut<BotAiSensor>,
) {
    let now = time.elapsed_secs();
    if now - sensor.last_write_secs < tuning.sensor_period_secs.max(0.1) {
        return;
    }
    sensor.last_write_secs = now;
    let mut alive = 0u32;
    let mut with_los = 0u32;
    let mut alerted = 0u32;
    let mut chasing = 0u32;
    let mut attacking = 0u32;
    for bot in &bots {
        if bot.state == BotState::Dead {
            continue;
        }
        alive += 1;
        if bot.has_los {
            with_los += 1;
        }
        if bot.alerted {
            alerted += 1;
        }
        match bot.state {
            BotState::Chase => chasing += 1,
            BotState::Attack => attacking += 1,
            _ => {}
        }
    }
    sensor.bots_alive = alive;
    sensor.bots_with_los = with_los;
    sensor.bots_alerted = alerted;
    sensor.bots_chasing = chasing;
    sensor.bots_attacking = attacking;
    let json = format!(
        r#"{{"timestamp_secs":{:.2},"bots_alive":{},"bots_with_los":{},"bots_alerted":{},"bots_chasing":{},"bots_attacking":{},"los_checks_session":{},"alerts_triggered_session":{},"tuning":{{"los_hz":{:.1},"strafe_amp_m":{:.2},"alert_radius_m":{:.1}}}}}"#,
        now, alive, with_los, alerted, chasing, attacking,
        sensor.los_checks_session, sensor.alerts_triggered_session,
        tuning.los_check_hz, tuning.strafe_amplitude_m, tuning.gunshot_alert_radius_m,
    );
    let _ = std::fs::write("forgia_bot_ai.json", json);
}
