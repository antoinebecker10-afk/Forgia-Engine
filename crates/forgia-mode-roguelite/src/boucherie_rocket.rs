//! boucherie_rocket.rs — Story-534 AC1+AC10. La roquette parabolique de Boucherie.
//!
//! GDD Mission 3 : « roquette parabolique 12 m/s, explosion 4 m radius, 70 dmg
//! avec knockback, mag 3, reload shell ». Le fire path forgia-fps reste le
//! plombier (ammo/cooldown/recoil/muzzle/SFX) avec `damage = 0` dans le genome
//! (`projectile_weapon` guard) ; ICI vivent la balistique et l'explosion.
//!
//! - **Spawn** : `WeaponFiredEvent { weapon: RocketLauncher }` (multicast,
//!   audio.rs lit le même message) → roquette depuis la caméra, vel = fwd×12.
//! - **Vol** : intégration manuelle (gravité projectile dédiée = lob lisible),
//!   segment-raycast rapier par frame (anti-tunneling), prédicat exclut le
//!   joueur + ses descendants (viewmodel/armes ont des colliders).
//! - **Explosion** : `shockwave::aoe_strike` (dégâts + Knockback pop +
//!   CombatHitEvent attribué RocketLauncher → barks/killfeed/loot) +
//!   `shockwave::spawn_disc_vfx` VERT poison + CameraTrauma.
//!
//! VFX = identité élément **Poison** (vert) : roquette, explosion et preview de
//! trajectoire sont verts (étaient orange = feu → confusion avec Bourrasque).

use bevy::prelude::*;
use bevy_rapier3d::prelude::{QueryFilter, ReadRapierContext};
use forgia_ai_arena_bot::ArenaBot;
use forgia_combat::combat_juice::{CameraTrauma, CombatHitEvent, WeaponFiredEvent};
use forgia_combat::weapons::{EquippedWeapons, WeaponType};
use forgia_combat::Health;
use forgia_core::prelude::*;
use forgia_player::{FpsCamera, Player};
use std::fs;

use crate::shockwave::{spawn_disc_vfx, Knockback};

const SENSOR_PATH: &str = "forgia2_boucherie.json";

// ─── Constantes balistique/explosion (V1 — genome story-566 avec les sorts) ──
/// Vitesse roquette (user 2026-06-20 : 12 → 30 m/s, plus rapide/tendu). Le
/// preview de trajectoire montre désormais l'impact, donc la lisibilité ne
/// dépend plus d'une vitesse lente. Ajustable ici (hardcode V1 ; genome story-566).
const ROCKET_SPEED: f32 = 30.0;
/// Gravité projectile dédiée (pas la gravité monde) : lob parabolique doux.
const ROCKET_GRAVITY: f32 = 5.0;
/// Petit coup de pouce vertical au lancement (l'arc « festif »).
const ROCKET_LOB_UP: f32 = 2.0;
/// GDD : explosion 4 m.
const EXPLOSION_RADIUS: f32 = 4.0;
/// GDD : 70 dmg (plein dans le rayon — pas de falloff V1, lisible cartoon).
const EXPLOSION_DAMAGE: f32 = 70.0;
/// GDD : knockback 8 m — vitesse × KNOCKBACK_DURATION (0.32 s) ≈ 8 m.
const EXPLOSION_PUSH_SPEED: f32 = 25.0;
const EXPLOSION_POP: f32 = 2.0;
/// Failsafe : explose en l'air si rien touché (sortie d'arène).
const ROCKET_LIFETIME: f32 = 6.0;
/// Spawn devant la caméra, au-delà du viewmodel (offset_z = -1.3 + marge).
const SPAWN_AHEAD: f32 = 1.8;

/// Une roquette en vol (intégration manuelle, pas de RigidBody).
#[derive(Component)]
pub struct RocketProjectile {
    pub vel: Vec3,
    pub age: f32,
}

/// Stats de la run — story-534 AC10.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct BoucherieStats {
    pub rockets_fired: u32,
    pub explosions_run: u32,
    pub enemies_hit_run: u32,
    pub kills_run: u32,
}

impl BoucherieStats {
    pub fn reset_run(&mut self) {
        *self = Self::default();
    }
}

// ─── Balistique pure (testable) ──────────────────────────────────────────────

/// Vélocité de lancement : forward caméra × vitesse + lob vertical.
pub(crate) fn launch_velocity(cam_forward: Vec3) -> Vec3 {
    cam_forward * ROCKET_SPEED + Vec3::Y * ROCKET_LOB_UP
}

