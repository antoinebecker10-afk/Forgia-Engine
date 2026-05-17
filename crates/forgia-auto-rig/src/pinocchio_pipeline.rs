//! # pinocchio_pipeline — Backend auto-rig basé sur le pipeline Pinocchio
//!
//! Wire les 3 crates Phase 1 (`forgia-mesh-voxelizer`, `forgia-medial-axis`,
//! `forgia-skeleton-embedder`) dans un système Bevy qui spawn les bones.
//!
//! **vs le pipeline TemplateFit historique** :
//! - TemplateFit : AABB + landmarks heuristiques → bones placés selon
//!   proportions Vitruvian scalées. Échec sur morphologies exotiques + GLB
//!   avec accessoires invisibles.
//! - PinocchioV1 : voxelize mesh → distance field → medial axis → snap bones
//!   sur les sphères médial. Bones GARANTIS à l'intérieur du mesh, peu importe
//!   la morphologie.
//!
//! ## Trade-offs V1
//!
//! - Coût ~100-300ms one-shot par mesh (~30k vertices, res 32). Acceptable car
//!   1-shot au spawn.
//! - Multi-Mesh3d : V1 ne traite que le PREMIER mesh3d trouvé (le plus gros
//!   en pratique). V2 fusionnera tous les meshes pour AABB unifiée.
//! - Embedding greedy nearest-sphere (vs Mixed Integer Program Pinocchio
//!   original) : suffisant pour humanoid + lézard ; échec possible sur
//!   morphologies complexes (octopus, dragon multi-ailes).

use bevy::camera::primitives::Aabb;
use bevy::mesh::{Indices, Mesh, Mesh3d, VertexAttributeValues};
use bevy::prelude::*;
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_medial_axis::{extract_medial_axis, MedialAxisConfig};
use forgia_mesh_voxelizer::{voxelize_mesh, VoxelizerConfig};
use forgia_skeleton_embedder::{
    embed_template_skeleton, EmbeddedSkeleton, SkeletonTemplate,
};

use crate::{
    anatomy_detect, AutoRigStats, AutoRigTemplate, AutoRigged, BoneEntity, NeedsAutoRig,
};

// ── Genome handles (TOML-driven templates — no more hardcode) ────────────

/// Handle vers `assets/genomes/skeleton_humanoid.toml`. Loaded au Startup.
#[derive(Resource)]
pub struct SkeletonHumanoidGenomeHandle(pub Handle<Genome<SkeletonTemplate>>);

/// Handle vers `assets/genomes/skeleton_biped_lizard.toml`. Loaded au Startup.
#[derive(Resource)]
pub struct SkeletonBipedLizardGenomeHandle(pub Handle<Genome<SkeletonTemplate>>);

/// Système Startup : load les 2 templates en TOML genomes.
pub fn load_skeleton_genomes(mut commands: Commands, asset_server: Res<AssetServer>) {
    let humanoid: Handle<Genome<SkeletonTemplate>> =
        asset_server.load("genomes/skeleton_humanoid.toml");
    let lizard: Handle<Genome<SkeletonTemplate>> =
        asset_server.load("genomes/skeleton_biped_lizard.toml");
    commands.insert_resource(SkeletonHumanoidGenomeHandle(humanoid));
    commands.insert_resource(SkeletonBipedLizardGenomeHandle(lizard));
    info!("[forgia-auto-rig::pinocchio] skeleton templates loading from TOML genomes");
}

/// Type alias pour le loader Bevy (utilisé par `ForgiaAutoRigPlugin::build`).
#[allow(dead_code)]
pub type SkeletonGenomeLoader = GenomeLoader<SkeletonTemplate>;

/// Backend du pipeline auto-rig. Switcher via `world.resource_mut::<AutoRigBackend>()`.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AutoRigBackend {
    /// Pipeline V1 Phase 1 (Pinocchio-inspired) : voxelize → medial axis →
    /// embed template via greedy nearest-sphere snapping. Default story-440 R&D.
    #[default]
    PinocchioV1,
    /// Pipeline historique (template-fit AABB + landmarks anatomiques).
    /// Échec connu sur GLB Meshy avec accessoires invisibles → mesh aplati.
    TemplateFit,
}

