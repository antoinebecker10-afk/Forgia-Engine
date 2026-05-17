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

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use forgia_core::prelude::GameSet;
use std::collections::HashMap;

mod anatomy_detect;
mod skinning;
mod debug_gizmos;
mod pinocchio_pipeline;

pub use anatomy_detect::{detect_landmarks_from_vertices, MeshLandmarks};
pub use skinning::{
    inject_skinning_for_rigged_meshes, BoneEntity, SkinningConfig, SkinningInjected,
};
pub use debug_gizmos::{draw_rig_gizmos, AutoRigGizmosConfig};
pub use pinocchio_pipeline::{
    auto_rig_pinocchio_v1, load_skeleton_genomes, AutoRigBackend,
    SkeletonBipedLizardGenomeHandle, SkeletonHumanoidGenomeHandle,
};

pub mod prelude {
    pub use crate::{
        AutoRigBackend, AutoRigGizmosConfig, AutoRigStats, AutoRigTemplate, AutoRigged, BoneDef,
        BoneEntity, ForgiaAutoRigPlugin, MeshLandmarks, NeedsAutoRig, PlacedBone, SkinningInjected,
    };
}

// ── Sensor / observability ──────────────────────────────────────────────────

/// Intervalle d'écriture `forgia_auto_rig.json` (secondes).
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

/// Morphologie de référence. Phase 1A : 2 templates hardcodés. Phase 1C :
/// templates en TOML hot-reload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoRigTemplate {
    /// 18 bones bipède Vitruvien (humain, gobelin, orc, nain, céleste).
    Humanoid,
    /// 18 bones bipède lézard avec tail 4-segments (Rex).
    BipedLizard,
}

impl AutoRigTemplate {
    pub fn bones(self) -> &'static [BoneDef] {
        match self {
            Self::Humanoid => HUMANOID_BONES,
            Self::BipedLizard => BIPED_LIZARD_BONES,
        }
    }
}

/// Définition d'un bone dans un template. Coordonnées normalisées à `height=1.0`
/// (= sera multiplié par `mesh_height` au fit). `local_translation` est local
/// au parent dans la hiérarchie.
#[derive(Debug, Clone, Copy)]
pub struct BoneDef {
    pub name: &'static str,
    /// `None` = root bone (enfant direct du mesh root).
    pub parent: Option<&'static str>,
    /// Position locale relative au parent, normalisée height=1.0.
    pub local_translation: Vec3,
}

/// Résultat d'un placement (utilisé par tests + Phase 1B skinning).
#[derive(Debug, Clone)]
pub struct PlacedBone {
    pub name: &'static str,
    pub parent: Option<&'static str>,
    /// Position locale au parent en unités mesh (= déjà scaled).
    pub local_translation: Vec3,
    /// Position monde relative au mesh root (= accumulée BFS).
    pub world_pos: Vec3,
}

// ── Humanoid template (18 bones) ────────────────────────────────────────────
//
// Convention coords : Y vertical, mesh.bottom = 0, mesh.top = 1.0, X latéral
// (positif = droite), Z profondeur (positif = avant).
//
// Noms compatibles avec `forgia-rig-topology::name_boost` :
// - "thigh"/"hip" → leg | "arm"/"clavicle" → arm | "spine"/"chest" → spine
// - "head"/"neck" → head

const HUMANOID_BONES: &[BoneDef] = &[
    // Root : hip à 55% de la hauteur (proportions Vitruvian).
    BoneDef { name: "hip",          parent: None,                local_translation: Vec3::new(0.0, 0.55, 0.0) },

    // Spine chain (centrale, axe Y).
    BoneDef { name: "spine_lower",  parent: Some("hip"),         local_translation: Vec3::new(0.0, 0.08, 0.0) },
    BoneDef { name: "spine_mid",    parent: Some("spine_lower"), local_translation: Vec3::new(0.0, 0.10, 0.0) },
    BoneDef { name: "chest",        parent: Some("spine_mid"),   local_translation: Vec3::new(0.0, 0.10, 0.0) },
    BoneDef { name: "neck",         parent: Some("chest"),       local_translation: Vec3::new(0.0, 0.07, 0.0) },
    BoneDef { name: "head",         parent: Some("neck"),        local_translation: Vec3::new(0.0, 0.07, 0.0) },

    // Bras gauche (chest → clavicle_L → arm_L → forearm_L).
    BoneDef { name: "clavicle_L",   parent: Some("chest"),       local_translation: Vec3::new(-0.10, 0.05, 0.0) },
    BoneDef { name: "arm_L",        parent: Some("clavicle_L"),  local_translation: Vec3::new(-0.10, -0.02, 0.0) },
    BoneDef { name: "forearm_L",    parent: Some("arm_L"),       local_translation: Vec3::new(-0.02, -0.20, 0.0) },

    // Bras droit (symétrique).
    BoneDef { name: "clavicle_R",   parent: Some("chest"),       local_translation: Vec3::new(0.10, 0.05, 0.0) },
    BoneDef { name: "arm_R",        parent: Some("clavicle_R"),  local_translation: Vec3::new(0.10, -0.02, 0.0) },
    BoneDef { name: "forearm_R",    parent: Some("arm_R"),       local_translation: Vec3::new(0.02, -0.20, 0.0) },

    // Jambe gauche (hip → thigh_L → shin_L → foot_L).
    BoneDef { name: "thigh_L",      parent: Some("hip"),         local_translation: Vec3::new(-0.10, -0.08, 0.0) },
    BoneDef { name: "shin_L",       parent: Some("thigh_L"),     local_translation: Vec3::new(0.0, -0.25, 0.0) },
    BoneDef { name: "foot_L",       parent: Some("shin_L"),      local_translation: Vec3::new(0.0, -0.22, 0.05) },

    // Jambe droite (symétrique).
    BoneDef { name: "thigh_R",      parent: Some("hip"),         local_translation: Vec3::new(0.10, -0.08, 0.0) },
    BoneDef { name: "shin_R",       parent: Some("thigh_R"),     local_translation: Vec3::new(0.0, -0.25, 0.0) },
    BoneDef { name: "foot_R",       parent: Some("shin_R"),      local_translation: Vec3::new(0.0, -0.22, 0.05) },
];

// ── BipedLizard template (14 + 4 tail = 18 bones) ───────────────────────────
//
// Rex / lézard : hip plus bas (40% hauteur, posture penchée), bras plus courts,
// tail chain linéaire 4 segments derrière (-Z = arrière).