/// Un pas d'intégration : gravité projectile puis translation.
pub(crate) fn integrate(pos: Vec3, vel: Vec3, dt: f32) -> (Vec3, Vec3) {
    let vel = vel - Vec3::Y * ROCKET_GRAVITY * dt;
    (pos + vel * dt, vel)
}

// ─── Systems ─────────────────────────────────────────────────────────────────

/// Spawn d'une roquette à chaque tir Boucherie (le tir lui-même = forgia-fps).
fn sys_spawn_rockets(
    mut fired: MessageReader<WeaponFiredEvent>,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    mut stats: ResMut<BoucherieStats>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in fired.read() {
        if ev.weapon != WeaponType::RocketLauncher {
            continue;
        }
        let Ok(cam) = q_cam.single() else { continue };
        let fwd: Vec3 = cam.forward().into();
        let origin = cam.translation() + fwd * SPAWN_AHEAD;
        // Vert POISON (identité élément Boucherie) — était orange = feu, source de
        // confusion avec Bourrasque. VFX aligné sur l'élément (cf story-611).
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.95, 0.18),
            emissive: LinearRgba::rgb(0.6, 4.0, 0.3),
            unlit: true,
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(0.16))),
            MeshMaterial3d(mat),
            Transform::from_translation(origin),
            RocketProjectile {
                vel: launch_velocity(fwd),
                age: 0.0,
            },
            DespawnOnExit(GameMode::Roguelite),
        ));
        stats.rockets_fired += 1;
    }
}

/// Vol + détection d'impact (segment-raycast) + explosion.
#[allow(clippy::too_many_arguments)]
fn sys_fly_rockets(
    time: Res<Time>,
    rapier: ReadRapierContext,
    mut q_rockets: Query<(Entity, &mut Transform, &mut RocketProjectile)>,
    q_player: Query<Entity, With<Player>>,
    q_child_of: Query<&ChildOf>,
    q_bots: Query<(Entity, &Transform), (With<ArenaBot>, Without<RocketProjectile>)>,
    mut q_health: Query<&mut Health>,
    mut hits_w: MessageWriter<CombatHitEvent>,
    mut trauma: ResMut<CameraTrauma>,
    mut stats: ResMut<BoucherieStats>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if q_rockets.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let Ok(player_e) = q_player.single() else {
        return;
    };
    // Exclut le joueur ET ses descendants (viewmodel/colliders d'arme) du ray.
    let is_world = |e: Entity| -> bool {
        let mut cur = e;
        for _ in 0..8 {
            if cur == player_e {
                return false;
            }
            match q_child_of.get(cur) {
                Ok(parent) => cur = parent.parent(),
                Err(_) => return true,
            }
        }
        true
    };

    let ctx = rapier.single().ok();
    for (rocket_e, mut tf, mut rocket) in &mut q_rockets {
        rocket.age += dt;
        let prev = tf.translation;
        let (next, vel) = integrate(prev, rocket.vel, dt);
        rocket.vel = vel;

        // Segment-cast prev→next (anti-tunneling sur murs/bots).
        let step = next - prev;
        let step_len = step.length();
        let impact = if step_len > 1e-4 {
            ctx.as_ref().and_then(|c| {
                let filter = QueryFilter::default().predicate(&is_world);
                c.cast_ray(prev, step / step_len, step_len, true, filter)
                    .map(|(_, toi)| prev + (step / step_len) * toi)
            })
        } else {
            None
        };

        match impact {
            Some(point) => {
                explode(
                    point, player_e, &q_bots, &mut q_health, &mut hits_w, &mut trauma,
                    &mut stats, &mut commands, &mut meshes, &mut materials,
                );
                commands.entity(rocket_e).despawn();
            }
            None if rocket.age >= ROCKET_LIFETIME => {
                explode(
                    next, player_e, &q_bots, &mut q_health, &mut hits_w, &mut trauma,
                    &mut stats, &mut commands, &mut meshes, &mut materials,
                );
                commands.entity(rocket_e).despawn();
            }
            None => {
                tf.translation = next;
                // Oriente le nez dans la direction du vol (lisible en l'air).
                if vel.length_squared() > 1e-4 {
                    tf.look_to(vel.normalize(), Vec3::Y);
                }
            }
        }
    }
}

