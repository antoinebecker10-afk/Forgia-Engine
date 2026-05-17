//! # forgia-skeleton-embedder — Template skeleton embedding sur medial axis
//!
//! **Phase 1 du pipeline auto-rig Forgia (Pinocchio-inspired, story-440)** :
//! prend un `SkeletonTemplate` (positions normalisées des bones) + un
//! `MedialAxisGraph` (output de `forgia-medial-axis`), et produit un
//! `EmbeddedSkeleton` où chaque bone est positionné sur une sphère médiale
//! qui matche sa morphologie attendue.
//!
//! ## Approche (V1 simplifiée)
//!
//! Pinocchio (Baran 2007) utilise un Mixed Integer Program pour optimiser
//! discrètement l'embedding (NP-hard mais résolu en ~10ms via heuristiques).
//! V1 Forgia : **greedy nearest-sphere matching** par BFS du template tree.
//!
//! Algorithme :
//! 1. **Compute target positions monde** depuis les positions normalisées
//!    du template + AABB du graph.
//! 2. **Place root** (hip) sur la sphère médiale la plus proche de la position
//!    target du hip.
//! 3. **BFS template tree** : pour chaque bone enfant, calcule sa position
//!    target (= parent_embedded_pos + delta_template_normalisé × mesh_height),
//!    trouve la sphère médiale la plus proche, place le bone dessus.
//!
//! Avantages vs template-fit AABB :
//! - Bones contraints à rester DANS la forme du mesh (medial axis = intérieur)
//! - Auto-adapte aux morphologies non-Vitruvian (hanches larges, tête grosse, etc.)
//! - Robuste aux outliers AABB (cheveux, cape) car medial axis lisse
//!
//! ## API
//!
//! ```ignore
//! let template = SkeletonTemplate::humanoid();
//! let embedded = embed_template_skeleton(&template, &medial_graph, 1.75);
//! for bone in &embedded.bones {
//!     spawn_bone_entity(bone.world_pos, &bone.name);
//! }
//! ```

use bevy::math::Vec3;
use bevy::reflect::TypePath;
use forgia_medial_axis::{MedialAxisGraph, MedialSphere};
use serde::Deserialize;

/// Bone d'un template anatomique. Position normalisée : `(x, y, z)` où
/// `y ∈ [0, 1]` (0=sol, 1=sommet du mesh) et `x, z` en fraction de la
/// hauteur (latéralité).
///
/// **Deserializable** depuis TOML via `forgia-genome-core::Genome` —
/// `[f32; 3]` au lieu de `Vec3` pour serde simple (sans feature glam-serde).
#[derive(Debug, Clone, Deserialize)]
pub struct TemplateBone {
    pub name: String,
    /// Index du parent dans `SkeletonTemplate.bones`. `None` = root.
    pub parent: Option<usize>,
    /// Position normalisée [x, y, z] dans le repère mesh.
    pub pos: [f32; 3],
}

impl TemplateBone {
    /// Helper : position en `Vec3` (compatibilité ergonomique).
    pub fn pos_vec3(&self) -> Vec3 {
        Vec3::from_array(self.pos)
    }
}

/// Template anatomique : hiérarchie de bones avec positions normalisées.
///
/// **Format TOML** :
/// ```toml
/// [[bones]]
/// name = "hip"
/// parent = ""  # vide = root
/// pos = [0.0, 0.50, 0.0]
///
/// [[bones]]
/// name = "spine_lower"
/// parent = 0  # index du hip
/// pos = [0.0, 0.58, 0.0]
/// ```
#[derive(Debug, Clone, Deserialize, TypePath)]
pub struct SkeletonTemplate {
    pub bones: Vec<TemplateBone>,
}

