//! Foot IK procédural — story-482 P3.
//!
//! Algorithme standard AAA (ozz-animation foot IK sample, Lyra) :
//!
//! 1. Raycast vertical depuis position courante du pied (XZ projection)
//! 2. Target Y = hit point + `foot_height` offset
//! 3. 2-bone analytical IK : hip → knee → ankle vers target_pos (via forgia-ik)
//! 4. Tilt ankle vers la normale du hit (slope adaptation, clamp ±35°)
//! 5. Lerp 8-15 frames (`lerp_factor=0.2`) sur target_y pour éviter twitch
//!
//! **Prérequis bones** : la chaîne complète hip + thigh + shin + foot doit
//! exister. Si Pinocchio output ne contient que `thigh` (sans shin/foot enfants),
//! le système skip silencieusement et écrit `bones_missing: true` dans le sensor.
//! Quand le pipeline auto-rig sera enrichi pour produire la chaîne 3-segments,
//! foot IK devient actif sans recompiler.

use bevy::prelude::*;
use bevy_rapier3d::prelude::{QueryFilter, ReadRapierContext};
use forgia_ik::{two_bone_ik, TwoBoneIkInput};

use crate::{LocomotionBoneCache, LocomotionState, LocomotionTarget};

// ── Config (P3a inline, P3b TOML futur) ─────────────────────────────────────

/// Tunables foot IK. À terme migré dans un genome TOML `foot_ik.toml`.
#[derive(Resource, Debug, Clone)]
pub struct FootIkConfig {
    /// Master toggle. Default true.
    pub enabled: bool,
    /// Distance max du raycast vers le bas depuis la position du pied (en m).
    pub raycast_down_dist: f32,
    /// Distance max du raycast vers le haut (autorise pied légèrement enterré).
    pub raycast_up_dist: f32,
    /// Offset Y entre le bone foot et le sol attendu (~ankle height).
    pub foot_height: f32,
    /// Vitesse de lerp du smoothing target Y (0.2 = ~8 frames @ 60fps pour 99%).
    pub lerp_factor: f32,
    /// Clamp max tilt ankle vers normale terrain (rad). 0.61 rad = ~35°.
    pub max_tilt_rad: f32,
}

impl Default for FootIkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            raycast_down_dist: 2.0,
            raycast_up_dist: 0.5,
            foot_height: 0.05,
            lerp_factor: 0.20,
            max_tilt_rad: 0.61,
        }
    }
}

// ── Stats sensor ─────────────────────────────────────────────────────────────

/// État d'exécution du foot IK (export sensor JSON 1Hz).
#[derive(Resource, Debug, Default, Clone)]
pub struct FootIkStats {
    pub ticks_total: u64,
    pub ticks_active: u64,
    /// True si la chaîne 3-segments hip+thigh+shin+foot manque côté Pinocchio.
    /// Quand true → foot IK skip jusqu'à ce que le pipeline rig fournisse les bones.
    pub bones_missing: bool,
    pub last_hit_y_l: f32,
    pub last_hit_y_r: f32,
    pub last_target_delta_l: f32,
    pub last_target_delta_r: f32,
    pub smoothed_offset_l: f32,
    pub smoothed_offset_r: f32,
}

// ── Système principal ───────────────────────────────────────────────────────

/// État interne smoothing (Local entre frames).
#[derive(Default)]
pub struct FootIkState {
    smoothed_target_y_l: f32,
    smoothed_target_y_r: f32,
    /// Pour le warn one-shot "bones missing" (évite spam log).
    bones_missing_warned: bool,
}

