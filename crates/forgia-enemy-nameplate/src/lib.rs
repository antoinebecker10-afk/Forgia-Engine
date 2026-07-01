//! # forgia-enemy-nameplate (story-457, 2026-05-19)
//!
//! Floating HP nameplate au-dessus des enemies — 3D world-space billboard
//! avec `StandardMaterial unlit`. Remplace `forgia-ui-hud::bot_hp_floaters`
//! (egui screen-space) qui était limité aux écrans (pas de profondeur z,
//! pas de occlusion par les murs si projeté hors-écran).
//!
//! ## Architecture
//!
//! - Spawn-on-hit : un `CombatHitEvent` matérialise (ou refresh) un nameplate
//!   enfant du bot ciblé. Lifetime reset à chaque hit.
//! - Billboard cylindrique : yaw vers la caméra, pitch=0 (jamais tilt vertical).
//! - 2 quads superposés : background (full width) + fill (scale.x = hp_fraction).
//!   Pattern AAA simple, alpha unifiée pour le fade.
//! - Tuning genome `genomes/ui/enemy_nameplate.toml` (hot-reload).
//!
//! ## Custom shader
//!
//! `assets/shaders/nameplate_hp.wgsl` est livré (deliverable plan) pour un
//! upgrade futur — implémenter `Material` custom avec `AsBindGroup` uniforme
//! `hp_fraction` + bord arrondi GPU. V1 reste `StandardMaterial unlit + 2 quads`
//! pour shipper rapide.
//!
//! ## Sensor
//!
//! `forgia_enemy_nameplate.json` (1Hz) — active_count + tracked targets.

use bevy::prelude::*;
use forgia_combat::prelude::CombatHitEvent;
use forgia_combat::Health as CombatHealth;
use forgia_damage::DefenseLayer;
use std::collections::HashMap;
use std::fs;

mod tuning;
pub use tuning::{EnemyNameplate, EnemyNameplateTuning, EnemyNameplateTuningHandle};

pub mod prelude {
    pub use crate::{EnemyNameplate, EnemyNameplateTuning, ForgiaEnemyNameplatePlugin};
}

/// Marker root du nameplate (enfant du bot). Lifetime reset on each hit.
///
/// Story-461 (Vague 3) : `#[require(Transform, Visibility)]` garantit que tout
/// spawn de NameplateRoot insère Transform + Visibility avec Default si non fournis.
#[derive(Component)]
#[require(Transform, Visibility)]
pub struct NameplateRoot {
    pub target: Entity,
    pub lifetime_left: f32,
}

/// Marker (sur le bot) qui demande un nameplate permanent visible tant que
/// l'entité existe. Sans ce marker, l'ancien comportement spawn-on-hit reste.
///
/// Pattern AAA (Overwatch/Apex/CSGO) : nameplate enemy toujours visible
/// au-dessus de la cible quand celle-ci est dans le frustum. Le fade-on-damage
/// V1 était une approximation cartoon, remplacée ici par presence-permanent.
#[derive(Component)]
pub struct NameplateTarget;

/// Marker du quad fill HP (scale.x = hp_fraction).
#[derive(Component)]
pub struct NameplateFill;

/// Marker du quad background (full width).
#[derive(Component)]
pub struct NameplateBg;

/// Marker du quad fill Bouclier (bleu) — scale.x = shield/shield_max (story-644 P1 Inc.2).
#[derive(Component)]
pub struct NameplateShieldFill;

/// Marker du quad fill Armure (jaune) — scale.x = armor/armor_max (story-644 P1 Inc.2).
#[derive(Component)]
pub struct NameplateArmorFill;

/// Index target_entity → nameplate_root_entity (évite duplication).
#[derive(Resource, Default)]
pub struct NameplateRegistry {
    pub map: HashMap<Entity, Entity>,
}

pub struct ForgiaEnemyNameplatePlugin;

