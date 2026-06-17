//! # forgia-auto-rig — Runtime Skeleton Synthesis (story-440 Phase 1A)
//!
//! Donne un squelette à un mesh statique en 1 frame. Phase 1A = bones spawnés
//! en hiérarchie, **pas encore** de skinning weights (le mesh ne déforme pas).
//! Consommé par `forgia-rig-topology` (classifie L/R legs/arms/spine/tail) puis
//! `forgia-rpg::character::procedural_locomotion` (anime).
//!
//! ## Usage
//!
//! ```ignore
//! commands.entity(mesh_root).insert(NeedsAutoRig::Template(AutoRigTemplate::BipedLizard));
//! // 1+ frame plus tard (le temps que la Scene GLB instance ses Children + AABB) :
//! // - bones spawnés en hiérarchie sous mesh_root
//! // - marker AutoRigged inséré
//! // - NeedsAutoRig retiré
//! ```
//!
//! ## Algorithme Phase 1A
//!
//! 1. Walk descendants → calcule AABB local (centre XZ + min Y + height).
//! 2. Scale uniforme du template par `mesh_height`.
//! 3. Spawn bones template en BFS, parentés en hiérarchie. Hip est enfant
//!    direct du mesh root (= compatible avec `analyze_rig_topology` qui cherche
//!    "l'enfant direct avec le plus de descendants").
//! 4. Insert `AutoRigged` marker, retire `NeedsAutoRig`.
//!
//! ## Phase 1B (à venir) — skinning weights nearest-bone + SkinnedMesh injection.
//! ## Phase 1C (à venir) — templates en TOML genome hot-reload.

use bevy::prelude::*;
use forgia_core::prelude::GameSet;

mod anatomy_detect;
mod debug_gizmos;
mod pinocchio_pipeline;
mod skinning;

pub use anatomy_detect::{detect_landmarks_from_vertices, MeshLandmarks};
pub use debug_gizmos::{draw_rig_gizmos, AutoRigGizmosConfig};
pub use pinocchio_pipeline::{auto_rig_pinocchio_v1, auto_rig_to_skeleton_template_id};
pub use skinning::{
    inject_skinning_for_rigged_meshes, BoneEntity, SkinningConfig, SkinningInjected,
};

pub mod prelude {
    pub use crate::{
        AutoRigGizmosConfig, AutoRigStats, AutoRigTemplate, AutoRigged, BoneEntity,
        ForgiaAutoRigPlugin, MeshLandmarks, NeedsAutoRig, SkinningInjected,
    };
}

// ── Sensor / observability ──────────────────────────────────────────────────

/// Intervalle d'écriture `forgia2_auto_rig.json` (secondes).
pub const SENSOR_INTERVAL_S: f32 = 1.0;
/// Seuil au-delà duquel un mesh "pending" depuis trop longtemps déclenche
/// l'alerte health (= GLB jamais loaded ou AABB jamais calculable).
pub const PENDING_STALE_THRESHOLD_S: f32 = 5.0;

/// Stats agrégées du crate. Mutées par `auto_rig_pending_meshes`, lues par
/// le sensor writer.
#[derive(Resource, Default, Debug)]
pub struct AutoRigStats {
    pub total_rig_successes: u32,
    pub total_rig_failures_degenerate_aabb: u32,
    pub last_template: Option<AutoRigTemplate>,
    pub last_bone_count: usize,
    pub last_mesh_height: f32,
    pub last_rigged_at_secs: f32,
    // ── Auto-detect diagnostics (debug fix 2026-05-17 PM) ──────────────────
    /// Template demandé par le caller (NeedsAutoRig::Template(...)).
    pub last_requested_template: Option<AutoRigTemplate>,
    /// Landmarks anatomiques détectés (None si vertex_count < 100 = fallback).
    pub last_landmarks: Option<MeshLandmarks>,
    /// True si auto-switch template a déclenché (requested != effective).
    pub last_auto_switched: bool,
    // ── Deep debug (2026-05-17 evening) — soupçon mismatch AABB vs visible ─
    /// AABB local detected pour le dernier rig (repère mesh-root-local).
    pub last_aabb_min: Vec3,
    pub last_aabb_max: Vec3,
    /// Nombre de Mesh3d entities trouvés dans la hiérarchie (>1 = accessoires).
    pub last_mesh3d_count: usize,
    /// Total vertices vus dans tous les Mesh3d (utile vs sample landmarks).
    pub last_total_mesh_vertices: usize,
    /// Scales appliqués au placement (1.0 = no override = no landmarks).
    pub last_spine_scale: f32,
    pub last_leg_scale: f32,
    pub last_arm_scale: f32,
    /// Positions monde (= mesh-local, sans propager rex_tf) de bones clés.
    /// Permet à Antoine de comparer visuellement vs AABB et identifier le décalage.
    pub last_hip_world_y: f32,
    pub last_head_world_y: f32,
    pub last_foot_l_world_y: f32,
    pub last_forearm_l_world_x: f32,
    // ── Pinocchio V1 backend stats (story-440 Phase 1) ─────────────────────
    pub last_pinocchio_voxel_count: usize,
    pub last_pinocchio_sphere_count: usize,
    pub last_pinocchio_embedding_cost: f32,
    pub last_pinocchio_voxelize_ms: u32,
    pub last_pinocchio_medial_ms: u32,
    // ── Phase 1B skinning stats ────────────────────────────────────────────
    pub total_meshes_skinned: u32,
    pub total_meshes_skinning_failed: u32,
    pub last_verts_skinned: u32,
    pub last_skinning_at_secs: f32,
}