impl SkeletonTemplate {
    /// Template humanoid Vitruvian 18 bones. Compatible avec
    /// `forgia-rig-topology` classification.
    pub fn humanoid() -> Self {
        Self::from_data(&[
            ("hip",         None,     [0.0,   0.50,  0.0]),
            ("spine_lower", Some(0),  [0.0,   0.58,  0.0]),
            ("spine_mid",   Some(1),  [0.0,   0.66,  0.0]),
            ("chest",       Some(2),  [0.0,   0.75,  0.0]),
            ("neck",        Some(3),  [0.0,   0.85,  0.0]),
            ("head",        Some(4),  [0.0,   0.95,  0.0]),
            ("clavicle_L",  Some(3),  [-0.08, 0.80,  0.0]),
            ("arm_L",       Some(6),  [-0.20, 0.78,  0.0]),
            ("forearm_L",   Some(7),  [-0.38, 0.78,  0.0]),
            ("clavicle_R",  Some(3),  [0.08,  0.80,  0.0]),
            ("arm_R",       Some(9),  [0.20,  0.78,  0.0]),
            ("forearm_R",   Some(10), [0.38,  0.78,  0.0]),
            ("thigh_L",     Some(0),  [-0.10, 0.40,  0.0]),
            ("shin_L",      Some(12), [-0.10, 0.20,  0.0]),
            ("foot_L",      Some(13), [-0.10, 0.02,  0.05]),
            ("thigh_R",     Some(0),  [0.10,  0.40,  0.0]),
            ("shin_R",      Some(15), [0.10,  0.20,  0.0]),
            ("foot_R",      Some(16), [0.10,  0.02,  0.05]),
        ])
    }

    /// Template biped lézard (Rex). 14+4 tail = 18 bones.
    pub fn biped_lizard() -> Self {
        Self::from_data(&[
            ("hip",         None,     [0.0,   0.45,  0.0]),
            ("spine_lower", Some(0),  [0.0,   0.53,  0.04]),
            ("spine_mid",   Some(1),  [0.0,   0.63,  0.10]),
            ("chest",       Some(2),  [0.0,   0.71,  0.16]),
            ("neck",        Some(3),  [0.0,   0.76,  0.24]),
            ("head",        Some(4),  [0.0,   0.82,  0.29]),
            ("arm_L",       Some(3),  [-0.12, 0.71,  0.18]),
            ("forearm_L",   Some(6),  [-0.16, 0.59,  0.22]),
            ("arm_R",       Some(3),  [0.12,  0.71,  0.18]),
            ("forearm_R",   Some(8),  [0.16,  0.59,  0.22]),
            ("thigh_L",     Some(0),  [-0.10, 0.37,  0.0]),
            ("shin_L",      Some(10), [-0.10, 0.19,  0.0]),
            ("foot_L",      Some(11), [-0.10, 0.04,  0.06]),
            ("thigh_R",     Some(0),  [0.10,  0.37,  0.0]),
            ("shin_R",      Some(13), [0.10,  0.19,  0.0]),
            ("foot_R",      Some(14), [0.10,  0.04,  0.06]),
            ("tail_01",     Some(0),  [0.0,   0.41, -0.12]),
            ("tail_02",     Some(16), [0.0,   0.39, -0.22]),
            ("tail_03",     Some(17), [0.0,   0.37, -0.32]),
            ("tail_04",     Some(18), [0.0,   0.35, -0.42]),
        ])
    }

    /// Helper : construit un template depuis `(name, parent, pos)` tuples.
    /// Pattern utilisé pour les fallbacks hardcoded ET pour les tests unitaires.
    /// La source de vérité runtime = TOML genome via `forgia-genome-core`.
    fn from_data(data: &[(&str, Option<usize>, [f32; 3])]) -> Self {
        Self {
            bones: data
                .iter()
                .map(|(name, parent, pos)| TemplateBone {
                    name: (*name).to_string(),
                    parent: *parent,
                    pos: *pos,
                })
                .collect(),
        }
    }
}

/// Bone embedded sur le medial axis : position monde + rayon de la sphère
/// médiale associée.
#[derive(Debug, Clone)]
pub struct EmbeddedBone {
    pub name: String,
    pub parent: Option<usize>,
    pub world_pos: Vec3,
    /// Rayon de la sphère médiale snapped (utile pour visualisation + skinning).
    pub medial_radius: f32,
    /// Distance entre la position target (template) et la position snapped.
    /// Diagnostic : si grande, le template ne fit pas bien la morphologie.
    pub snap_distance: f32,
}

