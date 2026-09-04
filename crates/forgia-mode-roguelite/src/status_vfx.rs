//! status_vfx.rs — VFX hanabi continus des DoT (remplace le dot-pulse sphère).
//!
//! Flamme sur `StatusBurn`, nuage toxique sur `StatusPoison`, arcs sur la marque
//! électrique (`Vulnerability`). L'effet est une entité hanabi **NON parentée**
//! (même pattern que le muzzle flash, seul chemin hanabi prouvé visible du repo),
//! **repositionnée chaque frame** sur le `GlobalTransform` de l'ennemi + offset
//! `vfx.status_y` (genome, hot-reload).
//!
//! ## LOT B bis (audit fire-path 2026-07-20) — pool partagé persistant
//!
//! L'ancien cycle spawn-sur-`Added` / despawn-sur-`RemovedComponents` créait et
//! détruisait une **instance Hanabi par (ennemi × statut)** — churn EffectCache/
//! bind-group/dispatch-init GPU à chaque application/expiration de statut, mesuré
//! par `spikes_15` (perf_diag : freezes 64-80 ms avec `status_auras` > 0).
//! Remplacé par un **pool partagé de [`STATUS_VFX_POOL_SIZE`] slots persistants**
//! (pattern `weapon_vfx::MuzzlePool` / `element_vfx::ElementBurstPool`) :
//! - **attach** = lease d'un slot libre : swap handle+texture (layout identique —
//!   capacités des 3 effets UNIFIÉES, cf `weapon_vfx/status.rs`), reposition,
//!   retrigger via `remove::<EffectSpawner>()` (ré-ajouté frais et actif par
//!   `tick_spawners`, cf source vendored 0.18 — ne ré-invalide pas le cache).
//! - **detach** = `spawner.active = false` (émission coupée) + slot caché.
//!   L'entité n'est JAMAIS despawnée.
//! - **saturation** = l'attach est sauté (les DÉGÂTS DoT continuent, le visuel
//!   est découplé du gameplay) ; nouvelle chance à la prochaine application
//!   FRAÎCHE du statut (expiration puis re-hit — un refresh par overwrite
//!   `try_insert` ne réarme pas `Added<T>`, vérifié auto-QA 2026-07-20).
//!
//! Le lease vit dans la Resource (mise à jour IMMÉDIATE, pas via Commands
//! différées) : deux attaches dans la même frame ne peuvent pas obtenir le même
//! slot. Hot-reload VFX : les slots libres reprennent les handles frais au
//! prochain attach (lus depuis `WeaponVfxEffects` courant) — les leases actives
//! pendant le reload CONTINUENT de rendre avec l'ancien asset (handle
//! ref-compté, vérifié auto-QA 2026-07-20) : staleness visuelle temporaire,
//! dev-only, assumée.
//!
//! Anti-occlusion (leçon 2026-06-24) : à `status_y` trop bas + rayon de spawn
//! étroit, les particules naissent DANS le mesh opaque du mob (capsule ~2 m,
//! rayon 0.4) et sont z-occluses. Fix : `status_y` ≈ haut du corps + rayon de
//! spawn large (cf `status.rs`, ~0.5) → particules à la surface/aux bords +
//! panache au-dessus = visibles.

use bevy::prelude::*;
use bevy_hanabi::{EffectAsset, EffectSpawner};
use forgia_effects::prelude::{EffectMaterial, ParticleEffect, WeaponVfxEffects};
use forgia_enemy_nameplate::{EnemyNameplate, NameplateRegistry};
use forgia_enemy_nameplate::{NameplateRoot, NameplateSight};

use crate::element_vfx::ElementVfxStats;
use crate::elements::{Element, ElementConfig, StatusBurn, StatusPoison};
// Story-653 — la « marque électrique » runtime est `forgia_damage::Vulnerability`
// (posée/retirée par elements.rs sur hit Shock ; `ShockParams` = config genome).
use crate::enemies::{skeleton_scale, EnemyArchetype};
use forgia_damage::Vulnerability;

/// Taille du pool partagé d'auras (borne de dimensionnement GPU interne, pas une
/// valeur gameplay — no-hardcode exempt). ~12 afflictions simultanées couvrent la
/// charge typique observée (« cap 48 >> ~12 brûlants typiques », ancien commentaire).
pub const STATUS_VFX_POOL_SIZE: usize = 12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusVfxKind {
    Burn,
    Poison,
    /// Story-653 — arcs électriques (StatusShock, identité Pépin).
    Shock,
}

