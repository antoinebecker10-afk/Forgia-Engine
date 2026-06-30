//! shockwave.rs — Sorts F PAR ARME parlante (story-572/573).
//!
//! Le F est dispatché selon `EquippedWeapons.current` (forgia-combat) — chaque
//! arme parlante a SON sort = différenciateur identité Forgia :
//! - **Pépin** (ModernAR)      : Câlin — heal joueur + push doux, 0 dégât (défensif)
//! - **Bourrasque** (AssaultR.) : Coup de Bourrasque — repousse fort + pop (gust)
//! - **Mme Lenoir** (Shotgun=SNIPER) : Tir Perçant — rayon précision qui traverse
//!   tous les ennemis alignés (longue portée, gros dégâts)
//! - **Boucherie** (Rocket)     : Boum-Bidoche — AOE dirigé au point visé + pop vertical
//!
//! Cooldown PAR ARME (HashMap<WeaponType,f32>). Dégâts : mutation directe de
//! `forgia_combat::Health` (PAS forgia_damage::Health — cf piège mémoire) +
//! `CombatHitEvent` (chiffres/flash/killfeed/kill_popup/boons), mort déléguée à
//! `forgia-fps::despawn_dead_cubes`. JAMAIS `DamageEvent` (double DeathEvent).
//!
//! Constantes V1 — à externaliser genome (story-566).

use bevy::prelude::*;
use forgia_ai_arena_bot::ArenaBot;
use forgia_combat::combat_juice::{CameraTrauma, CombatHitEvent};
use forgia_combat::ultimate::UltimateState;
use forgia_combat::weapons::{EquippedWeapons, WeaponType};
use forgia_combat::Health;
use forgia_core::prelude::*;
use forgia_damage::HitZone;
use forgia_player::{FpsCamera, Player};
use std::collections::HashMap;

use crate::run::RunState;

// ─── Constantes par sort (V1 — genome story-566) ───────────────────────────
// Pépin (ModernAR) — Câlin : défensif.
const CALIN_CD: f32 = 12.0;
const CALIN_HEAL_FRAC: f32 = 0.35;
const CALIN_PUSH_RADIUS: f32 = 6.0;
const CALIN_PUSH_SPEED: f32 = 8.0;
// Bourrasque (AssaultRifle) — Gust : repousse.
const GUST_CD: f32 = 7.0;
const GUST_RADIUS: f32 = 12.0;
const GUST_DAMAGE: f32 = 25.0;
const GUST_PUSH_SPEED: f32 = 22.0;
const GUST_POP: f32 = 1.5; // hauteur du saut (m), retombe au sol
// Mme Lenoir (Shotgun = SNIPER) — Tir Perçant : rayon ligne.
const PIERCE_CD: f32 = 9.0;
const PIERCE_RANGE: f32 = 30.0;
const PIERCE_HALF_WIDTH: f32 = 1.5;
const PIERCE_DAMAGE: f32 = 60.0;
// Boucherie (RocketLauncher) — Boum : AOE dirigé.
const BOUM_CD: f32 = 10.0;
const BOUM_RANGE: f32 = 14.0;
const BOUM_RADIUS: f32 = 6.0;
const BOUM_DAMAGE: f32 = 70.0;
const BOUM_PUSH_SPEED: f32 = 10.0;
const BOUM_POP: f32 = 2.5; // hauteur du saut (m), retombe au sol

const KNOCKBACK_DURATION: f32 = 0.32;
const VFX_DURATION: f32 = 0.40;
/// Offset centre-capsule → pieds. Le Player spawn `capsule_y(0.7, 0.3)` et son
/// Transform est au CENTRE de la capsule → pieds = centre - (0.7 + 0.3) = -1.0.
/// Les VFX au sol retranchent ça (sinon flottent), le rayon part des yeux (caméra).
const PLAYER_GROUND_OFFSET: f32 = 1.0;

/// Cooldown du F PAR ARME + compteur de casts (sensor).
#[derive(Resource, Default, Debug)]
pub struct ShockwaveAbility {
    pub cooldowns: HashMap<WeaponType, f32>,
    pub casts_total: u32,
}

/// Cooldown nominal d'un sort (pour le radial HUD).
pub fn spell_max_cooldown(w: WeaponType) -> f32 {
    match w {
        WeaponType::ModernAR => CALIN_CD,
        WeaponType::AssaultRifle => GUST_CD,
        WeaponType::Shotgun => PIERCE_CD,
        WeaponType::RocketLauncher => BOUM_CD,
        _ => GUST_CD,
    }
}

