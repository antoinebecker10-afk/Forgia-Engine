//! # character.rs — Rex playable character + procedural locomotion (story-438)
//!
//! NOTE 2026-05-17 PM (story-441 v2) : les systèmes de bones/locomotion/IK sont
//! temporairement désactivés (WIP story-440). Rex spawn en bind-pose T-pose
//! statique. Le code conservé sous `#![allow(dead_code)]` pour réactivation
//! rapide quand auto-rig stable.
//!
//! Architecture (cible — actuellement partiellement OFF) :
//! 1. `OnEnter(GameMode::Rpg)` : spawn Rex.glb scene attaché au Player + spawn OrbitCamera
//!    + désactive FpsCamera (Player.children())
//! 2. `Update gated Rpg` :
//!    - `attach_rex_bone_systems` (run jusqu'à ce que cache populated) : BFS scène,
//!      trouve les bones par Name, attache spring chains (queue) + cache locomotion bones
//!    - `procedural_locomotion` : depuis Player velocity, anime jambes/bras/spine
//!    - `procedural_idle_sway` : si vélocité < seuil, sin breathing sur spine
//! 3. `OnExit(GameMode::Rpg)` : despawn Rex + OrbitCamera, ré-active FpsCamera
//!
//! Bone naming : Meshy/AccuRig sortent généralement `mixamorig:*` ou similaire.
//! On essaie plusieurs conventions, on log si rien trouvé.
//!
//! Hot path : la BFS s'arrête après population (LocomotionBoneCache.ready = true).
//! Locomotion = quelques rotations par frame sur ~6 bones, négligeable.

#![allow(dead_code)]

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy_rapier3d::prelude::{QueryFilter, ReadRapierContext};
use forgia_anim_locomotion::{
    LocomotionBoneCache, LocomotionState, LocomotionTarget, ProcBodyAnim,
    AIRBORNE_VY_THRESHOLD, FALL_STRETCH_AMP, IDLE_BREATH_AMP, IDLE_BREATH_FREQ,
    IDLE_SPEED_THRESHOLD, JUMP_SQUASH_AMP, LEAN_FORWARD_AMP, ROLL_WADDLE_AMP,
    WALK_BOB_AMP, WALK_FREQ,
};
use forgia_auto_rig::{AutoRigTemplate, NeedsAutoRig};
use forgia_camera_orbit::OrbitCamera;
use forgia_player::prelude::{FpsCamera, Player};
use std::f32::consts::TAU;

/// Marker de l'entité Scene Rex (enfant du Player).
#[derive(Component)]
pub struct RexCharacter;

/// Marker de la caméra orbit RPG (à despawn en cleanup).
#[derive(Component)]
pub struct RpgOrbitCamera;

/// Cache des entités bones utilisés par la locomotion procédurale.
/// Posé sur Rex SceneRoot. `ready = true` quand la topologie a été analysée.
///
/// Story-440 : on n'utilise plus la BFS-par-Name (fragile cross-rig). On délègue
/// à `forgia-rig-topology` qui classifie par heuristiques 3D positionnelles —
/// marche sur Meshy/AccuRig/Mixamo/Blender sans modifier le GLB.
// Story-482 P1 : types LocomotionBoneCache / BonePose / ArticulatedBones /
// LocomotionState / ProcBodyAnim + tunables WALK_* / LEAN_* / IDLE_* / etc.
// déplacés vers `forgia-anim-locomotion`. Imports en haut du fichier.

// (Story-440) Tables LEFT_THIGH_NAMES/etc retirées : la classification 3D de
// `forgia-rig-topology` fait son boulot sans dépendre des Names. Les Names
// servent juste de boost de score (cf `name_boost` dans le crate).

// ── Spawn / cleanup ─────────────────────────────────────────────────────────

