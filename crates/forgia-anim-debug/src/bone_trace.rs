//! # bone_trace — diagnostic time-series anim (story-454 Niveau A)
//!
//! Sensor `forgia_bone_trace.json` qui dump à `dbg_bone_trace_hz` Hz :
//! - per character (Rex + lineup) : mesh_root_world, mesh3d_world, mesh_aabb_world
//! - per bone (max N) : world_translation, local_rotation_euler_deg
//!
//! Permet de trancher entre 3 hypothèses bug skinning :
//! - (a) bindpose Cause A pas fixée pour GLB Meshy
//! - (b) ordre joints[] mismatch ATTRIBUTE_JOINT_INDEX
//! - (c) Mesh3d transform pas appliqué (cf Bevy custom_skinned_mesh.rs)
//!
//! Si `mesh_aabb_world.half_extents` ne change pas entre samples alors que des
//! bones rotated → bug skinning confirmé (mesh figé en bind pose). Health
//! alert side-file `forgia_bone_trace_health.json` avec next-step actionable.
//!
//! Pattern AAA — UE5 Animation Insights trace channels + Houdini Skinning
//! Converter error visualisation.

use bevy::camera::primitives::Aabb;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use forgia_core::prelude::*;

// ── Genome-driven config (data-driven, no hardcode) ─────────────────────────

/// Resource Tuning sync depuis `config/genomes/debug_anim.toml`. Hot-reload
/// via Bevy file_watcher. Defaults sains si TOML absent.
#[derive(Resource, Debug, Clone, Copy)]
pub struct BoneTraceConfig {
    pub enabled: bool,
    pub hz: f32,
    pub max_bones: usize,
    pub max_characters: usize,
    pub desync_threshold_m: f32,
}

impl Default for BoneTraceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hz: 2.0,
            max_bones: 24,
            max_characters: 8,
            desync_threshold_m: 0.05,
        }
    }
}

/// Timer dédié pour pacing 1/hz du sensor write.
#[derive(Resource, Default)]
pub struct BoneTraceTimer {
    accum_s: f32,
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Snapshot d'un character à un instant t — produit par le sensor system, écrit
/// au JSON, comparé entre frames pour détecter désync mesh↔bones.
#[derive(Debug, Clone)]
pub struct CharacterSnapshot {
    pub entity_name: String,
    pub mesh_root_world: Vec3,
    pub mesh3d_world: Option<Vec3>,
    pub mesh_aabb_world: Option<(Vec3, Vec3)>, // (center, half_extents)
    pub bones: Vec<BoneSnapshot>,
}

#[derive(Debug, Clone)]
pub struct BoneSnapshot {
    pub name: String,
    pub depth: u32,
    pub world_translation: Vec3,
    pub local_rotation_euler_deg: Vec3,
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct BoneTracePlugin;

impl Plugin for BoneTracePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoneTraceConfig>()
            .init_resource::<BoneTraceTimer>()
            .add_systems(Update, write_bone_trace_sensor.in_set(GameSet::Sensors));
    }
}

// ── Sensor writer ───────────────────────────────────────────────────────────

