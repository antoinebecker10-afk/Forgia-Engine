//! arms.rs — Mains + avant-bras FPS **générés proceduralement** (story-617 inc.2,
//! refonte v2 après retour visuel : v1 = « gros tubes »).
//!
//! Aucun asset externe. Chaque bras = un **poing** (paume + 4 doigts repliés +
//! pouce, en peau) au bout d'un **avant-bras court à manche** (capsule, tissu),
//! enfant de la `FpsCamera` (taille CONSTANTE, ≠ arme auto-scalée). Reçoit l'offset
//! sway/bob partagé ([`crate::pose::ViewmodelMotionOffset`]).
//!
//! Plafond assumé : procédural = **stylisé cartoon** (cohérent toon shader), pas
//! photoréaliste. Réalisme poussé = importer un mesh de mains riggé (asset).
//!
//! Placement global (offset + échelle) **réglable à chaud** via
//! `fps_tuning.toml [viewmodel_arms]` → itération sans rebuild. La géométrie fine
//! (proportions du poing) reste en constantes cosmétiques (non exposées créateur).

use bevy::prelude::*;
use forgia_core::prelude::GameMode;
use forgia_player::prelude::FpsCamera;

use crate::pose::{apply_viewmodel_sway_bob, ViewmodelMotionOffset};

/// Marker sur le root des bras (enfant de la FpsCamera).
#[derive(Component)]
pub struct ViewmodelArms;

/// Placement global des bras — réglable à chaud (`fps_tuning.toml [viewmodel_arms]`).
/// Offset + échelle du root → ajuster où les mains se posent sans rebuild.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ViewmodelArmsTuning {
    pub enabled: bool,
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub offset_z: f32,
}

impl Default for ViewmodelArmsTuning {
    fn default() -> Self {
        Self {
            enabled: true,
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            offset_z: 0.0,
        }
    }
}

// ── Couleurs cartoon (cel-shadées par le toon post-process), RGB sRGB ──
const SKIN: [f32; 3] = [0.80, 0.58, 0.42];
const SLEEVE: [f32; 3] = [0.20, 0.23, 0.30];

// ── Proportions du poing (mètres, repère main-local : +Y = vers les doigts,
//    X = largeur, Z = épaisseur). Réalistes : paume ~7cm, avant-bras ~15cm. ──
const PALM: Vec3 = Vec3::new(0.072, 0.055, 0.038);
const FINGER: Vec3 = Vec3::new(0.014, 0.042, 0.022);
const THUMB: Vec3 = Vec3::new(0.016, 0.038, 0.022);
const FOREARM_RADIUS: f32 = 0.034;
const FOREARM_LEN: f32 = 0.15;

// Ancrages des 2 poignets + coudes (camera-local : -Z avant, +X droite, -Y bas).
// Coude → poignet définit l'orientation de l'avant-bras. Main dominante (droite,
// gâchette) un peu en retrait ; main de soutien (gauche) plus en avant (foregrip).
const WRIST_R: Vec3 = Vec3::new(0.075, -0.215, -0.40);
const ELBOW_R: Vec3 = Vec3::new(0.20, -0.42, -0.20);
const WRIST_L: Vec3 = Vec3::new(-0.02, -0.185, -0.52);
const ELBOW_L: Vec3 = Vec3::new(-0.19, -0.40, -0.26);

fn cuboid(meshes: &mut Assets<Mesh>, s: Vec3) -> Handle<Mesh> {
    meshes.add(Cuboid::new(s.x, s.y, s.z))
}