/// Mode de spawn du character RPG. Permet de valider le pipeline auto-rig
/// sur des meshes de complexité croissante avant d'attaquer Rex.glb (Meshy).
///
/// **Méthode** (Antoine 2026-05-17 PM) : tester `HumanoidBlocks` d'abord
/// (mesh propre, pivot Y=0 = sol, 6 Mesh3d primitives connues). Si auto-rig
/// fit correctement le squelette dessus, le pipeline est validé.
///
/// Puis switch à `RexGlb` pour debug spécifique au mesh Meshy (bug Bevy
/// #18921, sub-meshes invisibles, pivot non-standard).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TestCharacterMode {
    /// Mesh Meshy Rex.glb (cas réel). Default 2026-05-17 PM (revert demande Antoine).
    #[default]
    RexGlb,
    /// Humanoid en primitives Bevy (cubes assemblés). Référence baseline propre
    /// pour validation pipeline auto-rig hors bug Bevy #18921 / sub-meshes Meshy.
    /// Activer via `*world.resource_mut::<TestCharacterMode>() = HumanoidBlocks`.
    HumanoidBlocks,
}

/// Spawn character RPG (mode déterminé par `TestCharacterMode`) attaché au
/// Player + OrbitCamera ciblant Player + désactive FpsCamera.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_rex_character(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mode: Res<TestCharacterMode>,
    q_player: Query<Entity, With<Player>>,
    q_existing_rex: Query<(), With<RexCharacter>>,
    mut q_fps_cam: Query<&mut Camera, With<FpsCamera>>,
) {
    // Guard idempotent : ce système tourne en Update (retry-jusqu'à-success)
    // car le spawn Player vit dans un autre plugin OnEnter Rpg — race condition.
    if !q_existing_rex.is_empty() {
        return;
    }
    let Ok(player_entity) = q_player.single() else {
        return;
    };

    // 1. Désactive FpsCamera (la garde en place, juste invisible — re-toggle en sortie RPG)
    for mut cam in &mut q_fps_cam {
        cam.is_active = false;
    }

    // 2. Spawn character selon mode (HumanoidBlocks baseline ou RexGlb).
    match *mode {
        TestCharacterMode::HumanoidBlocks => {
            spawn_humanoid_blocks(
                &mut commands,
                &mut meshes,
                &mut materials,
                player_entity,
            );
        }
        TestCharacterMode::RexGlb => {
            // Story-440 R&D 2026-05-17 night : re-add NeedsAutoRig pour que
            // Pinocchio backend tourne et spawn les bones gizmos. Skinning
            // disable (ne touche pas le mesh visible). ProcBodyAnim/locomotion
            // restent disable. Just bones visualization.
            //
            // Pieds dans le sol : translation Y=-0.85 hardcodée → certains
            // Rex.glb Meshy ont pivot pas exactement à 0.85 sous le mesh top.
            // Fix proprement avec calibration AABB en Phase 4 (foot IK +
            // ground snap). Pour cette session, on accepte le Y offset par défaut.
            commands.entity(player_entity).with_children(|parent| {
                parent.spawn((
                    RexCharacter,
                    // Story-482 P1 : marker exposé par forgia-anim-locomotion.
                    // Découple les queries anim de l'identité RexCharacter
                    // (n'importe quel character avec LocomotionTarget reçoit l'anim).
                    LocomotionTarget,
                    SceneRoot(asset_server.load("models/characters/Rex.glb#Scene0")),
                    Transform::from_xyz(0.0, -0.85, 0.0)
                        .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                    NeedsAutoRig::Template(AutoRigTemplate::Humanoid),
                    LocomotionBoneCache::default(),
                    ProcBodyAnim::default(),
                ));
            });
        }
    }
    // LocomotionState gardé sur le Player : Resource lue par d'autres systèmes
    // (HUD speedometer éventuel) même quand l'animation est désactivée. Coût nul.
    commands
        .entity(player_entity)
        .insert(LocomotionState::default());

    // 3. Spawn OrbitCamera (entité séparée, pas enfant du Player — sinon hérite rotation Y
    //    et le 3P ne ressemble plus à du 3P)
    commands.spawn((
        RpgOrbitCamera,
        Camera3d::default(),
        Camera {
            is_active: true,
            ..default()
        },
        Transform::default(),
        OrbitCamera::new(player_entity),
        Name::new("RpgOrbitCamera"),
    ));

    info!("[forgia-rpg::character] Rex spawned + OrbitCamera active, FpsCamera disabled");
}

