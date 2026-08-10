//! Spawn — turn a [`PointCloud`] into instanced, grounded, collidable entities.
//!
//! Each point becomes a parent entity (grounded transform) with the kit mesh primitives as
//! visual children and a collider built per [`ColliderKind`]. Spawning is **budgeted**
//! (N per frame) so a large cloud never freezes a frame (perf pillar #3, story-578 §3).
//!
//! Grounding uses the P0 pivot: `placement_y(ground_y, scale)` puts the module's bottom
//! exactly on the ground — no sinking, no floating, whatever the pivot.

use crate::points::Point;
use crate::registry::{AssetMeta, AssetRegistry, ColliderKind};
use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, ComputedColliderShape, RigidBody};
use std::collections::VecDeque;

/// How many modules to spawn per frame (anti-freeze budget).
const SPAWN_BUDGET_PER_FRAME: usize = 8;

/// Marks a spawned worldgen module. Carries its local AABB so debug viz can draw bounds
/// without a registry lookup.
#[derive(Component)]
pub struct WorldgenModule {
    pub module_id: String,
    /// Local-space AABB center (mesh coordinates, pre-scale).
    pub local_center: Vec3,
    /// Local-space AABB half-extents.
    pub local_half: Vec3,
}

/// Pending placement points, drained over several frames.
#[derive(Resource, Default)]
pub struct SpawnQueue {
    pub pending: VecDeque<Point>,
}

/// Running counters for the sensor.
#[derive(Resource, Default)]
pub struct WorldgenStats {
    /// Modules spawned since startup.
    pub spawned: u32,
    /// Size of the last generated row.
    pub last_row: u32,
}

/// The loaded kit GLB + a fallback material for primitives without one + the road material.
#[derive(Resource)]
pub struct WorldgenKit {
    pub gltf: Handle<Gltf>,
    pub fallback_mat: Handle<StandardMaterial>,
    pub road_mat: Handle<StandardMaterial>,
    /// White, double-sided material for procedural houses (per-vertex colors show through).
    pub house_mat: Handle<StandardMaterial>,
}

/// Drain the spawn queue, budgeted. No-op until the registry and kit GLB are loaded.
#[allow(clippy::too_many_arguments)]
pub fn sys_spawn_drain(
    mut commands: Commands,
    mut queue: ResMut<SpawnQueue>,
    registry: Option<Res<AssetRegistry>>,
    kit: Option<Res<WorldgenKit>>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
    mut stats: ResMut<WorldgenStats>,
) {
    if queue.pending.is_empty() {
        return;
    }
    let (Some(registry), Some(kit)) = (registry, kit) else {
        return;
    };
    // Wait until the kit GLB has finished loading (named_meshes populated).
    let Some(gltf) = gltfs.get(&kit.gltf) else {
        return;
    };

    let mut done = 0usize;
    while done < SPAWN_BUDGET_PER_FRAME {
        let Some(point) = queue.pending.pop_front() else {
            break;
        };
        done += 1;

        let Some(meta) = registry.get(&point.module_id) else {
            warn!("[worldgen] unknown module id '{}'", point.module_id);
            continue;
        };
        let Some(gm_handle) = gltf.named_meshes.get(point.module_id.as_str()) else {
            warn!("[worldgen] module '{}' not in kit GLB", point.module_id);
            continue;
        };
        let Some(gltf_mesh) = gltf_meshes.get(gm_handle) else {
            // Sub-asset not ready yet — requeue and stop this frame.
            queue.pending.push_front(point);
            break;
        };

        spawn_module(&mut commands, &kit, &meshes, meta, gltf_mesh, &point);
        stats.spawned += 1;
    }
}

/// Spawn one module: a grounded parent entity (the P0 pivot) with the kit mesh primitives as
/// visual children and a collider per [`ColliderKind`]. Returns the parent entity. Shared by the
/// spawn-queue drain and the P4 streamer (callers resolve `meta` + `gltf_mesh` first).
pub(crate) fn spawn_module(
    commands: &mut Commands,
    kit: &WorldgenKit,
    meshes: &Assets<Mesh>,
    meta: &AssetMeta,
    gltf_mesh: &GltfMesh,
    point: &Point,
) -> Entity {
    // Grounded transform: bottom rests on ground_pos.y (the P0 pivot fix).
    let origin_y = meta.placement_y(point.ground_pos.y, point.scale);
    let tf = Transform {
        translation: Vec3::new(point.ground_pos.x, origin_y, point.ground_pos.z),
        rotation: Quat::from_rotation_y(point.yaw),
        scale: Vec3::splat(point.scale),
    };
    let center = Vec3::new(
        (meta.aabb_min.0 + meta.aabb_max.0) * 0.5,
        (meta.aabb_min.1 + meta.aabb_max.1) * 0.5,
        (meta.aabb_min.2 + meta.aabb_max.2) * 0.5,
    );
    let half = Vec3::new(
        (meta.aabb_max.0 - meta.aabb_min.0) * 0.5,
        (meta.aabb_max.1 - meta.aabb_min.1) * 0.5,
        (meta.aabb_max.2 - meta.aabb_min.2) * 0.5,
    );
    let collider_kind = meta.collider;
    let primitives = gltf_mesh.primitives.clone();
    let fallback_mat = kit.fallback_mat.clone();

    let mut parent = commands.spawn((
        WorldgenModule {
            module_id: point.module_id.clone(),
            local_center: center,
            local_half: half,
        },
        Name::new(format!("worldgen:{}", point.module_id)),
        tf,
        Visibility::default(),
    ));

    parent.with_children(|p| {
        // Visual: one child per glTF primitive (same local space → identity transform).
        for prim in &primitives {
            let material = prim
                .material
                .clone()
                .unwrap_or_else(|| fallback_mat.clone());
            p.spawn((
                Mesh3d(prim.mesh.clone()),
                MeshMaterial3d(material),
                Transform::default(),
            ));
        }
        // Collider per kind. Cuboid/Cylinder come straight from the AABB (cheap, no mesh
        // needed); ConvexHull/TriMesh are built from the primitive meshes.
        match collider_kind {
            ColliderKind::NoCollider => {}
            ColliderKind::Cuboid => {
                p.spawn((
                    Transform::from_translation(center),
                    Collider::cuboid(half.x, half.y, half.z),
                ));
            }
            ColliderKind::Cylinder => {
                p.spawn((
                    Transform::from_translation(center),
                    Collider::cylinder(half.y, half.x.max(half.z)),
                ));
            }
            ColliderKind::ConvexHull | ColliderKind::TriMesh => {
                let shape = if matches!(collider_kind, ColliderKind::TriMesh) {
                    ComputedColliderShape::TriMesh(default())
                } else {
                    ComputedColliderShape::ConvexHull
                };
                for prim in &primitives {
                    if let Some(mesh) = meshes.get(&prim.mesh) {
                        if let Some(col) = Collider::from_bevy_mesh(mesh, &shape) {
                            p.spawn((Transform::default(), col));
                        }
                    }
                }
            }
        }
    });

    if !matches!(collider_kind, ColliderKind::NoCollider) {
        parent.insert(RigidBody::Fixed);
    }

    parent.id()
}