/// Repoussement appliqué à un ennemi touché (push horizontal + pop vertical en
/// arc). Poussé par `sys_apply_knockback` (GameSet::Effects, APRÈS l'AI → gagne
/// sur le Transform des bots KinematicPositionBased).
#[derive(Component)]
pub struct Knockback {
    pub vel: Vec3,       // vélocité HORIZONTALE
    pub time_left: f32,
    pub total: f32,      // durée totale (arc pop + décélération horizontale)
    pub pop_height: f32, // hauteur du saut (0 = pas de pop) — arc qui retombe
    pub ground_y: f32,   // y de repos à restaurer en fin de pop
}

/// VFX disque/rayon (scale interpolé start→end + fade + despawn).
#[derive(Component)]
pub struct ShockwaveVfx {
    pub age: f32,
    pub mat: Handle<StandardMaterial>,
    pub start_scale: f32,
    pub end_scale: f32,
    pub base_color: Srgba,
}

// ─── Cooldown tick ─────────────────────────────────────────────────────────

pub fn sys_tick_shockwave_cooldown(time: Res<Time>, mut ab: ResMut<ShockwaveAbility>) {
    let dt = time.delta_secs();
    for v in ab.cooldowns.values_mut() {
        *v = (*v - dt).max(0.0);
    }
}

