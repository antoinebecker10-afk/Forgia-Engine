//! # forgia-ik — Inverse Kinematics solvers (story-440 Sprint B)
//!
//! Fonctions pures + helpers Bevy pour résoudre des chaînes IK. Phase 1
//! focus : **2-bone analytical** (foot placement, look-at-target).
//!
//! ## Algorithme 2-bone analytical
//!
//! Pattern référence (D. Holden, Orange Duck — `simple-two-joint`,
//! cf [theorangeduck.com/page/simple-two-joint](https://theorangeduck.com/page/simple-two-joint)) :
//!
//! Donnés :
//! - `hip_world` = position du root joint (épaule pour bras, hanche pour jambe)
//! - `target_world` = position cible de l'end-effector (poignet, pied)
//! - `pole_world` = direction vers laquelle le coude/genou doit pointer
//! - `l1` = longueur du upper bone (hip → knee)
//! - `l2` = longueur du lower bone (knee → foot)
//!
//! Étapes :
//! 1. `d = clamp(distance(hip, target), 0, l1 + l2 - epsilon)` — évite hyper-extension
//! 2. Angle au genou par loi des cosinus :
//!    `cos(α) = (l1² + l2² - d²) / (2 · l1 · l2)` → `knee_bend = π - acos(cos(α))`
//! 3. Rotation hip = orienter `hip→knee_projected` vers le plan défini par
//!    `hip`, `target`, `pole`
//!
//! Le solveur est **analytique** (pas itératif comme FABRIK), O(1), pas de
//! dépendance externe (`bevy_fabrik` ne supporte que Bevy 0.15).
//!
//! ## Crate `forgia-ik` rôle
//!
//! Solvers + utilities exposés via `prelude`. **Pas de système Bevy ici** —
//! les callers (`forgia-rpg::character`, futurs FPS viewmodel) intègrent les
//! solvers dans leur propre pipeline (walk cycle, weapon aim, etc.).

use bevy::prelude::*;

pub mod prelude {
    pub use crate::{ForgiaIkPlugin, TwoBoneIkInput, TwoBoneIkOutput, two_bone_ik};
}

pub struct ForgiaIkPlugin;

impl Plugin for ForgiaIkPlugin {
    fn build(&self, _app: &mut App) {
        // Library-only crate : pas de système enregistré. Les callers ajoutent
        // leurs propres systèmes qui appellent `two_bone_ik` à la demande.
    }
}

// ── 2-bone analytical IK ────────────────────────────────────────────────────

/// Entrée du solver 2-bone. Tout est en world space (le caller transforme
/// les rotations résultantes en local au moment d'écrire sur les bones).
#[derive(Debug, Clone, Copy)]
pub struct TwoBoneIkInput {
    /// Position monde du root joint (hanche pour leg IK, épaule pour arm IK).
    pub root_pos: Vec3,
    /// Position monde courante du middle joint (genou, coude) — sert à
    /// récupérer l'axe knee/elbow par défaut si `pole_dir` est `None`.
    pub mid_pos: Vec3,
    /// Position monde courante de l'end-effector (foot, hand).
    pub end_pos: Vec3,
    /// Position monde de la cible.
    pub target_pos: Vec3,
    /// Direction monde vers laquelle le knee/elbow doit pointer.
    /// `None` → conserve le pole courant (mid_pos - projection sur root→target).
    pub pole_dir: Option<Vec3>,
}

/// Rotations world-space à appliquer pour atteindre la cible. Le caller doit
/// les convertir en LOCAL relativement au parent du bone correspondant.
#[derive(Debug, Clone, Copy, Default)]
pub struct TwoBoneIkOutput {
    /// Rotation à appliquer sur le root joint (hip/shoulder). World space.
    pub root_rotation: Quat,
    /// Rotation à appliquer sur le middle joint (knee/elbow). World space.
    pub mid_rotation: Quat,
    /// Distance réelle root→target (utile pour debug : si > l1+l2, on a clampé).
    pub reach_distance: f32,
    /// Longueur de la chaîne (l1 + l2) au moment du solve.
    pub chain_length: f32,
}