/// Système Pinocchio V1. Tourne en parallèle de `auto_rig_pending_meshes`
/// (historique TemplateFit), gated sur `AutoRigBackend == PinocchioV1`.
///
/// Templates : lit depuis `Genome<SkeletonTemplate>` (TOML asset) avec
/// fallback hardcoded si genome pas encore chargé (pattern graceful Forgia).
#[allow(clippy::too_many_arguments)]
pub fn auto_rig_pinocchio_v1(
    mut commands: Commands,
    time: Res<Time>,
    backend: Res<AutoRigBackend>,
    mut stats: ResMut<AutoRigStats>,
    humanoid_handle: Option<Res<SkeletonHumanoidGenomeHandle>>,
    lizard_handle: Option<Res<SkeletonBipedLizardGenomeHandle>>,
    skeleton_genomes: Res<Assets<Genome<SkeletonTemplate>>>,
    q_pending: Query<(Entity, &NeedsAutoRig), Without<AutoRigged>>,
    children_q: Query<&Children>,
    transforms_q: Query<&Transform>,
    _aabbs_q: Query<&Aabb>,
    meshes_assets: Res<Assets<Mesh>>,
    q_mesh3d: Query<&Mesh3d>,
) {
    // Gate sur le backend choisi.
    if *backend != AutoRigBackend::PinocchioV1 {
        return;
    }

    for (mesh_root, request) in &q_pending {
        let NeedsAutoRig::Template(requested_template) = *request;

        // 1. Extract first mesh3d (positions + indices) en repère mesh-local.
        let Some((positions_local, indices)) =
            extract_first_mesh_local(mesh_root, &children_q, &transforms_q, &q_mesh3d, &meshes_assets)
        else {
            // Pas encore de Mesh3d dispo, retry frame suivant.
            continue;
        };

        if positions_local.len() < 100 || indices.len() < 3 {
            warn!(
                "[forgia-auto-rig::pinocchio] mesh too small ({} verts, {} indices) — skip",
                positions_local.len(),
                indices.len()
            );
            continue;
        }

        // Compute bounds depuis positions
        let (bounds_min, bounds_max) = compute_bounds(&positions_local);
        let mesh_height = (bounds_max.y - bounds_min.y).max(0.001);

        // ── Auto-switch template via landmarks anatomiques ───────────────────
        // BUG runtime 2026-05-17 night : Pinocchio appliquait BipedLizard sur
        // Apprenti humanoid → bras vers l'avant (template lizard) au lieu de
        // latéral T-pose. Fix : detect landmarks + switch template suggested.
        let aabb_for_landmarks = bevy::camera::primitives::Aabb {
            center: ((bounds_min + bounds_max) * 0.5).into(),
            half_extents: ((bounds_max - bounds_min) * 0.5).into(),
        };
        let landmarks =
            anatomy_detect::detect_landmarks_from_vertices(&positions_local, &aabb_for_landmarks);
        // Template = explicitly requested by caller. Auto-detection is logged
        // for diagnostic only, but does NOT override the requested template.
        // Origine 2026-05-18 : Rex demandait BipedLizard mais looks_humanoid()
        // basé sur has_tail (geom) faux-positivait. Le caller connaît sa
        // morphologie, on lui fait confiance. Si auto-detection souhaitée plus
        // tard : ajouter variant `NeedsAutoRig::Auto` opt-in.
        if landmarks.vertex_count >= 100 {
            let suggested = if landmarks.looks_humanoid() {
                AutoRigTemplate::Humanoid
            } else {
                AutoRigTemplate::BipedLizard
            };
            if suggested != requested_template {
                info!(
                    "[forgia-auto-rig::pinocchio] hint: landmarks suggest {:?} but caller requested {:?} \
                     (aspect={:.2}, hip_y_frac={:.2}, has_tail={}) — honoring caller",
                    suggested,
                    requested_template,
                    landmarks.aspect_ratio_xy,
                    landmarks.hip_y_frac,
                    landmarks.has_tail
                );
            }
        }
        let template = requested_template;

        // 2. Voxelize
        let voxel_cfg = VoxelizerConfig {
            resolution: VoxelizerConfig::resolution_for_mesh_size(mesh_height),
            ..Default::default()
        };
        let t0 = std::time::Instant::now();
        let grid = voxelize_mesh(&positions_local, &indices, &voxel_cfg);
        let voxel_ms = t0.elapsed().as_millis();
        let voxel_count = grid.filled_count();

        if voxel_count < 50 {
            warn!(
                "[forgia-auto-rig::pinocchio] voxelization yielded only {} voxels (mesh non-watertight?) — falling back to TemplateFit logic next frame",
                voxel_count
            );
            // Skip ce frame ; ne pas insert AutoRigged → retry possible.
            continue;
        }

        // 3. Medial axis
        let t1 = std::time::Instant::now();
        let graph = extract_medial_axis(&grid, &MedialAxisConfig::default());
        let medial_ms = t1.elapsed().as_millis();
        let sphere_count = graph.sphere_count();

        if sphere_count == 0 {
            warn!("[forgia-auto-rig::pinocchio] 0 medial spheres extracted — skip");
            continue;
        }

        // 4. Embed template skeleton — load depuis TOML genome avec fallback hardcoded.
        let (skel_template, genome_used) = load_skeleton_template(
            template,
            &humanoid_handle,
            &lizard_handle,
            &skeleton_genomes,
        );
        let embedded = embed_template_skeleton(&skel_template, &graph, (bounds_min, bounds_max));

        // 5. Spawn bones aux positions embedded (en repère mesh-root-local).
        let bone_count = spawn_embedded_bones(&mut commands, mesh_root, &embedded);

        // 6. Marker + stats
        commands
            .entity(mesh_root)
            .remove::<NeedsAutoRig>()
            .insert(AutoRigged {
                template: Some(template),
                bone_count,
                mesh_height,
            });

        stats.total_rig_successes += 1;
        stats.last_template = Some(template);
        stats.last_requested_template = Some(requested_template);
        stats.last_auto_switched = template != requested_template;
        stats.last_landmarks = if landmarks.vertex_count >= 100 {
            Some(landmarks)
        } else {
            None
        };
        stats.last_bone_count = bone_count;
        stats.last_mesh_height = mesh_height;
        stats.last_rigged_at_secs = time.elapsed_secs();
        stats.last_aabb_min = bounds_min;
        stats.last_aabb_max = bounds_max;
        stats.last_total_mesh_vertices = positions_local.len();
        stats.last_pinocchio_voxel_count = voxel_count;
        stats.last_pinocchio_sphere_count = sphere_count;
        stats.last_pinocchio_embedding_cost = embedded.total_embedding_cost;
        stats.last_pinocchio_voxelize_ms = voxel_ms as u32;
        stats.last_pinocchio_medial_ms = medial_ms as u32;

        // Bones clés pour sensor bones_world (parité avec TemplateFit pipeline
        // lib.rs:884). Positions en repère mesh-root-local — non propagées par
        // le Transform parent. Permet à diagnose_interpretation() de comparer
        // foot_L.y vs aabb_min.y au lieu d'afficher des zéros trompeurs.
        let find_bone = |name: &str| {
            embedded
                .bones
                .iter()
                .find(|b| b.name == name)
                .map(|b| b.world_pos)
        };
        stats.last_hip_world_y = find_bone("hip").map(|p| p.y).unwrap_or(0.0);
        stats.last_head_world_y = find_bone("head").map(|p| p.y).unwrap_or(0.0);
        stats.last_foot_l_world_y = find_bone("foot_L").map(|p| p.y).unwrap_or(0.0);
        stats.last_forearm_l_world_x = find_bone("forearm_L").map(|p| p.x).unwrap_or(0.0);

        info!(
            "[forgia-auto-rig::pinocchio] mesh {:?} : {} verts → {} voxels ({}ms) → {} medial spheres ({}ms) → {} bones embedded (cost={:.3}) (template_source={})",
            mesh_root,
            positions_local.len(),
            voxel_count,
            voxel_ms,
            sphere_count,
            medial_ms,
            bone_count,
            embedded.total_embedding_cost,
            genome_used
        );
    }
}

