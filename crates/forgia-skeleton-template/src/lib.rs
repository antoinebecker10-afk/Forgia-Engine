//! # forgia-skeleton-template — Data layer for anatomical skeleton templates
//!
//! Source de vérité unique pour les templates skeleton (Humanoid, BipedLizard,
//! Quadruped, …) chargés depuis `assets/genomes/skeleton_*.toml` via
//! [`forgia_genome_core::Genome<SkeletonTemplate>`]. Conforme au pattern AAA
//! dominant (Unreal USkeleton, Unity Avatar, Godot SkeletonProfile) : asset
//! data unique, code Rust = builder/algo seulement.
//!
//! Story-480 — extraction depuis `forgia-skeleton-embedder`.
//!
//! ## Layers (Larian Generic Behaviour 4-layer)
//!
//! - **Definition** : `assets/genomes/skeleton_*.toml` (source de vérité)
//! - **Framework (cette crate)** : structs + validation + helpers de transformation
//! - **Framework (consumers)** : `forgia-skeleton-embedder` (algo embedding), `forgia-auto-rig` (pipeline)
//!
//! Phase 1 = pure data + fonctions pures.
//! Phase 2 (cette mise à jour) = Plugin + Registry + sensor `forgia2_skeleton_template_registry.json`.

use bevy::prelude::*;
use bevy::reflect::TypePath;
use forgia_genome_core::{Genome, GenomeLoader};
use serde::Deserialize;
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Bone definition
// ─────────────────────────────────────────────────────────────────────────────

/// Classification anatomique déclarative d'un bone (story-481).
///
/// Avant story-481, la classe était inférée par substring matching sur
/// `bone.name` (ex: `n.starts_with("thigh") → Leg`). Ce pattern était dupliqué
/// dans 3 crates et cassait sur tout naming convention différent (Meshy,
/// Mixamo, AccuRig). Pattern AAA : Unreal Skeleton / Unity Avatar / Godot
/// SkeletonProfile déclarent la classe dans l'asset.
///
/// Sérialisation TOML : `class = "leg"` (snake_case).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoneClass {
    /// Chaîne axiale centrale : hip, spine_*, chest, neck. Lock X+Z template.
    Spine,
    /// Crâne / face. Lock X au template, Z et Y libres.
    Head,
    /// Chaîne jambe : thigh, shin, foot. Lock Y au template (knee Y précis).
    Leg,
    /// Chaîne bras : clavicle, arm, forearm, hand. Suit centerline path walking.
    Arm,
    /// Chaîne queue : tail_*. Suit la courbe medial axis.
    Tail,
    /// Non classifié — comportement par défaut (free embedding).
    #[default]
    Other,
}

/// Bone d'un template anatomique. Position normalisée : `(x, y, z)` où
/// `y ∈ [0, 1]` (0=sol, 1=sommet du mesh) et `x, z` en fraction de la
/// hauteur (latéralité).
///
/// **Format TOML** :
/// ```toml
/// [[bones]]
/// name = "hip"
/// pos = [0.0, 0.50, 0.0]
/// class = "spine"
///
/// [[bones]]
/// name = "thigh_L"
/// parent = 0
/// pos = [-0.10, 0.40, 0.0]
/// class = "leg"
/// ```
///
/// Le champ `class` est declarative (story-481). `#[serde(default)]` permet
/// la rétro-compatibilité — un TOML sans `class` parse en `Other`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TemplateBone {
    pub name: String,
    /// Index du parent dans `SkeletonTemplate.bones`. `None` = root.
    pub parent: Option<usize>,
    /// Position normalisée [x, y, z] dans le repère mesh.
    pub pos: [f32; 3],
    /// Classe anatomique declarative (story-481). Default `Other`.
    #[serde(default)]
    pub class: BoneClass,
}

impl TemplateBone {
    /// Helper : position en `Vec3` (compatibilité ergonomique).
    pub fn pos_vec3(&self) -> Vec3 {
        Vec3::from_array(self.pos)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SkeletonTemplate
// ─────────────────────────────────────────────────────────────────────────────

/// Template anatomique : hiérarchie de bones avec positions normalisées.
///
/// Chargeable comme asset Bevy via [`forgia_genome_core::Genome<SkeletonTemplate>`]
/// + `GenomeLoader<SkeletonTemplate>` enregistrés au boot par
///   `SkeletonTemplatePlugin` (story-480 Phase 2).
#[derive(Debug, Clone, Deserialize, TypePath, PartialEq)]
pub struct SkeletonTemplate {
    pub bones: Vec<TemplateBone>,
    /// Story-482 P2 : offsets de stance à appliquer entre la bind pose
    /// (rest pose Pinocchio) et la pose game (ex: bras vertical descendant
    /// pour un mesh T-pose Vitruvian arms-horizontal).
    ///
    /// Format TOML :
    /// ```toml
    /// [stance_offsets]
    /// arm_l_euler_deg = [0.0, 0.0, 90.0]
    /// arm_r_euler_deg = [0.0, 0.0, -90.0]
    /// ```
    ///
    /// Convention : composé en ordre Bevy par-dessus le bind via
    /// `Quat::from_euler(EulerRot::XYZ, x_rad, y_rad, z_rad)` AVANT le swing
    /// du walk cycle. Defaults = identity (mesh déjà en pose game).
    #[serde(default)]
    pub stance_offsets: StanceOffsetsTable,
}

/// Table des offsets de stance par classe anatomique. Read par
/// `forgia-anim-locomotion` pour positionner les bones en pose game
/// par-dessus le bind Pinocchio.
///
/// Default : tous zéros (mesh est déjà dans la pose game souhaitée).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct StanceOffsetsTable {
    /// Offset euler XYZ degrés sur le bone left_arm.
    #[serde(default)]
    pub arm_l_euler_deg: [f32; 3],
    /// Offset euler XYZ degrés sur le bone right_arm.
    #[serde(default)]
    pub arm_r_euler_deg: [f32; 3],
    /// Offset euler XYZ degrés sur le bone left_leg.
    #[serde(default)]
    pub leg_l_euler_deg: [f32; 3],
    /// Offset euler XYZ degrés sur le bone right_leg.
    #[serde(default)]
    pub leg_r_euler_deg: [f32; 3],
    /// Offset euler XYZ degrés sur le bone spine.
    #[serde(default)]
    pub spine_euler_deg: [f32; 3],
    /// Offset euler XYZ degrés sur le bone hip.
    #[serde(default)]
    pub hip_euler_deg: [f32; 3],
    /// Offset euler XYZ degrés sur le bone clavicle_L (shoulder area).
    /// Story-482 P2c (2026-05-21) : clavicle ancre visuellement l'arm si
    /// non animée. Stance offset permet de "dropper" le shoulder vers le
    /// bas pour matcher la pose game arms-down même quand arm_L rotate.
    #[serde(default)]
    pub clavicle_l_euler_deg: [f32; 3],
    /// Offset euler XYZ degrés sur le bone clavicle_R.
    #[serde(default)]
    pub clavicle_r_euler_deg: [f32; 3],
}

// ─────────────────────────────────────────────────────────────────────────────
// Identifiers
// ─────────────────────────────────────────────────────────────────────────────

/// Identifiant typé d'un template skeleton. Utilisé par le Registry
/// (Phase 2) pour indexer les handles chargés.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkeletonTemplateId {
    /// Vitruvian biped — humans, gobelins, orcs, dwarves, celestials.
    Humanoid,
    /// Variante arms-down du Humanoid (story-601) — meshes générés bras le long
    /// du corps (assets IA Meshy/Tripo, ex. Cyber). Bind A-pose + stance à zéro.
    HumanoidApose,
    /// Bipède lézard digitigrade avec queue 4-segments (Rex / T-rex / raptors).
    BipedLizard,
    /// Quadrupède (chevaux, loups) — futur, prévu Phase 5 story-480.
    Quadruped,
}

