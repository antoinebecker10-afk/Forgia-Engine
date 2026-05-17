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
use forgia_anim_debug::{AnimLayerStats, AnimTimer};
use forgia_auto_rig::{AutoRigTemplate, NeedsAutoRig};
use forgia_camera_orbit::OrbitCamera;
use forgia_player::prelude::{FpsCamera, Player};
use forgia_rig_topology::{analyze_rig_topology, RigTopology};
use forgia_secondary_motion::{SpringBone, SpringBoneChain};
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
#[derive(Component, Default)]
pub struct LocomotionBoneCache {
    pub topology: RigTopology,
    pub ready: bool,
    /// Frames écoulées depuis spawn — sert au timeout diagnostic et au giveup
    /// "no skeleton detected after N frames".
    pub frames_waited: u32,
    /// True une fois la calibration Y (AABB-based) appliquée.
    pub calibrated: bool,
    /// True une fois qu'on a logué "no skeleton" pour éviter le spam.
    pub gave_up: bool,
}

/// Vitesse précédente du Player (frame n-1), pour estimer la vélocité.
/// `Local` ne convient pas car il faut survivre les transitions de state.
#[derive(Component, Default)]
pub struct LocomotionState {
    pub prev_pos: Vec3,
    pub speed: f32,
    pub gait_phase: f32,
}

/// Animation procédurale whole-body — placeholder pour Rex.glb statique (sans
/// squelette). Bob walk, lean forward, waddle roll, jump squash-stretch, idle
/// breathing — tout appliqué sur le Transform du Rex root (pas de bones requis).
///
/// Le jour où Rex.glb est rigged (Meshy Auto-Rig), `forgia-rig-topology` + le
/// walk cycle par-bone prendront le relais ; ce ProcBodyAnim restera utile
/// comme couche de "déplacement racine" (root motion forgia-style).
#[derive(Component, Default)]
pub struct ProcBodyAnim {
    /// Phase walk (TAU-modulo), incrémentée par dt*speed.
    pub walk_phase: f32,
    /// Y de base (post-calibration AABB). Le bob s'ajoute par-dessus.
    pub base_y: f32,
    /// True après snapshot de base_y au 1er run post-calibration.
    pub initialized: bool,
    /// Bob offset lissé (smooth en sortie de loop pour éviter snap).
    pub bob_smooth: f32,
    /// Rotation euler lissée (yaw, pitch, roll en repère Rex local).
    pub lean_smooth: Vec3,
    /// Ground hug offset lissé : correction Y pour aligner mesh.bottom sur le
    /// terrain via raycast vertical. Reset à 0 quand airborne.
    pub ground_hug_smooth: f32,
}

// ── Procedural body anim tunables ───────────────────────────────────────────
/// Vitesse pas (cycles par mètre parcouru × TAU).
const WALK_FREQ: f32 = 2.5;
/// Amplitude bob Y (m) à speed_factor=1.0.
const WALK_BOB_AMP: f32 = 0.07;
/// Lean forward max (rad) à speed_factor=1.0.
const LEAN_FORWARD_AMP: f32 = 0.18;
/// Roll waddle (rad) — sin(phase).
const ROLL_WADDLE_AMP: f32 = 0.06;
/// Squash en montée (compress Y, scale).
const JUMP_SQUASH_AMP: f32 = 0.08;
/// Stretch en chute libre (extend Y).
const FALL_STRETCH_AMP: f32 = 0.06;
/// Velocity Y absolue au-delà de laquelle on considère airborne.
const AIRBORNE_VY_THRESHOLD: f32 = 0.8;