/// Explosion : AOE dégâts + Knockback pop + VFX + trauma. Miroir local
/// d'`aoe_strike` (shockwave) — le filtre `Without<RocketProjectile>` de notre
/// query bots (disjonction Bevy avec la query roquettes `&mut Transform`) rend
/// les types incompatibles avec la signature partagée.
#[allow(clippy::too_many_arguments)]
fn explode(
    center: Vec3,
    player_e: Entity,
    q_bots: &Query<(Entity, &Transform), (With<ArenaBot>, Without<RocketProjectile>)>,
    q_health: &mut Query<&mut Health>,
    hits_w: &mut MessageWriter<CombatHitEvent>,
    trauma: &mut CameraTrauma,
    stats: &mut BoucherieStats,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let r2 = EXPLOSION_RADIUS * EXPLOSION_RADIUS;
    let mut affected = 0u32;
    for (e, tf) in q_bots {
        let dx = tf.translation.x - center.x;
        let dz = tf.translation.z - center.z;
        if dx * dx + dz * dz > r2 {
            continue;
        }
        affected += 1;
        // Push radial depuis le point d'impact + pop vertical (« ils volent »).
        let mut dir = Vec3::new(dx, 0.0, dz).normalize_or_zero();
        if dir == Vec3::ZERO {
            dir = Vec3::Z;
        }
        commands.entity(e).insert(Knockback {
            vel: dir * EXPLOSION_PUSH_SPEED,
            time_left: 0.32,
            total: 0.32,
            pop_height: EXPLOSION_POP,
            ground_y: tf.translation.y,
        });
        if let Ok(mut hp) = q_health.get_mut(e) {
            if !hp.is_dead() {
                hp.current = (hp.current - EXPLOSION_DAMAGE).max(0.0);
                let dead = hp.is_dead();
                hits_w.write(CombatHitEvent {
                    target: e,
                    attacker: Some(player_e),
                    damage: EXPLOSION_DAMAGE,
                    is_kill: dead,
                    is_headshot: false,
                    hit_world_pos: tf.translation + Vec3::Y * 1.2,
                    weapon: Some(WeaponType::RocketLauncher),
                    body_zone: forgia_damage::HitZone::Body,
                });
            }
        }
    }
    stats.explosions_run += 1;
    stats.enemies_hit_run += affected;
    spawn_disc_vfx(
        commands, meshes, materials, center, 0.5, EXPLOSION_RADIUS,
        // Vert POISON (cf roquette) — distinct de l'AOE Pépin (jaune) et du feu.
        Srgba::new(0.35, 0.95, 0.20, 0.90), LinearRgba::rgb(0.6, 3.5, 0.3),
    );
    trauma.add(0.5);
    info!("[boucherie] BOUM ! roquette → {affected} touchés (r={EXPLOSION_RADIUS}m)");
}

// ─── Preview de trajectoire (aide de visée Boucherie) ────────────────────────

/// Pas de simulation du preview (dt grossier = moins de raycasts qu'à 60 fps).
const PREVIEW_DT: f32 = 1.0 / 30.0;
/// Plafond de pas (~3 s de vol) — early-break dès l'impact en pratique.
const PREVIEW_MAX_STEPS: usize = 90;

