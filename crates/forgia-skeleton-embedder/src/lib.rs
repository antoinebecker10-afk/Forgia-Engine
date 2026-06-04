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
use forgia_medial_axis::{MedialAxisGraph, MedialSphere};

// Story-480 Phase 3 (2026-05-20) : SkeletonTemplate + TemplateBone +
// impl SkeletonTemplate (validate, flipped_z, rescaled_for_landmarks*,
// humanoid(), biped_lizard(), from_data) ont été extraits vers la crate
// dédiée `forgia-skeleton-template` (source de vérité unique, conforme
// pattern AAA Unreal USkeleton / Unity Avatar / Godot SkeletonProfile).
//
// Re-export pour compatibilité API : les consumers existants
// (pinocchio_pipeline, tests) continuent de pouvoir importer ces types
// depuis `forgia_skeleton_embedder` sans changer leurs `use` statements.
pub use forgia_skeleton_template::{BoneClass, SkeletonTemplate, TemplateBone};

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

/// Walk le graphe medial axis depuis le sphère index `start` vers `end` via BFS
/// shortest path. Retourne la séquence d'index sphères (incluant start et end).
/// `None` si pas de path (graphe disconnected).
fn shortest_path_indices(
    spheres_count: usize,
    edges: &[(usize, usize)],
    start: usize,
    end: usize,
) -> Option<Vec<usize>> {
    if start == end {
        return Some(vec![start]);
    }
    if start >= spheres_count || end >= spheres_count {
        return None;
    }
    // Build adjacency list (undirected).
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); spheres_count];
    for &(a, b) in edges {
        if a < spheres_count && b < spheres_count {
            adj[a].push(b);
            adj[b].push(a);
        }
    }
    // BFS.
    let mut prev: Vec<Option<usize>> = vec![None; spheres_count];
    let mut visited = vec![false; spheres_count];
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    visited[start] = true;
    queue.push_back(start);
    let mut found = false;
    while let Some(u) = queue.pop_front() {
        if u == end {
            found = true;
            break;
        }
        for &v in &adj[u] {
            if !visited[v] {
                visited[v] = true;
                prev[v] = Some(u);
                queue.push_back(v);
            }
        }
    }
    if !found {
        return None;
    }
    // Reconstruct.
    let mut path = vec![end];
    let mut cur = end;
    while let Some(p) = prev[cur] {
        path.push(p);
        cur = p;
        if cur == start {
            break;
        }
    }
    path.reverse();
    Some(path)
}

