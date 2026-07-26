//! Procedural locomotion — extraction story-482 P1 depuis forgia-rpg/character.rs.
//!
//! Architecture pose-agnostic (story-481 Action 2) :
//! - `LocomotionTarget` marker = character qui doit recevoir l'anim
//! - `LocomotionState` = entité qui drive l'anim (position + vitesse)
//! - bind rotations capturées une fois à `cache.ready = true`
//! - tout COMPOSE avec bind, jamais OVERWRITE

use bevy::camera::primitives::Aabb;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use forgia_anim_debug::{AnimLayerStats, AnimTimer};
use forgia_genome_core::Genome;
use forgia_rig_topology::{analyze_rig_topology, RigTopology};
use forgia_secondary_motion::{SpringBone, SpringBoneChain};
use forgia_skeleton_template::{SkeletonTemplate, SkeletonTemplateId, SkeletonTemplateRegistry};

/// Marker à insérer sur le character qui doit recevoir l'animation procédurale.
/// forgia-rpg ajoute ce marker sur `RexCharacter` au spawn.
#[derive(Component)]
pub struct LocomotionTarget;

/// Story-482 P2b : référence vers le template TOML chargé via
/// `SkeletonTemplateRegistry`. Le système `apply_stance_offsets_from_template`
/// lit `template.stance_offsets` et insère un Component `StanceOffsets` sur
/// l'entité (avec hot-reload via AssetEvent).
///
/// Insérée par le consumer (forgia-rpg::spawn_rex_character) à la place de
/// `StanceOffsets::humanoid_tpose()` hardcodé.
#[derive(Component, Debug, Clone, Copy)]
pub struct LocomotionTemplate(pub SkeletonTemplateId);

/// Story-482 P2 — Stance offsets data-driven par character.
/// Composé avec bind ET swing : `tf.rotation = bind * stance * swing_delta`.
/// Permet à un mesh T-pose Vitruvian (arms horizontal) d'avoir des bras
/// verticaux en pose game sans toucher le code Rust. Valeurs typiquement
/// dérivées de `SkeletonTemplate.stance_offsets` (TOML hot-reloadable).
#[derive(Component, Debug, Clone)]
pub struct StanceOffsets {
    pub arm_l: Quat,
    pub arm_r: Quat,
    pub leg_l: Quat,
    pub leg_r: Quat,
    pub spine: Quat,
    pub hip: Quat,
    /// Story-482 P2c (2026-05-21) — shoulder anchor. Si non animée, la
    /// clavicle reste à bind T-pose et ancre visuellement le bras horizontal
    /// malgré la rotation arm_L/R. Permet d'accompagner la pose game.
    pub clavicle_l: Quat,
    pub clavicle_r: Quat,
}

impl Default for StanceOffsets {
    /// Default = aucune stance offset (identity partout). Pour un mesh déjà
    /// en pose game (arms-down naturel), ne touche rien.
    fn default() -> Self {
        Self {
            arm_l: Quat::IDENTITY,
            arm_r: Quat::IDENTITY,
            leg_l: Quat::IDENTITY,
            leg_r: Quat::IDENTITY,
            spine: Quat::IDENTITY,
            hip: Quat::IDENTITY,
            clavicle_l: Quat::IDENTITY,
            clavicle_r: Quat::IDENTITY,
        }
    }
}

impl StanceOffsets {
    /// Helper humanoid T-pose Vitruvian : arms horizontaux → vertical via Z ±π/2.
    /// Backward compat avec le hardcode story-481 ARM_STANCE_DROP_RAD.
    pub fn humanoid_tpose() -> Self {
        Self {
            arm_l: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            arm_r: Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
            ..Default::default()
        }
    }

    /// Helper depuis euler XYZ degrés (format TOML SkeletonTemplate.stance_offsets).
    #[allow(clippy::too_many_arguments)]
    pub fn from_euler_degs(
        arm_l: [f32; 3],
        arm_r: [f32; 3],
        leg_l: [f32; 3],
        leg_r: [f32; 3],
        spine: [f32; 3],
        hip: [f32; 3],
        clavicle_l: [f32; 3],
        clavicle_r: [f32; 3],
    ) -> Self {
        let q = |e: [f32; 3]| {
            Quat::from_euler(
                EulerRot::XYZ,
                e[0].to_radians(),
                e[1].to_radians(),
                e[2].to_radians(),
            )
        };
        Self {
            arm_l: q(arm_l),
            arm_r: q(arm_r),
            leg_l: q(leg_l),
            leg_r: q(leg_r),
            spine: q(spine),
            hip: q(hip),
            clavicle_l: q(clavicle_l),
            clavicle_r: q(clavicle_r),
        }
    }
}

/// Cache des bones articulés + topology + bind rotations.
#[derive(Component, Default)]
pub struct LocomotionBoneCache {
    pub topology: RigTopology,
    pub ready: bool,
    pub frames_waited: u32,
    pub calibrated: bool,
    pub gave_up: bool,
    pub bones: ArticulatedBones,
}

/// Une entité bone + sa rotation au repos (capturée au moment où Pinocchio a
/// fini d'embed). Utilisée par le walk cycle pour composer un delta de swing
/// PAR-DESSUS la rest pose réelle (pose-agnostic).
#[derive(Clone, Copy)]
pub struct BonePose {
    pub entity: Option<Entity>,
    pub bind: Quat,
    /// Axe de flexion médio-latéral en espace LOCAL de l'os, dérivé à
    /// `cache.ready` depuis la direction head→tip (story-496). Le walk cycle
    /// compose `from_axis_angle(flex_axis, swing)` au lieu d'un `from_rotation_x`
    /// hardcodé — robuste au frame Pinocchio arbitraire (bras le long de X local
    /// vs jambes le long de Y local).
    pub flex_axis: Vec3,
}

impl Default for BonePose {
    fn default() -> Self {
        // flex_axis = X par défaut = comportement historique `from_rotation_x`
        // (jamais ZERO : un axe nul casserait `from_axis_angle`).
        Self {
            entity: None,
            bind: Quat::IDENTITY,
            flex_axis: Vec3::X,
        }
    }
}

impl BonePose {
    pub fn from_entity(entity: Option<Entity>, rot_of: &dyn Fn(Entity) -> Option<Quat>) -> Self {
        let bind = entity.and_then(rot_of).unwrap_or(Quat::IDENTITY);
        Self {
            entity,
            bind,
            flex_axis: Vec3::X,
        }
    }
}

/// Axe médio-latéral du personnage (gauche↔droite) en espace repos. La flexion
/// du walk cycle (cuisse, genou, épaule, coude, cheville) est TOUJOURS un
/// pivot sagittal autour de cet axe. Sur Rex (Pinocchio), le squelette de repos
/// est aligné sur les axes monde (`bind ≈ identité` pour tous les os — confirmé
/// par witness `forgia2_rex_bones.json` : euler_xyz ≈ 0, `bind_was_identity`),
/// donc l'axe latéral = `Vec3::X`.
const LATERAL_AXIS: Vec3 = Vec3::X;

/// Axe de swing dans l'espace LOCAL d'un os, tel que
/// `bone.bind * from_axis_angle(axis, θ)` produise une flexion sagittale autour
/// de [`LATERAL_AXIS`]. C'est `bind⁻¹ · latéral` : robuste à un bind non-identité
/// de l'os lui-même (story-496).
///
/// ⚠️ Ne corrige PAS le stance d'un ANCÊTRE. Deux compose-paths complètent :
/// - [`compose_stance_swing`] : annule le stance PROPRE de l'os (bras = Rz(±90°)).
/// - [`compose_swing_inherit`] : annule le stance HÉRITÉ du parent (avant-bras,
///   enfants d'un bras stancé). Les tibias n'en ont pas besoin : leur parent
///   (cuisse) tourne autour de X au walk, et X est invariant par rotation autour
///   de X.
fn swing_axis_local(bind: Quat) -> Vec3 {
    (bind.inverse() * LATERAL_AXIS).normalize_or(Vec3::X)
}

/// Label de l'axe canonique dominant d'un vecteur (pour le sensor witness).
fn axis_label(v: Vec3) -> &'static str {
    let a = v.abs();
    if a.x >= a.y && a.x >= a.z {
        if v.x >= 0.0 {
            "+X"
        } else {
            "-X"
        }
    } else if a.y >= a.z {
        if v.y >= 0.0 {
            "+Y"
        } else {
            "-Y"
        }
    } else if v.z >= 0.0 {
        "+Z"
    } else {
        "-Z"
    }
}

/// Tous les bones articulés du walk cycle, résolus une fois pour toutes à
/// `cache.ready = true`.
#[derive(Default)]
pub struct ArticulatedBones {
    pub left_arm: BonePose,
    pub right_arm: BonePose,
    pub forearm_l: BonePose,
    pub forearm_r: BonePose,
    pub hand_l: BonePose,
    pub hand_r: BonePose,
    pub clavicle_l: BonePose,
    pub clavicle_r: BonePose,
    pub left_leg: BonePose,
    pub right_leg: BonePose,
    pub shin_l: BonePose,
    pub shin_r: BonePose,
    pub foot_l: BonePose,
    pub foot_r: BonePose,
    pub spine: BonePose,
    pub hip: BonePose,
    /// Tête (look-around idle façon WoW). Posée au yaw idle, slerp bind en marche.
    pub head: BonePose,
    /// Nuque (parent de la tête). Reçoit la MOITIÉ du yaw du regard pour que la
    /// tête tourne AVEC la nuque (pas de twist relatif fort → pas de cisaillement
    /// de la peau au cou). 2026-06-06.
    pub neck: BonePose,
    pub tail_chain: Vec<BonePose>,
}

/// Compteur d'entrées dans `GameMode::Rpg`. Incrémenté `OnEnter(Rpg)` par le
/// consumer (forgia-rpg) via [`increment_rpg_entry_count`]. Utilisé par les
/// sensors anim pour produire des fichiers indexés par cycle
/// (`forgia2_rex_bones_entry_{N}_bind.json` et `..._live.json`), permettant
/// de diff la signature des bones entre cycles successifs (diagnostic
/// state leak hand_l/r — WIP story-482 2026-05-20).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct RpgEntryCount(pub u32);

/// `OnEnter(GameMode::Rpg)` system — incrémente [`RpgEntryCount`] et log.
/// À brancher côté consumer (forgia-rpg) sur OnEnter(Rpg).
pub fn increment_rpg_entry_count(mut count: ResMut<RpgEntryCount>) {
    count.0 = count.0.saturating_add(1);
    info!(
        "[anim-locomotion] RPG entry #{} — sensors will write forgia2_rex_bones_entry_{}_*.json",
        count.0, count.0
    );
}

/// Debug : fige tous les os locomotion à leur bind pose → le mesh skinné revient
/// à son état ORIGINAL non déformé (T-pose Meshy). Sert à vérifier l'alignement
/// os↔mesh (rings dans les features mesh) sans la déformation du skinning. Toggle F6.
#[derive(Resource, Default)]
pub struct AnimFreezeBind(pub bool);

/// Toggle F6 du freeze-bind (mesh non déformé).
pub fn toggle_anim_freeze_bind(keys: Res<ButtonInput<KeyCode>>, mut freeze: ResMut<AnimFreezeBind>) {
    if keys.just_pressed(KeyCode::F6) {
        freeze.0 = !freeze.0;
        info!(
            "[anim-locomotion] FREEZE-BIND (mesh original non déformé) = {} (F6)",
            freeze.0
        );
    }
}