/// Lien du SLOT de pool vers l'ennemi affligé (suivi + capteur perf_diag).
/// Présent uniquement pendant une lease — `perf_diag` compte les auras actives
/// via `With<StatusVfxLink>`, inchangé.
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

// ─── Pool partagé persistant (LOT B bis, 2026-07-20) ────────────────────────

/// Pool des slots d'aura : entités hanabi persistantes, jamais despawnées.
#[derive(Resource)]
pub struct StatusVfxPool {
    pub slots: [Entity; STATUS_VFX_POOL_SIZE],
    /// Occupation IMMÉDIATE — `None` = libre. Mise à jour dans le corps des
    /// systèmes (pas via Commands différées) pour que deux attaches de la même
    /// frame (burn puis poison, autre ennemi) ne claiment pas le même slot.
    pub leased: [Option<(Entity, StatusVfxKind)>; STATUS_VFX_POOL_SIZE],
}

/// Handle d'effet du kind (lu depuis le `WeaponVfxEffects` COURANT → les handles
/// frais post-hot-reload sont repris au prochain attach).
fn status_effect_handle(effects: &WeaponVfxEffects, kind: StatusVfxKind) -> Handle<EffectAsset> {
    match kind {
        StatusVfxKind::Burn => effects.status_flame.clone(),
        StatusVfxKind::Poison => effects.status_poison_cloud.clone(),
        StatusVfxKind::Shock => effects.status_shock.clone(),
    }
}

/// Texture du slot "color" par kind (léchure de flamme / volutes / étincelle).
fn status_texture(effects: &WeaponVfxEffects, kind: StatusVfxKind) -> Handle<Image> {
    match kind {
        StatusVfxKind::Burn => effects.tex_flame.clone(),
        StatusVfxKind::Poison => effects.tex_poison.clone(),
        StatusVfxKind::Shock => effects.tex_spark.clone(),
    }
}

/// Assets PARTAGÉS des icônes de statut (M1 auto-QA 2026-07-20) : quad 1×1
/// (la taille passe par le scale du Transform → suit le tuning nameplate
/// courant) + 1 matériau par kind, construits UNE fois. L'ancien
/// `spawn_status_icon` faisait `meshes.add` + `materials.add` PAR icône — même
/// cadence événementielle (attach/expire) que le churn d'auras corrigé par le
/// pool. Teintes baked au boot depuis le genome (tradeoff accepté, précédent
/// `ElementBurstAssets`).
#[derive(Resource)]
pub struct StatusIconAssets {
    pub quad: Handle<Mesh>,
    /// Indexé par [`icon_mat_index`] : [Burn, Poison, Shock].
    pub mats: [Handle<StandardMaterial>; 3],
}

fn icon_mat_index(kind: StatusVfxKind) -> usize {
    match kind {
        StatusVfxKind::Burn => 0,
        StatusVfxKind::Poison => 1,
        StatusVfxKind::Shock => 2,
    }
}

