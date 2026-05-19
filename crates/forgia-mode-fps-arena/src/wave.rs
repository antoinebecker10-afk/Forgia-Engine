//! # Wave system arena
//!
//! Gère les vagues progressives FPS arena : spawn wave N → all bots dead →
//! break_timer → spawn wave N+1. Données depuis `assets/genomes/arena_waves.toml`.
//!
//! Layer = framework + definition. Hot-reload natif Bevy (`Genome<ArenaWavesGenome>`).
//!
//! Sensor : `forgia_arena_waves.json` (1Hz) — current_wave, phase, bots_alive,
//! bots_total_wave, break_secs_left.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_ai_arena_bot::{ArenaBot, BotShootConfig};
use forgia_combat::Health;
use forgia_core::prelude::*;
use forgia_genome_core::{Genome, GenomeLoader};
use serde::Deserialize;
use std::fs;

use crate::{
    ArenaBotsGenome, ArenaBotsGenomeHandle, ArenaMarker, TargetCube, default_arena_bots,
};

// ─── Wave config genome ──────────────────────────────────────────────

#[derive(Deserialize, TypePath, Clone)]
pub struct ArenaWavesGenome {
    pub break_secs: f32,
    pub final_wave_index: u32,
    #[serde(rename = "wave")]
    pub waves: Vec<WaveDef>,
}

#[derive(Deserialize, TypePath, Clone)]
pub struct WaveDef {
    pub index: u32,
    #[serde(default = "default_one")]
    pub hp_mult: f32,
    #[serde(default = "default_one")]
    pub ai_cooldown_mult: f32,
    #[serde(default = "default_one")]
    pub ai_damage_mult: f32,
    pub spawns: Vec<WaveSpawnEntry>,
}

#[derive(Deserialize, TypePath, Clone)]
pub struct WaveSpawnEntry {
    pub character_glb: String,
    #[serde(default)]
    pub character_yaw_deg: f32,
    #[serde(default = "default_y_offset")]
    pub character_y_offset: f32,
    #[serde(default = "default_one")]
    pub character_scale: f32,
    pub x: f32,
    pub z: f32,
    /// Story-457 — head proxy sensor Y offset (m) relatif au parent bot.
    #[serde(default = "default_head_y")]
    pub head_y_offset: f32,
    /// Story-457 — head proxy sensor radius (m).
    #[serde(default = "default_head_r")]
    pub head_radius: f32,
}

fn default_head_y() -> f32 { 1.75 }
fn default_head_r() -> f32 { 0.22 }

fn default_one() -> f32 {
    1.0
}
fn default_y_offset() -> f32 {
    0.9
}

#[derive(Resource)]
pub struct ArenaWavesHandle(pub Handle<Genome<ArenaWavesGenome>>);

/// Fallback minimal si TOML pas encore loaded au boot.
fn fallback_waves() -> ArenaWavesGenome {
    ArenaWavesGenome {
        break_secs: 4.0,
        final_wave_index: 1,
        waves: vec![WaveDef {
            index: 1,
            hp_mult: 1.0,
            ai_cooldown_mult: 1.0,
            ai_damage_mult: 1.0,
            spawns: vec![],
        }],
    }
}

// ─── Wave state runtime ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavePhase {
    /// Pas encore commencé (entre OnEnter Fps et 1er spawn).
    PreBoot,
    /// Bots en jeu, on attend qu'ils meurent.
    Active,
    /// Pause entre 2 vagues — break_timer en cours.
    Break,
    /// Dernière vague terminée — victoire.
    AllComplete,
}

#[derive(Resource)]
pub struct WaveState {
    pub current_wave: u32,
    pub phase: WavePhase,
    pub break_timer: Timer,
    pub bots_alive: u32,
    pub bots_total_wave: u32,
    pub final_wave_index: u32,
}

impl Default for WaveState {
    fn default() -> Self {
        Self {
            current_wave: 0,
            phase: WavePhase::PreBoot,
            break_timer: Timer::from_seconds(4.0, TimerMode::Once),
            bots_alive: 0,
            bots_total_wave: 0,
            final_wave_index: 1,
        }
    }
}

#[derive(Message)]
pub struct WaveStartedEvent {
    pub wave_index: u32,
}