// ─── Character lineup (auto-rig tuning playground) ──────────────────────────
//
// 2026-05-18 : spawn 4 personnages humanoïdes (Dorin, Mira, Apprenti,
// Maitre Forgeron) côte à côte derrière le spawn Rex pour démarrer le même
// 2026-05-20 : Kael retiré (mesh identique MD5 à Rex.glb, doublon).
// process de tuning template auto-rig (cf `reference_auto_rig_template_creation_process.md`).
//
// Tous démarrent avec template `Humanoid`. Quand on aura tuné chacun, on
// pourra créer des templates dédiés (Goblin/Orc/Dwarf/Celestial/Human) en
// dupliquant skeleton_humanoid.toml + tweak Y/Z par anatomie.

/// Marker pour les characters du lineup tuning (despawn en cleanup RPG).
#[derive(Component)]
pub struct LineupCharacter;

/// Nom affiché au-dessus du character lineup (render egui world→viewport).
#[derive(Component)]
pub struct LineupName(pub String);

/// État d'attente du spawn lineup. On attend que le Player se stabilise (village
/// teleport peut décaler de 20m+ après spawn), sinon le lineup spawn trop loin.
#[derive(Resource, Default)]
pub struct LineupSpawned {
    pub done: bool,
    /// Position Player observée frame précédente, pour détecter stabilité.
    pub last_player_pos: Vec3,
    /// Frames consécutives où la position du Player n'a pas bougé > 0.05m.
    pub stable_frames: u32,
}

/// Définitions des 5 personnages humanoïdes du lineup auto-rig.
/// (name_display, glb_path, future_dedicated_template_hint).
const LINEUP_CHARACTERS: &[(&str, &str)] = &[
    ("Dorin", "models/characters/Dorin.glb#Scene0"),
    ("Mira", "models/characters/Mira.glb#Scene0"),
    ("Apprenti", "models/characters/L'Apprenti .glb#Scene0"),
    ("MaitreForgeron", "models/characters/Maitre Forgeron Célèste.glb#Scene0"),
];

/// Spawn 5 personnages côte à côte (1.6m d'écart en X) à 4m derrière le spawn
/// player. Chaque character a `NeedsAutoRig::Template(Humanoid)` → bones gizmos
/// visibles → tune template Y/Z dans `skeleton_humanoid.toml` ou créer template
/// dédié quand nécessaire.
///
/// Standalone entities (pas enfants de Player) → ils restent en place quand le
/// player bouge, pour les comparer side-by-side pendant le tuning.
pub(crate) fn spawn_character_lineup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut spawned: ResMut<LineupSpawned>,
    q_player: Query<&Transform, With<Player>>,
) {
    if spawned.done {
        return;
    }
    let Ok(player_tf) = q_player.single() else {
        return; // wait for player spawn
    };

    // Attendre stabilisation du player (village teleport peut le déplacer après
    // spawn). On considère stable après 30 frames consécutives sans mouvement > 5cm.
    let pos = player_tf.translation;
    if (pos - spawned.last_player_pos).length() < 0.05 {
        spawned.stable_frames += 1;
    } else {
        spawned.stable_frames = 0;
    }
    spawned.last_player_pos = pos;
    if spawned.stable_frames < 30 {
        return;
    }

    // Positionnement : 4m derrière le spawn (côté +Z = camera-facing), espacés
    // de 1.6m en X. Y au niveau du spawn pour cohérence avec le terrain.
    let base = pos;
    let spawn_z = base.z + 4.0;
    let spawn_y = base.y - 0.85; // même offset que Rex (mesh.bottom au sol)

    let n = LINEUP_CHARACTERS.len() as f32;
    let total_width = (n - 1.0) * 1.6;
    let start_x = base.x - total_width * 0.5;

    for (i, (name, glb_path)) in LINEUP_CHARACTERS.iter().enumerate() {
        let x = start_x + i as f32 * 1.6;
        commands.spawn((
            LineupCharacter,
            LineupName(name.to_string()),
            SceneRoot(asset_server.load(*glb_path)),
            Transform::from_xyz(x, spawn_y, spawn_z)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            // Tous Humanoid pour l'instant ; créer template dédié per-character au besoin.
            NeedsAutoRig::Template(AutoRigTemplate::Humanoid),
            Name::new(format!("LineupChar_{name}")),
        ));
    }

    spawned.done = true;
    info!(
        "[forgia-rpg::character] Lineup spawned : {} characters at x=[{:.1}..{:.1}] z={:.1} (Humanoid template, around player pos {:?})",
        LINEUP_CHARACTERS.len(),
        start_x,
        start_x + total_width,
        spawn_z,
        base
    );
}

