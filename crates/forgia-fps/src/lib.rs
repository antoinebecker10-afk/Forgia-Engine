//! # forgia-fps
//!
//! Orchestrator Arena FPS — assemble le firing path, l'arena, le scoring, les
//! ammo systems et le tuning global. Le rendering du viewmodel 1P (mesh, ADS,
//! scope glass, auto-scale) vit dans `forgia-viewmodel` (Tier 2B 2026-05-19),
//! équivalent `CBaseViewModel` du Source SDK.
//!
//! Frontière respectée (Source SDK pattern + Bevy official example) :
//! - **forgia-viewmodel** : ce qui touche le mesh 1P (data layer, attach, pose, fade).
//! - **forgia-fps** : firing path orchestré + FPS tuning + ammo + scoring.
//! - **forgia-combat** : domaine `WeaponType`, `EquippedWeapons`, `WeaponFireCooldown`.

use bevy::ecs::system::SystemParam;
use bevy::input::mouse::MouseButtonInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
// Story-490 — DamageKind + DeathEvent pour bridge V7 damage pipeline (despawn_dead_cubes triggers DeathEvent before despawn → Roguelite observers loot/defeat fire correctement).
use forgia_combat::prelude::*;
use forgia_combat::weapons::{EquippedWeapons, WeaponFireCooldown};
use forgia_core::prelude::*;
use forgia_crosshair::CrosshairTuning;
use forgia_damage::{DamageKind, DeathEvent};
use forgia_effects::prelude::{
    spawn_hitscan_tracer, spawn_impact_vfx, spawn_muzzle_flash, TracerResources, WeaponVfxEffects,
};
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_juice_lib::camera_shake::{CameraShakeTuning, ForgiaJuiceCameraShakePlugin, ShakeImpulse};
use forgia_juice_lib::fov_punch::{ForgiaJuiceFovPunchPlugin, FovPunchImpulse, FovPunchTuning};
use forgia_juice_lib::hit_stop::HitStopState;
use forgia_juice_lib::recoil::{ForgiaJuiceRecoilPlugin, WeaponRecoilImpulse};
use forgia_mode_fps_arena::TargetCube;
use forgia_player::prelude::MouseLookTuning;
use forgia_player::prelude::*;
use forgia_viewmodel::{
    AdsState, AdsTuning, ForgiaViewmodelPlugin, ViewmodelGenomeCtx, ViewmodelGenomeEntry,
};
use serde::Deserialize;

mod ammo_systems;
mod hitscan_sensor;
pub use hitscan_sensor::{HitscanCategory, HitscanLogEntry, HitscanSensorState};
mod score;
pub mod aim_assist;

pub mod prelude {
    pub use crate::aim_assist::AimAssistTuning;
    pub use crate::score::{ArenaScore, ArenaScorePlugin, ScoreboardVisible};
    pub use crate::ForgiaFpsPlugin;
}

// ════════════════════════════════════════════════════════════════════════════
// Fire trigger state (Resources + dispatch pure)
// ════════════════════════════════════════════════════════════════════════════

/// État du clic gauche pour dispatch fire_mode (V2 : ButtonInput consommé par egui →
/// tracking via MessageReader<MouseButtonInput>).
/// `just_pressed` = transition Released→Pressed cette frame (mode semi/pump/burst).
/// `held` = bouton actuellement enfoncé (mode auto).
#[derive(Resource, Default)]
pub struct LeftMouseState {
    pub just_pressed: bool,
    pub held: bool,
}

/// État d'une rafale en cours (fire_mode = "burst").
/// Inséré au just_pressed initial, retiré quand `shots_remaining == 0`.
/// Pendant qu'il existe, le cooldown standard est bypassé — le timer interne pilote la cadence.
#[derive(Resource)]
pub struct BurstState {
    pub shots_remaining: u8,
    pub interval_timer: Timer,
}

