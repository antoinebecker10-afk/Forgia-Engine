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
//! - Poids par distance point→**segment** d'os (envelope, story-A1) : un vertex
//!   reste assigné à son os sur toute sa longueur → le mesh suit la rotation
//!   même en pose extrême (bras 90°). distance-to-head capturait le haut du bras
//!   sur clavicle/chest (count_primary effondré). Heat-diffusion géodésique =
//!   Phase 2 (meilleur blending aux articulations multi-os).
//! - `SkinningConfig::max_verts_per_mesh` caps les meshes lourds (perf safety).
//! - Multi-Mesh3d : on traite tous les Mesh3d descendants, chacun avec ses propres
//!   ATTRIBUTE_JOINT_*. Tous partagent les mêmes bones.

use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::{Mesh, Mesh3d, VertexAttributeValues};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

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

/// Reconstruit les `GlobalTransform` (Mat4) de **rest pose** de tous les
/// descendants de `mesh_root`, en forçant la rotation LOCALE de chaque entité à
/// l'identité. Marche la hiérarchie via `q_children`.
///
/// Pourquoi : la inverse-bindpose et le matching de poids du skinning doivent
/// référencer la pose de REPOS du squelette (os à l'embed, rotation identité),
/// PAS la pose live. La locomotion (`forgia-anim-locomotion`) ne modifie QUE les
/// rotations locales des os (jamais les translations) ; si l'injection skinning
/// tourne pendant que la locomotion pose (race PostUpdate vs Update), lire le
/// `GlobalTransform` live capturerait une rest tordue → mesh déformé au retour au
/// bind (bug observé 2026-06-04, masqué jusque-là par le freeze forcé).
///
/// Invariant : les os spawnent en rotation identité (pinocchio `from_translation`,
/// sensor `bind_deg=[0,0,0]`). Si un futur template embarque une rotation de repos
/// non-nulle sur les os, ce helper devra capturer le bind à l'embed.
fn compute_rest_bone_globals(
    mesh_root: Entity,
    q_global: &Query<&GlobalTransform>,
    q_children: &Query<&Children>,
    q_transform: &Query<&Transform>,
) -> HashMap<Entity, Mat4> {
    let mut rest: HashMap<Entity, Mat4> = HashMap::new();
    let root_global = q_global
        .get(mesh_root)
        .map(|g| g.to_matrix())
        .unwrap_or(Mat4::IDENTITY);
    rest.insert(mesh_root, root_global);
    let mut stack = vec![mesh_root];
    while let Some(e) = stack.pop() {
        let parent_global = rest[&e];
        if let Ok(children) = q_children.get(e) {
            for child in children.iter() {
                let local = q_transform.get(child).copied().unwrap_or_default();
                // Rotation locale forcée à IDENTITÉ = pose de repos.
                let rest_local = Mat4::from_scale_rotation_translation(
                    local.scale,
                    Quat::IDENTITY,
                    local.translation,
                );
                let child_global = parent_global * rest_local;
                rest.insert(child, child_global);
                stack.push(child);
            }
        }
    }
    rest
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
    q_transform: Query<&Transform>,
    q_bones: Query<(Entity, &BoneEntity)>,
    q_names: Query<&Name>,
) {
    for (mesh_root, rigged) in &q_rigged {
        if rigged.bone_count == 0 {
            // Phase 1A a marqué AutoRigged sans bones (AABB dégénérée) — skip.
            commands
                .entity(mesh_root)
                .insert(SkinningInjected::default());
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
            commands
                .entity(mesh_root)
                .insert(SkinningInjected::default());
            continue;
        }

        // 2. Guard de propagation : si tous les bones sont à (0,0,0), le
        //    GlobalTransform n'est pas encore propagé → retry au prochain frame.
        let live_positions: Vec<Vec3> = bones
            .iter()
            .map(|&b| q_global.get(b).map(|gt| gt.translation()).unwrap_or(Vec3::ZERO))
            .collect();
        if live_positions.iter().all(|p| p.length_squared() < 1e-6) {
            // Retry attendu (pas de stats incrément).
            continue;
        }

        // 2b. Positions/globals de REST des os (rotation locale forcée identité).
        //     CRITIQUE (fix 2026-06-04) : la inverse-bindpose ET le matching de
        //     poids doivent référencer la pose de REPOS, PAS la pose live. La
        //     locomotion tourne en Update et pose les os AVANT que ce système
        //     (PostUpdate) lise leur GlobalTransform → sans freeze, lire le live
        //     capturerait une rest tordue (os parent tourné = enfants déplacés en
        //     FK) → mesh déformé au retour au bind. Le freeze forcé masquait ce
        //     bug. On reconstruit la rest pose, robuste au timing d'injection.
        //     Cf compute_rest_bone_globals + la locomotion ne change QUE les
        //     rotations locales (jamais les translations), donc les translations
        //     locales lues sont déjà celles du repos.
        let rest_globals =
            compute_rest_bone_globals(mesh_root, &q_global, &q_children, &q_transform);
        let bone_world_positions: Vec<Vec3> = bones
            .iter()
            .map(|&b| {
                rest_globals
                    .get(&b)
                    .map(|m| m.transform_point3(Vec3::ZERO))
                    .unwrap_or(Vec3::ZERO)
            })
            .collect();

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

        // Segments d'os [tête → bout] pour le skinning par distance-au-segment
        // (story-A1) : distance-to-head capturait les vertices du haut du bras
        // sur clavicle/chest (têtes proches) → le mesh ne suivait pas la rotation
        // de l'os (arm_L count_primary=312). Le bout = tête de l'os enfant le
        // plus éloigné (direction principale de l'os) ; os feuille (main/pied/
        // tête sans enfant) → segment dégénéré = point. Les os sont parentés
        // bone→bone (spawn_embedded_bones) donc q_children donne les enfants-os.
        let bone_idx: HashMap<Entity, usize> =
            bones.iter().enumerate().map(|(i, &e)| (e, i)).collect();
        // Carte os→parent (depuis q_children) pour extrapoler le bout des os feuilles.
        let mut parent_of: HashMap<usize, usize> = HashMap::new();
        for (i, &e) in bones.iter().enumerate() {
            if let Ok(children) = q_children.get(e) {
                for child in children.iter() {
                    if let Some(&ci) = bone_idx.get(&child) {
                        parent_of.insert(ci, i);
                    }
                }
            }
        }
        // 2026-06-30 (suite story-637) — os FEUILLE (main/pied/tête, sans os-enfant) :
        // sans extrapolation, `tip == head` → segment dégénéré (point au joint) → le
        // VOLUME de l'extrémité est capturé par le parent (forearm/shin/neck) →
        // `count_primary = 0` (capteur forgia_skinning_weights : head/hand/foot morts,
        // l'extrémité ne se déforme pas). Fix : on extrapole le bout le long de la
        // direction parent→os, d'une fraction de la longueur de l'os parent → l'os
        // feuille SPAN son extrémité et capture ses vertices. (Le pied pointe vers
        // l'avant alors que son os pointe vers le bas → capture partielle du talon ;
        // suffisant pour le réveiller, raffinement avant/toe = follow-up.)
        const LEAF_BONE_EXTENSION_RATIO: f32 = 1.0;
        let bone_segments_local: Vec<(Vec3, Vec3)> = bones
            .iter()
            .enumerate()
            .map(|(i, &e)| {
                let head = bone_positions_local[i];
                let mut tip = head;
                let mut best_d2 = 0.0_f32;
                if let Ok(children) = q_children.get(e) {
                    for child in children.iter() {
                        if let Some(&ci) = bone_idx.get(&child) {
                            let cpos = bone_positions_local[ci];
                            let d2 = (cpos - head).length_squared();
                            if d2 > best_d2 {
                                best_d2 = d2;
                                tip = cpos;
                            }
                        }
                    }
                }
                // Os feuille (aucun os-enfant → tip resté == head) : extrapole.
                if best_d2 < 1e-8 {
                    if let Some(&pi) = parent_of.get(&i) {
                        let dir = head - bone_positions_local[pi];
                        if dir.length_squared() > 1e-10 {
                            tip = head + dir * LEAF_BONE_EXTENSION_RATIO;
                        }
                    }
                }
                (head, tip)
            })
            .collect();

        // Classe skin par os (anti-déformation tête) : un os de bras n'influence
        // pas les vertices dont l'os le plus proche est tête/nuque. Index-aligné
        // avec `bones` (donc avec joints[] du SkinnedMesh).
        let bone_kinds: Vec<BoneSkinKind> = bones
            .iter()
            .map(|&e| {
                q_names
                    .get(e)
                    .map(|n| classify_bone_skin_kind(n.as_str()))
                    .unwrap_or_default()
            })
            .collect();

        // 3. Walk descendants → trouve les Mesh3d entities + leur GlobalTransform.
        let mut mesh3d_entities: Vec<Entity> = Vec::new();
        collect_mesh3d_descendants(mesh_root, &q_children, &q_mesh3d, &mut mesh3d_entities);

        if mesh3d_entities.is_empty() {
            // Aucun Mesh3d : peut arriver si le SceneRoot n'a pas encore instancié
            // ses meshes. Retry au prochain frame.
            continue;
        }

        // 4. Bones globaux au bind pose = pose de REST (rotation os = identité),
        //    PAS la pose live (cf 2b : la locomotion a pu poser les os ce frame).
        //    Mat4 complets translation+rotation+scale.
        let bone_globals: Vec<Mat4> = bones
            .iter()
            .map(|&b| {
                rest_globals
                    .get(&b)
                    .copied()
                    .unwrap_or_else(|| q_global.get(b).copied().unwrap_or_default().to_matrix())
            })
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
            let (joint_indices, joint_weights) = compute_nearest_bone_weights(
                &positions,
                &mesh3d_to_root_local,
                &bone_segments_local,
                &bone_kinds,
                &config,
            );

            // Story-482 P3+ — Diagnostic sensor : compte la distribution des
            // poids par bone pour exposer le bug "vertices arm assignés à
            // chest" (bone segment vs bone head distance). Écrit
            // forgia2_skinning_weights.json côte à côte de l'injection.
            write_skinning_weights_sensor(
                &bones,
                &q_names,
                &joint_indices,
                &joint_weights,
                n_verts,
            );

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
            mesh_root,
            meshes_processed,
            total_verts_processed,
            bones.len()
        );
    }
}