/// Marker + métriques après calibration AABB du lineup.
/// `height` = hauteur réelle du mesh (AABB.max.y - min.y, en repère mesh-local).
#[derive(Component)]
pub struct LineupCalibrated {
    pub height: f32,
}

/// Calibration Y du lineup : aligne `mesh.bottom_y` avec le sol (= player.y - 1.0
/// pour un Player capsule centré). Mesure aussi la hauteur totale du mesh pour
/// que les noms s'affichent proportionnellement à la taille du personnage.
///
/// Retry chaque frame tant que pas calibré (mesh GLB async). Idempotent via
/// marker `LineupCalibrated`.
pub(crate) fn calibrate_lineup_y_and_height(
    mut commands: Commands,
    mut q_lineup: Query<
        (Entity, &mut Transform),
        (With<LineupCharacter>, Without<LineupCalibrated>),
    >,
    children_q: Query<&Children>,
    transforms_q: Query<&Transform, Without<LineupCharacter>>,
    aabbs_q: Query<&Aabb>,
    q_player: Query<&Transform, (With<Player>, Without<LineupCharacter>)>,
) {
    let Ok(player_tf) = q_player.single() else {
        return;
    };
    // Sol = bottom du capsule Player (half_height 0.7 + radius 0.3 = 1.0m).
    let ground_y = player_tf.translation.y - 1.0;

    for (entity, mut char_tf) in &mut q_lineup {
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut found = false;
        let mut stack: Vec<(Entity, Vec3)> = vec![(entity, Vec3::ZERO)];
        while let Some((e, parent_local)) = stack.pop() {
            let local_pos = if e == entity {
                Vec3::ZERO
            } else {
                transforms_q.get(e).map(|t| t.translation).unwrap_or(Vec3::ZERO)
            };
            let mesh_local_pos = parent_local + local_pos;
            if let Ok(aabb) = aabbs_q.get(e) {
                let c: Vec3 = aabb.center.into();
                let h: Vec3 = aabb.half_extents.into();
                min_y = min_y.min(mesh_local_pos.y + c.y - h.y);
                max_y = max_y.max(mesh_local_pos.y + c.y + h.y);
                found = true;
            }
            if let Ok(children) = children_q.get(e) {
                for ch in children.iter() {
                    stack.push((ch, mesh_local_pos));
                }
            }
        }
        if found && min_y.is_finite() && max_y.is_finite() {
            let height = max_y - min_y;
            // Aligne mesh.bottom_y monde avec ground_y :
            //   char_tf.translation.y + min_y = ground_y
            //   ⇒ char_tf.translation.y = ground_y - min_y
            let old_y = char_tf.translation.y;
            char_tf.translation.y = ground_y - min_y;
            commands.entity(entity).insert(LineupCalibrated { height });
            info!(
                "[forgia-rpg::character] Lineup calibrated entity {:?}: min_y={:.3} height={:.3} tf.y {:.3} → {:.3}",
                entity, min_y, height, old_y, char_tf.translation.y
            );
        }
    }
}

/// Rend les noms des characters du lineup au-dessus de leur tête via egui
/// world→viewport. Style chunky outline cartoon, lisible sur tous fonds.
/// Hauteur du texte = `Y character + height + marge` (utilise la hauteur réelle
/// mesurée par `calibrate_lineup_y_and_height`).
pub(crate) fn draw_lineup_names(
    mut contexts: bevy_egui::EguiContexts,
    q_chars: Query<(&Transform, &LineupName, Option<&LineupCalibrated>), With<LineupCharacter>>,
    q_cam: Query<(&Camera, &GlobalTransform), With<OrbitCamera>>,
) {
    use bevy_egui::egui;
    let Ok((cam, cam_tf)) = q_cam.single() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_lineup_names"),
    ));

    for (tf, name, calib) in &q_chars {
        // Position monde au-dessus de la tête : tf.y + height + petite marge.
        // Fallback 2.0m si pas encore calibré (mesh GLB pas loaded).
        let height = calib.map(|c| c.height).unwrap_or(2.0);
        let world_pos = tf.translation + Vec3::Y * (height + 0.05);
        let Ok(screen_pos) = cam.world_to_viewport(cam_tf, world_pos) else {
            continue;
        };

        // Distance-based scale (lisible de loin mais pas envahissant de près).
        let dist = (cam_tf.translation() - world_pos).length();
        let scale = (12.0 / dist.max(2.0)).clamp(0.6, 2.5);
        let font = egui::FontId::proportional(18.0 * scale);

        let pos = egui::pos2(screen_pos.x, screen_pos.y);
        // Outline noir 8 passes pour lisibilité sur tout fond.
        let outline_thickness = (1.5 * scale).max(1.0);
        for (dx, dy) in &[
            (-1.0_f32, -1.0_f32), (0.0, -1.0), (1.0, -1.0),
            (-1.0,  0.0),                       (1.0,  0.0),
            (-1.0,  1.0),         (0.0,  1.0),  (1.0,  1.0),
        ] {
            painter.text(
                egui::pos2(
                    pos.x + dx * outline_thickness,
                    pos.y + dy * outline_thickness,
                ),
                egui::Align2::CENTER_CENTER,
                &name.0,
                font.clone(),
                egui::Color32::from_rgb(8, 8, 12),
            );
        }
        // Texte central — orange Forgia.
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            &name.0,
            font,
            egui::Color32::from_rgb(255, 200, 80),
        );
    }
}

