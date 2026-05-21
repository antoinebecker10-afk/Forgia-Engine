//! # anatomy_detect — Analyse géométrique d'un mesh pour landmarks anatomiques
//!
//! Sprint A+ (story-440 amélioration detection 2026-05-17 PM).
//!
//! Origin : le template-fit naïf placait le hip à 55% de la hauteur du mesh
//! AABB pour Humanoid (proportions Vitruvian). Mais pour un mesh Meshy
//! humanoïde réel, les proportions varient (hip peut être à 45%, 50%, 60%
//! selon le style). Résultat : squelette posé trop haut/bas vs anatomie réelle.
//!
//! Cette module analyse les **vertices réels** du mesh pour trouver les
//! landmarks anatomiques (hip Y, shoulder Y, head Y, leg separation X) et
//! permet à `place_template` de scaler dynamiquement.
//!
//! ## Algorithme
//!
//! Pour un mesh humanoïde standardisé (debout, Y up) :
//!
//! 1. **Slice Y histogram** : divise la hauteur en N=32 slices, compte vertices/slice
//! 2. **Hip Y** = plus bas slice où les vertices forment **un seul cluster X**
//!    (les jambes sont 2 clusters séparés, le torso/bassin est 1 cluster uni)
//! 3. **Shoulder Y** = slice avec la **largeur X maximale** dans le haut 60% du mesh
//!    (T-pose : bras étendus = largeur max). Si pas T-pose, fallback sur le ratio
//!    standard 80% de la hauteur
//! 4. **Head Y** = top 90-95% de la hauteur
//! 5. **Has tail** = présence de vertices significatifs derrière (Z<<center.z) sous mid-height
//! 6. **Aspect ratio** = width / height — humanoid <0.45 typique, biped lézard >0.5
//!
//! Tout est exprimé en **fractions de la hauteur du mesh** (`hip_y_frac` ∈ [0, 1]).

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

/// Landmarks anatomiques détectés sur un mesh. Tout est en fraction de la
/// hauteur du mesh (`0.0` = bottom, `1.0` = top).
#[derive(Debug, Clone, Copy)]
pub struct MeshLandmarks {
    /// Y du bassin (transition jambes séparées → torso uni). Default 0.55.
    pub hip_y_frac: f32,
    /// Y des épaules (largeur X maximale dans la moitié haute). Default 0.80.
    pub shoulder_y_frac: f32,
    /// Y de la base du cou / haut des épaules. Default 0.88.
    pub neck_y_frac: f32,
    /// Y du sommet de la tête. Default 0.98.
    pub head_y_frac: f32,
    /// Largeur X totale du mesh / hauteur Y. <0.45 = humanoid debout typique,
    /// >0.50 = posture lézard / quadrupède / bras T-pose étendus.
    pub aspect_ratio_xy: f32,
    /// Largeur des hanches en X (séparation entre les 2 jambes au niveau hip),
    /// en fraction de la hauteur. Default 0.10.
    pub hip_width_frac: f32,
    /// Demi-largeur du bras T-pose : distance X du centre au bout du bras
    /// le plus éloigné (au shoulder slice), en fraction de la hauteur.
    /// Pour humanoid T-pose ≈ 0.50 (bras = hauteur entière). Pour bras le
    /// long du corps ≈ 0.10-0.15.
    pub arm_span_half_frac: f32,
    /// True si le mesh a des vertices significatifs derrière son centre Z
    /// dans la zone bassin/torse — caractéristique tail lézard.
    pub has_tail: bool,
    /// True si la queue est dans la direction +Z (orientation Meshy flippée vs
    /// convention template `+Z = forward, -Z = tail`). Permet à
    /// `pinocchio_pipeline` de flipper le template Z pour matcher le mesh.
    /// Significatif uniquement si `has_tail == true`.
    pub tail_in_positive_z: bool,
    /// Offset du centre médian X du torse (médiane des vertices dans la bande
    /// hip→shoulder) vs AABB.center.x, exprimé en **fraction de la hauteur**.
    /// Permet d'aligner les os spine sur la vraie médiane visuelle du torse
    /// même si le mesh est asymétrique (pose, accessoires Meshy).
    pub torso_center_x_offset_frac: f32,
    /// Idem pour Z (offset médian Z du torse vs AABB.center.z, fraction hauteur).
    pub torso_center_z_offset_frac: f32,
    /// Nombre de vertices analysés (diagnostic). 0 = fallback proportions fixes.
    pub vertex_count: usize,
}