// ── Idle / movement detection ───────────────────────────────────────────────
// Sprint A (story-440) : les tunables walk cycle vivent maintenant dans
// `crate::proc_walk` (anatomy-driven : stance/swing 60-40, pelvic yaw+bob 2× freq,
// knee cloche, transitions walk→run). Ici on garde juste les seuils idle.
const IDLE_SPEED_THRESHOLD: f32 = 0.15;  // m/s en dessous = idle breathing
const IDLE_BREATH_FREQ: f32 = 1.2;
const IDLE_BREATH_AMP: f32 = 0.03;

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
                    SceneRoot(asset_server.load("models/characters/Rex.glb#Scene0")),
                    Transform::from_xyz(0.0, -0.85, 0.0)
                        .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                    NeedsAutoRig::Template(AutoRigTemplate::BipedLizard),
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

// ── Topology-based bone discovery (story-440) ───────────────────────────────

/// Analyse topologie + calibre Y via AABB. Retry chaque frame jusqu'à ce que
/// la Scene Bevy ait instancié des enfants. Si après `GIVEUP_FRAMES`, toujours
/// rien → log explicite "no skeleton" pour diagnostic (le mesh est statique).
const GIVEUP_FRAMES: u32 = 120;

pub(crate) fn attach_rex_bone_systems(
    mut commands: Commands,
    mut q_cache: Query<(Entity, &mut LocomotionBoneCache, &mut Transform), With<RexCharacter>>,
    children_query: Query<&Children>,
    names: Query<&Name>,
    transforms: Query<&Transform, Without<RexCharacter>>,
    aabbs: Query<&Aabb>,
) {
    for (rex_entity, mut cache, mut rex_tf) in &mut q_cache {
        if cache.ready || cache.gave_up {
            continue;
        }
        cache.frames_waited += 1;

        // ── Pass 1 : calibration Y via AABB (indépendante du squelette) ──────
        // Walk descendants pour accumuler bounding box en repère Rex-local.
        if !cache.calibrated {
            let mut min_y = f32::INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            let mut count = 0usize;

            let mut stack: Vec<(Entity, Vec3)> = vec![(rex_entity, Vec3::ZERO)];
            while let Some((e, parent_world)) = stack.pop() {
                let local_pos = transforms
                    .get(e)
                    .map(|t| t.translation)
                    .unwrap_or(Vec3::ZERO);
                let world_pos = parent_world + local_pos;
                if let Ok(aabb) = aabbs.get(e) {
                    let c: Vec3 = aabb.center.into();
                    let h: Vec3 = aabb.half_extents.into();
                    min_y = min_y.min(world_pos.y + c.y - h.y);
                    max_y = max_y.max(world_pos.y + c.y + h.y);
                    count += 1;
                }
                if let Ok(children) = children_query.get(e) {
                    for c in children.iter() {
                        stack.push((c, world_pos));
                    }
                }
            }

            if count > 0 && min_y.is_finite() && max_y.is_finite() {
                // Player capsule : half_height=0.7, radius=0.3 → bottom à -1.0 (Player-frame).
                // `min_y` est dans le repère Rex AVANT translation (la stack démarre à
                // ZERO, le Transform de Rex est exclu via Without<RexCharacter>).
                // => mesh_bottom_in_player_frame = rex_tf.y + min_y. On veut == -1.0.
                // => rex_tf.y = -1.0 - min_y (ASSIGNMENT, pas +=).
                let target_bottom = -1.0_f32;
                let new_y = target_bottom - min_y;
                let old_y = rex_tf.translation.y;
                rex_tf.translation.y = new_y;
                info!(
                    "[forgia-rpg::character] AABB calibrated: aabb_y_local=[{:.3}..{:.3}] count={} → tf.y {:.3} → {:.3} (delta {:+.3})",
                    min_y, max_y, count, old_y, new_y, new_y - old_y
                );
                cache.calibrated = true;
            }
        }

        // ── Pass 2 : topology analysis ───────────────────────────────────────
        let children_of = |e: Entity| {
            children_query
                .get(e)
                .map(|c| c.iter().collect())
                .unwrap_or_default()
        };
        let transform_of = |e: Entity| transforms.get(e).ok().copied();
        let name_of = |e: Entity| names.get(e).ok().map(|n| n.to_string());

        let topo = analyze_rig_topology(rex_entity, &children_of, &transform_of, &name_of);

        // Au 1er run où on a >0 enfants ET >0 bones diagnostiqués, on dump TOUT
        // (utile pour comprendre pourquoi is_usable() est false sur un rig exotique)
        if !cache.gave_up && !topo.diagnostics.is_empty() && cache.frames_waited <= 10 {
            info!(
                "[forgia-rpg::character] Skeleton scan: {} bones found, is_usable={}",
                topo.diagnostics.len(),
                topo.is_usable()
            );
            for diag in topo.diagnostics.iter().take(60) {
                info!(
                    "  bone '{}' depth={} pos=({:.2}, {:.2}, {:.2}) children={}",
                    diag.name, diag.depth, diag.local_pos.x, diag.local_pos.y, diag.local_pos.z, diag.child_count
                );
            }
        }

        if topo.is_usable() {
            // Spring chain queue
            if topo.tail_chain.len() >= 2 {
                let followers: Vec<Entity> = topo.tail_chain[1..].to_vec();
                for &bone in &followers {
                    commands.entity(bone).insert(SpringBone {
                        stiffness: 0.35,
                        damping: 0.65,
                        gravity: Vec3::new(0.0, -3.0, 0.0),
                    });
                }
                commands.entity(topo.tail_chain[0]).insert(SpringBoneChain {
                    bones: followers,
                    ..default()
                });
                info!(
                    "[forgia-rpg::character] Tail spring attached: chain_len={}",
                    topo.tail_chain.len()
                );
            }

            info!(
                "[forgia-rpg::character] RigTopology classified: legs L/R={}/{}, arms L/R={}/{}, spine={}, head={}, tail={}",
                topo.left_leg.is_some(),
                topo.right_leg.is_some(),
                topo.left_arm.is_some(),
                topo.right_arm.is_some(),
                topo.spine.is_some(),
                topo.head.is_some(),
                topo.tail_chain.len(),
            );
            cache.topology = topo;
            cache.ready = true;
        } else if cache.frames_waited >= GIVEUP_FRAMES && !cache.gave_up {
            warn!(
                "[forgia-rpg::character] No usable skeleton after {} frames — Rex.glb is likely a STATIC mesh (no armature). \
                 Procedural animation IMPOSSIBLE without a rigged GLB. Action: re-export Rex via Meshy with rigging enabled \
                 (Meshy → Generate → Animation tab → Auto-Rig humanoid or quadruped).",
                cache.frames_waited
            );
            cache.gave_up = true;
        }
    }
}