/// Helper : load `SkeletonTemplate` depuis TOML genome. Fallback hardcoded si
/// genome pas encore chargé (pattern graceful Forgia identique à
/// `ArenaBotsGenome` dans `forgia-mode-fps-arena`). Retourne aussi un label
/// "toml" ou "fallback" pour diagnostic logs.
fn load_skeleton_template(
    template: AutoRigTemplate,
    humanoid_handle: &Option<Res<SkeletonHumanoidGenomeHandle>>,
    lizard_handle: &Option<Res<SkeletonBipedLizardGenomeHandle>>,
    skeleton_genomes: &Assets<Genome<SkeletonTemplate>>,
) -> (SkeletonTemplate, &'static str) {
    let handle: Option<&Handle<Genome<SkeletonTemplate>>> = match template {
        AutoRigTemplate::Humanoid => humanoid_handle.as_ref().map(|h| &h.0),
        AutoRigTemplate::BipedLizard => lizard_handle.as_ref().map(|h| &h.0),
    };
    if let Some(h) = handle {
        if let Some(genome) = skeleton_genomes.get(h) {
            return (genome.data.clone(), "toml");
        }
    }
    // Fallback hardcoded (graceful — log warn pour visibilité)
    warn!(
        "[forgia-auto-rig::pinocchio] genome {:?} pas chargé, fallback hardcoded",
        template
    );
    let fallback = match template {
        AutoRigTemplate::Humanoid => SkeletonTemplate::humanoid(),
        AutoRigTemplate::BipedLizard => SkeletonTemplate::biped_lizard(),
    };
    (fallback, "fallback_hardcoded")
}