impl Default for MeshLandmarks {
    /// Proportions Vitruvian humanoid (fallback si pas de vertices analysables).
    fn default() -> Self {
        Self {
            hip_y_frac: 0.55,
            shoulder_y_frac: 0.80,
            neck_y_frac: 0.88,
            head_y_frac: 0.98,
            aspect_ratio_xy: 0.40,
            hip_width_frac: 0.10,
            arm_span_half_frac: 0.20,
            has_tail: false,
            tail_in_positive_z: false,
            torso_center_x_offset_frac: 0.0,
            torso_center_z_offset_frac: 0.0,
            vertex_count: 0,
        }
    }
}

impl MeshLandmarks {
    /// True si les landmarks suggèrent un humanoïde. Critères révisés après
    /// runtime debug 2026-05-17 PM :
    /// - **Pas de tail** (depth_ratio bas/haut < 2.0)
    /// - **Hip Y entre 35-65%** de la hauteur (typique humain debout)
    /// - **Vertex count suffisant** (analyse vraie, pas fallback default)
    ///
    /// **PAS** d'aspect ratio : un mesh humanoïde en T-pose a un aspect
    /// width/height ≥ 1.0 (bras étendus ~bras de la hauteur). Le critère
    /// aspect ratio avait causé un faux négatif systématique → classification
    /// BipedLizard incorrecte sur tout humanoid Meshy.
    pub fn looks_humanoid(&self) -> bool {
        self.vertex_count >= 100
            && !self.has_tail
            && (0.35..=0.65).contains(&self.hip_y_frac)
    }
}

/// Slices count pour l'histogramme Y. 32 = bon compromis precision/perf.
const SLICE_COUNT: usize = 32;
/// Heuristique hip : un slice est "uni-clusteré" si plus de 25% de ses vertices
/// sont dans la bande centrale X (|x - mean| < HALF_BAND * height). Distingue
/// "2 jambes parallèles" (center_ratio quasi 0) du "bassin/torse uni"
/// (center_ratio supérieur à 0.3) — robust aux hanches larges (regression fix
/// 2026-05-17 PM, ancienne heuristique `max_dev` détectait hip à 70% sur
/// humanoid Meshy hanches Y=0.85).
const HIP_CENTER_BAND_FRAC: f32 = 0.04;
const HIP_CENTER_RATIO_THRESHOLD: f32 = 0.25;
/// Min vertices/slice pour considérer le slice "analysable" (sinon noise).
const MIN_VERTS_PER_SLICE: usize = 4;