/// Vitesse précédente du driver (frame n-1), pour estimer la vélocité.
/// Insérée par forgia-rpg sur l'entité Player.
#[derive(Component, Default)]
pub struct LocomotionState {
    pub prev_pos: Vec3,
    pub speed: f32,
    pub gait_phase: f32,
}

/// QW1 (story-637) — lie un `LocomotionTarget` (perso animé) à l'entité qui
/// porte son `LocomotionState` (source vitesse/gait). Pour Rex = le Player ;
/// pour un NPC autonome = lui-même. Permet d'animer N persos (un driver chacun),
/// au lieu de l'ancien couple unique driver/target via `.single()`.
#[derive(Component, Clone, Copy)]
pub struct LocomotionDriver(pub Entity);

/// Animation procédurale whole-body — bob, lean, squash. Appliqué au Transform
/// root du character (composé par procedural_whole_body_anim qui reste dans
/// forgia-rpg car couplé à Player.vertical_velocity).
#[derive(Component, Default)]
pub struct ProcBodyAnim {
    pub walk_phase: f32,
    pub base_y: f32,
    pub initialized: bool,
    pub bob_smooth: f32,
    pub lean_smooth: Vec3,
    pub ground_hug_smooth: f32,
}

// ── Tunables pub (utilisés par forgia-rpg::procedural_whole_body_anim) ──────
pub const WALK_FREQ: f32 = 2.5;
pub const WALK_BOB_AMP: f32 = 0.025;
pub const LEAN_FORWARD_AMP: f32 = 0.18;
// 2026-06-05 : 0.06 → 0.03. 2026-06-06 : 0.03 → 0.015 — user « moins basculer le
// corps de droite à gauche quand on avance ». Roulis CORPS ENTIER (waddle root,
// character.rs target_roll_z) ramené à ~0.86° au lieu de 1.7°. 0 = aucun roulis.
pub const ROLL_WADDLE_AMP: f32 = 0.015;
pub const JUMP_SQUASH_AMP: f32 = 0.08;
pub const FALL_STRETCH_AMP: f32 = 0.06;
pub const AIRBORNE_VY_THRESHOLD: f32 = 0.8;

// ARM_STANCE_DROP_RAD supprimé en P2 (2026-05-20). Stance offsets
// data-driven via Component StanceOffsets, défaut humanoid_tpose() pour
// backward compat. Lire depuis SkeletonTemplate TOML quand l'asset
// est chargé via forgia-genome-core.

pub const IDLE_SPEED_THRESHOLD: f32 = 0.15;
// 2026-06-06 : 1.2 → 0.45 Hz — user « mouvements trop rapides ». 1.2 Hz = un souffle
// toutes les 0.8s (halètement) ; un souffle calme ≈ 0.25-0.45 Hz (toutes les 2-4s).
pub const IDLE_BREATH_FREQ: f32 = 0.45;
pub const IDLE_BREATH_AMP: f32 = 0.03;

const GIVEUP_FRAMES: u32 = 120;

// ── attach_locomotion_bones (ex attach_rex_bone_systems) ─────────────────────

/// Analyse topologie + calibre Y via AABB + capture bind rotations pose-agnostic.
/// Retry chaque frame jusqu'à `cache.ready = true` ou `GIVEUP_FRAMES` atteint.
#[allow(clippy::too_many_arguments)]
pub fn attach_locomotion_bones(
    mut commands: Commands,
    mut q_cache: Query<(Entity, &mut LocomotionBoneCache, &mut Transform), With<LocomotionTarget>>,
    children_query: Query<&Children>,
    names: Query<&Name>,
    transforms: Query<&Transform, Without<LocomotionTarget>>,
    aabbs: Query<&Aabb>,
    entry_count: Res<RpgEntryCount>,
) {
    for (rex_entity, mut cache, mut rex_tf) in &mut q_cache {
        if cache.ready || cache.gave_up {
            continue;
        }
        cache.frames_waited += 1;

        // Pass 1 : calibration Y via AABB
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
                let target_bottom = -1.0_f32;
                let new_y = target_bottom - min_y;
                let old_y = rex_tf.translation.y;
                rex_tf.translation.y = new_y;
                info!(
                    "[anim-locomotion] AABB calibrated: aabb_y_local=[{:.3}..{:.3}] count={} → tf.y {:.3} → {:.3} (delta {:+.3})",
                    min_y, max_y, count, old_y, new_y, new_y - old_y
                );
                cache.calibrated = true;
            }
        }

        // Pass 2 : topology analysis
        let children_of = |e: Entity| {
            children_query
                .get(e)
                .map(|c| c.iter().collect())
                .unwrap_or_default()
        };
        let transform_of = |e: Entity| transforms.get(e).ok().copied();
        let name_of = |e: Entity| names.get(e).ok().map(|n| n.to_string());

        let topo = analyze_rig_topology(rex_entity, &children_of, &transform_of, &name_of);

        if !cache.gave_up && !topo.diagnostics.is_empty() && cache.frames_waited <= 10 {
            info!(
                "[anim-locomotion] Skeleton scan: {} bones found, is_usable={}",
                topo.diagnostics.len(),
                topo.is_usable()
            );
            for diag in topo.diagnostics.iter().take(60) {
                info!(
                    "  bone '{}' depth={} pos=({:.2}, {:.2}, {:.2}) children={}",
                    diag.name,
                    diag.depth,
                    diag.local_pos.x,
                    diag.local_pos.y,
                    diag.local_pos.z,
                    diag.child_count
                );
            }
        }

        if topo.is_usable() {
            // Spring chain queue (tail)
            // 2026-06-04 (audit anim) : double-driver queue résolu. La queue est
            // pilotée par le YAW PROCÉDURAL (procedural_locomotion → tail_segment_yaw,
            // ~l.788). Le SpringBone (forgia-secondary-motion) écrasait ce yaw en
            // PostUpdate en supposant l'axe d'os +Y (≠ bind Pinocchio de Rex) → whip
            // Verlet, queue de travers. Option B retenue : SpringBone OFF (driver unique
            // = procédural). Pour repasser en secondary-motion physique (Option A),
            // remettre `true` ET corriger l'axe supposé +Y du solver
            // (forgia-secondary-motion/src/solver.rs:159).
            const TAIL_USE_SPRINGBONE: bool = false;
            if TAIL_USE_SPRINGBONE && topo.tail_chain.len() >= 2 {
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
                    "[anim-locomotion] Tail spring attached: chain_len={}",
                    topo.tail_chain.len()
                );
            }

            info!(
                "[anim-locomotion] RigTopology classified: legs L/R={}/{}, arms L/R={}/{}, spine={}, head={}, tail={}",
                topo.left_leg.is_some(),
                topo.right_leg.is_some(),
                topo.left_arm.is_some(),
                topo.right_arm.is_some(),
                topo.spine.is_some(),
                topo.head.is_some(),
                topo.tail_chain.len(),
            );

            // Capture bind rotations pose-agnostic
            let rot_of = |e: Entity| transforms.get(e).ok().map(|t| t.rotation);

            // Story-482 fix 2026-05-20 : Pinocchio output spawne les bones à
            // plat (sibling under Armature root), donc first_child(leg) renvoie
            // None. Le sensor forgia2_skinning_weights.json confirme que les
            // bones shin/foot/forearm/hand EXISTENT (1018-1458 verts primary).
            // Fix : BFS descendants de rex_entity + Name lookup.
            let mut name_to_entity: std::collections::HashMap<String, Entity> =
                std::collections::HashMap::default();
            {
                let mut stack: Vec<Entity> = vec![rex_entity];
                while let Some(e) = stack.pop() {
                    if let Ok(name) = names.get(e) {
                        name_to_entity.insert(name.to_string(), e);
                    }
                    if let Ok(children) = children_query.get(e) {
                        for c in children.iter() {
                            stack.push(c);
                        }
                    }
                }
            }
            let lookup = |name: &str| -> Option<Entity> { name_to_entity.get(name).copied() };

            let left_arm_e = topo.left_arm;
            let right_arm_e = topo.right_arm;
            // 2026-06-03 : la topologie 3D classait la cuisse sur l'os du PIED
            // (sonde : left_leg.world_head == foot_l.world_head exactement → leg
            // collapse, ring genou faux). On résout la cuisse par NOM (thigh_L/R),
            // comme shin/foot, avec fallback topologie si le mesh ne nomme pas ses os.
            let left_leg_e = lookup("thigh_L").or(topo.left_leg);
            let right_leg_e = lookup("thigh_R").or(topo.right_leg);
            // Résolution par NOM (templates Forgia : forearm_L/R, shin_L/R, foot_L/R)
            // avec FALLBACK topologie (QW3 story-637) `.or(topo.child)` — comme
            // thigh_L au-dessus. Le nom garde la priorité (rigs template inchangés) ;
            // les rigs aux noms étrangers résolvent désormais via la chaîne 3D.
            let forearm_l_e = lookup("forearm_L").or(topo.left_forearm);
            let forearm_r_e = lookup("forearm_R").or(topo.right_forearm);
            let hand_l_e = lookup("hand_L").or(topo.left_hand);
            let hand_r_e = lookup("hand_R").or(topo.right_hand);
            // Clavicule : pas de fallback topologie (pas une chaîne linéaire claire).
            let clavicle_l_e = lookup("clavicle_L");
            let clavicle_r_e = lookup("clavicle_R");
            let neck_e = lookup("neck").or(topo.neck);
            let shin_l_e = lookup("shin_L").or(topo.left_shin);
            let shin_r_e = lookup("shin_R").or(topo.right_shin);
            let foot_l_e = lookup("foot_L").or(topo.left_foot);
            let foot_r_e = lookup("foot_R").or(topo.right_foot);
            info!(
                "[anim-locomotion] Name-lookup bones : forearm L/R={}/{}, hand L/R={}/{}, clavicle L/R={}/{}, shin L/R={}/{}, foot L/R={}/{}",
                forearm_l_e.is_some(), forearm_r_e.is_some(),
                hand_l_e.is_some(), hand_r_e.is_some(),
                clavicle_l_e.is_some(), clavicle_r_e.is_some(),
                shin_l_e.is_some(), shin_r_e.is_some(),
                foot_l_e.is_some(), foot_r_e.is_some(),
            );
            let mut bones = ArticulatedBones {
                left_arm: BonePose::from_entity(left_arm_e, &rot_of),
                right_arm: BonePose::from_entity(right_arm_e, &rot_of),
                forearm_l: BonePose::from_entity(forearm_l_e, &rot_of),
                forearm_r: BonePose::from_entity(forearm_r_e, &rot_of),
                hand_l: BonePose::from_entity(hand_l_e, &rot_of),
                hand_r: BonePose::from_entity(hand_r_e, &rot_of),
                clavicle_l: BonePose::from_entity(clavicle_l_e, &rot_of),
                clavicle_r: BonePose::from_entity(clavicle_r_e, &rot_of),
                left_leg: BonePose::from_entity(left_leg_e, &rot_of),
                right_leg: BonePose::from_entity(right_leg_e, &rot_of),
                shin_l: BonePose::from_entity(shin_l_e, &rot_of),
                shin_r: BonePose::from_entity(shin_r_e, &rot_of),
                foot_l: BonePose::from_entity(foot_l_e, &rot_of),
                foot_r: BonePose::from_entity(foot_r_e, &rot_of),
                spine: BonePose::from_entity(topo.spine, &rot_of),
                hip: BonePose::from_entity(topo.root, &rot_of),
                head: BonePose::from_entity(topo.head, &rot_of),
                neck: BonePose::from_entity(neck_e, &rot_of),
                tail_chain: topo
                    .tail_chain
                    .iter()
                    .map(|&e| BonePose::from_entity(Some(e), &rot_of))
                    .collect(),
            };

            // ── story-496 : axe de flexion par-os depuis le frame local ──
            // Pinocchio n'impose aucune convention de frame ; on dérive l'axe
            // médio-latéral de chaque os depuis la direction head→tip locale
            // (translation locale du bone enfant). Évite `from_rotation_x` qui
            // roulait les bras (os le long de leur propre X local) → T-pose.
            // La flexion du walk est toujours un pivot sagittal autour de l'axe
            // latéral du perso ([`LATERAL_AXIS`]). L'axe LOCAL par-os = bind⁻¹·latéral
            // (= X tant que bind≈identité, ce que Pinocchio produit sur Rex). Le
            // stance propre des bras est corrigé dans compose_stance_swing ; le
            // stance hérité (avant-bras) dans compose_swing_inherit.
            for b in [
                &mut bones.left_arm,
                &mut bones.right_arm,
                &mut bones.forearm_l,
                &mut bones.forearm_r,
                &mut bones.left_leg,
                &mut bones.right_leg,
                &mut bones.shin_l,
                &mut bones.shin_r,
                &mut bones.foot_l,
                &mut bones.foot_r,
            ] {
                b.flex_axis = swing_axis_local(b.bind);
            }

            let dump = |label: &str, b: &BonePose| {
                let (x, y, z) = b.bind.to_euler(EulerRot::XYZ);
                info!(
                    "  [bind] {:<12} entity={:?} euler_xyz_deg=({:+6.1}, {:+6.1}, {:+6.1})",
                    label,
                    b.entity.is_some(),
                    x.to_degrees(),
                    y.to_degrees(),
                    z.to_degrees(),
                );
            };
            info!("[anim-locomotion] Bind rotations captured (pose-agnostic compose) :");
            dump("left_arm", &bones.left_arm);
            dump("right_arm", &bones.right_arm);
            dump("left_leg", &bones.left_leg);
            dump("right_leg", &bones.right_leg);
            dump("spine", &bones.spine);
            dump("hip", &bones.hip);
            info!("  [stance] StanceOffsets Component read at walk cycle (P2 data-driven)");

            // Sensor : dump bind euler + translations
            let tx_of = |opt_e: Option<Entity>| {
                opt_e
                    .and_then(|e| transforms.get(e).ok())
                    .map(|t| t.translation)
                    .unwrap_or(Vec3::ZERO)
            };
            let fmt_bone = |b: &BonePose, child_e: Option<Entity>| {
                let (x, y, z) = b.bind.to_euler(EulerRot::XYZ);
                let head = tx_of(b.entity);
                let tip = tx_of(child_e);
                format!(
                    "{{\"present\":{},\"euler_xyz_deg\":[{:.2},{:.2},{:.2}],\"head\":[{:.3},{:.3},{:.3}],\"tip_local\":[{:.3},{:.3},{:.3}],\"bone_dir\":\"{}\",\"flex_axis\":[{:.2},{:.2},{:.2}],\"flex_axis_dir\":\"{}\"}}",
                    b.entity.is_some(),
                    x.to_degrees(), y.to_degrees(), z.to_degrees(),
                    head.x, head.y, head.z,
                    tip.x, tip.y, tip.z,
                    axis_label(tip),
                    b.flex_axis.x, b.flex_axis.y, b.flex_axis.z,
                    axis_label(b.flex_axis),
                )
            };
            let entry_idx = entry_count.0;
            let json = format!(
                "{{\n  \"captured_at\": \"cache.ready\",\n  \"entry_index\": {},\n  \"stance_source\": \"StanceOffsets Component (P2 data-driven)\",\n  \"bones\": {{\n    \"left_arm\": {},\n    \"right_arm\": {},\n    \"forearm_l\": {},\n    \"forearm_r\": {},\n    \"hand_l\": {},\n    \"hand_r\": {},\n    \"clavicle_l\": {},\n    \"clavicle_r\": {},\n    \"left_leg\": {},\n    \"right_leg\": {},\n    \"spine\": {},\n    \"hip\": {}\n  }}\n}}\n",
                entry_idx,
                fmt_bone(&bones.left_arm, bones.forearm_l.entity),
                fmt_bone(&bones.right_arm, bones.forearm_r.entity),
                fmt_bone(&bones.forearm_l, bones.hand_l.entity),
                fmt_bone(&bones.forearm_r, bones.hand_r.entity),
                fmt_bone(&bones.hand_l, None),
                fmt_bone(&bones.hand_r, None),
                fmt_bone(&bones.clavicle_l, bones.left_arm.entity),
                fmt_bone(&bones.clavicle_r, bones.right_arm.entity),
                fmt_bone(&bones.left_leg, bones.shin_l.entity),
                fmt_bone(&bones.right_leg, bones.shin_r.entity),
                fmt_bone(&bones.spine, None),
                fmt_bone(&bones.hip, None),
            );
            let _ = forgia_core::sensor_io::enqueue("forgia2_rex_bones.json", json.clone());
            // Indexed per-cycle sensor — persists across RPG re-entries.
            let indexed_path = format!("forgia2_rex_bones_entry_{}_bind.json", entry_idx);
            let _ = forgia_core::sensor_io::enqueue(indexed_path.clone(), json);
            info!("[anim-locomotion] Bind snapshot queued → {indexed_path}");
            cache.bones = bones;
            cache.topology = topo;
            cache.ready = true;
        } else if cache.frames_waited >= GIVEUP_FRAMES && !cache.gave_up {
            warn!(
                "[anim-locomotion] No usable skeleton after {} frames — mesh likely STATIC (no armature). \
                 Procedural animation IMPOSSIBLE without a rigged GLB.",
                cache.frames_waited
            );
            cache.gave_up = true;
        }
    }
}

