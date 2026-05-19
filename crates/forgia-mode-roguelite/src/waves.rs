//! waves.rs — M2 step 4 : système 3-wave end-to-end Roguelite.
//!
//! Pipeline :
//! 1. OnEnter(GameMode::Roguelite) : `RogueliteWave::default()` Resource inséré
//! 2. wave 1 spawn auto (caller : `sys_spawn_roguelite_scene` appelle `spawn_wave_enemies(1)`)
//! 3. `sys_wave_orchestrator` poll Query<&ArenaBot> chaque frame :
//!    - si bots_alive == 0 ET wave en cours : démarre break (3s)
//!    - quand break_secs_left <= 0 : spawn next wave (current_wave +=1)
//!    - quand current_wave > WAVES_TOTAL : emit `EndRunEvent(Victory)`
//!
//! Scaling : wave 1 = 8 ennemis (3T/3R/2S), wave 2 = 12 (4T/4R/4S), wave 3 = 16 (6T/6R/4S).

use crate::enemies::{self, EnemyArchetype};
use crate::run::{EndRunEvent, RogueliteRunMarker, RunResult};
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_rapier3d::prelude::{Collider, RigidBody};
use forgia_ai_arena_bot::ArenaBot;
use forgia_core::prelude::*;
use forgia_damage::{Health, Mortal};
use forgia_mode_fps_arena::TargetCube;
use rand_xoshiro::Xoshiro256StarStar;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};

pub const WAVES_TOTAL: u8 = 3;
pub const BREAK_SECS: f32 = 3.0;
const WAVE_BASE_SEED: u64 = 0xC0FF_EE51_C0BA_1700;

#[derive(Resource, Debug, Clone)]
pub struct RogueliteWave {
    pub current_wave: u8,
    pub bots_alive: u32,
    pub break_secs_left: f32,
    /// True quand on est en train d'attendre le prochain spawn (entre 2 vagues).
    pub in_break: bool,
    pub victory_emitted: bool,
}

impl Default for RogueliteWave {
    fn default() -> Self {
        Self {
            current_wave: 1,
            bots_alive: 0,
            break_secs_left: 0.0,
            in_break: false,
            victory_emitted: false,
        }
    }
}

/// Composition par vague : `(archetype, count, ring_radius)`.
pub fn wave_composition(wave: u8) -> Vec<(EnemyArchetype, u32, f32)> {
    match wave {
        1 => vec![
            (EnemyArchetype::Tank, 3, 12.0),
            (EnemyArchetype::Runner, 3, 25.0),
            (EnemyArchetype::Sniper, 2, 50.0),
        ],
        2 => vec![
            (EnemyArchetype::Tank, 4, 14.0),
            (EnemyArchetype::Runner, 4, 28.0),
            (EnemyArchetype::Sniper, 4, 55.0),
        ],
        _ => vec![
            // Wave 3 (final boss — M3 step 1) : 1 boss + 4 ennemis support pour
            // pression de zone (cohérent climax RoR2 / Hadès).
            (EnemyArchetype::Boss, 1, 12.0),
            (EnemyArchetype::Runner, 4, 28.0),
        ],
    }
}

/// Spawn N ennemis de la composition wave donnée. Caller : OnEnter scene ou
/// orchestrator au passage de vague.
pub fn spawn_wave_enemies(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    wave: u8,
) -> u32 {
    let composition = wave_composition(wave);
    let mut yaw_rng = Xoshiro256StarStar::seed_from_u64(WAVE_BASE_SEED ^ u64::from(wave));
    let mut total = 0u32;
    for (archetype, count, ring_radius) in &composition {
        let stats = enemies::stats_for(*archetype);
        let mesh = meshes.add(Capsule3d::new(
            stats.capsule_radius,
            stats.capsule_half_height * 2.0,
        ));
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgb(stats.color_rgb[0], stats.color_rgb[1], stats.color_rgb[2]),
            emissive: LinearRgba::new(
                stats.emissive_rgb[0],
                stats.emissive_rgb[1],
                stats.emissive_rgb[2],
                1.0,
            ),
            perceptual_roughness: 0.55,
            ..default()
        });
        let yaw0 =
            (yaw_rng.next_u64() as f64 / u64::MAX as f64) as f32 * std::f32::consts::TAU;
        for i in 0..*count {
            let theta = yaw0 + (i as f32 / *count as f32) * std::f32::consts::TAU;
            let x = ring_radius * theta.cos();
            let z = ring_radius * theta.sin();
            let y = stats.capsule_half_height + stats.capsule_radius + 0.05;
            commands.spawn((
                Name::new(format!("RogueliteEnemy_W{wave}_{}_{i}", archetype.label())),
                RogueliteRunMarker,
                DespawnOnExit(GameMode::Roguelite),
                *archetype,
                TargetCube,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_xyz(x, y, z),
                RigidBody::KinematicPositionBased,
                Collider::capsule_y(stats.capsule_half_height, stats.capsule_radius),
                Health::new(stats.hp),
                Mortal,
                enemies::arena_bot_for(*archetype),
            ));
            total += 1;
        }
    }
    info!("[roguelite] Wave {wave} spawned : {total} enemies");
    total
}