const BIPED_LIZARD_BONES: &[BoneDef] = &[
    BoneDef { name: "hip",          parent: None,                local_translation: Vec3::new(0.0, 0.45, 0.0) },

    // Spine penchée vers l'avant (+Z = avant ; le mesh face -Z mais on rote au spawn).
    BoneDef { name: "spine_lower",  parent: Some("hip"),         local_translation: Vec3::new(0.0, 0.08, 0.04) },
    BoneDef { name: "spine_mid",    parent: Some("spine_lower"), local_translation: Vec3::new(0.0, 0.10, 0.06) },
    BoneDef { name: "chest",        parent: Some("spine_mid"),   local_translation: Vec3::new(0.0, 0.08, 0.06) },
    BoneDef { name: "neck",         parent: Some("chest"),       local_translation: Vec3::new(0.0, 0.05, 0.08) },
    BoneDef { name: "head",         parent: Some("neck"),        local_translation: Vec3::new(0.0, 0.06, 0.05) },

    // Bras courts (lézard biped façon raptor).
    BoneDef { name: "arm_L",        parent: Some("chest"),       local_translation: Vec3::new(-0.12, 0.0, 0.02) },
    BoneDef { name: "forearm_L",    parent: Some("arm_L"),       local_translation: Vec3::new(-0.04, -0.12, 0.04) },
    BoneDef { name: "arm_R",        parent: Some("chest"),       local_translation: Vec3::new(0.12, 0.0, 0.02) },
    BoneDef { name: "forearm_R",    parent: Some("arm_R"),       local_translation: Vec3::new(0.04, -0.12, 0.04) },

    // Jambes (équivalent humanoid mais hip plus bas → jambes plus courtes en absolu).
    BoneDef { name: "thigh_L",      parent: Some("hip"),         local_translation: Vec3::new(-0.10, -0.08, 0.0) },
    BoneDef { name: "shin_L",       parent: Some("thigh_L"),     local_translation: Vec3::new(0.0, -0.18, 0.0) },
    BoneDef { name: "foot_L",       parent: Some("shin_L"),      local_translation: Vec3::new(0.0, -0.15, 0.06) },
    BoneDef { name: "thigh_R",      parent: Some("hip"),         local_translation: Vec3::new(0.10, -0.08, 0.0) },
    BoneDef { name: "shin_R",       parent: Some("thigh_R"),     local_translation: Vec3::new(0.0, -0.18, 0.0) },
    BoneDef { name: "foot_R",       parent: Some("shin_R"),      local_translation: Vec3::new(0.0, -0.15, 0.06) },

    // Tail : chaîne linéaire 4 segments en -Z (arrière), légèrement descendante.
    BoneDef { name: "tail_01",      parent: Some("hip"),         local_translation: Vec3::new(0.0, -0.04, -0.12) },
    BoneDef { name: "tail_02",      parent: Some("tail_01"),     local_translation: Vec3::new(0.0, -0.02, -0.10) },
    BoneDef { name: "tail_03",      parent: Some("tail_02"),     local_translation: Vec3::new(0.0, -0.02, -0.10) },
    BoneDef { name: "tail_04",      parent: Some("tail_03"),     local_translation: Vec3::new(0.0, -0.02, -0.10) },
];

// ── Placement algorithm (pure — testable headless sans World) ───────────────

/// Vérifie que chaque BoneDef référence un parent déclaré AVANT lui dans la
/// slice (invariant BFS implicite). Retourne `Err(message)` au premier
/// manquement. Appelé par `place_template` et exposé pour les tests Phase 1C
/// (TOML genome) qui doivent valider les templates user-supplied.
pub fn validate_template(bones: &[BoneDef]) -> Result<(), String> {
    use std::collections::HashSet;
    let mut declared: HashSet<&'static str> = HashSet::with_capacity(bones.len());
    for (idx, def) in bones.iter().enumerate() {
        if let Some(parent_name) = def.parent {
            if !declared.contains(parent_name) {
                return Err(format!(
                    "BoneDef[{}] '{}' references parent '{}' that is not declared before it",
                    idx, def.name, parent_name
                ));
            }
        }
        declared.insert(def.name);
    }
    Ok(())
}

/// Classification d'un bone par son nom — détermine quelle scale appliquer
/// dans `place_template_with_landmarks`. Aligné avec les naming conventions
/// des templates Humanoid + BipedLizard (cf `forgia-rig-topology` name_boost).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoneChain {
    /// hip, spine_*, chest, neck, head — chaîne verticale au-dessus du hip
    Spine,
    /// thigh, shin, foot — chaîne descendante depuis hip jusqu'au sol
    Leg,
    /// clavicle, arm, forearm, hand — chaîne latérale depuis chest
    Arm,
    /// tail, etc. — pas de scale particulier
    Other,
}

fn classify_bone_chain(name: &str) -> BoneChain {
    let n = name.to_ascii_lowercase();
    if n.contains("thigh") || n.contains("shin") || n.contains("foot") {
        BoneChain::Leg
    } else if n.contains("clavicle")
        || n.contains("forearm")
        || n.contains("hand")
        || n == "arm_l"
        || n == "arm_r"
        || n.starts_with("arm_")
    {
        BoneChain::Arm
    } else if n.contains("spine")
        || n.contains("chest")
        || n.contains("neck")
        || n.contains("head")
        || n.contains("hip")
        || n.contains("torso")
    {
        BoneChain::Spine
    } else {
        BoneChain::Other
    }
}

/// Wrapper backward-compat : place sans landmarks (utilise proportions fixes
/// du template). Pour le placement amélioré, utiliser `place_template_with_landmarks`.
pub fn place_template(template: AutoRigTemplate, mesh_aabb: &Aabb) -> Vec<PlacedBone> {
    place_template_with_landmarks(template, mesh_aabb, &MeshLandmarks::default())
}