// ── Procedural locomotion (walk cycle + idle breathing) ─────────────────────

/// Walk cycle anatomique (Sprint A story-440) :
/// - Bugs #1-8 du rapport recherche corrigés : knee/elbow séparés, stance/swing
///   60/40 + cloche knee, pelvic yaw+roll+bob 2× freq, spine counter-rot Y
///   (pas Z), tail Rex counter-balance, walk→run transitions via `GaitTunables`.
/// - Fonctions pures dans `proc_walk` (12 tests headless anatomiques).
/// - Bones secondaires (shin, foot, forearm, tail) résolus via Children
///   queries depuis la topology (cohérent avec hiérarchie BipedLizard template).
#[allow(clippy::too_many_arguments)]
pub(crate) fn procedural_locomotion(
    time: Res<Time>,
    mut q_player: Query<(&Transform, &mut LocomotionState), With<Player>>,
    q_cache: Query<&LocomotionBoneCache, With<RexCharacter>>,
    q_children: Query<&Children>,
    mut bones: Query<&mut Transform, Without<Player>>,
    mut stats: ResMut<AnimLayerStats>,
) {
    let timer = AnimTimer::start();
    stats.locomotion_active = true;

    let dt = time.delta_secs();
    if dt <= 0.0 {
        stats.locomotion_us = timer.elapsed_us();
        return;
    }

    let Ok((player_tf, mut state)) = q_player.single_mut() else {
        stats.locomotion_active = false;
        stats.locomotion_us = timer.elapsed_us();
        return;
    };
    // ── Speed estimation : DOIT tourner même sans squelette ────────────────
    let pos = player_tf.translation;
    let velocity = (pos - state.prev_pos) / dt;
    let horiz_vel = Vec3::new(velocity.x, 0.0, velocity.z);
    let speed_now = horiz_vel.length();
    state.speed = state.speed * 0.85 + speed_now * 0.15;
    state.prev_pos = pos;
    let speed = state.speed;
    let is_moving = speed > IDLE_SPEED_THRESHOLD;
    stats.locomotion_speed = speed;
    stats.locomotion_is_moving = is_moving;

    let Ok(cache) = q_cache.single() else {
        stats.locomotion_us = timer.elapsed_us();
        return;
    };
    stats.locomotion_cache_ready = cache.ready;
    if !cache.ready {
        // Pas de bones : speed update fait, walk cycle skip
        stats.locomotion_us = timer.elapsed_us();
        return;
    }

    if !is_moving {
        // Idle breathing — léger sin sur X du spine + reset slerp jambes/bras
        let t_secs = time.elapsed_secs();
        let breath = (t_secs * IDLE_BREATH_FREQ).sin() * IDLE_BREATH_AMP;
        if let Some(e) = cache.topology.spine {
            if let Ok(mut tf) = bones.get_mut(e) {
                tf.rotation = Quat::from_rotation_x(breath);
            }
        }
        for e in [
            cache.topology.left_leg,
            cache.topology.right_leg,
            cache.topology.left_arm,
            cache.topology.right_arm,
        ]
        .into_iter()
        .flatten()
        {
            if let Ok(mut tf) = bones.get_mut(e) {
                tf.rotation = tf.rotation.slerp(Quat::IDENTITY, 0.15);
            }
        }
        stats.locomotion_gait_phase = state.gait_phase;
        stats.locomotion_us = timer.elapsed_us();
        return;
    }

    // ── Walk cycle anatomique (Sprint A) ────────────────────────────────────
    let tunables = crate::proc_walk::GaitTunables::for_speed(speed);
    state.gait_phase =
        crate::proc_walk::update_gait_phase(state.gait_phase, speed, dt, &tunables);
    let gait = state.gait_phase;
    // speed_factor : 0 below idle threshold, 1 at walk speed peak, capped 1.2 in run
    let speed_factor =
        ((speed - IDLE_SPEED_THRESHOLD) / crate::proc_walk::SPEED_WALK_PEAK_M_S).clamp(0.0, 1.2);

    let topo = &cache.topology;

    // Resolve secondary bones via Children walk (Phase 1A template : thigh→shin→foot,
    // arm→forearm). Helper local pour éviter répétition.
    let first_child = |e: Entity| -> Option<Entity> {
        q_children.get(e).ok().and_then(|c| c.iter().next())
    };
    let shin_l = topo.left_leg.and_then(first_child);
    let shin_r = topo.right_leg.and_then(first_child);
    let foot_l = shin_l.and_then(first_child);
    let foot_r = shin_r.and_then(first_child);
    let forearm_l = topo.left_arm.and_then(first_child);
    let forearm_r = topo.right_arm.and_then(first_child);

    // Leg poses (L on gait, R déphasé 0.5)
    let (thigh_l, knee_l, ankle_l) = crate::proc_walk::leg_pose(gait, &tunables);
    let (thigh_r, knee_r, ankle_r) =
        crate::proc_walk::leg_pose((gait + 0.5).rem_euclid(1.0), &tunables);

    apply_pitch(&mut bones, topo.left_leg, thigh_l * speed_factor);
    apply_pitch(&mut bones, topo.right_leg, thigh_r * speed_factor);
    apply_pitch(&mut bones, shin_l, knee_l * speed_factor);
    apply_pitch(&mut bones, shin_r, knee_r * speed_factor);
    apply_pitch(&mut bones, foot_l, ankle_l * speed_factor);
    apply_pitch(&mut bones, foot_r, ankle_r * speed_factor);

    // Arm poses (sub-phase = leg opposée)
    let (arm_l_pitch, elbow_l) =
        crate::proc_walk::arm_pose((gait + 0.5).rem_euclid(1.0), &tunables);
    let (arm_r_pitch, elbow_r) = crate::proc_walk::arm_pose(gait, &tunables);
    apply_pitch(&mut bones, topo.left_arm, arm_l_pitch * speed_factor);
    apply_pitch(&mut bones, topo.right_arm, arm_r_pitch * speed_factor);
    apply_pitch(&mut bones, forearm_l, elbow_l * speed_factor);
    apply_pitch(&mut bones, forearm_r, elbow_r * speed_factor);

    // Pelvic : yaw + roll + bob Y (sur le hip = topo.root) — bob translation, yaw+roll rotation
    let (pelvic_yaw, pelvic_roll, _bob_y) =
        crate::proc_walk::pelvic_pose(gait, speed_factor, &tunables);
    if let Some(hip) = topo.root {
        if let Ok(mut tf) = bones.get_mut(hip) {
            tf.rotation =
                Quat::from_rotation_y(pelvic_yaw) * Quat::from_rotation_z(pelvic_roll);
            // bob_y appliqué sur le Rex root par procedural_whole_body_anim (déjà en place).
        }
    }

    // Spine counter-rotation Y (fix bug #6 : était sur Z)
    if let Some(e) = topo.spine {
        if let Ok(mut tf) = bones.get_mut(e) {
            tf.rotation = Quat::from_rotation_y(crate::proc_walk::spine_counter_rot(pelvic_yaw));
        }
    }

    // Tail Rex counter-balance (chaîne tail_chain[0..])
    for (idx, &tail_seg) in topo.tail_chain.iter().enumerate() {
        if let Ok(mut tf) = bones.get_mut(tail_seg) {
            let yaw = crate::proc_walk::tail_segment_yaw(idx, topo.tail_chain.len(), pelvic_yaw);
            tf.rotation = Quat::from_rotation_y(yaw);
        }
    }

    stats.locomotion_gait_phase = state.gait_phase;
    stats.locomotion_us = timer.elapsed_us();
    // silence unused if procedural_whole_body_anim absent
    let _ = TAU;
}

