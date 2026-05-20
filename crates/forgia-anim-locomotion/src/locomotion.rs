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
use forgia_rig_topology::{analyze_rig_topology, RigTopology};
use forgia_secondary_motion::{SpringBone, SpringBoneChain};

/// Marker à insérer sur le character qui doit recevoir l'animation procédurale.
/// forgia-rpg ajoute ce marker sur `RexCharacter` au spawn.
#[derive(Component)]
pub struct LocomotionTarget;

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

/// Stance offset bras (T-pose Vitruvian → vertical descendant).
/// TODO P2 : extraire dans le template TOML par rig (humanoid_tpose=π/2,
/// humanoid_armsdown=0). Hardcodé en P1 pour zéro régression.
pub const ARM_STANCE_DROP_RAD: f32 = std::f32::consts::FRAC_PI_2;

pub const IDLE_SPEED_THRESHOLD: f32 = 0.15;
pub const IDLE_BREATH_FREQ: f32 = 1.2;
pub const IDLE_BREATH_AMP: f32 = 0.03;

const GIVEUP_FRAMES: u32 = 120;

// ── attach_locomotion_bones (ex attach_rex_bone_systems) ─────────────────────

/// Analyse topologie + calibre Y via AABB + capture bind rotations pose-agnostic.
/// Retry chaque frame jusqu'à `cache.ready = true` ou `GIVEUP_FRAMES` atteint.
pub fn attach_locomotion_bones(
    mut commands: Commands,
    mut q_cache: Query<(Entity, &mut LocomotionBoneCache, &mut Transform), With<LocomotionTarget>>,
    children_query: Query<&Children>,
    names: Query<&Name>,
    transforms: Query<&Transform, Without<LocomotionTarget>>,
    aabbs: Query<&Aabb>,
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
                    diag.name, diag.depth, diag.local_pos.x, diag.local_pos.y, diag.local_pos.z, diag.child_count
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
            let first_child = |e: Entity| -> Option<Entity> {
                children_query.get(e).ok().and_then(|c| c.iter().next())
            };
            let left_arm_e = topo.left_arm;
            let right_arm_e = topo.right_arm;
            let left_leg_e = topo.left_leg;
            let right_leg_e = topo.right_leg;
            let forearm_l_e = left_arm_e.and_then(first_child);
            let forearm_r_e = right_arm_e.and_then(first_child);
            let shin_l_e = left_leg_e.and_then(first_child);
            let shin_r_e = right_leg_e.and_then(first_child);
            let foot_l_e = shin_l_e.and_then(first_child);
            let foot_r_e = shin_r_e.and_then(first_child);
            let bones = ArticulatedBones {
                left_arm: BonePose::from_entity(left_arm_e, &rot_of),
                right_arm: BonePose::from_entity(right_arm_e, &rot_of),
                forearm_l: BonePose::from_entity(forearm_l_e, &rot_of),
                forearm_r: BonePose::from_entity(forearm_r_e, &rot_of),
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
            info!(
                "  [stance] ARM_STANCE_DROP_RAD applied at walk cycle: L=+{:.1}deg, R=-{:.1}deg",
                ARM_STANCE_DROP_RAD.to_degrees(),
                ARM_STANCE_DROP_RAD.to_degrees(),
            );

            // Sensor : dump bind euler + translations
            let tx_of = |opt_e: Option<Entity>| {
                opt_e.and_then(|e| transforms.get(e).ok()).map(|t| t.translation).unwrap_or(Vec3::ZERO)
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
            let json = format!(
                "{{\n  \"captured_at\": \"cache.ready\",\n  \"arm_stance_drop_deg\": {:.1},\n  \"bones\": {{\n    \"left_arm\": {},\n    \"right_arm\": {},\n    \"forearm_l\": {},\n    \"forearm_r\": {},\n    \"left_leg\": {},\n    \"right_leg\": {},\n    \"spine\": {},\n    \"hip\": {}\n  }}\n}}\n",
                ARM_STANCE_DROP_RAD.to_degrees(),
                fmt_bone(&bones.left_arm, bones.forearm_l.entity),
                fmt_bone(&bones.right_arm, bones.forearm_r.entity),
                fmt_bone(&bones.forearm_l, None),
                fmt_bone(&bones.forearm_r, None),
                fmt_bone(&bones.left_leg, bones.shin_l.entity),
                fmt_bone(&bones.right_leg, bones.shin_r.entity),
                fmt_bone(&bones.spine, None),
                fmt_bone(&bones.hip, None),
            );
            if let Err(e) = std::fs::write("forgia_rex_bones.json", &json) {
                warn!("[anim-locomotion] Failed to write forgia_rex_bones.json: {e}");
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
    q_cache: Query<&LocomotionBoneCache, With<LocomotionTarget>>,
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

    let Ok(cache) = q_cache.single() else {
        stats.locomotion_us = timer.elapsed_us();
        return;
    };
    stats.locomotion_cache_ready = cache.ready;
    if !cache.ready {
        stats.locomotion_us = timer.elapsed_us();
        return;
    }

    let b = &cache.bones;

    if !is_moving {
        let t_secs = time.elapsed_secs();
        let breath = (t_secs * IDLE_BREATH_FREQ).sin() * IDLE_BREATH_AMP;
        compose_swing(&mut bones, &b.spine, breath);

        slerp_to_stance(&mut bones, &b.left_arm, ARM_STANCE_DROP_RAD, 0.15);
        slerp_to_stance(&mut bones, &b.right_arm, -ARM_STANCE_DROP_RAD, 0.15);
        for bone in [
            &b.forearm_l, &b.forearm_r,
            &b.left_leg, &b.right_leg, &b.shin_l, &b.shin_r, &b.foot_l, &b.foot_r,
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
    state.gait_phase =
        crate::proc_walk::update_gait_phase(state.gait_phase, speed, dt, &tunables);
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
    compose_stance_swing(&mut bones, &b.left_arm, ARM_STANCE_DROP_RAD, arm_l_pitch * speed_factor);
    compose_stance_swing(&mut bones, &b.right_arm, -ARM_STANCE_DROP_RAD, arm_r_pitch * speed_factor);
    compose_swing(&mut bones, &b.forearm_l, elbow_l * speed_factor);
    compose_swing(&mut bones, &b.forearm_r, elbow_r * speed_factor);

    let (pelvic_yaw, pelvic_roll, _bob_y) =
        crate::proc_walk::pelvic_pose(gait, speed_factor, &tunables);
    if let Some(e) = b.hip.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            tf.rotation = b.hip.bind
                * Quat::from_rotation_y(pelvic_yaw)
                * Quat::from_rotation_z(pelvic_roll);
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
    stance_z: f32,
    swing_x: f32,
) {
    if let Some(e) = bone.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            tf.rotation = bone.bind
                * Quat::from_rotation_z(stance_z)
                * Quat::from_rotation_x(swing_x);
        }
    }
}

#[inline]
fn slerp_to_stance(
    bones: &mut Query<&mut Transform, (Without<LocomotionState>, Without<LocomotionTarget>)>,
    bone: &BonePose,
    stance_z: f32,
    factor: f32,
) {
    if let Some(e) = bone.entity {
        if let Ok(mut tf) = bones.get_mut(e) {
            let target = bone.bind * Quat::from_rotation_z(stance_z);
            tf.rotation = tf.rotation.slerp(target, factor);
        }
    }
}

// ── Sensors ─────────────────────────────────────────────────────────────────

const REX_BONES_LIVE_SENSOR_PATH: &str = "forgia_rex_bones_live.json";
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
) {
    let dt = time.delta_secs();
    timer.accum_s += dt;
    if timer.accum_s < REX_BONES_LIVE_INTERVAL_S {
        return;
    }
    timer.accum_s = 0.0;

    let Ok(cache) = q_cache.single() else { return };
    if !cache.ready {
        return;
    }
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

    let json = format!(
        "{{\n  \"timestamp_secs\": {:.4},\n  \"arm_stance_drop_deg\": {:.1},\n  \"current_rotations\": {{\n{},\n{},\n{},\n{},\n{},\n{},\n{},\n{}\n  }}\n}}\n",
        time.elapsed_secs(),
        ARM_STANCE_DROP_RAD.to_degrees(),
        fmt("left_arm", &b.left_arm),
        fmt("right_arm", &b.right_arm),
        fmt("forearm_l", &b.forearm_l),
        fmt("forearm_r", &b.forearm_r),
        fmt("left_leg", &b.left_leg),
        fmt("right_leg", &b.right_leg),
        fmt("spine", &b.spine),
        fmt("hip", &b.hip),
    );
    let _ = std::fs::write(REX_BONES_LIVE_SENSOR_PATH, json);
}

const WALK_POSE_SENSOR_PATH: &str = "forgia_walk_pose.json";
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
        return;
    };

    let speed = state.speed;
    let gait = state.gait_phase;
    let speed_factor = ((speed - IDLE_SPEED_THRESHOLD)
        / crate::proc_walk::SPEED_WALK_PEAK_M_S)
        .clamp(0.0, 1.2);
    let snap = crate::proc_walk::WalkPoseSnapshot::from_gait(gait, speed, speed_factor);

    let is_moving = speed > IDLE_SPEED_THRESHOLD;
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