/// Squelette embedded sur le medial axis graph.
#[derive(Debug, Clone)]
pub struct EmbeddedSkeleton {
    pub bones: Vec<EmbeddedBone>,
    /// Coût total de l'embedding (sum of snap_distances). Faible = bon fit.
    pub total_embedding_cost: f32,
}

/// Chaîne anatomique : segment continu de bones du squelette (spine, arm L,
/// leg R, tail...). Définie par un `attach` (bone parent déjà embedded depuis
/// chaîne précédente, `None` si chaîne root) et la liste des bones de la
/// chaîne dans l'ordre proximal→distal.
///
/// **Pourquoi les chaînes au lieu de bones isolés** : l'anatomie a une
/// structure CONTRAINTE (foot doit être proche de shin, etc.). Embedding
/// bone-par-bone produit des chaînes désolidarisées (jambes en V inversé
/// observé runtime 2026-05-17 night). Embedding chain-aware garantit
/// continuité géométrique.
#[derive(Debug, Clone)]
pub struct BoneChain {
    /// Index du bone d'attache (déjà embedded). `None` = chaîne root.
    pub attach: Option<usize>,
    /// Bones de la chaîne, proximal→distal. `bones[0]` est attaché au parent
    /// `attach` (ou est le root si `attach=None`).
    pub bones: Vec<usize>,
}

/// Décompose un template en chaînes anatomiques. Pour Humanoid (18 bones) :
/// 7 chaînes typiques (root + spine + neck + 2 arms + 2 legs).
///
/// Algorithme : BFS depuis root. Tant qu'un bone a 1 seul enfant, on continue
/// la chaîne courante. Si 0 enfants → chaîne terminée (terminal). Si ≥2
/// enfants → on push N nouvelles chaînes (chacune démarrant à un enfant) et
/// la chaîne courante se termine au branch point.
pub fn decompose_into_chains(template: &SkeletonTemplate) -> Vec<BoneChain> {
    let n = template.bones.len();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut root_idx = 0;
    for (i, b) in template.bones.iter().enumerate() {
        match b.parent {
            Some(p) => children[p].push(i),
            None => root_idx = i,
        }
    }

    let mut chains: Vec<BoneChain> = Vec::new();
    // Root chain : juste le bone root (= branch point ≥2 enfants typique).
    chains.push(BoneChain {
        attach: None,
        bones: vec![root_idx],
    });
    let mut stack: Vec<(usize, usize)> = Vec::new(); // (attach_idx, start_bone)
    for &k in &children[root_idx] {
        stack.push((root_idx, k));
    }

    while let Some((attach_idx, start)) = stack.pop() {
        let mut chain = vec![start];
        let mut current = start;
        loop {
            let kids = &children[current];
            match kids.len() {
                0 => break, // terminal
                1 => {
                    chain.push(kids[0]);
                    current = kids[0];
                }
                _ => {
                    // Branche : termine la chaîne courante au branch point.
                    // Démarre N nouvelles chaînes attachées à current.
                    for &k in kids {
                        stack.push((current, k));
                    }
                    break;
                }
            }
        }
        chains.push(BoneChain {
            attach: Some(attach_idx),
            bones: chain,
        });
    }

    chains
}