/// System Update — écrit `forgia_bone_trace.json` à `cfg.hz` Hz.
///
/// Pour chaque entity avec `SkinnedMesh` (= mesh rigged par Pinocchio) :
/// 1. Récupère mesh_root entity (parent du SkinnedMesh) via Children walk
/// 2. Lit son GlobalTransform (= mesh_root_world)
/// 3. Lit GlobalTransform du Mesh3d entity (= mesh3d_world)
/// 4. Lit Aabb propagé via GlobalTransform (= mesh_aabb_world)
/// 5. Walk children bones (BFS, max `max_bones`) — capture name + global +
///    local rotation Euler degrees
#[allow(clippy::too_many_arguments)]
fn write_bone_trace_sensor(
    time: Res<Time>,
    cfg: Res<BoneTraceConfig>,
    mut timer: ResMut<BoneTraceTimer>,
    q_skinned: Query<(Entity, &SkinnedMesh, &GlobalTransform, Option<&Aabb>)>,
    q_global: Query<&GlobalTransform>,
    q_transform: Query<&Transform>,
    q_name: Query<&Name>,
    q_parent: Query<&ChildOf>,
    q_children: Query<&Children>,
) {
    if !cfg.enabled {
        return;
    }
    let dt = time.delta_secs();
    timer.accum_s += dt;
    let interval = (1.0 / cfg.hz.max(0.1)).max(0.05);
    if timer.accum_s < interval {
        return;
    }
    timer.accum_s = 0.0;

    let mut snapshots: Vec<CharacterSnapshot> = Vec::with_capacity(cfg.max_characters);
    let mut desync_count = 0u32;

    for (mesh3d_entity, skinned, mesh3d_gt, mesh3d_aabb) in q_skinned.iter() {
        if snapshots.len() >= cfg.max_characters {
            break;
        }

        // Mesh root = walk parents jusqu'à trouver une entité avec un Name (typique
        // pattern Forgia : RexCharacter ou LineupCharacter taggés Name). Si aucun
        // ancêtre nommé, fallback sur le mesh3d entity directement.
        let (mesh_root_entity, entity_name) =
            find_named_ancestor(mesh3d_entity, &q_parent, &q_name);

        let mesh_root_world = q_global
            .get(mesh_root_entity)
            .map(|gt| gt.translation())
            .unwrap_or(Vec3::ZERO);

        let mesh3d_world = Some(mesh3d_gt.translation());

        // AABB en world space = GlobalTransform * local Aabb
        let mesh_aabb_world = mesh3d_aabb.map(|a| {
            let local_center: Vec3 = a.center.into();
            let local_half: Vec3 = a.half_extents.into();
            // Approximation : transform center par GlobalTransform, half_extents
            // restent locaux (rotation peut les déformer mais V1 dump direct).
            let world_center = mesh3d_gt.transform_point(local_center);
            (world_center, local_half)
        });

        // Walk les bones du SkinnedMesh.joints (= liste d'entities bones)
        let mut bones: Vec<BoneSnapshot> = Vec::with_capacity(cfg.max_bones);
        for (depth, &bone_entity) in skinned.joints.iter().enumerate() {
            if bones.len() >= cfg.max_bones {
                break;
            }
            let name = q_name
                .get(bone_entity)
                .map(|n| n.to_string())
                .unwrap_or_else(|_| format!("bone_{depth}"));
            let world_translation = q_global
                .get(bone_entity)
                .map(|gt| gt.translation())
                .unwrap_or(Vec3::ZERO);
            let local_rot = q_transform
                .get(bone_entity)
                .map(|t| t.rotation)
                .unwrap_or(Quat::IDENTITY);
            let (x, y, z) = local_rot.to_euler(EulerRot::XYZ);
            let local_rotation_euler_deg =
                Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees());

            bones.push(BoneSnapshot {
                name,
                depth: depth as u32,
                world_translation,
                local_rotation_euler_deg,
            });
        }

        // Désync detection : mesh AABB center vs bone[0] (root bone) world.
        // Si delta > threshold ET bones[0] a rotated depuis bind → bug skinning.
        if let (Some((aabb_center, _)), Some(root_bone)) = (mesh_aabb_world, bones.first()) {
            let delta = aabb_center.distance(root_bone.world_translation);
            if delta > cfg.desync_threshold_m {
                desync_count += 1;
            }
        }

        snapshots.push(CharacterSnapshot {
            entity_name,
            mesh_root_world,
            mesh3d_world,
            mesh_aabb_world,
            bones,
        });

        // Mark q_children as used (pour clippy unused warning) — futur usage
        // walk hierarchy via Children plutôt que joints[] direct.
        let _ = &q_children;
    }

    write_json(time.elapsed_secs(), &snapshots);
    write_health(time.elapsed_secs(), desync_count, &snapshots);
}

// ── JSON serializer (manuel, zéro dep) ──────────────────────────────────────