// ── Procedural locomotion (walk cycle + idle breathing) ─────────────────────

#[allow(clippy::too_many_arguments)]
pub fn procedural_locomotion(
    time: Res<Time>,
    mut q_driver: Query<(&Transform, &mut LocomotionState)>,
    q_targets: Query<
        (&LocomotionBoneCache, Option<&StanceOffsets>, &LocomotionDriver),
        With<LocomotionTarget>,
    >,
    mut bones: Query<&mut Transform, (Without<LocomotionState>, Without<LocomotionTarget>)>,
    mut stats: ResMut<AnimLayerStats>,
    freeze: Res<AnimFreezeBind>,
) {
    let timer = AnimTimer::start();
    stats.locomotion_active = false;

    let dt = time.delta_secs();
    if dt <= 0.0 {
        stats.locomotion_us = timer.elapsed_us();
        return;
    }

    // QW1 (story-637) — multi-perso : itère CHAQUE target, résout SON driver
    // (entité portant LocomotionState : Player pour Rex, soi-même pour un NPC).
    // Remplace le double `.single()` qui bridait l'anim à un seul personnage.
    // (corps par-target volontairement non ré-indenté pour un diff minimal —
    //  `bones` reste la Query owned, tous les helpers `&mut bones` inchangés.)
    for (cache, stance_opt, driver_link) in &q_targets {
    let Ok((driver_tf, mut state)) = q_driver.get_mut(driver_link.0) else {
        continue;
    };
    stats.locomotion_active = true;

    let pos = driver_tf.translation;
    let velocity = (pos - state.prev_pos) / dt;
    let horiz_vel = Vec3::new(velocity.x, 0.0, velocity.z);
    let speed_now = horiz_vel.length();
    state.speed = state.speed * 0.85 + speed_now * 0.15;
    state.prev_pos = pos;
    let speed = state.speed;
    let is_moving = speed > IDLE_SPEED_THRESHOLD;
    stats.locomotion_speed = speed;
    stats.locomotion_is_moving = is_moving;

    stats.locomotion_cache_ready = cache.ready;
    if !cache.ready {
        continue;
    }

    let b = &cache.bones;
    // Story-482 P2 : stance offsets data-driven via Component. Fallback
    // identity si pas de Component (mesh assumé déjà en pose game).
    let stance_default = StanceOffsets::default();
    let stance = stance_opt.unwrap_or(&stance_default);

    // Debug F6 : fige TOUS les os à leur bind → le skinning rend le mesh ORIGINAL
    // non déformé (T-pose Meshy). Permet de vérifier l'alignement os↔mesh (rings
    // dans le genou/épaule) sans la déformation du stance+swing+skinning.
    if freeze.0 {
        for bone in [
            &b.left_arm, &b.right_arm, &b.forearm_l, &b.forearm_r, &b.hand_l, &b.hand_r,
            &b.clavicle_l, &b.clavicle_r, &b.left_leg, &b.right_leg, &b.shin_l, &b.shin_r,
            &b.foot_l, &b.foot_r, &b.spine, &b.hip, &b.head, &b.neck,
        ] {
            slerp_to_bind(&mut bones, bone, 1.0);
        }
        for seg in &b.tail_chain {
            slerp_to_bind(&mut bones, seg, 1.0);
        }
        stats.locomotion_gait_phase = state.gait_phase;
        continue;
    }

    if !is_moving {
        let t_secs = time.elapsed_secs();
        let breath = (t_secs * IDLE_BREATH_FREQ).sin() * IDLE_BREATH_AMP;
        // Posture WoW : léger voûtement AVANT du buste (épaules arrondies, pas
        // militaire-droit) → c'est LE cue anti-« crispé ». Avance aussi les épaules
        // donc les mains pendent vers l'avant (pas remontées aux hanches). +X =
        // avant (même convention/sens que le lean buste du walk). + respiration.
        const IDLE_FORWARD_HUNCH: f32 = 0.09; // ~5° avant
        if let Some(e) = b.spine.entity {
            if let Ok(mut tf) = bones.get_mut(e) {
                tf.rotation = b.spine.bind
                    * Quat::from_rotation_x(IDLE_FORWARD_HUNCH)
                    * Quat::from_axis_angle(b.spine.flex_axis, breath);
            }
        }

        // ── Bras vivants en idle (moins « poteau collé », demande user 2026-06-06) :
        // (1) ABDUCTION : décolle les bras du corps (gap naturel) → tue l'effet
        //     « soudé droit le long du torse ». (2) léger swing d'épaule + (3)
        //     micro-respiration du coude — lents, petits, L/R déphasés (organique).
        const IDLE_ARM_SWAY_FREQ: f32 = 0.32; // 2026-06-06 : 0.55→0.32, user « trop rapide »
        const IDLE_ARM_SWAY_AMP: f32 = 0.07; // ~4° swing épaule (ampli gardée, juste + lent)
        const IDLE_ELBOW_BREATHE_AMP: f32 = 0.09; // ~5° respiration coude
        const IDLE_ARM_ABDUCT_DEG: f32 = 6.0; // écartement bras↔corps (gap)
        let arm_sway_l = (t_secs * IDLE_ARM_SWAY_FREQ).sin() * IDLE_ARM_SWAY_AMP;
        let arm_sway_r = (t_secs * IDLE_ARM_SWAY_FREQ + 0.7).sin() * IDLE_ARM_SWAY_AMP;
        let elbow_breathe_l =
            (t_secs * IDLE_ARM_SWAY_FREQ * 0.8).sin() * IDLE_ELBOW_BREATHE_AMP;
        let elbow_breathe_r =
            (t_secs * IDLE_ARM_SWAY_FREQ * 0.8 + 0.9).sin() * IDLE_ELBOW_BREATHE_AMP;

        // Abduction : réduit le Rz du stance bras (moins « rabattu vers la verticale »
        // = bras plus écarté du corps). Rz pur → commute avec le stance bras (pur Rz).
        // L : stance +Rz, on retranche ; R : stance −Rz, on ajoute (symétrique).
        let abduct = IDLE_ARM_ABDUCT_DEG.to_radians();
        let arm_l_idle = stance.arm_l * Quat::from_rotation_z(-abduct);
        let arm_r_idle = stance.arm_r * Quat::from_rotation_z(abduct);

        // Épaule : stance abducté + léger swing fore-aft (slerp vers une cible qui
        // oscille). Axe corrigé sur la chaîne clavicule×bras (sagittal, comme le walk).
        for (arm, own, parent, sway) in [
            (&b.left_arm, arm_l_idle, stance.clavicle_l, arm_sway_l),
            (&b.right_arm, arm_r_idle, stance.clavicle_r, arm_sway_r),
        ] {
            if let Some(e) = arm.entity {
                if let Ok(mut tf) = bones.get_mut(e) {
                    let chain = parent * own;
                    let axis = (chain.inverse() * arm.flex_axis).normalize_or(Vec3::X);
                    let target = arm.bind * own * Quat::from_axis_angle(axis, sway);
                    tf.rotation = tf.rotation.slerp(target, 0.15);
                }
            }
        }
        // Clavicule : stance pur (ancre l'épaule, pas de swing).
        slerp_to_stance(&mut bones, &b.clavicle_l, stance.clavicle_l, 0.15);
        slerp_to_stance(&mut bones, &b.clavicle_r, stance.clavicle_r, 0.15);
        // Coude : flexion de repos LÉGÈRE (ELBOW_REST_FLEX_IDLE ~9° → mains basses,
        // près des cuisses pas des hanches) + micro-respiration. Parent = clavicule×
        // bras ABDUCTÉ (l'avant-bras suit l'écartement). Axe sagittal comme le walk.
        for (fa, parent, breathe) in [
            (&b.forearm_l, stance.clavicle_l * arm_l_idle, elbow_breathe_l),
            (&b.forearm_r, stance.clavicle_r * arm_r_idle, elbow_breathe_r),
        ] {
            if let Some(e) = fa.entity {
                if let Ok(mut tf) = bones.get_mut(e) {
                    let axis = (parent.inverse() * fa.flex_axis).normalize_or(Vec3::X);
                    let flex = crate::proc_walk::ELBOW_REST_FLEX_IDLE + breathe;
                    let target = fa.bind * Quat::from_axis_angle(axis, -flex);
                    tf.rotation = tf.rotation.slerp(target, 0.15);
                }
            }
        }
        for bone in [
            &b.left_leg,
            &b.right_leg,
            &b.shin_l,
            &b.shin_r,
            &b.foot_l,
            &b.foot_r,
            &b.hip,
        ] {
            slerp_to_bind(&mut bones, bone, 0.15);
        }

        // ── Idle vivant : queue qui ondule (lézard) + tête qui regarde G/D (WoW) ─
        // Queue : onde lente VOYAGEUSE le long des segments (yaw autour de Y =
        // balancement latéral), amplitude croissante vers le bout. Les rotations
        // locales se composent le long de la chaîne → garder petit.
        const TAIL_IDLE_FREQ: f32 = 0.55; // 2026-06-06 : 1.1→0.55, user « trop rapide »
        const TAIL_IDLE_AMP: f32 = 0.09; // ~5° base/segment (compose vers le bout)
        const TAIL_IDLE_PHASE: f32 = 0.7; // déphasage/segment = onde voyageuse
        for (idx, seg) in b.tail_chain.iter().enumerate() {
            let grow = 0.5 + 0.13 * idx as f32; // racine raide, bout souple
            let yaw = (t_secs * TAIL_IDLE_FREQ - idx as f32 * TAIL_IDLE_PHASE).sin()
                * TAIL_IDLE_AMP
                * grow;
            if let Some(e) = seg.entity {
                if let Ok(mut tf) = bones.get_mut(e) {
                    tf.rotation = seg.bind * Quat::from_rotation_y(yaw);
                }
            }
        }
        // Tête : balayage lent gauche↔droite (2 sinus → rythme organique, pas
        // robotique). Yaw autour de Y (bind ≈ identité → vertical monde).
        // 2026-06-07 : HEAD-LOOK DÉSACTIVÉ — tête + nuque STATIQUES au bind en idle.
        // Diagnostic `forgia_skinning_weights.json` : l'os `neck` sur-influence 8936
        // verts (26% du mesh, un vert à 0.987) et `head` 3725 verts → animer la tête/
        // nuque, même de quelques degrés, drague les verts épaule/haut-torse → la
        // déformation (régression du neck-split + « tête déformée à droite » d'origine).
        // Le bind statique = la pose validée « parfaite ». Pour ré-activer le balayage
        // SANS déformer, il faut LOCALISER les poids nuque/tête (re-rig, skinning.rs),
        // pas tweaker la pose ici. Story candidate : skinning weights localization.
        slerp_to_bind(&mut bones, &b.head, 0.15);
        slerp_to_bind(&mut bones, &b.neck, 0.15);

        stats.locomotion_gait_phase = state.gait_phase;
        continue;
    }

    // Walk cycle anatomique — DIRECTION-AWARE (avancer vs reculer).
    // 2026-06-05 : le swing de base lit comme une marche ARRIÈRE quand on AVANCE
    // (convention d'axe d'os, open-question audit confirmée visuellement). On
    // signe donc l'avance du gait par la direction de marche relative au facing :
    // avancer INVERSE le sens du cycle (→ marche avant correcte), reculer le GARDE
    // (→ marche arrière). Knee/ankle suivent la phase inversée naturellement.
    let facing_fwd = (driver_tf.rotation * Vec3::NEG_Z)
        .with_y(0.0)
        .normalize_or_zero();
    let forward_speed = horiz_vel.dot(facing_fwd);
    // FLIP_FORWARD = -1 : avancer (forward_speed ≥ 0) inverse le cycle. Si c'est
    // inversé in-game (avancer reste une marche arrière), passer FLIP_FORWARD à 1.0.
    const FLIP_FORWARD: f32 = -1.0;
    let phase_dir = if forward_speed >= 0.0 {
        FLIP_FORWARD
    } else {
        -FLIP_FORWARD
    };

    let tunables = crate::proc_walk::GaitTunables::for_speed(speed);
    state.gait_phase =
        crate::proc_walk::update_gait_phase(state.gait_phase, speed * phase_dir, dt, &tunables);
    let gait = state.gait_phase;
    let speed_factor =
        ((speed - IDLE_SPEED_THRESHOLD) / crate::proc_walk::SPEED_WALK_PEAK_M_S).clamp(0.0, 1.2);

    let (thigh_l, knee_l, ankle_l) = crate::proc_walk::leg_pose(gait, &tunables);
    let (thigh_r, knee_r, ankle_r) =
        crate::proc_walk::leg_pose((gait + 0.5).rem_euclid(1.0), &tunables);

    // 2026-06-03 : la sonde par-os (forward_lean, position-based) a montré que les
    // CUISSES sont déjà correctes (avant au plant). L'ancien "moonwalk jambes" était
    // un faux positif d'une sonde foot-velocity polluée par la flexion genou (92°).
    // Jambes intactes ; seuls les BRAS étaient ipsilatéraux (inversion ci-dessous).
    compose_swing(&mut bones, &b.left_leg, thigh_l * speed_factor);
    compose_swing(&mut bones, &b.right_leg, thigh_r * speed_factor);
    compose_swing(&mut bones, &b.shin_l, knee_l * speed_factor);
    compose_swing(&mut bones, &b.shin_r, knee_r * speed_factor);
    compose_swing(&mut bones, &b.foot_l, ankle_l * speed_factor);
    compose_swing(&mut bones, &b.foot_r, ankle_r * speed_factor);

    let (arm_l_pitch, elbow_l) =
        crate::proc_walk::arm_pose((gait + 0.5).rem_euclid(1.0), &tunables);
    let (arm_r_pitch, elbow_r) = crate::proc_walk::arm_pose(gait, &tunables);
    // 2026-06-03 : test sur binaire FRAIS → la négation faisait balancer les bras
    // vers l'ARRIÈRE. Retour au pristine = vers l'avant (contralatéral correct).
    // Tout le gait est désormais pristine : le walk d'origine était bon ; les
    // négations venaient de données polluées (binaire stale + sonde pied faussée).
    // 2026-06-05 (workflow swing-axis-derive) : le bras hérite du stance de la
    // CLAVICULE (Rz±40° A-pose) appliqué par l'os parent. compose_stance_swing
    // ne corrigeait l'axe que pour le stance PROPRE du bras → l'axe de swing
    // restait incliné de 40° → les mains TWISTAIENT au lieu de basculer avant-
    // arrière. On corrige l'axe pour la chaîne héritée COMPLÈTE clavicule×bras
    // (préfixe local = bras seul, la clavicule étant déjà un os parent réel).
    // ── Protraction de l'ÉPAULE (la clavicule BOUGE en marche) ───────────────
    // 2026-06-06 (user : "le point de l'épaule est quasiment statique"). Le point
    // d'épaule (arm_L) est le PIVOT du swing du bras → statique par construction.
    // Pour qu'il bouge, on anime la CLAVICULE : rotation autour de la VERTICALE
    // (Y, pré-multipliée = frame torse) en sync avec le swing du bras → l'épaule
    // avance/recule (protraction/rétraction) et le bras entier hérite ce décalage.
    // `clav_*_rot` = rotation locale TOTALE clavicule (protraction × stance A-pose),
    // réutilisée comme parent_stance du bras ET de l'avant-bras → leur swing reste
    // fore-aft (axe corrigé chaîne complète). Y (pas X) car la clavicule s'étend le
    // long de X → un swing X la twisterait sans déplacer l'épaule.
    let clav_l_protract =
        crate::proc_walk::CLAVICLE_PROTRACT_FACTOR * arm_l_pitch * speed_factor;
    let clav_r_protract =
        crate::proc_walk::CLAVICLE_PROTRACT_FACTOR * arm_r_pitch * speed_factor;
    let clav_l_rot = Quat::from_rotation_y(clav_l_protract) * stance.clavicle_l;
    let clav_r_rot = Quat::from_rotation_y(clav_r_protract) * stance.clavicle_r;
    if let Some(e) = b.clavicle_l.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            tf.rotation = b.clavicle_l.bind * clav_l_rot;
        }
    }
    if let Some(e) = b.clavicle_r.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            tf.rotation = b.clavicle_r.bind * clav_r_rot;
        }
    }
    // Bras : swing épaule propre (arm_pitch), axe corrigé pour la clavicule
    // PROTRACTÉE (parent_stance = clav_*_rot, plus le stance statique).
    compose_inherited_stance_swing(
        &mut bones,
        &b.left_arm,
        stance.arm_l,
        clav_l_rot,
        arm_l_pitch * speed_factor,
    );
    compose_inherited_stance_swing(
        &mut bones,
        &b.right_arm,
        stance.arm_r,
        clav_r_rot,
        arm_r_pitch * speed_factor,
    );
    // Avant-bras : hérite clavicule(protractée) × bras → parent_stance complet.
    // 2026-06-03 : signe du coude inversé pour que l'avant-bras suive vers l'avant.
    compose_swing_inherit(
        &mut bones,
        &b.forearm_l,
        clav_l_rot * stance.arm_l,
        -elbow_l * speed_factor,
    );
    compose_swing_inherit(
        &mut bones,
        &b.forearm_r,
        clav_r_rot * stance.arm_r,
        -elbow_r * speed_factor,
    );

    let (pelvic_yaw, pelvic_roll, _bob_y) =
        crate::proc_walk::pelvic_pose(gait, speed_factor, &tunables);
    if let Some(e) = b.hip.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            tf.rotation =
                b.hip.bind * Quat::from_rotation_y(pelvic_yaw) * Quat::from_rotation_z(pelvic_roll);
        }
    }

    if let Some(e) = b.spine.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            // Forward lean du SPINE — petite emphase torse PAR-DESSUS le lean root
            // (dominant, character.rs, désormais +). 2026-06-06 : le vrai coupable
            // "penche en arrière" était le lean ROOT (flippé en +). Le spine
            // accompagne en avant (même convention, sous le Y=π du root). Petit
            // (+0.10) pour que le torse penche un poil plus que les jambes.
            const FORWARD_LEAN_RAD: f32 = 0.10; // ~6° torse, en plus du root
            tf.rotation = b.spine.bind
                * Quat::from_rotation_x(FORWARD_LEAN_RAD * speed_factor)
                * Quat::from_rotation_y(crate::proc_walk::spine_counter_rot(pelvic_yaw));
        }
    }

    // En marche, la tête ET la nuque regardent DEVANT (annule le look-around idle,
    // sinon elles restent figées de travers). Slerp doux vers le bind.
    slerp_to_bind(&mut bones, &b.head, 0.15);
    slerp_to_bind(&mut bones, &b.neck, 0.15);

    let tail_len = b.tail_chain.len();
    for (idx, seg) in b.tail_chain.iter().enumerate() {
        if let Some(e) = seg.entity {
            if let Ok(mut tf) = bones.get_mut(e) {
                let yaw = crate::proc_walk::tail_segment_yaw(idx, tail_len, pelvic_yaw);
                tf.rotation = seg.bind * Quat::from_rotation_y(yaw);
            }
        }
    }

    stats.locomotion_gait_phase = state.gait_phase;
    } // fin boucle multi-perso (QW1 story-637)
    stats.locomotion_us = timer.elapsed_us();
}

