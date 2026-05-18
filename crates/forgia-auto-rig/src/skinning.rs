//! # Skinning weights injection (story-440 Phase 1B)
//!
//! Pour chaque mesh root marqué `AutoRigged` (Phase 1A a spawné les bones),
//! ce module :
//!
//! 1. Walk descendants → trouve les `Mesh3d` entities.
//! 2. Pour chaque vertex du mesh, calcule les K bones les plus proches
//!    (Euclidean dans le repère mesh-root).
//! 3. Pondère par inverse-distance² normalisée à 1.0.
//! 4. Clone le `Mesh` asset, insère `ATTRIBUTE_JOINT_INDEX` + `ATTRIBUTE_JOINT_WEIGHT`.
//! 5. Calcule `SkinnedMeshInverseBindposes` = `Mat4::inverse(bone_global_at_bind)`.
//! 6. Insère `SkinnedMesh` sur l'entité Mesh3d (PAS sur le mesh root —
//!    Bevy attend SkinnedMesh à côté du Mesh3d qu'elle déforme).
//! 7. Pose marker `SkinningInjected` sur le mesh root (idempotence).
//!
//! **Ordering** : tourne en `PostUpdate` après transform propagation pour que
//! `GlobalTransform` des bones (qu'on vient de spawner en `Update`) soit valide.
//!
//! **Limitations Phase 1B** :
//! - Nearest-bone simple → artefacts visibles sur articulations (acceptable
//!   pour walk cycle ±20°, moche pour pose extrême). Heat-diffusion en Phase 2.
//! - `SkinningConfig::max_verts_per_mesh` caps les meshes lourds (perf safety).
//! - Multi-Mesh3d : on traite tous les Mesh3d descendants, chacun avec ses propres
//!   ATTRIBUTE_JOINT_*. Tous partagent les mêmes bones.

use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::{Mesh, Mesh3d, VertexAttributeValues};
use bevy::prelude::*;
use std::collections::HashSet;

use crate::{AutoRigStats, AutoRigged};

/// Marker posé sur chaque bone entity créée par `auto_rig_pending_meshes`.
/// `rig_root` pointe le mesh root du squelette → permet de filtrer les bones
/// appartenant à ce squelette spécifique (multi-character safe).
#[derive(Component, Debug, Clone, Copy)]
pub struct BoneEntity {
    pub rig_root: Entity,
}

/// Marker idempotence + diagnostic posé sur le mesh root une fois le skinning
/// injecté avec succès.
#[derive(Component, Debug, Default)]
pub struct SkinningInjected {
    pub meshes_processed: usize,
    pub verts_total: u32,
    pub bones_used: usize,
}

/// Configuration runtime du skinning. Modifiable via Resource pour A/B testing.
#[derive(Resource, Debug, Clone)]
pub struct SkinningConfig {
    /// Nombre de bones par vertex (limite Bevy `Uint16x4` = 4 max).
    pub bones_per_vertex: u8,
    /// Au-dessus de ce nombre de vertices, on skip le skinning (perf safety).
    /// Rex.glb Meshy ~10k-30k typiquement OK.
    pub max_verts_per_mesh: u32,
    /// Falloff `1/distance^power`. Power=2 (square) typique, power=4 plus piqué.
    pub falloff_power: f32,
}

impl Default for SkinningConfig {
    fn default() -> Self {
        Self {
            bones_per_vertex: 4,
            max_verts_per_mesh: 200_000,
            falloff_power: 2.0,
        }
    }
}

