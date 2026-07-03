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
use forgia_effects::prelude::{EffectMaterial, ParticleEffect, WeaponVfxEffects};
use forgia_enemy_nameplate::{EnemyNameplate, NameplateRegistry};

use crate::element_vfx::ElementVfxStats;
use crate::elements::{Element, ElementConfig, StatusBurn, StatusPoison};
// Story-653 — la « marque électrique » runtime est `forgia_damage::Vulnerability`
// (posée/retirée par elements.rs sur hit Shock ; `ShockParams` = config genome).
use forgia_damage::Vulnerability;
use crate::enemies::{skeleton_scale, EnemyArchetype};

/// Cap concurrent d'auras de statut (au-delà, l'attach est sauté — les DÉGÂTS
/// DoT continuent, le visuel est découplé du gameplay). Miroir `MAX_ACTIVE_SPARKS`.
const MAX_STATUS_VFX: usize = 48;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusVfxKind {
    Burn,
    Poison,
    /// Story-653 — arcs électriques (StatusShock, identité Pépin).
    Shock,
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
#[derive(Component)]
pub struct ShockVfxAttached;

/// Story-654 — icône de statut (feu/poison/élec) à côté de la barre HP du
/// nameplate (langage WoW/Gunfire : le debuff se LIT sur la barre, l'aura sur
/// le corps n'est que l'ambiance). Enfant du nameplate root → billboard/despawn
/// avec lui ; despawn ciblé au retrait du statut (sys_detach_*).
#[derive(Component)]
pub struct StatusIcon {
    pub target: Entity,
    pub kind: StatusVfxKind,
}

/// Slot horizontal de l'icône à droite de la barre (feu=0, poison=1, élec=2).
fn icon_slot(kind: StatusVfxKind) -> f32 {
    match kind {
        StatusVfxKind::Burn => 0.0,
        StatusVfxKind::Poison => 1.0,
        StatusVfxKind::Shock => 2.0,
    }
}

/// Spawn l'icône de statut accrochée au nameplate de `enemy` (skip si pas de
/// nameplate — avec les nameplates permanents, il existe dès le spawn).
#[allow(clippy::too_many_arguments)]
fn spawn_status_icon(
    commands: &mut Commands,
    registry: &NameplateRegistry,
    nameplate: &EnemyNameplate,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    texture: Handle<Image>,
    tint: [f32; 3],
    enemy: Entity,
    kind: StatusVfxKind,
) {
    let Some(&root) = registry.map.get(&enemy) else {
        return;
    };
    let t = &nameplate.0;
    let icon = t.height * 1.4;
    let x = t.width / 2.0 + t.height * (1.0 + icon_slot(kind) * 1.7);
    let mesh = meshes.add(Rectangle::new(icon, icon));
    let mat = materials.add(StandardMaterial {
        base_color: Color::linear_rgb(tint[0], tint[1], tint[2]),
        base_color_texture: Some(texture),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        StatusIcon { target: enemy, kind },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_xyz(x, 0.0, 0.0),
        ChildOf(root),
        Name::new("StatusIcon"),
    ));
}

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
    // Story-654 — icône de statut sur le nameplate.
    registry: Res<NameplateRegistry>,
    nameplate: Res<EnemyNameplate>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
            // Story-647 : texture léchure de flamme (slot "color" de l'EffectAsset).
            EffectMaterial { images: vec![effects.tex_flame.clone()] },
            Transform::from_translation(gt.translation() + Vec3::Y * (y * factor))
                .with_scale(Vec3::splat(factor)),
            StatusVfxLink { target: enemy, kind: StatusVfxKind::Burn, scale: factor },
            DespawnOnExit(GameMode::Roguelite),
        ));
        spawn_status_icon(
            &mut commands,
            &registry,
            &nameplate,
            &mut meshes,
            &mut materials,
            effects.tex_flame.clone(),
            Element::Fire.rgb(&config.vfx),
            enemy,
            StatusVfxKind::Burn,
        );
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
    // Story-654 — icône de statut sur le nameplate.
    registry: Res<NameplateRegistry>,
    nameplate: Res<EnemyNameplate>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
            // Story-647 : texture volutes (slot "color" de l'EffectAsset).
            EffectMaterial { images: vec![effects.tex_poison.clone()] },
            Transform::from_translation(gt.translation() + Vec3::Y * (y * factor))
                .with_scale(Vec3::splat(factor)),
            StatusVfxLink { target: enemy, kind: StatusVfxKind::Poison, scale: factor },
            DespawnOnExit(GameMode::Roguelite),
        ));
        spawn_status_icon(
            &mut commands,
            &registry,
            &nameplate,
            &mut meshes,
            &mut materials,
            effects.tex_poison.clone(),
            Element::Poison.rgb(&config.vfx),
            enemy,
            StatusVfxKind::Poison,
        );
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
    q_icons: Query<(Entity, &StatusIcon)>,
) {
    for enemy in removed.read() {
        for (vfx, link) in &q_links {
            if link.target == enemy && link.kind == StatusVfxKind::Burn {
                if let Ok(mut e) = commands.get_entity(vfx) {
                    e.try_despawn();
                }
            }
        }
        for (icon_e, icon) in &q_icons {
            if icon.target == enemy && icon.kind == StatusVfxKind::Burn {
                if let Ok(mut e) = commands.get_entity(icon_e) {
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
    q_icons: Query<(Entity, &StatusIcon)>,
) {
    for enemy in removed.read() {
        for (vfx, link) in &q_links {
            if link.target == enemy && link.kind == StatusVfxKind::Poison {
                if let Ok(mut e) = commands.get_entity(vfx) {
                    e.try_despawn();
                }
            }
        }
        for (icon_e, icon) in &q_icons {
            if icon.target == enemy && icon.kind == StatusVfxKind::Poison {
                if let Ok(mut e) = commands.get_entity(icon_e) {
                    e.try_despawn();
                }
            }
        }
        if let Ok(mut e) = commands.get_entity(enemy) {
            e.remove::<PoisonVfxAttached>();
        }
    }
}

/// Story-653 — spawn les arcs électriques quand `StatusShock` apparaît
/// (identité Pépin : l'ennemi choqué GRÉSILLE pendant la fenêtre de vulnérabilité).
/// Miroir exact du pattern burn/poison (cap partagé, suivi par `sys_follow_status_vfx`).
pub fn sys_attach_shock_vfx(
    mut commands: Commands,
    effects: Option<Res<WeaponVfxEffects>>,
    config: Res<ElementConfig>,
    q_new: Query<
        (Entity, &GlobalTransform, &EnemyArchetype),
        (Added<Vulnerability>, Without<ShockVfxAttached>),
    >,
    q_live: Query<(), With<StatusVfxLink>>,
    mut stats: ResMut<ElementVfxStats>,
    // Story-654 — icône de statut sur le nameplate.
    registry: Res<NameplateRegistry>,
    nameplate: Res<EnemyNameplate>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
        commands.entity(enemy).insert(ShockVfxAttached);
        commands.spawn((
            ParticleEffect::new(effects.status_shock.clone()),
            // Texture étincelle (slot "color" de l'EffectAsset, partagée sparks).
            EffectMaterial { images: vec![effects.tex_spark.clone()] },
            Transform::from_translation(gt.translation() + Vec3::Y * (y * factor))
                .with_scale(Vec3::splat(factor)),
            StatusVfxLink { target: enemy, kind: StatusVfxKind::Shock, scale: factor },
            DespawnOnExit(GameMode::Roguelite),
        ));
        spawn_status_icon(
            &mut commands,
            &registry,
            &nameplate,
            &mut meshes,
            &mut materials,
            effects.tex_spark.clone(),
            Element::Shock.rgb(&config.vfx),
            enemy,
            StatusVfxKind::Shock,
        );
        stats.dot_pulses = stats.dot_pulses.saturating_add(1);
        live += 1;
    }
}

/// Despawn les arcs quand `StatusShock` est retiré (expiration ou mort).
pub fn sys_detach_shock_vfx(
    mut commands: Commands,
    mut removed: RemovedComponents<Vulnerability>,
    q_links: Query<(Entity, &StatusVfxLink)>,
    q_icons: Query<(Entity, &StatusIcon)>,
) {
    for enemy in removed.read() {
        for (vfx, link) in &q_links {
            if link.target == enemy && link.kind == StatusVfxKind::Shock {
                if let Ok(mut e) = commands.get_entity(vfx) {
                    e.try_despawn();
                }
            }
        }
        for (icon_e, icon) in &q_icons {
            if icon.target == enemy && icon.kind == StatusVfxKind::Shock {
                if let Ok(mut e) = commands.get_entity(icon_e) {
                    e.try_despawn();
                }
            }
        }
        if let Ok(mut e) = commands.get_entity(enemy) {
            e.remove::<ShockVfxAttached>();
        }
    }
}