/// État interne sensor (timers accum). Public mais fields privés —
/// inspect via `Res<AutoRigSensorTimer>` côté outils debug si besoin.
#[derive(Resource, Default)]
pub struct AutoRigSensorTimer {
    accum_s: f32,
    /// Wall-clock du premier `pending` non encore résolu — reset à 0 dès que
    /// pending_count retombe à 0.
    first_pending_seen_secs: Option<f32>,
}

// ── Components ──────────────────────────────────────────────────────────────

/// Demande un auto-rig sur l'entité (mesh root). Retiré une fois processé.
#[derive(Component, Debug, Clone)]
pub enum NeedsAutoRig {
    Template(AutoRigTemplate),
}

/// Marker posé sur le mesh root après rig réussi. Idempotence.
#[derive(Component, Debug, Default)]
pub struct AutoRigged {
    pub template: Option<AutoRigTemplate>,
    pub bone_count: usize,
    pub mesh_height: f32,
}

// ── Templates ───────────────────────────────────────────────────────────────

/// Identifiant de morphologie demandé par le caller (`NeedsAutoRig::Template(...)`).
/// API publique stable du crate auto-rig — mappé vers `SkeletonTemplateId` du
/// crate `forgia-skeleton-template` via `auto_rig_to_skeleton_template_id`.
///
/// Story-480 Phase 3 (2026-05-20) : le path TemplateFit (constantes
/// `HUMANOID_BONES`/`BIPED_LIZARD_BONES`, `BoneDef`/`PlacedBone`,
/// `place_template[_with_landmarks]`, `validate_template`, `auto_rig_pending_meshes`,
/// enum `AutoRigBackend`) a été supprimé. Pinocchio est le seul path runtime
/// et la source de vérité vit dans le TOML + Registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoRigTemplate {
    /// 20 bones bipède Vitruvien (humain, gobelin, orc, nain, céleste).
    Humanoid,
    /// 20 bones bipède Vitruvien variante arms-down (story-601) — meshes générés
    /// bras le long du corps (Cyber et assets IA similaires).
    HumanoidApose,
    /// 20 bones bipède lézard avec tail 4-segments (Rex).
    BipedLizard,
}

// ── Sensor writer ───────────────────────────────────────────────────────────

/// Interprète les stats pour produire un hint runtime pour Antoine. Vérifie
/// les invariants attendus (foot ≈ aabb.min.y, head ≈ aabb.max.y, etc.) et
/// flag les mismatches courants (sub-meshes invisibles, AABB outliers).
fn diagnose_interpretation(s: &AutoRigStats) -> String {
    let mut issues: Vec<String> = Vec::new();
    if s.last_mesh3d_count > 1 {
        issues.push(format!(
            "{} Mesh3d entities (accessoires/cape ?) — AABB peut inclure invisibles",
            s.last_mesh3d_count
        ));
    }
    let foot_to_bottom_diff = (s.last_foot_l_world_y - s.last_aabb_min.y).abs();
    if foot_to_bottom_diff > 0.10 {
        issues.push(format!(
            "foot_L Y={:.2} vs aabb.min.y={:.2} (diff {:.2}m) — foot pas au sol",
            s.last_foot_l_world_y, s.last_aabb_min.y, foot_to_bottom_diff
        ));
    }
    let head_to_top_diff = (s.last_head_world_y - s.last_aabb_max.y).abs();
    if head_to_top_diff > 0.10 {
        issues.push(format!(
            "head Y={:.2} vs aabb.max.y={:.2} (diff {:.2}m) — head pas au sommet",
            s.last_head_world_y, s.last_aabb_max.y, head_to_top_diff
        ));
    }
    if issues.is_empty() {
        "OK : foot ≈ aabb_min, head ≈ aabb_max, single Mesh3d".to_string()
    } else {
        issues.join(" | ")
    }
}

