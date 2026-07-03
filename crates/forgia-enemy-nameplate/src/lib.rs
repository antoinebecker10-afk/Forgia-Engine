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
    pub use crate::{EnemyNameplate, EnemyNameplateTuning, ForgiaEnemyNameplatePlugin, NameplateAnchor};
}

/// Override (m) de la hauteur locale du nameplate au-dessus du parent — posé par le
/// caller au spawn (story-644 fix boss). Sans lui, le `y_offset` genome (fixe) est
/// utilisé : OK pour les petits ennemis mais le **boss géant** (capsule ~7 m) rendait
/// son nameplate DANS son corps. Le caller met `capsule_half_height + radius + marge`
/// → le nameplate colle la tête de CHAQUE ennemi, taille-agnostique.
#[derive(Component, Debug, Clone, Copy)]
pub struct NameplateAnchor(pub f32);

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

/// Plaque de Bouclier (bleu) — story-654 : visible tant que `shield > threshold`
/// (les plaques « sautent » une à une, façon Apex/DRG — remplace la barre scalée).
#[derive(Component)]
pub struct NameplateShieldFill {
    pub threshold: f32,
}

/// Plaque d'Armure (jaune) — story-654 : visible tant que `armor > threshold`.
#[derive(Component)]
pub struct NameplateArmorFill {
    pub threshold: f32,
}