impl SkeletonTemplateId {
    /// Chemin asset relatif convention `assets/genomes/skeleton_<id>.toml`.
    pub fn asset_path(self) -> &'static str {
        match self {
            Self::Humanoid => "genomes/skeleton_humanoid.toml",
            Self::HumanoidApose => "genomes/skeleton_humanoid_apose.toml",
            Self::BipedLizard => "genomes/skeleton_biped_lizard.toml",
            Self::Quadruped => "genomes/skeleton_quadruped.toml",
        }
    }

    /// Slug snake_case (pour logs + sensor JSON).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Humanoid => "humanoid",
            Self::HumanoidApose => "humanoid_apose",
            Self::BipedLizard => "biped_lizard",
            Self::Quadruped => "quadruped",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Validation
// ─────────────────────────────────────────────────────────────────────────────

/// Erreurs de validation d'un template.
#[derive(Debug, PartialEq)]
pub enum TemplateValidationError {
    /// Le template ne contient aucun bone.
    Empty,
    /// Plus d'un bone est marqué root (`parent = None`).
    MultipleRoots,
    /// Aucun bone n'est marqué root.
    NoRoot,
    /// Un bone référence un parent dont l'index est >= position du bone (viole BFS).
    /// `bone_idx, parent_idx` : le bone à `bone_idx` référence parent à `parent_idx`.
    ForwardReference { bone_idx: usize, parent_idx: usize },
    /// Index parent hors bornes.
    OutOfBoundsParent {
        bone_idx: usize,
        parent_idx: usize,
        total: usize,
    },
    /// Auto-référence (un bone se déclare son propre parent).
    SelfReference { bone_idx: usize },
}

impl std::fmt::Display for TemplateValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "skeleton template is empty"),
            Self::MultipleRoots => write!(
                f,
                "skeleton template has multiple root bones (parent = None)"
            ),
            Self::NoRoot => write!(f, "skeleton template has no root bone (all have parent)"),
            Self::ForwardReference {
                bone_idx,
                parent_idx,
            } => write!(
                f,
                "bone[{}] references parent[{}] declared after it (BFS order violated)",
                bone_idx, parent_idx
            ),
            Self::OutOfBoundsParent {
                bone_idx,
                parent_idx,
                total,
            } => write!(
                f,
                "bone[{}] references parent[{}] but template has only {} bones",
                bone_idx, parent_idx, total
            ),
            Self::SelfReference { bone_idx } => {
                write!(f, "bone[{}] references itself as parent", bone_idx)
            }
        }
    }
}

impl std::error::Error for TemplateValidationError {}