/// Walk descendants pour trouver le PREMIER Mesh3d et extraire ses positions
/// avec ses indices en repère mesh-root-local (transforms intermédiaires
/// propagés manuellement via Mat4 — bug Bevy #18921 contourné).
///
/// V1 : un seul mesh. V2 fusionnera tous les meshes.
fn extract_first_mesh_local(
    root: Entity,
    children_q: &Query<&Children>,
    transforms_q: &Query<&Transform>,
    q_mesh3d: &Query<&Mesh3d>,
    meshes_assets: &Assets<Mesh>,
) -> Option<(Vec<Vec3>, Vec<u32>)> {
    let mut stack: Vec<(Entity, Mat4)> = vec![(root, Mat4::IDENTITY)];
    while let Some((entity, parent_world)) = stack.pop() {
        let local_mat = if entity == root {
            Mat4::IDENTITY
        } else {
            transforms_q
                .get(entity)
                .map(|t| t.to_matrix())
                .unwrap_or(Mat4::IDENTITY)
        };
        let world_mat = parent_world * local_mat;

        if let Ok(m3d) = q_mesh3d.get(entity) {
            if let Some(mesh) = meshes_assets.get(&m3d.0) {
                // Positions → transform vers mesh-root-local
                let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                    Some(VertexAttributeValues::Float32x3(p)) => p
                        .iter()
                        .map(|v| world_mat.transform_point3(Vec3::from_array(*v)))
                        .collect::<Vec<_>>(),
                    _ => continue,
                };
                let indices = match mesh.indices() {
                    Some(Indices::U32(i)) => i.clone(),
                    Some(Indices::U16(i)) => i.iter().map(|&x| u32::from(x)).collect(),
                    None => {
                        // Mesh sans indices (= triangle soup) : generate sequential
                        (0..positions.len() as u32).collect()
                    }
                };
                return Some((positions, indices));
            }
        }

        if let Ok(children) = children_q.get(entity) {
            for c in children.iter() {
                stack.push((c, world_mat));
            }
        }
    }
    None
}

fn compute_bounds(positions: &[Vec3]) -> (Vec3, Vec3) {
    let mut min_v = Vec3::splat(f32::INFINITY);
    let mut max_v = Vec3::splat(f32::NEG_INFINITY);
    for p in positions {
        min_v = min_v.min(*p);
        max_v = max_v.max(*p);
    }
    (min_v, max_v)
}

/// Spawn bones aux positions embedded. Hiérarchie : root bone enfant du mesh_root,
/// puis chaque bone enfant de son parent embedded. Compatible avec
/// `forgia-rig-topology` qui s'attend à `mesh_root → root_bone → ...`.
fn spawn_embedded_bones(
    commands: &mut Commands,
    mesh_root: Entity,
    embedded: &EmbeddedSkeleton,
) -> usize {
    use std::collections::HashMap;
    let mut entity_by_idx: HashMap<usize, Entity> = HashMap::with_capacity(embedded.bones.len());

    for (idx, bone) in embedded.bones.iter().enumerate() {
        // Position locale : si root, en local au mesh_root (= embedded.world_pos).
        // Si enfant, en local au parent (= delta entre embedded positions).
        let parent_entity = match bone.parent {
            None => mesh_root,
            Some(parent_idx) => *entity_by_idx.get(&parent_idx).unwrap_or(&mesh_root),
        };
        let parent_world_pos = match bone.parent {
            None => Vec3::ZERO,
            Some(parent_idx) => embedded.bones[parent_idx].world_pos,
        };
        let local_translation = bone.world_pos - parent_world_pos;

        let bone_entity = commands
            .spawn((
                Name::new(bone.name.clone()),
                Transform::from_translation(local_translation),
                Visibility::default(),
                BoneEntity { rig_root: mesh_root },
            ))
            .id();
        entity_by_idx.insert(idx, bone_entity);

        commands.entity(parent_entity).add_child(bone_entity);
    }

    embedded.bones.len()
}

// ── Tests headless ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_default_is_pinocchio_v1() {
        // Story-440 R&D : Pinocchio V1 par défaut pour validation visuelle.
        let b = AutoRigBackend::default();
        assert_eq!(b, AutoRigBackend::PinocchioV1);
    }

    #[test]
    fn compute_bounds_matches_aabb() {
        let positions = vec![
            Vec3::new(-1.0, 0.0, 0.5),
            Vec3::new(1.0, 2.0, -0.5),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let (min, max) = compute_bounds(&positions);
        assert_eq!(min, Vec3::new(-1.0, 0.0, -0.5));
        assert_eq!(max, Vec3::new(1.0, 2.0, 0.5));
    }
}