/// Applique une rotation pitch (X) à un bone optionnel. Helper pour réduire la
/// répétition du pattern `if let Some(e) = ... { if let Ok(mut tf) = bones.get_mut(e) ... }`.
#[inline]
fn apply_pitch(bones: &mut Query<&mut Transform, Without<Player>>, e: Option<Entity>, pitch: f32) {
    if let Some(e) = e {
        if let Ok(mut tf) = bones.get_mut(e) {
            tf.rotation = Quat::from_rotation_x(pitch);
        }
    }
}

// ── Sensor walk pose (Vague 3 story-440) ────────────────────────────────────

const WALK_POSE_SENSOR_PATH: &str = "forgia_walk_pose.json";
const WALK_POSE_SENSOR_INTERVAL_S: f32 = 0.5;

/// Timer dédié pour l'export `forgia_walk_pose.json`.
#[derive(Resource, Default)]
pub(crate) struct WalkPoseSensorTimer {
    accum_s: f32,
}

/// Système sensor : écrit `forgia_walk_pose.json` toutes les 0.5s. Permet à
/// Antoine de vérifier en temps réel que le walk cycle produit des angles
/// anatomiquement cohérents (jambes opposées, knee plié seulement en swing,
/// bob Y 2× freq, etc.).
///
/// **Comment l'utiliser pour débug** :
/// - Lance le jeu, entre en RPG, marche en ligne droite ~2s
/// - `cat forgia_walk_pose.json` → check `thigh_l_deg ≈ -thigh_r_deg` (opposés)
/// - check `knee_l_deg > 0` seulement quand `gait_phase ∈ [0.6, 1.0]` (swing L)
/// - check `bob_y_cm` oscille entre 0 et ~4cm
pub(crate) fn write_walk_pose_sensor(
    time: Res<Time>,
    mut timer: ResMut<WalkPoseSensorTimer>,
    q_state: Query<&LocomotionState, With<Player>>,
) {
    let dt = time.delta_secs();
    timer.accum_s += dt;
    if timer.accum_s < WALK_POSE_SENSOR_INTERVAL_S {
        return;
    }
    timer.accum_s = 0.0;

    let Ok(state) = q_state.single() else {
        return;
    };

    let speed = state.speed;
    let gait = state.gait_phase;
    let speed_factor = ((speed - IDLE_SPEED_THRESHOLD)
        / crate::proc_walk::SPEED_WALK_PEAK_M_S)
        .clamp(0.0, 1.2);
    let snap = crate::proc_walk::WalkPoseSnapshot::from_gait(gait, speed, speed_factor);

    let is_moving = speed > IDLE_SPEED_THRESHOLD;
    // Quelle jambe est en swing ?
    let tunables = crate::proc_walk::GaitTunables::for_speed(speed);
    let leg_l_in_swing = gait >= tunables.stance_frac;
    let leg_r_sub = (gait + 0.5).rem_euclid(1.0);
    let leg_r_in_swing = leg_r_sub >= tunables.stance_frac;

    let json = format!(
        r#"{{
  "timestamp_secs": {:.1},
  "is_moving": {},
  "speed_m_s": {:.3},
  "speed_factor": {:.3},
  "gait_phase": {:.3},
  "tunables": {{
    "stride_per_m": {:.3},
    "stance_frac": {:.3},
    "amp_thigh_rad": {:.3},
    "knee_flex_peak_rad": {:.3}
  }},
  "legs": {{
    "left_in_swing": {},
    "right_in_swing": {},
    "thigh_l_deg": {:.1},
    "thigh_r_deg": {:.1},
    "knee_l_deg": {:.1},
    "knee_r_deg": {:.1}
  }},
  "pelvis": {{
    "yaw_deg": {:.2},
    "roll_deg": {:.2},
    "bob_y_cm": {:.2}
  }},
  "hints": [
    "thigh_l_deg and thigh_r_deg should have OPPOSITE signs when both in stance",
    "knee_*_deg should be > 0 ONLY when corresponding leg_*_in_swing == true",
    "bob_y_cm should oscillate 0..~4cm with 2x gait frequency (signature humain)",
    "If thigh_*/knee_* all == 0 and is_moving == true : LocomotionBoneCache not ready (see forgia_anim_layer.json cache_ready)"
  ]
}}"#,
        time.elapsed_secs(),
        is_moving,
        snap.speed_m_s,
        snap.speed_factor,
        snap.gait_phase,
        tunables.stride_per_m,
        tunables.stance_frac,
        tunables.amp_thigh,
        tunables.knee_flex_peak,
        leg_l_in_swing,
        leg_r_in_swing,
        snap.thigh_l_deg,
        snap.thigh_r_deg,
        snap.knee_l_deg,
        snap.knee_r_deg,
        snap.pelvic_yaw_deg,
        snap.pelvic_roll_deg,
        snap.bob_y_cm,
    );

    if let Err(e) = std::fs::write(WALK_POSE_SENSOR_PATH, json) {
        warn!("[forgia-rpg::character] walk pose sensor write failed: {e}");
    }
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