// ── Helpers compose/slerp ───────────────────────────────────────────────────

#[inline]
fn compose_swing(
    bones: &mut Query<&mut Transform, (Without<LocomotionState>, Without<LocomotionTarget>)>,
    bone: &BonePose,
    swing_x: f32,
) {
    if let Some(e) = bone.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            tf.rotation = bone.bind * Quat::from_axis_angle(bone.flex_axis, swing_x);
        }
    }
}

/// Comme [`compose_swing`] mais corrige le stance HÉRITÉ d'un os parent. Un
/// avant-bras est enfant d'un bras dont le stance `Rz(±90°)` tourne le frame :
/// son axe latéral local n'est donc plus X mais ±Y. On ré-exprime l'axe via
/// `parent_stance⁻¹` → la flexion du coude reste sagittale (story-496).
#[inline]
fn compose_swing_inherit(
    bones: &mut Query<&mut Transform, (Without<LocomotionState>, Without<LocomotionTarget>)>,
    bone: &BonePose,
    parent_stance: Quat,
    swing_x: f32,
) {
    if let Some(e) = bone.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            let axis = (parent_stance.inverse() * bone.flex_axis).normalize_or(Vec3::X);
            tf.rotation = bone.bind * Quat::from_axis_angle(axis, swing_x);
        }
    }
}