impl Plugin for ForgiaEnemyNameplatePlugin {
    fn build(&self, app: &mut App) {
        tuning::register_tuning(app);
        app.init_resource::<NameplateRegistry>()
            .add_systems(
                Update,
                (
                    cleanup_registry_on_target_removed,
                    spawn_nameplate_for_targets,
                    spawn_or_refresh_on_hit,
                    keep_permanent_nameplates_alive,
                    update_hp_fill,
                    update_defense_bars,
                    tick_lifetime_and_despawn,
                    sensor_write,
                )
                    .chain(),
            )
            // BUG-464-02 — billboard en PostUpdate après TransformPropagate pour
            // lire des `GlobalTransform` à jour (sinon lag 1 frame sur le yaw
            // parent extrait). Pattern Bevy standard pour billboards.
            .add_systems(
                PostUpdate,
                billboard_to_camera.after(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// Helper interne — spawn un nameplate enfant de `target`. Idempotent via registry.
#[allow(clippy::too_many_arguments)]
fn build_nameplate_for(
    target: Entity,
    commands: &mut Commands,
    registry: &mut NameplateRegistry,
    tuning: &EnemyNameplate,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    // Story-644 P1 Inc.2 — couche défensive de la cible (au spawn) : détermine quelles
    // barres Bouclier/Armure ajouter au-dessus de la HP (aucune si `None` / max=0).
    defense: Option<&DefenseLayer>,
) -> Option<Entity> {
    if registry.map.contains_key(&target) {
        return None;
    }
    let t = &tuning.0;
    let bg_mesh = meshes.add(Rectangle::new(t.width, t.height));
    let fill_mesh = meshes.add(Rectangle::new(
        t.width - t.border_thickness * 2.0,
        t.height - t.border_thickness * 2.0,
    ));

    let bg_mat = materials.add(StandardMaterial {
        base_color: Color::linear_rgb(t.bg_color[0], t.bg_color[1], t.bg_color[2]),
        unlit: true,
        ..default()
    });
    let fill_mat = materials.add(StandardMaterial {
        base_color: Color::linear_rgb(t.fill_color[0], t.fill_color[1], t.fill_color[2]),
        unlit: true,
        ..default()
    });

    let root_id = commands
        .spawn((
            NameplateRoot {
                target,
                lifetime_left: t.lifetime,
            },
            Transform::from_xyz(0.0, t.y_offset, 0.0),
            Name::new("EnemyNameplate"),
            ChildOf(target),
        ))
        .id();

    // Story-644 P1 Inc.2 — matériaux + positions des barres défensives (Bouclier bleu,
    // Armure jaune) empilées AU-DESSUS de la HP. Créées seulement pour les couches
    // présentes → 0 quad superflu (Tank = armure seule, Runner = bouclier seul).
    let shield_mat = defense.filter(|d| d.shield_max > 0.0).map(|_| {
        materials.add(StandardMaterial {
            base_color: Color::linear_rgb(t.shield_color[0], t.shield_color[1], t.shield_color[2]),
            unlit: true,
            ..default()
        })
    });
    let armor_mat = defense.filter(|d| d.armor_max > 0.0).map(|_| {
        materials.add(StandardMaterial {
            base_color: Color::linear_rgb(t.armor_color[0], t.armor_color[1], t.armor_color[2]),
            unlit: true,
            ..default()
        })
    });
    let bar_gap = t.height * 1.15;
    let shield_y = bar_gap;
    let armor_y = if shield_mat.is_some() { bar_gap * 2.0 } else { bar_gap };

    commands.entity(root_id).with_children(|p| {
        p.spawn((
            NameplateBg,
            Mesh3d(bg_mesh.clone()),
            MeshMaterial3d(bg_mat.clone()),
            Transform::from_xyz(0.0, 0.0, -0.005),
            Name::new("NameplateBg"),
        ));
        p.spawn((
            NameplateFill,
            Mesh3d(fill_mesh.clone()),
            MeshMaterial3d(fill_mat),
            Transform::from_xyz(0.0, 0.0, 0.0),
            Name::new("NameplateFill"),
        ));
        if let Some(sm) = shield_mat {
            p.spawn((
                NameplateBg,
                Mesh3d(bg_mesh.clone()),
                MeshMaterial3d(bg_mat.clone()),
                Transform::from_xyz(0.0, shield_y, -0.005),
                Name::new("NameplateShieldBg"),
            ));
            p.spawn((
                NameplateShieldFill,
                Mesh3d(fill_mesh.clone()),
                MeshMaterial3d(sm),
                Transform::from_xyz(0.0, shield_y, 0.0),
                Name::new("NameplateShieldFill"),
            ));
        }
        if let Some(am) = armor_mat {
            p.spawn((
                NameplateBg,
                Mesh3d(bg_mesh.clone()),
                MeshMaterial3d(bg_mat.clone()),
                Transform::from_xyz(0.0, armor_y, -0.005),
                Name::new("NameplateArmorBg"),
            ));
            p.spawn((
                NameplateArmorFill,
                Mesh3d(fill_mesh.clone()),
                MeshMaterial3d(am),
                Transform::from_xyz(0.0, armor_y, 0.0),
                Name::new("NameplateArmorFill"),
            ));
        }
    });

    registry.map.insert(target, root_id);
    Some(root_id)
}

/// BUG-464-01 — Purge les entrées registry dont la target a perdu son marker
/// `NameplateTarget` (typiquement mort du bot → recursive despawn enlève le
/// Component). Sans ça, la `HashMap` croît unbounded sur respawns successifs
/// et `registry_size` diverge de `active_count` dans le sensor.
fn cleanup_registry_on_target_removed(
    mut removed: RemovedComponents<NameplateTarget>,
    mut registry: ResMut<NameplateRegistry>,
    mut commands: Commands,
) {
    for target in removed.read() {
        if let Some(root_e) = registry.map.remove(&target) {
            // Si le bot est despawné par parent recursive, le root est déjà
            // mort — `get_entity` retourne Err, on skip silencieusement.
            if let Ok(mut ec) = commands.get_entity(root_e) {
                ec.try_despawn();
            }
        }
    }
}

/// Spawn permanent — pour toute entité `With<NameplateTarget>` sans nameplate.
/// Idempotent : check registry.
fn spawn_nameplate_for_targets(
    mut commands: Commands,
    mut registry: ResMut<NameplateRegistry>,
    q_targets: Query<Entity, With<NameplateTarget>>,
    q_defense: Query<&DefenseLayer>,
    tuning: Res<EnemyNameplate>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for target in &q_targets {
        let _ = build_nameplate_for(
            target,
            &mut commands,
            &mut registry,
            &tuning,
            &mut meshes,
            &mut materials,
            q_defense.get(target).ok(),
        );
    }
}

/// Spawn nameplate enfant du bot au premier hit, ou refresh lifetime/HP au hit suivant.
/// Conservé pour entités SANS marker `NameplateTarget` (mode legacy spawn-on-damage).
fn spawn_or_refresh_on_hit(
    mut events: MessageReader<CombatHitEvent>,
    mut commands: Commands,
    mut registry: ResMut<NameplateRegistry>,
    mut q_existing: Query<&mut NameplateRoot>,
    q_defense: Query<&DefenseLayer>,
    tuning: Res<EnemyNameplate>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        // Refresh si déjà existant.
        if let Some(root) = registry.map.get(&ev.target).copied() {
            if let Ok(mut np) = q_existing.get_mut(root) {
                np.lifetime_left = tuning.0.lifetime;
                continue;
            }
        }
        let _ = build_nameplate_for(
            ev.target,
            &mut commands,
            &mut registry,
            &tuning,
            &mut meshes,
            &mut materials,
            q_defense.get(ev.target).ok(),
        );
    }
}

/// Refresh lifetime continu pour les nameplates dont la cible est `NameplateTarget`.
/// Empêche le fade/despawn tant que la cible existe (permanent presence-driven).
fn keep_permanent_nameplates_alive(
    mut q_np: Query<&mut NameplateRoot>,
    q_target: Query<Entity, With<NameplateTarget>>,
    tuning: Res<EnemyNameplate>,
) {
    for mut np in &mut q_np {
        if q_target.contains(np.target) {
            np.lifetime_left = tuning.0.lifetime;
        }
    }
}

/// Met à jour scale.x du fill quad selon hp_fraction du target.
fn update_hp_fill(
    q_roots: Query<(&NameplateRoot, &Children)>,
    q_fill: Query<&NameplateFill>,
    mut q_xform: Query<&mut Transform, With<NameplateFill>>,
    q_health: Query<&CombatHealth>,
) {
    for (root, children) in &q_roots {
        let Ok(hp) = q_health.get(root.target) else {
            continue;
        };
        let frac = if hp.max > 0.01 {
            (hp.current / hp.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        for child in children.iter() {
            if q_fill.get(child).is_ok() {
                if let Ok(mut xf) = q_xform.get_mut(child) {
                    // Anchor center : scale.x autour de l'origine. Pour anchor left,
                    // décale translation.x de -(1 - frac) * half_width.
                    xf.scale.x = frac;
                }
            }
        }
    }
}

/// Story-644 P1 Inc.2 — met à jour scale.x des fills Bouclier/Armure selon la couche
/// défensive de la cible (miroir de `update_hp_fill`). Les deux queries sont
/// disjointes (`Without<NameplateShieldFill>` sur l'armure) → double `&mut Transform` OK.
fn update_defense_bars(
    q_roots: Query<(&NameplateRoot, &Children)>,
    q_defense: Query<&DefenseLayer>,
    mut q_shield: Query<&mut Transform, With<NameplateShieldFill>>,
    mut q_armor: Query<&mut Transform, (With<NameplateArmorFill>, Without<NameplateShieldFill>)>,
) {
    for (root, children) in &q_roots {
        let Ok(dl) = q_defense.get(root.target) else {
            continue;
        };
        for child in children.iter() {
            if let Ok(mut xf) = q_shield.get_mut(child) {
                xf.scale.x = if dl.shield_max > 0.0 {
                    (dl.shield / dl.shield_max).clamp(0.0, 1.0)
                } else {
                    0.0
                };
            } else if let Ok(mut xf) = q_armor.get_mut(child) {
                xf.scale.x = if dl.armor_max > 0.0 {
                    (dl.armor / dl.armor_max).clamp(0.0, 1.0)
                } else {
                    0.0
                };
            }
        }
    }
}

/// Billboard cylindrique — yaw vers caméra, pitch = 0. Évite tilt vertical
/// désagréable quand le joueur regarde en haut/bas.
///
/// Le nameplate root est `ChildOf(bot)`. Le bot rotate en permanence vers le
/// player (`bot_state_machine` set `Quat::from_rotation_y(yaw)`), donc la
/// rotation parente n'est PAS négligeable. On extrait le yaw parent et on le
/// soustrait du yaw cible cam → la rotation locale compense exactement pour
/// que la face mesh pointe la caméra peu importe où le bot regarde.
/// Sans cette soustraction, la face quad pouvait pointer dans le mauvais
/// sens et le back-face culling masquait le nameplate selon l'angle.
fn billboard_to_camera(
    q_cam: Query<&GlobalTransform, With<Camera3d>>,
    q_parent: Query<&GlobalTransform, Without<NameplateRoot>>,
    mut q_np: Query<(&GlobalTransform, &mut Transform, &ChildOf), With<NameplateRoot>>,
) {
    let Ok(cam_xf) = q_cam.single() else { return };
    let cam_pos = cam_xf.translation();
    for (np_global, mut np_local, child_of) in &mut q_np {
        let np_world = np_global.translation();
        let dx = cam_pos.x - np_world.x;
        let dz = cam_pos.z - np_world.z;
        let target_world_yaw = dx.atan2(dz);
        let parent_yaw = q_parent
            .get(child_of.parent())
            .map(|gt| gt.rotation().to_euler(EulerRot::YXZ).0)
            .unwrap_or(0.0);
        np_local.rotation = Quat::from_rotation_y(target_world_yaw - parent_yaw);
    }
}

/// Décrémente lifetime, fade alpha sur la fin, despawn à 0.
fn tick_lifetime_and_despawn(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut NameplateRoot, &Children)>,
    q_mat: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut registry: ResMut<NameplateRegistry>,
    tuning: Res<EnemyNameplate>,
) {
    let dt = time.delta_secs();
    for (root_e, mut np, children) in &mut q {
        np.lifetime_left -= dt;
        let alpha = if np.lifetime_left > tuning.0.fade_out_secs {
            1.0
        } else {
            (np.lifetime_left / tuning.0.fade_out_secs).clamp(0.0, 1.0)
        };
        for child in children.iter() {
            if let Ok(mat_h) = q_mat.get(child) {
                if let Some(mat) = materials.get_mut(&mat_h.0) {
                    let lc = mat.base_color.to_linear();
                    mat.base_color = Color::linear_rgba(lc.red, lc.green, lc.blue, alpha);
                    mat.alpha_mode = AlphaMode::Blend;
                }
            }
        }
        if np.lifetime_left <= 0.0 {
            registry.map.remove(&np.target);
            commands.entity(root_e).despawn();
        }
    }
}

/// Sensor 1Hz → `forgia_enemy_nameplate.json`.
fn sensor_write(
    time: Res<Time>,
    mut acc: Local<f32>,
    registry: Res<NameplateRegistry>,
    q: Query<&NameplateRoot>,
) {
    *acc += time.delta_secs();
    if *acc < 1.0 {
        return;
    }
    *acc = 0.0;

    let active_count = q.iter().count();
    let registry_size = registry.map.len();
    let mean_lifetime = if active_count > 0 {
        let s: f32 = q.iter().map(|n| n.lifetime_left).sum();
        s / active_count as f32
    } else {
        0.0
    };
    let payload = serde_json::json!({
        "timestamp_secs": time.elapsed_secs(),
        "active_count": active_count,
        "registry_size": registry_size,
        "mean_lifetime_left": mean_lifetime,
        "status": if active_count == 0 { "idle" } else { "active" },
    });
    let _ = fs::write("forgia_enemy_nameplate.json", payload.to_string());
}