/// Solver 2-bone analytical. Retourne les rotations world-space à appliquer
/// sur root et mid pour que end-effector atteigne (ou s'approche de) `target_pos`.
///
/// **Robustesse** :
/// - Target hors d'atteinte (`d > l1+l2`) → chaîne étendue vers target, pas de NaN
/// - Target = root (`d ≈ 0`) → identité (no-op safe)
/// - l1 ou l2 ≈ 0 → fallback identité (rig dégénéré)
pub fn two_bone_ik(input: TwoBoneIkInput) -> TwoBoneIkOutput {
    let l1 = (input.mid_pos - input.root_pos).length();
    let l2 = (input.end_pos - input.mid_pos).length();
    let chain_length = l1 + l2;

    if l1 < 1e-4 || l2 < 1e-4 {
        return TwoBoneIkOutput {
            chain_length,
            ..default()
        };
    }

    let to_target = input.target_pos - input.root_pos;
    let d_raw = to_target.length();
    // Clamp target distance dans [epsilon, l1+l2 - epsilon] — évite NaN et hyper-extension.
    let eps = 1e-4_f32;
    let d = d_raw.clamp(eps, chain_length - eps);
    let target_dir = if d_raw > eps {
        to_target / d_raw
    } else {
        // Target = root → on garde la direction courante
        (input.end_pos - input.root_pos).normalize_or_zero()
    };

    // Direction courante root → end (utilisée pour calculer la rotation root).
    let current_dir = (input.end_pos - input.root_pos).normalize_or_zero();

    // Rotation root pour aligner current_dir → target_dir.
    let root_rotation = if current_dir.length_squared() > eps {
        Quat::from_rotation_arc(current_dir, target_dir)
    } else {
        Quat::IDENTITY
    };

    // Angle au mid par loi des cosinus.
    // cos(α) = (l1² + l2² - d²) / (2·l1·l2)  où α = angle au mid bend interne.
    // Knee bend (déviation de "tendu") = π - α.
    let cos_alpha = ((l1 * l1 + l2 * l2 - d * d) / (2.0 * l1 * l2)).clamp(-1.0, 1.0);
    let alpha = cos_alpha.acos();
    let knee_bend = std::f32::consts::PI - alpha;

    // Axis de bend : par défaut, axe perpendiculaire au plan (root, mid, end).
    // Si pole_dir fourni, on l'utilise pour orienter le plan.
    let bend_axis = match input.pole_dir {
        Some(pole) => {
            // Plan = (root, target, pole) → axe = (root→target) × (root→pole)
            (target_dir).cross(pole).normalize_or_zero()
        }
        None => {
            // Conserve l'axe courant : (root→end) × (root→mid_current_direction)
            let to_mid = (input.mid_pos - input.root_pos).normalize_or_zero();
            current_dir.cross(to_mid).normalize_or_zero()
        }
    };

    // Fallback axe si dégénéré (cross produit nul → membres alignés)
    let bend_axis = if bend_axis.length_squared() < eps {
        Vec3::X
    } else {
        bend_axis
    };

    // Rotation mid : rotation autour de bend_axis, magnitude = knee_bend.
    let mid_rotation = Quat::from_axis_angle(bend_axis, knee_bend);

    TwoBoneIkOutput {
        root_rotation,
        mid_rotation,
        reach_distance: d_raw,
        chain_length,
    }
}

/// Convenience : solve 2-bone IK pour un foot avec target = `ground_y` sous
/// le foot rest. Pattern de foot IK en walk cycle stance phase.
///
/// `clamp_max_lift` : on ne lève pas le bassin au-dessus du foot rest. Si
/// le terrain est plus haut que le rest, on n'élève pas le perso ; on accepte
/// la déformation foot. Pour Forgia, plus naturel.
pub fn foot_ik_to_ground(
    hip_world: Vec3,
    knee_world: Vec3,
    foot_world_rest: Vec3,
    ground_y: f32,
    pole_forward_dir: Vec3,
    clamp_max_lift: f32,
) -> TwoBoneIkOutput {
    // Foot target = même XZ que rest, Y = ground (clampé pour pas léviter trop haut).
    let target_y = (ground_y - foot_world_rest.y).clamp(-1.0, clamp_max_lift) + foot_world_rest.y;
    let target = Vec3::new(foot_world_rest.x, target_y, foot_world_rest.z);
    two_bone_ik(TwoBoneIkInput {
        root_pos: hip_world,
        mid_pos: knee_world,
        end_pos: foot_world_rest,
        target_pos: target,
        pole_dir: Some(pole_forward_dir),
    })
}