/// Espace entre plaques (fraction de la largeur du nameplate) — layout UI.
const PLATE_GAP_FRAC: f32 = 0.02;
/// Nombre max de plaques par couche (au-delà, les plaques valent plus de PV).
const MAX_PLATES: usize = 8;

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
                    promote_anchored_to_permanent,
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
    // Story-644 fix boss — hauteur locale du nameplate (m). None → `y_offset` genome fixe.
    anchor_y: Option<f32>,
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
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let fill_mat = materials.add(StandardMaterial {
        base_color: Color::linear_rgb(t.fill_color[0], t.fill_color[1], t.fill_color[2]),
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    let root_id = commands
        .spawn((
            NameplateRoot {
                target,
                lifetime_left: t.lifetime,
            },
            // Story-644 fix boss — hauteur par-ennemi si fournie (colle la tête même
            // sur le boss géant), sinon fallback `y_offset` genome (fixe).
            Transform::from_xyz(0.0, anchor_y.unwrap_or(t.y_offset), 0.0),
            // 2026-07-03 — Visibility OBLIGATOIRE sur le root : les quads enfants
            // (Mesh3d → Visibility auto) ont besoin d'une chaîne d'ancêtres complète
            // pour l'InheritedVisibility, sinon barres invisibles (classe B0004).
            Visibility::default(),
            Name::new("EnemyNameplate"),
            ChildOf(target),
        ))
        .id();

    // Story-654 — la barre principale = LA VIE SEULE ; les couches défensives
    // s'affichent en PLAQUES segmentées au-dessus (façon Apex/DRG : « il reste
    // 2 plaques bleues » se lit d'un coup d'œil, une barre scalée non).
    let bar_gap = t.height * 1.15;
    let has_shield = defense.map(|d| d.shield_max > 0.0).unwrap_or(false);
    let shield_y = bar_gap;
    let armor_y = if has_shield { bar_gap * 2.0 } else { bar_gap };

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
    });

    // Rangée de plaques pour une couche : n = ceil(max / segment_hp), seuil de la
    // plaque i = i × segment_hp (visible tant que la couche dépasse le seuil).
    let mut spawn_plate_row = |max: f32, seg_hp: f32, y: f32, color: [f32; 3], shield: bool| {
        let seg_hp = seg_hp.max(1.0);
        let n = ((max / seg_hp).ceil() as usize).clamp(1, MAX_PLATES);
        let gap = t.width * PLATE_GAP_FRAC;
        let plate_w = (t.width - gap * (n as f32 - 1.0)) / n as f32;
        let plate_mesh = meshes.add(Rectangle::new(plate_w, t.height * 0.7));
        let mat = materials.add(StandardMaterial {
            base_color: Color::linear_rgb(color[0], color[1], color[2]),
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        for i in 0..n {
            let x = -t.width / 2.0 + plate_w / 2.0 + i as f32 * (plate_w + gap);
            let threshold = i as f32 * seg_hp;
            commands.entity(root_id).with_children(|p| {
                if shield {
                    p.spawn((
                        NameplateShieldFill { threshold },
                        Mesh3d(plate_mesh.clone()),
                        MeshMaterial3d(mat.clone()),
                        Transform::from_xyz(x, y, 0.0),
                        Name::new("ShieldPlate"),
                    ));
                } else {
                    p.spawn((
                        NameplateArmorFill { threshold },
                        Mesh3d(plate_mesh.clone()),
                        MeshMaterial3d(mat.clone()),
                        Transform::from_xyz(x, y, 0.0),
                        Name::new("ArmorPlate"),
                    ));
                }
            });
        }
    };
    if let Some(d) = defense {
        if d.shield_max > 0.0 {
            spawn_plate_row(d.shield_max, t.shield_segment_hp, shield_y, t.shield_color, true);
        }
        if d.armor_max > 0.0 {
            spawn_plate_row(d.armor_max, t.armor_segment_hp, armor_y, t.armor_color, false);
        }
    }

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

/// Auto-promotion (2026-07-03, feedback user « les PV des mobs ne sont pas
/// visibles ») : toute entité qui reçoit un `NameplateAnchor` (posé au spawn des
/// ennemis par waves.rs) devient cible PERMANENTE — barres HP/Bouclier/Armure
/// visibles dès le spawn, pas seulement au premier hit. C'est le pattern voulu
/// par ce crate (doc en tête : « presence-permanent » Overwatch/Apex) — aucun
/// caller n'optait jamais dedans. Le mode legacy on-hit reste pour les entités
/// sans anchor.
fn promote_anchored_to_permanent(
    mut commands: Commands,
    q_new: Query<Entity, (Added<NameplateAnchor>, Without<NameplateTarget>)>,
) {
    for e in &q_new {
        commands.entity(e).insert(NameplateTarget);
    }
}

/// Spawn permanent — pour toute entité `With<NameplateTarget>` sans nameplate.
/// Idempotent : check registry.
fn spawn_nameplate_for_targets(
    mut commands: Commands,
    mut registry: ResMut<NameplateRegistry>,
    q_targets: Query<Entity, With<NameplateTarget>>,
    q_defense: Query<&DefenseLayer>,
    q_anchor: Query<&NameplateAnchor>,
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
            q_anchor.get(target).ok().map(|a| a.0),
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
    q_anchor: Query<&NameplateAnchor>,
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
            q_anchor.get(ev.target).ok().map(|a| a.0),
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

/// Story-654 — met à jour la VISIBILITÉ des plaques Bouclier/Armure : la plaque
/// reste affichée tant que la couche dépasse son seuil → les plaques « sautent »
/// une à une (remplace les barres scalées story-644). Queries disjointes
/// (`Without<NameplateShieldFill>` sur l'armure) → double `&mut Visibility` OK.
fn update_defense_bars(
    q_roots: Query<(&NameplateRoot, &Children)>,
    q_defense: Query<&DefenseLayer>,
    mut q_shield: Query<(&NameplateShieldFill, &mut Visibility)>,
    mut q_armor: Query<
        (&NameplateArmorFill, &mut Visibility),
        Without<NameplateShieldFill>,
    >,
) {
    for (root, children) in &q_roots {
        let Ok(dl) = q_defense.get(root.target) else {
            continue;
        };
        for child in children.iter() {
            if let Ok((plate, mut vis)) = q_shield.get_mut(child) {
                *vis = if dl.shield > plate.threshold {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            } else if let Ok((plate, mut vis)) = q_armor.get_mut(child) {
                *vis = if dl.armor > plate.threshold {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
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
    // 2026-07-03 — BUG racine « barres invisibles » : `.single()` échouait en
    // SILENCE dès qu'une 2e Camera3d existe (ViewmodelCamera, enfant de la
    // FpsCamera) → billboard jamais exécuté → quads single-sided jamais tournés
    // vers la caméra = cullés. Les deux cams partagent la même position monde
    // (viewmodel = enfant à IDENTITY) → n'importe laquelle donne le bon yaw.
    let Some(cam_xf) = q_cam.iter().next() else {
        return;
    };
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