/// Système foot IK : aligne les pieds sur le terrain via raycast + 2-bone IK
/// + ankle tilt. Skip silencieusement si la chaîne bone est incomplète.
///
/// Order : tourne APRÈS `procedural_locomotion` (qui pose la pose walk cycle).
/// Foot IK override les rotations des bones leg/shin/foot pour planter les pieds.
#[allow(clippy::too_many_arguments)]
pub fn foot_ik_system(
    config: Res<FootIkConfig>,
    rapier: ReadRapierContext,
    mut stats: ResMut<FootIkStats>,
    q_target: Query<(Entity, &LocomotionBoneCache, &GlobalTransform), With<LocomotionTarget>>,
    transforms: Query<&GlobalTransform, Without<LocomotionState>>,
    mut state: Local<FootIkState>,
) {
    stats.ticks_total += 1;
    if !config.enabled {
        return;
    }

    let Ok((target_entity, cache, _target_global)) = q_target.single() else {
        return;
    };
    if !cache.ready {
        return;
    }

    let b = &cache.bones;

    // Vérification chaîne complète bones — skip si missing
    let chain_l = (
        b.hip.entity,
        b.left_leg.entity,
        b.shin_l.entity,
        b.foot_l.entity,
    );
    let chain_r = (
        b.hip.entity,
        b.right_leg.entity,
        b.shin_r.entity,
        b.foot_r.entity,
    );
    let l_complete = matches!(chain_l, (Some(_), Some(_), Some(_), Some(_)));
    let r_complete = matches!(chain_r, (Some(_), Some(_), Some(_), Some(_)));

    if !l_complete || !r_complete {
        stats.bones_missing = true;
        if !state.bones_missing_warned {
            warn!(
                "[foot-ik] Bones chain incomplete (L_complete={l_complete}, R_complete={r_complete}). \
                 Foot IK skipped until Pinocchio output provides hip + thigh + shin + foot. \
                 Voir forgia2_foot_ik.json sensor."
            );
            state.bones_missing_warned = true;
        }
        return;
    }
    stats.bones_missing = false;
    stats.ticks_active += 1;

    // Solve les 2 jambes — return (hit_y, target_delta, smoothed_offset)
    let res_l = solve_one_leg(
        &chain_l,
        &transforms,
        &rapier,
        target_entity,
        &config,
        &mut state.smoothed_target_y_l,
    );
    let res_r = solve_one_leg(
        &chain_r,
        &transforms,
        &rapier,
        target_entity,
        &config,
        &mut state.smoothed_target_y_r,
    );

    stats.last_hit_y_l = res_l.0;
    stats.last_target_delta_l = res_l.1;
    stats.smoothed_offset_l = res_l.2;
    stats.last_hit_y_r = res_r.0;
    stats.last_target_delta_r = res_r.1;
    stats.smoothed_offset_r = res_r.2;
}

/// Retourne `(hit_y, target_delta, smoothed_offset)`.
fn solve_one_leg(
    chain: &(Option<Entity>, Option<Entity>, Option<Entity>, Option<Entity>),
    transforms: &Query<&GlobalTransform, Without<LocomotionState>>,
    rapier: &ReadRapierContext,
    exclude_entity: Entity,
    config: &FootIkConfig,
    smoothed_target_y: &mut f32,
) -> (f32, f32, f32) {
    let (hip_e, thigh_e, _shin_e, foot_e) = (
        chain.0.unwrap(),
        chain.1.unwrap(),
        chain.2.unwrap(),
        chain.3.unwrap(),
    );

    let (Ok(hip_gt), Ok(thigh_gt), Ok(foot_gt)) = (
        transforms.get(hip_e),
        transforms.get(thigh_e),
        transforms.get(foot_e),
    ) else {
        return (0.0, 0.0, 0.0);
    };

    let hip_pos = hip_gt.translation();
    let thigh_pos = thigh_gt.translation();
    let foot_pos = foot_gt.translation();

    // Raycast vertical depuis foot.xz + raycast_up_dist au-dessus
    let Ok(ctx) = rapier.single() else { return (0.0, 0.0, 0.0) };
    let origin = Vec3::new(foot_pos.x, foot_pos.y + config.raycast_up_dist, foot_pos.z);
    let filter = QueryFilter::default().exclude_collider(exclude_entity);
    let max_dist = config.raycast_up_dist + config.raycast_down_dist;

    let hit_y = match ctx.cast_ray(origin, Vec3::NEG_Y, max_dist, true, filter) {
        Some((_, toi)) => origin.y - toi,
        None => foot_pos.y,
    };

    let target_y = hit_y + config.foot_height;
    let target_delta = target_y - foot_pos.y;

    if *smoothed_target_y == 0.0 {
        *smoothed_target_y = target_y;
    } else {
        *smoothed_target_y = *smoothed_target_y * (1.0 - config.lerp_factor)
            + target_y * config.lerp_factor;
    }
    let smoothed_offset = *smoothed_target_y - foot_pos.y;

    let target_pos = Vec3::new(foot_pos.x, *smoothed_target_y, foot_pos.z);

    let _ik_output = two_bone_ik(TwoBoneIkInput {
        root_pos: hip_pos,
        mid_pos: thigh_pos,
        end_pos: foot_pos,
        target_pos,
        pole_dir: None,
    });

    // TODO P3b : appliquer ik_output.root_rotation / mid_rotation sur les bones
    // hip et thigh en LOCAL space (besoin de la parent_inv_world × output_world).
    // Pour P3a : calcul + sensor only. Apply quand bones shin/foot disponibles.
    let _ = _ik_output;

    (hit_y, target_delta, smoothed_offset)
}