#[inline]
fn slerp_to_bind(
    bones: &mut Query<&mut Transform, (Without<LocomotionState>, Without<LocomotionTarget>)>,
    bone: &BonePose,
    factor: f32,
) {
    if let Some(e) = bone.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            tf.rotation = tf.rotation.slerp(bone.bind, factor);
        }
    }
}

/// Comme [`compose_swing`] mais pour un os qui porte un stance PROPRE
/// (`own_stance`, ex bras Rz(±25°)) ET hérite d'un stance d'un ANCÊTRE
/// (`parent_stance`, ex clavicule Rz(±40°) appliqué par l'os parent réel en monde).
///
/// Le swing doit pivoter autour de l'axe latéral MONDE. Le frame monde de l'os
/// avant swing = `parent_stance * own_stance` (binds Pinocchio ≈ identité), donc
/// l'axe local corrigé = `(parent_stance * own_stance)⁻¹ · flex_axis`. L'ordre
/// `parent * own` est OBLIGATOIRE (un test à stance pure-Z ne le discrimine pas,
/// car deux Rz commutent — cf test à clavicule non-Z).
///
/// ⚠️ Le préfixe appliqué dans le local reste `own_stance` SEUL : `parent_stance`
/// est déjà appliqué par l'os parent réel en monde. Le ré-appliquer ici le baked
/// DEUX FOIS → pose de repos cassée (workflow swing-axis-derive 2026-06-05,
/// key_risk unanime des 3 dérivations). Cas `parent_stance = IDENTITY` = l'ancien
/// `compose_stance_swing` (stance propre seul).
#[inline]
fn compose_inherited_stance_swing(
    bones: &mut Query<&mut Transform, (Without<LocomotionState>, Without<LocomotionTarget>)>,
    bone: &BonePose,
    own_stance: Quat,
    parent_stance: Quat,
    swing_x: f32,
) {
    if let Some(e) = bone.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            let chain = parent_stance * own_stance;
            let axis = (chain.inverse() * bone.flex_axis).normalize_or(Vec3::X);
            tf.rotation = bone.bind * own_stance * Quat::from_axis_angle(axis, swing_x);
        }
    }
}

#[inline]
fn slerp_to_stance(
    bones: &mut Query<&mut Transform, (Without<LocomotionState>, Without<LocomotionTarget>)>,
    bone: &BonePose,
    stance: Quat,
    factor: f32,
) {
    if let Some(e) = bone.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            let target = bone.bind * stance;
            tf.rotation = tf.rotation.slerp(target, factor);
        }
    }
}

// ── Stance offsets loader (story-482 P2b) ──────────────────────────────────

/// Lit `SkeletonTemplate.stance_offsets` depuis le registry asset et insère
/// un Component `StanceOffsets` sur chaque entité `LocomotionTarget` ayant
/// un `LocomotionTemplate`. Hot-reload via `AssetEvent::Modified`.
///
/// Idempotent : insère uniquement si missing OU si l'asset a été modifié
/// (Modified/Added/LoadedWithDependencies). Defer 1 frame si le registry
/// n'a pas encore le template `Ready` (loading async glTF/TOML).
pub fn apply_stance_offsets_from_template(
    mut commands: Commands,
    registry: Res<SkeletonTemplateRegistry>,
    assets: Res<Assets<Genome<SkeletonTemplate>>>,
    mut asset_events: MessageReader<AssetEvent<Genome<SkeletonTemplate>>>,
    q_targets: Query<(Entity, &LocomotionTemplate, Option<&StanceOffsets>), With<LocomotionTarget>>,
    mut dirty: Local<bool>,
) {
    // Détecte hot-reload OR premier load
    for ev in asset_events.read() {
        if matches!(
            ev,
            AssetEvent::Modified { .. }
                | AssetEvent::Added { .. }
                | AssetEvent::LoadedWithDependencies { .. }
        ) {
            *dirty = true;
        }
    }

    for (entity, template, current_stance) in &q_targets {
        // Skip si déjà appliqué ET pas de hot-reload pending
        if current_stance.is_some() && !*dirty {
            continue;
        }
        // Try fetch template — defer 1 frame si pas Ready
        let Some(tpl) = registry.try_get(template.0, &assets) else {
            continue;
        };
        let so = &tpl.stance_offsets;
        let new_stance = StanceOffsets::from_euler_degs(
            so.arm_l_euler_deg,
            so.arm_r_euler_deg,
            so.leg_l_euler_deg,
            so.leg_r_euler_deg,
            so.spine_euler_deg,
            so.hip_euler_deg,
            so.clavicle_l_euler_deg,
            so.clavicle_r_euler_deg,
        );
        if current_stance.is_none() {
            info!(
                "[anim-locomotion] StanceOffsets loaded from TOML template '{}': arm_l_z={:.1}deg arm_r_z={:.1}deg",
                template.0.as_str(),
                so.arm_l_euler_deg[2],
                so.arm_r_euler_deg[2],
            );
        } else {
            info!(
                "[anim-locomotion] StanceOffsets HOT-RELOADED from template '{}': arm_l_z={:.1}deg arm_r_z={:.1}deg",
                template.0.as_str(),
                so.arm_l_euler_deg[2],
                so.arm_r_euler_deg[2],
            );
        }
        commands.entity(entity).insert(new_stance);
    }
    *dirty = false;
}

// ── Sensors ─────────────────────────────────────────────────────────────────

const REX_BONES_LIVE_SENSOR_PATH: &str = "forgia2_rex_bones_live.json";
const REX_BONES_LIVE_INTERVAL_S: f32 = 0.1;

#[derive(Resource, Default)]
pub struct RexBonesLiveSensorTimer {
    accum_s: f32,
}

pub fn write_rex_bones_live_sensor(
    time: Res<Time>,
    mut timer: ResMut<RexBonesLiveSensorTimer>,
    q_cache: Query<&LocomotionBoneCache, With<LocomotionTarget>>,
    bones_q: Query<&Transform, (Without<LocomotionState>, Without<LocomotionTarget>)>,
    entry_count: Res<RpgEntryCount>,
) {
    let dt = time.delta_secs();
    timer.accum_s += dt;
    if timer.accum_s < REX_BONES_LIVE_INTERVAL_S {
        return;
    }
    timer.accum_s = 0.0;

    // Phase A.2 (story-anim-pipeline-observability) — unconditional write :
    // au lieu d'un early-return silencieux, écrire un payload explicite avec
    // `state` + `severity` + `next_step` pour rester visible au sensor_health.
    let cache_opt = q_cache.iter().next();
    let (state_str, severity, next_step, ready, frames_waited, gave_up) = match cache_opt {
        None => (
            "no_locomotion_target",
            "warn",
            "Aucune entité avec LocomotionTarget — vérifier spawn_rex_character (mode RPG entré ?)",
            false, 0u32, false,
        ),
        Some(c) if c.gave_up => (
            "gave_up", "warn",
            "LocomotionBoneCache a abandonné le BFS bones (GIVEUP_FRAMES dépassé). Pinocchio bones absents.",
            c.ready, c.frames_waited, true,
        ),
        Some(c) if !c.ready => (
            "cache_pending", "warn",
            "LocomotionBoneCache.ready=false — attente BFS bones depuis Pinocchio spawn (frames_waited compte).",
            false, c.frames_waited, false,
        ),
        Some(c) => ("ok", "ok", "", true, c.frames_waited, false),
    };
    if !ready {
        let entry_idx = entry_count.0;
        let payload = format!(
            "{{\n  \"id\":\"rex_bones_live\",\n  \"severity\":\"{}\",\n  \"next_step\":\"{}\",\n  \"timestamp_secs\":{:.4},\n  \"entry_index\":{},\n  \"state\":\"{}\",\n  \"cache_ready\":{},\n  \"frames_waited\":{},\n  \"gave_up\":{}\n}}\n",
            severity, next_step, time.elapsed_secs(), entry_idx, state_str, ready, frames_waited, gave_up
        );
        let _ = forgia_core::sensor_io::enqueue(REX_BONES_LIVE_SENSOR_PATH, payload);
        return;
    }
    let cache = cache_opt.unwrap();
    let b = &cache.bones;

    let fmt = |label: &str, bone: &BonePose| -> String {
        let euler = bone
            .entity
            .and_then(|e| bones_q.get(e).ok())
            .map(|t| {
                let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                (x.to_degrees(), y.to_degrees(), z.to_degrees())
            })
            .unwrap_or((f32::NAN, f32::NAN, f32::NAN));
        format!(
            "    \"{}\": {{\"euler_xyz_deg\":[{:.2},{:.2},{:.2}],\"bind_was_identity\":{}}}",
            label,
            euler.0,
            euler.1,
            euler.2,
            (bone.bind.to_euler(EulerRot::XYZ).0.abs() < 0.01
                && bone.bind.to_euler(EulerRot::XYZ).1.abs() < 0.01
                && bone.bind.to_euler(EulerRot::XYZ).2.abs() < 0.01),
        )
    };

    let entry_idx = entry_count.0;
    let json = format!(
        "{{\n  \"id\":\"rex_bones_live\",\n  \"severity\":\"ok\",\n  \"next_step\":\"\",\n  \"state\":\"ok\",\n  \"timestamp_secs\": {:.4},\n  \"entry_index\": {},\n  \"stance_source\": \"StanceOffsets Component (P2)\",\n  \"current_rotations\": {{\n{},\n{},\n{},\n{},\n{},\n{},\n{},\n{},\n{},\n{},\n{},\n{}\n  }}\n}}\n",
        time.elapsed_secs(),
        entry_idx,
        fmt("left_arm", &b.left_arm),
        fmt("right_arm", &b.right_arm),
        fmt("forearm_l", &b.forearm_l),
        fmt("forearm_r", &b.forearm_r),
        fmt("hand_l", &b.hand_l),
        fmt("hand_r", &b.hand_r),
        fmt("clavicle_l", &b.clavicle_l),
        fmt("clavicle_r", &b.clavicle_r),
        fmt("left_leg", &b.left_leg),
        fmt("right_leg", &b.right_leg),
        fmt("spine", &b.spine),
        fmt("hip", &b.hip),
    );
    let _ = forgia_core::sensor_io::enqueue(REX_BONES_LIVE_SENSOR_PATH, json.clone());
    // Indexed per-cycle live snapshot — écrasé à chaque tick mais le path
    // change quand RpgEntryCount incrémente, donc entry_1_live.json reste
    // figé sur le dernier tick du cycle 1 quand on entre dans le cycle 2.
    let indexed_path = format!("forgia2_rex_bones_entry_{}_live.json", entry_idx);
    let _ = forgia_core::sensor_io::enqueue(indexed_path, json);
}

