//! ultimate_apply.rs — Branchement runtime des techniques d'Ultime (story-596 T3).
//!
//! Pendant l'État Ultime (`UltimateState::is_active`), chaque tir de l'arme
//! équipée applique sa **technique signature**, EN PLUS de l'élément passif
//! (lequel reste géré par `elements::sys_apply_elements_on_hit` — « passif
//! inchangé », design validé) :
//!
//! | Arme | Technique d'Ultime |
//! |------|--------------------|
//! | Pépin (ModernAR)      | Explosion AOE autour de l'impact |
//! | Bourrasque (AssaultR) | Chaîne électrique (rebonds de proche en proche) |
//! | Mme Lenoir (Shotgun)  | Perforation (couloir) + Poison sur les perforés |
//! | Pompe (RocketLauncher)| Gel de zone (immobilise les ennemis du rayon) |
//!
//! ## Décisions (cohérentes avec elements.rs)
//!
//! - Les dégâts secondaires (splash/chaîne/perforation) **mutent
//!   `forgia_combat::Health` directement** — JAMAIS un nouveau `CombatHitEvent`
//!   (sinon cascade ré-entrante : ce système relit ses propres events). La mort
//!   est déléguée à `despawn_dead_cubes` (forgia-fps).
//! - **Gel** : `StatusFreeze` est posé sur les bots, puis `sys_tick_ultimate_freeze`
//!   **pin** leur position (XZ) en `GameSet::Effects` APRÈS l'AI (même fenêtre que
//!   `shockwave::sys_apply_knockback`) → aucune dépendance de `forgia-ai-arena-bot`
//!   au mode. `slow_factor = 0` ⇒ immobilisation totale.
//! - 0 alloc hot path : deux buffers `Local` réutilisés (cand + résultats).
//! - Tunables/géométrie viennent de `ultimate_tech` (logique pure testée, T2).

use bevy::prelude::*;
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_combat::ultimate::UltimateState;
use forgia_combat::weapons::WeaponType;
use forgia_combat::Health;
use forgia_player::FpsCamera;
use std::fs;

use crate::elements::{ElementConfig, StatusPoison};
use crate::enemies::EnemyArchetype;
use crate::ultimate_tech::{
    chain_targets, freeze_secs_after_tick, frozen_speed_factor, pierce_targets,
    ULTIMATE_CHAIN_DAMAGE_FACTOR, ULTIMATE_CHAIN_HOP_RADIUS, ULTIMATE_CHAIN_MAX_EXTRA,
    ULTIMATE_EXPLOSION_DAMAGE_FACTOR, ULTIMATE_EXPLOSION_RADIUS, ULTIMATE_FREEZE_AOE_RADIUS,
    ULTIMATE_FREEZE_SECS, ULTIMATE_FREEZE_SLOW, ULTIMATE_PIERCE_DAMAGE_FACTOR,
    ULTIMATE_PIERCE_HALF_WIDTH, ULTIMATE_PIERCE_RANGE,
};

const SENSOR_PATH: &str = "forgia2_ultimate_tech.json";
const SENSOR_PERIOD_SECS: f32 = 1.0;

/// Gel posé par l'Ultime Pompe. Tant que `secs_left > 0` et `slow_factor == 0`,
/// `sys_tick_ultimate_freeze` rétablit la position `pin` (immobilisation).
#[derive(Component, Debug, Clone, Copy)]
pub struct StatusFreeze {
    pub secs_left: f32,
    pub slow_factor: f32,
    pub pin: Vec3,
}

/// Compteurs sensor des techniques d'Ultime (diagnostic sans rendu — T3).
#[derive(Resource, Default, Debug, Clone)]
pub struct UltimateTechStats {
    pub explosions: u32,
    pub explosion_hits: u32,
    pub chains: u32,
    pub chain_hits: u32,
    pub pierces: u32,
    pub pierce_hits: u32,
    pub poisons: u32,
    pub freezes: u32,
    pub freeze_hits: u32,
}