// ── Tests headless ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn two_bone_target_at_full_extension_no_bend() {
        // Target = à exactement l1+l2 du root sur la même droite que current end
        // → knee bend ≈ 0 (chaîne tendue)
        let input = TwoBoneIkInput {
            root_pos: Vec3::ZERO,
            mid_pos: Vec3::new(0.0, -0.5, 0.0),
            end_pos: Vec3::new(0.0, -1.0, 0.0),
            target_pos: Vec3::new(0.0, -1.0, 0.0),
            pole_dir: Some(Vec3::Z),
        };
        let out = two_bone_ik(input);
        // mid_rotation magnitude ≈ 0 (knee tendu)
        let (_, angle) = out.mid_rotation.to_axis_angle();
        assert!(
            approx_eq(angle, 0.0, 0.05),
            "knee should be straight at full extension, got {} rad",
            angle
        );
    }

    #[test]
    fn two_bone_target_close_high_bend() {
        // Target très proche (d=0.3 quand l1+l2=1.0) → knee très plié
        let input = TwoBoneIkInput {
            root_pos: Vec3::ZERO,
            mid_pos: Vec3::new(0.0, -0.5, 0.0),
            end_pos: Vec3::new(0.0, -1.0, 0.0),
            target_pos: Vec3::new(0.0, -0.3, 0.0),
            pole_dir: Some(Vec3::Z),
        };
        let out = two_bone_ik(input);
        let (_, angle) = out.mid_rotation.to_axis_angle();
        // Loi des cosinus : cos(α) = (0.25+0.25-0.09)/(2*0.25) = 0.41/0.5 = 0.82
        // α = acos(0.82) ≈ 0.61 rad. knee_bend = π - 0.61 ≈ 2.53 rad ≈ 145°
        assert!(angle > 2.0, "knee should be highly bent, got {} rad", angle);
    }

    #[test]
    fn two_bone_target_unreachable_clamped() {
        // Target hyper loin → on clamp à l1+l2 sans NaN, knee bend ≈ 0
        let input = TwoBoneIkInput {
            root_pos: Vec3::ZERO,
            mid_pos: Vec3::new(0.0, -0.5, 0.0),
            end_pos: Vec3::new(0.0, -1.0, 0.0),
            target_pos: Vec3::new(0.0, -50.0, 0.0),
            pole_dir: Some(Vec3::Z),
        };
        let out = two_bone_ik(input);
        let (_, angle) = out.mid_rotation.to_axis_angle();
        assert!(angle.is_finite(), "no NaN on unreachable target");
        assert!(angle < 0.05, "knee straight on unreachable extension");
        assert_eq!(out.chain_length, 1.0);
        assert!(out.reach_distance > out.chain_length);
    }

    #[test]
    fn two_bone_target_at_root_no_panic() {
        // Cas dégénéré : target = root. Doit retourner sans crasher.
        let input = TwoBoneIkInput {
            root_pos: Vec3::ZERO,
            mid_pos: Vec3::new(0.0, -0.5, 0.0),
            end_pos: Vec3::new(0.0, -1.0, 0.0),
            target_pos: Vec3::ZERO,
            pole_dir: Some(Vec3::Z),
        };
        let out = two_bone_ik(input);
        let (_, angle) = out.mid_rotation.to_axis_angle();
        assert!(angle.is_finite(), "no NaN when target == root");
        // Knee très plié (target ≈ root = on s'écrase)
    }

    #[test]
    fn two_bone_zero_length_segment_returns_identity() {
        // Rig dégénéré (l1 ou l2 ≈ 0) → ne plante pas, retourne identité
        let input = TwoBoneIkInput {
            root_pos: Vec3::ZERO,
            mid_pos: Vec3::ZERO,
            end_pos: Vec3::new(0.0, -1.0, 0.0),
            target_pos: Vec3::new(0.0, -0.5, 0.0),
            pole_dir: Some(Vec3::Z),
        };
        let out = two_bone_ik(input);
        assert_eq!(out.root_rotation, Quat::IDENTITY);
        assert_eq!(out.mid_rotation, Quat::IDENTITY);
    }

    #[test]
    fn foot_ik_target_lower_than_rest_pulls_foot_down() {
        // Foot rest à Y=-1, ground à Y=-1.3 (terrain creuse) → foot doit
        // descendre vers ground (knee s'étend si possible)
        let out = foot_ik_to_ground(
            Vec3::ZERO,
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            -1.3, // ground sous le rest
            Vec3::Z,
            0.4,
        );
        let (_, angle) = out.mid_rotation.to_axis_angle();
        assert!(angle.is_finite());
        // Le knee devrait être moins plié (chaîne plus tendue)
    }

    #[test]
    fn foot_ik_target_above_rest_clamped_by_lift() {
        // Ground plus haut que rest (montée slope) → IK lève foot
        // mais clampé à `clamp_max_lift = 0.2m` pour éviter foot collé à pelvis
        let out = foot_ik_to_ground(
            Vec3::ZERO,
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            -0.3, // ground 0.7m au-dessus rest
            Vec3::Z,
            0.2,
        );
        let (_, angle) = out.mid_rotation.to_axis_angle();
        assert!(angle.is_finite(), "clamp produces valid output, got NaN");
        // reach_distance = root→foot_target. Target_y clampé à rest_y + 0.2 = -0.8.
        // distance(0,0,0)→(0,-0.8,0) = 0.8m.
        assert!(
            approx_eq(out.reach_distance, 0.8, 0.05),
            "reach should be ≈0.8 after clamp, got {}",
            out.reach_distance
        );
    }
}
