//! status_vfx.rs — VFX hanabi continus des DoT (remplace le dot-pulse sphère).
//!
//! Flamme sur `StatusBurn`, nuage toxique sur `StatusPoison`. L'effet est une
//! entité hanabi **NON parentée** (même pattern que le muzzle flash, seul chemin
//! hanabi prouvé visible du repo), **repositionnée chaque frame** sur le
//! `GlobalTransform` de l'ennemi + offset `vfx.status_y` (genome, hot-reload).
//! Spawn sur `Added<Status*>`, despawn sur `RemovedComponents<Status*>`.
//!
//! Anti-occlusion (leçon 2026-06-24) : à `status_y` trop bas + rayon de spawn
//! étroit, les particules naissent DANS le mesh opaque du mob (capsule ~2 m,
//! rayon 0.4) et sont z-occluses. Fix : `status_y` ≈ haut du corps + rayon de
//! spawn large (cf `status.rs`, ~0.5) → particules à la surface/aux bords +
//! panache au-dessus = visibles.

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use forgia_core::prelude::*;
use forgia_effects::prelude::{ParticleEffect, WeaponVfxEffects};

use crate::element_vfx::ElementVfxStats;
use crate::elements::{ElementConfig, StatusBurn, StatusPoison};
use crate::enemies::{skeleton_scale, EnemyArchetype};

/// Cap concurrent d'auras de statut (au-delà, l'attach est sauté — les DÉGÂTS
/// DoT continuent, le visuel est découplé du gameplay). Miroir `MAX_ACTIVE_SPARKS`.
const MAX_STATUS_VFX: usize = 48;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusVfxKind {
    Burn,
    Poison,
}

/// Lien de l'entité VFX vers l'ennemi affligé (suivi + despawn ciblé).
#[derive(Component, Clone, Copy, Debug)]
pub struct StatusVfxLink {
    pub target: Entity,
    pub kind: StatusVfxKind,
    /// Facteur de taille du mob (`skeleton_scale`) → l'aura monte + s'élargit avec
    /// lui (sinon enfouie dans les gros : Tank ×1.4, Boss ×2.5).
    pub scale: f32,
}

/// Garde sur l'ennemi : empêche le re-attach tant que l'aura existe.
#[derive(Component)]
pub struct BurnVfxAttached;
#[derive(Component)]
pub struct PoisonVfxAttached;

/// Spawn une flamme hanabi (world-space) à la position de l'ennemi quand
/// `StatusBurn` apparaît. Le suivi est fait par `sys_follow_status_vfx`.
pub fn sys_attach_burn_vfx(
    mut commands: Commands,
    effects: Option<Res<WeaponVfxEffects>>,
    config: Res<ElementConfig>,
    q_new: Query<
        (Entity, &GlobalTransform, &EnemyArchetype),
        (Added<StatusBurn>, Without<BurnVfxAttached>),
    >,
    q_live: Query<(), With<StatusVfxLink>>,
    mut stats: ResMut<ElementVfxStats>,
) {
    let Some(effects) = effects else {
        return;
    };
    let y = config.vfx.status_y;
    let mut live = q_live.iter().count();
    for (enemy, gt, arch) in &q_new {
        // Cap atteint : ennemis au-delà sans aura cette frame (Added one-shot →
        // pas de retry). Tradeoff accepté : cap 48 >> ~12 brûlants typiques ; les
        // DÉGÂTS DoT ne sont JAMAIS affectés.
        if live >= MAX_STATUS_VFX {
            break;
        }
        let factor = skeleton_scale(*arch); // l'aura monte + s'élargit avec le mob
        commands.entity(enemy).insert(BurnVfxAttached);
        commands.spawn((
            ParticleEffect::new(effects.status_flame.clone()),
            Transform::from_translation(gt.translation() + Vec3::Y * (y * factor))
                .with_scale(Vec3::splat(factor)),
            StatusVfxLink { target: enemy, kind: StatusVfxKind::Burn, scale: factor },
            DespawnOnExit(GameMode::Roguelite),
        ));
        stats.dot_pulses = stats.dot_pulses.saturating_add(1);
        live += 1;
    }
}