/// Cleanup du lineup OnExit Rpg (despawn entities + reset Resource marker pour
/// le prochain enter).
pub(crate) fn cleanup_character_lineup(
    mut commands: Commands,
    mut spawned: ResMut<LineupSpawned>,
    q: Query<Entity, With<LineupCharacter>>,
) {
    let count = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    spawned.done = false;
    spawned.last_player_pos = Vec3::ZERO;
    spawned.stable_frames = 0;
    if count > 0 {
        info!(
            "[forgia-rpg::character] Lineup cleaned : {} characters despawned",
            count
        );
    }
}

/// Marker idempotence pour `calibrate_rex_y_one_shot`.
#[derive(Component)]
pub struct RexYCalibrated;

/// **Fix "Rex pieds dans le sol"** : 1-shot après spawn, walk descendants de
/// Rex pour trouver `min_y` du AABB en repère Rex-local, ajuste
/// `rex_tf.translation.y = -1.0 - min_y` pour aligner mesh.bottom au bottom
/// du Player capsule (Y=-1.0 en repère Player).
///
/// Retry chaque frame tant que pas calibré (mesh GLB asynchrone). Idempotent
/// via marker `RexYCalibrated`.
pub(crate) fn calibrate_rex_y_one_shot(
    mut commands: Commands,
    mut q_rex: Query<
        (Entity, &mut Transform),
        (With<RexCharacter>, Without<RexYCalibrated>),
    >,
    children_q: Query<&Children>,
    transforms_q: Query<&Transform, Without<RexCharacter>>,
    aabbs_q: Query<&Aabb>,
) {
    for (rex_entity, mut rex_tf) in &mut q_rex {
        let mut min_y = f32::INFINITY;
        let mut found = false;
        let mut stack: Vec<(Entity, Vec3)> = vec![(rex_entity, Vec3::ZERO)];
        while let Some((entity, parent_world)) = stack.pop() {
            let local_pos = if entity == rex_entity {
                Vec3::ZERO
            } else {
                transforms_q
                    .get(entity)
                    .map(|t| t.translation)
                    .unwrap_or(Vec3::ZERO)
            };
            let world_pos = parent_world + local_pos;
            if let Ok(aabb) = aabbs_q.get(entity) {
                let c: Vec3 = aabb.center.into();
                let h: Vec3 = aabb.half_extents.into();
                min_y = min_y.min(world_pos.y + c.y - h.y);
                found = true;
            }
            if let Ok(children) = children_q.get(entity) {
                for c in children.iter() {
                    stack.push((c, world_pos));
                }
            }
        }
        if found && min_y.is_finite() {
            // Player capsule : half_height=0.7 + radius=0.3 → bottom Y=-1.0
            let target_bottom = -1.0_f32;
            let old_y = rex_tf.translation.y;
            rex_tf.translation.y = target_bottom - min_y;
            commands.entity(rex_entity).insert(RexYCalibrated);
            info!(
                "[forgia-rpg::character] Rex Y calibrated: aabb_min_y={:.3} → tf.y {:.3} → {:.3}",
                min_y,
                old_y,
                rex_tf.translation.y
            );
        }
    }
}