/// Story-482 — Sensor diagnostic skinning weights.
///
/// Pour chaque bone du rig, compte :
/// - `count_primary` : nombre de vertices où ce bone est le top-1 (slot[0])
/// - `count_any` : nombre de vertices avec poids non-nul (slots 0-3)
/// - `mean_weight` : poids moyen quand bone est présent
/// - `max_weight` : poids max obtenu
///
/// Permet de voir empiriquement si certains bones sont sous-utilisés
/// (count_primary ≈ 0 → vertices vont ailleurs). Cas typique Forgia :
/// `arm_L` count_primary=0 si nearest-bone-head met les arm vertices sur
/// chest (bone segment vs head distance issue).
fn write_skinning_weights_sensor(
    bones: &[Entity],
    q_names: &Query<&Name>,
    joint_indices: &[[u16; 4]],
    joint_weights: &[[f32; 4]],
    n_verts: usize,
) {
    #[derive(Default, Clone)]
    struct Per {
        count_primary: u32,
        count_any: u32,
        sum_weight: f32,
        max_weight: f32,
    }
    let mut per_bone: Vec<Per> = vec![Per::default(); bones.len()];
    for (idx_row, w_row) in joint_indices.iter().zip(joint_weights.iter()) {
        // Top-1 (slot 0, déjà sorted asc dans compute_nearest_bone_weights)
        let bi_primary = idx_row[0] as usize;
        if w_row[0] > 0.0 && bi_primary < per_bone.len() {
            per_bone[bi_primary].count_primary += 1;
        }
        // Tous slots
        for slot in 0..4 {
            let bi = idx_row[slot] as usize;
            let w = w_row[slot];
            if w > 0.0 && bi < per_bone.len() {
                per_bone[bi].count_any += 1;
                per_bone[bi].sum_weight += w;
                if w > per_bone[bi].max_weight {
                    per_bone[bi].max_weight = w;
                }
            }
        }
    }

    let names: Vec<String> = bones
        .iter()
        .map(|&e| {
            q_names
                .get(e)
                .map(|n| n.to_string())
                .unwrap_or_else(|_| format!("bone_{:?}", e))
        })
        .collect();

    // Build per-bone JSON array
    let mut entries: Vec<String> = Vec::with_capacity(per_bone.len());
    for (i, p) in per_bone.iter().enumerate() {
        let mean_w = if p.count_any > 0 {
            p.sum_weight / p.count_any as f32
        } else {
            0.0
        };
        let name = names.get(i).cloned().unwrap_or_default();
        entries.push(format!(
            "    {{\"index\":{},\"name\":\"{}\",\"count_primary\":{},\"count_any\":{},\"mean_weight\":{:.4},\"max_weight\":{:.4}}}",
            i, name, p.count_primary, p.count_any, mean_w, p.max_weight
        ));
    }

    let json = format!(
        "{{\n  \"n_verts\": {},\n  \"n_bones\": {},\n  \"bones\": [\n{}\n  ],\n  \"diagnostic_hint\": \"count_primary=0 sur un bone qui devrait porter du mesh = vertices captures par un voisin (chest/spine si arm est short). Indique besoin de distance-to-segment au lieu de distance-to-head.\"\n}}\n",
        n_verts,
        bones.len(),
        entries.join(",\n"),
    );
    let _ = std::fs::write("forgia_skinning_weights.json", json);
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

/// Distance² d'un point `p` au segment `[a, b]` (clamp aux extrémités). Segment
/// dégénéré (`a ≈ b`, os feuille sans enfant) → distance² au point `a`.
#[inline]
fn point_to_segment_dist_sq(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-12 {
        return (p - a).length_squared();
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    (p - (a + ab * t)).length_squared()
}

/// Classe d'os pour le skinning anti-déformation tête. Permet d'empêcher les os
/// de bras d'influencer les vertices de la région tête : ainsi une rotation de
/// stance de la clavicule (A-pose) ne drague JAMAIS les écailles du crâne/nuque
/// (régression 2026-06-05, clavicle_L influençait ~5887 verts dont des écailles).
#[derive(Clone, Copy, Default)]
struct BoneSkinKind {
    /// Os de bras (clavicule, bras, avant-bras, main) — exclu des verts tête ET torse.
    is_arm: bool,
    /// Os de tête/nuque — un vertex dont l'os le plus proche est `is_head`
    /// définit la "région tête" protégée (exclut les os de bras).
    is_head: bool,
    /// Os de torse (spine/chest/hip/pelvis) — un vertex dont l'os le plus proche
    /// est `is_body` définit la "région torse" protégée : exclut les os de bras
    /// pour que le balancement du bras / la protraction de la clavicule ne
    /// **compriment PAS la peau du buste**. Symétrique de `is_head`.
    ///
    /// 2026-06-06 : compression « arm-driven » confirmée (user + sensor d'os
    /// stable `torso_y_range≈0.9mm` → la colonne ne bouge pas, donc 100 %
    /// skinning). La frontière deltoïde/torse est naturelle : un vert d'épaule
    /// (proche=clavicule/bras) suit toujours le bras ; seul le torse-propre
    /// (proche=spine) devient immunisé. Permet de resserrer les bras (coudes
    /// collés) SANS re-déformer — découple largeur d'os et déformation.
    is_body: bool,
}

/// Classe un os par sous-chaîne de son nom (aligné conventions templates Forgia
/// humanoid/biped_lizard : clavicle_L/R, arm_L/R, forearm_L/R, hand_L/R, neck, head).
fn classify_bone_skin_kind(name: &str) -> BoneSkinKind {
    let n = name.to_ascii_lowercase();
    BoneSkinKind {
        is_arm: n.contains("arm") || n.contains("clavicle") || n.contains("hand"),
        is_head: n.contains("head") || n.contains("neck") || n.contains("skull"),
        is_body: n.contains("spine")
            || n.contains("chest")
            || n.contains("hip")
            || n.contains("torso")
            || n.contains("pelvis"),
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
    bone_segments_local: &[(Vec3, Vec3)],
    bone_kinds: &[BoneSkinKind],
    config: &SkinningConfig,
) -> (Vec<[u16; 4]>, Vec<[f32; 4]>) {
    let k = (config.bones_per_vertex as usize).min(4);
    let n_bones = bone_segments_local.len();
    let mut indices: Vec<[u16; 4]> = Vec::with_capacity(positions.len());
    let mut weights: Vec<[f32; 4]> = Vec::with_capacity(positions.len());
    // Buffer distances² réutilisé par vertex (skinning = one-shot par rig, mais on
    // évite une alloc par vertex). Une seule passe de distance par vertex.
    let mut d2_buf = vec![0.0_f32; n_bones];

    for p in positions {
        let v_root = mesh3d_to_root_local.transform_point3(Vec3::from_array(*p));

        // Distance² au segment de chaque os (squared pour éviter sqrt + diviser
        // après). distance-au-segment (pas à la tête) : un vertex au milieu d'un
        // os long lui reste assigné (story-A1, fix arm count_primary effondré).
        // On note au passage l'os le plus proche (pour la règle anti-tête).
        let mut nearest_bi = 0usize;
        let mut nearest_d2 = f32::INFINITY;
        for (bi, seg) in bone_segments_local.iter().enumerate() {
            let d2 = point_to_segment_dist_sq(v_root, seg.0, seg.1);
            d2_buf[bi] = d2;
            if d2 < nearest_d2 {
                nearest_d2 = d2;
                nearest_bi = bi;
            }
        }

        // Compartimentage par région (anti-déformation tête / torse / bras). Le vertex
        // appartient à la région de son os le plus proche ; on EXCLUT de ses poids les
        // os des régions INCOMPATIBLES. Régions : tête/nuque (`is_head`), torse/colonne
        // (`is_body`), bras (`is_arm`) ; jambes/queue = neutres (jamais exclues).
        //  - vert TÊTE  : exclut les os de bras (clavicule A-pose ne drague pas le crâne,
        //    régression 2026-06-05).
        //  - vert TORSE : exclut bras (compression arm-driven 2026-06-06) ET tête/nuque
        //    (la nuque sur-influençait 8936 verts / 26% du mesh, dont l'épaule/haut-torse
        //    → léger pli au cou quand le buste/bras bougent, 2026-06-07).
        //  - vert BRAS  : exclut tête/nuque (l'épaule ne suit plus la nuque).
        // Effet : nuque/tête localisées à la région tête. La frontière reste lisse car
        // les verts tête blendent encore avec le chest (is_body non exclu côté tête).
        let nk = bone_kinds.get(nearest_bi).copied().unwrap_or_default();
        let exclude_arm = nk.is_head || nk.is_body;
        let exclude_head = nk.is_body || nk.is_arm;

        // Sélection top-K par distance (avec exclusion conditionnelle des bras).
        // K petit (≤4) → partial selection plus rapide qu'un sort complet O(n log n).
        let mut top: Vec<(usize, f32)> = Vec::with_capacity(k);
        for (bi, &d2) in d2_buf.iter().enumerate() {
            let kd = bone_kinds.get(bi).copied().unwrap_or_default();
            if exclude_arm && kd.is_arm {
                continue;
            }
            if exclude_head && kd.is_head {
                continue;
            }
            if top.len() < k {
                top.push((bi, d2));
                if top.len() == k {
                    top.sort_unstable_by(|a, b| {
                        a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
            } else if d2 < top[k - 1].1 {
                top[k - 1] = (bi, d2);
                // Re-sort la tail (k≤4 → coût constant).
                top.sort_unstable_by(|a, b| {
                    a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                });
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
            [
                1.0 / k as f32,
                1.0 / k as f32,
                1.0 / k as f32,
                1.0 / k as f32,
            ]
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

    /// Helper : têtes d'os → segments dégénérés (`head == tip`) pour réutiliser
    /// les tests historiques basés sur distance-au-point (story-A1).
    fn pts(heads: &[Vec3]) -> Vec<(Vec3, Vec3)> {
        heads.iter().map(|&h| (h, h)).collect()
    }

    /// Classes "neutres" (ni bras ni tête) pour les tests historiques qui ne
    /// testent pas l'exclusion anti-tête (comportement = aucun filtrage).
    fn neutral_kinds(n: usize) -> Vec<BoneSkinKind> {
        vec![BoneSkinKind::default(); n]
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

    /// Fix 2026-06-04 : la rest pose du skinning doit IGNORER la rotation locale
    /// des os (la locomotion peut les avoir posés avant l'injection). Un os enfant
    /// avec une rotation locale non triviale doit produire un rest global à
    /// rotation identité (translation pure héritée du parent).
    #[test]
    fn rest_globals_ignore_bone_rotation() {
        use bevy::ecs::system::SystemState;
        let mut world = World::new();
        let root = world
            .spawn((Transform::IDENTITY, GlobalTransform::IDENTITY))
            .id();
        // Os enfant : translation + rotation locale 90° Z (pose, à ignorer).
        let child = world
            .spawn((
                Transform::from_translation(Vec3::new(0.0, 1.0, 0.0))
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                GlobalTransform::IDENTITY,
            ))
            .id();
        world.entity_mut(root).add_child(child);

        let mut state: SystemState<(
            Query<&GlobalTransform>,
            Query<&Children>,
            Query<&Transform>,
        )> = SystemState::new(&mut world);
        let (q_global, q_children, q_transform) = state.get(&world);
        let rest = compute_rest_bone_globals(root, &q_global, &q_children, &q_transform);

        let child_rest = rest.get(&child).expect("child rest global présent");
        let (scale, rot, trans) = child_rest.to_scale_rotation_translation();
        assert!(
            (trans - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-5,
            "translation = translation locale (rest), got {trans:?}"
        );
        assert!(
            rot.angle_between(Quat::IDENTITY) < 1e-4,
            "rotation doit être identité (rest, rotation d'os ignorée), got {rot:?}"
        );
        assert!((scale - Vec3::ONE).length() < 1e-5, "scale 1");
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
        let (_, weights) =
            compute_nearest_bone_weights(&positions, &Mat4::IDENTITY, &pts(&bones), &neutral_kinds(bones.len()), &cfg());
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
        let (indices, weights) =
            compute_nearest_bone_weights(&positions, &Mat4::IDENTITY, &pts(&bones), &neutral_kinds(bones.len()), &cfg());
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
            compute_nearest_bone_weights(&positions, &Mat4::IDENTITY, &pts(&bones), &neutral_kinds(bones.len()), &cfg());
        assert_eq!(indices[0][0], 0);
        let sum: f32 = weights[0].iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-4 || sum.abs() < 1e-4,
            "got sum={}",
            sum
        );
    }

    #[test]
    fn weights_indices_have_correct_length() {
        let positions = vec![[0.0, 0.0, 0.0]; 100];
        let bones = vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z, Vec3::NEG_X];
        let (indices, weights) =
            compute_nearest_bone_weights(&positions, &Mat4::IDENTITY, &pts(&bones), &neutral_kinds(bones.len()), &cfg());
        assert_eq!(indices.len(), 100);
        assert_eq!(weights.len(), 100);
    }

    /// Story-A1 — distance-to-segment : un vertex au MILIEU d'un os long doit
    /// s'assigner à cet os, même si la TÊTE d'un petit os voisin est plus proche.
    /// Régression du bug "arm count_primary=312" (vertices du bras capturés par
    /// clavicle/chest car distance-to-head). Échoue avec l'ancienne formule.
    #[test]
    fn segment_distance_beats_head_distance_for_long_bone() {
        // bone A : os long [(0,0,0) → (0,0,4)]. bone B : petit os dont la tête
        // est près du MILIEU de A (0.3, 0, 2). Vertex au milieu de A.
        let positions = vec![[0.0, 0.0, 2.0]];
        let segments = vec![
            (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 4.0)),
            (Vec3::new(0.3, 0.0, 2.0), Vec3::new(0.3, 0.0, 2.1)),
        ];
        let (indices, weights) = compute_nearest_bone_weights(
            &positions,
            &Mat4::IDENTITY,
            &segments,
            &neutral_kinds(segments.len()),
            &cfg(),
        );
        assert_eq!(
            indices[0][0], 0,
            "vertex au milieu de l'os long A doit s'assigner à A (segment), pas B (tête proche)"
        );
        assert!(
            weights[0][0] > weights[0][1],
            "A doit dominer B : got A={}, B={}",
            weights[0][0],
            weights[0][1]
        );
    }

    /// Anti-déformation tête (2026-06-05) : un vertex dont l'os le plus proche est
    /// tête/nuque ne doit recevoir AUCUN poids d'un os de bras (clavicule), même
    /// si la clavicule est géométriquement proche. Garantit qu'une rotation de
    /// stance de la clavicule (A-pose) ne déforme jamais les écailles de la tête.
    #[test]
    fn head_region_vertex_excludes_arm_bones() {
        // Vertex juste sous la tête. head = le plus proche ; clavicle (arm) est
        // 2e plus proche → sans exclusion elle capterait le vertex.
        let positions = vec![[0.0, 0.05, 0.0]];
        let segments = vec![
            (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0)), // 0 head (le + proche)
            (Vec3::new(0.1, 0.0, 0.0), Vec3::new(0.2, 0.0, 0.0)), // 1 clavicle (arm, proche)
            (Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.0, -0.5, 0.0)), // 2 chest
        ];
        let kinds = vec![
            BoneSkinKind { is_arm: false, is_head: true, is_body: false },
            BoneSkinKind { is_arm: true, is_head: false, is_body: false },
            BoneSkinKind { is_arm: false, is_head: false, is_body: false },
        ];
        let (indices, _) =
            compute_nearest_bone_weights(&positions, &Mat4::IDENTITY, &segments, &kinds, &cfg());
        assert!(
            !indices[0].contains(&1),
            "clavicle (arm, index 1) ne doit apparaître dans AUCUN slot d'un vertex tête, got {:?}",
            indices[0]
        );
        assert_eq!(indices[0][0], 0, "head doit dominer le vertex tête");
        // Contre-preuve : SANS exclusion (kinds neutres), la clavicle DOIT entrer.
        let (indices_no_excl, _) = compute_nearest_bone_weights(
            &positions,
            &Mat4::IDENTITY,
            &segments,
            &neutral_kinds(segments.len()),
            &cfg(),
        );
        assert!(
            indices_no_excl[0].contains(&1),
            "sans exclusion, la clavicle proche DOIT capter le vertex (sinon le test ne prouve rien)"
        );
    }

    /// Symétrique du test tête : un vertex de la **région torse** (os le plus
    /// proche = chest, `is_body`) doit EXCLURE les os de bras. Garantit que le
    /// balancement du bras ne comprime pas la peau du buste (2026-06-06).
    #[test]
    fn body_region_vertex_excludes_arm_bones() {
        // Vertex sur le flanc du buste : chest le plus proche (dist 0.04) ; arm
        // (bras) 2e très proche (dist 0.06) → sans exclusion il capterait le
        // vertex et le dragguerait au swing du bras.
        let positions = vec![[0.04, 0.0, 0.0]];
        let segments = vec![
            (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0)), // 0 chest (le + proche, 0.04)
            (Vec3::new(0.10, 0.0, 0.0), Vec3::new(0.30, 0.0, 0.0)), // 1 arm (2e proche, 0.06)
            (Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.0, -0.5, 0.0)), // 2 hip (body, loin)
        ];
        let kinds = vec![
            BoneSkinKind { is_arm: false, is_head: false, is_body: true },
            BoneSkinKind { is_arm: true, is_head: false, is_body: false },
            BoneSkinKind { is_arm: false, is_head: false, is_body: true },
        ];
        let (indices, _) =
            compute_nearest_bone_weights(&positions, &Mat4::IDENTITY, &segments, &kinds, &cfg());
        assert!(
            !indices[0].contains(&1),
            "arm (index 1) ne doit apparaître dans AUCUN slot d'un vertex torse, got {:?}",
            indices[0]
        );
        assert_eq!(indices[0][0], 0, "chest doit dominer le vertex torse");
        // Contre-preuve : sans exclusion (kinds neutres), le bras DOIT entrer.
        let (indices_no_excl, _) = compute_nearest_bone_weights(
            &positions,
            &Mat4::IDENTITY,
            &segments,
            &neutral_kinds(segments.len()),
            &cfg(),
        );
        assert!(
            indices_no_excl[0].contains(&1),
            "sans exclusion, le bras proche DOIT capter le vertex torse (sinon le test ne prouve rien)"
        );
    }

    /// Compartimentage 2026-06-07 : un vertex de torse (proche=chest, is_body) doit
    /// AUSSI exclure les os tête/nuque (`is_head`) — la nuque sur-influençait le
    /// haut-torse/épaule → léger pli au cou. Localise nuque/tête à la région tête.
    #[test]
    fn body_region_vertex_excludes_head_bones() {
        // Vertex de haut-torse : chest le plus proche (0.04) ; neck (is_head) 2e
        // proche (0.06). Sans exclusion la nuque le capterait et le dragguerait.
        let positions = vec![[0.04, 0.0, 0.0]];
        let segments = vec![
            (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0)), // 0 chest (proche, 0.04)
            (Vec3::new(0.10, 0.0, 0.0), Vec3::new(0.30, 0.0, 0.0)), // 1 neck (is_head, 2e, 0.06)
            (Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.0, -0.5, 0.0)), // 2 hip (body, loin)
        ];
        let kinds = vec![
            BoneSkinKind { is_arm: false, is_head: false, is_body: true },
            BoneSkinKind { is_arm: false, is_head: true, is_body: false },
            BoneSkinKind { is_arm: false, is_head: false, is_body: true },
        ];
        let (indices, _) =
            compute_nearest_bone_weights(&positions, &Mat4::IDENTITY, &segments, &kinds, &cfg());
        assert!(
            !indices[0].contains(&1),
            "neck (is_head, index 1) ne doit PAS capter un vertex torse, got {:?}",
            indices[0]
        );
        assert_eq!(indices[0][0], 0, "chest doit dominer le vertex torse");
        // Contre-preuve : sans exclusion, la nuque proche DOIT entrer.
        let (no_excl, _) = compute_nearest_bone_weights(
            &positions,
            &Mat4::IDENTITY,
            &segments,
            &neutral_kinds(segments.len()),
            &cfg(),
        );
        assert!(
            no_excl[0].contains(&1),
            "sans exclusion, la nuque proche DOIT capter le vertex (sinon le test ne prouve rien)"
        );
    }
}