/// Place le template en utilisant des landmarks anatomiques détectés
/// dynamiquement (cf `anatomy_detect`) — le hip est posé à la vraie hauteur
/// du bassin du mesh au lieu d'une fraction fixe du template.
///
/// `mesh_aabb` est en repère mesh-local (= relatif à l'entité qui porte
/// `NeedsAutoRig`). `landmarks.hip_y_frac` override la position Y du root bone.
/// Les autres bones gardent leurs offsets relatifs au parent (proportions
/// templates).
///
/// `compute_local_aabb` propage les transformées enfants en `Mat4` complet
/// (translation+rotation+scale) et accumule les 8 corners post-transform —
/// AABB world-axis-aligned correcte même pour GLB avec sous-meshes rotés
/// (fix BUG-440-QA-04). Worst case sous rotation : AABB +41% en diag,
/// acceptable pour fit conservatif du squelette.
///
/// Panique si le template viole l'invariant BFS (parent déclaré après l'enfant).
/// Utiliser `validate_template` au préalable pour les templates user-supplied.
pub fn place_template_with_landmarks(
    template: AutoRigTemplate,
    mesh_aabb: &Aabb,
    landmarks: &MeshLandmarks,
) -> Vec<PlacedBone> {
    let center: Vec3 = mesh_aabb.center.into();
    let half: Vec3 = mesh_aabb.half_extents.into();
    let mesh_bottom_y = center.y - half.y;
    let mesh_height = (half.y * 2.0).max(0.001);
    let anchor = Vec3::new(center.x, mesh_bottom_y, center.z);

    let bones = template.bones();
    debug_assert!(
        validate_template(bones).is_ok(),
        "auto-rig template {:?} violates BFS invariant: {}",
        template,
        validate_template(bones).unwrap_err()
    );

    let mut placed: Vec<PlacedBone> = Vec::with_capacity(bones.len());
    let mut world_by_name: HashMap<&'static str, Vec3> = HashMap::with_capacity(bones.len());

    // ── Landmark override pour root + scale spine chain ─────────────────────
    // Le template hardcode hip à `0.55 * height` (Humanoid) ou `0.45 * height`
    // (BipedLizard). Sans rescale, le head bone se retrouve à `template_head_y *
    // mesh_height` (Humanoid 0.97 → 1.69m) mais le vrai head est à
    // `landmarks.head_y_frac * mesh_height` (souvent ~0.98 = 1.71m). Pour
    // matcher la VRAIE anatomie, on calcule un `spine_scale` qui rescale les
    // bones above hip (= toute la chaîne spine/chest/neck/head + clavicles).
    //
    // Fix runtime regression 2026-05-17 PM : visuel "head bone au-dessus du
    // mesh" car spine template trop court vs span réel (0.42 vs 0.56 typique).
    let use_landmarks = landmarks.vertex_count >= 100;

    // ── Template metrics hardcoded par template ────────────────────────────
    // template_*_y dans le repère normalisé height=1.
    // template_arm_tip_x_abs = distance |X| max d'un bone arm chain depuis le
    // centre, dans le repère normalisé (sum |x| cumulé : clavicle + arm + forearm).
    let (template_hip_y, template_head_y, template_arm_tip_x_abs) = match template {
        AutoRigTemplate::Humanoid => {
            // hip 0.55 + spine_lower+spine_mid+chest+neck+head = 0.97
            // clavicle 0.10 + arm 0.10 + forearm 0.02 = 0.22
            (0.55_f32, 0.97_f32, 0.22_f32)
        }
        AutoRigTemplate::BipedLizard => {
            // hip 0.45 + cumul spine = 0.82
            // arm 0.12 + forearm 0.04 = 0.16
            (0.45_f32, 0.82_f32, 0.16_f32)
        }
    };
    let template_span = (template_head_y - template_hip_y).max(0.001);
    let real_span = (landmarks.head_y_frac - landmarks.hip_y_frac).max(0.001);
    let spine_scale = if use_landmarks { real_span / template_span } else { 1.0 };

    // leg_scale : compresse/étire les jambes pour que foot bone touche le sol
    // (Y=0 dans repère mesh-local). template_legs_span = template_hip_y (= la
    // distance entre hip et foot en normalisé). real_legs_span =
    // landmarks.hip_y_frac (= distance entre hip réel et sol mesh).
    let leg_scale = if use_landmarks {
        (landmarks.hip_y_frac / template_hip_y).clamp(0.3, 3.0)
    } else {
        1.0
    };

    // arm_scale : étire les bras pour matcher l'arm_span détectée. Apply sur
    // |X| (et Y pour cohérence proportions arm intra-chain).
    let arm_scale = if use_landmarks {
        (landmarks.arm_span_half_frac / template_arm_tip_x_abs).clamp(0.3, 5.0)
    } else {
        1.0
    };

    let landmark_hip_y = if use_landmarks {
        landmarks.hip_y_frac * mesh_height
    } else {
        template_hip_y * mesh_height
    };

    for def in bones {
        // Classify bone par name pour choisir le scale approprié.
        // - Spine chain (spine/chest/neck/head) → spine_scale sur Y
        // - Leg chain (thigh/shin/foot) → leg_scale sur Y (compresse pour
        //   que foot bone touche sol)
        // - Arm chain (clavicle/arm/forearm/hand) → arm_scale sur X (étend
        //   pour que main atteigne extrémité bras T-pose) + spine_scale sur Y
        //   (proportionnel à la spine pour rester cohérent shoulder height)
        let chain = classify_bone_chain(def.name);
        let (x_scale, y_scale) = if use_landmarks {
            match chain {
                BoneChain::Spine => (1.0, spine_scale),
                BoneChain::Leg => (1.0, leg_scale),
                BoneChain::Arm => (arm_scale, spine_scale),
                BoneChain::Other => (1.0, 1.0),
            }
        } else {
            (1.0, 1.0)
        };
        let local_scaled = Vec3::new(
            def.local_translation.x * x_scale * mesh_height,
            def.local_translation.y * y_scale * mesh_height,
            def.local_translation.z * mesh_height,
        );
        let world_pos = match def.parent {
            None => {
                // Root bone (hip) : override Y absolu si landmarks détectés
                let hip_offset = if use_landmarks {
                    Vec3::new(local_scaled.x, landmark_hip_y, local_scaled.z)
                } else {
                    local_scaled
                };
                anchor + hip_offset
            }
            Some(parent_name) => {
                // Invariant BFS : parent toujours déclaré avant l'enfant (validé ci-dessus).
                // En release sans debug_assert, on log + fallback sur anchor pour ne pas crasher.
                match world_by_name.get(parent_name) {
                    Some(p) => *p + local_scaled,
                    None => {
                        warn!(
                            "[forgia-auto-rig] template {:?} : bone '{}' parent '{}' missing, falling back to anchor",
                            template, def.name, parent_name
                        );
                        anchor + local_scaled
                    }
                }
            }
        };
        world_by_name.insert(def.name, world_pos);
        placed.push(PlacedBone {
            name: def.name,
            parent: def.parent,
            local_translation: local_scaled,
            world_pos,
        });
    }

    placed
}

// ── System : process pending auto-rigs ──────────────────────────────────────

/// Compute un AABB "effectif" via bottom-percentile sur les vertices.
///
/// Bug Bevy #18921 + pratiques industrie (Mixamo/Pinocchio) : l'AABB strict
/// min/max d'un GLB Meshy inclut souvent des outliers (cheveux, capes, LOD
/// impostors, sub-meshes invisibles) qui faussent `mesh_height`. Solution :
/// trier les vertices par axe et prendre le percentile `p` / `1-p`
/// (p=5 typique → exclut 5% des vertices les plus extrêmes par axe).
///
/// Coût : O(n log n) sur 4k-30k vertices, ~5ms one-shot au spawn = acceptable.
///
/// Retourne `None` si positions vide.
fn compute_effective_aabb_from_vertices(positions: &[Vec3], percentile_pct: f32) -> Option<Aabb> {
    if positions.is_empty() {
        return None;
    }
    let n = positions.len();
    let p = (percentile_pct / 100.0).clamp(0.0, 0.49);
    let lo_idx = ((n as f32 * p) as usize).min(n - 1);
    let hi_idx = (n - 1).saturating_sub(lo_idx);

    let mut ys: Vec<f32> = positions.iter().map(|v| v.y).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let min_y = ys[lo_idx];
    let max_y = ys[hi_idx];

    let mut xs: Vec<f32> = positions.iter().map(|v| v.x).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let min_x = xs[lo_idx];
    let max_x = xs[hi_idx];

    let mut zs: Vec<f32> = positions.iter().map(|v| v.z).collect();
    zs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let min_z = zs[lo_idx];
    let max_z = zs[hi_idx];

    let center = Vec3::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5, (min_z + max_z) * 0.5);
    let half = Vec3::new((max_x - min_x) * 0.5, (max_y - min_y) * 0.5, (max_z - min_z) * 0.5);
    Some(Aabb {
        center: center.into(),
        half_extents: half.into(),
    })
}

/// Compte les Mesh3d descendants + total vertices (pour debug sensor).
fn collect_mesh3d_count(
    root: Entity,
    children_q: &Query<&Children>,
    q_mesh3d: &Query<&bevy::mesh::Mesh3d>,
    meshes_assets: &Assets<bevy::mesh::Mesh>,
    out_count: &mut usize,
    out_total_verts: &mut usize,
) {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok(m3d) = q_mesh3d.get(e) {
            *out_count += 1;
            if let Some(mesh) = meshes_assets.get(&m3d.0) {
                if let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
                    mesh.attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION)
                {
                    *out_total_verts += p.len();
                }
            }
        }
        if let Ok(children) = children_q.get(e) {
            for c in children.iter() {
                stack.push(c);
            }
        }
    }
}