const SENSOR_JSON_PATH: &str = "forgia2_auto_rig.json";
const SENSOR_HEALTH_PATH: &str = "forgia2_auto_rig_health.json";

/// Écrit `forgia2_auto_rig.json` toutes les `SENSOR_INTERVAL_S`. Émet
/// `forgia2_auto_rig_health.json` (severity warning) si un mesh reste `pending`
/// plus de `PENDING_STALE_THRESHOLD_S` (= GLB jamais loaded / AABB jamais
/// calculable). Convention V1 : fichier health absent = OK.
pub fn write_auto_rig_sensor(
    time: Res<Time>,
    mut timer: ResMut<AutoRigSensorTimer>,
    stats: Res<AutoRigStats>,
    q_pending: Query<Entity, (With<NeedsAutoRig>, Without<AutoRigged>)>,
    q_rigged: Query<&AutoRigged>,
) {
    let dt = time.delta_secs();
    timer.accum_s += dt;

    let pending_count = q_pending.iter().count();
    let rigged_count = q_rigged.iter().count();
    let bones_total: usize = q_rigged.iter().map(|r| r.bone_count).sum();

    // Track depuis combien de temps il y a au moins 1 pending non résolu.
    let now = time.elapsed_secs();
    if pending_count == 0 {
        timer.first_pending_seen_secs = None;
    } else if timer.first_pending_seen_secs.is_none() {
        timer.first_pending_seen_secs = Some(now);
    }
    let pending_age_s = timer
        .first_pending_seen_secs
        .map(|t0| now - t0)
        .unwrap_or(0.0);

    if timer.accum_s < SENSOR_INTERVAL_S {
        return;
    }
    timer.accum_s = 0.0;

    let (severity, state_str, next_step) = if pending_age_s > PENDING_STALE_THRESHOLD_S {
        ("warn", "stale_pending", "Mesh(es) coincés en pending — AABB jamais computed (GLB load échoué ou pas d'Aabb component)")
    } else if pending_count > 0 {
        ("ok", "pending_in_progress", "")
    } else if rigged_count > 0 {
        ("ok", "rigged", "")
    } else {
        (
            "ok",
            "no_target",
            "Aucune entité NeedsAutoRig — pipeline en attente (mode RPG pas entré ?)",
        )
    };
    let last_template_str = stats
        .last_template
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|| "null".into());
    let last_template_json = if stats.last_template.is_some() {
        format!("\"{}\"", last_template_str)
    } else {
        "null".into()
    };
    let requested_template_json = stats
        .last_requested_template
        .map(|t| format!("\"{:?}\"", t))
        .unwrap_or_else(|| "null".into());
    let landmarks_json = match stats.last_landmarks {
        Some(l) => format!(
            "{{ \"vertex_count\": {}, \"hip_y_frac\": {:.3}, \"shoulder_y_frac\": {:.3}, \"head_y_frac\": {:.3}, \"aspect_ratio_xy\": {:.3}, \"hip_width_frac\": {:.3}, \"has_tail\": {}, \"tail_in_positive_z\": {}, \"torso_x_off\": {:.3}, \"torso_z_off\": {:.3}, \"looks_humanoid\": {} }}",
            l.vertex_count, l.hip_y_frac, l.shoulder_y_frac, l.head_y_frac,
            l.aspect_ratio_xy, l.hip_width_frac, l.has_tail, l.tail_in_positive_z,
            l.torso_center_x_offset_frac, l.torso_center_z_offset_frac, l.looks_humanoid()
        ),
        None => "null".into(),
    };

    let json = format!(
        r#"{{
  "id": "auto_rig",
  "severity": "{}",
  "next_step": "{}",
  "state": "{}",
  "timestamp_secs": {:.1},
  "rigged_count": {},
  "pending_count": {},
  "pending_age_s": {:.2},
  "bones_total": {},
  "total_rig_successes": {},
  "total_rig_failures_degenerate_aabb": {},
  "last_template": {},
  "last_requested_template": {},
  "last_auto_switched": {},
  "last_landmarks": {},
  "last_bone_count": {},
  "last_mesh_height": {:.3},
  "last_rigged_at_secs": {:.1},
  "debug": {{
    "aabb_min": [{:.3}, {:.3}, {:.3}],
    "aabb_max": [{:.3}, {:.3}, {:.3}],
    "mesh3d_count": {},
    "total_mesh_vertices": {},
    "spine_scale": {:.3},
    "leg_scale": {:.3},
    "arm_scale": {:.3},
    "bones_world": {{
      "hip_y": {:.3},
      "head_y": {:.3},
      "foot_l_y": {:.3},
      "forearm_l_x": {:.3}
    }},
    "interpretation": "{}"
  }},
  "skinning": {{
    "total_meshes_skinned": {},
    "total_meshes_failed": {},
    "last_verts_skinned": {},
    "last_skinning_at_secs": {:.1}
  }},
  "severity": "{}"
}}"#,
        severity,
        next_step,
        state_str,
        now,
        rigged_count,
        pending_count,
        pending_age_s,
        bones_total,
        stats.total_rig_successes,
        stats.total_rig_failures_degenerate_aabb,
        last_template_json,
        requested_template_json,
        stats.last_auto_switched,
        landmarks_json,
        stats.last_bone_count,
        stats.last_mesh_height,
        stats.last_rigged_at_secs,
        // debug block
        stats.last_aabb_min.x,
        stats.last_aabb_min.y,
        stats.last_aabb_min.z,
        stats.last_aabb_max.x,
        stats.last_aabb_max.y,
        stats.last_aabb_max.z,
        stats.last_mesh3d_count,
        stats.last_total_mesh_vertices,
        stats.last_spine_scale,
        stats.last_leg_scale,
        stats.last_arm_scale,
        stats.last_hip_world_y,
        stats.last_head_world_y,
        stats.last_foot_l_world_y,
        stats.last_forearm_l_world_x,
        diagnose_interpretation(&stats),
        // skinning block
        stats.total_meshes_skinned,
        stats.total_meshes_skinning_failed,
        stats.last_verts_skinned,
        stats.last_skinning_at_secs,
        severity,
    );
    if let Err(e) = std::fs::write(SENSOR_JSON_PATH, json) {
        warn!("[forgia-auto-rig] sensor write failed: {e}");
    }

    // Health side-file : créé seulement si severity != "ok", supprimé sinon
    // (convention V1 reprise de forgia-anim-debug).
    if severity == "ok" {
        let _ = std::fs::remove_file(SENSOR_HEALTH_PATH);
    } else {
        let health = format!(
            r#"{{
  "timestamp_secs": {:.1},
  "severity": "{}",
  "message": "{} mesh(es) pending auto-rig since {:.1}s — AABB likely never computed (GLB load failure / mesh has no Aabb component)",
  "next_step": "Read forgia2_auto_rig.json then inspect the SceneRoot of pending entities — check that GLB instantiated children with Aabb component (cargo run + log filter '[forgia-auto-rig]')"
}}"#,
            now, severity, pending_count, pending_age_s,
        );
        if let Err(e) = std::fs::write(SENSOR_HEALTH_PATH, health) {
            warn!("[forgia-auto-rig] health write failed: {e}");
        }
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct ForgiaAutoRigPlugin;

