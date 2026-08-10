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
    LocomotionBoneCache, LocomotionDriver, LocomotionState, LocomotionTarget, LocomotionTemplate,
    ProcBodyAnim, AIRBORNE_VY_THRESHOLD, FALL_STRETCH_AMP, IDLE_SPEED_THRESHOLD, JUMP_SQUASH_AMP,
    LEAN_FORWARD_AMP, ROLL_WADDLE_AMP, WALK_BOB_AMP, WALK_FREQ,
};
use forgia_auto_rig::{AutoRigGizmosConfig, AutoRigTemplate, NeedsAutoRig};
use forgia_camera_orbit::OrbitCamera;
use forgia_core::prelude::GameMode;
use forgia_player::prelude::{FpsCamera, Player};
use forgia_skeleton_template::SkeletonTemplateId;
use std::f32::consts::TAU;

/// Marker de l'entité Scene Rex (enfant du Player).
#[derive(Component)]
pub struct RexCharacter;

/// Marker de la caméra orbit RPG (à despawn en cleanup).
#[derive(Component)]
pub struct RpgOrbitCamera;

// Cache des entités bones utilisés par la locomotion procédurale.
// Posé sur Rex SceneRoot. `ready = true` quand la topologie a été analysée.
//
// Story-440 : on n'utilise plus la BFS-par-Name (fragile cross-rig). On délègue
// à `forgia-rig-topology` qui classifie par heuristiques 3D positionnelles —
// marche sur Meshy/AccuRig/Mixamo/Blender sans modifier le GLB.
//
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
    state: Res<State<GameMode>>,
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
            spawn_humanoid_blocks(&mut commands, &mut meshes, &mut materials, player_entity);
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
            //
            // 2026-06-16 — Cyber City démo : en GameMode::CyberCity le perso animé
            // est Cyber.glb (androïde humanoïde) au lieu de Rex (BipedLizard). Même
            // pipeline auto-rig Pinocchio — Cyber.glb = mesh brut sans skin, comme
            // Rex (vérifié GLB : 0 skin / 1 mesh). Calibration mesurée sur le GLB :
            // foot_at_Y = -0.949 (Rex -0.873) → offset -0.95 ; template Humanoid
            // (Cyber n'a ni queue ni jambes digitigrades). Dette : sélection perso
            // hardcodée par mode — à terme genome per-character (cf audit anim 2026-06-07).
            let (char_glb, char_y, auto_tpl, loco_tpl) =
                if matches!(state.get(), GameMode::CyberCity) {
                    (
                        // Story-601 incr.2 : LOD décimé 281k→56k verts (gltf-transform
                        // weld+simplify) pour passer sous le cap skinning 200k
                        // (skinning.rs:320). Original Cyber.glb conservé, même bbox.
                        "models/characters/Cyber_lod.glb#Scene0",
                        -0.95_f32,
                        // Story-601 incr.1 : Cyber est généré bras le long du corps
                        // → template A-pose (bind arms-down). QW2 (story-637) :
                        // HumanoidAuto laisse le pipeline DÉTECTER l'A-pose depuis
                        // arm_span_half_frac (≈0.1 < seuil 0.30) → résout HumanoidApose
                        // tout seul. loco_tpl reste HumanoidApose (résultat attendu).
                        AutoRigTemplate::HumanoidAuto,
                        SkeletonTemplateId::HumanoidApose,
                    )
                } else {
                    (
                        "models/characters/Rex.glb#Scene0",
                        -0.85_f32,
                        AutoRigTemplate::BipedLizard,
                        SkeletonTemplateId::BipedLizard,
                    )
                };
            commands.entity(player_entity).with_children(|parent| {
                parent.spawn((
                    RexCharacter,
                    SceneRoot(asset_server.load(char_glb)),
                    Transform::from_xyz(0.0, char_y, 0.0)
                        .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                    NeedsAutoRig::Template(auto_tpl),
                    // ── 2026-06-02 — Anim pipeline RÉACTIVÉ (story-496 Incrément 1) ──
                    // Cause historique (T-pose + pied déformé) corrigée : proc_walk
                    // ne suppose plus que le X local d'un os est l'axe de flexion.
                    // Le walk pivote chaque membre autour de l'axe latéral du perso
                    // (forgia_anim_locomotion::swing_axis_local + correction stance
                    // propre/hérité) : flexion sagittale correcte bras ET jambes.
                    // Witness : forgia2_rex_bones.json champs flex_axis /
                    // flex_axis_dir. Foot IK reste inactif (Pinocchio 1-os/jambe) →
                    // Incrément 3. Réversible : recommenter les 4 lignes ci-dessous.
                    LocomotionTarget,
                    LocomotionBoneCache::default(),
                    ProcBodyAnim::default(),
                    LocomotionTemplate(loco_tpl),
                    // QW1 (story-637) — driver = le Player (Rex/Cyber est son enfant,
                    // suit son déplacement). Requis par la query multi-perso.
                    LocomotionDriver(player_entity),
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

/// Définitions des personnages humanoïdes du lineup auto-rig, qui servent aussi
/// de PNJ interactifs RPG (story-570). `(name, glb_path, greeting)`.
/// Le `name` produit le tree_id de dialogue via interact_system : "Dorin" →
/// "npc_dorin", "MaitreForgeron" → "npc_maitreforgeron" (cf register_sample_dialogues).
const LINEUP_CHARACTERS: &[(&str, &str, &str)] = &[
    (
        "Dorin",
        "models/characters/Dorin.glb#Scene0",
        "Salut l'ami ! Va voir le Maître Forgeron, il cherche de l'aide.",
    ),
    (
        "Mira",
        "models/characters/Mira.glb#Scene0",
        "Mes étals sont ouverts ! Que cherches-tu ?",
    ),
    (
        "Apprenti",
        "models/characters/L'Apprenti .glb#Scene0",
        "Oh... bonjour. Je m'entraîne encore.",
    ),
    (
        "MaitreForgeron",
        "models/characters/Maitre Forgeron Célèste.glb#Scene0",
        "Approche, apprenti. J'ai une tâche pour toi.",
    ),
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
    anchor: Option<Res<crate::RpgVillageAnchor>>,
) {
    if spawned.done {
        return;
    }
    // Story-570 : ancrage au puits / centre village (point fixe trouvable, pas
    // relatif au joueur qui s'éloigne). L'ancre est insérée par spawn_world
    // OnEnter(Rpg) ; on attend simplement qu'elle existe.
    let Some(anchor) = anchor else {
        return;
    };
    let center = anchor.center;

    // Arc de 90° côté +Z (face d'arrivée naturelle), rayon 3.5m. Chaque PNJ
    // regarde vers le puits. Y = terrain flattené - 0.85 (offset mesh-bottom Rex).
    const RADIUS: f32 = 3.5;
    let n = LINEUP_CHARACTERS.len();
    let spread = std::f32::consts::FRAC_PI_2; // 90° total
    let start_angle = std::f32::consts::FRAC_PI_2 - spread * 0.5; // centré sur +Z
    let spawn_y = center.y - 0.85;
    // Story-539 : chaque PNJ se tient devant « son » bâtiment (forge / marché / taverne / puits).
    let stations = crate::worldgen_village::npc_stations(&anchor);

    for (i, (name, glb_path, greeting)) in LINEUP_CHARACTERS.iter().enumerate() {
        // Station dédiée si elle existe, sinon arc autour du puits (fallback).
        let (px, pz, yaw) =
            if let Some((_, xz, syaw)) = stations.iter().find(|(s, _, _)| *s == *name) {
                (xz.x, xz.y, *syaw)
            } else {
                let t = if n > 1 {
                    i as f32 / (n as f32 - 1.0)
                } else {
                    0.5
                };
                let angle = start_angle + t * spread;
                let px = center.x + angle.cos() * RADIUS;
                let pz = center.z + angle.sin() * RADIUS;
                let yaw = (px - center.x).atan2(pz - center.z);
                (px, pz, yaw)
            };
        let mut ent = commands.spawn((
            LineupCharacter,
            LineupName(name.to_string()),
            // Story-570 : ces personnages on-brand sont les PNJ interactifs du RPG
            // (E pour parler), ancrés au puits village. TODO(story-445) : migrer
            // vers forgia-village-npc-spawner data-driven.
            crate::Npc {
                name: name.to_string(),
                greeting: greeting.to_string(),
            },
            crate::InteractablePoint {
                label: name.to_string(),
                radius: 4.0,
            },
            SceneRoot(asset_server.load(*glb_path)),
            Transform::from_xyz(px, spawn_y, pz).with_rotation(Quat::from_rotation_y(yaw)),
            // Tous Humanoid pour l'instant ; créer template dédié per-character au besoin.
            NeedsAutoRig::Template(AutoRigTemplate::Humanoid),
            Name::new(format!("LineupChar_{name}")),
        ));
        // Note (story-637) : le moteur locomotion est désormais multi-perso, donc
        // animer ces PNJ ne casserait plus Rex. MAIS l'observabilité (capteurs
        // forgia_anim_full/walk_pose/rex_bones_live) est encore mono-target et la
        // pose statique vit dans `npc_pose.rs`. Animer le lineup = follow-up propre
        // (observabilité multi-target + arbitrage npc_pose). Laissé statique ici.
        // Story-58x Phase 4 : le Maître Forgeron donne ET reçoit la quête gobelins
        // → marqueur ! (dispo) puis ? (à rendre) au-dessus de sa tête.
        if *name == "MaitreForgeron" {
            ent.insert(crate::QuestGiver {
                offers: vec![forgia_rpg_data::quests::QuestId("kill_goblins".into())],
                completes: vec![forgia_rpg_data::quests::QuestId("kill_goblins".into())],
            });
        } else if *name == "Mira" {
            // Story-58x Phase 5 : Mira est la marchande (achat/vente).
            ent.insert(forgia_rpg_data::shop::ShopInventory {
                items: vec![
                    forgia_rpg_data::inventory::ItemId("potion_heal".into()),
                    forgia_rpg_data::inventory::ItemId("health_potion".into()),
                    forgia_rpg_data::inventory::ItemId("iron_dagger".into()),
                ],
                sell_ratio: 0.33,
            });
        }
    }

    spawned.done = true;
    info!(
        "[forgia-rpg::character] {} PNJ on-brand spawnés en arc autour du puits village {:?} (rayon {:.1}m)",
        n, center, RADIUS
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
    anchor: Option<Res<crate::RpgVillageAnchor>>,
) {
    // Story-539 : le sol = le Y plat du village (ancre stable), pas `player.y - 1.0` — celui-ci est
    // transitoire au spawn (le joueur tombe), ce qui figeait les PNJ en l'air (calibration one-shot).
    let ground_y = if let Some(a) = &anchor {
        a.center.y
    } else {
        let Ok(player_tf) = q_player.single() else {
            return;
        };
        player_tf.translation.y - 1.0
    };

    for (entity, mut char_tf) in &mut q_lineup {
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut found = false;
        let mut stack: Vec<(Entity, Vec3)> = vec![(entity, Vec3::ZERO)];
        while let Some((e, parent_local)) = stack.pop() {
            let local_pos = if e == entity {
                Vec3::ZERO
            } else {
                transforms_q
                    .get(e)
                    .map(|t| t.translation)
                    .unwrap_or(Vec3::ZERO)
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
            (-1.0_f32, -1.0_f32),
            (0.0, -1.0),
            (1.0, -1.0),
            (-1.0, 0.0),
            (1.0, 0.0),
            (-1.0, 1.0),
            (0.0, 1.0),
            (1.0, 1.0),
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
    mut q_rex: Query<(Entity, &mut Transform), (With<RexCharacter>, Without<RexYCalibrated>)>,
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
                min_y, old_y, rex_tf.translation.y
            );
        }
    }
}

/// Active l'overlay debug du rig (rings gizmos + transparence du mesh) le temps
/// de la démo Cyber City, pour voir le squelette auto-rigué à travers Cyber.
/// Couplé à `AutoRigGizmosConfig.enabled` (rings via `draw_rig_gizmos`,
/// transparence via `rex_make_transparent_one_shot`). Remis OFF en sortie
/// (`disable_rig_overlay`) → le RPG reste opaque (rendu final validé 2026-06-07).
pub(crate) fn enable_rig_overlay(mut gizmos: ResMut<AutoRigGizmosConfig>) {
    gizmos.enabled = true;
}

/// Restaure l'overlay rig OFF en quittant la démo Cyber City (cf `enable_rig_overlay`).
pub(crate) fn disable_rig_overlay(mut gizmos: ResMut<AutoRigGizmosConfig>) {
    gizmos.enabled = false;
}

/// Marker idempotence pour `rex_make_transparent_one_shot`.
#[derive(Component)]
pub struct RexMaterialTransparent;

/// **Rend Rex légèrement transparent** pour visualiser le squelette gizmos à
/// travers le mesh. Clone les `StandardMaterial` des Mesh3d descendants de
/// Rex, set `alpha = 0.40` + `alpha_mode = Blend`. Idempotent via marker.
pub(crate) fn rex_make_transparent_one_shot(
    gizmos_config: Res<AutoRigGizmosConfig>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_rex: Query<Entity, With<RexCharacter>>,
    children_q: Query<&Children>,
    q_mat: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Without<RexMaterialTransparent>>,
) {
    // La transparence n'existe QUE pour voir les gizmos du squelette à travers
    // le mesh. Rig debug overlay OFF (AutoRigGizmosConfig.enabled=false en mode
    // RPG) → Rex reste opaque = rendu final. Flip le flag pour re-déboguer.
    if !gizmos_config.enabled {
        return;
    }
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
                commands
                    .entity(entity)
                    .insert((MeshMaterial3d(new_handle), RexMaterialTransparent));
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
        commands.entity(player).remove::<LocomotionState>();
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
    mut q_rex: Query<(&mut Transform, &mut ProcBodyAnim, &LocomotionBoneCache), With<RexCharacter>>,
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
            // 2026-06-05 : idle = AUCUN bob du corps entier. Le user veut une
            // respiration du TORSE (poitrine), pas le corps qui s'élève/redescend
            // dans les airs. La respiration est portée par le spine breath dans
            // procedural_locomotion (compose_swing sur b.spine, locomotion.rs).
            0.0
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
                    // delta = terrain - capsule_bottom = correction pour poser le BAS
                    // du mesh pile sur le terrain (base_y cale mesh_bottom = capsule_bottom).
                    // 2026-06-05 : on AUTORISE le négatif (clamp -0.5) — avant, clamp
                    // [0.0, 0.4] ne pouvait que REMONTER Rex → s'il flottait (capsule
                    // au-dessus du sol) ou en pente descendante, il lévitait sans fix.
                    // Borne -0.5 = pas de dangle infini au bord d'une falaise ; +0.4 =
                    // montée de pente. Bidirectionnel = suit le terrain dans les 2 sens.
                    delta.clamp(-0.5, 0.4)
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
            // 2026-06-06 : signe flippé (+) — c'est le lean ROOT corps-entier
            // DOMINANT. Avec Y=π appliqué avant X (from_euler YXZ), `-` penchait en
            // ARRIÈRE (confirmé user). `+` = vers l'AVANT en marche.
            LEAN_FORWARD_AMP * speed_factor
        } else if airborne && vy > 0.0 {
            0.10 // léger tuck forward en montée (même convention que la marche)
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