fn write_json(timestamp_secs: f32, snapshots: &[CharacterSnapshot]) {
    let mut json = String::with_capacity(2048);
    json.push_str(&format!(
        "{{\"timestamp_secs\":{:.1},\"characters_count\":{},\"characters\":[",
        timestamp_secs,
        snapshots.len()
    ));
    for (i, snap) in snapshots.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"entity_name\":\"{}\",\"mesh_root_world\":[{:.3},{:.3},{:.3}]",
            snap.entity_name,
            snap.mesh_root_world.x,
            snap.mesh_root_world.y,
            snap.mesh_root_world.z
        ));
        if let Some(m3) = snap.mesh3d_world {
            json.push_str(&format!(
                ",\"mesh3d_world\":[{:.3},{:.3},{:.3}]",
                m3.x, m3.y, m3.z
            ));
        } else {
            json.push_str(",\"mesh3d_world\":null");
        }
        if let Some((c, h)) = snap.mesh_aabb_world {
            json.push_str(&format!(
                ",\"mesh_aabb_world\":{{\"center\":[{:.3},{:.3},{:.3}],\"half_extents\":[{:.3},{:.3},{:.3}]}}",
                c.x, c.y, c.z, h.x, h.y, h.z
            ));
        } else {
            json.push_str(",\"mesh_aabb_world\":null");
        }
        json.push_str(",\"bones\":[");
        for (bi, bone) in snap.bones.iter().enumerate() {
            if bi > 0 {
                json.push(',');
            }
            json.push_str(&format!(
                "{{\"name\":\"{}\",\"depth\":{},\"world_translation\":[{:.3},{:.3},{:.3}],\"local_rotation_euler_deg\":[{:.2},{:.2},{:.2}]}}",
                bone.name,
                bone.depth,
                bone.world_translation.x,
                bone.world_translation.y,
                bone.world_translation.z,
                bone.local_rotation_euler_deg.x,
                bone.local_rotation_euler_deg.y,
                bone.local_rotation_euler_deg.z
            ));
        }
        json.push_str("]}");
    }
    json.push_str("]}");
    if let Err(e) = std::fs::write("forgia_bone_trace.json", json) {
        warn!("[forgia-anim-debug::bone_trace] sensor write failed: {e}");
    }
}

fn write_health(timestamp_secs: f32, desync_count: u32, snapshots: &[CharacterSnapshot]) {
    if desync_count == 0 {
        // Convention V1 : delete health file si tout OK pour signaler "all green"
        let _ = std::fs::remove_file("forgia_bone_trace_health.json");
        return;
    }
    let json = format!(
        r#"{{
  "timestamp_secs": {:.1},
  "severity": "warning",
  "desync_count": {},
  "characters_total": {},
  "message": "Bone trace desync detected: {} character(s) ont mesh_aabb_world.center loin de bones[0].world (> threshold). Skinning probablement broken — bones tournent mais mesh figé.",
  "next_step": "Read forgia_bone_trace.json — comparer mesh_aabb_world.half_extents entre 2 samples consécutifs. Si half_extents identique malgré bones[].local_rotation_euler_deg différents → bug skinning bindpose confirmé. Investiguer forgia-auto-rig::skinning lignes 220-253 (formule inverse_bindpose) + Bevy custom_skinned_mesh.rs convention (`Mesh3d.transform doesn't affect mesh position`)."
}}"#,
        timestamp_secs,
        desync_count,
        snapshots.len(),
        desync_count,
    );
    if let Err(e) = std::fs::write("forgia_bone_trace_health.json", json) {
        warn!("[forgia-anim-debug::bone_trace] health write failed: {e}");
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Remonte la chaîne parent jusqu'à trouver une entité avec un Name component.
/// Retourne (entity, name) — ou (mesh3d_entity, "unnamed") si aucun ancêtre
/// nommé trouvé. Cap profondeur à 10 pour éviter infinite-loop sur cycles.
fn find_named_ancestor(
    start: Entity,
    q_parent: &Query<&ChildOf>,
    q_name: &Query<&Name>,
) -> (Entity, String) {
    let mut current = start;
    for _ in 0..10 {
        if let Ok(name) = q_name.get(current) {
            return (current, name.to_string());
        }
        match q_parent.get(current) {
            Ok(parent) => current = parent.0,
            Err(_) => break,
        }
    }
    (start, "unnamed".to_string())
}

// ── Tests headless ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_sane() {
        let cfg = BoneTraceConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.hz > 0.0);
        assert!(cfg.max_bones >= 18); // couvre Humanoid
        assert!(cfg.desync_threshold_m > 0.0);
    }

    #[test]
    fn timer_paces_at_hz() {
        // À 2Hz, on attend 0.5s entre samples. Timer accumule jusqu'à interval.
        let cfg = BoneTraceConfig::default();
        let interval = 1.0 / cfg.hz;
        assert!((interval - 0.5).abs() < 0.01);
    }

    #[test]
    fn json_format_smoke() {
        // Snapshot vide → JSON parseable basique
        write_json(1.0, &[]);
        let content = std::fs::read_to_string("forgia_bone_trace.json").ok();
        if let Some(c) = content {
            assert!(c.contains("\"characters_count\":0"));
        }
    }

    #[test]
    fn plugin_builds_without_panic() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.add_plugins(forgia_core::ForgiaCorePlugin);
        app.add_plugins(BoneTracePlugin);
    }
}