const WALK_POSE_SENSOR_PATH: &str = "forgia2_walk_pose.json";
const WALK_POSE_SENSOR_INTERVAL_S: f32 = 0.5;

#[derive(Resource, Default)]
pub struct WalkPoseSensorTimer {
    accum_s: f32,
}

pub fn write_walk_pose_sensor(
    time: Res<Time>,
    mut timer: ResMut<WalkPoseSensorTimer>,
    q_state: Query<&LocomotionState>,
) {
    let dt = time.delta_secs();
    timer.accum_s += dt;
    if timer.accum_s < WALK_POSE_SENSOR_INTERVAL_S {
        return;
    }
    timer.accum_s = 0.0;

    let Some(state) = q_state.iter().next() else {
        // Phase A.2 unconditional : pas de LocomotionState → mode RPG pas entré.
        let payload = format!(
            "{{\n  \"id\":\"walk_pose\",\n  \"severity\":\"warn\",\n  \"next_step\":\"LocomotionState absent — entrer en GameMode::Rpg pour activer locomotion\",\n  \"state\":\"no_locomotion_state\",\n  \"timestamp_secs\":{:.1}\n}}\n",
            time.elapsed_secs()
        );
        let _ = forgia_core::sensor_io::enqueue(WALK_POSE_SENSOR_PATH, payload);
        return;
    };

    let speed = state.speed;
    let gait = state.gait_phase;
    let speed_factor =
        ((speed - IDLE_SPEED_THRESHOLD) / crate::proc_walk::SPEED_WALK_PEAK_M_S).clamp(0.0, 1.2);
    let snap = crate::proc_walk::WalkPoseSnapshot::from_gait(gait, speed, speed_factor);

    let is_moving = speed > IDLE_SPEED_THRESHOLD;
    let tunables = crate::proc_walk::GaitTunables::for_speed(speed);
    let leg_l_in_swing = gait >= tunables.stance_frac;
    let leg_r_sub = (gait + 0.5).rem_euclid(1.0);
    let leg_r_in_swing = leg_r_sub >= tunables.stance_frac;

    let json = format!(
        r#"{{
  "id": "walk_pose",
  "severity": "ok",
  "next_step": "",
  "state": "ok",
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
  }}
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

    let _ = forgia_core::sensor_io::enqueue(WALK_POSE_SENSOR_PATH, json);
}

// ── Walk direction probe v2 (debug 2026-06-03) ──────────────────────────────
// Mesure PROPRE par-os : orientation avant/arrière de chaque os en MONDE vs sens
// de marche (le Ry(PI) du root est pris en compte via GlobalTransform). Pour
// chaque os : forward_lean = projection horizontale de (tip − head) sur la
// direction de marche. POSITION-based → immune au bruit de vélocité du genou (92°)
// qui faussait la v1 basée sur la vélocité du pied. Dump un buffer roulant (~1
// cycle) pour lire l'oscillation par-os : une cuisse qui atteint son max en
// NÉGATIF quand elle devrait avancer = moonwalk de cette jambe.
const WALK_DIR_PROBE_PATH: &str = "forgia2_walk_dir_probe.json";
const WALK_DIR_PROBE_PUSH_S: f32 = 0.03;
const WALK_DIR_PROBE_WRITE_S: f32 = 0.30;
const WALK_DIR_PROBE_BUF: usize = 32;

/// État accumulé du probe (Local, pas de Resource à enregistrer).
#[derive(Default)]
pub struct WalkDirProbe {
    prev_root: Option<Vec3>,
    buf: Vec<String>,
    push_accum: f32,
    write_accum: f32,
}

pub fn write_walk_dir_probe(
    time: Res<Time>,
    q_cache: Query<(Entity, &LocomotionBoneCache), With<LocomotionTarget>>,
    q_global: Query<&GlobalTransform>,
    q_state: Query<&LocomotionState>,
    mut st: Local<WalkDirProbe>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Some((root_e, cache)) = q_cache.iter().next() else {
        return;
    };
    let Some(state) = q_state.iter().next() else {
        return;
    };
    if !cache.ready {
        return;
    }

    let world = |e: Option<Entity>| e.and_then(|e| q_global.get(e).ok()).map(|g| g.translation());
    let Some(root) = q_global.get(root_e).ok().map(|g| g.translation()) else {
        return;
    };

    let body_vel = st.prev_root.map(|p| (root - p) / dt).unwrap_or(Vec3::ZERO);
    st.prev_root = Some(root);
    let travel = Vec3::new(body_vel.x, 0.0, body_vel.z);
    if travel.length() <= 0.05 {
        return;
    }
    let travel_dir = travel.normalize_or_zero();

    st.push_accum += dt;
    st.write_accum += dt;

    if st.push_accum >= WALK_DIR_PROBE_PUSH_S {
        st.push_accum = 0.0;
        let b = &cache.bones;
        // forward_lean d'un os = projection horizontale de (tip − head) sur la marche.
        // > 0 : l'os pointe vers l'AVANT (sens déplacement). Position-based → immune
        // au bruit de vélocité du genou (92°). tip = tête de l'os enfant.
        let lean = |head_e: Option<Entity>, tip_e: Option<Entity>| -> Option<f32> {
            let h = world(head_e)?;
            let t = world(tip_e)?;
            let d = t - h;
            Some(Vec3::new(d.x, 0.0, d.z).dot(travel_dir))
        };
        let fmt =
            |o: Option<f32>| o.map(|v| format!("{v:.3}")).unwrap_or_else(|| "null".to_string());
        let thigh_l = lean(b.left_leg.entity, b.shin_l.entity);
        let thigh_r = lean(b.right_leg.entity, b.shin_r.entity);
        let arm_l = lean(b.left_arm.entity, b.forearm_l.entity);
        let arm_r = lean(b.right_arm.entity, b.forearm_r.entity);
        let shin_l_foot = lean(b.shin_l.entity, b.foot_l.entity);
        let sample = format!(
            "{{\"gait\":{:.3},\"thigh_l\":{},\"thigh_r\":{},\"arm_l\":{},\"arm_r\":{},\"shin_l_foot\":{}}}",
            state.gait_phase,
            fmt(thigh_l),
            fmt(thigh_r),
            fmt(arm_l),
            fmt(arm_r),
            fmt(shin_l_foot),
        );
        st.buf.push(sample);
        if st.buf.len() > WALK_DIR_PROBE_BUF {
            st.buf.remove(0);
        }
    }

    if st.write_accum >= WALK_DIR_PROBE_WRITE_S {
        st.write_accum = 0.0;
        let samples = st.buf.join(",\n    ");
        let json = format!(
            "{{\n  \"id\": \"walk_dir_probe_v3\",\n  \"BUILD_MARKER\": \"ALL-PRISTINE_2026-06-03-2045\",\n  \"timestamp_secs\": {:.1},\n  \"guide\": \"forward_lean = (tip-head) projete sur la direction de marche. >0 = os pointe AVANT (sens marche), <0 = ARRIERE. thigh_l/r = cuisses (driver direction jambe, signal propre), arm_l/r = bras (propre), shin_l_foot = pied vs genou (fold). Buffer ~1 cycle : chaque os doit atteindre du POSITIF a sa phase de pas-en-avant. Si une cuisse atteint son max en NEGATIF = jambe moonwalk.\",\n  \"buffer\": [\n    {}\n  ]\n}}\n",
            time.elapsed_secs(),
            samples,
        );
        let _ = forgia_core::sensor_io::enqueue(WALK_DIR_PROBE_PATH, json);
    }
}

// ── Full animation sensor (2026-06-03, observabilité 100% pipeline anim) ─────
// Pour CHAQUE os : bind → stance → local → MONDE (GlobalTransform, inclut Ry(PI))
// → direction monde (head→tip, POSITION-based, immune au gimbal) → forward_lean
// → alignement vs AABB mesh. Plus gait complet, root/facing, mesh AABB+desync.
// Conçu pour diagnostiquer SANS deviner : sens du swing en monde, désalignement
// os↔mesh (rings genoux), gimbal euler (Y≈90° après stance), twist, multi-perso.
const ANIM_FULL_PATH: &str = "forgia_anim_full.json";
const ANIM_FULL_WRITE_S: f32 = 0.25;