/// Valide la cohérence structurelle d'un template :
///
/// - Au moins 1 bone
/// - Exactement 1 root (`parent = None`)
/// - Tout parent référencé existe et est déclaré AVANT son enfant (invariant BFS)
/// - Pas d'auto-référence
///
/// Source de vérité pour les checks au boot du Registry (Phase 2) et au
/// hot-reload (Shift+F12).
pub fn validate(template: &SkeletonTemplate) -> Result<(), TemplateValidationError> {
    if template.bones.is_empty() {
        return Err(TemplateValidationError::Empty);
    }

    let total = template.bones.len();
    let mut root_count = 0usize;
    for (idx, bone) in template.bones.iter().enumerate() {
        match bone.parent {
            None => root_count += 1,
            Some(p) => {
                if p == idx {
                    return Err(TemplateValidationError::SelfReference { bone_idx: idx });
                }
                if p >= total {
                    return Err(TemplateValidationError::OutOfBoundsParent {
                        bone_idx: idx,
                        parent_idx: p,
                        total,
                    });
                }
                if p >= idx {
                    return Err(TemplateValidationError::ForwardReference {
                        bone_idx: idx,
                        parent_idx: p,
                    });
                }
            }
        }
    }

    match root_count {
        0 => Err(TemplateValidationError::NoRoot),
        1 => Ok(()),
        _ => Err(TemplateValidationError::MultipleRoots),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Transformations
// ─────────────────────────────────────────────────────────────────────────────
//
// Note story-481 : `classify_bone(name: &str)` (substring matching anti-pattern)
// a été supprimé. La classe est désormais lue depuis `bone.class` (déclaratif
// TOML). Conforme pattern AAA — Unreal Skeleton / Unity Avatar / Godot
// SkeletonProfile déclarent la classe dans l'asset, ne l'infèrent pas.

impl SkeletonTemplate {
    /// Clone le template en inversant Z de toutes les positions. Utilisé quand
    /// la détection anatomy (anatomy_detect.tail_in_positive_z) indique que le
    /// mesh Meshy est orienté avec sa queue en +Z (vs convention template
    /// `+Z = forward`).
    pub fn flipped_z(&self) -> Self {
        Self {
            bones: self
                .bones
                .iter()
                .map(|b| TemplateBone {
                    name: b.name.clone(),
                    parent: b.parent,
                    pos: [b.pos[0], b.pos[1], -b.pos[2]],
                    class: b.class,
                })
                .collect(),
            stance_offsets: self.stance_offsets.clone(),
        }
    }

    /// Rescale les positions Y des bones pour matcher les vrais landmarks du
    /// mesh (hip/shoulder/head/arm_span). Garde la HIÉRARCHIE intacte mais
    /// déplace chaque bone à sa position anatomique réelle :
    ///
    /// - Spine chain (spine/chest/neck/head) : interpole entre `hip_y_frac` et
    ///   `head_y_frac` selon position relative dans le template.
    /// - Leg chain (thigh/shin/foot) : interpole entre `hip_y_frac` et `0`
    ///   (sol). Foot atteint le sol pile.
    /// - Arm chain (arm/forearm/clavicle/hand) : Y rescale au shoulder_y_frac
    ///   réel ; X scale selon `arm_span_half_frac` réel.
    /// - Tail chain (tail_*) : Y conservé proche du hip, X/Z préservés.
    pub fn rescaled_for_landmarks(
        &self,
        hip_y_frac: f32,
        shoulder_y_frac: f32,
        head_y_frac: f32,
        arm_span_half_frac: f32,
    ) -> Self {
        self.rescaled_for_landmarks_with_torso(
            hip_y_frac,
            shoulder_y_frac,
            head_y_frac,
            arm_span_half_frac,
            0.0,
            0.0,
        )
    }

    /// Variante de `rescaled_for_landmarks` avec offset torse explicite.
    /// `torso_x_offset_frac` / `torso_z_offset_frac` : offset du centre torse
    /// réel vs AABB.center, en fraction de la hauteur. Shift les os spine/arm
    /// vers ce vrai centre pour matcher l'asymétrie visuelle du mesh.
    #[allow(clippy::too_many_arguments)]
    pub fn rescaled_for_landmarks_with_torso(
        &self,
        hip_y_frac: f32,
        shoulder_y_frac: f32,
        head_y_frac: f32,
        arm_span_half_frac: f32,
        torso_x_offset_frac: f32,
        // 2026-06-03 : ré-activé (lizard) → applique l'offset Z du torse au tronc/
        // bassin/bras/queue, sinon ils restent en arrière du corps mesh (signalé user).
        torso_z_offset_frac: f32,
    ) -> Self {
        let find_y = |needle: &str| -> Option<f32> {
            self.bones
                .iter()
                .find(|b| b.name.contains(needle))
                .map(|b| b.pos[1])
        };
        let template_hip_y = find_y("hip").unwrap_or(0.50);
        let template_head_y = find_y("head").unwrap_or(0.95);
        let template_shoulder_y = find_y("chest")
            .or_else(|| find_y("shoulder"))
            .or_else(|| find_y("neck"))
            .unwrap_or(0.80);

        // story-481 : filtre via bone.class (declarative TOML) au lieu de
        // substring matching `contains("arm"|"hand")`. Robuste à tout naming
        // convention Meshy / Mixamo / AccuRig.
        let template_arm_tip_x_abs = self
            .bones
            .iter()
            .filter(|b| matches!(b.class, BoneClass::Arm))
            .map(|b| b.pos[0].abs())
            .fold(0.10_f32, f32::max);

        let real_chest_y = shoulder_y_frac.clamp(hip_y_frac + 0.05, head_y_frac - 0.05);
        let template_chest_y = template_shoulder_y;

        let leg_scale = (hip_y_frac / template_hip_y.max(0.001)).clamp(0.3, 3.0);
        let arm_scale = (arm_span_half_frac / template_arm_tip_x_abs.max(0.01)).clamp(0.3, 5.0);

        let map_y_spine = |template_y: f32| -> f32 {
            if template_y <= template_chest_y {
                let span = (template_chest_y - template_hip_y).max(0.001);
                let t = ((template_y - template_hip_y) / span).clamp(-0.5, 1.5);
                hip_y_frac + t * (real_chest_y - hip_y_frac)
            } else {
                let span = (template_head_y - template_chest_y).max(0.001);
                let t = ((template_y - template_chest_y) / span).clamp(-0.5, 1.5);
                real_chest_y + t * (head_y_frac - real_chest_y)
            }
        };

        let new_bones: Vec<TemplateBone> = self
            .bones
            .iter()
            .map(|b| {
                // story-481 : classe lue depuis `bone.class` declarative (TOML),
                // plus de classify_bone(name) substring matching.
                let class = b.class;
                let (x, mut y, z) = (b.pos[0], b.pos[1], b.pos[2]);
                y = match class {
                    BoneClass::Spine | BoneClass::Head => map_y_spine(y),
                    BoneClass::Leg => (y * leg_scale).clamp(0.0, hip_y_frac + 0.05),
                    BoneClass::Arm => map_y_spine(y),
                    BoneClass::Tail => {
                        let template_span_total = (template_head_y - template_hip_y).max(0.001);
                        let real_span_total = (head_y_frac - hip_y_frac).max(0.001);
                        let t = ((y - template_hip_y) / template_span_total).clamp(-1.5, 0.5);
                        hip_y_frac + t * real_span_total * 0.6
                    }
                    BoneClass::Other => y,
                };
                let x_scaled = if matches!(class, BoneClass::Arm) {
                    x * arm_scale
                } else {
                    x
                };
                let apply_x_offset = torso_x_offset_frac.abs() > 0.006;
                let x_offset = if apply_x_offset {
                    torso_x_offset_frac
                } else {
                    0.0
                };
                // 2026-06-03 : offset Z du torse → le tronc/bassin/bras/queue suivent
                // le corps mesh avancé (torso_z_off≈0.11). RÉGLER TORSO_Z_FACTOR pour
                // avancer/reculer le bassin (1.0 = plein offset, 0 = aucun). Les jambes
                // gardent leur Z propre (embedder path + biais genou).
                const TORSO_Z_FACTOR: f32 = 0.45;
                let apply_z_offset = torso_z_offset_frac.abs() > 0.006;
                let z_offset = if apply_z_offset {
                    torso_z_offset_frac * TORSO_Z_FACTOR
                } else {
                    0.0
                };
                let (x_final, z_final) = match class {
                    BoneClass::Spine | BoneClass::Head | BoneClass::Arm | BoneClass::Tail => {
                        (x_scaled + x_offset, z + z_offset)
                    }
                    BoneClass::Leg | BoneClass::Other => (x_scaled, z),
                };
                TemplateBone {
                    name: b.name.clone(),
                    parent: b.parent,
                    pos: [x_final, y, z_final],
                    class,
                }
            })
            .collect();

        Self {
            bones: new_bones,
            stance_offsets: self.stance_offsets.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Builders (legacy — fallback runtime supprimé Phase 3 story-480)
// ─────────────────────────────────────────────────────────────────────────────
//
// Ces fonctions reproduisent fidèlement le contenu des TOMLs au 2026-05-20.
// Pendant Phase 1-2, elles servent encore de fallback à `pinocchio_pipeline`
// si le `Genome<SkeletonTemplate>` n'est pas chargé. Phase 3 les déplacera en
// `#[cfg(test)]` fixtures et le pipeline défèrera au lieu de fallback.

impl SkeletonTemplate {
    /// Template humanoid Vitruvian 20 bones (avec hand_L/R).
    /// **Source de vérité** : `assets/genomes/skeleton_humanoid.toml`.
    pub fn humanoid() -> Self {
        use BoneClass::*;
        Self::from_data(&[
            ("hip", None, [0.0, 0.50, 0.0], Spine),
            ("spine_lower", Some(0), [0.0, 0.58, 0.0], Spine),
            ("spine_mid", Some(1), [0.0, 0.66, 0.0], Spine),
            ("chest", Some(2), [0.0, 0.75, 0.0], Spine),
            ("neck", Some(3), [0.0, 0.85, 0.0], Spine),
            ("head", Some(4), [0.0, 0.95, 0.0], Head),
            ("clavicle_L", Some(3), [-0.08, 0.80, 0.0], Arm),
            ("arm_L", Some(6), [-0.25, 0.78, 0.0], Arm),
            ("forearm_L", Some(7), [-0.55, 0.78, 0.0], Arm),
            ("hand_L", Some(8), [-0.85, 0.78, 0.0], Arm),
            ("clavicle_R", Some(3), [0.08, 0.80, 0.0], Arm),
            ("arm_R", Some(10), [0.25, 0.78, 0.0], Arm),
            ("forearm_R", Some(11), [0.55, 0.78, 0.0], Arm),
            ("hand_R", Some(12), [0.85, 0.78, 0.0], Arm),
            ("thigh_L", Some(0), [-0.10, 0.40, 0.0], Leg),
            ("shin_L", Some(14), [-0.10, 0.27, 0.03], Leg),
            ("foot_L", Some(15), [-0.10, 0.02, 0.05], Leg),
            ("thigh_R", Some(0), [0.10, 0.40, 0.0], Leg),
            ("shin_R", Some(17), [0.10, 0.27, 0.03], Leg),
            ("foot_R", Some(18), [0.10, 0.02, 0.05], Leg),
        ])
    }

    /// Template biped lézard (Rex) 25 bones (humanoïde digitigrade à queue).
    /// **Source de vérité** : `assets/genomes/skeleton_biped_lizard.toml`.
    /// 2026-06-04 : `spine_mid` retiré (3 os spine), `chest` re-parenté.
    /// 2026-06-05 : `toe_L/R` (orteil digitigrade) + chaîne bras complète
    /// `clavicule → upper → forearm → main` (best practices). Placement bras
    /// déterministe (embedder `is_arm`, anti-snap-cou). Ordre BFS. Cf TOML.
    pub fn biped_lizard() -> Self {
        use BoneClass::*;
        Self::from_data(&[
            ("hip", None, [0.0, 0.45, 0.05], Spine),
            ("spine_lower", Some(0), [0.0, 0.53, 0.08], Spine),
            ("chest", Some(1), [0.0, 0.74, 0.08], Spine),
            ("neck", Some(2), [0.0, 0.76, 0.06], Spine),
            ("head", Some(3), [0.0, 0.82, 0.10], Head),
            ("clavicle_L", Some(2), [-0.09, 0.73, 0.0], Arm),
            ("arm_L", Some(5), [-0.15, 0.71, 0.0], Arm),
            ("forearm_L", Some(6), [-0.30, 0.71, 0.0], Arm),
            ("hand_L", Some(7), [-0.52, 0.71, 0.0], Arm),
            ("clavicle_R", Some(2), [0.09, 0.73, 0.0], Arm),
            ("arm_R", Some(9), [0.15, 0.71, 0.0], Arm),
            ("forearm_R", Some(10), [0.30, 0.71, 0.0], Arm),
            ("hand_R", Some(11), [0.52, 0.71, 0.0], Arm),
            ("thigh_L", Some(0), [-0.10, 0.37, 0.09], Leg),
            ("shin_L", Some(13), [-0.10, 0.20, 0.12], Leg),
            ("foot_L", Some(14), [-0.10, 0.04, 0.06], Leg),
            ("toe_L", Some(15), [-0.10, 0.01, 0.16], Leg),
            ("thigh_R", Some(0), [0.10, 0.37, 0.09], Leg),
            ("shin_R", Some(17), [0.10, 0.20, 0.12], Leg),
            ("foot_R", Some(18), [0.10, 0.04, 0.06], Leg),
            ("toe_R", Some(19), [0.10, 0.01, 0.16], Leg),
            ("tail_01", Some(0), [0.0, 0.41, -0.12], Tail),
            ("tail_02", Some(21), [0.0, 0.39, -0.22], Tail),
            ("tail_03", Some(22), [0.0, 0.37, -0.32], Tail),
            ("tail_04", Some(23), [0.0, 0.27, -0.42], Tail),
        ])
    }

    /// Helper : construit un template depuis `(name, parent, pos, class)` tuples.
    /// story-481 : signature étendue avec `class: BoneClass` (declarative).
    pub fn from_data(data: &[(&str, Option<usize>, [f32; 3], BoneClass)]) -> Self {
        Self {
            bones: data
                .iter()
                .map(|(name, parent, pos, class)| TemplateBone {
                    name: (*name).to_string(),
                    parent: *parent,
                    pos: *pos,
                    class: *class,
                })
                .collect(),
            // Defaults Humanoid Vitruvian (T-pose mesh) : arms ±90° Z pour
            // ramener vertical. Builders biped_lizard/quadruped overrideront.
            stance_offsets: StanceOffsetsTable {
                arm_l_euler_deg: [0.0, 0.0, 90.0],
                arm_r_euler_deg: [0.0, 0.0, -90.0],
                ..Default::default()
            },
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Registry — Phase 2 (story-480)
// ─────────────────────────────────────────────────────────────────────────────

/// Statut de chargement d'un template skeleton dans le Registry.
///
/// Source de vérité pour `forgia2_skeleton_template_registry.json` (1Hz) et
/// pour le health check (Failed > 5s → warn).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateLoadState {
    /// Handle créé, asset pas encore arrivé du loader.
    Loading,
    /// Asset chargé ET validé.
    Ready,
    /// Asset chargé mais `validate()` a échoué — message d'erreur stocké.
    InvalidStructure(String),
    /// Loader a renvoyé une erreur (IO, parse TOML).
    Failed(String),
}

impl TemplateLoadState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::InvalidStructure(_) => "invalid",
            Self::Failed(_) => "failed",
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// Registry centralisé des templates skeleton chargés depuis
/// `assets/genomes/skeleton_*.toml`.
///
/// Pattern inspiré d'**Epic Data Registry** (lookup typé centralisé).
/// Seule source de vérité runtime pour les consommateurs (`forgia-auto-rig`).
///
/// **Usage** :
/// ```ignore
/// fn my_system(registry: Res<SkeletonTemplateRegistry>) {
///     if let Some(tpl) = registry.try_get(SkeletonTemplateId::Humanoid) {
///         // use tpl
///     } else {
///         // defer 1 frame — pas de fallback hardcoded
///     }
/// }
/// ```
#[derive(Resource, Default)]
pub struct SkeletonTemplateRegistry {
    handles: HashMap<SkeletonTemplateId, Handle<Genome<SkeletonTemplate>>>,
    load_state: HashMap<SkeletonTemplateId, TemplateLoadState>,
    /// Set au moment où on insère le handle, pour calculer "load_state == Failed depuis N secs"
    /// dans le health check.
    requested_at_elapsed: HashMap<SkeletonTemplateId, f32>,
    /// Templates qu'on a tenté de charger mais dont l'asset path est introuvable
    /// (loader retourne error AssetSourceNotFound). Pour sensor missing_files.
    missing_files: Vec<String>,
}

impl SkeletonTemplateRegistry {
    /// Lookup typé : retourne `Some(&SkeletonTemplate)` SEULEMENT si
    /// `Ready` (chargé + validé). Sinon `None` → consumer doit defer.
    pub fn try_get<'a>(
        &self,
        id: SkeletonTemplateId,
        assets: &'a Assets<Genome<SkeletonTemplate>>,
    ) -> Option<&'a SkeletonTemplate> {
        if !self
            .load_state
            .get(&id)
            .map(|s| s.is_ready())
            .unwrap_or(false)
        {
            return None;
        }
        let handle = self.handles.get(&id)?;
        assets.get(handle).map(|g| &g.data)
    }

    /// Statut de chargement pour sensor JSON.
    pub fn load_state(&self, id: SkeletonTemplateId) -> Option<&TemplateLoadState> {
        self.load_state.get(&id)
    }

    /// Handle interne (utile pour AssetEvent matching).
    pub fn handle(&self, id: SkeletonTemplateId) -> Option<&Handle<Genome<SkeletonTemplate>>> {
        self.handles.get(&id)
    }

    /// Liste templates demandés (utile sensor).
    pub fn requested_ids(&self) -> Vec<SkeletonTemplateId> {
        self.handles.keys().copied().collect()
    }

    pub fn missing_files(&self) -> &[String] {
        &self.missing_files
    }
}

/// Set ECS pour le Plugin — placé en `PreUpdate` (avant que les systèmes
/// `Update` consument le Registry).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkeletonTemplateRegistrySet;

/// Plugin Bevy : enregistre l'asset loader `Genome<SkeletonTemplate>`,
/// kick start le chargement des 2 templates par défaut au `Startup`,
/// maintient le `load_state` via AssetEvent watch en `PreUpdate`,
/// écrit le sensor JSON 1Hz en `Update`.
///
/// Convention path : `assets/genomes/skeleton_<id>.toml` (voir
/// `SkeletonTemplateId::asset_path`).
pub struct SkeletonTemplatePlugin {
    /// Templates à pré-charger au Startup. Par défaut : Humanoid + BipedLizard.
    /// Quadruped pas inclus par défaut (TOML absent au 2026-05-20).
    pub preload: Vec<SkeletonTemplateId>,
}

impl Default for SkeletonTemplatePlugin {
    fn default() -> Self {
        Self {
            preload: vec![
                SkeletonTemplateId::Humanoid,
                SkeletonTemplateId::HumanoidApose,
                SkeletonTemplateId::BipedLizard,
            ],
        }
    }
}

impl Plugin for SkeletonTemplatePlugin {
    fn build(&self, app: &mut App) {
        // init_asset + register_asset_loader manuels (évite la contrainte
        // FromReflect du helper `register_genome`, qui forcerait à dériver
        // `Reflect` sur SkeletonTemplate + TemplateBone — pas justifié ici,
        // c'est de la pure data Deserialize).
        app.init_asset::<Genome<SkeletonTemplate>>()
            .register_asset_loader(GenomeLoader::<SkeletonTemplate>::default());
        app.init_resource::<SkeletonTemplateRegistry>();

        let preload = self.preload.clone();
        app.add_systems(
            Startup,
            move |asset_server: Res<AssetServer>,
                  mut registry: ResMut<SkeletonTemplateRegistry>,
                  time: Res<Time>| {
                for id in &preload {
                    let handle: Handle<Genome<SkeletonTemplate>> =
                        asset_server.load(id.asset_path());
                    registry.handles.insert(*id, handle);
                    registry.load_state.insert(*id, TemplateLoadState::Loading);
                    registry
                        .requested_at_elapsed
                        .insert(*id, time.elapsed_secs());
                }
            },
        );

        app.add_systems(
            PreUpdate,
            update_skeleton_registry_load_state.in_set(SkeletonTemplateRegistrySet),
        );

        app.add_systems(Update, sys_write_skeleton_template_registry_sensor);
    }
}

/// Système `PreUpdate` : écoute `AssetEvent<Genome<SkeletonTemplate>>` et met à
/// jour `load_state` (Loading→Ready si validate OK, ou InvalidStructure si KO).
///
/// Bevy 0.18 : `MessageReader` (renommé depuis `EventReader`).
pub fn update_skeleton_registry_load_state(
    mut events: MessageReader<AssetEvent<Genome<SkeletonTemplate>>>,
    assets: Res<Assets<Genome<SkeletonTemplate>>>,
    mut registry: ResMut<SkeletonTemplateRegistry>,
) {
    if events.is_empty() {
        return;
    }
    let pairs: Vec<(SkeletonTemplateId, AssetId<Genome<SkeletonTemplate>>)> = registry
        .handles
        .iter()
        .map(|(id, h)| (*id, h.id()))
        .collect();
    for event in events.read() {
        match event {
            AssetEvent::Added { id }
            | AssetEvent::LoadedWithDependencies { id }
            | AssetEvent::Modified { id } => {
                for (template_id, h_id) in &pairs {
                    if *h_id == *id {
                        let new_state = match assets.get(*id) {
                            Some(genome) => match validate(&genome.data) {
                                Ok(_) => TemplateLoadState::Ready,
                                Err(e) => TemplateLoadState::InvalidStructure(e.to_string()),
                            },
                            None => TemplateLoadState::Loading,
                        };
                        registry.load_state.insert(*template_id, new_state);
                    }
                }
            }
            AssetEvent::Removed { id } | AssetEvent::Unused { id } => {
                for (template_id, h_id) in &pairs {
                    if *h_id == *id {
                        registry
                            .load_state
                            .insert(*template_id, TemplateLoadState::Loading);
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sensor — forgia2_skeleton_template_registry.json (1Hz)
// ─────────────────────────────────────────────────────────────────────────────

/// Filename du sensor JSON. Public pour permettre aux dashboards externes
/// (sensor_reader, dashboards Tauri) de pointer dessus sans dupliquer la string.
pub const SKELETON_TEMPLATE_REGISTRY_SENSOR_FILE: &str = "forgia2_skeleton_template_registry.json";

/// Décide severity + next_step depuis l'état Registry. **Pur** — extrait pour
/// tests headless. Convention next-step Quality Gate (§3 QUALITY_GATE.md).
pub fn severity_for_registry(
    states: &[(SkeletonTemplateId, &TemplateLoadState, f32)],
    now_secs: f32,
) -> (&'static str, String) {
    let mut worst_failed: Option<(SkeletonTemplateId, &str)> = None;
    let mut worst_invalid: Option<(SkeletonTemplateId, &str)> = None;
    let mut stale_loading: Option<SkeletonTemplateId> = None;

    for (id, state, requested_at) in states {
        let age = (now_secs - *requested_at).max(0.0);
        match state {
            TemplateLoadState::Failed(msg) => {
                worst_failed = Some((*id, msg.as_str()));
            }
            TemplateLoadState::InvalidStructure(msg) => {
                worst_invalid = Some((*id, msg.as_str()));
            }
            TemplateLoadState::Loading if age > 5.0 => {
                stale_loading = Some(*id);
            }
            _ => {}
        }
    }

    if let Some((id, msg)) = worst_failed {
        return (
            "warn",
            format!(
                "skeleton template '{}' failed to load: {} \u{2014} check assets/genomes/skeleton_{}.toml exists + is valid TOML",
                id.as_str(), msg, id.as_str()
            ),
        );
    }
    if let Some((id, msg)) = worst_invalid {
        return (
            "warn",
            format!(
                "skeleton template '{}' loaded but failed validation: {} \u{2014} fix bone hierarchy or BFS order in TOML",
                id.as_str(), msg
            ),
        );
    }
    if let Some(id) = stale_loading {
        return (
            "warn",
            format!(
                "skeleton template '{}' stuck Loading > 5s \u{2014} asset_server.load() may have failed silently; verify file at assets/genomes/skeleton_{}.toml",
                id.as_str(), id.as_str()
            ),
        );
    }
    ("ok", String::new())
}

/// Système 1Hz : écrit `forgia2_skeleton_template_registry.json`.
/// Suit pattern Forgia (cf `audio_sensor.rs`).
pub fn sys_write_skeleton_template_registry_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    registry: Res<SkeletonTemplateRegistry>,
    assets: Res<Assets<Genome<SkeletonTemplate>>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let now = time.elapsed_secs();
    let ids = registry.requested_ids();

    // Collect (id, state, requested_at) tuples for severity decision.
    let state_view: Vec<(SkeletonTemplateId, &TemplateLoadState, f32)> = ids
        .iter()
        .filter_map(|id| {
            let st = registry.load_state.get(id)?;
            let req = registry
                .requested_at_elapsed
                .get(id)
                .copied()
                .unwrap_or(0.0);
            Some((*id, st, req))
        })
        .collect();

    let (severity, next_step) = severity_for_registry(&state_view, now);

    // Build templates object manually (no serde_json — pattern Forgia).
    let mut templates_json = String::with_capacity(256);
    templates_json.push('{');
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            templates_json.push(',');
        }
        let state = registry.load_state.get(id);
        let state_str = state.map(|s| s.as_str()).unwrap_or("unknown");
        let bones_count = registry
            .handles
            .get(id)
            .and_then(|h| assets.get(h))
            .map(|g| g.data.bones.len())
            .unwrap_or(0);
        let valid = matches!(state, Some(TemplateLoadState::Ready));
        let error_msg = match state {
            Some(TemplateLoadState::InvalidStructure(m)) | Some(TemplateLoadState::Failed(m)) => {
                m.replace('"', "'").replace('\n', " ")
            }
            _ => String::new(),
        };
        templates_json.push_str(&format!(
            r#""{}":{{"load_state":"{}","bones_count":{},"valid":{},"error":"{}"}}"#,
            id.as_str(),
            state_str,
            bones_count,
            valid,
            error_msg,
        ));
    }
    templates_json.push('}');

    let missing_files_json = registry
        .missing_files()
        .iter()
        .map(|s| format!("\"{}\"", s.replace('"', "'")))
        .collect::<Vec<_>>()
        .join(",");

    let next_step_escaped = next_step.replace('"', "'");

    let json = format!(
        r#"{{"id":"skeleton_template_registry","severity":"{severity}","next_step":"{next_step_escaped}","timestamp_secs":{:.1},"templates_requested":{},"templates":{},"missing_files":[{}]}}"#,
        now,
        ids.len(),
        templates_json,
        missing_files_json,
    );

    if let Err(e) = std::fs::write(SKELETON_TEMPLATE_REGISTRY_SENSOR_FILE, &json) {
        warn!("[forgia-skeleton-template] sensor write failed: {e}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanoid_has_20_bones_with_root_hip() {
        let t = SkeletonTemplate::humanoid();
        assert_eq!(
            t.bones.len(),
            20,
            "humanoid should have 20 bones (with hand_L/R)"
        );
        assert_eq!(t.bones[0].name, "hip");
        assert!(t.bones[0].parent.is_none(), "hip must be root");
    }

    #[test]
    fn biped_lizard_has_25_bones_with_root_hip() {
        let t = SkeletonTemplate::biped_lizard();
        // 3 spine + 2 head/neck + 8 bras (clav+upper+fore+main ×2) + 8 jambes
        // (thigh+shin+foot+toe ×2) + 4 tail = 25.
        assert_eq!(t.bones.len(), 25, "biped_lizard = 25 os (humanoïde complet)");
        assert_eq!(t.bones[0].name, "hip");
        assert!(t.bones[0].parent.is_none());
        assert!(!t.bones.iter().any(|b| b.name == "spine_mid"));
        let idx = |n: &str| t.bones.iter().position(|b| b.name == n).unwrap();
        assert_eq!(t.bones[idx("chest")].parent, Some(idx("spine_lower")));
        // Chaîne bras complète : clavicule → upper → forearm → main.
        assert_eq!(t.bones[idx("clavicle_L")].parent, Some(idx("chest")));
        assert_eq!(t.bones[idx("arm_L")].parent, Some(idx("clavicle_L")));
        assert_eq!(t.bones[idx("forearm_L")].parent, Some(idx("arm_L")));
        assert_eq!(t.bones[idx("hand_L")].parent, Some(idx("forearm_L")));
        // Jambe : toe parenté sur foot (digitigrade).
        assert_eq!(t.bones[idx("toe_L")].parent, Some(idx("foot_L")));
    }

    #[test]
    fn validate_accepts_humanoid_and_lizard() {
        validate(&SkeletonTemplate::humanoid()).expect("humanoid valid");
        validate(&SkeletonTemplate::biped_lizard()).expect("biped_lizard valid");
    }

    #[test]
    fn validate_rejects_empty() {
        let t = SkeletonTemplate {
            bones: vec![],
            stance_offsets: Default::default(),
        };
        assert_eq!(validate(&t), Err(TemplateValidationError::Empty));
    }

    #[test]
    fn validate_rejects_no_root() {
        let t = SkeletonTemplate {
            bones: vec![
                TemplateBone {
                    name: "a".into(),
                    parent: Some(1),
                    pos: [0.0; 3],
                    class: BoneClass::Other,
                },
                TemplateBone {
                    name: "b".into(),
                    parent: Some(0),
                    pos: [0.0; 3],
                    class: BoneClass::Other,
                },
            ],
            stance_offsets: Default::default(),
        };
        // Forward ref detected first.
        assert!(matches!(
            validate(&t),
            Err(TemplateValidationError::ForwardReference { .. })
        ));
    }

    #[test]
    fn validate_rejects_multiple_roots() {
        let t = SkeletonTemplate {
            bones: vec![
                TemplateBone {
                    name: "a".into(),
                    parent: None,
                    pos: [0.0; 3],
                    class: BoneClass::Other,
                },
                TemplateBone {
                    name: "b".into(),
                    parent: None,
                    pos: [0.0; 3],
                    class: BoneClass::Other,
                },
            ],
            stance_offsets: Default::default(),
        };
        assert_eq!(validate(&t), Err(TemplateValidationError::MultipleRoots));
    }

    #[test]
    fn validate_rejects_self_reference() {
        let t = SkeletonTemplate {
            bones: vec![TemplateBone {
                name: "a".into(),
                parent: Some(0),
                pos: [0.0; 3],
                class: BoneClass::Other,
            }],
            stance_offsets: Default::default(),
        };
        assert_eq!(
            validate(&t),
            Err(TemplateValidationError::SelfReference { bone_idx: 0 })
        );
    }

    #[test]
    fn validate_rejects_forward_reference() {
        // bone[0] references bone[1] which is declared after.
        let t = SkeletonTemplate {
            bones: vec![
                TemplateBone {
                    name: "a".into(),
                    parent: Some(1),
                    pos: [0.0; 3],
                    class: BoneClass::Other,
                },
                TemplateBone {
                    name: "b".into(),
                    parent: None,
                    pos: [0.0; 3],
                    class: BoneClass::Other,
                },
            ],
            stance_offsets: Default::default(),
        };
        assert!(matches!(
            validate(&t),
            Err(TemplateValidationError::ForwardReference {
                bone_idx: 0,
                parent_idx: 1
            })
        ));
    }

    #[test]
    fn validate_rejects_out_of_bounds_parent() {
        let t = SkeletonTemplate {
            bones: vec![
                TemplateBone {
                    name: "a".into(),
                    parent: None,
                    pos: [0.0; 3],
                    class: BoneClass::Other,
                },
                TemplateBone {
                    name: "b".into(),
                    parent: Some(99),
                    pos: [0.0; 3],
                    class: BoneClass::Other,
                },
            ],
            stance_offsets: Default::default(),
        };
        assert!(matches!(
            validate(&t),
            Err(TemplateValidationError::OutOfBoundsParent { .. })
        ));
    }

    #[test]
    fn rescale_humanoid_hip_and_head_match_target_fractions() {
        let t = SkeletonTemplate::humanoid();
        let rescaled = t.rescaled_for_landmarks(0.42, 0.67, 0.95, 0.50);

        let hip = rescaled.bones.iter().find(|b| b.name == "hip").unwrap();
        assert!(
            (hip.pos[1] - 0.42).abs() < 0.01,
            "hip should map to 0.42 got {}",
            hip.pos[1]
        );

        let head = rescaled.bones.iter().find(|b| b.name == "head").unwrap();
        assert!(
            (head.pos[1] - 0.95).abs() < 0.01,
            "head should map to 0.95 got {}",
            head.pos[1]
        );
    }

    #[test]
    fn rescale_lizard_keeps_tail_extended_backward() {
        let t = SkeletonTemplate::biped_lizard();
        let rescaled = t.rescaled_for_landmarks(0.42, 0.67, 0.95, 0.40);
        let tail4 = rescaled.bones.iter().find(|b| b.name == "tail_04").unwrap();
        assert!(
            tail4.pos[2] < -0.30,
            "tail_04 z must stay backward, got {}",
            tail4.pos[2]
        );
    }

    #[test]
    fn flipped_z_negates_all_z_positions() {
        let t = SkeletonTemplate::biped_lizard();
        let flipped = t.flipped_z();
        assert_eq!(t.bones.len(), flipped.bones.len());
        for (orig, flip) in t.bones.iter().zip(flipped.bones.iter()) {
            assert_eq!(orig.name, flip.name);
            assert_eq!(orig.parent, flip.parent);
            assert_eq!(orig.pos[0], flip.pos[0]);
            assert_eq!(orig.pos[1], flip.pos[1]);
            assert!(
                (orig.pos[2] + flip.pos[2]).abs() < 1e-6,
                "flip should negate z: {} vs {}",
                orig.pos[2],
                flip.pos[2]
            );
        }
    }

    #[test]
    fn skeleton_template_id_asset_path_convention() {
        assert_eq!(
            SkeletonTemplateId::Humanoid.asset_path(),
            "genomes/skeleton_humanoid.toml"
        );
        assert_eq!(
            SkeletonTemplateId::BipedLizard.asset_path(),
            "genomes/skeleton_biped_lizard.toml"
        );
        assert_eq!(
            SkeletonTemplateId::Quadruped.asset_path(),
            "genomes/skeleton_quadruped.toml"
        );
    }

    // ── Phase 2 Registry tests ──────────────────────────────────────────────

    #[test]
    fn load_state_str_mapping_stable() {
        assert_eq!(TemplateLoadState::Loading.as_str(), "loading");
        assert_eq!(TemplateLoadState::Ready.as_str(), "ready");
        assert_eq!(
            TemplateLoadState::InvalidStructure("x".into()).as_str(),
            "invalid"
        );
        assert_eq!(TemplateLoadState::Failed("y".into()).as_str(), "failed");
    }

    #[test]
    fn load_state_is_ready_only_when_ready() {
        assert!(!TemplateLoadState::Loading.is_ready());
        assert!(TemplateLoadState::Ready.is_ready());
        assert!(!TemplateLoadState::InvalidStructure("x".into()).is_ready());
        assert!(!TemplateLoadState::Failed("y".into()).is_ready());
    }

    #[test]
    fn severity_ok_when_all_ready() {
        let ready = TemplateLoadState::Ready;
        let states = vec![
            (SkeletonTemplateId::Humanoid, &ready, 0.0),
            (SkeletonTemplateId::BipedLizard, &ready, 0.0),
        ];
        let (sev, msg) = severity_for_registry(&states, 1.0);
        assert_eq!(sev, "ok");
        assert!(msg.is_empty());
    }

    #[test]
    fn severity_ok_when_recent_loading() {
        // Loading age < 5s → not stale → still ok.
        let loading = TemplateLoadState::Loading;
        let states = vec![(SkeletonTemplateId::Humanoid, &loading, 0.0)];
        let (sev, _) = severity_for_registry(&states, 2.0);
        assert_eq!(sev, "ok");
    }

    #[test]
    fn severity_warn_when_stale_loading_over_5s() {
        let loading = TemplateLoadState::Loading;
        let states = vec![(SkeletonTemplateId::Humanoid, &loading, 0.0)];
        let (sev, msg) = severity_for_registry(&states, 6.0);
        assert_eq!(sev, "warn");
        assert!(msg.contains("Loading > 5s"), "got: {msg}");
        assert!(msg.contains("humanoid"));
        assert!(
            msg.contains("assets/genomes/skeleton_humanoid.toml"),
            "next-step should point to actionable file path: {msg}"
        );
    }

    #[test]
    fn severity_warn_on_failed_with_path_hint() {
        let failed = TemplateLoadState::Failed("io: file not found".into());
        let states = vec![(SkeletonTemplateId::BipedLizard, &failed, 0.0)];
        let (sev, msg) = severity_for_registry(&states, 1.0);
        assert_eq!(sev, "warn");
        assert!(msg.contains("failed to load"));
        assert!(msg.contains("biped_lizard"));
    }

    #[test]
    fn severity_warn_on_invalid_structure() {
        let invalid = TemplateLoadState::InvalidStructure(
            "bone[0] references parent[1] declared after it".into(),
        );
        let states = vec![(SkeletonTemplateId::Humanoid, &invalid, 0.0)];
        let (sev, msg) = severity_for_registry(&states, 1.0);
        assert_eq!(sev, "warn");
        assert!(msg.contains("failed validation"));
        assert!(msg.contains("BFS order"));
    }

    #[test]
    fn severity_failed_takes_priority_over_loading() {
        let loading = TemplateLoadState::Loading;
        let failed = TemplateLoadState::Failed("io".into());
        let states = vec![
            (SkeletonTemplateId::Humanoid, &loading, 0.0),
            (SkeletonTemplateId::BipedLizard, &failed, 0.0),
        ];
        let (sev, msg) = severity_for_registry(&states, 10.0);
        assert_eq!(sev, "warn");
        assert!(
            msg.contains("biped_lizard"),
            "failed dominates stale loading"
        );
        assert!(msg.contains("failed to load"));
    }

    #[test]
    fn registry_default_empty() {
        let r = SkeletonTemplateRegistry::default();
        assert!(r.requested_ids().is_empty());
        assert!(r.missing_files().is_empty());
        assert!(r.load_state(SkeletonTemplateId::Humanoid).is_none());
    }

    #[test]
    fn sensor_filename_canonical() {
        assert_eq!(
            SKELETON_TEMPLATE_REGISTRY_SENSOR_FILE, "forgia2_skeleton_template_registry.json",
            "filename must match Forgia2 sensor convention (forgia2_<feature>.json)"
        );
    }

    // ── Phase 4 — Test régression cross-crate (TOML filesystem ↔ Rust builder) ─

    /// Parse un TOML genome depuis le workspace `assets/genomes/` et le
    /// décrypte en `SkeletonTemplate`. Helper test seulement.
    fn parse_workspace_toml(filename: &str) -> SkeletonTemplate {
        // CARGO_MANIFEST_DIR = `<workspace>/crates/forgia-skeleton-template`
        // → workspace assets at `../../assets/genomes/<filename>`.
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/genomes")
            .join(filename);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
        toml::from_str::<SkeletonTemplate>(&content)
            .unwrap_or_else(|e| panic!("failed to parse {path:?}: {e}"))
    }

    /// Compare bone-par-bone : noms identiques + parent identique + pos
    /// avec tolérance flottante 1e-6. Échoue avec un message ciblé sur le
    /// premier bone qui diverge.
    fn assert_templates_match(a: &SkeletonTemplate, b: &SkeletonTemplate, label: &str) {
        assert_eq!(
            a.bones.len(),
            b.bones.len(),
            "{label}: bone count mismatch (toml={} vs builder={})",
            a.bones.len(),
            b.bones.len()
        );
        for (i, (ba, bb)) in a.bones.iter().zip(b.bones.iter()).enumerate() {
            assert_eq!(ba.name, bb.name, "{label}: bone[{i}] name mismatch");
            assert_eq!(
                ba.parent, bb.parent,
                "{label}: bone[{i}] '{}' parent mismatch (toml={:?} vs builder={:?})",
                ba.name, ba.parent, bb.parent
            );
            for axis in 0..3 {
                let delta = (ba.pos[axis] - bb.pos[axis]).abs();
                assert!(
                    delta < 1e-6,
                    "{label}: bone[{i}] '{}' pos[{axis}] mismatch (toml={} vs builder={}, delta={})",
                    ba.name, ba.pos[axis], bb.pos[axis], delta
                );
            }
        }
    }

    /// Régression : si quelqu'un modifie le TOML humanoid (e.g. tune une
    /// proportion via Shift+F12, fix un bone parent), le builder Rust
    /// `humanoid()` DOIT être sync. Sinon les tests headless cross-crate
    /// (qui utilisent le builder car pas d'AssetServer) divergent de la
    /// vérité runtime (qui utilise le TOML).
    ///
    /// Origin : story-480 §3 critère acceptance — "Empêche tout futur drift
    /// entre fixture test et TOML".
    #[test]
    fn assert_humanoid_toml_matches_builder_fixture() {
        let toml_template = parse_workspace_toml("skeleton_humanoid.toml");
        let builder_template = SkeletonTemplate::humanoid();
        assert_templates_match(&toml_template, &builder_template, "humanoid");
    }

    #[test]
    fn assert_biped_lizard_toml_matches_builder_fixture() {
        let toml_template = parse_workspace_toml("skeleton_biped_lizard.toml");
        let builder_template = SkeletonTemplate::biped_lizard();
        assert_templates_match(&toml_template, &builder_template, "biped_lizard");
    }

    /// Méta-régression : tout TOML skeleton dans `assets/genomes/skeleton_*.toml`
    /// doit passer la validation structurelle (BFS, root unique, parents
    /// in-range). Empêche un commit qui shippe un asset corrompu — l'asset
    /// serait chargé à runtime, validé par le Registry, et bloquerait Rex en
    /// `InvalidStructure` jusqu'à fix.
    #[test]
    fn all_shipped_skeleton_tomls_pass_validate() {
        for filename in ["skeleton_humanoid.toml", "skeleton_biped_lizard.toml"] {
            let t = parse_workspace_toml(filename);
            validate(&t).unwrap_or_else(|e| panic!("{filename} fails validate(): {e}"));
        }
    }
}