/// Trouve le sphère index le plus proche d'une position monde.
fn nearest_sphere_index(spheres: &[MedialSphere], target: Vec3) -> Option<usize> {
    spheres
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.center.distance_squared(target)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
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
        Some(attach_idx) => center_xz + template.bones[attach_idx].pos_vec3() * mesh_height,
        None => center_xz + template.bones[chain.bones[0]].pos_vec3() * mesh_height,
    };
    let terminal_template_pos =
        center_xz + template.bones[*chain.bones.last().unwrap()].pos_vec3() * mesh_height;
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

    // PATH WALKING (2026-05-18 PM) : suit le graphe medial axis depuis la
    // sphère attach jusqu'à la sphère terminale via BFS shortest path. Permet
    // de placer les bones intermédiaires SUR la courbure réelle du membre
    // (queue qui se courbe, jambe pliée, spine penchée) au lieu d'une ligne
    // droite qui sort du mesh.
    //
    // Fallback : si pas de path (graph disconnect), ou path trop court, on
    // revient à l'interpolation linéaire avec snap contraint.
    let attach_sphere_idx = nearest_sphere_index(&graph.spheres, attach_world);
    let terminal_sphere_idx = nearest_sphere_index(&graph.spheres, terminal_sphere.center);
    let path_info: Option<(Vec<Vec3>, Vec<f32>, Vec<f32>, f32)> =
        match (attach_sphere_idx, terminal_sphere_idx) {
            (Some(a), Some(t)) if a != t => {
                shortest_path_indices(graph.spheres.len(), &graph.edges, a, t).and_then(
                    |path_idx| {
                        if path_idx.len() < 2 {
                            return None;
                        }
                        let centers: Vec<Vec3> =
                            path_idx.iter().map(|&i| graph.spheres[i].center).collect();
                        let radii: Vec<f32> =
                            path_idx.iter().map(|&i| graph.spheres[i].radius).collect();
                        // Cumul arc length along the path.
                        let mut arc: Vec<f32> = vec![0.0; centers.len()];
                        for i in 1..centers.len() {
                            arc[i] = arc[i - 1] + centers[i - 1].distance(centers[i]);
                        }
                        let total = arc[arc.len() - 1];
                        if total < 1e-6 {
                            return None;
                        }
                        Some((centers, radii, arc, total))
                    },
                )
            }
            _ => None,
        };

    // Helper : sample la path à la fraction t ∈ [0,1] → (position, radius).
    let sample_path = |t: f32| -> Option<(Vec3, f32)> {
        let (centers, radii, arc, total) = path_info.as_ref()?;
        let target = t.clamp(0.0, 1.0) * total;
        // Find segment that contains target.
        for i in 1..arc.len() {
            if arc[i] >= target {
                let seg_len = arc[i] - arc[i - 1];
                if seg_len < 1e-6 {
                    return Some((centers[i], radii[i]));
                }
                let local_t = (target - arc[i - 1]) / seg_len;
                let pos = centers[i - 1].lerp(centers[i], local_t);
                let r = radii[i - 1] * (1.0 - local_t) + radii[i] * local_t;
                return Some((pos, r));
            }
        }
        // Past end : return last.
        Some((centers[centers.len() - 1], radii[radii.len() - 1]))
    };

    for (i, &bone_idx) in chain.bones.iter().enumerate() {
        let t = cumul_lengths[i + 1] / total_template_length;
        let linear_pos = attach_world + chain_direction_actual * (t * actual_chain_length);

        // Refinement medial axis CONTRAINT (2026-05-18 PM) : pour les bones
        // INTERMÉDIAIRES, snap à une sphère médiale qui reste SUR L'AXE de la
        // chaîne. Contraintes :
        // - Perpendiculaire à la ligne attach→terminal : < 15% chain length
        //   (sinon on saute sur un membre voisin).
        // - Along-chain offset depuis t_target : < 20% chain length
        //   (sinon on snap sur un autre segment de la chaîne).
        //
        // Évite que le coude/genou saute sur des sphères médiales d'autres
        // membres (problème observé runtime quand snap_radius était isotrope 25%).
        //
        // Le bone terminal garde la position terminal_sphere (déjà snappée).
        // Identifie l'axe centerline du bone :
        // - is_spine_xz : lock X ET Z au template (centrage strict spine)
        // - is_spine_x_only : lock X seul, Z libre (= head, doit entrer dans le mesh tête)
        // - is_leg_y_lock : lock Y au template (knee/shin précis à la fraction
        //   anatomique correcte), X/Z depuis path pour suivre la centerline.
        //   Évite que la distribution de sphères medial axis biaise shin vers le mollet.
        // Tail/Arm gardent le path complet (XYZ) pour suivre la courbure naturelle.
        // story-481 : locks YXZ déterminés par `bone.class` (déclarative TOML),
        // plus de substring matching `starts_with("thigh") || contains("spine")`.
        // Pattern AAA conforme — Unreal Skeleton / Unity Avatar / Godot
        // SkeletonProfile : la classe vit dans l'asset, pas dans le name.
        let bone_class = template.bones[bone_idx].class;
        let is_head = matches!(bone_class, BoneClass::Head);
        let is_spine_xz = matches!(bone_class, BoneClass::Spine);
        let is_spine_x_only = is_head;
        let is_leg_y_lock = matches!(bone_class, BoneClass::Leg);

        let (bone_world_pos, radius) = if i == n - 1 {
            // Terminal bone : prend la sphère terminale, mais si c'est un spine
            // bone override X/Z (ou X seul pour head) au template pour garder
            // l'alignement vertical strict du dos.
            if is_spine_xz {
                let template_pos = center_xz + template.bones[bone_idx].pos_vec3() * mesh_height;
                (
                    Vec3::new(template_pos.x, terminal_sphere.center.y, template_pos.z),
                    terminal_sphere.radius,
                )
            } else if is_spine_x_only {
                // Head : X centré, Y et Z depuis la sphère terminale (entre dans le mesh tête).
                let template_pos = center_xz + template.bones[bone_idx].pos_vec3() * mesh_height;
                (
                    Vec3::new(
                        template_pos.x,
                        terminal_sphere.center.y,
                        terminal_sphere.center.z,
                    ),
                    terminal_sphere.radius,
                )
            } else {
                (terminal_sphere.center, terminal_sphere.radius)
            }
        } else if let Some((path_pos, path_r)) = sample_path(t) {
            // Walk le medial axis : bone à la fraction t le long du graphe réel.
            // Beaucoup plus précis que la ligne droite pour membres courbés.
            if is_spine_xz {
                // Spine : Y depuis path (curvature anatomique), X/Z depuis
                // template rescalé (centrage strict, pas de dérive latérale).
                let template_pos = center_xz + template.bones[bone_idx].pos_vec3() * mesh_height;
                (
                    Vec3::new(template_pos.x, path_pos.y, template_pos.z),
                    path_r,
                )
            } else if is_spine_x_only {
                // Head intermediate (rare) : X centré, Y/Z depuis path.
                let template_pos = center_xz + template.bones[bone_idx].pos_vec3() * mesh_height;
                (Vec3::new(template_pos.x, path_pos.y, path_pos.z), path_r)
            } else if is_leg_y_lock {
                // Leg : Y depuis template (hauteur genou), X depuis path médial
                // (centerline latérale). Z = centerline médial + biais vers le
                // template (digitigrade lizard : genou en avant, shin z=0.12, cuisse
                // z=0, pied z=0.06). 2026-06-03 : pur path_pos.z mettait le genou trop
                // en ARRIÈRE (centerline) pour le raptor → on l'avance via le biais.
                // RÉGLER LEG_Z_FORWARD_BIAS pour avancer/reculer le ring genou
                // (0.0 = centerline pure, 1.0 = template forward digitigrade plein).
                const LEG_Z_FORWARD_BIAS: f32 = 0.6;
                let template_pos = center_xz + template.bones[bone_idx].pos_vec3() * mesh_height;
                let z_final = path_pos.z + LEG_Z_FORWARD_BIAS * (template_pos.z - path_pos.z);
                (
                    Vec3::new(path_pos.x, template_pos.y, z_final),
                    path_r,
                )
            } else {
                (path_pos, path_r)
            }
        } else if actual_chain_length > 1e-3 {
            // Spacing entre bones intermédiaires : 1/n. Pour empêcher un bone
            // de sauter sur le segment voisin, on limite l'offset à la MOITIÉ
            // du spacing. Ex: chaîne de 5 bones → spacing 0.2 → max_offset = 0.1.
            let segment_spacing = 1.0_f32 / n as f32;
            let max_along_offset = actual_chain_length * (segment_spacing * 0.5);
            let max_perp = actual_chain_length * 0.12;
            let target_along = t * actual_chain_length;

            let mut best: Option<(Vec3, f32, f32)> = None; // (center, radius, score)
            for s in &graph.spheres {
                // Exclude terminal sphere du snap intermédiaire : sinon le
                // cou/genou colle à la tête/pied.
                let to_terminal = (s.center - terminal_sphere.center).length_squared();
                if to_terminal < 1e-6 {
                    continue;
                }
                let rel = s.center - attach_world;
                let along = rel.dot(chain_direction_actual);
                let along_offset = (along - target_along).abs();
                if along_offset > max_along_offset {
                    continue;
                }
                let perp_vec = rel - chain_direction_actual * along;
                let perp = perp_vec.length();
                if perp > max_perp {
                    continue;
                }
                // Score : combine perp (priorité centrage) + along_offset (priorité bonne position).
                let score = perp + along_offset * 0.5;
                if best.is_none_or(|(_, _, s_old)| score < s_old) {
                    best = Some((s.center, s.radius, score));
                }
            }
            match best {
                Some((c, r, _)) => (c, r),
                None => {
                    // Pas de sphère propre : garde la position linéaire (sur axe chaîne).
                    let (nearest, _) = find_nearest_sphere(&graph.spheres, linear_pos);
                    (linear_pos, nearest.radius)
                }
            }
        } else {
            // Chain dégénérée : on garde la position linéaire.
            let (nearest, _) = find_nearest_sphere(&graph.spheres, linear_pos);
            (linear_pos, nearest.radius)
        };

        let tb = &template.bones[bone_idx];
        let snap_dist = bone_world_pos.distance(center_xz + tb.pos_vec3() * mesh_height);

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
    fn rescale_humanoid_matches_landmarks() {
        // Vérifie que `rescaled_for_landmarks` repositionne le head et le hip
        // exactement aux fractions demandées (précision anatomique).
        let t = SkeletonTemplate::humanoid();
        let rescaled = t.rescaled_for_landmarks(0.42, 0.67, 0.95, 0.50);

        let hip = rescaled.bones.iter().find(|b| b.name == "hip").unwrap();
        assert!(
            (hip.pos[1] - 0.42).abs() < 0.01,
            "hip y should be 0.42, got {}",
            hip.pos[1]
        );

        let head = rescaled.bones.iter().find(|b| b.name == "head").unwrap();
        assert!(
            (head.pos[1] - 0.95).abs() < 0.01,
            "head y should be 0.95, got {}",
            head.pos[1]
        );

        // Arm tip étendu : forearm_L pos.x devrait être négatif et plus large
        // (templat humanoid forearm_L x = -0.20 + 0.20 = mais en cumul depuis hip).
        // Le test direct : la valeur absolue max sur les bones arm doit s'approcher de arm_span_half_frac.
        let max_arm_x: f32 = rescaled
            .bones
            .iter()
            .filter(|b| b.name.contains("arm") || b.name.contains("hand"))
            .map(|b| b.pos[0].abs())
            .fold(0.0, f32::max);
        assert!(
            max_arm_x > 0.20,
            "arm scale should extend arm bones (got max |x|={})",
            max_arm_x
        );
    }

    #[test]
    fn rescale_lizard_keeps_tail_offset() {
        let t = SkeletonTemplate::biped_lizard();
        let rescaled = t.rescaled_for_landmarks(0.42, 0.67, 0.95, 0.40);
        // Tail bones doivent garder leur Z négatif (template) — pas écrasé en 0.
        let tail4 = rescaled.bones.iter().find(|b| b.name == "tail_04").unwrap();
        assert!(
            tail4.pos[2] < -0.30,
            "tail_04 z must stay extended back (got {})",
            tail4.pos[2]
        );
    }

    #[test]
    fn humanoid_template_has_20_bones() {
        // Story-480 : aligné sur skeleton_humanoid.toml (avec hand_L/R).
        let t = SkeletonTemplate::humanoid();
        assert_eq!(t.bones.len(), 20);
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
                class: BoneClass::Other,
            }],
            stance_offsets: Default::default(),
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