/// Walk descendants pour collecter toutes les positions vertex des Mesh3d
/// dans le repère mesh-root-local. Utilisé par `auto_rig_pending_meshes` pour
/// l'analyse `anatomy_detect`. Mat4 propagation comme `compute_local_aabb`.
fn collect_vertex_positions_local(
    root: Entity,
    children_q: &Query<&Children>,
    transforms_q: &Query<&Transform>,
    q_mesh3d: &Query<&bevy::mesh::Mesh3d>,
    meshes_assets: &Assets<bevy::mesh::Mesh>,
    out: &mut Vec<Vec3>,
) {
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

        if let Ok(mesh3d) = q_mesh3d.get(entity) {
            if let Some(mesh) = meshes_assets.get(&mesh3d.0) {
                if let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION)
                {
                    // Sample 1 vertex sur 4 pour limiter le coût sur gros meshes
                    // (analyse landmarks tolère ~25% downsample sans perte de précision).
                    let step = (positions.len() / 4000).max(1);
                    for (i, p) in positions.iter().enumerate() {
                        if i % step == 0 {
                            let local_p = Vec3::from_array(*p);
                            out.push(world_mat.transform_point3(local_p));
                        }
                    }
                }
            }
        }

        if let Ok(children) = children_q.get(entity) {
            for c in children.iter() {
                stack.push((c, world_mat));
            }
        }
    }
}

/// Walk descendants pour accumuler l'AABB world-axis-aligned en repère
/// mesh-local. Propage la transformée complète (translation + rotation + scale)
/// via `GlobalTransform` accumulé, et transforme les 8 corners de chaque AABB
/// enfant pour rester correct sous rotation arbitraire (BUG-440-QA-04 fix).
///
/// Le Transform du `root` est exclu : on calcule en repère mesh-local où le
/// root est à l'identité. Les enfants GLB peuvent avoir n'importe quelle
/// orientation locale, l'AABB résultante reste axis-aligned dans ce repère.
///
/// Retourne `None` si aucun AABB trouvé (mesh pas encore instancié par AssetServer).
fn compute_local_aabb(
    root: Entity,
    children_q: &Query<&Children>,
    transforms_q: &Query<&Transform>,
    aabbs_q: &Query<&Aabb>,
) -> Option<Aabb> {
    let mut min_v = Vec3::splat(f32::INFINITY);
    let mut max_v = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;

    // Stack : (Entity, transformée cumulée parent en repère mesh-local).
    let mut stack: Vec<(Entity, Mat4)> = vec![(root, Mat4::IDENTITY)];
    while let Some((entity, parent_world)) = stack.pop() {
        // Le Transform du root est exclu (identité). Les autres : transformée complète.
        let local_mat = if entity == root {
            Mat4::IDENTITY
        } else {
            transforms_q
                .get(entity)
                .map(|t| t.to_matrix())
                .unwrap_or(Mat4::IDENTITY)
        };
        let world_mat = parent_world * local_mat;

        if let Ok(aabb) = aabbs_q.get(entity) {
            let c: Vec3 = aabb.center.into();
            let h: Vec3 = aabb.half_extents.into();
            // 8 corners de l'AABB local, transformés par world_mat.
            // Sous rotation, l'AABB world-aligned résultante peut être plus large
            // que l'AABB local (worst case +41% en diag) — acceptable, on est
            // côté "fit conservatif" pour le squelette.
            for sx in [-1.0_f32, 1.0] {
                for sy in [-1.0_f32, 1.0] {
                    for sz in [-1.0_f32, 1.0] {
                        let corner_local = c + Vec3::new(sx * h.x, sy * h.y, sz * h.z);
                        let corner_world = world_mat.transform_point3(corner_local);
                        min_v = min_v.min(corner_world);
                        max_v = max_v.max(corner_world);
                    }
                }
            }
            found = true;
        }

        if let Ok(children) = children_q.get(entity) {
            for c in children.iter() {
                stack.push((c, world_mat));
            }
        }
    }

    if !found || !min_v.is_finite() || !max_v.is_finite() {
        return None;
    }
    let center = (min_v + max_v) * 0.5;
    let half = (max_v - min_v) * 0.5;
    Some(Aabb {
        center: center.into(),
        half_extents: half.into(),
    })
}