/// Embed un template skeleton sur un medial axis graph via chain-aware
/// algorithm.
///
/// **Algorithme** :
/// 1. Decompose template en chaînes anatomiques (spine, arms, legs, tail).
/// 2. Process chains topologiquement (root first, descendants after).
/// 3. Pour chaque chaîne :
///    a. `attach_pos` = position embedded du parent (ou template root si root chain).
///    b. Compute direction + longueur template de la chaîne.
///    c. Find **terminal sphere** : medial sphere proche du target terminal,
///    à distance ≈ chain_length de attach, alignée avec chain direction.
///    d. **Backfill** bones intermédiaires : interpolation linéaire entre
///    attach et terminal. Pas de snap medial sur intermédiaires (= garantit
///    continuité, évite jambes en V inversé).
/// 4. Output `EmbeddedSkeleton` avec bones dans l'ordre template original.
///
/// `mesh_bounds` = (min, max) AABB du mesh — utilisé pour scaler les positions
/// normalisées du template en coordonnées monde.
pub fn embed_template_skeleton(
    template: &SkeletonTemplate,
    graph: &MedialAxisGraph,
    mesh_bounds: (Vec3, Vec3),
) -> EmbeddedSkeleton {
    let (bounds_min, bounds_max) = mesh_bounds;
    let mesh_size = bounds_max - bounds_min;
    let mesh_height = mesh_size.y.max(0.001);
    let center_xz = Vec3::new(
        (bounds_min.x + bounds_max.x) * 0.5,
        bounds_min.y, // ancrage au sol
        (bounds_min.z + bounds_max.z) * 0.5,
    );

    if graph.spheres.is_empty() {
        // Fallback : positions template scalées sans embedding (= template-fit AABB classique)
        let bones = template
            .bones
            .iter()
            .map(|tb| {
                let world_pos = center_xz + tb.pos_vec3() * mesh_height;
                EmbeddedBone {
                    name: tb.name.clone(),
                    parent: tb.parent,
                    world_pos,
                    medial_radius: 0.05,
                    snap_distance: 0.0,
                }
            })
            .collect();
        return EmbeddedSkeleton {
            bones,
            total_embedding_cost: 0.0,
        };
    }

    // 1. Decompose en chaînes anatomiques.
    let chains = decompose_into_chains(template);

    // 2. Process chains, fill embedded indexé par template bone index.
    let mut embedded: Vec<Option<EmbeddedBone>> = vec![None; template.bones.len()];
    let mut total_cost = 0.0;

    for chain in &chains {
        embed_one_chain(
            chain,
            template,
            graph,
            center_xz,
            mesh_height,
            &mut embedded,
            &mut total_cost,
        );
    }

    // 3. Output dans l'ordre template original (chaque template bone DOIT être embedded).
    let bones: Vec<EmbeddedBone> = embedded
        .into_iter()
        .enumerate()
        .map(|(i, opt)| {
            opt.unwrap_or_else(|| {
                // Fallback : si bone pas embedded (bug decompose ?), pos template pure.
                let tb = &template.bones[i];
                EmbeddedBone {
                    name: tb.name.clone(),
                    parent: tb.parent,
                    world_pos: center_xz + tb.pos_vec3() * mesh_height,
                    medial_radius: 0.05,
                    snap_distance: 0.0,
                }
            })
        })
        .collect();

    EmbeddedSkeleton {
        bones,
        total_embedding_cost: total_cost,
    }
}