#[derive(Message)]
pub struct WaveCompletedEvent {
    pub wave_index: u32,
}

// ─── Plugin & systems ────────────────────────────────────────────────

pub struct ArenaWavesPlugin;

impl Plugin for ArenaWavesPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Genome<ArenaWavesGenome>>()
            .register_asset_loader(GenomeLoader::<ArenaWavesGenome>::default())
            .init_resource::<WaveState>()
            .add_message::<WaveStartedEvent>()
            .add_message::<WaveCompletedEvent>()
            .add_systems(Startup, load_arena_waves_genome)
            .add_systems(OnEnter(GameMode::Fps), reset_wave_state_on_enter)
            .add_systems(
                Update,
                (wave_orchestrator, wave_sensor)
                    .chain()
                    .run_if(in_state(GameMode::Fps)),
            );
    }
}

fn load_arena_waves_genome(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Genome<ArenaWavesGenome>> = asset_server.load("genomes/arena_waves.toml");
    commands.insert_resource(ArenaWavesHandle(handle));
    info!("[arena-waves] loading genomes/arena_waves.toml");
}

fn reset_wave_state_on_enter(mut state: ResMut<WaveState>) {
    state.current_wave = 0;
    state.phase = WavePhase::PreBoot;
    state.bots_alive = 0;
    state.bots_total_wave = 0;
    state.break_timer.reset();
}

/// Orchestrateur principal : compte les bots vivants, transitionne entre phases,
/// trigger le spawn de la wave suivante.
#[allow(clippy::too_many_arguments)]
fn wave_orchestrator(
    time: Res<Time>,
    mut state: ResMut<WaveState>,
    bots: Query<&Health, With<TargetCube>>,
    waves_handle: Option<Res<ArenaWavesHandle>>,
    waves_assets: Res<Assets<Genome<ArenaWavesGenome>>>,
    bots_handle: Option<Res<ArenaBotsGenomeHandle>>,
    bots_assets: Res<Assets<Genome<ArenaBotsGenome>>>,
    asset_server: Res<AssetServer>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
    commands: Commands,
    mut started: MessageWriter<WaveStartedEvent>,
    mut completed: MessageWriter<WaveCompletedEvent>,
) {
    let alive: u32 = bots.iter().filter(|h| h.current > 0.0).count() as u32;
    state.bots_alive = alive;

    let waves_owned;
    let waves_data: &ArenaWavesGenome = match waves_handle
        .as_deref()
        .and_then(|h| waves_assets.get(&h.0))
    {
        Some(g) => &g.data,
        None => {
            waves_owned = fallback_waves();
            &waves_owned
        }
    };

    // Sync final_wave_index depuis TOML (hot-reload friendly).
    state.final_wave_index = waves_data.final_wave_index;
    state.break_timer.set_duration(std::time::Duration::from_secs_f32(
        waves_data.break_secs.max(0.5),
    ));

    match state.phase {
        WavePhase::PreBoot => {
            // 1er spawn : tente wave 1 si waves loaded.
            if !waves_data.waves.is_empty() {
                let wave1 = &waves_data.waves[0];
                spawn_wave_bots(
                    commands,
                    asset_server,
                    meshes,
                    materials,
                    bots_handle.as_deref(),
                    &bots_assets,
                    wave1,
                );
                state.current_wave = wave1.index;
                state.bots_total_wave = wave1.spawns.len() as u32;
                state.bots_alive = state.bots_total_wave;
                state.phase = WavePhase::Active;
                started.write(WaveStartedEvent { wave_index: wave1.index });
                info!(
                    "[arena-waves] WAVE {} START — {} enemies",
                    wave1.index, state.bots_total_wave
                );
            }
        }
        WavePhase::Active => {
            if alive == 0 {
                completed.write(WaveCompletedEvent { wave_index: state.current_wave });
                if state.current_wave >= state.final_wave_index {
                    state.phase = WavePhase::AllComplete;
                    info!("[arena-waves] ALL WAVES COMPLETE — Boss defeated");
                } else {
                    state.phase = WavePhase::Break;
                    state.break_timer.reset();
                    info!(
                        "[arena-waves] Wave {} done → break {:.1}s",
                        state.current_wave,
                        waves_data.break_secs
                    );
                }
            }
        }
        WavePhase::Break => {
            state.break_timer.tick(time.delta());
            if state.break_timer.just_finished() {
                let next_idx = state.current_wave + 1;
                if let Some(next) = waves_data.waves.iter().find(|w| w.index == next_idx) {
                    spawn_wave_bots(
                        commands,
                        asset_server,
                        meshes,
                        materials,
                        bots_handle.as_deref(),
                        &bots_assets,
                        next,
                    );
                    state.current_wave = next.index;
                    state.bots_total_wave = next.spawns.len() as u32;
                    state.bots_alive = state.bots_total_wave;
                    state.phase = WavePhase::Active;
                    started.write(WaveStartedEvent { wave_index: next.index });
                    info!(
                        "[arena-waves] WAVE {} START — {} enemies",
                        next.index, state.bots_total_wave
                    );
                } else {
                    state.phase = WavePhase::AllComplete;
                    warn!("[arena-waves] No wave with index {next_idx} — stopping");
                }
            }
        }
        WavePhase::AllComplete => {
            // Idle final.
        }
    }
}