// ─── Input F : dispatch par arme ───────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn sys_shockwave_input(
    keys: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppMode>>,
    run_state: Option<Res<State<RunState>>>,
    mut ab: ResMut<ShockwaveAbility>,
    mut ult: ResMut<UltimateState>,
    q_player: Query<(Entity, &Transform), With<Player>>,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_bots: Query<(Entity, &Transform), With<ArenaBot>>,
    mut q_health: Query<&mut Health>,
    equipped: Option<Res<EquippedWeapons>>,
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
    if !in_run || !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    // T1b (story-596) — F ouvre l'État Ultime 10s (gaté par SON cooldown propre,
    // indépendant du cooldown par-arme du sort d'ouverture ci-dessous). Pendant la
    // fenêtre active, T3 branchera la technique signature par-tir. Le sort instantané
    // reste pour l'instant le "burst d'ouverture" — sa fusion/retrait = T3.
    if ult.try_activate() {
        info!(
            "[roguelite] ULTIME activé — {}s (cooldown {}s)",
            ult.duration as u32, ult.cooldown as u32
        );
    }
    let current = equipped.as_ref().map(|e| e.current).unwrap_or_default();
    if ab.cooldowns.get(&current).copied().unwrap_or(0.0) > 0.0 {
        return;
    }
    let Ok((player_e, player_tf)) = q_player.single() else {
        return;
    };
    let origin = player_tf.translation;
    // Player Transform = CENTRE capsule → sol = origin.y - PLAYER_GROUND_OFFSET.
    let ground_y = origin.y - PLAYER_GROUND_OFFSET;
    let ground_center = Vec3::new(origin.x, ground_y, origin.z);
    // Caméra (yeux) : position pour le rayon + forward horizontal pour viser.
    let (cam_pos, fwd_xz) = q_cam
        .single()
        .ok()
        .map(|gt| {
            let f = gt.forward();
            let fxz = Vec3::new(f.x, 0.0, f.z).normalize_or_zero();
            let fxz = if fxz == Vec3::ZERO { Vec3::NEG_Z } else { fxz };
            (gt.translation(), fxz)
        })
        .unwrap_or((origin, Vec3::NEG_Z));

    let spell_cd = match current {
        // ── Pépin : Câlin (heal + push doux, 0 dégât) ─────────────────────
        WeaponType::ModernAR => {
            commands.queue(|world: &mut World| {
                let mut q = world.query_filtered::<&mut forgia_damage::Health, With<Player>>();
                if let Ok(mut hp) = q.single_mut(world) {
                    hp.current = (hp.current + hp.max * CALIN_HEAL_FRAC).min(hp.max);
                }
            });
            let (aff, tot) = aoe_strike(
                origin, CALIN_PUSH_RADIUS, 0.0, origin, CALIN_PUSH_SPEED, 0.0,
                Some(current), player_e, &q_bots, &mut q_health, &mut hits_w, &mut commands,
            );
            spawn_disc_vfx(
                &mut commands, &mut meshes, &mut materials, ground_center, 0.5, CALIN_PUSH_RADIUS,
                Srgba::new(0.45, 0.85, 0.40, 0.55), LinearRgba::rgb(0.3, 1.2, 0.4),
            );
            trauma.add(0.2);
            info!(
                "[roguelite] Câlin de Pépin — heal +{}% + push {aff}/{tot} bots",
                (CALIN_HEAL_FRAC * 100.0) as u32
            );
            CALIN_CD
        }
        // ── Bourrasque : repousse (gust) ──────────────────────────────────
        WeaponType::AssaultRifle => {
            let (aff, tot) = aoe_strike(
                origin, GUST_RADIUS, GUST_DAMAGE, origin, GUST_PUSH_SPEED, GUST_POP,
                Some(current), player_e, &q_bots, &mut q_health, &mut hits_w, &mut commands,
            );
            spawn_disc_vfx(
                &mut commands, &mut meshes, &mut materials, ground_center, 0.5, GUST_RADIUS,
                Srgba::new(0.43, 0.71, 0.94, 0.85), LinearRgba::rgb(0.6, 1.2, 2.0),
            );
            trauma.add(0.4);
            info!("[roguelite] Coup de Bourrasque — repousse {aff}/{tot} bots ({GUST_RADIUS}m)");
            GUST_CD
        }
        // ── Mme Lenoir (sniper) : Tir Perçant (ligne) ─────────────────────
        WeaponType::Shotgun => {
            let (hit, tot) = line_strike(
                origin, fwd_xz, PIERCE_RANGE, PIERCE_HALF_WIDTH, PIERCE_DAMAGE,
                Some(current), player_e, &q_bots, &mut q_health, &mut hits_w,
            );
            spawn_beam_vfx(
                &mut commands, &mut meshes, &mut materials,
                Vec3::new(origin.x, cam_pos.y, origin.z), fwd_xz, PIERCE_RANGE,
                PIERCE_HALF_WIDTH, Srgba::new(0.71, 0.51, 0.86, 0.85), LinearRgba::rgb(1.4, 0.6, 2.0),
            );
            trauma.add(0.45);
            info!("[roguelite] Tir Perçant — {hit}/{tot} alignés touchés (ligne {PIERCE_RANGE}m, large {PIERCE_HALF_WIDTH}m)");
            PIERCE_CD
        }
        // ── Boucherie : Boum dirigé + pop vertical ────────────────────────
        WeaponType::RocketLauncher => {
            let impact = Vec3::new(
                origin.x + fwd_xz.x * BOUM_RANGE,
                ground_y,
                origin.z + fwd_xz.z * BOUM_RANGE,
            );
            let (aff, tot) = aoe_strike(
                impact, BOUM_RADIUS, BOUM_DAMAGE, impact, BOUM_PUSH_SPEED, BOUM_POP,
                Some(current), player_e, &q_bots, &mut q_health, &mut hits_w, &mut commands,
            );
            spawn_disc_vfx(
                &mut commands, &mut meshes, &mut materials, impact, 0.5, BOUM_RADIUS,
                Srgba::new(0.90, 0.30, 0.20, 0.90), LinearRgba::rgb(3.0, 0.6, 0.3),
            );
            trauma.add(0.7);
            info!("[roguelite] Boum-Bidoche — {aff}/{tot} touchés (impact {BOUM_RANGE}m devant)");
            BOUM_CD
        }
        // ── Fallback (AK47/Plasma/Chainsaw hors set V1) : gust ────────────
        _ => {
            aoe_strike(
                origin, GUST_RADIUS, GUST_DAMAGE, origin, GUST_PUSH_SPEED, GUST_POP,
                Some(current), player_e, &q_bots, &mut q_health, &mut hits_w, &mut commands,
            );
            spawn_disc_vfx(
                &mut commands, &mut meshes, &mut materials, ground_center, 0.5, GUST_RADIUS,
                Srgba::new(0.43, 0.71, 0.94, 0.85), LinearRgba::rgb(0.6, 1.2, 2.0),
            );
            trauma.add(0.4);
            GUST_CD
        }
    };

    ab.cooldowns.insert(current, spell_cd);
    ab.casts_total = ab.casts_total.saturating_add(1);
}