/// Embed une chaîne : place terminal via medial axis (alignement direction +
/// longueur), interpole les intermédiaires linéairement.
fn embed_one_chain(
    chain: &BoneChain,
    template: &SkeletonTemplate,
    graph: &MedialAxisGraph,
    center_xz: Vec3,
    mesh_height: f32,
    embedded: &mut [Option<EmbeddedBone>],
    total_cost: &mut f32,
) {
    if chain.bones.is_empty() {
        return;
    }

    // Position attache (parent déjà embedded ou template root).
    let attach_world = match chain.attach {
        Some(attach_idx) => embedded[attach_idx]
            .as_ref()
            .map(|b| b.world_pos)
            .unwrap_or_else(|| {
                let tb = &template.bones[attach_idx];
                center_xz + tb.pos_vec3() * mesh_height
            }),
        None => {
            // Root chain : pos = template root scalée + snappée nearest sphere.
            let root_tb = &template.bones[chain.bones[0]];
            let root_target = center_xz + root_tb.pos_vec3() * mesh_height;
            let (root_sphere, snap_dist) = find_nearest_sphere(&graph.spheres, root_target);
            embedded[chain.bones[0]] = Some(EmbeddedBone {
                name: root_tb.name.clone(),
                parent: root_tb.parent,
                world_pos: root_sphere.center,
                medial_radius: root_sphere.radius,
                snap_distance: snap_dist,
            });
            *total_cost += snap_dist;
            // Si root chain a 1 seul bone, fini.
            if chain.bones.len() == 1 {
                return;
            }
            root_sphere.center
        }
    };

    // Direction & longueur template chaîne (attach → terminal en template normalisé).
    let attach_template_pos = match chain.attach {
        Some(attach_idx) => {
            center_xz + template.bones[attach_idx].pos_vec3() * mesh_height
        }
        None => center_xz + template.bones[chain.bones[0]].pos_vec3() * mesh_height,
    };
    let terminal_template_pos = center_xz
        + template.bones[*chain.bones.last().unwrap()].pos_vec3() * mesh_height;
    let chain_direction = (terminal_template_pos - attach_template_pos).normalize_or_zero();
    let chain_length = attach_template_pos.distance(terminal_template_pos);

    // Find terminal sphere : alignement direction + distance ≈ chain_length depuis attach.
    let target_terminal = attach_world + chain_direction * chain_length;
    let terminal_sphere = if chain_direction.length_squared() > 1e-6 {
        find_terminal_sphere(
            &graph.spheres,
            attach_world,
            target_terminal,
            chain_direction,
            chain_length,
        )
    } else {
        // Chaîne degénérée (attach == terminal en template), juste nearest.
        find_nearest_sphere(&graph.spheres, target_terminal).0
    };

    // Backfill : interpolation linéaire attach → terminal_sphere, par segment template.
    let n = chain.bones.len();
    // Compute cumulative lengths template pour interpolation proportionnelle.
    let mut cumul_lengths: Vec<f32> = vec![0.0; n + 1];
    cumul_lengths[0] = 0.0;
    let mut prev_pos = attach_template_pos;
    for (i, &bone_idx) in chain.bones.iter().enumerate() {
        let pos = center_xz + template.bones[bone_idx].pos_vec3() * mesh_height;
        cumul_lengths[i + 1] = cumul_lengths[i] + prev_pos.distance(pos);
        prev_pos = pos;
    }
    let total_template_length = cumul_lengths[n].max(0.001);

    // Actual chain length in world = distance attach_world → terminal_sphere.center
    let actual_chain_length = attach_world.distance(terminal_sphere.center);
    let chain_direction_actual = if actual_chain_length > 1e-6 {
        (terminal_sphere.center - attach_world) / actual_chain_length
    } else {
        chain_direction
    };

    for (i, &bone_idx) in chain.bones.iter().enumerate() {
        let t = cumul_lengths[i + 1] / total_template_length;
        let bone_world_pos = attach_world + chain_direction_actual * (t * actual_chain_length);

        let tb = &template.bones[bone_idx];
        let snap_dist = bone_world_pos.distance(center_xz + tb.pos_vec3() * mesh_height);

        // Radius : use terminal sphere radius pour le bone terminal, sinon estim.
        let radius = if i == n - 1 {
            terminal_sphere.radius
        } else {
            // Find nearest sphere just for radius estimation (no position snap).
            find_nearest_sphere(&graph.spheres, bone_world_pos).0.radius
        };

        embedded[bone_idx] = Some(EmbeddedBone {
            name: tb.name.clone(),
            parent: tb.parent,
            world_pos: bone_world_pos,
            medial_radius: radius,
            snap_distance: snap_dist,
        });
        *total_cost += snap_dist;
    }
}

/// Trouve la sphère terminale d'une chaîne : alignement direction (cos angle)
/// + distance attach ≈ chain_length attendu + proche du target.
fn find_terminal_sphere(
    spheres: &[MedialSphere],
    attach: Vec3,
    target: Vec3,
    direction: Vec3,
    chain_length: f32,
) -> MedialSphere {
    let mut best_score = f32::INFINITY;
    let mut best = spheres[0];
    for s in spheres {
        let to_sphere = s.center - attach;
        let to_sphere_len = to_sphere.length().max(0.001);
        let to_sphere_dir = to_sphere / to_sphere_len;
        let cos_angle = to_sphere_dir.dot(direction); // -1..1
        // Score = distance_to_target + length_penalty + alignment_penalty
        let dist_to_target = s.center.distance(target);
        let length_penalty = 2.0 * (to_sphere_len - chain_length).abs();
        let alignment_penalty = chain_length * (1.0 - cos_angle); // weight ~chain_length
        let score = dist_to_target + length_penalty + alignment_penalty;
        if score < best_score {
            best_score = score;
            best = *s;
        }
    }
    best
}