/// Trace l'arc balistique prévu + le cercle de la zone d'explosion au point
/// d'arrivée, tant que la roquette (RocketLauncher) est en main. Rejoue la MÊME
/// balistique que la vraie roquette (`launch_velocity` + `integrate` +
/// segment-raycast) → la projection ne ment pas. Hot path borné : gated arme en
/// main, early-break à l'impact, 0 allocation (gizmos immediate-mode).
fn sys_draw_trajectory_preview(
    mut gizmos: Gizmos,
    equipped: Res<EquippedWeapons>,
    rapier: ReadRapierContext,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_player: Query<Entity, With<Player>>,
    q_child_of: Query<&ChildOf>,
) {
    if equipped.current != WeaponType::RocketLauncher {
        return;
    }
    let Ok(cam) = q_cam.single() else { return };
    let Ok(player_e) = q_player.single() else { return };
    // Exclut le joueur + ses descendants (mêmes règles que la roquette réelle).
    let is_world = |e: Entity| -> bool {
        let mut cur = e;
        for _ in 0..8 {
            if cur == player_e {
                return false;
            }
            match q_child_of.get(cur) {
                Ok(parent) => cur = parent.parent(),
                Err(_) => return true,
            }
        }
        true
    };
    let ctx = rapier.single().ok();
    let fwd: Vec3 = cam.forward().into();
    let mut pos = cam.translation() + fwd * SPAWN_AHEAD;
    let mut vel = launch_velocity(fwd);

    let arc = Color::srgb(0.35, 0.95, 0.20); // vert poison (cf roquette/explosion)
    let mut impact: Option<Vec3> = None;
    for _ in 0..PREVIEW_MAX_STEPS {
        let (next, v) = integrate(pos, vel, PREVIEW_DT);
        let step = next - pos;
        let len = step.length();
        let hit = if len > 1e-4 {
            ctx.as_ref().and_then(|c| {
                let filter = QueryFilter::default().predicate(&is_world);
                c.cast_ray(pos, step / len, len, true, filter)
                    .map(|(_, toi)| pos + (step / len) * toi)
            })
        } else {
            None
        };
        match hit {
            Some(point) => {
                gizmos.line(pos, point, arc);
                impact = Some(point);
                break;
            }
            None => {
                gizmos.line(pos, next, arc);
                pos = next;
                vel = v;
            }
        }
    }
    // Marqueur d'arrivée : cercle = rayon d'explosion (la zone qui va sauter).
    if let Some(point) = impact {
        gizmos.circle(
            bevy::math::Isometry3d::new(
                point + Vec3::Y * 0.03,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            EXPLOSION_RADIUS,
            Color::srgb(0.25, 0.90, 0.18), // vert poison — zone d'explosion
        );
    }
}

/// Nouvelle run = compteurs à zéro.
fn sys_reset_stats(mut stats: ResMut<BoucherieStats>) {
    stats.reset_run();
}

/// Comptage des kills attribués RocketLauncher (multicast).
fn sys_count_kills(mut hits: MessageReader<CombatHitEvent>, mut stats: ResMut<BoucherieStats>) {
    for hit in hits.read() {
        if hit.weapon == Some(WeaponType::RocketLauncher) && hit.is_kill {
            stats.kills_run += 1;
        }
    }
}

/// Sensor `forgia2_boucherie.json` 1Hz — story-534 AC10.
fn sys_write_boucherie_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    stats: Res<BoucherieStats>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    let avg_hits = if stats.explosions_run > 0 {
        f64::from(stats.enemies_hit_run) / f64::from(stats.explosions_run)
    } else {
        0.0
    };
    let json = format!(
        r#"{{"id":"boucherie","severity":"ok","next_step":"","timestamp_secs":{:.1},"rockets_fired":{},"explosions_run":{},"enemies_hit_run":{},"avg_hits_per_explosion":{:.2},"kills_run":{}}}"#,
        time.elapsed_secs(),
        stats.rockets_fired,
        stats.explosions_run,
        stats.enemies_hit_run,
        avg_hits,
        stats.kills_run,
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[boucherie] sensor write failed: {e}");
    }
}

pub struct BoucherieRocketPlugin;

impl Plugin for BoucherieRocketPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoucherieStats>()
            .add_systems(OnEnter(GameMode::Roguelite), sys_reset_stats)
            .add_systems(
                Update,
                (sys_spawn_rockets, sys_fly_rockets, sys_count_kills)
                    .chain()
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(
                Update,
                sys_draw_trajectory_preview
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(Update, sys_write_boucherie_sensor.in_set(GameSet::Sensors));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_velocity_is_forward_times_speed_plus_lob() {
        let v = launch_velocity(Vec3::NEG_Z);
        assert!((v.z + ROCKET_SPEED).abs() < 1e-5);
        assert!((v.y - ROCKET_LOB_UP).abs() < 1e-5);
    }

    /// La parabole monte (lob) puis retombe — l'apex existe.
    #[test]
    fn integrate_is_parabolic() {
        let mut pos = Vec3::ZERO;
        let mut vel = launch_velocity(Vec3::NEG_Z);
        let mut max_y = f32::MIN;
        let mut went_down = false;
        for _ in 0..200 {
            let (p, v) = integrate(pos, vel, 1.0 / 60.0);
            pos = p;
            vel = v;
            if pos.y > max_y {
                max_y = pos.y;
            } else if pos.y < max_y - 0.5 {
                went_down = true;
            }
        }
        assert!(max_y > 0.1, "le lob monte d'abord (apex {max_y})");
        assert!(went_down, "puis retombe (gravité projectile)");
        assert!(pos.z < -30.0, "et avance vers l'avant ({})", pos.z);
    }

    #[test]
    fn stats_reset() {
        let mut s = BoucherieStats {
            rockets_fired: 5,
            explosions_run: 4,
            enemies_hit_run: 7,
            kills_run: 3,
        };
        s.reset_run();
        assert_eq!(s.rockets_fired, 0);
        assert_eq!(s.kills_run, 0);
    }
}