/// Applique la technique d'Ultime de l'arme à chaque `CombatHitEvent`, tant que
/// l'Ultime est actif. Hors Ultime : draine les events (curseur à jour) et sort.
#[allow(clippy::too_many_arguments)]
pub fn sys_apply_ultimate_technique(
    mut events: MessageReader<CombatHitEvent>,
    ult: Res<UltimateState>,
    config: Res<ElementConfig>,
    mut commands: Commands,
    mut stats: ResMut<UltimateTechStats>,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_pos: Query<(Entity, &GlobalTransform), With<EnemyArchetype>>,
    mut q_health: Query<&mut Health, With<EnemyArchetype>>,
    mut q_poison: Query<&mut StatusPoison>,
    mut cand: Local<Vec<(Entity, Vec3)>>,
    mut hits: Local<Vec<Entity>>,
) {
    if !ult.is_active() {
        // Draine le curseur (évite de retraiter de vieux hits quand l'Ultime
        // s'active) sans dépendre de l'API `clear()` (variable selon version).
        events.read().for_each(drop);
        return;
    }

    // Forward caméra (plan XZ) pour la perforation de Lenoir.
    let cam_fwd = q_cam
        .single()
        .ok()
        .map(|gt| {
            let f = gt.forward();
            Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
        })
        .filter(|v| *v != Vec3::ZERO)
        .unwrap_or(Vec3::NEG_Z);

    for ev in events.read() {
        let Some(weapon) = ev.weapon else {
            continue;
        };
        let origin = ev.hit_world_pos;

        match weapon {
            // ── Pépin : explosion AOE ─────────────────────────────────────
            WeaponType::ModernAR => {
                let r2 = ULTIMATE_EXPLOSION_RADIUS * ULTIMATE_EXPLOSION_RADIUS;
                let dmg = (ev.damage * ULTIMATE_EXPLOSION_DAMAGE_FACTOR).max(0.0);
                hits.clear();
                hits.extend(q_pos.iter().filter_map(|(e, gt)| {
                    (e != ev.target && (gt.translation() - origin).length_squared() <= r2)
                        .then_some(e)
                }));
                let mut n = 0u32;
                for &e in &*hits {
                    if let Ok(mut hp) = q_health.get_mut(e) {
                        hp.current = (hp.current - dmg).max(0.0);
                        n += 1;
                    }
                }
                stats.explosions = stats.explosions.saturating_add(1);
                stats.explosion_hits = stats.explosion_hits.saturating_add(n);
            }

            // ── Bourrasque : chaîne électrique ────────────────────────────
            WeaponType::AssaultRifle => {
                cand.clear();
                cand.extend(
                    q_pos
                        .iter()
                        .filter(|&(e, _)| e != ev.target)
                        .map(|(e, gt)| (e, gt.translation())),
                );
                chain_targets(
                    origin,
                    &cand,
                    ULTIMATE_CHAIN_MAX_EXTRA,
                    ULTIMATE_CHAIN_HOP_RADIUS,
                    &mut hits,
                );
                let dmg = (ev.damage * ULTIMATE_CHAIN_DAMAGE_FACTOR).max(0.0);
                let mut n = 0u32;
                for &e in &*hits {
                    if let Ok(mut hp) = q_health.get_mut(e) {
                        hp.current = (hp.current - dmg).max(0.0);
                        n += 1;
                    }
                }
                stats.chains = stats.chains.saturating_add(1);
                stats.chain_hits = stats.chain_hits.saturating_add(n);
            }

            // ── Mme Lenoir : perforation + poison ─────────────────────────
            WeaponType::Shotgun => {
                cand.clear();
                cand.extend(
                    q_pos
                        .iter()
                        .filter(|&(e, _)| e != ev.target)
                        .map(|(e, gt)| (e, gt.translation())),
                );
                pierce_targets(
                    origin,
                    cam_fwd,
                    ULTIMATE_PIERCE_RANGE,
                    ULTIMATE_PIERCE_HALF_WIDTH,
                    &cand,
                    &mut hits,
                );
                let dmg = (ev.damage * ULTIMATE_PIERCE_DAMAGE_FACTOR).max(0.0);
                let mut n = 0u32;
                let mut poisoned = 0u32;
                for &e in &*hits {
                    if let Ok(mut hp) = q_health.get_mut(e) {
                        hp.current = (hp.current - dmg).max(0.0);
                        n += 1;
                    }
                    // Poison (réutilise les params genome de l'élément).
                    if let Ok(mut p) = q_poison.get_mut(e) {
                        p.stacks = (p.stacks + 1).min(config.poison.max_stacks);
                        p.secs_left = config.poison.duration;
                        p.dps_per_stack = config.poison.dps_per_stack;
                    } else {
                        commands.entity(e).try_insert(StatusPoison {
                            stacks: 1,
                            dps_per_stack: config.poison.dps_per_stack,
                            secs_left: config.poison.duration,
                            tick_accum: 0.0,
                        });
                    }
                    poisoned += 1;
                }
                stats.pierces = stats.pierces.saturating_add(1);
                stats.pierce_hits = stats.pierce_hits.saturating_add(n);
                stats.poisons = stats.poisons.saturating_add(poisoned);
            }

            // ── Pompe : gel de zone ───────────────────────────────────────
            WeaponType::RocketLauncher => {
                let r2 = ULTIMATE_FREEZE_AOE_RADIUS * ULTIMATE_FREEZE_AOE_RADIUS;
                let mut n = 0u32;
                for (e, gt) in q_pos.iter() {
                    let pos = gt.translation();
                    if (pos - origin).length_squared() <= r2 {
                        commands.entity(e).try_insert(StatusFreeze {
                            secs_left: ULTIMATE_FREEZE_SECS,
                            slow_factor: ULTIMATE_FREEZE_SLOW,
                            pin: pos,
                        });
                        n += 1;
                    }
                }
                stats.freezes = stats.freezes.saturating_add(1);
                stats.freeze_hits = stats.freeze_hits.saturating_add(n);
            }

            _ => {}
        }
    }
}