#[derive(Default)]
pub struct AnimFullState {
    prev_root: Option<Vec3>,
    accum_s: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn write_anim_full_sensor(
    time: Res<Time>,
    q_root: Query<(&LocomotionBoneCache, Option<&StanceOffsets>, &GlobalTransform), With<LocomotionTarget>>,
    q_bone: Query<(&GlobalTransform, &Transform), (Without<LocomotionTarget>, Without<LocomotionState>)>,
    q_state: Query<&LocomotionState>,
    q_mesh: Query<(&GlobalTransform, &Aabb), With<SkinnedMesh>>,
    entry: Res<RpgEntryCount>,
    mut st: Local<AnimFullState>,
) {
    let dt = time.delta_secs();
    st.accum_s += dt;
    if st.accum_s < ANIM_FULL_WRITE_S {
        return;
    }
    st.accum_s = 0.0;

    let n_targets = q_root.iter().count();
    let Some((cache, stance_opt, root_gt)) = q_root.iter().next() else {
        let _ = forgia_core::sensor_io::enqueue(
            ANIM_FULL_PATH,
            format!("{{\"id\":\"anim_full\",\"severity\":\"warn\",\"state\":\"locomotion_target_count={n_targets}\",\"next_step\":\"0 LocomotionTarget (RPG entré ? lineup spawné ?) — capteur lit le 1er target\"}}\n"),
        );
        return;
    };
    let Some(lstate) = q_state.iter().next() else {
        let _ = forgia_core::sensor_io::enqueue(
            ANIM_FULL_PATH,
            "{\"id\":\"anim_full\",\"severity\":\"warn\",\"state\":\"no_locomotion_state\"}\n",
        );
        return;
    };
    if !cache.ready {
        let _ = forgia_core::sensor_io::enqueue(
            ANIM_FULL_PATH,
            format!("{{\"id\":\"anim_full\",\"severity\":\"warn\",\"state\":\"cache_not_ready\",\"frames_waited\":{},\"gave_up\":{}}}\n", cache.frames_waited, cache.gave_up),
        );
        return;
    }

    // ── Root world + travel dir monde ───────────────────────────────────────
    let root_pos = root_gt.translation();
    let (_, root_rot, _) = root_gt.to_scale_rotation_translation();
    let (root_yaw, root_pitch, root_roll) = root_rot.to_euler(EulerRot::YXZ);
    let root_yaw_deg = root_yaw.to_degrees();
    let ry_pi_confirmed = (root_yaw_deg.abs() - 180.0).abs() < 5.0;

    let body_vel = st.prev_root.map(|p| (root_pos - p) / dt).unwrap_or(Vec3::ZERO);
    st.prev_root = Some(root_pos);
    let travel = Vec3::new(body_vel.x, 0.0, body_vel.z);
    let moving_world = travel.length() > 0.05;
    let travel_dir = travel.normalize_or_zero();

    // ── Mesh AABB monde (perso le plus proche du root parmi N) ───────────────
    let (mut mesh_center, mut mesh_half, mut mesh_found, mut best_d) =
        (Vec3::ZERO, Vec3::ZERO, false, f32::MAX);
    for (gt, aabb) in &q_mesh {
        let c = gt.transform_point(Vec3::from(aabb.center));
        let d = (c - root_pos).length_squared();
        if d < best_d {
            best_d = d;
            mesh_center = c;
            let m3 = gt.affine().matrix3;
            let he = Vec3::from(aabb.half_extents);
            // OBB → AABB monde : |M3| * half_extents.
            mesh_half = Vec3::new(
                m3.x_axis.x.abs() * he.x + m3.y_axis.x.abs() * he.y + m3.z_axis.x.abs() * he.z,
                m3.x_axis.y.abs() * he.x + m3.y_axis.y.abs() * he.y + m3.z_axis.y.abs() * he.z,
                m3.x_axis.z.abs() * he.x + m3.y_axis.z.abs() * he.y + m3.z_axis.z.abs() * he.z,
            );
            mesh_found = true;
        }
    }
    let mesh_min_y = mesh_center.y - mesh_half.y;
    let mesh_height = 2.0 * mesh_half.y;

    // ── Gait state complet (intentions proc_walk) ───────────────────────────
    let gait = lstate.gait_phase;
    let speed = lstate.speed;
    let tun = crate::proc_walk::GaitTunables::for_speed(speed);
    let speed_factor =
        ((speed - IDLE_SPEED_THRESHOLD) / crate::proc_walk::SPEED_WALK_PEAK_M_S).clamp(0.0, 1.2);
    let is_moving = speed > IDLE_SPEED_THRESHOLD;
    let rphase = (gait + 0.5).rem_euclid(1.0);
    let (thigh_l, knee_l, ankle_l) = crate::proc_walk::leg_pose(gait, &tun);
    let (thigh_r, knee_r, ankle_r) = crate::proc_walk::leg_pose(rphase, &tun);
    let (arm_l_pitch, elbow_l) = crate::proc_walk::arm_pose(rphase, &tun);
    let (arm_r_pitch, elbow_r) = crate::proc_walk::arm_pose(gait, &tun);
    let (pelvic_yaw, pelvic_roll, _bob) = crate::proc_walk::pelvic_pose(gait, speed_factor, &tun);
    let d = |r: f32| r.to_degrees();

    // ── Helper format par-os ─────────────────────────────────────────────────
    let euler_deg = |q: Quat| {
        let (x, y, z) = q.to_euler(EulerRot::XYZ);
        (x.to_degrees(), y.to_degrees(), z.to_degrees())
    };
    let world_of = |bp: &BonePose| bp.entity.and_then(|e| q_bone.get(e).ok());
    let fmt_bone = |name: &str,
                    bp: &BonePose,
                    child: Option<&BonePose>,
                    stance: Quat,
                    align_frac: Option<f32>|
     -> String {
        let present = bp.entity.is_some();
        let (bx, by, bz) = euler_deg(bp.bind);
        let (sx, sy, sz) = euler_deg(stance);
        let mut head = Vec3::ZERO;
        let (mut lx, mut ly, mut lz) = (0.0_f32, 0.0, 0.0);
        let (mut wx, mut wy, mut wz) = (0.0_f32, 0.0, 0.0);
        if let Some((gt, tf)) = world_of(bp) {
            let (a, b2, c) = euler_deg(tf.rotation);
            lx = a;
            ly = b2;
            lz = c;
            let (_, wr, _) = gt.to_scale_rotation_translation();
            let (a2, b3, c2) = euler_deg(wr);
            wx = a2;
            wy = b3;
            wz = c2;
            head = gt.translation();
        }
        let gimbal = wy.abs() > 85.0;
        let tip = child
            .and_then(|c| c.entity)
            .and_then(|e| q_bone.get(e).ok())
            .map(|(gt, _)| gt.translation());
        let (dir, lean) = match tip {
            Some(t) => {
                let dd = (t - head).normalize_or_zero();
                let l = if moving_world {
                    Vec3::new(dd.x, 0.0, dd.z).dot(travel_dir)
                } else {
                    0.0
                };
                (Some(dd), Some(l))
            }
            None => (None, None),
        };
        let align = align_frac
            .and_then(|f| (mesh_found && present).then_some(head.y - (mesh_min_y + mesh_height * f)));
        let fv = |v: Vec3| format!("[{:.3},{:.3},{:.3}]", v.x, v.y, v.z);
        let fd = |o: Option<Vec3>| o.map(fv).unwrap_or_else(|| "null".to_string());
        let ff = |o: Option<f32>| {
            o.map(|v| format!("{v:.3}")).unwrap_or_else(|| "null".to_string())
        };
        format!(
            "{{\"name\":\"{name}\",\"present\":{present},\"bind_deg\":[{bx:.1},{by:.1},{bz:.1}],\"flex_axis\":\"{}\",\"stance_deg\":[{sx:.1},{sy:.1},{sz:.1}],\"local_deg\":[{lx:.1},{ly:.1},{lz:.1}],\"world_deg\":[{wx:.1},{wy:.1},{wz:.1}],\"gimbal_warn\":{gimbal},\"world_head\":{},\"world_dir\":{},\"forward_lean\":{},\"mesh_align_dy\":{}}}",
            axis_label(bp.flex_axis),
            fv(head),
            fd(dir),
            ff(lean),
            ff(align),
        )
    };

    let id = Quat::IDENTITY;
    let stance = stance_opt.cloned().unwrap_or_default();
    let b = &cache.bones;
    let bones_json = [
        fmt_bone("hip", &b.hip, Some(&b.spine), stance.hip, Some(0.50)),
        fmt_bone("spine", &b.spine, None, stance.spine, Some(0.62)),
        fmt_bone("clavicle_l", &b.clavicle_l, Some(&b.left_arm), stance.clavicle_l, Some(0.80)),
        fmt_bone("left_arm", &b.left_arm, Some(&b.forearm_l), stance.arm_l, Some(0.78)),
        fmt_bone("forearm_l", &b.forearm_l, Some(&b.hand_l), id, None),
        fmt_bone("hand_l", &b.hand_l, None, id, None),
        fmt_bone("clavicle_r", &b.clavicle_r, Some(&b.right_arm), stance.clavicle_r, Some(0.80)),
        fmt_bone("right_arm", &b.right_arm, Some(&b.forearm_r), stance.arm_r, Some(0.78)),
        fmt_bone("forearm_r", &b.forearm_r, Some(&b.hand_r), id, None),
        fmt_bone("hand_r", &b.hand_r, None, id, None),
        fmt_bone("left_leg", &b.left_leg, Some(&b.shin_l), stance.leg_l, Some(0.50)),
        fmt_bone("shin_l", &b.shin_l, Some(&b.foot_l), id, Some(0.25)),
        fmt_bone("foot_l", &b.foot_l, None, id, Some(0.02)),
        fmt_bone("right_leg", &b.right_leg, Some(&b.shin_r), stance.leg_r, Some(0.50)),
        fmt_bone("shin_r", &b.shin_r, Some(&b.foot_r), id, Some(0.25)),
        fmt_bone("foot_r", &b.foot_r, None, id, Some(0.02)),
    ]
    .join(",\n    ");

    // ── Global ───────────────────────────────────────────────────────────────
    let bit = |o: &BonePose| if o.entity.is_some() { "1" } else { "0" };
    let topo = format!(
        "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
        bit(&b.hip), bit(&b.spine), bit(&b.clavicle_l), bit(&b.left_arm),
        bit(&b.forearm_l), bit(&b.hand_l), bit(&b.clavicle_r), bit(&b.right_arm),
        bit(&b.forearm_r), bit(&b.hand_r), bit(&b.left_leg), bit(&b.shin_l),
        bit(&b.foot_l), bit(&b.right_leg), bit(&b.shin_r), bit(&b.foot_r),
    );
    let stance_source = if stance_opt.is_some() && stance.arm_l != Quat::IDENTITY {
        "template_applied"
    } else if stance_opt.is_some() {
        "component_identity"
    } else {
        "no_stance_component"
    };
    let hip_world = world_of(&b.hip).map(|(gt, _)| gt.translation()).unwrap_or(root_pos);
    let desync = if mesh_found { (mesh_center - hip_world).length() } else { -1.0 };
    let leg_l_swing = gait >= tun.stance_frac;
    let leg_r_swing = rphase >= tun.stance_frac;

    let json = format!(
        "{{\n  \"id\": \"anim_full\",\n  \"BUILD_MARKER\": \"ARMS-DOWN_2026-06-05\",\n  \"severity\": \"ok\",\n  \"timestamp_secs\": {:.1},\n  \"entry_index\": {},\n  \"guide\": \"world_deg/world_dir/forward_lean = orientation MONDE (apres Ry PI) — SEUL fiable (euler peut gimbal-lock a Y=90, cf gimbal_warn). forward_lean>0 = os pointe AVANT (sens marche), <0 = arriere. mesh_align_dy = head.y - Y_attendu(AABB mesh) ; |.|>0.10 = os hors feature mesh (rings genoux/epaules). hand_l/r jamais pilotes (figes au bind). stance gauche/droite via stance_deg.\",\n  \"pipeline\": {{\n    \"cache_ready\": {}, \"gave_up\": {}, \"frames_waited\": {},\n    \"locomotion_target_count\": {},\n    \"topology_[hip,spine,clavL,armL,foreL,handL,clavR,armR,foreR,handR,legL,shinL,footL,legR,shinR,footR]\": \"{}\",\n    \"stance_source\": \"{}\"\n  }},\n  \"gait\": {{\n    \"is_moving\": {}, \"moving_world\": {}, \"speed_m_s\": {:.3}, \"speed_factor\": {:.3},\n    \"gait_phase\": {:.3}, \"stance_frac\": {:.3}, \"leg_l_in_swing\": {}, \"leg_r_in_swing\": {},\n    \"thigh_l_deg\": {:.1}, \"thigh_r_deg\": {:.1}, \"knee_l_deg\": {:.1}, \"knee_r_deg\": {:.1},\n    \"ankle_l_deg\": {:.1}, \"ankle_r_deg\": {:.1},\n    \"arm_l_pitch_deg\": {:.1}, \"arm_r_pitch_deg\": {:.1}, \"elbow_l_deg\": {:.1}, \"elbow_r_deg\": {:.1},\n    \"pelvic_yaw_deg\": {:.1}, \"pelvic_roll_deg\": {:.1}\n  }},\n  \"root\": {{\n    \"world_translation\": [{:.3},{:.3},{:.3}],\n    \"world_yaw_deg\": {:.1}, \"ry_pi_confirmed\": {}, \"pitch_deg\": {:.1}, \"roll_deg\": {:.1},\n    \"travel_dir_world\": [{:.3},{:.3},{:.3}]\n  }},\n  \"mesh\": {{\n    \"found\": {}, \"aabb_world_center\": [{:.3},{:.3},{:.3}], \"aabb_world_half\": [{:.3},{:.3},{:.3}],\n    \"mesh_height\": {:.3}, \"hip_vs_mesh_center_desync_m\": {:.3}\n  }},\n  \"foot_ik_see\": \"forgia_foot_ik.json\",\n  \"spring_see\": \"forgia_bone_trace.json (tail)\",\n  \"bones\": [\n    {}\n  ]\n}}\n",
        time.elapsed_secs(),
        entry.0,
        cache.ready, cache.gave_up, cache.frames_waited,
        n_targets,
        topo,
        stance_source,
        is_moving, moving_world, speed, speed_factor,
        gait, tun.stance_frac, leg_l_swing, leg_r_swing,
        d(thigh_l), d(thigh_r), d(knee_l), d(knee_r),
        d(ankle_l), d(ankle_r),
        d(arm_l_pitch), d(arm_r_pitch), d(elbow_l), d(elbow_r),
        d(pelvic_yaw), d(pelvic_roll),
        root_pos.x, root_pos.y, root_pos.z,
        root_yaw_deg, ry_pi_confirmed, root_pitch.to_degrees(), root_roll.to_degrees(),
        travel_dir.x, travel_dir.y, travel_dir.z,
        mesh_found, mesh_center.x, mesh_center.y, mesh_center.z,
        mesh_half.x, mesh_half.y, mesh_half.z, mesh_height, desync,
        bones_json,
    );
    let _ = forgia_core::sensor_io::enqueue(ANIM_FULL_PATH, json);
}

#[cfg(test)]
mod swing_axis_tests {
    use super::{swing_axis_local, LATERAL_AXIS};
    use bevy::math::{EulerRot, Quat, Vec3};
    use std::f32::consts::FRAC_PI_2;