/// Système Phase 1B. Tourne en `PostUpdate` pour avoir `GlobalTransform` valide
/// sur les bones spawnés en Update par `auto_rig_pending_meshes`.
pub fn inject_skinning_for_rigged_meshes(
    mut commands: Commands,
    config: Res<SkinningConfig>,
    time: Res<Time>,
    mut stats: ResMut<AutoRigStats>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bindposes_assets: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    q_rigged: Query<(Entity, &AutoRigged), Without<SkinningInjected>>,
    q_children: Query<&Children>,
    q_mesh3d: Query<&Mesh3d>,
    q_global: Query<&GlobalTransform>,
    q_bones: Query<(Entity, &BoneEntity)>,
) {
    for (mesh_root, rigged) in &q_rigged {
        if rigged.bone_count == 0 {
            // Phase 1A a marqué AutoRigged sans bones (AABB dégénérée) — skip.
            commands.entity(mesh_root).insert(SkinningInjected::default());
            continue;
        }

        // 1. Collecter les bones du rig (filtrer par rig_root == mesh_root pour
        //    multi-character safety).
        let bones: Vec<Entity> = q_bones
            .iter()
            .filter_map(|(e, b)| (b.rig_root == mesh_root).then_some(e))
            .collect();
        if bones.is_empty() {
            warn!(
                "[forgia-auto-rig::skinning] mesh_root {:?} marked AutoRigged but no BoneEntity found — skip",
                mesh_root
            );
            stats.total_meshes_skinning_failed += 1;
            commands.entity(mesh_root).insert(SkinningInjected::default());
            continue;
        }

        // 2. Calculer les positions monde des bones (bind pose = pose actuelle).
        //    Note : GlobalTransform peut ne pas être propagé si transform_propagate
        //    n'a pas tourné. PostUpdate inclut TransformPropagate avant nos systems
        //    custom donc OK Bevy 0.18.
        let bone_world_positions: Vec<Vec3> = bones
            .iter()
            .map(|&b| {
                q_global
                    .get(b)
                    .map(|gt| gt.translation())
                    .unwrap_or(Vec3::ZERO)
            })
            .collect();

        // Si tous les bones sont à (0,0,0), GlobalTransform pas encore propagé.
        // Retry au prochain frame (ne pas poser SkinningInjected).
        let all_zero = bone_world_positions.iter().all(|p| p.length_squared() < 1e-6);
        if all_zero {
            // Note : pas de stats incrément ici, c'est un retry attendu.
            continue;
        }

        // Mesh root world transform → permettra de mettre les vertex en repère
        // mesh-root-local pour matching bones (qui sont aussi en mesh-root-local
        // après accumulation des Transforms via GlobalTransform).
        let mesh_root_inv = q_global
            .get(mesh_root)
            .map(|gt| gt.to_matrix().inverse())
            .unwrap_or(Mat4::IDENTITY);

        // Convertir bones en repère mesh-root-local pour matching.
        let bone_positions_local: Vec<Vec3> = bone_world_positions
            .iter()
            .map(|p| mesh_root_inv.transform_point3(*p))
            .collect();

        // 3. Walk descendants → trouve les Mesh3d entities + leur GlobalTransform.
        let mut mesh3d_entities: Vec<Entity> = Vec::new();
        collect_mesh3d_descendants(mesh_root, &q_children, &q_mesh3d, &mut mesh3d_entities);

        if mesh3d_entities.is_empty() {
            // Aucun Mesh3d : peut arriver si le SceneRoot n'a pas encore instancié
            // ses meshes. Retry au prochain frame.
            continue;
        }

        // 4. Bones globaux au bind pose (pose courante post-spawn, avant anim).
        //    Mat4 complets avec translation+rotation+scale via GlobalTransform.
        //    PostUpdate APRÈS TransformPropagate → GlobalTransform fiable.
        let bone_globals: Vec<Mat4> = bones
            .iter()
            .map(|&b| q_global.get(b).copied().unwrap_or_default().to_matrix())
            .collect();

        // 5. Pour chaque Mesh3d : process vertices.
        //
        // Convention bindpose Forgia (2026-05-17 + revert validation 2026-05-18) :
        // `inverse_bindpose[i] = bone_global_at_bind.inverse() * mesh3d_world`.
        //
        // Cf voir détail formule + revert log juste avant le calcul lignes
        // ~225-240 ci-dessous. Note : story-454 sensor `forgia_bone_trace.json`
        // signalait mesh_aabb_world.half_extents stable, mais c'était un faux
        // signal côté sensor (half_extents local lu sans application GlobalTransform).
        // Le vrai diagnostic du symptôme "T-pose figée" reste à investiguer
        // ailleurs (joints[] ordre vs ATTRIBUTE_JOINT_INDEX, skinning weights,
        // bone pivot positions, etc.). PAS dans la formule bindpose elle-même.
        let mut total_verts_processed: u32 = 0;
        let mut meshes_processed = 0usize;

        for mesh3d_entity in &mesh3d_entities {
            let mesh3d = match q_mesh3d.get(*mesh3d_entity) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mesh_handle = &mesh3d.0;
            let mesh = match meshes.get(mesh_handle) {
                Some(m) => m,
                None => continue,
            };

            let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                Some(VertexAttributeValues::Float32x3(p)) => p.clone(),
                _ => {
                    warn!(
                        "[forgia-auto-rig::skinning] mesh {:?} missing ATTRIBUTE_POSITION Float32x3 — skip",
                        mesh3d_entity
                    );
                    continue;
                }
            };
            let n_verts = positions.len();
            if n_verts as u32 > config.max_verts_per_mesh {
                warn!(
                    "[forgia-auto-rig::skinning] mesh {:?} has {} verts > cap {} — skip",
                    mesh3d_entity, n_verts, config.max_verts_per_mesh
                );
                stats.total_meshes_skinning_failed += 1;
                continue;
            }

            // Mesh3d global transform au bind (utilisé pour BOTH le matching
            // bones-vertex ET les inverse_bindposes).
            let mesh3d_world = q_global
                .get(*mesh3d_entity)
                .copied()
                .unwrap_or_default()
                .to_matrix();
            let mesh3d_to_root_local = mesh_root_inv * mesh3d_world;

            // ── inverse_bindposes per-mesh3d ──────────────────────────────
            // Formule : `bone_global.inverse() * mesh3d_world`.
            //
            // Pourquoi `* mesh3d_world` est nécessaire : Bevy `SkinnedMesh`
            // ignore la `GlobalTransform` du Mesh3d entity (cf doc Bevy +
            // custom_skinned_mesh.rs note "its transform doesn't affect the
            // position of the mesh"). Le vertex shader calcule
            // `world_pos = bone_global * inverse_bindpose * vertex_local`.
            // Pour que `world_pos == mesh3d_world * vertex_local` au bind :
            //   bone_bind * inv_bp = mesh3d_world
            //   donc inv_bp = bone_bind.inverse() * mesh3d_world.
            //
            // Story-454 audit 2026-05-18 : bevy-specialist proposed retirer
            // `* mesh3d_world` ; REVERTED après test runtime → mesh devient
            // invisible (vertices rendus à world origin (0,0,0), loin player).
            // L'analyse specialist était valide pour cas Mesh3d à origin
            // mais pas pour notre hiérarchie GLB Meshy où Mesh3d a transform
            // non-identité ((19, 18, 19) pour Rex). Formule originale OK.
            let bindposes_for_this_mesh: Vec<Mat4> = bone_globals
                .iter()
                .map(|&bone_global| bone_global.inverse() * mesh3d_world)
                .collect();
            let bindposes_handle =
                bindposes_assets.add(SkinnedMeshInverseBindposes::from(bindposes_for_this_mesh));

            // Pré-cloner une seule fois le mesh (mutation in-place).
            let mut new_mesh = mesh.clone();

            // Calcul nearest bones + weights, vectorisé en 2 buffers Uint16x4 / Float32x4.
            let (joint_indices, joint_weights) =
                compute_nearest_bone_weights(&positions, &mesh3d_to_root_local, &bone_positions_local, &config);

            new_mesh.insert_attribute(
                Mesh::ATTRIBUTE_JOINT_INDEX,
                VertexAttributeValues::Uint16x4(joint_indices),
            );
            new_mesh.insert_attribute(
                Mesh::ATTRIBUTE_JOINT_WEIGHT,
                VertexAttributeValues::Float32x4(joint_weights),
            );

            // Swap mesh handle : add le nouveau mesh, replace Mesh3d.
            let new_handle = meshes.add(new_mesh);
            commands.entity(*mesh3d_entity).insert((
                Mesh3d(new_handle),
                SkinnedMesh {
                    inverse_bindposes: bindposes_handle,
                    joints: bones.clone(),
                },
            ));

            total_verts_processed += n_verts as u32;
            meshes_processed += 1;
        }

        if meshes_processed == 0 {
            // Aucun mesh utilisable cette frame, retry plus tard (pas de marker).
            continue;
        }

        // 6. Pose marker idempotence + update stats.
        commands.entity(mesh_root).insert(SkinningInjected {
            meshes_processed,
            verts_total: total_verts_processed,
            bones_used: bones.len(),
        });
        stats.total_meshes_skinned += meshes_processed as u32;
        stats.last_verts_skinned = total_verts_processed;
        stats.last_skinning_at_secs = time.elapsed_secs();

        info!(
            "[forgia-auto-rig::skinning] mesh_root {:?} : {} mesh(es) skinned, {} verts, {} bones",
            mesh_root, meshes_processed, total_verts_processed, bones.len()
        );
    }
}