impl Plugin for ForgiaAutoRigPlugin {
    fn build(&self, app: &mut App) {
        // Story-480 Phase 3 : SkeletonTemplatePlugin gère le load asset
        // (init_asset + register_asset_loader + Startup load + Registry).
        // ForgiaAutoRigPlugin doit l'inclure exactement une fois — `is_plugin_added`
        // gère l'idempotence si le user l'a déjà ajouté à la racine.
        if !app.is_plugin_added::<forgia_skeleton_template::SkeletonTemplatePlugin>() {
            app.add_plugins(forgia_skeleton_template::SkeletonTemplatePlugin::default());
        }
        app.init_resource::<AutoRigStats>()
            .init_resource::<AutoRigSensorTimer>()
            .init_resource::<SkinningConfig>()
            .init_resource::<AutoRigGizmosConfig>()
            .add_systems(Update, draw_rig_gizmos.in_set(GameSet::Effects))
            // Producer Movement : Pinocchio est désormais le seul path (story-480).
            .add_systems(
                Update,
                auto_rig_pinocchio_v1.in_set(GameSet::Movement),
            )
            // Skinning injection : ACTIVÉ 2026-05-18 (story-451 Phase 1).
            // Pinocchio Phase 1B validé (20 bones BipedLizard Rex OK depuis
            // story-440). Le skinning per-vertex nearest-bone applique les
            // bone transforms au mesh → Rex peut être animé par bone rotation.
            // Si mesh aplati au retour : revert via re-commenter cette ligne.
            .add_systems(PostUpdate, inject_skinning_for_rigged_meshes)
            // Sensor dans Sensors set (après Effects, avant UI) — pattern V2.
            .add_systems(
                Update,
                write_auto_rig_sensor.in_set(GameSet::Sensors),
            );
    }
}

// ── Tests headless ──────────────────────────────────────────────────────────