fn find_nearest_sphere(spheres: &[MedialSphere], target: Vec3) -> (MedialSphere, f32) {
    let mut best_dist = f32::INFINITY;
    let mut best_sphere = spheres[0];
    for s in spheres {
        let d = s.center.distance(target);
        if d < best_dist {
            best_dist = d;
            best_sphere = *s;
        }
    }
    (best_sphere, best_dist)
}

/// Find best sphere with optional direction hint. Si `direction_hint =
/// (parent_pos, expected_direction)`, le score combine distance + alignement
/// directionnel : `score = distance - alignment_bonus * cos_angle`.
///
/// Évite le snap collapse sur l'axe central quand le template attend une
/// position latérale (bras T-pose, jambes). Cf bug runtime 2026-05-17 night :
/// medial axis ligne verticale → tous bones nearest = centre.
///
/// **Note** : superseded par `embed_one_chain` (chain-aware) qui interpole
/// les bones d'une chaîne. Cette fonction reste exposée pour cas d'usage
/// custom (snap d'un bone isolé hors d'une chaîne).
#[allow(dead_code)]
fn find_best_sphere(
    spheres: &[MedialSphere],
    target: Vec3,
    direction_hint: Option<(Vec3, Vec3)>,
) -> (MedialSphere, f32) {
    if direction_hint.is_none() {
        return find_nearest_sphere(spheres, target);
    }
    let (parent_pos, expected_dir) = direction_hint.unwrap();
    let target_distance_from_parent = (target - parent_pos).length().max(0.01);

    // Bonus directionnel proportionnel à target_distance_from_parent (les bones
    // proches du parent = peu d'orientation requise ; les bones loin = direction
    // critique).
    let alignment_weight = (target_distance_from_parent * 2.0).clamp(0.2, 5.0);

    let mut best_score = f32::INFINITY;
    let mut best_sphere = spheres[0];
    for s in spheres {
        let to_sphere = s.center - parent_pos;
        let to_sphere_len = to_sphere.length().max(0.001);
        let to_sphere_dir = to_sphere / to_sphere_len;
        let cos_angle = to_sphere_dir.dot(expected_dir); // -1..1
        // Distance pénalisée par mauvais alignement (cos -1 ajoute 2*weight au score)
        let distance = s.center.distance(target);
        let score = distance + alignment_weight * (1.0 - cos_angle);
        if score < best_score {
            best_score = score;
            best_sphere = *s;
        }
    }
    let snap_dist = best_sphere.center.distance(target);
    (best_sphere, snap_dist)
}