/// AOE radiale autour de `center` (rayon XZ) : push depuis `push_from` (avec pop
/// vertical en arc) et dégâts positifs via CombatHitEvent pour chaque ennemi.
/// Renvoie `(affected, total)` pour diagnostic (affected = bots dans le rayon).
/// `weapon` = attribution du hit (barks de kill persona + killfeed + sensors).
#[allow(clippy::too_many_arguments)]
fn aoe_strike(
    center: Vec3,
    radius: f32,
    damage: f32,
    push_from: Vec3,
    push_speed: f32,
    push_pop: f32,
    weapon: Option<WeaponType>,
    player_e: Entity,
    q_bots: &Query<(Entity, &Transform), With<ArenaBot>>,
    q_health: &mut Query<&mut Health>,
    hits_w: &mut MessageWriter<CombatHitEvent>,
    commands: &mut Commands,
) -> (u32, u32) {
    let r2 = radius * radius;
    let mut total = 0u32;
    let mut affected = 0u32;
    for (e, tf) in q_bots {
        total += 1;
        let dx = tf.translation.x - center.x;
        let dz = tf.translation.z - center.z;
        if dx * dx + dz * dz > r2 {
            continue;
        }
        affected += 1;
        // Push depuis push_from (+ pop vertical en arc).
        let mut dir = Vec3::new(tf.translation.x - push_from.x, 0.0, tf.translation.z - push_from.z)
            .normalize_or_zero();
        if dir == Vec3::ZERO {
            dir = Vec3::Z;
        }
        commands.entity(e).insert(Knockback {
            vel: dir * push_speed,
            time_left: KNOCKBACK_DURATION,
            total: KNOCKBACK_DURATION,
            pop_height: push_pop,
            ground_y: tf.translation.y,
        });
        deal_damage(e, tf.translation, damage, weapon, player_e, q_health, hits_w);
    }
    (affected, total)
}

/// Tir en LIGNE (sniper) : dégâts à tous les ennemis dans un couloir devant le
/// joueur (longueur `range`, demi-largeur `half_width`). Pas de knockback
/// (précision pure). Traverse tous les ennemis alignés.
#[allow(clippy::too_many_arguments)]
fn line_strike(
    origin: Vec3,
    fwd_xz: Vec3,
    range: f32,
    half_width: f32,
    damage: f32,
    weapon: Option<WeaponType>,
    player_e: Entity,
    q_bots: &Query<(Entity, &Transform), With<ArenaBot>>,
    q_health: &mut Query<&mut Health>,
    hits_w: &mut MessageWriter<CombatHitEvent>,
) -> (u32, u32) {
    // Perpendiculaire au forward dans le plan XZ (rotation -90°).
    let perp = Vec3::new(fwd_xz.z, 0.0, -fwd_xz.x);
    let mut total = 0u32;
    let mut hit = 0u32;
    for (e, tf) in q_bots {
        total += 1;
        let rel = Vec3::new(tf.translation.x - origin.x, 0.0, tf.translation.z - origin.z);
        let along = rel.dot(fwd_xz);
        if along < 0.0 || along > range {
            continue;
        }
        if rel.dot(perp).abs() > half_width {
            continue;
        }
        hit += 1;
        deal_damage(e, tf.translation, damage, weapon, player_e, q_health, hits_w);
    }
    (hit, total)
}

/// Applique des dégâts à un bot + émet `CombatHitEvent` (chiffres/flash/boons).
/// Mort déléguée à `despawn_dead_cubes`. Skip si damage == 0 (sorts non-létaux).
/// `weapon` attribue le hit (barks de kill persona + killfeed + sensors).
fn deal_damage(
    e: Entity,
    bot_pos: Vec3,
    damage: f32,
    weapon: Option<WeaponType>,
    player_e: Entity,
    q_health: &mut Query<&mut Health>,
    hits_w: &mut MessageWriter<CombatHitEvent>,
) {
    if damage <= 0.0 {
        return;
    }
    if let Ok(mut hp) = q_health.get_mut(e) {
        if !hp.is_dead() {
            hp.current = (hp.current - damage).max(0.0);
            let dead = hp.is_dead();
            hits_w.write(CombatHitEvent {
                target: e,
                attacker: Some(player_e),
                damage,
                is_kill: dead,
                is_headshot: false,
                hit_world_pos: bot_pos + Vec3::Y * 1.2,
                weapon,
                body_zone: HitZone::Body,
            });
        }
    }
}

/// Spawn d'un disque VFX au sol (anime par sys_animate_shockwave_vfx, scale).
/// pub(crate) : réutilisé par l'explosion roquette (boucherie_rocket.rs).
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_disc_vfx(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
    start_scale: f32,
    end_scale: f32,
    base_color: Srgba,
    emissive: LinearRgba,
) {
    let mat = materials.add(StandardMaterial {
        base_color: base_color.into(),
        emissive,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let mesh = meshes.add(Cylinder::new(1.0, 0.06));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(center + Vec3::Y * 0.1)
            .with_scale(Vec3::new(start_scale, 1.0, start_scale)),
        ShockwaveVfx { age: 0.0, mat, start_scale, end_scale, base_color },
    ));
}