/// Système principal Phase 1A : pour chaque entité avec `NeedsAutoRig` et sans
/// `AutoRigged`, calcule l'AABB local, place le template, spawne les bones en
/// hiérarchie, insère le marker, retire la demande. Retry chaque frame tant que
/// l'AABB n'est pas calculable (Scene GLB asynchrone).
#[allow(clippy::too_many_arguments)]
pub fn auto_rig_pending_meshes(
    mut commands: Commands,
    time: Res<Time>,
    backend: Res<AutoRigBackend>,
    mut stats: ResMut<AutoRigStats>,
    q_pending: Query<(Entity, &NeedsAutoRig), Without<AutoRigged>>,
    children_q: Query<&Children>,
    transforms_q: Query<&Transform>,
    aabbs_q: Query<&Aabb>,
    _names_q: Query<&Name>,
    meshes_assets: Res<Assets<bevy::mesh::Mesh>>,
    q_mesh3d: Query<&bevy::mesh::Mesh3d>,
) {
    // BUG FIX 2026-05-17 NUIT (logs Antoine) : sans cette gate, TemplateFit
    // tournait en parallèle de Pinocchio dans la même frame → DOUBLE rig
    // sur la même entité, TemplateFit écrasait Pinocchio. Maintenant les 2
    // pipelines sont mutually exclusive via backend Resource.
    if *backend != AutoRigBackend::TemplateFit {
        return;
    }
    for (mesh_root, request) in &q_pending {
        let NeedsAutoRig::Template(requested_template) = *request;

        // 1. Collect vertex positions DABORD (utilisé pour landmarks ET pour
        //    compute l'AABB effective via bottom-percentile).
        //    Bug Bevy #18921 contourné via Mat4 propagation manuelle.
        let mut all_vertices_early: Vec<Vec3> = Vec::new();
        collect_vertex_positions_local(
            mesh_root,
            &children_q,
            &transforms_q,
            &q_mesh3d,
            &meshes_assets,
            &mut all_vertices_early,
        );

        // 2. AABB effective via percentile p5/p95 sur les vertices (Pinocchio/
        //    industry approach — exclut outliers cheveux/capes/LOD impostors).
        //    Fallback sur AABB Bevy strict si trop peu de vertices analysables.
        let aabb = match compute_effective_aabb_from_vertices(&all_vertices_early, 5.0) {
            Some(a) if all_vertices_early.len() >= 100 => a,
            _ => match compute_local_aabb(mesh_root, &children_q, &transforms_q, &aabbs_q) {
                Some(a) => a,
                None => continue, // retry frame suivant (GLB pas instancié)
            },
        };

        let mesh_height = aabb.half_extents.y * 2.0;
        if mesh_height < 0.05 {
            // AABB dégénérée — passe sans bloquer le caller.
            warn!(
                "[forgia-auto-rig] mesh AABB height ({:.3}m) too small, skipping rig for {:?}",
                mesh_height, mesh_root
            );
            stats.total_rig_failures_degenerate_aabb += 1;
            commands
                .entity(mesh_root)
                .remove::<NeedsAutoRig>()
                .insert(AutoRigged {
                    template: Some(requested_template),
                    bone_count: 0,
                    mesh_height,
                });
            continue;
        }

        // 3. Détecter landmarks anatomiques sur les vertices (collected step 1) +
        //    l'AABB effective (step 2). Auto-switch template si mismatch.
        let landmarks = anatomy_detect::detect_landmarks_from_vertices(&all_vertices_early, &aabb);
        let template = if landmarks.vertex_count >= 100 {
            let suggested = if landmarks.looks_humanoid() {
                AutoRigTemplate::Humanoid
            } else {
                AutoRigTemplate::BipedLizard
            };
            if suggested != requested_template {
                warn!(
                    "[forgia-auto-rig] template requested={:?} but landmarks suggest {:?} \
                     (aspect={:.2}, hip_y_frac={:.2}, has_tail={}) — using suggested",
                    requested_template,
                    suggested,
                    landmarks.aspect_ratio_xy,
                    landmarks.hip_y_frac,
                    landmarks.has_tail
                );
                suggested
            } else {
                requested_template
            }
        } else {
            requested_template
        };

        info!(
            "[forgia-auto-rig] landmarks for {:?}: vertices={}, hip_y_frac={:.2}, shoulder_y_frac={:.2}, aspect={:.2}, has_tail={}, looks_humanoid={}, template={:?}",
            mesh_root,
            landmarks.vertex_count,
            landmarks.hip_y_frac,
            landmarks.shoulder_y_frac,
            landmarks.aspect_ratio_xy,
            landmarks.has_tail,
            landmarks.looks_humanoid(),
            template,
        );

        // 4. Place le template avec landmarks (override hip Y dynamique).
        let placed = place_template_with_landmarks(template, &aabb, &landmarks);

        // 3. Spawn les bones en hiérarchie. On itère dans l'ordre de la const
        //    (parent toujours déclaré avant ses enfants) → on peut résoudre le
        //    parent Entity au fur et à mesure via HashMap<name, Entity>.
        let mut entity_by_name: HashMap<&'static str, Entity> = HashMap::with_capacity(placed.len());

        for bone in &placed {
            let bone_entity = commands
                .spawn((
                    Name::new(bone.name),
                    Transform::from_translation(bone.local_translation),
                    Visibility::default(),
                    // Marker pour que `skinning` retrouve les bones du squelette
                    // (par opposition aux Mesh3d entities) sans deviner par Name.
                    BoneEntity { rig_root: mesh_root },
                ))
                .id();
            entity_by_name.insert(bone.name, bone_entity);

            // Parent : soit le mesh root (root bone), soit le bone parent.
            // Invariant BFS validé en debug par `place_template`. En release, on log
            // + fallback mesh_root pour ne pas casser le spawn entier.
            let parent_entity = match bone.parent {
                None => mesh_root,
                Some(parent_name) => match entity_by_name.get(parent_name) {
                    Some(e) => *e,
                    None => {
                        warn!(
                            "[forgia-auto-rig] bone '{}' parent '{}' not yet spawned, attaching to mesh_root",
                            bone.name, parent_name
                        );
                        mesh_root
                    }
                },
            };
            commands.entity(parent_entity).add_child(bone_entity);
        }

        // 4. Marker + retire la demande.
        commands
            .entity(mesh_root)
            .remove::<NeedsAutoRig>()
            .insert(AutoRigged {
                template: Some(template),
                bone_count: placed.len(),
                mesh_height,
            });

        // Update stats pour le sensor.
        stats.total_rig_successes += 1;
        stats.last_template = Some(template);
        stats.last_requested_template = Some(requested_template);
        stats.last_landmarks = if landmarks.vertex_count >= 100 {
            Some(landmarks)
        } else {
            None
        };
        stats.last_auto_switched = template != requested_template;
        stats.last_bone_count = placed.len();
        stats.last_mesh_height = mesh_height;
        stats.last_rigged_at_secs = time.elapsed_secs();

        // ── Deep debug : AABB + mesh3d count + bones clés ──────────────────
        let aabb_min: Vec3 = aabb.center.into();
        let aabb_half: Vec3 = aabb.half_extents.into();
        stats.last_aabb_min = aabb_min - aabb_half;
        stats.last_aabb_max = aabb_min + aabb_half;
        // count Mesh3d descendants
        let mut mesh3d_n = 0usize;
        let mut total_verts = 0usize;
        collect_mesh3d_count(
            mesh_root,
            &children_q,
            &q_mesh3d,
            &meshes_assets,
            &mut mesh3d_n,
            &mut total_verts,
        );
        stats.last_mesh3d_count = mesh3d_n;
        stats.last_total_mesh_vertices = total_verts;
        // Scales appliqués (re-compute pour stats, déjà computés dans place_template)
        let (t_hip, t_head, t_arm) = match template {
            AutoRigTemplate::Humanoid => (0.55_f32, 0.97_f32, 0.22_f32),
            AutoRigTemplate::BipedLizard => (0.45_f32, 0.82_f32, 0.16_f32),
        };
        let use_lm = landmarks.vertex_count >= 100;
        stats.last_spine_scale = if use_lm {
            (landmarks.head_y_frac - landmarks.hip_y_frac) / (t_head - t_hip).max(0.001)
        } else { 1.0 };
        stats.last_leg_scale = if use_lm { landmarks.hip_y_frac / t_hip } else { 1.0 };
        stats.last_arm_scale = if use_lm { landmarks.arm_span_half_frac / t_arm } else { 1.0 };
        // Bones clés (positions en repère mesh-root-local = pas propagées par rex_tf)
        let find_bone = |name: &str| placed.iter().find(|b| b.name == name).map(|b| b.world_pos);
        stats.last_hip_world_y = find_bone("hip").map(|p| p.y).unwrap_or(0.0);
        stats.last_head_world_y = find_bone("head").map(|p| p.y).unwrap_or(0.0);
        stats.last_foot_l_world_y = find_bone("foot_L").map(|p| p.y).unwrap_or(0.0);
        stats.last_forearm_l_world_x = find_bone("forearm_L").map(|p| p.x).unwrap_or(0.0);

        info!(
            "[forgia-auto-rig] rigged mesh {:?} with {:?} : {} bones, mesh_height={:.2}m",
            mesh_root,
            template,
            placed.len(),
            mesh_height
        );
    }
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

const SENSOR_JSON_PATH: &str = "forgia_auto_rig.json";
const SENSOR_HEALTH_PATH: &str = "forgia_auto_rig_health.json";

/// Écrit `forgia_auto_rig.json` toutes les `SENSOR_INTERVAL_S`. Émet
/// `forgia_auto_rig_health.json` (severity warning) si un mesh reste `pending`
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

    let severity = if pending_age_s > PENDING_STALE_THRESHOLD_S {
        "warning"
    } else {
        "ok"
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
            "{{ \"vertex_count\": {}, \"hip_y_frac\": {:.3}, \"shoulder_y_frac\": {:.3}, \"head_y_frac\": {:.3}, \"aspect_ratio_xy\": {:.3}, \"hip_width_frac\": {:.3}, \"has_tail\": {}, \"looks_humanoid\": {} }}",
            l.vertex_count, l.hip_y_frac, l.shoulder_y_frac, l.head_y_frac,
            l.aspect_ratio_xy, l.hip_width_frac, l.has_tail, l.looks_humanoid()
        ),
        None => "null".into(),
    };

    let json = format!(
        r#"{{
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
        stats.last_aabb_min.x, stats.last_aabb_min.y, stats.last_aabb_min.z,
        stats.last_aabb_max.x, stats.last_aabb_max.y, stats.last_aabb_max.z,
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
  "next_step": "Read forgia_auto_rig.json then inspect the SceneRoot of pending entities — check that GLB instantiated children with Aabb component (cargo run + log filter '[forgia-auto-rig]')"
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
        use forgia_genome_core::{Genome, GenomeLoader};
        use forgia_skeleton_embedder::SkeletonTemplate;
        app.init_asset::<Genome<SkeletonTemplate>>()
            .register_asset_loader(GenomeLoader::<SkeletonTemplate>::default())
            .add_systems(Startup, load_skeleton_genomes)
            .init_resource::<AutoRigStats>()
            .init_resource::<AutoRigSensorTimer>()
            .init_resource::<SkinningConfig>()
            .init_resource::<AutoRigGizmosConfig>()
            .init_resource::<AutoRigBackend>()
            .add_systems(Update, draw_rig_gizmos.in_set(GameSet::Effects))
            // Producer Movement : 2 systèmes mutually exclusive via AutoRigBackend.
            // TemplateFit historique + PinocchioV1 nouveau (story-440 Phase 1).
            .add_systems(
                Update,
                (auto_rig_pending_meshes, auto_rig_pinocchio_v1)
                    .in_set(GameSet::Movement),
            )
            // Skinning injection : DISABLED story-440 R&D 2026-05-17 night.
            // Cause : mesh aplati (bug inverse_bindposes pas encore validé).
            // Validation visuelle bones gizmos d'abord, skinning Phase 2 après.
            // .add_systems(PostUpdate, inject_skinning_for_rigged_meshes)
            // Sensor dans Sensors set (après Effects, avant UI) — pattern V2.
            .add_systems(
                Update,
                write_auto_rig_sensor.in_set(GameSet::Sensors),
            );
    }
}