// ── Tests headless ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_medial_axis::MedialSphere;

    /// Mock simple : 3 sphères alignées sur Y (cylindrique).
    fn axial_3_spheres() -> MedialAxisGraph {
        MedialAxisGraph {
            spheres: vec![
                MedialSphere {
                    center: Vec3::new(0.0, 0.2, 0.0),
                    radius: 0.15,
                },
                MedialSphere {
                    center: Vec3::new(0.0, 0.5, 0.0),
                    radius: 0.15,
                },
                MedialSphere {
                    center: Vec3::new(0.0, 0.8, 0.0),
                    radius: 0.15,
                },
            ],
            edges: vec![(0, 1), (1, 2)],
        }
    }

    #[test]
    fn humanoid_template_has_18_bones() {
        let t = SkeletonTemplate::humanoid();
        assert_eq!(t.bones.len(), 18);
        // Premier bone = root (parent None)
        assert!(t.bones[0].parent.is_none());
        assert_eq!(t.bones[0].name, "hip");
    }

    #[test]
    fn biped_lizard_template_has_20_bones() {
        let t = SkeletonTemplate::biped_lizard();
        assert_eq!(t.bones.len(), 20); // 14 body + 6 (foot L/R = 6 leg total) = 16 + 4 tail = 20
    }

    #[test]
    fn embed_on_empty_graph_uses_fallback() {
        let template = SkeletonTemplate::humanoid();
        let empty_graph = MedialAxisGraph {
            spheres: vec![],
            edges: vec![],
        };
        let bounds = (Vec3::ZERO, Vec3::ONE);
        let embedded = embed_template_skeleton(&template, &empty_graph, bounds);
        assert_eq!(embedded.bones.len(), template.bones.len());
        // Sans graph, on fallback sur template-fit AABB → cost = 0 (pas de snap)
        assert_eq!(embedded.total_embedding_cost, 0.0);
    }

    #[test]
    fn embed_humanoid_on_axial_spheres_hip_near_middle() {
        let template = SkeletonTemplate::humanoid();
        let graph = axial_3_spheres();
        let bounds = (Vec3::new(-0.2, 0.0, -0.2), Vec3::new(0.2, 1.0, 0.2));
        let embedded = embed_template_skeleton(&template, &graph, bounds);
        let hip = embedded
            .bones
            .iter()
            .find(|b| b.name == "hip")
            .expect("hip");
        // Hip cible normalisé 0.5 → world Y = 0.5. Plus proche sphère = sphere[1] (Y=0.5)
        assert!(
            (hip.world_pos.y - 0.5).abs() < 0.05,
            "hip should snap to middle sphere (Y=0.5), got {:.3}",
            hip.world_pos.y
        );
    }

    #[test]
    fn embed_head_above_hip() {
        let template = SkeletonTemplate::humanoid();
        let graph = axial_3_spheres();
        let bounds = (Vec3::new(-0.2, 0.0, -0.2), Vec3::new(0.2, 1.0, 0.2));
        let embedded = embed_template_skeleton(&template, &graph, bounds);
        let hip_y = embedded
            .bones
            .iter()
            .find(|b| b.name == "hip")
            .unwrap()
            .world_pos
            .y;
        let head_y = embedded
            .bones
            .iter()
            .find(|b| b.name == "head")
            .unwrap()
            .world_pos
            .y;
        assert!(
            head_y > hip_y,
            "head ({}) must be above hip ({})",
            head_y,
            hip_y
        );
    }

    #[test]
    fn embedded_bones_preserve_template_count() {
        let template = SkeletonTemplate::humanoid();
        let graph = axial_3_spheres();
        let bounds = (Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5));
        let embedded = embed_template_skeleton(&template, &graph, bounds);
        assert_eq!(embedded.bones.len(), template.bones.len());
        // Parents préservés
        for (i, b) in embedded.bones.iter().enumerate() {
            assert_eq!(b.parent, template.bones[i].parent);
            assert_eq!(b.name, template.bones[i].name);
        }
    }

    #[test]
    fn embed_total_cost_nonzero_when_snapping() {
        let template = SkeletonTemplate::humanoid();
        let graph = axial_3_spheres();
        let bounds = (Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5));
        let embedded = embed_template_skeleton(&template, &graph, bounds);
        // 18 bones snapped sur 3 sphères → distance non nulle (bras latéraux snappés)
        assert!(
            embedded.total_embedding_cost > 0.0,
            "expected non-zero embedding cost"
        );
    }

    #[test]
    fn snap_distance_is_zero_when_target_equals_sphere_center() {
        let template = SkeletonTemplate {
            bones: vec![TemplateBone {
                name: "single".to_string(),
                parent: None,
                pos: [0.0, 0.5, 0.0],
            }],
        };
        // bounds : center_xz = (0.5, 0, 0.5). target_world = (0.5, 0.5, 0.5).
        // Place la sphère là pour snap_distance = 0.
        let graph = MedialAxisGraph {
            spheres: vec![MedialSphere {
                center: Vec3::new(0.5, 0.5, 0.5),
                radius: 0.1,
            }],
            edges: vec![],
        };
        let bounds = (Vec3::ZERO, Vec3::ONE);
        let embedded = embed_template_skeleton(&template, &graph, bounds);
        assert!(
            embedded.bones[0].snap_distance < 1e-4,
            "snap_distance should be ~0 when target == sphere, got {}",
            embedded.bones[0].snap_distance
        );
    }
}
