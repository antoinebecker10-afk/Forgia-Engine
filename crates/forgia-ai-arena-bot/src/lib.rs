//! forgia-ai-arena-bot — Simple FPS Arena bot.
//!
//! State machine : Idle -> Chase (player in detect range) -> Attack (in attack range).
//! Listens for DeathEvent from forgia-damage to despawn + schedule respawn.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_damage::{DeathEvent, Health, Mortal};

#[derive(Component, Debug, Clone, Copy)]
pub struct ArenaBot {
    pub state: BotState,
    pub speed: f32,
    pub detect_range: f32,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    pub attack_left: f32,
}

impl Default for ArenaBot {
    fn default() -> Self {
        Self {
            state: BotState::Idle,
            speed: 4.0,
            detect_range: 25.0,
            attack_range: 1.8,
            attack_cooldown: 1.0,
            attack_left: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotState {
    Idle,
    Chase,
    Attack,
    Dead,
}

/// Marker — entity is targeted by bots (typically the player).
#[derive(Component)]
pub struct BotTarget;

#[derive(Component, Debug, Clone, Copy)]
pub struct BotSpawnPoint {
    pub position: Vec3,
    pub respawn_delay: f32,
}

#[derive(Resource, Default)]
pub struct PendingRespawns {
    pub queue: Vec<(f32, Vec3)>,
}

pub struct ForgiaAiArenaBotPlugin;

impl Plugin for ForgiaAiArenaBotPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingRespawns>()
            .add_systems(
                Update,
                (
                    bot_state_machine,
                    bot_attack_cooldown,
                    handle_bot_deaths,
                    tick_respawns,
                ),
            );
    }
}

fn bot_state_machine(
    mut bots: Query<(&mut ArenaBot, &mut Transform), Without<BotTarget>>,
    targets: Query<&Transform, With<BotTarget>>,
    time: Res<Time>,
) {
    let Some(target) = targets.iter().next() else { return };
    let target_pos = target.translation;
    let dt = time.delta_secs();

    for (mut bot, mut xf) in &mut bots {
        if bot.state == BotState::Dead { continue; }
        let to_target = target_pos - xf.translation;
        let dist = to_target.length();

        bot.state = if dist <= bot.attack_range {
            BotState::Attack
        } else if dist <= bot.detect_range {
            BotState::Chase
        } else {
            BotState::Idle
        };

        if matches!(bot.state, BotState::Chase) && dist > 0.01 {
            let dir = to_target / dist;
            let step = dir * bot.speed * dt;
            xf.translation += Vec3::new(step.x, 0.0, step.z);
            let yaw = (-dir.x).atan2(-dir.z);
            xf.rotation = Quat::from_rotation_y(yaw);
        }
    }
}

fn bot_attack_cooldown(time: Res<Time>, mut bots: Query<&mut ArenaBot>) {
    let dt = time.delta_secs();
    for mut bot in &mut bots {
        bot.attack_left = (bot.attack_left - dt).max(0.0);
    }
}

fn handle_bot_deaths(
    mut deaths: MessageReader<DeathEvent>,
    mut commands: Commands,
    bots: Query<(&Transform, Option<&BotSpawnPoint>), With<ArenaBot>>,
    mut pending: ResMut<PendingRespawns>,
) {
    for ev in deaths.read() {
        let Ok((xf, spawn)) = bots.get(ev.target) else { continue };
        let pos = spawn.map(|s| s.position).unwrap_or(xf.translation);
        let delay = spawn.map(|s| s.respawn_delay).unwrap_or(3.0);
        pending.queue.push((delay, pos));
        commands.entity(ev.target).despawn();
    }
}

fn tick_respawns(
    time: Res<Time>,
    mut pending: ResMut<PendingRespawns>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    let mut ready = Vec::new();
    pending.queue.retain_mut(|(t, pos)| {
        *t -= dt;
        if *t <= 0.0 {
            ready.push(*pos);
            false
        } else {
            true
        }
    });
    for pos in ready {
        commands.spawn((
            ArenaBot::default(),
            Mortal,
            Health::new(60.0),
            Transform::from_translation(pos),
            GlobalTransform::default(),
            BotSpawnPoint { position: pos, respawn_delay: 3.0 },
            RigidBody::KinematicPositionBased,
            Collider::capsule_y(0.6, 0.4),
        ));
    }
}