/// Spawn un nuage toxique hanabi quand `StatusPoison` apparaît.
pub fn sys_attach_poison_vfx(
    mut commands: Commands,
    effects: Option<Res<WeaponVfxEffects>>,
    config: Res<ElementConfig>,
    q_new: Query<
        (Entity, &GlobalTransform, &EnemyArchetype),
        (Added<StatusPoison>, Without<PoisonVfxAttached>),
    >,
    q_live: Query<(), With<StatusVfxLink>>,
    mut stats: ResMut<ElementVfxStats>,
) {
    let Some(effects) = effects else {
        return;
    };
    let y = config.vfx.status_y;
    let mut live = q_live.iter().count();
    for (enemy, gt, arch) in &q_new {
        if live >= MAX_STATUS_VFX {
            break;
        }
        let factor = skeleton_scale(*arch);
        commands.entity(enemy).insert(PoisonVfxAttached);
        commands.spawn((
            ParticleEffect::new(effects.status_poison_cloud.clone()),
            Transform::from_translation(gt.translation() + Vec3::Y * (y * factor))
                .with_scale(Vec3::splat(factor)),
            StatusVfxLink { target: enemy, kind: StatusVfxKind::Poison, scale: factor },
            DespawnOnExit(GameMode::Roguelite),
        ));
        stats.dot_pulses = stats.dot_pulses.saturating_add(1);
        live += 1;
    }
}

/// Repositionne chaque aura sur le `GlobalTransform` de son ennemi + `status_y`,
/// chaque frame (l'aura n'est PAS parentée → suivi manuel ; `status_y` lu en
/// continu → la hauteur est hot-reloadable). Ennemi disparu → `sys_detach_*`
/// despawne l'aura sur `RemovedComponents`.
pub fn sys_follow_status_vfx(
    config: Res<ElementConfig>,
    q_targets: Query<&GlobalTransform, Without<StatusVfxLink>>,
    mut q_vfx: Query<(&mut Transform, &StatusVfxLink)>,
) {
    let y = config.vfx.status_y;
    for (mut tf, link) in &mut q_vfx {
        if let Ok(gt) = q_targets.get(link.target) {
            // scale (taille du mob) figée au spawn ; on ne touche que la position.
            tf.translation = gt.translation() + Vec3::Y * (y * link.scale);
        }
    }
}

/// Despawn la flamme quand `StatusBurn` est retiré (expiration ou mort).
pub fn sys_detach_burn_vfx(
    mut commands: Commands,
    mut removed: RemovedComponents<StatusBurn>,
    q_links: Query<(Entity, &StatusVfxLink)>,
) {
    for enemy in removed.read() {
        for (vfx, link) in &q_links {
            if link.target == enemy && link.kind == StatusVfxKind::Burn {
                if let Ok(mut e) = commands.get_entity(vfx) {
                    e.try_despawn();
                }
            }
        }
        if let Ok(mut e) = commands.get_entity(enemy) {
            e.remove::<BurnVfxAttached>();
        }
    }
}

/// Despawn le nuage quand `StatusPoison` est retiré (expiration ou mort).
pub fn sys_detach_poison_vfx(
    mut commands: Commands,
    mut removed: RemovedComponents<StatusPoison>,
    q_links: Query<(Entity, &StatusVfxLink)>,
) {
    for enemy in removed.read() {
        for (vfx, link) in &q_links {
            if link.target == enemy && link.kind == StatusVfxKind::Poison {
                if let Ok(mut e) = commands.get_entity(vfx) {
                    e.try_despawn();
                }
            }
        }
        if let Ok(mut e) = commands.get_entity(enemy) {
            e.remove::<PoisonVfxAttached>();
        }
    }
}