// ── Tests headless ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_rig_topology::analyze_rig_topology;

    fn unit_cube_aabb() -> Aabb {
        Aabb {
            center: Vec3::new(0.0, 0.5, 0.0).into(), // mesh debout : bottom=0, top=1
            half_extents: Vec3::splat(0.5).into(),
        }
    }

    #[test]
    fn humanoid_template_has_18_bones() {
        assert_eq!(HUMANOID_BONES.len(), 18);
    }

    #[test]
    fn templates_satisfy_parent_bfs_order() {
        // BUG-440-QA-01 : invariant "parent déclaré avant l'enfant" doit tenir
        // pour tous les templates buit-in. Sinon `place_template` fallback
        // silencieusement sur anchor → rig corrompu.
        validate_template(HUMANOID_BONES).expect("humanoid BFS order");
        validate_template(BIPED_LIZARD_BONES).expect("biped_lizard BFS order");
    }

    #[test]
    fn validate_template_catches_out_of_order_parent() {
        let bad = [
            BoneDef { name: "child", parent: Some("not_yet_declared"), local_translation: Vec3::ZERO },
            BoneDef { name: "not_yet_declared", parent: None, local_translation: Vec3::ZERO },
        ];
        let err = validate_template(&bad).expect_err("must reject out-of-order template");
        assert!(err.contains("not_yet_declared"), "error must name the missing parent: {}", err);
    }

    #[test]
    fn biped_lizard_has_4_tail_bones() {
        let tail_count = BIPED_LIZARD_BONES
            .iter()
            .filter(|b| b.name.starts_with("tail_"))
            .count();
        assert_eq!(tail_count, 4);
    }

    #[test]
    fn humanoid_fits_unit_cube_aabb() {
        let aabb = unit_cube_aabb();
        let placed = place_template(AutoRigTemplate::Humanoid, &aabb);
        for bone in &placed {
            assert!(
                bone.world_pos.x.abs() <= 0.55,
                "bone {} world_x={} outside cube",
                bone.name,
                bone.world_pos.x
            );
            assert!(
                (-0.02..=1.02).contains(&bone.world_pos.y),
                "bone {} world_y={} outside [0,1]",
                bone.name,
                bone.world_pos.y
            );
        }
    }

    #[test]
    fn biped_lizard_fits_unit_cube_aabb() {
        let aabb = unit_cube_aabb();
        let placed = place_template(AutoRigTemplate::BipedLizard, &aabb);
        for bone in &placed {
            assert!(
                bone.world_pos.x.abs() <= 0.55,
                "bone {} world_x={}",
                bone.name,
                bone.world_pos.x
            );
            assert!(
                (-0.02..=1.02).contains(&bone.world_pos.y),
                "bone {} world_y={}",
                bone.name,
                bone.world_pos.y
            );
        }
    }

    #[test]
    fn place_template_with_landmarks_rescales_spine_to_head() {
        // REGRESSION 2026-05-17 PM (screenshot Antoine) : sans rescale, le head
        // bone se retrouvait à template_head_y * mesh_height = 0.97 * 1.745 = 1.69m,
        // mais landmarks.head_y_frac = 0.98 → vraie head à 1.71m. Plus grave :
        // avec hip override à 0.42, head bone tombe à 0.84 * 1.745 = 1.47m,
        // soit 25cm SOUS la vraie tête (visuel "dépassé" sur screenshot dû au
        // mesh root translation côté character.rs amplifiant le décalage).
        //
        // Fix : spine_scale = real_span / template_span rescale les offsets Y.
        let aabb = Aabb {
            center: Vec3::new(0.0, 0.5, 0.0).into(),
            half_extents: Vec3::splat(0.5).into(),
        };
        let landmarks = MeshLandmarks {
            hip_y_frac: 0.42,
            shoulder_y_frac: 0.67,
            neck_y_frac: 0.83,
            head_y_frac: 0.98,
            aspect_ratio_xy: 1.0,
            hip_width_frac: 0.20,
            arm_span_half_frac: 0.50,
            has_tail: false,
            vertex_count: 5000,
        };
        let placed = place_template_with_landmarks(AutoRigTemplate::Humanoid, &aabb, &landmarks);

        // Head bone doit être proche de landmarks.head_y_frac * mesh_height
        let head = placed.iter().find(|b| b.name == "head").expect("head bone");
        let mesh_height = 1.0; // unit cube
        let mesh_bottom = 0.0;
        let expected_head_y = mesh_bottom + landmarks.head_y_frac * mesh_height;
        let diff = (head.world_pos.y - expected_head_y).abs();
        assert!(
            diff < 0.02,
            "head bone Y={} expected ~{} (landmarks.head_y_frac * mesh_height), diff={}",
            head.world_pos.y, expected_head_y, diff
        );

        // Hip bone doit être à landmarks.hip_y_frac
        let hip = placed.iter().find(|b| b.name == "hip").expect("hip bone");
        let expected_hip_y = mesh_bottom + landmarks.hip_y_frac * mesh_height;
        assert!(
            (hip.world_pos.y - expected_hip_y).abs() < 0.02,
            "hip Y={} expected ~{}",
            hip.world_pos.y, expected_hip_y
        );
    }

    #[test]
    fn effective_aabb_strips_outliers() {
        // REGRESSION runtime 2026-05-17 PM (Antoine screenshot 3) : Meshy GLB
        // a souvent des outliers (cheveux longs, capes) qui étendent l'AABB
        // strict. mesh_height surévalué → squelette trop grand placé trop haut
        // (bug Bevy #18921 confirmé par bg agent).
        //
        // Mock : 1000 vertices "body" Y∈[0, 1] + 10 vertices "cheveux outliers"
        // Y∈[1.5, 2.0]. AABB strict → height=2.0. Percentile p5 → height≈1.0.
        let mut positions = Vec::new();
        for i in 0..1000 {
            let y = i as f32 / 1000.0;
            positions.push(Vec3::new(0.1, y, 0.0));
            positions.push(Vec3::new(-0.1, y, 0.0));
        }
        // 10 outliers cheveux
        for i in 0..10 {
            positions.push(Vec3::new(0.0, 1.5 + i as f32 * 0.05, 0.0));
        }

        let aabb_strict_max = positions.iter().map(|v| v.y).fold(f32::NEG_INFINITY, f32::max);
        assert!(aabb_strict_max > 1.5, "strict AABB inclus outliers (got {})", aabb_strict_max);

        let aabb_p5 = compute_effective_aabb_from_vertices(&positions, 5.0)
            .expect("aabb computed");
        let center: Vec3 = aabb_p5.center.into();
        let half: Vec3 = aabb_p5.half_extents.into();
        let height = half.y * 2.0;
        let max_y = center.y + half.y;
        assert!(
            height < 1.2,
            "p5 height should exclude outliers, expected ~1.0, got {:.3}",
            height
        );
        assert!(
            max_y < 1.2,
            "p5 max_y should ignore cheveux outliers (1.5+), got {:.3}",
            max_y
        );
    }

    #[test]
    fn effective_aabb_preserves_main_body() {
        // Vérifie qu'avec un mesh propre (sans outliers), le percentile donne
        // un AABB quasi-identique au AABB strict.
        let mut positions = Vec::new();
        for i in 0..1000 {
            let y = i as f32 / 1000.0; // Y 0..1
            for x in [-0.20_f32, -0.10, 0.10, 0.20] {
                positions.push(Vec3::new(x, y, 0.0));
            }
        }
        let aabb = compute_effective_aabb_from_vertices(&positions, 5.0)
            .expect("aabb computed");
        let half: Vec3 = aabb.half_extents.into();
        let height = half.y * 2.0;
        // p5/p95 sur mesh uniforme → exclus 5% bottom + 5% top = height ≈ 0.9
        assert!(
            (0.85..=0.95).contains(&height),
            "p5 sur mesh uniforme = ~0.9, got {:.3}",
            height
        );
    }

    #[test]
    fn place_template_with_landmarks_foot_touches_ground() {
        // Request Antoine 2026-05-17 PM : foot bone DOIT toucher le sol mesh
        // (Y ≈ aabb.bottom). Sans leg_scale, hip override à 0.42 ferait
        // descendre foot à Y=-0.13 (sous sol).
        let aabb = Aabb {
            center: Vec3::new(0.0, 0.5, 0.0).into(),
            half_extents: Vec3::splat(0.5).into(),
        };
        let landmarks = MeshLandmarks {
            hip_y_frac: 0.42,
            shoulder_y_frac: 0.67,
            neck_y_frac: 0.83,
            head_y_frac: 0.98,
            aspect_ratio_xy: 1.0,
            hip_width_frac: 0.20,
            arm_span_half_frac: 0.50,
            has_tail: false,
            vertex_count: 5000,
        };
        let placed = place_template_with_landmarks(AutoRigTemplate::Humanoid, &aabb, &landmarks);

        // Foot bones (L + R) doivent toucher le sol (Y proche de aabb.bottom = 0)
        let foot_l = placed.iter().find(|b| b.name == "foot_L").expect("foot_L");
        let foot_r = placed.iter().find(|b| b.name == "foot_R").expect("foot_R");
        let aabb_bottom = 0.0_f32; // unit cube centered at 0.5
        assert!(
            (foot_l.world_pos.y - aabb_bottom).abs() < 0.05,
            "foot_L Y={} expected ~{} (sol mesh), diff={}",
            foot_l.world_pos.y, aabb_bottom, (foot_l.world_pos.y - aabb_bottom).abs()
        );
        assert!(
            (foot_r.world_pos.y - aabb_bottom).abs() < 0.05,
            "foot_R Y={} expected ~{} (sol mesh)",
            foot_r.world_pos.y, aabb_bottom
        );
    }

    #[test]
    fn place_template_with_landmarks_arms_reach_extremities() {
        // Request Antoine 2026-05-17 PM : main (= forearm bone, end of arm
        // chain) DOIT atteindre les extrémités latérales du mesh T-pose
        // (= ±arm_span_half_frac * mesh_height).
        let aabb = Aabb {
            center: Vec3::new(0.0, 0.5, 0.0).into(),
            half_extents: Vec3::splat(0.5).into(),
        };
        let landmarks = MeshLandmarks {
            hip_y_frac: 0.42,
            shoulder_y_frac: 0.67,
            neck_y_frac: 0.83,
            head_y_frac: 0.98,
            aspect_ratio_xy: 1.0,
            hip_width_frac: 0.20,
            arm_span_half_frac: 0.45, // T-pose : bras atteignent 45% mesh height
            has_tail: false,
            vertex_count: 5000,
        };
        let placed = place_template_with_landmarks(AutoRigTemplate::Humanoid, &aabb, &landmarks);

        let forearm_l = placed.iter().find(|b| b.name == "forearm_L").expect("forearm_L");
        let forearm_r = placed.iter().find(|b| b.name == "forearm_R").expect("forearm_R");

        // Arm tip X attendu : ≈ ±arm_span_half_frac * mesh_height = ±0.45
        let expected_abs_x = landmarks.arm_span_half_frac;
        assert!(
            (forearm_l.world_pos.x.abs() - expected_abs_x).abs() < 0.05,
            "forearm_L |X|={} expected ~{} (arm_span_half_frac)",
            forearm_l.world_pos.x.abs(), expected_abs_x
        );
        assert!(forearm_l.world_pos.x < 0.0, "forearm_L doit être à gauche (X<0)");
        assert!(forearm_r.world_pos.x > 0.0, "forearm_R doit être à droite (X>0)");
    }

    #[test]
    fn place_template_without_landmarks_uses_template_proportions() {
        // Backward-compat : sans landmarks (vertex_count=0), le template
        // garde ses proportions Vitruvian. Head à 0.97 du mesh height.
        let aabb = Aabb {
            center: Vec3::new(0.0, 0.5, 0.0).into(),
            half_extents: Vec3::splat(0.5).into(),
        };
        let placed = place_template(AutoRigTemplate::Humanoid, &aabb);
        let head = placed.iter().find(|b| b.name == "head").expect("head bone");
        // Template: hip 0.55 + spine cumul 0.42 = head at 0.97 → world Y = 0.97
        assert!(
            (head.world_pos.y - 0.97).abs() < 0.02,
            "head should be at template ratio 0.97, got {}",
            head.world_pos.y
        );
    }

    #[test]
    fn topology_classifies_placed_humanoid() {
        // Place le template humanoïde puis vérifie que forgia-rig-topology
        // identifie correctement legs L/R, arms L/R, spine, head.
        let aabb = unit_cube_aabb();
        let placed = place_template(AutoRigTemplate::Humanoid, &aabb);

        // Spawn dans un World Bevy minimal en respectant la hiérarchie.
        let mut world = World::new();
        let mesh_root = world
            .spawn((Transform::default(), Name::new("mesh_root")))
            .id();

        let mut entity_by_name: HashMap<&str, Entity> = HashMap::new();
        for bone in &placed {
            let e = world
                .spawn((
                    Transform::from_translation(bone.local_translation),
                    Name::new(bone.name),
                ))
                .id();
            entity_by_name.insert(bone.name, e);
            let parent = match bone.parent {
                None => mesh_root,
                Some(pname) => entity_by_name[pname],
            };
            world.entity_mut(parent).add_child(e);
        }

        let children_of = |e: Entity| {
            world
                .get::<Children>(e)
                .map(|c| c.iter().collect())
                .unwrap_or_default()
        };
        let transform_of = |e: Entity| world.get::<Transform>(e).copied();
        let name_of = |e: Entity| world.get::<Name>(e).map(|n| n.to_string());

        let topo = analyze_rig_topology(mesh_root, &children_of, &transform_of, &name_of);
        assert!(topo.is_usable(), "topology must be usable");
        assert!(topo.spine.is_some(), "spine must be classified");
        assert!(topo.left_leg.is_some(), "left_leg must be classified");
        assert!(topo.right_leg.is_some(), "right_leg must be classified");
        assert!(topo.left_arm.is_some(), "left_arm must be classified");
        assert!(topo.right_arm.is_some(), "right_arm must be classified");
        assert!(topo.head.is_some(), "head must be classified");
    }

    #[test]
    fn topology_detects_lizard_tail_chain() {
        let aabb = unit_cube_aabb();
        let placed = place_template(AutoRigTemplate::BipedLizard, &aabb);

        let mut world = World::new();
        let mesh_root = world
            .spawn((Transform::default(), Name::new("mesh_root")))
            .id();
        let mut entity_by_name: HashMap<&str, Entity> = HashMap::new();
        for bone in &placed {
            let e = world
                .spawn((
                    Transform::from_translation(bone.local_translation),
                    Name::new(bone.name),
                ))
                .id();
            entity_by_name.insert(bone.name, e);
            let parent = match bone.parent {
                None => mesh_root,
                Some(pname) => entity_by_name[pname],
            };
            world.entity_mut(parent).add_child(e);
        }

        let children_of = |e: Entity| {
            world
                .get::<Children>(e)
                .map(|c| c.iter().collect())
                .unwrap_or_default()
        };
        let transform_of = |e: Entity| world.get::<Transform>(e).copied();
        let name_of = |e: Entity| world.get::<Name>(e).map(|n| n.to_string());

        let topo = analyze_rig_topology(mesh_root, &children_of, &transform_of, &name_of);
        assert!(
            !topo.tail_chain.is_empty(),
            "biped_lizard tail must be detected"
        );
        assert!(
            topo.tail_chain.len() >= 3,
            "tail chain length should be >= 3, got {}",
            topo.tail_chain.len()
        );
    }

    #[test]
    fn compute_local_aabb_handles_rotated_child() {
        // BUG-440-QA-04 regression : un enfant tourné de PI/2 autour de Y voit
        // son AABB swap X↔Z dans l'espace parent. La V1 (Vec3 add) plaçait
        // l'AABB au mauvais endroit → squelette décalé.
        let mut app = App::new();
        app.add_plugins(bevy::MinimalPlugins);
        let root = app.world_mut().spawn(Transform::default()).id();
        // Enfant translaté en X=1 ET tourné de PI/2 autour de Y → un AABB
        // étendu en X-local devient étendu en Z-monde.
        let child = app
            .world_mut()
            .spawn((
                Transform::from_xyz(1.0, 0.5, 0.0)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
                Aabb {
                    center: Vec3::new(0.5, 0.0, 0.0).into(), // décalé en X local
                    half_extents: Vec3::new(0.5, 0.5, 0.1).into(),
                },
            ))
            .id();
        app.world_mut().entity_mut(root).add_child(child);

        let mut q_children = app.world_mut().query::<&Children>();
        let mut q_transform = app.world_mut().query::<&Transform>();
        let mut q_aabb = app.world_mut().query::<&Aabb>();
        let children_q = q_children.query(app.world());
        let transforms_q = q_transform.query(app.world());
        let aabbs_q = q_aabb.query(app.world());

        let aabb = compute_local_aabb(root, &children_q, &transforms_q, &aabbs_q)
            .expect("aabb must be computed");
        let center: Vec3 = aabb.center.into();
        let half: Vec3 = aabb.half_extents.into();

        // Centre attendu : enfant translaté (1, 0.5, 0) + AABB center local (0.5, 0, 0)
        // tourné PI/2 Y → (0, 0, -0.5) world. Donc centre world = (1, 0.5, -0.5).
        assert!(
            (center.x - 1.0).abs() < 0.01,
            "center.x={} expected ~1.0", center.x
        );
        assert!(
            (center.y - 0.5).abs() < 0.01,
            "center.y={} expected ~0.5", center.y
        );
        assert!(
            (center.z + 0.5).abs() < 0.01,
            "center.z={} expected ~-0.5", center.z
        );
        // Half-extents : l'AABB local (0.5, 0.5, 0.1) tournée PI/2 Y devient
        // (0.1, 0.5, 0.5) — donc on attend half ≈ (0.1, 0.5, 0.5).
        assert!(half.x < 0.2, "half.x={} expected <0.2 (rotated X→Z)", half.x);
        assert!(half.z > 0.4, "half.z={} expected >0.4 (rotated X→Z)", half.z);
    }

    #[test]
    fn auto_rig_system_is_idempotent() {
        // 2 passes du système sur un même mesh → 1 seul rig, pas 2.
        // Test scoped au Phase 1A producer uniquement — pas le plugin global
        // (qui charge le système skinning Phase 1B nécessitant Assets<Mesh>+SkinnedMeshInverseBindposes).
        let mut app = App::new();
        app.add_plugins(bevy::MinimalPlugins);
        app.init_resource::<AutoRigStats>();
        app.init_resource::<AutoRigSensorTimer>();
        // Mesh assets requis depuis ajout de l'analyse anatomique (collect_vertex_positions_local).
        app.init_resource::<Assets<bevy::mesh::Mesh>>();
        // Backend pour gate Pinocchio vs TemplateFit. Test = TemplateFit.
        app.insert_resource(AutoRigBackend::TemplateFit);
        app.add_systems(Update, auto_rig_pending_meshes);

        // Spawn mesh root avec AABB + 1 enfant porteur d'AABB pour que
        // `compute_local_aabb` retourne Some.
        let mesh_root = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::default(),
                NeedsAutoRig::Template(AutoRigTemplate::Humanoid),
            ))
            .id();
        let child = app
            .world_mut()
            .spawn((
                Transform::default(),
                Aabb {
                    center: Vec3::new(0.0, 0.9, 0.0).into(),
                    half_extents: Vec3::new(0.3, 0.9, 0.3).into(),
                },
            ))
            .id();
        app.world_mut().entity_mut(mesh_root).add_child(child);

        // Pass 1 : rig
        app.update();
        // Pass 2 : doit no-op (marker AutoRigged présent, NeedsAutoRig retiré)
        app.update();

        let rigged = app.world().get::<AutoRigged>(mesh_root).expect("AutoRigged marker");
        assert!(rigged.bone_count >= 14, "humanoid expected >=14 bones, got {}", rigged.bone_count);
        assert!(app.world().get::<NeedsAutoRig>(mesh_root).is_none(), "NeedsAutoRig should be removed");
    }
}