/// Marker idempotence pour `rex_make_transparent_one_shot`.
#[derive(Component)]
pub struct RexMaterialTransparent;

/// **Rend Rex légèrement transparent** pour visualiser le squelette gizmos à
/// travers le mesh. Clone les `StandardMaterial` des Mesh3d descendants de
/// Rex, set `alpha = 0.40` + `alpha_mode = Blend`. Idempotent via marker.
pub(crate) fn rex_make_transparent_one_shot(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_rex: Query<Entity, With<RexCharacter>>,
    children_q: Query<&Children>,
    q_mat: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>),
        Without<RexMaterialTransparent>,
    >,
) {
    let Ok(rex_entity) = q_rex.single() else {
        return;
    };

    let mut stack = vec![rex_entity];
    let mut applied = 0usize;
    while let Some(e) = stack.pop() {
        if let Ok((entity, mat_handle)) = q_mat.get(e) {
            if let Some(mat) = materials.get(&mat_handle.0) {
                let mut new_mat = mat.clone();
                new_mat.base_color = new_mat.base_color.with_alpha(0.40);
                new_mat.alpha_mode = AlphaMode::Blend;
                let new_handle = materials.add(new_mat);
                commands.entity(entity).insert((
                    MeshMaterial3d(new_handle),
                    RexMaterialTransparent,
                ));
                applied += 1;
            }
        }
        if let Ok(children) = children_q.get(e) {
            for c in children.iter() {
                stack.push(c);
            }
        }
    }
    if applied > 0 {
        info!(
            "[forgia-rpg::character] Rex transparency applied on {} materials (alpha=0.40)",
            applied
        );
    }
}

/// Spawn un humanoid baseline en primitives Bevy (cubes assemblés) pour
/// valider le pipeline auto-rig sur un mesh propre, à pivot connu Y=0=sol.
///
/// **Dimensions humain ~1.75m T-pose** :
/// - Pieds : Y=0 (base mesh-local)
/// - Jambes : Y 0 → 0.85 (Cuboid 0.15 × 0.85 × 0.15, centre Y=0.425, X=±0.10)
/// - Torse : Y 0.85 → 1.45 (Cuboid 0.40 × 0.60 × 0.20, centre Y=1.15)
/// - Tête : Y 1.45 → 1.75 (Cuboid 0.24 × 0.30 × 0.24, centre Y=1.60)
/// - Bras T-pose : Y=1.30, X=±0.20 → ±0.85 (Cuboid 0.65 × 0.10 × 0.10)
///
/// hip réel attendu (jonction jambes/torse) : Y=0.85, soit hip_y_frac=0.486
/// head sommet : Y=1.75, soit head_y_frac=1.0
/// arm_span_half_frac : 0.85/1.75 = 0.486
fn spawn_humanoid_blocks(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    player_entity: Entity,
) {
    let mat_body = materials.add(StandardMaterial {
        base_color: Color::srgb(0.65, 0.45, 0.30),
        perceptual_roughness: 0.85,
        ..default()
    });
    let mat_head = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.65, 0.50),
        perceptual_roughness: 0.85,
        ..default()
    });

    let mesh_torso = meshes.add(Cuboid::new(0.40, 0.60, 0.20));
    let mesh_head = meshes.add(Cuboid::new(0.24, 0.30, 0.24));
    let mesh_leg = meshes.add(Cuboid::new(0.15, 0.85, 0.15));
    let mesh_arm = meshes.add(Cuboid::new(0.65, 0.10, 0.10));

    commands.entity(player_entity).with_children(|parent| {
        parent
            .spawn((
                RexCharacter,
                // Pivot mesh-local Y=0 = pieds. Pour aligner pieds sur le sol
                // Player capsule (bottom Y=-1.0 dans repère Player), translation Y=-1.0.
                Transform::from_xyz(0.0, -1.0, 0.0),
                Visibility::default(),
                InheritedVisibility::default(),
                LocomotionBoneCache::default(),
                ProcBodyAnim {
                    lean_smooth: Vec3::ZERO,
                    ..default()
                },
                // Auto-rig should auto-switch to Humanoid via landmarks
                NeedsAutoRig::Template(AutoRigTemplate::Humanoid),
                Name::new("TestHumanoidBlocks"),
            ))
            .with_children(|root| {
                // Torso (Y center 1.15)
                root.spawn((
                    Mesh3d(mesh_torso.clone()),
                    MeshMaterial3d(mat_body.clone()),
                    Transform::from_xyz(0.0, 1.15, 0.0),
                    Name::new("torso"),
                ));
                // Head (Y center 1.60)
                root.spawn((
                    Mesh3d(mesh_head.clone()),
                    MeshMaterial3d(mat_head.clone()),
                    Transform::from_xyz(0.0, 1.60, 0.0),
                    Name::new("head"),
                ));
                // Leg L (X=-0.10, Y=0.425)
                root.spawn((
                    Mesh3d(mesh_leg.clone()),
                    MeshMaterial3d(mat_body.clone()),
                    Transform::from_xyz(-0.10, 0.425, 0.0),
                    Name::new("leg_L"),
                ));
                // Leg R (X=+0.10, Y=0.425)
                root.spawn((
                    Mesh3d(mesh_leg.clone()),
                    MeshMaterial3d(mat_body.clone()),
                    Transform::from_xyz(0.10, 0.425, 0.0),
                    Name::new("leg_R"),
                ));
                // Arm L (X=-0.525, Y=1.30, T-pose)
                root.spawn((
                    Mesh3d(mesh_arm.clone()),
                    MeshMaterial3d(mat_body.clone()),
                    Transform::from_xyz(-0.525, 1.30, 0.0),
                    Name::new("arm_L"),
                ));
                // Arm R (X=+0.525, Y=1.30, T-pose)
                root.spawn((
                    Mesh3d(mesh_arm.clone()),
                    MeshMaterial3d(mat_body.clone()),
                    Transform::from_xyz(0.525, 1.30, 0.0),
                    Name::new("arm_R"),
                ));
            });
    });

    info!("[forgia-rpg::character] HumanoidBlocks spawned (baseline pipeline test, mesh_height=1.75m)");
}