/// Spawn les bots d'une wave : extrait de l'ancien `spawn_arena`.
/// Story-453 baseline reset (2026-05-18) — bot = 1 entité avec capsule unique.
/// Plus de Head/Body children, plus de calibration AABB.
/// Le ray hit le parent direct → Health appliqué. Simple. Prévisible.
#[allow(clippy::too_many_arguments)]
pub fn spawn_wave_bots(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    bots_handle: Option<&ArenaBotsGenomeHandle>,
    bots_assets: &Assets<Genome<ArenaBotsGenome>>,
    wave: &WaveDef,
) {
    let bots_owned;
    let bots_data: &ArenaBotsGenome = match bots_handle.and_then(|h| bots_assets.get(&h.0)) {
        Some(g) => &g.data,
        None => {
            warn!("[arena-waves] arena_bots genome pas chargé — fallback hardcoded");
            bots_owned = default_arena_bots();
            &bots_owned
        }
    };

    let ai = &bots_data.ai;
    let hp = bots_data.hp * wave.hp_mult;
    let shot_cd = ai.shot_cooldown_secs * wave.ai_cooldown_mult;
    let shot_dmg = ai.shot_damage * wave.ai_damage_mult;
    let _ = (&mut meshes, &mut materials); // unused (no more debug capsule mesh)

    for spawn in &wave.spawns {
        let (x, z) = (spawn.x, spawn.z);
        let bot_name = spawn
            .character_glb
            .rsplit('/')
            .next()
            .and_then(|s| s.split('.').next())
            .unwrap_or("Enemy")
            .to_string();

        let character_path = spawn.character_glb.clone();
        let character_scale = spawn.character_scale;
        let character_yaw = spawn.character_yaw_deg.to_radians();
        let character_y_offset = spawn.character_y_offset;
        let head_y_offset = spawn.head_y_offset;
        let head_radius = spawn.head_radius;

        // Story-453 v2 (2026-05-18) — mesh-exact hitbox.
        // Parent = anchor world position + Health + RigidBody (PAS de Collider).
        // Le ConvexHull mesh-driven est généré par `AsyncSceneCollider` sur le SceneRoot child.
        // Ray hit le collider généré → ChildOf walk dans damage path → trouve parent Health.
        let parent_id = commands
            .spawn((
                ArenaMarker,
                TargetCube,
                Transform::from_xyz(x, 0.0, z),
                Visibility::default(),
                Health::new(hp),
                ArenaBot {
                    state: forgia_ai_arena_bot::BotState::Idle,
                    // Story-456 Phase 1 — speed depuis genome (était 0.0 hardcoded V2 baseline).
                    speed: ai.speed,
                    detect_range: ai.detect_range,
                    attack_range: ai.shot_range,
                    attack_cooldown: shot_cd,
                    attack_left: ai.shot_warmup_secs,
                    // stop_distance = distance d'arrêt en Chase. Bot s'approche jusqu'à
                    // cette distance puis stagne pour tirer. Doit être < shot_range.
                    stop_distance: ai.stop_distance,
                    // Story-456 Phase 2-4 — tactical state (init défaut, populated runtime
                    // par tactical::bot_los_check / bot_perception_alert / bot_tactical_movement).
                    has_los: false,
                    los_check_left: 0.0,
                    los_grace_left: 0.0,
                    // strafe_phase_rad offset par-bot pour désync (sinon tous bougent en sync).
                    // Hash basé sur position spawn = déterministe + diversifié.
                    strafe_phase_rad: ((x.to_bits() ^ z.to_bits()) as f32 * 0.0001) % std::f32::consts::TAU,
                    strafe_noise_seed: (x.to_bits() ^ z.to_bits()).wrapping_mul(2654435761),
                    alerted: false,
                    alert_left: 0.0,
                },
                BotShootConfig {
                    damage: shot_dmg,
                    range: ai.shot_range,
                    jitter_rad: ai.shot_jitter_deg.to_radians(),
                    tracer_emissive: LinearRgba::new(
                        ai.tracer_emissive[0],
                        ai.tracer_emissive[1],
                        ai.tracer_emissive[2],
                        1.0,
                    ),
                    shoulder_y: 1.4,
                    target_torso_y: 1.0,
                },
                // Story-456 Phase 1 — KinematicPositionBased au lieu de Fixed.
                // Rapier 0.33 : Kinematic suit le Transform mutation directe par le state
                // machine ArenaBot. Pas de simulation physique, pas de collision auto
                // (raycast obstacle avoidance phase 3 gère ça).
                RigidBody::KinematicPositionBased,
                Name::new(format!("Enemy_{bot_name}_W{}_{x}_{z}", wave.index)),
            ))
            .id();

        // Character mesh = enfant SceneRoot. AsyncSceneCollider walk les meshes
        // et génère un collider ConvexHull par mesh (épouse silhouette exacte).
        // Rapier attache automatiquement ces colliders au RigidBody du parent.
        if !character_path.is_empty() {
            commands.entity(parent_id).with_children(|p| {
                p.spawn((
                    SceneRoot(asset_server.load(&character_path)),
                    Transform::from_xyz(0.0, character_y_offset, 0.0)
                        .with_rotation(Quat::from_rotation_y(character_yaw))
                        .with_scale(Vec3::splat(character_scale)),
                    Name::new("CharacterMesh"),
                    AsyncSceneCollider {
                        shape: Some(ComputedColliderShape::ConvexHull),
                        ..default()
                    },
                ));
            });
        }

        // Story-457 — Head proxy : sphere Sensor collider enfant du bot, tagué
        // HitZone::Head. Ray query hit cette sphère AVANT le convex hull body
        // si visée tête (sphère légèrement saillante du contour body convex
        // hull). Sensor = pas de collision physique solide (le bot reste
        // KinematicPositionBased), juste détecté en raycast.
        commands.entity(parent_id).with_children(|p| {
            p.spawn((
                Transform::from_xyz(0.0, head_y_offset, 0.0),
                Collider::ball(head_radius),
                Sensor,
                forgia_damage::HitZoneTag(forgia_damage::HitZone::Head),
                Name::new("HeadProxy"),
            ));
        });
    }
}

// ─── Sensor 1Hz ──────────────────────────────────────────────────────

fn wave_sensor(state: Res<WaveState>, mut last_write: Local<f32>, time: Res<Time>) {
    *last_write += time.delta_secs();
    if *last_write < 1.0 {
        return;
    }
    *last_write = 0.0;

    let phase = match state.phase {
        WavePhase::PreBoot => "preboot",
        WavePhase::Active => "active",
        WavePhase::Break => "break",
        WavePhase::AllComplete => "all_complete",
    };
    let json = format!(
        r#"{{"timestamp_secs":{:.2},"current_wave":{},"final_wave":{},"phase":"{}","bots_alive":{},"bots_total":{},"break_secs_left":{:.2}}}"#,
        time.elapsed_secs(),
        state.current_wave,
        state.final_wave_index,
        phase,
        state.bots_alive,
        state.bots_total_wave,
        state.break_timer.remaining_secs(),
    );
    let _ = fs::write("forgia_arena_waves.json", json);
}