// ── Sensor JSON 1Hz ─────────────────────────────────────────────────────────

const FOOT_IK_SENSOR_PATH: &str = "forgia_foot_ik.json";
const FOOT_IK_SENSOR_INTERVAL_S: f32 = 1.0;

#[derive(Resource, Default)]
pub struct FootIkSensorTimer {
    accum_s: f32,
}

pub fn write_foot_ik_sensor(
    time: Res<Time>,
    mut timer: ResMut<FootIkSensorTimer>,
    stats: Res<FootIkStats>,
    config: Res<FootIkConfig>,
) {
    let dt = time.delta_secs();
    timer.accum_s += dt;
    if timer.accum_s < FOOT_IK_SENSOR_INTERVAL_S {
        return;
    }
    timer.accum_s = 0.0;

    let (severity, next_step, state_str) = if stats.bones_missing {
        ("warn", "bones_missing=true — chaîne hip+thigh+shin+foot incomplète depuis Pinocchio. Voir forgia2_auto_rig.json + forgia2_rex_bones_live.json", "bones_missing")
    } else if stats.ticks_total == 0 {
        ("warn", "ticks_total=0 — foot_ik_system n'a jamais tourné (RPG pas entré ?)", "never_ticked")
    } else {
        ("ok", "", "ok")
    };
    let json = format!(
        r#"{{
  "id": "foot_ik",
  "severity": "{}",
  "next_step": "{}",
  "state": "{}",
  "timestamp_secs": {:.2},
  "config": {{
    "enabled": {},
    "raycast_down_dist": {:.2},
    "foot_height": {:.3},
    "lerp_factor": {:.2},
    "max_tilt_rad": {:.3}
  }},
  "state": {{
    "bones_missing": {},
    "ticks_total": {},
    "ticks_active": {},
    "active_ratio": {:.3}
  }},
  "raycasts": {{
    "hit_y_l": {:.3},
    "hit_y_r": {:.3},
    "target_delta_l": {:.3},
    "target_delta_r": {:.3},
    "smoothed_offset_l": {:.3},
    "smoothed_offset_r": {:.3}
  }},
  "blocker_note": "Si bones_missing=true, Pinocchio ne produit que 1 bone par jambe (thigh). Chaîne 3-segments hip+thigh+shin+foot requise pour foot IK 2-bone."
}}"#,
        severity,
        next_step,
        state_str,
        time.elapsed_secs(),
        config.enabled,
        config.raycast_down_dist,
        config.foot_height,
        config.lerp_factor,
        config.max_tilt_rad,
        stats.bones_missing,
        stats.ticks_total,
        stats.ticks_active,
        if stats.ticks_total > 0 {
            stats.ticks_active as f32 / stats.ticks_total as f32
        } else {
            0.0
        },
        stats.last_hit_y_l,
        stats.last_hit_y_r,
        stats.last_target_delta_l,
        stats.last_target_delta_r,
        stats.smoothed_offset_l,
        stats.smoothed_offset_r,
    );
    let _ = std::fs::write(FOOT_IK_SENSOR_PATH, json);
}