/// Cleanup OnExit Rpg : despawn Rex visuel + OrbitCamera, ré-active FpsCamera.
pub(crate) fn cleanup_rex_character(
    mut commands: Commands,
    q_rex: Query<Entity, With<RexCharacter>>,
    q_orbit: Query<Entity, With<RpgOrbitCamera>>,
    q_player: Query<Entity, With<Player>>,
    mut q_fps_cam: Query<&mut Camera, With<FpsCamera>>,
) {
    // BUG-ANIMQA-03 audit : Bevy 0.18 `despawn()` est RÉCURSIF par défaut depuis 0.16
    // (despawn_recursive a été déprécié). La hiérarchie Scene GLB + SpringBone children
    // est donc bien cleanup. La memory V1 `reference_bevy_018_scene_spawner_cancel` date
    // de Bevy 0.16-, plus applicable. Vérifié docs Bevy 0.18 EntityCommands::despawn.
    for e in &q_rex {
        commands.entity(e).despawn();
    }
    for e in &q_orbit {
        commands.entity(e).despawn();
    }
    if let Ok(player) = q_player.single() {
        commands
            .entity(player)
            .remove::<LocomotionState>();
    }
    for mut cam in &mut q_fps_cam {
        cam.is_active = true;
    }
    info!("[forgia-rpg::character] Rex despawned, FpsCamera re-enabled");
}

// ── Procedural whole-body animation (rigless fallback) ───────────────────────
//
// Marche / saute / cours visibles SANS squelette : on anime le Transform root
// du Rex (bob Y, lean forward, waddle roll, jump squash/stretch, idle breath).
// Tant que Rex.glb est statique, c'est le seul moyen d'avoir du feedback visuel.
// Quand Rex sera rigged, ce système restera comme couche root-motion par-dessus
// le walk cycle par-bone.

