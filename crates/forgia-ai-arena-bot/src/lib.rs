//! forgia-ai-arena-bot — Simple FPS Arena bot.
//!
//! State machine : Idle -> Chase (player in detect range) -> Attack (in attack range).
//! Shoots periodically at the BotTarget via rapier raycast + DamageEvent.
//! Listens for DeathEvent from forgia-damage to despawn + schedule respawn.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_damage::{DamageEvent, DamageKind, DeathEvent, ForgiaDamagePlugin, Health, Mortal};

pub mod tactical;
pub use tactical::{BotAiSensor, TacticalTuning};

#[derive(Component, Debug, Clone, Copy)]
pub struct ArenaBot {
    pub state: BotState,
    pub speed: f32,
    pub detect_range: f32,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    /// Cooldown restant avant prochain tir. Init = warmup (anti spawn-kill).
    pub attack_left: f32,
    /// Story-456 Phase 1 — distance d'arrêt en Chase (m). Bot s'arrête à `stop_distance`
    /// pour tirer au lieu de coller le player. Doit être < attack_range pour que le bot
    /// approche puis stagne en zone de tir. AAA standard 4-8m.
    pub stop_distance: f32,
    // ── Story-456 Phase 2 — LOS check ─────────────────────────────
    /// Ligne de vue claire vers player ? Recheck via `bot_los_check` à `los_check_hz`.
    pub has_los: bool,
    /// Timer décrémenté ; recheck LOS quand <= 0.
    pub los_check_left: f32,
    /// Grace window après acquisition LOS avant 1er tir (AAA reaction time ~350ms).
    pub los_grace_left: f32,
    // ── Story-456 Phase 3 — Strafing ──────────────────────────────
    /// Phase oscillation strafe sin (offset par-bot pour désync entre bots).
    pub strafe_phase_rad: f32,
    /// Random noise seed accumulé (xorshift32) pour bias strafe.
    pub strafe_noise_seed: u32,
    // ── Story-456 Phase 4 — Perception alert ──────────────────────
    /// True si player a tiré récemment dans rayon perception.
    pub alerted: bool,
    /// Timer décompte alerted (transition Idle→Chase forced pendant cette durée).
    pub alert_left: f32,
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
            stop_distance: 1.5,
            has_los: false,
            los_check_left: 0.0,
            los_grace_left: 0.0,
            strafe_phase_rad: 0.0,
            strafe_noise_seed: 0xDEADBEEF,
            alerted: false,
            alert_left: 0.0,
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

/// Configuration tir genome-driven. Inseré par le caller (e.g. forgia-mode-fps-arena)
/// au spawn. Permet aux différents modes d'avoir des bots avec damage / spread différents.
#[derive(Component, Debug, Clone, Copy)]
pub struct BotShootConfig {
    pub damage: f32,
    pub range: f32,
    /// Spread random ±jitter_deg appliqué sur la direction du tir (radians stockés).
    pub jitter_rad: f32,
    /// Couleur HDR du tracer (R, G, B linéaire — emissive multiplier).
    pub tracer_emissive: LinearRgba,
    /// Hauteur Y locale "shoulder" du bot (origine du raycast).
    pub shoulder_y: f32,
    /// Hauteur Y locale "torso" du player (cible du raycast). Default 1.0m.
    pub target_torso_y: f32,
}

impl Default for BotShootConfig {
    fn default() -> Self {
        Self {
            damage: 12.0,
            range: 35.0,
            jitter_rad: 4.0_f32.to_radians(),
            tracer_emissive: LinearRgba::new(4.0, 1.5, 0.5, 1.0),
            shoulder_y: 1.4,
            target_torso_y: 1.0,
        }
    }
}

/// Marker — entity is targeted by bots (typically the player).
#[derive(Component)]
pub struct BotTarget;

/// Marker — tracer transient ; despawn après son lifetime.
#[derive(Component)]
pub struct BotTracer {
    pub life_remaining: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct BotSpawnPoint {
    pub position: Vec3,
    pub respawn_delay: f32,
}

#[derive(Resource, Default)]
pub struct PendingRespawns {
    pub queue: Vec<(f32, Vec3)>,
}

/// Resource simple seed PRNG xorshift32 — anti-perfect-aim sans dep `rand`.
#[derive(Resource)]
pub struct BotShootRng(pub u32);

impl Default for BotShootRng {
    fn default() -> Self {
        Self(0xCAFEBABE)
    }
}

impl BotShootRng {
    /// Next u32 xorshift32. Mutate self.
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Float in [-0.5, 0.5].
    fn next_signed_unit(&mut self) -> f32 {
        let u = self.next_u32() as f32 / u32::MAX as f32;
        u - 0.5
    }
}

pub struct ForgiaAiArenaBotPlugin;

impl Plugin for ForgiaAiArenaBotPlugin {
    fn build(&self, app: &mut App) {
        // ForgiaDamagePlugin idempotent — emit DamageEvent → mutate Health → DeathEvent.
        if !app.is_plugin_added::<ForgiaDamagePlugin>() {
            app.add_plugins(ForgiaDamagePlugin);
        }
        app.init_resource::<PendingRespawns>()
            .init_resource::<BotShootRng>()
            .init_resource::<TacticalTuning>()
            .init_resource::<BotAiSensor>()
            .add_systems(
                Update,
                (
                    // Phase 2 LOS check + Phase 4 perception alert run AVANT
                    // state_machine pour que la state transition voie has_los/alerted à jour.
                    tactical::bot_los_check,
                    tactical::bot_perception_alert,
                    bot_state_machine,
                    // Phase 3 tactical_movement run APRÈS state_machine pour override le
                    // mouvement basique chase forward avec strafe + obstacle avoidance.
                    tactical::bot_tactical_movement,
                    bot_attack_cooldown,
                    bot_shoot_at_target,
                    bot_tracer_lifetime,
                    handle_bot_deaths,
                    tick_respawns,
                    tactical::write_bot_ai_sensor,
                )
                    .chain(),
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
        if bot.state == BotState::Dead {
            continue;
        }
        let to_target = target_pos - xf.translation;
        let dist = to_target.length();

        // Story-456 Phase 1 — state machine 3-tier :
        // - dist <= stop_distance : Attack (à portée tir, ne bouge plus)
        // - stop_distance < dist <= detect_range : Chase (s'approche)
        // - dist > detect_range : Idle (hors perception)
        // attack_range = shot_range >> stop_distance pour que le bot tire pendant Chase aussi.
        bot.state = if dist <= bot.stop_distance {
            BotState::Attack
        } else if dist <= bot.detect_range {
            BotState::Chase
        } else {
            BotState::Idle
        };

        // Story-456 Phase 3 — mouvement Chase délégué à `tactical::bot_tactical_movement`
        // (strafe + obstacle avoidance). Ce système ne gère plus que la state transition
        // + l'orientation visuelle ci-dessous. Le `let _ = dt` évite l'unused warning.
        let _ = dt;

        // Toujours faire face au target (même statique) pour orientation tracer + visuel.
        if dist > 0.01 {
            let dir = to_target / dist;
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

/// Système : pour chaque bot in Attack/Chase avec cooldown ready, raycast vers
/// le player → si hit player → DamageEvent + spawn tracer visuel. Reset cooldown.
#[allow(clippy::too_many_arguments)]
fn bot_shoot_at_target(
    mut bots: Query<(Entity, &mut ArenaBot, &GlobalTransform, &BotShootConfig)>,
    targets: Query<(Entity, &GlobalTransform), With<BotTarget>>,
    rapier: ReadRapierContext,
    mut rng: ResMut<BotShootRng>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut damage_events: MessageWriter<DamageEvent>,
) {
    let Some((target_entity, target_tf)) = targets.iter().next() else {
        return;
    };
    let Ok(ctx) = rapier.single() else {
        return;
    };

    let target_pos = target_tf.translation();

    for (bot_entity, mut bot, bot_tf, config) in &mut bots {
        if bot.state == BotState::Dead || bot.attack_left > 0.0 {
            continue;
        }
        if !matches!(bot.state, BotState::Chase | BotState::Attack) {
            continue;
        }
        // Story-456 Phase 2 — gate tir sur LOS + grace window (AAA reaction time).
        // Sans LOS = pas de tir (player derrière mur). Pendant grace = vu mais "réfléchit".
        if !bot.has_los || bot.los_grace_left > 0.0 {
            continue;
        }

        // Origine raycast = position bot + shoulder_y
        let origin = bot_tf.translation() + Vec3::Y * config.shoulder_y;
        // Cible visée = torse player
        let aim_at = Vec3::new(
            target_pos.x,
            target_pos.y + config.target_torso_y,
            target_pos.z,
        );
        let to_target = aim_at - origin;
        let dist = to_target.length();
        if dist < 0.5 || dist > config.range {
            // Trop proche (overlap) ou hors range → skip shoot (mais cooldown pas reset)
            continue;
        }
        let base_dir = to_target / dist;

        // Jitter : random tilt sur 2 axes perpendiculaires.
        let right = base_dir.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(base_dir).normalize_or_zero();
        let jx = rng.next_signed_unit() * config.jitter_rad;
        let jy = rng.next_signed_unit() * config.jitter_rad;
        let shot_dir = (base_dir + right * jx + up * jy).normalize_or_zero();
        if shot_dir.length_squared() < 0.001 {
            continue;
        }

        // Exclure soi-même (le bot) du raycast pour éviter self-hit.
        let predicate = |e: Entity| e != bot_entity;
        let filter = QueryFilter::default().predicate(&predicate);
        let hit = ctx.cast_ray(origin, shot_dir, config.range, true, filter);

        let hit_dist = if let Some((hit_entity, toi)) = hit {
            // On considère un hit valide même si on touche un enfant du player
            // (capsule collider directement sur player, OK)
            if hit_entity == target_entity {
                damage_events.write(DamageEvent {
                    target: target_entity,
                    source: Some(bot_entity),
                    amount: config.damage,
                    kind: DamageKind::Physical,
                });
            }
            toi
        } else {
            config.range
        };

        // Tracer visuel : cuboid stretché entre origin et hit point.
        spawn_tracer(
            &mut commands,
            &mut meshes,
            &mut materials,
            origin,
            shot_dir,
            hit_dist,
            config.tracer_emissive,
        );

        bot.attack_left = bot.attack_cooldown;
    }
}

fn spawn_tracer(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    origin: Vec3,
    dir: Vec3,
    length: f32,
    emissive: LinearRgba,
) {
    // Story-452 (2026-05-18 PM) — fix screenshot user : bot tracers étaient des laser
    // beams de 30m × 4cm bouchant l'écran. Solution : segment court (max 4m) au lieu
    // de full hit_dist, mesh fin (1cm), HDR /3, lifetime court.
    let core_width = 0.01;
    let mesh = meshes.add(Cuboid::new(core_width, core_width, 1.0));
    let dimmed = LinearRgba::new(
        emissive.red * 0.35,
        emissive.green * 0.35,
        emissive.blue * 0.35,
        emissive.alpha,
    );
    let mat = materials.add(StandardMaterial {
        emissive: dimmed,
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    });
    // Tracer segment court (4m max) partant du canon, pas tout le ray.
    let seg_len = length.min(4.0);
    let mid = origin + dir * (seg_len * 0.5);
    let tf = Transform::from_translation(mid)
        .looking_to(dir, Vec3::Y)
        .with_scale(Vec3::new(1.0, 1.0, seg_len));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        tf,
        BotTracer {
            life_remaining: 0.08, // 80ms (était 120ms)
        },
    ));
}

fn bot_tracer_lifetime(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut BotTracer)>,
) {
    let dt = time.delta_secs();
    for (e, mut tracer) in &mut q {
        tracer.life_remaining -= dt;
        if tracer.life_remaining <= 0.0 {
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.try_despawn();
            }
        }
    }
}

fn handle_bot_deaths(
    mut deaths: MessageReader<DeathEvent>,
    mut commands: Commands,
    bots: Query<(&Transform, Option<&BotSpawnPoint>), With<ArenaBot>>,
    mut pending: ResMut<PendingRespawns>,
) {
    for ev in deaths.read() {
        let Ok((xf, spawn)) = bots.get(ev.target) else {
            continue;
        };
        let pos = spawn.map(|s| s.position).unwrap_or(xf.translation);
        let delay = spawn.map(|s| s.respawn_delay).unwrap_or(3.0);
        pending.queue.push((delay, pos));
        if let Ok(mut ec) = commands.get_entity(ev.target) {
            ec.try_despawn();
        }
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
            BotSpawnPoint {
                position: pos,
                respawn_delay: 3.0,
            },
            RigidBody::KinematicPositionBased,
            Collider::capsule_y(0.6, 0.4),
        ));
    }
}

pub mod prelude {
    pub use crate::{
        ArenaBot, BotShootConfig, BotShootRng, BotSpawnPoint, BotState, BotTarget, BotTracer,
        ForgiaAiArenaBotPlugin, PendingRespawns,
    };
}