/// Décision pure du dispatch fire_mode → (doit tirer cette frame, démarre une rafale).
/// Extrait de `fire_weapon_minimal` pour testabilité headless.
///
/// - `auto` : tire tant que `held`.
/// - `semi` / `pump` : tire uniquement sur `just_pressed`.
/// - `burst` : si rafale active, suit `burst_fires_now` ; sinon démarre sur `just_pressed`.
/// - inconnu : fallback `semi` (warn loggé par l'appelant).
pub fn dispatch_fire_trigger(
    fire_mode: &str,
    held: bool,
    just_pressed: bool,
    burst_active: bool,
    burst_fires_now: bool,
) -> (bool, bool) {
    match fire_mode {
        "auto" => (held, false),
        "semi" | "pump" => (just_pressed, false),
        "burst" => {
            if burst_active {
                (burst_fires_now, false)
            } else if just_pressed {
                (true, true)
            } else {
                (false, false)
            }
        }
        _ => (just_pressed, false),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// FPS Tuning Genome (fps_tuning.toml — anti-hardcode constantes feel)
// ════════════════════════════════════════════════════════════════════════════
//
// Toutes les constantes "feel" qui étaient hardcodées dans les crates juice /
// crosshair / player / ads. Loadées au Startup depuis `assets/genomes/fps_tuning.toml`
// et propagées par `sync_fps_tuning` à chaque downstream Resource Tuning.

#[derive(Deserialize, TypePath, Clone)]
pub struct FpsTuning {
    pub camera_shake: FtCameraShake,
    pub fov_punch: FtFovPunch,
    pub ads: FtAds,
    pub mouse_look: FtMouseLook,
    pub crosshair_hipfire: FtCrosshairHipfire,
    pub crosshair_ads_dot: FtCrosshairAdsDot,
    pub crosshair_sniper_overlay: FtCrosshairSniper,
    // Story-528 AC1 — aim assist accessibility (Roblox kids + casual).
    #[serde(default)]
    pub aim_assist: FtAimAssist,
}

#[derive(Deserialize, Clone)]
pub struct FtAimAssist {
    pub strength: f32,
    pub max_angle_deg: f32,
    pub engage_distance_m: f32,
}

impl Default for FtAimAssist {
    fn default() -> Self {
        Self {
            strength: 0.5,
            max_angle_deg: 5.0,
            engage_distance_m: 50.0,
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct FtCameraShake {
    pub default_decay: f32,
    pub default_max_rotation_rad: f32,
    pub yaw_factor: f32,
    pub roll_factor: f32,
    pub pitch_upward_bias: f32,
    pub sample_rate_hz: f32,
}

#[derive(Deserialize, Clone)]
pub struct FtFovPunch {
    pub attack_secs: f32,
    pub decay_secs: f32,
}

#[derive(Deserialize, Clone)]
pub struct FtAds {
    pub lerp_speed: f32,
    pub default_fov_deg: f32,
    pub punch_attenuation: f32,
}

#[derive(Deserialize, Clone)]
pub struct FtMouseLook {
    pub base_sensitivity: f32,
    pub recoil_decay_per_sec: f32,
}

#[derive(Deserialize, Clone)]
pub struct FtCrosshairHipfire {
    pub cross_len: f32,
    pub cross_stroke: f32,
    pub cross_alpha: u8,
}

#[derive(Deserialize, Clone)]
pub struct FtCrosshairAdsDot {
    pub outer_radius: f32,
    pub inner_radius: f32,
}

#[derive(Deserialize, Clone)]
pub struct FtCrosshairSniper {
    pub scope_radius_factor: f32,
    pub dim_alpha: u8,
    pub dim_color: [u8; 3],
    pub vignette_rings: u32,
    pub ring_thickness_factor: f32,
    pub ring_max_alpha: f32,
    pub ring_color: [u8; 3],
    pub ring_outer_extent: f32,
    pub border_width: f32,
    pub border_inner_width: f32,
    pub border_inner_offset: f32,
    pub reticle_gap: f32,
    pub reticle_line_factor: f32,
    pub reticle_color: [u8; 3],
    pub reticle_line_stroke: f32,
    pub reticle_tick_stroke: f32,
    pub reticle_tick_size: f32,
    pub red_dot_radius: f32,
    pub red_dot_color: [u8; 3],
}

#[derive(Resource)]
pub struct FpsTuningHandle(pub Handle<Genome<FpsTuning>>);

// ════════════════════════════════════════════════════════════════════════════
// SystemParams orchestrator (réduit le param count de fire_weapon_minimal)
// ════════════════════════════════════════════════════════════════════════════

/// Bundle des resources timing pour fire system (cooldown + burst + time virtuel/réel).
#[derive(SystemParam)]
pub struct FireTimingCtx<'w> {
    pub cooldown: Option<Res<'w, WeaponFireCooldown>>,
    pub burst_state: Option<ResMut<'w, BurstState>>,
    pub time: Res<'w, Time>,
    pub virtual_time: ResMut<'w, Time<Virtual>>,
}

/// Bundle des MessageWriters juice (shake / recoil / fov punch) pour fire system.
#[derive(SystemParam)]
pub struct JuiceWriters<'w> {
    pub shake: MessageWriter<'w, ShakeImpulse>,
    pub recoil: MessageWriter<'w, WeaponRecoilImpulse>,
    pub fov_punch: MessageWriter<'w, FovPunchImpulse>,
}

impl JuiceWriters<'_> {
    /// Emit les 3 impulses depuis le genome de l'arme. Yaw random uniforme [-yaw_max..+yaw_max].
    /// `seed` pour PRNG yaw (pseudo-déterministe par tir).
    pub fn emit_from_genome(&mut self, e: &ViewmodelGenomeEntry, seed: u32) {
        if e.shake_trauma > 0.0 {
            self.shake.write(ShakeImpulse {
                trauma: e.shake_trauma,
            });
        }
        if e.recoil_pitch_deg.abs() > 0.001 || e.recoil_yaw_random_deg.abs() > 0.001 {
            let yaw_signed = (pseudo_rand(seed) - 0.5) * 2.0 * e.recoil_yaw_random_deg;
            self.recoil.write(WeaponRecoilImpulse {
                pitch_rad: e.recoil_pitch_deg.to_radians(),
                yaw_rad: yaw_signed.to_radians(),
            });
        }
        if e.fov_punch_deg.abs() > 0.01 {
            self.fov_punch.write(FovPunchImpulse {
                peak_deg: e.fov_punch_deg,
            });
        }
    }
}

/// Story-453 baseline reset (2026-05-18) — query simplifiée : ray hit le PARENT
/// directement (bot = 1 entité unique avec capsule + Health + TargetCube).
#[derive(SystemParam)]
pub struct HitApplyCtx<'w, 's> {
    pub health: Query<
        'w,
        's,
        (
            &'static mut Health,
            Option<&'static MeshMaterial3d<StandardMaterial>>,
        ),
        With<TargetCube>,
    >,
}

/// Bundle hitscan diagnostic — q_children pour predicate récursif + sensor state.
#[derive(SystemParam)]
pub struct HitscanCtx<'w, 's> {
    pub q_children: Query<'w, 's, &'static Children>,
    pub q_child_of: Query<'w, 's, &'static ChildOf>,
    pub q_name: Query<'w, 's, &'static Name>,
    /// Story-457 — lookup zone sur le collider directement frappé.
    pub q_zone: Query<'w, 's, &'static forgia_damage::HitZoneTag>,
    pub sensor: ResMut<'w, HitscanSensorState>,
    /// Story-457 — multiplicateurs damage par zone (genome-driven, hot-reload).
    pub feedback: Res<'w, forgia_damage::HitFeedback>,
}

/// Multiplicateur damage falloff selon distance. Linéaire entre start et end.
/// Avant start = 1.0, après end = falloff_min.
pub fn falloff_multiplier(toi: f32, e: &ViewmodelGenomeEntry) -> f32 {
    if toi <= e.damage_falloff_start {
        return 1.0;
    }
    if toi >= e.damage_falloff_end {
        return e.damage_falloff_min;
    }
    let span = (e.damage_falloff_end - e.damage_falloff_start).max(0.001);
    let t = ((toi - e.damage_falloff_start) / span).clamp(0.0, 1.0);
    1.0_f32.lerp(e.damage_falloff_min, t)
}

// ════════════════════════════════════════════════════════════════════════════
// Plugin
// ════════════════════════════════════════════════════════════════════════════

pub struct ForgiaFpsPlugin;

impl Plugin for ForgiaFpsPlugin {
    fn build(&self, app: &mut App) {
        // MeshFaderPlugin (idempotent — plusieurs crates peuvent l'utiliser).
        if !app.is_plugin_added::<forgia_effects::mesh_fader::MeshFaderPlugin>() {
            app.add_plugins(forgia_effects::mesh_fader::MeshFaderPlugin);
        }
        // Arena spawn/cleanup + clouds.
        if !app.is_plugin_added::<forgia_mode_fps_arena::ForgiaModeFpsArenaPlugin>() {
            app.add_plugins(forgia_mode_fps_arena::ForgiaModeFpsArenaPlugin);
        }
        // Juice plugins (idempotent — check anti double-add).
        if !app.is_plugin_added::<ForgiaJuiceCameraShakePlugin>() {
            app.add_plugins(ForgiaJuiceCameraShakePlugin);
        }
        if !app.is_plugin_added::<ForgiaJuiceFovPunchPlugin>() {
            app.add_plugins(ForgiaJuiceFovPunchPlugin);
        }
        if !app.is_plugin_added::<ForgiaJuiceRecoilPlugin>() {
            app.add_plugins(ForgiaJuiceRecoilPlugin);
        }
        // Viewmodel = render layer (Tier 2B 2026-05-19, équivalent CBaseViewModel
        // Source SDK). Compose attach/pose/fade + load_viewmodel_genome Startup.
        if !app.is_plugin_added::<ForgiaViewmodelPlugin>() {
            app.add_plugins(ForgiaViewmodelPlugin);
        }
        app.add_plugins(score::ArenaScorePlugin)
            .init_resource::<EquippedWeapons>()
            .init_resource::<LeftMouseState>()
            .init_resource::<HitscanSensorState>()
            .init_resource::<aim_assist::AimAssistTuning>()
            .add_systems(Update, hitscan_sensor::write_hitscan_sensor)
            .init_asset::<Genome<FpsTuning>>()
            .register_asset_loader(GenomeLoader::<FpsTuning>::default())
            .add_systems(Startup, load_fps_tuning)
            .add_systems(Update, sync_fps_tuning)
            // Fire system genome-driven : dispatch fire_mode (auto/semi/pump/burst) + multi-pellets
            // + per-weapon damage/fire_rate/range/spread depuis ViewmodelGenomeEntry TOML.
            //
            // Story-455 Phase A — sync_ammo_slots_from_genome NE DOIT PAS être gated Fps.
            // Le genome ViewmodelGenome se charge au Startup → AssetEvent::Added arrive
            // ~50-200ms après boot, alors que GameMode = None (menu). Si gated Fps, l'event
            // expire du buffer Bevy avant que le user entre Arena → slots jamais peuplés.
            //
            // Fix : sync system tourne en permanence (idempotent, no-op si handle absent).
            .add_systems(Update, ammo_systems::sync_ammo_slots_from_genome)
            // Story-528 AC1 — aim assist : tourne dans GameSet::Camera (après mouse_look
            // qui est en Update.chain() côté forgia-player, avant Combat). Gating cross-mode
            // FPS + Roguelite — RPG est exclu (3P pas concerné).
            .add_systems(
                Update,
                aim_assist::aim_assist_system
                    .in_set(GameSet::Camera)
                    .run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite))),
            )
            .add_systems(
                Update,
                (
                    track_left_mouse_state,
                    weapon_select_system,
                    ammo_systems::cancel_reload_on_weapon_switch,
                    ammo_systems::reload_key_input,
                    ammo_systems::tick_ammo_reload,
                    fire_weapon_minimal,
                    despawn_dead_cubes,
                )
                    .chain()
                    .in_set(GameSet::Combat)
                    .run_if(
                        in_state(GameMode::Fps).or(in_state(GameMode::Roguelite)),
                    ),
            );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Helpers firing path
// ════════════════════════════════════════════════════════════════════════════

/// PRNG pseudo-déterministe ultra-léger (xorshift32). Out [0, 1).
fn pseudo_rand(seed: u32) -> f32 {
    let mut x = seed.max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    (x as f32) / (u32::MAX as f32)
}

/// Walk ChildOf ancestors max `max_depth` niveaux pour trouver l'entité qui porte
/// `Health` (typiquement le parent TargetCube d'un bot — story-453 architecture
/// `AsyncSceneCollider ConvexHull` : ray hit child mesh, parent porte Health).
fn find_health_ancestor(
    hit_entity: Entity,
    q_child_of: &Query<&ChildOf>,
    health_query: &Query<
        (&mut Health, Option<&MeshMaterial3d<StandardMaterial>>),
        With<TargetCube>,
    >,
    max_depth: u32,
) -> Option<Entity> {
    let mut current = hit_entity;
    for _ in 0..max_depth {
        if health_query.get(current).is_ok() {
            return Some(current);
        }
        match q_child_of.get(current) {
            Ok(co) => current = co.parent(),
            Err(_) => return None,
        }
    }
    None
}

/// Despawn les cubes morts (HP=0). Système séparé chained après fire.
fn despawn_dead_cubes(mut commands: Commands, q: Query<(Entity, &Health), With<TargetCube>>) {
    for (entity, hp) in &q {
        if hp.is_dead() {
            // Story-490 — bridge V7 damage pipeline. Trigger DeathEvent AVANT
            // despawn pour que les observers Roguelite (loot pickup spawn cf
            // run.rs:257, defeat detection cf run.rs:219) puissent réagir.
            // Sans ça, ennemis Roguelite meurent silencieusement → 0 Souls drop.
            // source=None car despawn_dead_cubes n'a pas l'info attaquant à ce
            // point (story-491 future passera DamageEvent en amont).
            commands.trigger(DeathEvent {
                target: entity,
                source: None,
                final_kind: DamageKind::Physical,
            });
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
            info!(
                "[death] cube {:?} despawned (HP=0) + DeathEvent fired",
                entity
            );
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Input → state systems
// ════════════════════════════════════════════════════════════════════════════

/// Switch arme via Digit1-4 (Pépin / Bourrasque / Madame Lenoir / Boucherie).
fn weapon_select_system(keys: Res<ButtonInput<KeyCode>>, mut equipped: ResMut<EquippedWeapons>) {
    let new_idx: Option<usize> = if keys.just_pressed(KeyCode::Digit1) {
        Some(0)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(2)
    } else if keys.just_pressed(KeyCode::Digit4) {
        Some(3)
    } else {
        None
    };
    if let Some(i) = new_idx {
        let target = forgia_combat::weapons::ARENA_V1_WEAPONS[i];
        if equipped.current != target {
            equipped.current = target;
            info!("[forgia-fps] weapon_select : Digit{} → {:?}", i + 1, target);
        }
    }
}

/// Maintien `LeftMouseState` (held + just_pressed) via MessageReader<MouseButtonInput>.
fn track_left_mouse_state(
    mut evs: MessageReader<MouseButtonInput>,
    mut state: ResMut<LeftMouseState>,
) {
    state.just_pressed = false; // reset chaque frame
    for ev in evs.read() {
        if ev.button == MouseButton::Left {
            match ev.state {
                ButtonState::Pressed => {
                    state.held = true;
                    state.just_pressed = true;
                }
                ButtonState::Released => {
                    state.held = false;
                }
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// fire_weapon_minimal — orchestrator firing path
// ════════════════════════════════════════════════════════════════════════════

/// Fire system genome-driven (Forgia V2).
///
/// Dispatch via `ViewmodelGenomeEntry.fire_mode` :
/// - `"auto"` : tire tant que held (Bourrasque SMG)
/// - `"semi"` : tire UNIQUEMENT sur just_pressed (Pépin, Madame Lenoir sniper)
/// - `"pump"` : just_pressed + multi-pellets cone spread (Boucherie shotgun)
/// - `"burst"` : rafale `burst_count` tirs à cadence `fire_rate`, puis cooldown long
///
/// Cooldown = `1.0 / entry.fire_rate` secondes. Damage/range/pellets/spread depuis genome.
/// Muzzle flash spawn à `origin + direction * entry.barrel_length`.
/// Multi-pellets : xorshift32 PRNG déterministe seed=position+pellet_idx pour reproducibility.
#[allow(clippy::too_many_arguments)]
fn fire_weapon_minimal(
    rapier: ReadRapierContext,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_player: Query<Entity, With<Player>>,
    mut hit_ctx: HitApplyCtx,
    mut commands: Commands,
    flash_cache: Res<HitFlashCache>,
    tracer_res: Option<Res<TracerResources>>,
    weapon_vfx: Option<Res<WeaponVfxEffects>>,
    mut timing: FireTimingCtx,
    mut hit_events: MessageWriter<CombatHitEvent>,
    mut juice: JuiceWriters,
    left: Res<LeftMouseState>,
    mut ammo: ammo_systems::AmmoCtx,
    genome_ctx: ViewmodelGenomeCtx,
    ads: Res<AdsState>,
    mut hitscan_ctx: HitscanCtx,
) {
    let entry = genome_ctx.entry(ammo.equipped.current);
    let fire_mode = entry.map(|e| e.fire_mode.as_str()).unwrap_or("auto");
    let is_burst_mode = fire_mode == "burst";

    // Tick burst state (fait avancer le timer interne). just_finished cette frame = fire.
    let mut burst_fires_now = false;
    let mut burst_active = false;
    let mut burst_will_terminate = false;
    if let Some(burst) = timing.burst_state.as_mut() {
        burst_active = true;
        if burst
            .interval_timer
            .tick(timing.time.delta())
            .just_finished()
        {
            burst_fires_now = true;
            burst.shots_remaining = burst.shots_remaining.saturating_sub(1);
            if burst.shots_remaining == 0 {
                burst_will_terminate = true;
            }
        }
    }

    // Pendant un burst actif, on bypass le cooldown standard (pacing géré par BurstState).
    if timing.cooldown.is_some() && !burst_active {
        return;
    }

    // Dispatch trigger selon fire_mode (logique pure extraite cf dispatch_fire_trigger).
    if !matches!(fire_mode, "auto" | "semi" | "pump" | "burst") {
        warn!("[fire] fire_mode inconnu '{}' — fallback semi", fire_mode);
    }
    let (trigger, starts_burst) = dispatch_fire_trigger(
        fire_mode,
        left.held,
        left.just_pressed,
        burst_active,
        burst_fires_now,
    );
    if !trigger {
        return;
    }

    // Story-455 Phase A — Ammo gate.
    if !ammo.try_fire() {
        return;
    }

    let Ok(cam_tf) = q_cam.single() else {
        warn!("[fire] FpsCamera not found");
        return;
    };
    let Ok(ctx) = rapier.single() else {
        warn!("[fire] RapierContext not found");
        return;
    };

    // Juice per-arme : shake + recoil + FOV punch.
    if let Some(e) = entry {
        let juice_seed = (timing.time.elapsed_secs() * 1000.0) as u32;
        juice.emit_from_genome(e, juice_seed);
    }

    let origin = cam_tf.translation();
    let direction = cam_tf.forward().as_vec3();

    // Cooldown depuis genome (fallback 0.1s = ModernAR 10 shots/s).
    let cooldown_s = entry.map(|e| 1.0 / e.fire_rate.max(0.1)).unwrap_or(0.1);
    // Burst : pas de cooldown standard entre les shots de la rafale (interval géré par BurstState).
    if !is_burst_mode {
        commands.insert_resource(WeaponFireCooldown {
            timer: Timer::from_seconds(cooldown_s, TimerMode::Once),
        });
    } else if starts_burst {
        let burst_count = entry.map(|e| e.burst_count.max(1)).unwrap_or(3);
        commands.insert_resource(BurstState {
            shots_remaining: burst_count.saturating_sub(1),
            interval_timer: Timer::from_seconds(cooldown_s, TimerMode::Repeating),
        });
    } else if burst_will_terminate {
        // Rafale finie : cooldown long avant pouvoir re-trigger.
        commands.remove_resource::<BurstState>();
        commands.insert_resource(WeaponFireCooldown {
            timer: Timer::from_seconds(cooldown_s * 3.0, TimerMode::Once),
        });
    }

    // Muzzle flash : position au BOUT DU CANON, lerp hipfire ↔ ADS via AdsState.progress.
    let barrel_len_base = entry.map(|e| e.barrel_length).unwrap_or(0.55);
    let p = ads.progress.clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| a + (b - a) * p;
    let gun_off_x = entry
        .map(|e| lerp(e.offset_x, e.ads_offset_x))
        .unwrap_or(0.22);
    let gun_off_y = entry
        .map(|e| lerp(e.offset_y, e.ads_offset_y))
        .unwrap_or(-0.35);
    let gun_off_z = entry
        .map(|e| lerp(e.offset_z, e.ads_offset_z))
        .unwrap_or(-1.30);
    let viewmodel_scale = entry.map(|e| lerp(1.0, e.ads_scale_factor)).unwrap_or(1.0);
    let barrel_len = barrel_len_base * viewmodel_scale;
    let forward_dist = (-gun_off_z) + barrel_len;
    let cam_right_v = cam_tf.right().as_vec3();
    let cam_up_v = cam_tf.up().as_vec3();
    let barrel_tip =
        origin + direction * forward_dist + cam_right_v * gun_off_x + cam_up_v * gun_off_y;
    if let Some(vfx) = weapon_vfx.as_deref() {
        spawn_muzzle_flash(
            &mut commands,
            vfx,
            barrel_tip,
            direction,
            &ammo.equipped.current,
        );
    }

    let range = entry.map(|e| e.range).unwrap_or(100.0);
    let damage = entry.map(|e| e.damage).unwrap_or(25.0);
    let pellets = entry.map(|e| e.pellets.max(1)).unwrap_or(1);
    let spread_rad = entry.map(|e| e.spread_deg.to_radians()).unwrap_or(0.0);

    // Exclure Player ET TOUS ses descendants (FpsCamera, viewmodel mesh, weapon child
    // colliders) du raycast. Pattern Overwatch GDC 2017 — Tim Ford.
    let mut excluded: std::collections::HashSet<Entity> = std::collections::HashSet::default();
    if let Ok(player_entity) = q_player.single() {
        excluded.insert(player_entity);
        let mut stack = vec![player_entity];
        while let Some(e) = stack.pop() {
            if let Ok(children) = hitscan_ctx.q_children.get(e) {
                for c in children.iter() {
                    if excluded.insert(c) {
                        stack.push(c);
                    }
                }
            }
        }
    }
    let predicate = |e: Entity| !excluded.contains(&e);

    let right = cam_tf.right().as_vec3();
    let up = cam_tf.up().as_vec3();

    // Seed PRNG basé sur position cam + ms hash — reproductibilité par tir.
    let seed_base = (origin.x.abs() * 1000.0) as u32
        ^ (origin.z.abs() * 1000.0) as u32
        ^ (origin.y.abs() * 1000.0) as u32;

    let mut hit_record: Option<(Entity, f32)> = None;

    for pellet_idx in 0..pellets {
        let pellet_dir = if pellets > 1 && spread_rad > 0.0 {
            let seed = seed_base
                .wrapping_add(u32::from(pellet_idx))
                .wrapping_mul(2654435761);
            let r1 = pseudo_rand(seed) - 0.5;
            let r2 = pseudo_rand(seed.wrapping_mul(0x9E3779B1)) - 0.5;
            let dev = right * (r1 * spread_rad) + up * (r2 * spread_rad);
            (direction + dev).normalize()
        } else {
            direction
        };

        // Story-453 baseline reset (2026-05-18) — ray cast simple, first hit only.
        let filter = QueryFilter::default().predicate(&predicate);
        let hit_result = ctx.cast_ray(origin, pellet_dir, range, true, filter);

        // Tracer + impact par pellet.
        let hit_dist = hit_result.map(|(_, t)| t).unwrap_or(range);
        if let Some(tres) = tracer_res.as_deref() {
            spawn_hitscan_tracer(
                &mut commands,
                tres,
                origin,
                pellet_dir,
                hit_dist,
                &ammo.equipped.current,
                range.min(120.0),
                0.30,
            );
        }
        if let Some((_, toi)) = hit_result {
            let impact_pos = origin + pellet_dir * toi;
            if let Some(vfx) = weapon_vfx.as_deref() {
                spawn_impact_vfx(&mut commands, vfx, impact_pos, &ammo.equipped.current);
            }
        }

        // Sensor categorization (BUG-RUN-1 fix story-455 — walk ChildOf vers Health).
        let target_ancestor = match hit_result {
            None => None,
            Some((entity, _)) => {
                find_health_ancestor(entity, &hitscan_ctx.q_child_of, &hit_ctx.health, 8)
            }
        };
        let (sensor_category, sensor_hit_idx, sensor_name, sensor_toi) = match hit_result {
            None => (HitscanCategory::Miss, None, None, None),
            Some((entity, toi)) => {
                // Story-517 fix : différencier HitZoneHead vs HitZoneBody dans le
                // sensor log. Avant ce fix le code écrivait toujours Body même
                // quand le ray touchait le head_proxy sphere (tagué HitZoneTag(Head)).
                // Le damage multiplier était déjà appliqué correctement via q_zone,
                // mais le sensor cosmétique mentait. Maintenant cohérent.
                let cat = if target_ancestor.is_some() {
                    let zone = hitscan_ctx
                        .q_zone
                        .get(entity)
                        .map(|t| t.0)
                        .unwrap_or(forgia_damage::HitZone::Body);
                    if zone == forgia_damage::HitZone::Head {
                        HitscanCategory::HitZoneHead
                    } else {
                        HitscanCategory::HitZoneBody
                    }
                } else {
                    HitscanCategory::BlockerNonZone
                };
                let display_entity = target_ancestor.unwrap_or(entity);
                let name = hitscan_ctx
                    .q_name
                    .get(display_entity)
                    .ok()
                    .map(|n| n.as_str().to_string());
                (cat, Some(entity.to_bits()), name, Some(toi))
            }
        };
        hitscan_ctx.sensor.push(HitscanLogEntry {
            t: timing.time.elapsed_secs(),
            weapon: ammo.equipped.current,
            origin,
            dir: pellet_dir,
            hit_entity_idx: sensor_hit_idx,
            hit_name: sensor_name,
            toi: sensor_toi,
            category: sensor_category,
        });

        // Apply damage : story-457 zone-based multiplier + falloff.
        if let Some((hit_collider, toi)) = hit_result {
            if let Some(entity) = target_ancestor {
                let zone = hitscan_ctx
                    .q_zone
                    .get(hit_collider)
                    .map(|t| t.0)
                    .unwrap_or(forgia_damage::HitZone::Body);
                let zone_mul = hitscan_ctx.feedback.0.damage_mul(zone);

                if let Ok((mut hp, mat_opt)) = hit_ctx.health.get_mut(entity) {
                    let falloff_mul = entry.map(|e| falloff_multiplier(toi, e)).unwrap_or(1.0);
                    let effective_dmg = damage * falloff_mul * zone_mul;
                    hp.current = (hp.current - effective_dmg).max(0.0);
                    let dead = hp.is_dead();
                    let new_hp = hp.current;

                    if let Some(mat_comp) = mat_opt {
                        let flash_dur = entry.map(|e| e.hit_flash_duration).unwrap_or(0.15);
                        commands
                            .entity(entity)
                            .insert(MeshMaterial3d(flash_cache.flash_material.clone()))
                            .insert(HitFlashTimer {
                                timer: Timer::from_seconds(flash_dur, TimerMode::Once),
                                original_emissive: LinearRgba::new(0.0, 0.0, 0.0, 1.0),
                                original_handle: Some(mat_comp.0.clone()),
                            });
                    }

                    let attacker_entity = q_player.single().ok();
                    let hit_world = origin + pellet_dir * toi;
                    let is_headshot = zone == forgia_damage::HitZone::Head;
                    hit_events.write(CombatHitEvent {
                        target: entity,
                        attacker: attacker_entity,
                        damage: effective_dmg,
                        is_kill: dead,
                        is_headshot,
                        hit_world_pos: hit_world,
                        weapon: Some(ammo.equipped.current),
                        body_zone: zone,
                    });

                    if hit_record.is_none() {
                        hit_record = Some((entity, toi));
                    }
                    info!(
                        "[fire] pellet {}/{} HIT {entity:?} toi={toi:.2}m dmg={effective_dmg:.1} hp={new_hp:.1} dead={dead}",
                        pellet_idx + 1, pellets
                    );
                }
            }
        }
    }

    // Hit-stop UNE FOIS par tir (pas par pellet) si au moins une cible touchée.
    if hit_record.is_some() {
        let hs_dur = entry.map(|e| e.hit_stop_duration).unwrap_or(0.05);
        let hs_speed = entry.map(|e| e.hit_stop_speed).unwrap_or(0.05);
        timing.virtual_time.set_relative_speed(hs_speed);
        commands.insert_resource(HitStopState {
            timer: Timer::from_seconds(hs_dur, TimerMode::Once),
            restore_speed: 1.0,
        });
    } else {
        info!(
            "[fire] miss ({} pellets, {:?})",
            pellets, ammo.equipped.current
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// FPS Tuning load + sync
// ════════════════════════════════════════════════════════════════════════════

fn load_fps_tuning(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Genome<FpsTuning>> = asset_server.load("genomes/fps_tuning.toml");
    commands.insert_resource(FpsTuningHandle(handle));
    info!("[forgia-fps] fps_tuning genome loading : genomes/fps_tuning.toml");
}

/// Sync system : lit FpsTuning genome et push vers chaque downstream Resource Tuning.
/// Hot-reload : éditer le TOML → Bevy Asset re-load → ce système push automatiquement.
#[allow(clippy::too_many_arguments)]
fn sync_fps_tuning(
    handle: Option<Res<FpsTuningHandle>>,
    assets: Res<Assets<Genome<FpsTuning>>>,
    mut cs_tuning: ResMut<CameraShakeTuning>,
    mut fp_tuning: ResMut<FovPunchTuning>,
    mut ml_tuning: ResMut<MouseLookTuning>,
    mut ch_tuning: ResMut<CrosshairTuning>,
    mut ads_tuning: ResMut<AdsTuning>,
    mut aa_tuning: ResMut<aim_assist::AimAssistTuning>,
) {
    let Some(g) = handle.as_deref().and_then(|h| assets.get(&h.0)) else {
        return;
    };
    let t = &g.data;
    // Story-528 AC1 — aim assist hot-reload.
    aa_tuning.strength = t.aim_assist.strength;
    aa_tuning.max_angle_deg = t.aim_assist.max_angle_deg;
    aa_tuning.engage_distance_m = t.aim_assist.engage_distance_m;
    // Camera shake
    cs_tuning.default_decay = t.camera_shake.default_decay;
    cs_tuning.default_max_rotation = t.camera_shake.default_max_rotation_rad;
    cs_tuning.yaw_factor = t.camera_shake.yaw_factor;
    cs_tuning.roll_factor = t.camera_shake.roll_factor;
    cs_tuning.pitch_upward_bias = t.camera_shake.pitch_upward_bias;
    cs_tuning.sample_rate_hz = t.camera_shake.sample_rate_hz;
    // FOV punch
    fp_tuning.attack_secs = t.fov_punch.attack_secs;
    fp_tuning.decay_secs = t.fov_punch.decay_secs;
    // Mouse look
    ml_tuning.base_sensitivity = t.mouse_look.base_sensitivity;
    ml_tuning.recoil_decay_per_sec = t.mouse_look.recoil_decay_per_sec;
    // ADS
    ads_tuning.lerp_speed = t.ads.lerp_speed;
    ads_tuning.default_fov_deg = t.ads.default_fov_deg;
    ads_tuning.punch_attenuation = t.ads.punch_attenuation;
    // Crosshair
    ch_tuning.hipfire_cross_len = t.crosshair_hipfire.cross_len;
    ch_tuning.hipfire_cross_stroke = t.crosshair_hipfire.cross_stroke;
    ch_tuning.hipfire_cross_alpha = t.crosshair_hipfire.cross_alpha;
    ch_tuning.ads_dot_outer_radius = t.crosshair_ads_dot.outer_radius;
    ch_tuning.ads_dot_inner_radius = t.crosshair_ads_dot.inner_radius;
    let s = &t.crosshair_sniper_overlay;
    ch_tuning.sniper_scope_radius_factor = s.scope_radius_factor;
    ch_tuning.sniper_dim_alpha = s.dim_alpha;
    ch_tuning.sniper_dim_color = s.dim_color;
    ch_tuning.sniper_vignette_rings = s.vignette_rings;
    ch_tuning.sniper_ring_thickness_factor = s.ring_thickness_factor;
    ch_tuning.sniper_ring_max_alpha = s.ring_max_alpha;
    ch_tuning.sniper_ring_color = s.ring_color;
    ch_tuning.sniper_ring_outer_extent = s.ring_outer_extent;
    ch_tuning.sniper_border_width = s.border_width;
    ch_tuning.sniper_border_inner_width = s.border_inner_width;
    ch_tuning.sniper_border_inner_offset = s.border_inner_offset;
    ch_tuning.sniper_reticle_gap = s.reticle_gap;
    ch_tuning.sniper_reticle_line_factor = s.reticle_line_factor;
    ch_tuning.sniper_reticle_color = s.reticle_color;
    ch_tuning.sniper_reticle_line_stroke = s.reticle_line_stroke;
    ch_tuning.sniper_reticle_tick_stroke = s.reticle_tick_stroke;
    ch_tuning.sniper_reticle_tick_size = s.reticle_tick_size;
    ch_tuning.sniper_red_dot_radius = s.red_dot_radius;
    ch_tuning.sniper_red_dot_color = s.red_dot_color;
}

// ════════════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaFpsPlugin;
    }

    #[test]
    fn left_mouse_state_default_idle() {
        let s = LeftMouseState::default();
        assert!(!s.held, "default held doit être false");
        assert!(!s.just_pressed, "default just_pressed doit être false");
    }

    #[test]
    fn pseudo_rand_in_unit_range() {
        for seed in [1u32, 42, 12345, u32::MAX / 2] {
            let v = pseudo_rand(seed);
            assert!(
                (0.0..1.0).contains(&v),
                "pseudo_rand({}) = {} hors [0,1)",
                seed,
                v
            );
        }
    }

    #[test]
    fn pseudo_rand_deterministic_same_seed() {
        assert_eq!(pseudo_rand(12345), pseudo_rand(12345));
        assert_ne!(pseudo_rand(12345), pseudo_rand(12346));
    }

    #[test]
    fn burst_state_decrement_via_timer() {
        let mut burst = BurstState {
            shots_remaining: 3,
            interval_timer: Timer::from_seconds(0.05, TimerMode::Repeating),
        };
        assert!(!burst.interval_timer.just_finished());
        burst
            .interval_timer
            .tick(std::time::Duration::from_millis(60));
        assert!(burst.interval_timer.just_finished());
        burst.shots_remaining = burst.shots_remaining.saturating_sub(1);
        assert_eq!(burst.shots_remaining, 2);
    }

    #[test]
    fn burst_state_terminates_at_zero() {
        let mut burst = BurstState {
            shots_remaining: 1,
            interval_timer: Timer::from_seconds(0.05, TimerMode::Repeating),
        };
        burst.shots_remaining = burst.shots_remaining.saturating_sub(1);
        assert_eq!(burst.shots_remaining, 0);
    }

    #[test]
    fn track_left_mouse_pressed_sets_both() {
        let mut app = App::new();
        app.add_message::<MouseButtonInput>()
            .init_resource::<LeftMouseState>()
            .add_systems(Update, track_left_mouse_state);

        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        let s = app.world().resource::<LeftMouseState>();
        assert!(s.held);
        assert!(s.just_pressed);
    }

    #[test]
    fn track_left_mouse_just_pressed_resets_each_frame() {
        let mut app = App::new();
        app.add_message::<MouseButtonInput>()
            .init_resource::<LeftMouseState>()
            .add_systems(Update, track_left_mouse_state);

        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        app.update();

        let s = app.world().resource::<LeftMouseState>();
        assert!(s.held);
        assert!(!s.just_pressed);
    }

    #[test]
    fn track_left_mouse_released_clears_held() {
        let mut app = App::new();
        app.add_message::<MouseButtonInput>()
            .init_resource::<LeftMouseState>()
            .add_systems(Update, track_left_mouse_state);

        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Released,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        let s = app.world().resource::<LeftMouseState>();
        assert!(!s.held);
    }

    #[test]
    fn dispatch_auto_uses_held_only() {
        assert_eq!(
            dispatch_fire_trigger("auto", true, false, false, false),
            (true, false)
        );
        assert_eq!(
            dispatch_fire_trigger("auto", false, true, false, false),
            (false, false)
        );
    }

    #[test]
    fn dispatch_semi_uses_just_pressed_only() {
        assert_eq!(
            dispatch_fire_trigger("semi", false, true, false, false),
            (true, false)
        );
        assert_eq!(
            dispatch_fire_trigger("semi", true, false, false, false),
            (false, false)
        );
    }

    #[test]
    fn dispatch_pump_behaves_like_semi() {
        assert_eq!(
            dispatch_fire_trigger("pump", false, true, false, false),
            (true, false)
        );
        assert_eq!(
            dispatch_fire_trigger("pump", true, false, false, false),
            (false, false)
        );
    }

    #[test]
    fn dispatch_burst_starts_on_just_pressed() {
        assert_eq!(
            dispatch_fire_trigger("burst", false, true, false, false),
            (true, true)
        );
    }

    #[test]
    fn dispatch_burst_follows_timer_when_active() {
        assert_eq!(
            dispatch_fire_trigger("burst", false, false, true, true),
            (true, false)
        );
        assert_eq!(
            dispatch_fire_trigger("burst", false, false, true, false),
            (false, false)
        );
        assert_eq!(
            dispatch_fire_trigger("burst", false, true, true, false),
            (false, false)
        );
    }

    #[test]
    fn dispatch_unknown_mode_fallbacks_semi() {
        assert_eq!(
            dispatch_fire_trigger("railgun", false, true, false, false),
            (true, false)
        );
        assert_eq!(
            dispatch_fire_trigger("railgun", true, false, false, false),
            (false, false)
        );
    }

    #[test]
    fn track_left_mouse_ignores_other_buttons() {
        let mut app = App::new();
        app.add_message::<MouseButtonInput>()
            .init_resource::<LeftMouseState>()
            .add_systems(Update, track_left_mouse_state);

        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Right,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        let s = app.world().resource::<LeftMouseState>();
        assert!(!s.held);
        assert!(!s.just_pressed);
    }
}