/// Décompte le gel et **pin** la position des bots gelés (XZ) APRÈS l'AI.
/// `GameSet::Effects`, comme `shockwave::sys_apply_knockback`. Retire le component
/// à expiration. Y préservé (compatible pop/knockback).
pub fn sys_tick_ultimate_freeze(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut StatusFreeze)>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut fr) in &mut q {
        fr.secs_left = freeze_secs_after_tick(fr.secs_left, dt);
        // slow_factor == 0 → immobilisation : on rétablit la position d'origine.
        if frozen_speed_factor(fr.secs_left, fr.slow_factor) <= 0.0 {
            tf.translation.x = fr.pin.x;
            tf.translation.z = fr.pin.z;
        }
        if fr.secs_left <= 0.0 {
            commands.entity(e).try_remove::<StatusFreeze>();
        }
    }
}

/// OnEnter Roguelite — reset des compteurs sensor (run fraîche).
pub fn sys_reset_ultimate_tech_stats(mut stats: ResMut<UltimateTechStats>) {
    *stats = UltimateTechStats::default();
}

/// Écrit `forgia2_ultimate_tech.json` 1Hz : état Ultime + compteurs par technique
/// + gels actifs. Point de validation T3 (diagnostic sans rendu).
pub fn sys_write_ultimate_tech_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    ult: Res<UltimateState>,
    stats: Res<UltimateTechStats>,
    q_freeze: Query<(), With<StatusFreeze>>,
) {
    *accum += time.delta_secs();
    if *accum < SENSOR_PERIOD_SECS {
        return;
    }
    *accum = 0.0;

    let active_freezes = q_freeze.iter().count();
    let json = format!(
        r#"{{"id":"ultimate_tech","severity":"ok","next_step":"","ultimate_active":{},"explosions":{},"explosion_hits":{},"chains":{},"chain_hits":{},"pierces":{},"pierce_hits":{},"poisons":{},"freezes":{},"freeze_hits":{},"active_freezes":{active_freezes}}}"#,
        ult.is_active(),
        stats.explosions,
        stats.explosion_hits,
        stats.chains,
        stats.chain_hits,
        stats.pierces,
        stats.pierce_hits,
        stats.poisons,
        stats.freezes,
        stats.freeze_hits,
    );

    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[ultimate_tech] sensor write failed: {e}");
    }
}