pub(crate) fn procedural_whole_body_anim(
    time: Res<Time>,
    q_player: Query<(Entity, &Transform, &Player, &LocomotionState), Without<RexCharacter>>,
    mut q_rex: Query<
        (&mut Transform, &mut ProcBodyAnim, &LocomotionBoneCache),
        With<RexCharacter>,
    >,
    rapier: ReadRapierContext,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((player_entity, player_tf, player, locomotion)) = q_player.single() else {
        return;
    };
    let speed = locomotion.speed;
    let vy = player.vertical_velocity;
    let airborne = vy.abs() > AIRBORNE_VY_THRESHOLD;
    let is_moving = speed > IDLE_SPEED_THRESHOLD;
    let speed_factor = (speed / 3.0).clamp(0.0, 1.5);

    for (mut rex_tf, mut anim, cache) in &mut q_rex {
        if !cache.calibrated {
            continue;
        }
        // Snapshot base_y au premier passage post-calibration.
        if !anim.initialized {
            anim.base_y = rex_tf.translation.y;
            anim.lean_smooth = Vec3::new(0.0, std::f32::consts::PI, 0.0);
            anim.initialized = true;
        }

        // ── Walk phase ─────────────────────────────────────────────────────
        if is_moving && !airborne {
            anim.walk_phase = (anim.walk_phase + dt * speed * WALK_FREQ) % TAU;
        }

        // ── Bob Y target ───────────────────────────────────────────────────
        let target_bob = if airborne {
            // En montée : squash (compress vers le bas).
            // En chute : stretch (allongement vers le haut).
            if vy > 0.0 {
                -JUMP_SQUASH_AMP * (vy / 6.0).clamp(0.0, 1.0)
            } else {
                FALL_STRETCH_AMP * (-vy / 8.0).clamp(0.0, 1.0)
            }
        } else if is_moving {
            // Walk bob : |sin(2φ)| produit 2 oscillations par cycle (pied gauche+droit).
            (anim.walk_phase * 2.0).sin().abs() * WALK_BOB_AMP * speed_factor
        } else {
            // Idle breathing très subtil.
            (time.elapsed_secs() * IDLE_BREATH_FREQ).sin() * IDLE_BREATH_AMP * 0.5
        };
        anim.bob_smooth = anim.bob_smooth * 0.75 + target_bob * 0.25;

        // ── Ground hug raycast ─────────────────────────────────────────────
        // Cast 1m au-dessus du Player vers le bas, exclut le collider Player.
        // L'offset target = "écart entre la position du mesh.bottom et le terrain réel".
        // En l'air : on lerp vers 0 (pas de hug).
        let target_ground_hug = if !airborne {
            if let Ok(ctx) = rapier.single() {
                let origin = player_tf.translation + Vec3::new(0.0, 1.0, 0.0);
                let filter = QueryFilter::default().exclude_collider(player_entity);
                if let Some((_, toi)) = ctx.cast_ray(origin, Vec3::NEG_Y, 6.0, true, filter) {
                    let hit_world_y = origin.y - toi;
                    // mesh.bottom_world = player_tf.y - 1.0 quand ground_hug=0 (capsule rest).
                    // On veut mesh.bottom_world = hit_world_y.
                    // delta = hit_world_y - (player_tf.y - 1.0) = hit_world_y - player_tf.y + 1.0
                    let delta = hit_world_y - player_tf.translation.y + 1.0;
                    // Clamp pour éviter spikes (téléportation, terrain manquant) :
                    // positif = terrain plus haut que capsule bottom (montée slope) → ok
                    // négatif = terrain en dessous (gap, falaise) → ne pas dangler les pieds
                    delta.clamp(0.0, 0.4)
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };
        anim.ground_hug_smooth = anim.ground_hug_smooth * 0.85 + target_ground_hug * 0.15;

        rex_tf.translation.y = anim.base_y + anim.bob_smooth + anim.ground_hug_smooth;

        // ── Lean / Roll (rotation) ─────────────────────────────────────────
        let target_lean_x = if is_moving && !airborne {
            -LEAN_FORWARD_AMP * speed_factor
        } else if airborne && vy > 0.0 {
            -0.10 // léger tuck forward en montée
        } else {
            0.0
        };
        let target_roll_z = if is_moving && !airborne {
            anim.walk_phase.sin() * ROLL_WADDLE_AMP * speed_factor
        } else {
            0.0
        };
        // Y reste fixé à PI (mesh face -Z = dos à la cam).
        let target_euler = Vec3::new(target_lean_x, std::f32::consts::PI, target_roll_z);
        anim.lean_smooth = anim.lean_smooth.lerp(target_euler, 0.18);
        rex_tf.rotation = Quat::from_euler(
            EulerRot::YXZ,
            anim.lean_smooth.y,
            anim.lean_smooth.x,
            anim.lean_smooth.z,
        );
    }
}

