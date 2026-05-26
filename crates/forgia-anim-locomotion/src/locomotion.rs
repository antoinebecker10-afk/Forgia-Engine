//! Procedural locomotion — extraction story-482 P1 depuis forgia-rpg/character.rs.
//!
//! Architecture pose-agnostic (story-481 Action 2) :
//! - `LocomotionTarget` marker = character qui doit recevoir l'anim
//! - `LocomotionState` = entité qui drive l'anim (position + vitesse)
//! - bind rotations capturées une fois à `cache.ready = true`
//! - tout COMPOSE avec bind, jamais OVERWRITE

use bevy::camera::primitives::Aabb;
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
#[derive(Default, Clone, Copy)]
pub struct BonePose {
    pub entity: Option<Entity>,
    pub bind: Quat,
}

impl BonePose {
    pub fn from_entity(entity: Option<Entity>, rot_of: &dyn Fn(Entity) -> Option<Quat>) -> Self {
        let bind = entity.and_then(rot_of).unwrap_or(Quat::IDENTITY);
        Self { entity, bind }
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

/// Vitesse précédente du driver (frame n-1), pour estimer la vélocité.
/// Insérée par forgia-rpg sur l'entité Player.
#[derive(Component, Default)]
pub struct LocomotionState {
    pub prev_pos: Vec3,
    pub speed: f32,
    pub gait_phase: f32,
}

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
pub const ROLL_WADDLE_AMP: f32 = 0.06;
pub const JUMP_SQUASH_AMP: f32 = 0.08;
pub const FALL_STRETCH_AMP: f32 = 0.06;
pub const AIRBORNE_VY_THRESHOLD: f32 = 0.8;

// ARM_STANCE_DROP_RAD supprimé en P2 (2026-05-20). Stance offsets
// data-driven via Component StanceOffsets, défaut humanoid_tpose() pour
// backward compat. Lire depuis SkeletonTemplate TOML quand l'asset
// est chargé via forgia-genome-core.

pub const IDLE_SPEED_THRESHOLD: f32 = 0.15;
pub const IDLE_BREATH_FREQ: f32 = 1.2;
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
            let left_leg_e = topo.left_leg;
            let right_leg_e = topo.right_leg;
            // Name-based resolution (Pinocchio flat hierarchy).
            // Templates Forgia humanoid : forearm_L/R, shin_L/R, foot_L/R.
            // BipedLizard : forearm_L/R, shin_L/R, foot_L/R (mêmes noms).
            let forearm_l_e = lookup("forearm_L");
            let forearm_r_e = lookup("forearm_R");
            let hand_l_e = lookup("hand_L");
            let hand_r_e = lookup("hand_R");
            let clavicle_l_e = lookup("clavicle_L");
            let clavicle_r_e = lookup("clavicle_R");
            let shin_l_e = lookup("shin_L");
            let shin_r_e = lookup("shin_R");
            let foot_l_e = lookup("foot_L");
            let foot_r_e = lookup("foot_R");
            info!(
                "[anim-locomotion] Name-lookup bones : forearm L/R={}/{}, hand L/R={}/{}, clavicle L/R={}/{}, shin L/R={}/{}, foot L/R={}/{}",
                forearm_l_e.is_some(), forearm_r_e.is_some(),
                hand_l_e.is_some(), hand_r_e.is_some(),
                clavicle_l_e.is_some(), clavicle_r_e.is_some(),
                shin_l_e.is_some(), shin_r_e.is_some(),
                foot_l_e.is_some(), foot_r_e.is_some(),
            );
            let bones = ArticulatedBones {
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
                tail_chain: topo
                    .tail_chain
                    .iter()
                    .map(|&e| BonePose::from_entity(Some(e), &rot_of))
                    .collect(),
            };

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
                    "{{\"present\":{},\"euler_xyz_deg\":[{:.2},{:.2},{:.2}],\"head\":[{:.3},{:.3},{:.3}],\"tip_local\":[{:.3},{:.3},{:.3}]}}",
                    b.entity.is_some(),
                    x.to_degrees(), y.to_degrees(), z.to_degrees(),
                    head.x, head.y, head.z,
                    tip.x, tip.y, tip.z,
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
            if let Err(e) = std::fs::write("forgia2_rex_bones.json", &json) {
                warn!("[anim-locomotion] Failed to write forgia2_rex_bones.json: {e}");
            }
            // Indexed per-cycle sensor — persists across RPG re-entries.
            let indexed_path = format!("forgia2_rex_bones_entry_{}_bind.json", entry_idx);
            if let Err(e) = std::fs::write(&indexed_path, &json) {
                warn!("[anim-locomotion] Failed to write {indexed_path}: {e}");
            } else {
                info!("[anim-locomotion] Bind snapshot persisted → {indexed_path}");
            }
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
    q_cache: Query<(&LocomotionBoneCache, Option<&StanceOffsets>), With<LocomotionTarget>>,
    mut bones: Query<&mut Transform, (Without<LocomotionState>, Without<LocomotionTarget>)>,
    mut stats: ResMut<AnimLayerStats>,
) {
    let timer = AnimTimer::start();
    stats.locomotion_active = true;

    let dt = time.delta_secs();
    if dt <= 0.0 {
        stats.locomotion_us = timer.elapsed_us();
        return;
    }

    let Ok((driver_tf, mut state)) = q_driver.single_mut() else {
        stats.locomotion_active = false;
        stats.locomotion_us = timer.elapsed_us();
        return;
    };

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

    let Ok((cache, stance_opt)) = q_cache.single() else {
        stats.locomotion_us = timer.elapsed_us();
        return;
    };
    stats.locomotion_cache_ready = cache.ready;
    if !cache.ready {
        stats.locomotion_us = timer.elapsed_us();
        return;
    }

    let b = &cache.bones;
    // Story-482 P2 : stance offsets data-driven via Component. Fallback
    // identity si pas de Component (mesh assumé déjà en pose game).
    let stance_default = StanceOffsets::default();
    let stance = stance_opt.unwrap_or(&stance_default);

    if !is_moving {
        let t_secs = time.elapsed_secs();
        let breath = (t_secs * IDLE_BREATH_FREQ).sin() * IDLE_BREATH_AMP;
        compose_swing(&mut bones, &b.spine, breath);

        slerp_to_stance(&mut bones, &b.left_arm, stance.arm_l, 0.15);
        slerp_to_stance(&mut bones, &b.right_arm, stance.arm_r, 0.15);
        // P2c : clavicle stance (no swing, pure stance offset).
        slerp_to_stance(&mut bones, &b.clavicle_l, stance.clavicle_l, 0.15);
        slerp_to_stance(&mut bones, &b.clavicle_r, stance.clavicle_r, 0.15);
        for bone in [
            &b.forearm_l,
            &b.forearm_r,
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

        stats.locomotion_gait_phase = state.gait_phase;
        stats.locomotion_us = timer.elapsed_us();
        return;
    }

    // Walk cycle anatomique
    let tunables = crate::proc_walk::GaitTunables::for_speed(speed);
    state.gait_phase = crate::proc_walk::update_gait_phase(state.gait_phase, speed, dt, &tunables);
    let gait = state.gait_phase;
    let speed_factor =
        ((speed - IDLE_SPEED_THRESHOLD) / crate::proc_walk::SPEED_WALK_PEAK_M_S).clamp(0.0, 1.2);

    let (thigh_l, knee_l, ankle_l) = crate::proc_walk::leg_pose(gait, &tunables);
    let (thigh_r, knee_r, ankle_r) =
        crate::proc_walk::leg_pose((gait + 0.5).rem_euclid(1.0), &tunables);

    compose_swing(&mut bones, &b.left_leg, thigh_l * speed_factor);
    compose_swing(&mut bones, &b.right_leg, thigh_r * speed_factor);
    compose_swing(&mut bones, &b.shin_l, knee_l * speed_factor);
    compose_swing(&mut bones, &b.shin_r, knee_r * speed_factor);
    compose_swing(&mut bones, &b.foot_l, ankle_l * speed_factor);
    compose_swing(&mut bones, &b.foot_r, ankle_r * speed_factor);

    let (arm_l_pitch, elbow_l) =
        crate::proc_walk::arm_pose((gait + 0.5).rem_euclid(1.0), &tunables);
    let (arm_r_pitch, elbow_r) = crate::proc_walk::arm_pose(gait, &tunables);
    compose_stance_swing(
        &mut bones,
        &b.left_arm,
        stance.arm_l,
        arm_l_pitch * speed_factor,
    );
    compose_stance_swing(
        &mut bones,
        &b.right_arm,
        stance.arm_r,
        arm_r_pitch * speed_factor,
    );
    // P2c : clavicle stance (no swing pendant walk — la clavicle est statique
    // sur la pose game, seule l'arm pitch fait l'oscillation walk cycle).
    slerp_to_stance(&mut bones, &b.clavicle_l, stance.clavicle_l, 0.25);
    slerp_to_stance(&mut bones, &b.clavicle_r, stance.clavicle_r, 0.25);
    compose_swing(&mut bones, &b.forearm_l, elbow_l * speed_factor);
    compose_swing(&mut bones, &b.forearm_r, elbow_r * speed_factor);

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
            tf.rotation = b.spine.bind
                * Quat::from_rotation_y(crate::proc_walk::spine_counter_rot(pelvic_yaw));
        }
    }

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
            tf.rotation = bone.bind * Quat::from_rotation_x(swing_x);
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

#[inline]
fn compose_stance_swing(
    bones: &mut Query<&mut Transform, (Without<LocomotionState>, Without<LocomotionTarget>)>,
    bone: &BonePose,
    stance: Quat,
    swing_x: f32,
) {
    if let Some(e) = bone.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            tf.rotation = bone.bind * stance * Quat::from_rotation_x(swing_x);
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
        let _ = std::fs::write(REX_BONES_LIVE_SENSOR_PATH, &payload);
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
    let _ = std::fs::write(REX_BONES_LIVE_SENSOR_PATH, &json);
    // Indexed per-cycle live snapshot — écrasé à chaque tick mais le path
    // change quand RpgEntryCount incrémente, donc entry_1_live.json reste
    // figé sur le dernier tick du cycle 1 quand on entre dans le cycle 2.
    let indexed_path = format!("forgia2_rex_bones_entry_{}_live.json", entry_idx);
    let _ = std::fs::write(&indexed_path, json);
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

    let Ok(state) = q_state.single() else {
        // Phase A.2 unconditional : pas de LocomotionState → mode RPG pas entré.
        let payload = format!(
            "{{\n  \"id\":\"walk_pose\",\n  \"severity\":\"warn\",\n  \"next_step\":\"LocomotionState absent — entrer en GameMode::Rpg pour activer locomotion\",\n  \"state\":\"no_locomotion_state\",\n  \"timestamp_secs\":{:.1}\n}}\n",
            time.elapsed_secs()
        );
        let _ = std::fs::write(WALK_POSE_SENSOR_PATH, payload);
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

    if let Err(e) = std::fs::write(WALK_POSE_SENSOR_PATH, json) {
        warn!("[anim-locomotion] walk pose sensor write failed: {e}");
    }
}
