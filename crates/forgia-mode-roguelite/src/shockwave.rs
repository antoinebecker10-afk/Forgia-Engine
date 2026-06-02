//! shockwave.rs — Sort F « Onde de choc » (AOE), story-572.
//!
//! Première compétence active du Roguelite (modèle Gunfire « skill »). V1 universel
//! (même sort pour toutes les armes ; le per-arme lié aux armes parlantes = futur).
//!
//! Touche **F** → si cooldown prêt :
//! - dégâts en zone (`DamageEvent` kind Explosion → pipeline `apply_damage`, donc
//!   kills comptent : souls/killfeed/death) à tous les `ArenaBot` dans le rayon (XZ)
//! - punch caméra (`CameraTrauma`) + VFX disque au sol qui s'étend et fade
//! - cooldown réinitialisé
//!
//! Constantes V1 (radius/damage/cooldown) — à externaliser genome (story-566 balance).

use bevy::prelude::*;
use forgia_ai_arena_bot::ArenaBot;
use forgia_combat::combat_juice::{CameraTrauma, CombatHitEvent};
// IMPORTANT : les bots portent `forgia_combat::Health` (cf waves.rs:26), PAS
// `forgia_damage::Health` (type distinct). Querier le mauvais → 0 match → 0 dégât.
use forgia_combat::Health;
use forgia_core::prelude::*;
use forgia_damage::HitZone;
use forgia_player::Player;

use crate::run::RunState;

/// Cooldown du sort (s). V1 const — genome story-566.
pub const SHOCKWAVE_COOLDOWN: f32 = 8.0;
/// Rayon d'effet (m, distance XZ). 12 m = panic button utilisable même quand
/// le joueur combat à mi-distance (6 m attrapait trop rarement les ennemis).
pub const SHOCKWAVE_RADIUS: f32 = 12.0;
/// Dégâts infligés à chaque ennemi dans le rayon.
pub const SHOCKWAVE_DAMAGE: f32 = 45.0;
/// Durée de la VFX disque (s).
const VFX_DURATION: f32 = 0.40;

/// État du sort F (cooldown + compteur de casts pour sensor).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ShockwaveAbility {
    pub cooldown_left: f32,
    pub casts_total: u32,
}

/// VFX disque au sol (scale + fade + despawn). `mat` = handle pour animer l'alpha.
#[derive(Component)]
pub struct ShockwaveVfx {
    pub age: f32,
    pub mat: Handle<StandardMaterial>,
}

/// Décrémente le cooldown chaque frame.
pub fn sys_tick_shockwave_cooldown(time: Res<Time>, mut ab: ResMut<ShockwaveAbility>) {
    if ab.cooldown_left > 0.0 {
        ab.cooldown_left = (ab.cooldown_left - time.delta_secs()).max(0.0);
    }
}

/// Input F → cast de l'onde de choc (gate InGame + Roguelite + InRun/Boss).
///
/// Mirroir EXACT du fps fire path (forgia-fps:803) : on mutate `Health.current`
/// directement + on émet `CombatHitEvent`. Le despawn/loot/mort est géré par
/// `forgia-fps::despawn_dead_cubes` (détecte `hp.is_dead()` sur TargetCube →
/// DeathEvent + despawn). Surtout PAS de `DamageEvent` ici : `apply_damage`
/// fire AUSSI un DeathEvent → double DeathEvent → double loot sur kill.
/// `CombatHitEvent` pilote : chiffres flottants, hit flash, killfeed, kill_popup,
/// boons heal/chain on kill, screen flash, camera.
#[allow(clippy::too_many_arguments)]
pub fn sys_shockwave_input(
    keys: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppMode>>,
    run_state: Option<Res<State<RunState>>>,
    mut ab: ResMut<ShockwaveAbility>,
    q_player: Query<(Entity, &Transform), With<Player>>,
    q_bots: Query<(Entity, &Transform), With<ArenaBot>>,
    mut q_health: Query<&mut Health>,
    mut hits_w: MessageWriter<CombatHitEvent>,
    mut trauma: ResMut<CameraTrauma>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if *app_state.get() != AppMode::InGame {
        return;
    }
    let in_run = matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    );
    if !in_run || ab.cooldown_left > 0.0 || !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Ok((player_e, player_tf)) = q_player.single() else {
        return;
    };
    let origin = player_tf.translation;

    // Dégâts en zone — distance XZ (ignore la hauteur). Mutation HP directe +
    // CombatHitEvent par ennemi touché (= bullet hit). despawn_dead_cubes gère la mort.
    let r2 = SHOCKWAVE_RADIUS * SHOCKWAVE_RADIUS;
    let mut total = 0u32;
    let mut in_range = 0u32;
    let mut hits = 0u32;
    for (e, tf) in &q_bots {
        total += 1;
        let d = tf.translation - origin;
        if d.x * d.x + d.z * d.z > r2 {
            continue;
        }
        in_range += 1;
        // Lookup Health par entité (découplé de la détection — robuste si Health
        // n'était pas sur l'entité ArenaBot).
        let Ok(mut hp) = q_health.get_mut(e) else {
            continue;
        };
        if hp.is_dead() {
            continue;
        }
        hp.current = (hp.current - SHOCKWAVE_DAMAGE).max(0.0);
        let dead = hp.is_dead();
        hits_w.write(CombatHitEvent {
            target: e,
            attacker: Some(player_e),
            damage: SHOCKWAVE_DAMAGE,
            is_kill: dead,
            is_headshot: false,
            hit_world_pos: tf.translation + Vec3::Y * 1.2,
            weapon: None,
            body_zone: HitZone::Body,
        });
        hits += 1;
    }

    ab.cooldown_left = SHOCKWAVE_COOLDOWN;
    ab.casts_total = ab.casts_total.saturating_add(1);
    trauma.add(0.5);

    // VFX disque doré au sol (unlit + blend pour glow cartoon lisible).
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.77, 0.19, 0.85),
        emissive: LinearRgba::rgb(2.0, 1.4, 0.3),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let mesh = meshes.add(Cylinder::new(1.0, 0.06));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(origin + Vec3::Y * 0.1).with_scale(Vec3::new(0.5, 1.0, 0.5)),
        ShockwaveVfx { age: 0.0, mat },
    ));

    info!(
        "[roguelite] Onde de choc — {hits} touchés / {in_range} en portée / {total} bots (rayon {SHOCKWAVE_RADIUS}m, cd {SHOCKWAVE_COOLDOWN}s)"
    );
}

/// Anime la VFX : étend le disque (0.5→rayon), fade l'alpha, despawn en fin.
pub fn sys_animate_shockwave_vfx(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(Entity, &mut Transform, &mut ShockwaveVfx)>,
) {
    for (e, mut tf, mut vfx) in &mut q {
        vfx.age += time.delta_secs();
        let t = (vfx.age / VFX_DURATION).clamp(0.0, 1.0);
        let scale = 0.5 + (SHOCKWAVE_RADIUS - 0.5) * t;
        tf.scale = Vec3::new(scale, 1.0, scale);
        if let Some(m) = materials.get_mut(&vfx.mat) {
            m.base_color = Color::srgba(1.0, 0.77, 0.19, 0.85 * (1.0 - t));
        }
        if vfx.age >= VFX_DURATION {
            commands.entity(e).despawn();
        }
    }
}