/// Tourne chaque frame en GameMode::Roguelite. Update bots_alive et orchestre
/// les transitions de vague.
pub fn sys_wave_orchestrator(
    time: Res<Time>,
    mut wave: ResMut<RogueliteWave>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_bots: Query<&ArenaBot>,
    mut end_run: MessageWriter<EndRunEvent>,
) {
    let alive = q_bots.iter().count() as u32;
    wave.bots_alive = alive;

    // Victory déjà émise → no-op (évite spam events).
    if wave.victory_emitted {
        return;
    }

    if alive == 0 && !wave.in_break {
        // Vague nettoyée — démarre break ou victory.
        if wave.current_wave >= WAVES_TOTAL {
            wave.victory_emitted = true;
            end_run.write(EndRunEvent {
                result: RunResult::Victory,
            });
            info!("[roguelite] All {WAVES_TOTAL} waves cleared — VICTORY");
            return;
        }
        wave.in_break = true;
        wave.break_secs_left = BREAK_SECS;
        info!(
            "[roguelite] Wave {} cleared — break {BREAK_SECS}s before wave {}",
            wave.current_wave,
            wave.current_wave + 1
        );
    }

    if wave.in_break {
        wave.break_secs_left -= time.delta_secs();
        if wave.break_secs_left <= 0.0 {
            wave.current_wave += 1;
            wave.in_break = false;
            wave.break_secs_left = 0.0;
            spawn_wave_enemies(&mut commands, &mut meshes, &mut materials, wave.current_wave);
        }
    }
}

/// M3 step 1 — marker enrage phase 2. Inséré quand HP boss < 50%.
#[derive(Component, Default)]
pub struct BossEnraged;

/// Détecte boss à ≤50% HP → insert BossEnraged + boost stats AI runtime.
/// Idempotent : `Without<BossEnraged>` filtre évite re-trigger.
pub fn sys_boss_enrage(
    mut commands: Commands,
    mut q_boss: Query<
        (Entity, &Health, &EnemyArchetype, &mut ArenaBot),
        Without<BossEnraged>,
    >,
) {
    for (entity, health, archetype, mut bot) in &mut q_boss {
        if *archetype != EnemyArchetype::Boss {
            continue;
        }
        let fraction = health.fraction();
        if fraction <= 0.5 {
            let stats = enemies::stats_for(EnemyArchetype::Boss);
            bot.speed = stats.speed * 1.8;
            bot.attack_cooldown = stats.attack_cooldown * 0.55;
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.insert(BossEnraged);
            }
            info!(
                "[roguelite] BOSS ENRAGED — phase 2 (HP {:.0}%, speed {:.1}, cooldown {:.2}s)",
                fraction * 100.0,
                bot.speed,
                bot.attack_cooldown
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_composition_grows() {
        let w1: u32 = wave_composition(1).iter().map(|(_, c, _)| *c).sum();
        let w2: u32 = wave_composition(2).iter().map(|(_, c, _)| *c).sum();
        let w3: u32 = wave_composition(3).iter().map(|(_, c, _)| *c).sum();
        assert!(w1 < w2, "wave 2 doit avoir plus d'ennemis que wave 1");
        assert_eq!(w1, 8);
        assert_eq!(w2, 12);
        assert_eq!(w3, 5, "wave 3 = 1 boss + 4 support");
    }

    #[test]
    fn wave_3_contains_boss() {
        let w3 = wave_composition(3);
        assert!(
            w3.iter().any(|(a, _, _)| *a == EnemyArchetype::Boss),
            "wave 3 doit contenir un Boss"
        );
    }

    #[test]
    fn wave_default_state() {
        let w = RogueliteWave::default();
        assert_eq!(w.current_wave, 1);
        assert_eq!(w.bots_alive, 0);
        assert!(!w.in_break);
        assert!(!w.victory_emitted);
    }

    #[test]
    fn waves_total_is_3() {
        assert_eq!(WAVES_TOTAL, 3);
    }

    #[test]
    fn break_secs_positive() {
        assert!(BREAK_SECS > 0.0);
    }
}