    fn approx_eq(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    /// bind identité → axe local = latéral (X) directement.
    #[test]
    fn identity_bind_gives_lateral() {
        assert!(approx_eq(swing_axis_local(Quat::IDENTITY), LATERAL_AXIS));
        assert!((swing_axis_local(Quat::IDENTITY).length() - 1.0).abs() < 1e-4);
    }

    /// Invariant compose_swing (jambes/tibias/pieds, world_parent≈I) :
    /// `bind * flex_axis == latéral` pour tout bind → flexion toujours sagittale.
    #[test]
    fn compose_swing_world_axis_is_lateral() {
        for bind in [
            Quat::IDENTITY,
            Quat::from_rotation_y(0.3),
            Quat::from_rotation_x(0.5),
            Quat::from_euler(EulerRot::XYZ, 0.2, -0.4, 0.1),
        ] {
            let axis = swing_axis_local(bind);
            assert!(
                approx_eq(bind * axis, LATERAL_AXIS),
                "bind*flex_axis doit = latéral, bind={bind:?}"
            );
        }
    }

    /// Invariant compose_stance_swing (bras, stance Rz±90°) :
    /// `bind * stance * (stance⁻¹ * flex_axis) == latéral`. Le swing reste
    /// sagittal quelle que soit la stance (corrige le défaut frontal story-496).
    #[test]
    fn compose_stance_swing_world_axis_is_lateral() {
        let bind = Quat::IDENTITY;
        let flex = swing_axis_local(bind);
        for stance in [
            Quat::from_rotation_z(FRAC_PI_2),
            Quat::from_rotation_z(-FRAC_PI_2),
        ] {
            let axis = stance.inverse() * flex;
            assert!(
                approx_eq(bind * stance * axis, LATERAL_AXIS),
                "swing bras doit rester latéral, stance={stance:?}"
            );
        }
    }

    /// Invariant compose_swing_inherit (avant-bras enfant d'un bras stancé) :
    /// world_parent ≈ parent_stance, bind_fore ≈ I →
    /// `(parent_stance * bind) * (parent_stance⁻¹ * flex_axis) == latéral`.
    #[test]
    fn compose_swing_inherit_world_axis_is_lateral() {
        let bind = Quat::IDENTITY;
        let flex = swing_axis_local(bind);
        for parent_stance in [
            Quat::from_rotation_z(FRAC_PI_2),
            Quat::from_rotation_z(-FRAC_PI_2),
        ] {
            let axis = parent_stance.inverse() * flex;
            let world = parent_stance * bind * axis;
            assert!(
                approx_eq(world, LATERAL_AXIS),
                "flexion coude doit rester latérale, parent_stance={parent_stance:?}"
            );
        }
    }

    /// 2026-06-05 (workflow swing-axis-derive) : quand la clavicule a un stance
    /// (A-pose Rz±40°), le bras en hérite. L'axe de swing doit être corrigé pour
    /// la chaîne COMPLÈTE clavicule×bras, sinon il reste incliné de 40° → la main
    /// twiste. Test à clavicule NON purement-Z (composante X 15°) : indispensable
    /// car deux Rz commutent et un test Z-only validerait un mauvais ordre.
    #[test]
    fn clavicle_inherited_stance_swing_world_axis_is_lateral() {
        let bind = Quat::IDENTITY;
        let flex = swing_axis_local(bind); // = X
        // Clavicule non-Z (Rz40 * Rx15) pour discriminer l'ordre clav*arm.
        let clav = Quat::from_rotation_z(40.0_f32.to_radians())
            * Quat::from_rotation_x(15.0_f32.to_radians());
        let arm = Quat::from_rotation_z(25.0_f32.to_radians());
        let chain = clav * arm;

        // (1) Axe-monde du swing bras = latéral X (corrigé chaîne complète).
        let arm_axis = (chain.inverse() * flex).normalize_or(Vec3::X);
        assert!(
            approx_eq(chain * arm_axis, LATERAL_AXIS),
            "axe-monde swing bras doit = X, got {:?}",
            chain * arm_axis
        );

        // (2) Falsification du BUG : ancienne formule (arm⁻¹) s'écarte de X (~40°).
        let old_axis = (arm.inverse() * flex).normalize_or(Vec3::X);
        assert!(
            (chain * old_axis).angle_between(LATERAL_AXIS) > 0.5,
            "ancien axe (arm⁻¹) doit dévier ~40° (preuve que le test discrimine)"
        );

        // (3) Falsification de l'ORDRE : (arm*clav)⁻¹ dévie sur clavicule non-Z.
        let rev_axis = ((arm * clav).inverse() * flex).normalize_or(Vec3::X);
        assert!(
            (chain * rev_axis).angle_between(LATERAL_AXIS) > 0.02,
            "ordre inversé (arm*clav) doit dévier sur clavicule non-Z"
        );

        // (4) Avant-bras : parent_stance = clavicule×bras → axe-monde = X.
        let fore_axis = (chain.inverse() * flex).normalize_or(Vec3::X);
        assert!(
            approx_eq(chain * fore_axis, LATERAL_AXIS),
            "axe-monde coude doit = X, got {:?}",
            chain * fore_axis
        );

        // (5) Trajectoire USER-FACING : pivot pur autour de X-monde → le BOUT de
        // la main garde sa composante latérale X (= pas de twist) et balance en Z
        // (avant-arrière, signes opposés). Construction monde : arm_world = frame
        // parent (clavicule = chain⁻¹ déjà inclus) → arm_world(sw) = chain * R(arm_axis,sw)
        // (le préfixe own_stance=arm est DÉJÀ dans chain=clav*arm, binds=I).
        let arm_world = |sw: f32| chain * Quat::from_axis_angle(arm_axis, sw);
        let fore_world = |sw: f32, el: f32| arm_world(sw) * Quat::from_axis_angle(fore_axis, el);
        let tip = |sw: f32, el: f32| fore_world(sw, el) * Vec3::NEG_X; // main local = I
        let rest = tip(0.0, 0.0);
        let fwd = tip(0.5, 0.3);
        let bwd = tip(-0.5, -0.3);
        // (a) twist nul : composante latérale X conservée (pivot autour de X).
        assert!(
            (fwd.x - rest.x).abs() < 0.01,
            "twist bras (Δx avant) = {}, doit être ~0",
            (fwd.x - rest.x).abs()
        );
        assert!(
            (bwd.x - rest.x).abs() < 0.01,
            "twist bras (Δx arrière) = {}",
            (bwd.x - rest.x).abs()
        );
        // (b) fore-aft réel en Z, signes opposés avant/arrière.
        assert!(
            (fwd.z - rest.z).abs() > 0.2,
            "amplitude avant-arrière insuffisante, got {}",
            (fwd.z - rest.z).abs()
        );
        assert!(
            (fwd.z - rest.z) * (bwd.z - rest.z) < 0.0,
            "avant et arrière doivent être en Z opposés"
        );
    }

    /// 2026-06-05 : invariant USER-FACING (pas seulement l'axe). Le bras est en
    /// T-pose (pointe le long de X = l'axe de swing latéral) → SANS stance le swing
    /// le fait TWISTER sur lui-même ("pivote sur lui-même"). La stance Rz±90° le
    /// fait PENDRE (-Y) ; le même swing autour de X devient alors un BALANCEMENT
    /// AVANT-ARRIÈRE (±Z). On vérifie la trajectoire du BOUT du bras, pas l'axe.
    #[test]
    fn arm_swing_is_fore_aft_not_twist() {
        let bind = Quat::IDENTITY;
        let flex = swing_axis_local(bind); // = X
        // arm_l : Rz(+90°) descend le bras gauche (T-pose -X → pendant -Y).
        let stance = Quat::from_rotation_z(FRAC_PI_2);
        // Direction du bras au repos : T-pose gauche pointe le long de -X (bind=I).
        let local_tip = Vec3::NEG_X;
        let arm_dir = |swing: f32| -> Vec3 {
            let axis = stance.inverse() * flex;
            (bind * stance * Quat::from_axis_angle(axis, swing)) * local_tip
        };

        // swing=0 : le bras PEND vers le bas (-Y), plus en T-pose.
        let rest = arm_dir(0.0);
        assert!(rest.y < -0.9, "bras pendant au repos (-Y), got {rest:?}");

        // swing ± : le bout balance AVANT-ARRIÈRE (±Z), opposés, pas un twist.
        let fwd = arm_dir(0.5);
        let bwd = arm_dir(-0.5);
        assert!(
            fwd.z.abs() > 0.3,
            "le swing déplace le bras en Z (fore-aft), got {fwd:?}"
        );
        assert!(fwd.z * bwd.z < 0.0, "swings opposés = Z opposés (avant vs arrière)");
        assert!(
            (fwd - rest).length() > 0.3,
            "le bras bouge vraiment (pas un twist sur place), got {fwd:?}"
        );
    }
}