/// PostStartup (après `setup_weapon_vfx`) — spawn les slots persistants, cachés
/// à Y=-10000 : leur toute première émission (invisible) paie la compile shader
/// des 3 effets avant le 1er statut réel (leçon anti-freeze story-594 — les
/// kinds sont alternés pour warmer flamme, poison ET shock). Construit aussi
/// les [`StatusIconAssets`] partagés (M1 auto-QA).
pub fn sys_init_status_vfx_pool(
    mut commands: Commands,
    effects: Option<Res<WeaponVfxEffects>>,
    config: Option<Res<ElementConfig>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(effects) = effects else {
        return;
    };
    const KINDS: [StatusVfxKind; 3] = [
        StatusVfxKind::Burn,
        StatusVfxKind::Poison,
        StatusVfxKind::Shock,
    ];
    let hidden = Transform::from_xyz(0.0, -10_000.0, 0.0);
    let slots: [Entity; STATUS_VFX_POOL_SIZE] = std::array::from_fn(|i| {
        let kind = KINDS[i % KINDS.len()];
        commands
            .spawn((
                Name::new("StatusVfxSlot"),
                ParticleEffect::new(status_effect_handle(&effects, kind)),
                EffectMaterial {
                    images: vec![status_texture(&effects, kind)],
                },
                hidden,
                Visibility::Hidden,
            ))
            .id()
    });
    commands.insert_resource(StatusVfxPool {
        slots,
        leased: [None; STATUS_VFX_POOL_SIZE],
    });
    // Assets partagés des icônes : quad unitaire + 1 matériau par kind
    // (ordre = icon_mat_index : Burn, Poison, Shock).
    let vfx = config.map(|c| c.vfx.clone()).unwrap_or_default();
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    let mats = KINDS.map(|kind| {
        let tint = match kind {
            StatusVfxKind::Burn => Element::Fire.rgb(&vfx),
            StatusVfxKind::Poison => Element::Poison.rgb(&vfx),
            StatusVfxKind::Shock => Element::Shock.rgb(&vfx),
        };
        materials.add(StandardMaterial {
            base_color: Color::linear_rgb(tint[0], tint[1], tint[2]),
            base_color_texture: Some(status_texture(&effects, kind)),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    });
    commands.insert_resource(StatusIconAssets { quad, mats });
    info!(
        "[status-vfx] pool partagé de {STATUS_VFX_POOL_SIZE} auras hanabi + assets icônes construits (LOT B bis, audit fire-path 2026-07-20 — remplace spawn/despawn par statut)"
    );
}

/// Acquiert un slot libre pour (enemy, kind). Lease immédiat ; le reste (link,
/// visibilité, retrigger) passe par Commands — DIFFÉRÉ, donc appliqué APRÈS un
/// éventuel release du même slot dans la même frame (l'attach gagne toujours).
/// Retourne `false` si pool saturé — l'appelant saute garde/icône/stats ;
/// nouvelle chance à la prochaine application FRAÎCHE du statut (expiration
/// puis re-hit : un refresh par overwrite ne réarme pas `Added<T>`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn acquire_status_slot(
    commands: &mut Commands,
    pool: &mut StatusVfxPool,
    q_slots: &mut Query<(&mut ParticleEffect, &mut EffectMaterial, &mut Transform)>,
    effects: &WeaponVfxEffects,
    enemy: Entity,
    kind: StatusVfxKind,
    pos: Vec3,
    factor: f32,
) -> bool {
    let Some(idx) = pool.leased.iter().position(|l| l.is_none()) else {
        return false;
    };
    let slot = pool.slots[idx];
    let Ok((mut effect, mut material, mut tf)) = q_slots.get_mut(slot) else {
        return false;
    };
    pool.leased[idx] = Some((enemy, kind));
    // Swap handle+texture : layout identique (capacités unifiées, cf status.rs)
    // → reconfiguration légère du CompiledParticleEffect, pas de réalloc.
    effect.handle = status_effect_handle(effects, kind);
    material.images.clear();
    material.images.push(status_texture(effects, kind));
    *tf = Transform::from_translation(pos).with_scale(Vec3::splat(factor));
    commands
        .entity(slot)
        .insert((
            StatusVfxLink { target: enemy, kind, scale: factor },
            Visibility::Visible,
        ))
        // Retrigger : `tick_spawners` ré-ajoute un EffectSpawner frais et ACTIF
        // dès la frame suivante (clone des SpawnerSettings de l'asset) —
        // comportement identique à un spawn neuf, sans recréer l'entité.
        .remove::<EffectSpawner>();
    true
}

/// Libère le slot d'une affliction : lease rendu (immédiat), émission coupée
/// (`spawner.active = false`), link retiré + slot caché (différé — un attach de
/// la même frame qui réutilise le slot ré-applique link/Visible APRÈS, cf ordre
/// d'application des Commands). L'entité du slot n'est jamais despawnée.
fn release_status_slot(
    commands: &mut Commands,
    pool: &mut StatusVfxPool,
    q_spawners: &mut Query<&mut EffectSpawner>,
    enemy: Entity,
    kind: StatusVfxKind,
) {
    for (idx, lease) in pool.leased.iter_mut().enumerate() {
        if *lease == Some((enemy, kind)) {
            *lease = None;
            let slot = pool.slots[idx];
            if let Ok(mut spawner) = q_spawners.get_mut(slot) {
                spawner.active = false;
            }
            if let Ok(mut e) = commands.get_entity(slot) {
                e.remove::<StatusVfxLink>().insert(Visibility::Hidden);
            }
        }
    }
}

/// OnExit(GameMode::Roguelite) — libère TOUTES les leases : hors Roguelite les
/// systèmes de detach ne tournent plus (`run_if`) et les `RemovedComponents` des
/// ennemis despawnés par `DespawnOnExit` seraient perdus — sans ce reset, des
/// auras orphelines resteraient allumées dans le Bourg.
pub fn sys_reset_status_vfx_pool(
    mut commands: Commands,
    pool: Option<ResMut<StatusVfxPool>>,
    mut q_spawners: Query<&mut EffectSpawner>,
) {
    let Some(mut pool) = pool else {
        return;
    };
    for idx in 0..STATUS_VFX_POOL_SIZE {
        if pool.leased[idx].is_none() {
            continue;
        }
        pool.leased[idx] = None;
        let slot = pool.slots[idx];
        if let Ok(mut spawner) = q_spawners.get_mut(slot) {
            spawner.active = false;
        }
        if let Ok(mut e) = commands.get_entity(slot) {
            e.remove::<StatusVfxLink>().insert(Visibility::Hidden);
        }
    }
}

// ─── Icônes de statut (story-654) ───────────────────────────────────────────

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
/// M1 auto-QA 2026-07-20 : quad + matériau PARTAGÉS ([`StatusIconAssets`]) —
/// plus aucun `meshes.add`/`materials.add` par attach ; la taille passe par le
/// scale du Transform (mesh unitaire) → suit le tuning nameplate courant.
fn spawn_status_icon(
    commands: &mut Commands,
    registry: &NameplateRegistry,
    nameplate: &EnemyNameplate,
    icon_assets: &StatusIconAssets,
    enemy: Entity,
    kind: StatusVfxKind,
) {
    let Some(&root) = registry.map.get(&enemy) else {
        return;
    };
    let t = &nameplate.0;
    let icon = t.height * 1.4;
    let x = t.width / 2.0 + t.height * (1.0 + icon_slot(kind) * 1.7);
    commands.spawn((
        StatusIcon {
            target: enemy,
            kind,
        },
        Mesh3d(icon_assets.quad.clone()),
        MeshMaterial3d(icon_assets.mats[icon_mat_index(kind)].clone()),
        Transform::from_xyz(x, 0.0, 0.0).with_scale(Vec3::splat(icon)),
        ChildOf(root),
        Name::new("StatusIcon"),
    ));
}

// ─── Attach / follow / detach ───────────────────────────────────────────────

/// Lease une flamme du pool à la position de l'ennemi quand `StatusBurn`
/// apparaît. Le suivi est fait par `sys_follow_status_vfx`.
#[allow(clippy::too_many_arguments)]
pub fn sys_attach_burn_vfx(
    mut commands: Commands,
    effects: Option<Res<WeaponVfxEffects>>,
    config: Res<ElementConfig>,
    q_new: Query<
        (Entity, &GlobalTransform, &EnemyArchetype),
        (Added<StatusBurn>, Without<BurnVfxAttached>),
    >,
    pool: Option<ResMut<StatusVfxPool>>,
    mut q_slots: Query<(&mut ParticleEffect, &mut EffectMaterial, &mut Transform)>,
    mut stats: ResMut<ElementVfxStats>,
    // Story-654 — icône de statut sur le nameplate (assets partagés, M1 auto-QA).
    registry: Res<NameplateRegistry>,
    nameplate: Res<EnemyNameplate>,
    icon_assets: Option<Res<StatusIconAssets>>,
) {
    let (Some(effects), Some(mut pool), Some(icon_assets)) = (effects, pool, icon_assets) else {
        return;
    };
    let y = config.vfx.status_y;
    for (enemy, gt, arch) in &q_new {
        let factor = skeleton_scale(*arch); // l'aura monte + s'élargit avec le mob
        let pos = gt.translation() + Vec3::Y * (y * factor);
        if !acquire_status_slot(
            &mut commands,
            &mut pool,
            &mut q_slots,
            &effects,
            enemy,
            StatusVfxKind::Burn,
            pos,
            factor,
        ) {
            // Pool saturé : visuel sauté pour CETTE application (garde NON
            // posée ; un refresh par overwrite ne réarme pas Added<T> → re-tente
            // à l'expiration + réapplication). Dégâts DoT JAMAIS affectés.
            continue;
        }
        commands.entity(enemy).insert(BurnVfxAttached);
        spawn_status_icon(
            &mut commands,
            &registry,
            &nameplate,
            &icon_assets,
            enemy,
            StatusVfxKind::Burn,
        );
        stats.dot_pulses = stats.dot_pulses.saturating_add(1);
    }
}

/// Lease un nuage toxique du pool quand `StatusPoison` apparaît.
#[allow(clippy::too_many_arguments)]
pub fn sys_attach_poison_vfx(
    mut commands: Commands,
    effects: Option<Res<WeaponVfxEffects>>,
    config: Res<ElementConfig>,
    q_new: Query<
        (Entity, &GlobalTransform, &EnemyArchetype),
        (Added<StatusPoison>, Without<PoisonVfxAttached>),
    >,
    pool: Option<ResMut<StatusVfxPool>>,
    mut q_slots: Query<(&mut ParticleEffect, &mut EffectMaterial, &mut Transform)>,
    mut stats: ResMut<ElementVfxStats>,
    // Story-654 — icône de statut sur le nameplate (assets partagés, M1 auto-QA).
    registry: Res<NameplateRegistry>,
    nameplate: Res<EnemyNameplate>,
    icon_assets: Option<Res<StatusIconAssets>>,
) {
    let (Some(effects), Some(mut pool), Some(icon_assets)) = (effects, pool, icon_assets) else {
        return;
    };
    let y = config.vfx.status_y;
    for (enemy, gt, arch) in &q_new {
        let factor = skeleton_scale(*arch);
        let pos = gt.translation() + Vec3::Y * (y * factor);
        if !acquire_status_slot(
            &mut commands,
            &mut pool,
            &mut q_slots,
            &effects,
            enemy,
            StatusVfxKind::Poison,
            pos,
            factor,
        ) {
            continue; // pool saturé — cf commentaire du site Burn
        }
        commands.entity(enemy).insert(PoisonVfxAttached);
        spawn_status_icon(
            &mut commands,
            &registry,
            &nameplate,
            &icon_assets,
            enemy,
            StatusVfxKind::Poison,
        );
        stats.dot_pulses = stats.dot_pulses.saturating_add(1);
    }
}

/// Story-653 — lease les arcs électriques quand la marque (`Vulnerability`)
/// apparaît (identité Pépin : l'ennemi choqué GRÉSILLE pendant la fenêtre de
/// vulnérabilité). Miroir exact du pattern burn/poison.
#[allow(clippy::too_many_arguments)]
pub fn sys_attach_shock_vfx(
    mut commands: Commands,
    effects: Option<Res<WeaponVfxEffects>>,
    config: Res<ElementConfig>,
    q_new: Query<
        (Entity, &GlobalTransform, &EnemyArchetype),
        (Added<Vulnerability>, Without<ShockVfxAttached>),
    >,
    pool: Option<ResMut<StatusVfxPool>>,
    mut q_slots: Query<(&mut ParticleEffect, &mut EffectMaterial, &mut Transform)>,
    mut stats: ResMut<ElementVfxStats>,
    // Story-654 — icône de statut sur le nameplate (assets partagés, M1 auto-QA).
    registry: Res<NameplateRegistry>,
    nameplate: Res<EnemyNameplate>,
    icon_assets: Option<Res<StatusIconAssets>>,
) {
    let (Some(effects), Some(mut pool), Some(icon_assets)) = (effects, pool, icon_assets) else {
        return;
    };
    let y = config.vfx.status_y;
    for (enemy, gt, arch) in &q_new {
        let factor = skeleton_scale(*arch);
        let pos = gt.translation() + Vec3::Y * (y * factor);
        if !acquire_status_slot(
            &mut commands,
            &mut pool,
            &mut q_slots,
            &effects,
            enemy,
            StatusVfxKind::Shock,
            pos,
            factor,
        ) {
            continue; // pool saturé — cf commentaire du site Burn
        }
        commands.entity(enemy).insert(ShockVfxAttached);
        spawn_status_icon(
            &mut commands,
            &registry,
            &nameplate,
            &icon_assets,
            enemy,
            StatusVfxKind::Shock,
        );
        stats.dot_pulses = stats.dot_pulses.saturating_add(1);
    }
}

/// Repositionne chaque slot leasé sur le `GlobalTransform` de son ennemi +
/// `status_y`, chaque frame (le slot n'est PAS parenté → suivi manuel ;
/// `status_y` lu en continu → la hauteur est hot-reloadable). Ennemi disparu →
/// `sys_detach_*` libère le slot sur `RemovedComponents`.
pub fn sys_follow_status_vfx(
    config: Res<ElementConfig>,
    q_targets: Query<&GlobalTransform, Without<StatusVfxLink>>,
    mut q_vfx: Query<(&mut Transform, &StatusVfxLink)>,
) {
    let y = config.vfx.status_y;
    for (mut tf, link) in &mut q_vfx {
        if let Ok(gt) = q_targets.get(link.target) {
            // scale (taille du mob) figée au lease ; on ne touche que la position.
            tf.translation = gt.translation() + Vec3::Y * (y * link.scale);
        }
    }
}

/// Libère la flamme quand `StatusBurn` est retiré (expiration ou mort).
pub fn sys_detach_burn_vfx(
    mut commands: Commands,
    mut removed: RemovedComponents<StatusBurn>,
    pool: Option<ResMut<StatusVfxPool>>,
    mut q_spawners: Query<&mut EffectSpawner>,
    q_icons: Query<(Entity, &StatusIcon)>,
) {
    let Some(mut pool) = pool else {
        return;
    };
    for enemy in removed.read() {
        release_status_slot(
            &mut commands,
            &mut pool,
            &mut q_spawners,
            enemy,
            StatusVfxKind::Burn,
        );
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

/// Libère le nuage quand `StatusPoison` est retiré (expiration ou mort).
pub fn sys_detach_poison_vfx(
    mut commands: Commands,
    mut removed: RemovedComponents<StatusPoison>,
    pool: Option<ResMut<StatusVfxPool>>,
    mut q_spawners: Query<&mut EffectSpawner>,
    q_icons: Query<(Entity, &StatusIcon)>,
) {
    let Some(mut pool) = pool else {
        return;
    };
    for enemy in removed.read() {
        release_status_slot(
            &mut commands,
            &mut pool,
            &mut q_spawners,
            enemy,
            StatusVfxKind::Poison,
        );
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

/// Libère les arcs quand la marque électrique est retirée (expiration ou mort).
pub fn sys_detach_shock_vfx(
    mut commands: Commands,
    mut removed: RemovedComponents<Vulnerability>,
    pool: Option<ResMut<StatusVfxPool>>,
    mut q_spawners: Query<&mut EffectSpawner>,
    q_icons: Query<(Entity, &StatusIcon)>,
) {
    let Some(mut pool) = pool else {
        return;
    };
    for enemy in removed.read() {
        release_status_slot(
            &mut commands,
            &mut pool,
            &mut q_spawners,
            enemy,
            StatusVfxKind::Shock,
        );
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

// ─── Plaques de nom gouvernées par la ligne de vue ───────────────────────────

/// Ferme les plaques de nom des ennemis que le joueur ne voit pas.
///
/// ## Le défaut
///
/// « Je vois leurs noms à travers les assets », rapporté en jeu le 2026-08-04.
/// La plaque flotte au-dessus de la tête : elle dépasse par-dessus le prop qui
/// cache le mob, et trahit une position qu'on ne devrait pas connaître.
///
/// ## Pourquoi ce système vit ICI
///
/// `forgia-enemy-nameplate` ne connaît ni l'IA ni la physique — et ne doit pas,
/// le Hall s'en sert aussi. Elle expose un drapeau ; cette crate, qui connaît
/// les deux, l'écrit. La ligne de vue est **déjà** calculée à 8 Hz par l'IA :
/// la relire coûte une comparaison, un raycast par plaque et par frame la
/// paierait une seconde fois.
///
/// ## La fenêtre de grâce compte
///
/// On lit `has_los || los_lost_grace_left > 0` et non `has_los` seul : à 8 Hz,
/// une vue qui clignote ferait clignoter la plaque. La grâce de l'IA (2 s) lui
/// sert aussi d'amortisseur.
pub fn sys_nameplate_follows_sight(
    bots: Query<&forgia_ai_arena_bot::ArenaBot>,
    mut plates: Query<(&NameplateRoot, &mut NameplateSight)>,
) {
    for (root, mut sight) in &mut plates {
        // Une plaque dont la cible n'est pas un bot d'arène (PNJ du Hall,
        // boss scénarisé) est laissée VISIBLE : ce système restreint, il
        // n'impose rien à ce qu'il ne connaît pas.
        let Ok(bot) = bots.get(root.target) else {
            continue;
        };
        let vu = bot.has_los || bot.los_lost_grace_left > 0.0;
        // Écriture gardée : `NameplateSight` est lu via `Changed`, écrire à
        // l'identique réveillerait le système d'application chaque frame.
        if sight.visible != vu {
            sight.visible = vu;
        }
    }
}