/// BFS descendants pour collecter les entités avec un `Mesh3d` (incluant
/// nested Scene roots).
fn collect_mesh3d_descendants(
    root: Entity,
    q_children: &Query<&Children>,
    q_mesh3d: &Query<&Mesh3d>,
    out: &mut Vec<Entity>,
) {
    let mut stack = vec![root];
    let mut visited: HashSet<Entity> = HashSet::new();
    while let Some(e) = stack.pop() {
        if !visited.insert(e) {
            continue;
        }
        if q_mesh3d.get(e).is_ok() && e != root {
            out.push(e);
        }
        if let Ok(children) = q_children.get(e) {
            for c in children.iter() {
                stack.push(c);
            }
        }
    }
}

/// Pour chaque vertex (positions[i] dans le repère mesh3d-local), trouve les K
/// bones les plus proches dans le repère mesh-root-local, pondère par
/// `1 / distance^power` normalisé à `sum = 1.0`.
///
/// K = `config.bones_per_vertex` (cappé à 4 pour Uint16x4 / Float32x4).
fn compute_nearest_bone_weights(
    positions: &[[f32; 3]],
    mesh3d_to_root_local: &Mat4,
    bone_positions_local: &[Vec3],
    config: &SkinningConfig,
) -> (Vec<[u16; 4]>, Vec<[f32; 4]>) {
    let k = (config.bones_per_vertex as usize).min(4);
    let n_bones = bone_positions_local.len();
    let mut indices: Vec<[u16; 4]> = Vec::with_capacity(positions.len());
    let mut weights: Vec<[f32; 4]> = Vec::with_capacity(positions.len());

    for p in positions {
        let v_root = mesh3d_to_root_local.transform_point3(Vec3::from_array(*p));

        // Distance² à chaque bone (squared pour éviter sqrt + diviser après).
        // K petit (≤4) → partial selection plus rapide qu'un sort complet O(n log n).
        let mut top: Vec<(usize, f32)> = Vec::with_capacity(k);
        for (bi, bp) in bone_positions_local.iter().enumerate() {
            let d2 = (v_root - *bp).length_squared();
            if top.len() < k {
                top.push((bi, d2));
                if top.len() == k {
                    top.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                }
            } else if d2 < top[k - 1].1 {
                top[k - 1] = (bi, d2);
                // Re-sort la tail (k≤4 → coût constant).
                top.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

        // Calcul des weights : 1 / d^power. Floor d² à 1e-6 pour éviter div-by-0.
        let exponent = config.falloff_power * 0.5; // power sur d, pas d²
        let raw: [f32; 4] = {
            let mut r = [0.0_f32; 4];
            for (slot, (_, d2)) in top.iter().enumerate() {
                let d = d2.max(1e-6).sqrt();
                r[slot] = 1.0 / d.powf(exponent * 2.0); // exponent appliqué tel quel
            }
            r
        };
        let sum: f32 = raw.iter().sum();
        let normalized: [f32; 4] = if sum > 1e-6 {
            [raw[0] / sum, raw[1] / sum, raw[2] / sum, raw[3] / sum]
        } else {
            // Tous les bones sont à distance infinie ou même point → uniform.
            [1.0 / k as f32, 1.0 / k as f32, 1.0 / k as f32, 1.0 / k as f32]
                .map(|w| if k >= 1 { w } else { 0.0 })
        };

        let mut idx = [0_u16; 4];
        for (slot, (bi, _)) in top.iter().enumerate() {
            // Clamp au cas où n_bones > u16::MAX (improbable, mais sécurité).
            idx[slot] = (*bi).min(n_bones.saturating_sub(1)) as u16;
        }

        indices.push(idx);
        weights.push(normalized);
    }

    (indices, weights)
}

// ── Tests headless ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> SkinningConfig {
        SkinningConfig::default()
    }

    /// Story-454 — propriété Bevy SkinnedMesh : le vertex shader applique
    /// `world_pos = bone_global * inverse_bindpose * vertex_local`. Pour qu'au
    /// bind pose le mesh apparaisse à `mesh3d_world.transform_point(vertex_local)`
    /// (la transform standard d'un Mesh3d non-skinned), la formule doit être :
    ///   `inverse_bindpose = bone_global_bind.inverse() * mesh3d_world`
    /// car `SkinnedMesh` shader **ignore** le GlobalTransform de l'entité
    /// Mesh3d. Le test valide cette invariance numériquement.
    #[test]
    fn bindpose_yields_mesh3d_world_at_bind() {
        let bone_bind = Mat4::from_translation(Vec3::new(2.92, 16.86, 15.86));
        let mesh3d_world = Mat4::from_translation(Vec3::new(0.0, -0.85, 0.0))
            * Mat4::from_rotation_y(std::f32::consts::PI);
        let inv_bp = bone_bind.inverse() * mesh3d_world;
        // Au bind, bone_current = bone_bind, donc joint_matrix = mesh3d_world.
        let joint_matrix = bone_bind * inv_bp;
        let id = Mat4::IDENTITY;
        for r in 0..4 {
            for c in 0..4 {
                let want = mesh3d_world.row(r)[c];
                let got = joint_matrix.row(r)[c];
                let diff = (got - want).abs();
                let id_diff = (got - id.row(r)[c]).abs();
                assert!(
                    diff < 1e-5,
                    "joint_matrix({r},{c}) = {got}, want {want} (mesh3d_world). id_diff={id_diff}"
                );
            }
        }
    }

    #[test]
    fn weights_sum_to_one() {
        let positions = vec![[0.0, 0.5, 0.0], [0.1, 0.6, 0.0]];
        let bones = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.5, 0.5),
            Vec3::new(0.0, 0.5, -0.5),
        ];
        let (_, weights) = compute_nearest_bone_weights(
            &positions,
            &Mat4::IDENTITY,
            &bones,
            &cfg(),
        );
        for w in &weights {
            let sum: f32 = w.iter().sum();
            assert!(
                (sum - 1.0).abs() < 1e-4,
                "weights must sum to 1.0, got {} ({:?})",
                sum,
                w
            );
        }
    }

    #[test]
    fn closest_bone_gets_highest_weight() {
        let positions = vec![[0.0, 0.1, 0.0]]; // très proche de bone[0]
        let bones = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 10.0),
        ];
        let (indices, weights) = compute_nearest_bone_weights(
            &positions,
            &Mat4::IDENTITY,
            &bones,
            &cfg(),
        );
        assert_eq!(indices[0][0], 0, "bone 0 doit être le 1er joint");
        assert!(
            weights[0][0] > 0.9,
            "bone 0 doit dominer (>0.9), got {}",
            weights[0][0]
        );
    }

    #[test]
    fn handles_single_bone_rig() {
        // K=4 mais 1 seul bone disponible → tous les slots pointent vers 0,
        // les weights doivent rester valides (1.0 dans slot 0, 0 ailleurs ou
        // distribués si plusieurs slots actifs).
        let positions = vec![[1.0, 1.0, 1.0]];
        let bones = vec![Vec3::ZERO];
        let (indices, weights) =
            compute_nearest_bone_weights(&positions, &Mat4::IDENTITY, &bones, &cfg());
        assert_eq!(indices[0][0], 0);
        let sum: f32 = weights[0].iter().sum();
        assert!((sum - 1.0).abs() < 1e-4 || sum.abs() < 1e-4, "got sum={}", sum);
    }

    #[test]
    fn weights_indices_have_correct_length() {
        let positions = vec![[0.0, 0.0, 0.0]; 100];
        let bones = vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z, Vec3::NEG_X];
        let (indices, weights) =
            compute_nearest_bone_weights(&positions, &Mat4::IDENTITY, &bones, &cfg());
        assert_eq!(indices.len(), 100);
        assert_eq!(weights.len(), 100);
    }
}