/// Spawn d'un RAYON VFX (sniper) : box fine orientée le long du forward, du
/// joueur jusqu'à `range`. Pas d'anim de scale (start=end=1) → seul l'alpha fade.
#[allow(clippy::too_many_arguments)]
fn spawn_beam_vfx(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    origin: Vec3,
    fwd_xz: Vec3,
    range: f32,
    half_width: f32,
    base_color: Srgba,
    emissive: LinearRgba,
) {
    let mat = materials.add(StandardMaterial {
        base_color: base_color.into(),
        emissive,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    // Box : longueur le long de l'axe X local, fine en Y/Z.
    let mesh = meshes.add(Cuboid::new(range, 0.4, half_width * 2.0));
    // `origin` est déjà à la hauteur des yeux (caméra) → pas d'offset Y ajouté.
    let mid = origin + fwd_xz * (range * 0.5);
    // Aligner +X local sur fwd_xz : θ = atan2(-fwd.z, fwd.x).
    let angle = (-fwd_xz.z).atan2(fwd_xz.x);
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(mid).with_rotation(Quat::from_rotation_y(angle)),
        ShockwaveVfx { age: 0.0, mat, start_scale: 1.0, end_scale: 1.0, base_color },
    ));
}

/// Anime la VFX : scale interpolé start→end + fade alpha + despawn.
pub fn sys_animate_shockwave_vfx(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(Entity, &mut Transform, &mut ShockwaveVfx)>,
) {
    for (e, mut tf, mut vfx) in &mut q {
        vfx.age += time.delta_secs();
        let t = (vfx.age / VFX_DURATION).clamp(0.0, 1.0);
        let scale = vfx.start_scale + (vfx.end_scale - vfx.start_scale) * t;
        // Le rayon (start==end==1) garde son scale 1 (sa taille est dans le mesh +
        // sa rotation dans le Transform) ; les disques scalent uniformément.
        if (vfx.start_scale - vfx.end_scale).abs() < f32::EPSILON {
            tf.scale = Vec3::ONE; // rayon : pas de déformation
        } else {
            tf.scale = Vec3::new(scale, 1.0, scale);
        }
        if let Some(m) = materials.get_mut(&vfx.mat) {
            let c = vfx.base_color;
            m.base_color = Color::srgba(c.red, c.green, c.blue, c.alpha * (1.0 - t));
        }
        if vfx.age >= VFX_DURATION {
            commands.entity(e).despawn();
        }
    }
}

/// Applique le knockback (GameSet::Effects → après l'AI Movement). Décélère via
/// la fraction restante ; pop vertical en arc qui RETOMBE au sol (bots kinematic
/// = pas de gravité → on gère la parabole nous-mêmes).
pub fn sys_apply_knockback(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut Knockback)>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut kb) in &mut q {
        let frac = (kb.time_left / kb.total).clamp(0.0, 1.0);
        tf.translation.x += kb.vel.x * dt * frac;
        tf.translation.z += kb.vel.z * dt * frac;
        if kb.pop_height > 0.0 {
            let prog = ((kb.total - kb.time_left) / kb.total).clamp(0.0, 1.0);
            tf.translation.y = kb.ground_y + kb.pop_height * (std::f32::consts::PI * prog).sin();
        }
        kb.time_left -= dt;
        if kb.time_left <= 0.0 {
            if kb.pop_height > 0.0 {
                tf.translation.y = kb.ground_y; // atterrissage net
            }
            commands.entity(e).remove::<Knockback>();
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spell_cooldowns_distinct_and_sane() {
        assert_eq!(spell_max_cooldown(WeaponType::ModernAR), CALIN_CD);
        assert_eq!(spell_max_cooldown(WeaponType::AssaultRifle), GUST_CD);
        assert_eq!(spell_max_cooldown(WeaponType::Shotgun), PIERCE_CD);
        assert_eq!(spell_max_cooldown(WeaponType::RocketLauncher), BOUM_CD);
        assert!(BOUM_CD >= GUST_CD);
    }

    #[test]
    fn ability_default_empty() {
        let a = ShockwaveAbility::default();
        assert!(a.cooldowns.is_empty());
        assert_eq!(a.casts_total, 0);
    }
}