/// Construit un poing complet (paume + doigts repliés + pouce + avant-bras),
/// tout en repère main-local, enfant d'une entité « hand » placée au poignet.
#[allow(clippy::too_many_arguments)]
fn spawn_fist(
    commands: &mut Commands,
    parent: Entity,
    meshes: &mut Assets<Mesh>,
    skin: &Handle<StandardMaterial>,
    sleeve: &Handle<StandardMaterial>,
    elbow: Vec3,
    wrist: Vec3,
    mirror: f32, // +1 = main droite, -1 = main gauche (miroir du pouce)
) {
    // Repère main : +Y aligné sur l'avant-bras (coude→poignet).
    let fwd = (wrist - elbow).normalize_or_zero();
    let hand_rot = Quat::from_rotation_arc(Vec3::Y, fwd);
    let hand = commands
        .spawn((
            Transform::from_translation(wrist).with_rotation(hand_rot),
            Visibility::Inherited,
            Name::new("ViewmodelHand"),
        ))
        .id();
    commands.entity(parent).add_child(hand);

    let mut kids: Vec<Entity> = Vec::with_capacity(8);

    // Avant-bras (manche) : capsule le long de -Y (vers le coude).
    kids.push(
        commands
            .spawn((
                Mesh3d(meshes.add(Capsule3d::new(FOREARM_RADIUS, FOREARM_LEN))),
                MeshMaterial3d(sleeve.clone()),
                Transform::from_xyz(0.0, -FOREARM_LEN * 0.55, 0.0),
                Name::new("Forearm"),
            ))
            .id(),
    );
    // Paume.
    kids.push(
        commands
            .spawn((
                Mesh3d(cuboid(meshes, PALM)),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 0.028, 0.0),
                Name::new("Palm"),
            ))
            .id(),
    );
    // 4 doigts repliés (rotation -X = courbés vers la paume) en avant de la paume.
    let curl = Quat::from_rotation_x(-1.15);
    for i in 0..4 {
        let x = (i as f32 - 1.5) * 0.0165;
        kids.push(
            commands
                .spawn((
                    Mesh3d(cuboid(meshes, FINGER)),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(x, 0.066, 0.012).with_rotation(curl),
                    Name::new("Finger"),
                ))
                .id(),
        );
    }
    // Pouce sur le côté (miroir selon la main), légèrement replié.
    let thumb_rot = Quat::from_rotation_z(mirror * 0.7) * Quat::from_rotation_x(-0.5);
    kids.push(
        commands
            .spawn((
                Mesh3d(cuboid(meshes, THUMB)),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(mirror * 0.042, 0.04, 0.016).with_rotation(thumb_rot),
                Name::new("Thumb"),
            ))
            .id(),
    );

    commands.entity(hand).add_children(&kids);
}

/// Spawn le root des bras (enfant FpsCamera) une fois la caméra présente. Idempotent.
pub fn spawn_arms(
    mut commands: Commands,
    tuning: Res<ViewmodelArmsTuning>,
    q_cam: Query<Entity, With<FpsCamera>>,
    q_arms: Query<(), With<ViewmodelArms>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !tuning.enabled || !q_arms.is_empty() {
        return;
    }
    let Ok(cam) = q_cam.single() else {
        return;
    };

    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(SKIN[0], SKIN[1], SKIN[2]),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..default()
    });
    let sleeve = materials.add(StandardMaterial {
        base_color: Color::srgb(SLEEVE[0], SLEEVE[1], SLEEVE[2]),
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..default()
    });

    let root = commands
        .spawn((
            ViewmodelArms,
            Transform::IDENTITY,
            Visibility::Inherited,
            Name::new("ViewmodelArms"),
        ))
        .id();
    commands.entity(cam).add_child(root);

    spawn_fist(
        &mut commands, root, &mut meshes, &skin, &sleeve, ELBOW_R, WRIST_R, 1.0,
    );
    spawn_fist(
        &mut commands, root, &mut meshes, &skin, &sleeve, ELBOW_L, WRIST_L, -1.0,
    );

    info!("[forgia-viewmodel] poings cartoon procéduraux spawnés (v2)");
}

/// Applique placement réglable (offset + échelle) + offset sway/bob partagé au root.
/// Pose absolue chaque frame → pas d'accumulation. APRÈS `apply_viewmodel_sway_bob`.
pub fn apply_arms_motion(
    tuning: Res<ViewmodelArmsTuning>,
    offset: Res<ViewmodelMotionOffset>,
    mut q: Query<&mut Transform, With<ViewmodelArms>>,
) {
    for mut tf in &mut q {
        tf.translation = Vec3::new(tuning.offset_x, tuning.offset_y, tuning.offset_z)
            + offset.translation;
        tf.rotation = offset.rotation;
        tf.scale = Vec3::splat(tuning.scale.max(0.01));
    }
}

/// Plugin bras : spawn + placement/motion. Gated FPS + Roguelite. Les bras se
/// despawnent automatiquement avec la FpsCamera (enfants).
pub struct ForgiaViewmodelArmsPlugin;

impl Plugin for ForgiaViewmodelArmsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewmodelArmsTuning>().add_systems(
            Update,
            (
                spawn_arms,
                apply_arms_motion.after(apply_viewmodel_sway_bob),
            )
                .run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite))),
        );
    }
}