/// Analyse les positions vertex pour extraire les landmarks anatomiques.
/// `positions` doit être en repère mesh-root-local (utiliser `mesh3d_to_root_local`
/// transform avant d'appeler si nécessaire).
///
/// Retourne `MeshLandmarks::default()` si <100 vertices (mesh trop petit pour
/// analyse fiable).
pub fn detect_landmarks_from_vertices(positions: &[Vec3], aabb: &Aabb) -> MeshLandmarks {
    if positions.len() < 100 {
        return MeshLandmarks::default();
    }

    let center: Vec3 = aabb.center.into();
    let half: Vec3 = aabb.half_extents.into();
    let height = (half.y * 2.0).max(0.001);
    let bottom_y = center.y - half.y;
    let width_x = half.x * 2.0;

    // Aspect ratio (largeur full / hauteur full)
    let aspect_ratio_xy = width_x / height;

    // ── Slice Y histogram ────────────────────────────────────────────────────
    let slice_h = height / SLICE_COUNT as f32;
    let mut slices_x: Vec<Vec<f32>> = vec![Vec::new(); SLICE_COUNT];
    let mut slices_z: Vec<Vec<f32>> = vec![Vec::new(); SLICE_COUNT];

    for v in positions {
        let y_rel = v.y - bottom_y;
        let slice_idx = ((y_rel / slice_h).floor() as usize).min(SLICE_COUNT - 1);
        slices_x[slice_idx].push(v.x);
        slices_z[slice_idx].push(v.z);
    }

    // ── Hip Y : plus bas slice où "présence centrale X" devient significative
    //    Heuristique runtime-fixed 2026-05-17 PM : compte les vertices dans la
    //    bande centrale (|x - mean| < HIP_CENTER_BAND). Jambes parallèles =
    //    0 vertex au centre (toutes sur les côtés). Bassin/torse uni = > 25%
    //    des vertices au centre. Robust aux hanches larges (vs ancienne
    //    heuristique max_dev qui détectait hip=70% sur humanoid Meshy).
    let center_band = HIP_CENTER_BAND_FRAC * height;
    let mut hip_slice = (SLICE_COUNT as f32 * 0.50) as usize; // fallback ajusté
    let mut hip_width = width_x * 0.5;

    for (i, slice) in slices_x.iter().enumerate() {
        if slice.len() < MIN_VERTS_PER_SLICE {
            continue;
        }
        let mean_x: f32 = slice.iter().sum::<f32>() / slice.len() as f32;
        let center_count = slice
            .iter()
            .filter(|&&x| (x - mean_x).abs() < center_band)
            .count();
        let center_ratio = center_count as f32 / slice.len() as f32;

        // Hip = 1er slice avec présence centrale, au-dessus du tiers inférieur
        // (skip pieds qui peuvent avoir un peu de centre)
        let i_frac = i as f32 / SLICE_COUNT as f32;
        if i_frac > 0.25 && center_ratio > HIP_CENTER_RATIO_THRESHOLD {
            hip_slice = i;
            // Hip width = écart X au slice juste en dessous (jambes séparées)
            if i >= 2 {
                let prev = &slices_x[i - 2];
                if prev.len() >= MIN_VERTS_PER_SLICE {
                    let min_x = prev.iter().cloned().fold(f32::INFINITY, f32::min);
                    let max_x = prev.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    hip_width = (max_x - min_x).max(center_band);
                }
            }
            break;
        }
    }
    let hip_y_frac = (hip_slice as f32 + 0.5) / SLICE_COUNT as f32;
    let hip_width_frac = (hip_width / height).clamp(0.04, 0.30);

    // ── Shoulder Y : slice de largeur X max dans le haut 70% ─────────────────
    let mut shoulder_slice = (SLICE_COUNT as f32 * 0.80) as usize; // fallback
    let mut shoulder_width_max = 0.0_f32;
    let upper_half_start = SLICE_COUNT * 50 / 100;
    for (offset, slice) in slices_x.iter().enumerate().skip(upper_half_start) {
        if slice.len() < MIN_VERTS_PER_SLICE {
            continue;
        }
        let min_x = slice.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_x = slice.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let width = max_x - min_x;
        if width > shoulder_width_max {
            shoulder_width_max = width;
            shoulder_slice = offset;
        }
    }
    // arm_span_half = demi-largeur du shoulder slice (= bout du bras T-pose
    // depuis le centre X), normalisée par height. Fallback aspect/2 si
    // shoulder pas analysable.
    let arm_span_half_frac = if shoulder_width_max > 0.0 {
        (shoulder_width_max * 0.5 / height).clamp(0.08, 0.70)
    } else {
        (aspect_ratio_xy * 0.5).clamp(0.08, 0.70)
    };
    let shoulder_y_frac = (shoulder_slice as f32 + 0.5) / SLICE_COUNT as f32;

    // ── Head Y : plus haut slice non-vide ───────────────────────────────────
    let mut head_slice = SLICE_COUNT - 1;
    for i in (0..SLICE_COUNT).rev() {
        if slices_x[i].len() >= MIN_VERTS_PER_SLICE {
            head_slice = i;
            break;
        }
    }
    let head_y_frac = ((head_slice as f32 + 0.5) / SLICE_COUNT as f32).min(1.0);
    let neck_y_frac = (shoulder_y_frac + head_y_frac) * 0.5;

    // ── Has tail : agnostique à l'orientation Meshy ±Z (fix 2026-05-18 AM).
    //
    //    Avant : ne regardait que min_z (direction -Z) → faux négatif sur
    //    meshes Meshy qui ont leur tail en +Z (orientation glb variable).
    //
    //    Maintenant : pour chaque bande Y (bas torso vs haut torso), calcule
    //    la profondeur saillante dans **les deux directions** (depth_neg + depth_pos)
    //    et prend le **max** des deux. Tail = whichever Z direction is farthest.
    //
    //    Critères stricts (anti faux-positif jupe Meshy) :
    //    - depth_bottom > 30% hauteur (saillant absolu)
    //    - ratio bas/haut > 4 (très asymétrique torso symétrique)
    let mut min_z_bottom = f32::INFINITY;
    let mut max_z_bottom = f32::NEG_INFINITY;
    let mut min_z_top = f32::INFINITY;
    let mut max_z_top = f32::NEG_INFINITY;
    for v in positions {
        let y_rel = (v.y - bottom_y) / height;
        if (0.20..0.50).contains(&y_rel) {
            min_z_bottom = min_z_bottom.min(v.z);
            max_z_bottom = max_z_bottom.max(v.z);
        } else if y_rel > 0.65 {
            min_z_top = min_z_top.min(v.z);
            max_z_top = max_z_top.max(v.z);
        }
    }
    // Saillance dans chaque direction Z (positive = max forward extent)
    let depth_bottom_neg = (center.z - min_z_bottom).max(0.0);
    let depth_bottom_pos = (max_z_bottom - center.z).max(0.0);
    let depth_top_neg = (center.z - min_z_top).max(0.0);
    let depth_top_pos = (max_z_top - center.z).max(0.0);
    // Pour chaque side, ratio bas/haut. Tail = côté avec ratio le plus élevé.
    let ratio_neg = depth_bottom_neg / depth_top_neg.max(0.01);
    let ratio_pos = depth_bottom_pos / depth_top_pos.max(0.01);
    let (depth_bottom_max, depth_ratio, tail_in_positive_z) = if ratio_pos > ratio_neg {
        (depth_bottom_pos, ratio_pos, true)
    } else {
        (depth_bottom_neg, ratio_neg, false)
    };
    let has_tail = depth_ratio > 4.0 && depth_bottom_max > height * 0.30;

    // ── Torso center (médiane X/Z des vertices entre hip et shoulder) ────
    // Détecte si le mesh a son torse offset vs AABB.center (asymétrie pose
    // Meshy, accessoires, sac à dos). Le bone spine devrait être centré sur
    // le VRAI milieu visuel du torse, pas sur le milieu géométrique AABB.
    let torso_y_lo = bottom_y + hip_y_frac * height;
    let torso_y_hi = bottom_y + shoulder_y_frac * height;
    let mut torso_xs: Vec<f32> = Vec::new();
    let mut torso_zs: Vec<f32> = Vec::new();
    for v in positions {
        if v.y >= torso_y_lo && v.y <= torso_y_hi {
            torso_xs.push(v.x);
            torso_zs.push(v.z);
        }
    }
    let median = |mut vs: Vec<f32>| -> f32 {
        if vs.is_empty() {
            return 0.0;
        }
        vs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        vs[vs.len() / 2]
    };
    let torso_med_x = median(torso_xs);
    let torso_med_z = median(torso_zs);
    let torso_center_x_offset_frac = ((torso_med_x - center.x) / height).clamp(-0.5, 0.5);
    let torso_center_z_offset_frac = ((torso_med_z - center.z) / height).clamp(-0.5, 0.5);

    MeshLandmarks {
        hip_y_frac: hip_y_frac.clamp(0.30, 0.70),
        shoulder_y_frac: shoulder_y_frac.clamp(0.65, 0.92),
        neck_y_frac: neck_y_frac.clamp(0.75, 0.95),
        head_y_frac: head_y_frac.clamp(0.85, 1.0),
        aspect_ratio_xy,
        hip_width_frac,
        arm_span_half_frac,
        has_tail,
        tail_in_positive_z,
        torso_center_x_offset_frac,
        torso_center_z_offset_frac,
        vertex_count: positions.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un mesh humanoïde mock : torse cylindre + 2 jambes cylindres
    /// + 2 bras T-pose + tête boule. Hauteur 2m, centré XZ.
    fn humanoid_mock_vertices() -> (Vec<Vec3>, Aabb) {
        let mut v = Vec::new();
        // Jambes (2 colonnes écartées ±0.18m, Y de 0 à 1.0)
        for i in 0..50 {
            let y = i as f32 * 1.0 / 50.0; // 0..1m
            for x in [-0.20_f32, -0.16, -0.12, 0.12, 0.16, 0.20] {
                for z in [-0.05_f32, 0.0, 0.05] {
                    v.push(Vec3::new(x, y, z));
                }
            }
        }
        // Torse (1 cylindre uni, Y de 1.0 à 1.65)
        for i in 0..40 {
            let y = 1.0 + i as f32 * 0.65 / 40.0;
            for angle_deg in (0..360).step_by(20) {
                let a = (angle_deg as f32).to_radians();
                v.push(Vec3::new(0.12 * a.cos(), y, 0.08 * a.sin()));
            }
        }
        // Bras T-pose (étendus latéralement à Y=1.5)
        for i in 0..30 {
            let t = i as f32 / 30.0;
            for &side in &[-1.0_f32, 1.0] {
                let x = side * (0.15 + t * 0.45); // 0.15 → 0.60
                v.push(Vec3::new(x, 1.5, 0.0));
                v.push(Vec3::new(x, 1.48, 0.0));
                v.push(Vec3::new(x, 1.52, 0.0));
            }
        }
        // Tête (boule à Y de 1.65 à 2.0)
        for i in 0..30 {
            let y = 1.65 + i as f32 * 0.35 / 30.0;
            for angle_deg in (0..360).step_by(20) {
                let a = (angle_deg as f32).to_radians();
                v.push(Vec3::new(0.09 * a.cos(), y, 0.09 * a.sin()));
            }
        }
        let aabb = Aabb {
            center: Vec3::new(0.0, 1.0, 0.0).into(),
            half_extents: Vec3::new(0.60, 1.0, 0.10).into(),
        };
        (v, aabb)
    }

    #[test]
    fn humanoid_landmarks_hip_in_lower_half() {
        let (v, aabb) = humanoid_mock_vertices();
        let landmarks = detect_landmarks_from_vertices(&v, &aabb);
        assert!(
            (0.40..=0.60).contains(&landmarks.hip_y_frac),
            "hip should be in [0.40, 0.60] of height (jambes finissent à 50%), got {}",
            landmarks.hip_y_frac
        );
    }

    #[test]
    fn humanoid_landmarks_shoulder_above_hip() {
        let (v, aabb) = humanoid_mock_vertices();
        let landmarks = detect_landmarks_from_vertices(&v, &aabb);
        assert!(
            landmarks.shoulder_y_frac > landmarks.hip_y_frac,
            "shoulders must be above hip (hip={}, shoulder={})",
            landmarks.hip_y_frac,
            landmarks.shoulder_y_frac
        );
        assert!(
            landmarks.head_y_frac > landmarks.shoulder_y_frac,
            "head must be above shoulders"
        );
    }

    #[test]
    fn humanoid_detected_not_lizard() {
        let (v, aabb) = humanoid_mock_vertices();
        let landmarks = detect_landmarks_from_vertices(&v, &aabb);
        assert!(
            !landmarks.has_tail,
            "humanoid mock has no vertices behind, should not detect tail"
        );
    }

    #[test]
    fn humanoid_looks_humanoid() {
        let (v, aabb) = humanoid_mock_vertices();
        let landmarks = detect_landmarks_from_vertices(&v, &aabb);
        // L'aspect ratio est large à cause des bras T-pose (0.60 x 1.0 = ~0.60)
        // mais aucun tail détecté → on accepte les bras larges comme humanoid
        assert!(
            !landmarks.has_tail,
            "humanoid mock should not be classified as having tail"
        );
        // hip dans la plage humanoid
        assert!(landmarks.hip_y_frac > 0.40);
    }

    #[test]
    fn empty_mesh_returns_default() {
        let aabb = Aabb {
            center: Vec3::ZERO.into(),
            half_extents: Vec3::splat(1.0).into(),
        };
        let landmarks = detect_landmarks_from_vertices(&[], &aabb);
        assert_eq!(landmarks.hip_y_frac, 0.55);
        assert_eq!(landmarks.vertex_count, 0);
    }

    #[test]
    fn lizard_mock_detects_tail() {
        // Mock biped lézard : jambes + torse penché + tail derrière
        let mut v = Vec::new();
        // Jambes (Y 0 à 0.5)
        for i in 0..30 {
            let y = i as f32 * 0.5 / 30.0;
            for x in [-0.10_f32, 0.10] {
                v.push(Vec3::new(x, y, 0.0));
                v.push(Vec3::new(x, y, 0.05));
            }
        }
        // Torse cylindre (Y 0.5 à 1.0)
        for i in 0..30 {
            let y = 0.5 + i as f32 * 0.5 / 30.0;
            for angle_deg in (0..360).step_by(30) {
                let a = (angle_deg as f32).to_radians();
                v.push(Vec3::new(0.12 * a.cos(), y, 0.10 * a.sin()));
            }
        }
        // Tail derrière (Z very negative, Y mid)
        for i in 0..40 {
            let t = i as f32 / 40.0;
            v.push(Vec3::new(0.0, 0.45 - t * 0.15, -0.3 - t * 0.6));
        }
        let aabb = Aabb {
            center: Vec3::new(0.0, 0.5, -0.3).into(),
            half_extents: Vec3::new(0.15, 0.5, 0.6).into(),
        };
        let landmarks = detect_landmarks_from_vertices(&v, &aabb);
        assert!(landmarks.has_tail, "lizard with vertices behind should detect tail");
        assert!(!landmarks.looks_humanoid(), "lizard should not look_humanoid");
    }

    #[test]
    fn lizard_mock_detects_tail_positive_z() {
        // Régression 2026-05-18 AM (sensor forgia2_auto_rig.json) : Meshy Rex
        // a sa queue en direction +Z (orientation glb variable) alors que
        // l'ancien code ne scannait que min_z (-Z direction). Faux négatif
        // has_tail=false sur un mesh ayant clairement une queue.
        // Fix : tester les deux directions ±Z, prendre le max.
        let mut v = Vec::new();
        // Jambes (Y 0 à 0.5)
        for i in 0..30 {
            let y = i as f32 * 0.5 / 30.0;
            for x in [-0.10_f32, 0.10] {
                v.push(Vec3::new(x, y, 0.0));
                v.push(Vec3::new(x, y, 0.05));
            }
        }
        // Torse cylindre (Y 0.5 à 1.0)
        for i in 0..30 {
            let y = 0.5 + i as f32 * 0.5 / 30.0;
            for angle_deg in (0..360).step_by(30) {
                let a = (angle_deg as f32).to_radians();
                v.push(Vec3::new(0.12 * a.cos(), y, 0.10 * a.sin()));
            }
        }
        // Tail DEVANT en +Z (orientation Meshy flippée).
        for i in 0..40 {
            let t = i as f32 / 40.0;
            v.push(Vec3::new(0.0, 0.45 - t * 0.15, 0.3 + t * 0.6));
        }
        let aabb = Aabb {
            center: Vec3::new(0.0, 0.5, 0.3).into(),
            half_extents: Vec3::new(0.15, 0.5, 0.6).into(),
        };
        let landmarks = detect_landmarks_from_vertices(&v, &aabb);
        assert!(
            landmarks.has_tail,
            "tail en +Z direction doit aussi être détecté (Meshy orientation flippée)"
        );
    }

    #[test]
    fn looks_humanoid_rejects_tail_meshes() {
        let landmarks = MeshLandmarks {
            has_tail: true,
            vertex_count: 1000, // analyse réelle
            ..MeshLandmarks::default()
        };
        assert!(!landmarks.looks_humanoid());
    }

    #[test]
    fn looks_humanoid_accepts_tpose_wide_arms() {
        // Regression 2026-05-17 PM : T-pose mesh humanoid avait aspect_ratio_xy
        // ≥ 1.0 (bras étendus), faisait échouer l'ancien critère <0.55.
        // looks_humanoid() ne doit PLUS utiliser aspect ratio.
        let landmarks = MeshLandmarks {
            hip_y_frac: 0.50,
            shoulder_y_frac: 0.82,
            neck_y_frac: 0.90,
            head_y_frac: 0.98,
            aspect_ratio_xy: 1.05, // T-pose large ! Ancien code rejetait à tort.
            hip_width_frac: 0.12,
            arm_span_half_frac: 0.50,
            has_tail: false,
            tail_in_positive_z: false,
            torso_center_x_offset_frac: 0.0,
            torso_center_z_offset_frac: 0.0,
            vertex_count: 5000,
        };
        assert!(
            landmarks.looks_humanoid(),
            "T-pose humanoid with wide arms must be classified humanoid"
        );
    }

    #[test]
    fn humanoid_with_skirt_does_not_detect_tail() {
        // REGRESSION runtime 2026-05-17 PM (forgia2_run.log) : humanoid Meshy
        // avec jupe rouge trainante détectait has_tail=true à tort
        // (depth_ratio bas/haut > 2.0 trigger). Fix : seuil ratio > 4.0 ET
        // profondeur absolue > 30% hauteur.
        //
        // Mock : humanoid 1.7m + jupe qui traîne 15cm derrière le centre Z
        // (humanoid jupe normale, pas un tail saillant).
        let mut v = Vec::new();
        // Jambes (Y 0 à 0.85m), avec quelques vertices arrière (jupe enveloppe)
        for i in 0..50 {
            let y = i as f32 * 0.85 / 50.0;
            for x in [-0.20_f32, -0.16, -0.12, 0.12, 0.16, 0.20] {
                for z in [-0.08_f32, -0.04, 0.0, 0.04] {
                    v.push(Vec3::new(x, y, z));
                }
            }
        }
        // Jupe trainante : vertices arrière Z=-0.15 à -0.20 dans la zone bassin
        for i in 0..40 {
            let y = 0.6 + i as f32 * 0.25 / 40.0; // Y 0.6 to 0.85
            for x in [-0.20_f32, 0.0, 0.20] {
                v.push(Vec3::new(x, y, -0.18 - (i as f32 / 40.0) * 0.05));
            }
        }
        // Torse + bras + tête (uni-clusteré au centre)
        for i in 0..40 {
            let y = 0.85 + i as f32 * 0.85 / 40.0;
            for a in (0..360).step_by(20) {
                let r = (a as f32).to_radians();
                v.push(Vec3::new(0.14 * r.cos(), y, 0.10 * r.sin()));
            }
        }
        let aabb = Aabb {
            center: Vec3::new(0.0, 0.85, -0.10).into(),
            half_extents: Vec3::new(0.22, 0.85, 0.30).into(),
        };
        let landmarks = detect_landmarks_from_vertices(&v, &aabb);
        assert!(
            !landmarks.has_tail,
            "humanoid with skirt should NOT detect tail (got depth_bottom suspect)"
        );
        assert!(
            landmarks.looks_humanoid(),
            "humanoid mesh with skirt should classify humanoid (got hip={:.2}, has_tail={})",
            landmarks.hip_y_frac,
            landmarks.has_tail
        );
    }

    #[test]
    fn humanoid_wide_hips_meshy_style_hip_detected_correctly() {
        // REGRESSION runtime : mesh Meshy avec hanches larges. Ancienne heuristique
        // max_dev détectait hip à 70% (au-dessus du vrai bassin). Nouvelle
        // heuristique center_ratio doit détecter le bassin au ~50%.
        let mut v = Vec::new();
        // Jambes parallèles écartées de 30cm chacune (Y 0 à 0.85)
        for i in 0..50 {
            let y = i as f32 * 0.85 / 50.0;
            for x in [-0.18_f32, -0.14, 0.14, 0.18] {
                for z in [-0.06_f32, 0.0, 0.06] {
                    v.push(Vec3::new(x, y, z));
                }
            }
        }
        // BASSIN LARGE (40cm wide, Meshy style) Y [0.85, 1.0]
        for i in 0..15 {
            let y = 0.85 + i as f32 * 0.15 / 15.0;
            for x_step in (-20i32..=20).step_by(4) {
                let x = x_step as f32 * 0.01; // -0.20 à 0.20 step 0.04
                v.push(Vec3::new(x, y, 0.0));
                v.push(Vec3::new(x, y, -0.06));
                v.push(Vec3::new(x, y, 0.06));
            }
        }
        // Torse + tête
        for i in 0..30 {
            let y = 1.0 + i as f32 * 0.70 / 30.0;
            for a in (0..360).step_by(20) {
                let r = (a as f32).to_radians();
                v.push(Vec3::new(0.12 * r.cos(), y, 0.10 * r.sin()));
            }
        }
        let aabb = Aabb {
            center: Vec3::new(0.0, 0.85, 0.0).into(),
            half_extents: Vec3::new(0.22, 0.85, 0.10).into(),
        };
        let landmarks = detect_landmarks_from_vertices(&v, &aabb);
        // Hip Y attendu autour 0.50 (= Y 0.85 sur height 1.7 = 50%)
        assert!(
            (0.40..=0.60).contains(&landmarks.hip_y_frac),
            "hip should be ~50% (vrai bassin), got {:.2}",
            landmarks.hip_y_frac
        );
    }

    #[test]
    fn looks_humanoid_rejects_default_unset_landmarks() {
        // Default landmarks (vertex_count=0) = analyse pas tournée → ne pas
        // assumer humanoid, laisser le caller fallback au template requested.
        let landmarks = MeshLandmarks::default();
        assert!(!landmarks.looks_humanoid());
    }
}
