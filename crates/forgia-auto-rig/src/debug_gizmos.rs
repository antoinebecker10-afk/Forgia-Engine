//! # Debug gizmos rig (story-440 Sprint A Vague 3)
//!
//! Trace les bones spawnés par `forgia-auto-rig` au runtime via le système
//! Gizmos Bevy. Permet à l'utilisateur de :
//!
//! - **Voir** que le squelette est bien posé sur le mesh visuel (vs flotter
//!   ailleurs ou être à l'envers)
//! - **Voir** que les bones tournent quand procedural_locomotion s'exécute
//!   (jambes alternent, tail balance, etc.)
//! - **Diagnostiquer visuellement** un rig cassé (vs juste lire le sensor JSON)
//!
//! Toggle via `AutoRigGizmosConfig::enabled` ou directement F-key dans
//! l'editor settings (Phase 1C).
//!
//! ## Couleurs par rôle
//!
//! - **Spine / hip** : vert (`#5fcc5f`)
//! - **Leg L** : rouge (`#ff4d4d`)
//! - **Leg R** : rouge foncé (`#cc3333`)
//! - **Arm L** : bleu (`#4d9aff`)
//! - **Arm R** : bleu foncé (`#3366cc`)
//! - **Head / neck** : jaune (`#ffcc4d`)
//! - **Tail** : magenta (`#cc4dff`)
//! - **Bone non classifié** : gris (`#888888`)

use bevy::prelude::*;
use std::collections::HashMap;

use crate::BoneEntity;

/// Sensor de positions monde de TOUS les bones du rig (observabilité torse).
/// Écrit par `draw_rig_gizmos` (qui a déjà tous les `BoneEntity` + GlobalTransform).
const SENSOR_RIG_BONES_PATH: &str = "forgia_rig_bones.json";
/// Marqueur de build pour vérifier que l'exe tourne le bon code (cf rule stale-binary).
const RIG_BONES_BUILD_MARKER: &str = "TOE-BONE_2026-06-05";

/// Configuration globale du draw de gizmos rig. Default = enabled, toggle
/// via inspector ou keybind dans le caller.
#[derive(Resource, Debug, Clone)]
pub struct AutoRigGizmosConfig {
    pub enabled: bool,
    /// Rayon des sphères posées sur chaque bone (m). Default 0.03.
    pub sphere_radius: f32,
    /// Si true, draw aussi le nom du bone en text (coûteux, debug only).
    /// Phase 1A : pas implémenté, placeholder pour futur.
    pub draw_names: bool,
}

impl Default for AutoRigGizmosConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sphere_radius: 0.03,
            draw_names: false,
        }
    }
}

const COLOR_SPINE: Color = Color::srgb(0.37, 0.80, 0.37);
const COLOR_LEG: Color = Color::srgb(1.0, 0.30, 0.30);
const COLOR_ARM: Color = Color::srgb(0.30, 0.60, 1.0);
const COLOR_HEAD: Color = Color::srgb(1.0, 0.80, 0.30);
const COLOR_TAIL: Color = Color::srgb(0.80, 0.30, 1.0);
const COLOR_UNKNOWN: Color = Color::srgb(0.53, 0.53, 0.53);

/// Système Gizmos : pour chaque entity `BoneEntity`, trace une ligne de son
/// parent à lui-même + une sphère à sa position monde. Couleur selon
/// classification heuristique du nom.
///
/// Tourne dans `GameSet::Effects` (après Movement où les bones bougent, avant
/// Sensors). Coût négligeable (≤ ~30 bones × 18 chars = ~540 lignes).
pub fn draw_rig_gizmos(
    config: Res<AutoRigGizmosConfig>,
    time: Res<Time>,
    mut dump_timer: Local<f32>,
    mut gizmos: Gizmos,
    q_bones: Query<(Entity, &GlobalTransform, &Name, Option<&ChildOf>, &BoneEntity)>,
    q_global: Query<&GlobalTransform>,
) {
    if !config.enabled {
        return;
    }

    // Build map entity → world_pos pour résoudre les parents.
    let positions: HashMap<Entity, Vec3> = q_bones
        .iter()
        .map(|(e, gt, _, _, _)| (e, gt.translation()))
        .collect();

    for (entity, gt, name, child_of, _bone) in &q_bones {
        let pos = gt.translation();
        let color = color_for_name(name.as_str());

        // Sphère à la position du bone
        gizmos.sphere(pos, config.sphere_radius, color);

        // Ligne vers le parent (= bone parent OU mesh root si root bone)
        let parent_pos = child_of.and_then(|c| {
            let p = c.parent();
            // Si parent est un autre bone : sa world position
            positions
                .get(&p)
                .copied()
                // Sinon (parent = mesh_root), lookup global transform direct
                .or_else(|| q_global.get(p).ok().map(|gt| gt.translation()))
        });

        if let Some(pp) = parent_pos {
            gizmos.line(pp, pos, color);
        }

        let _ = entity; // explicit unused
    }

    // ── Dump sensor positions monde (observabilité torse) ──────────────────
    // Throttle 0.5s. Tri par Y monde décroissant (haut→bas) pour lisibilité.
    *dump_timer += time.delta_secs();
    if *dump_timer >= 0.5 {
        *dump_timer = 0.0;
        write_rig_bones_sensor(&q_bones);
    }
}

/// Écrit `forgia_rig_bones.json` : nom + couleur classe + position monde de
/// chaque bone, **groupé par squelette** (`rig` = entity du rig_root) puis trié
/// haut→bas. La scène contient plusieurs persos rigés (Rex + PNJ) → grouper par
/// `rig` permet d'isoler Rex (= le seul squelette avec des os `tail_*`).
/// `dy_prev`/`dz_prev` ne sont calculés qu'à l'intérieur d'un même `rig`.
fn write_rig_bones_sensor(
    q_bones: &Query<(Entity, &GlobalTransform, &Name, Option<&ChildOf>, &BoneEntity)>,
) {
    // (rig_id, name, color, world) — rig_id = bits stables de l'entity rig_root.
    let mut rows: Vec<(u64, String, &'static str, Vec3)> = q_bones
        .iter()
        .map(|(_, gt, name, _, bone)| {
            (
                bone.rig_root.to_bits(),
                name.as_str().to_string(),
                class_label(name.as_str()),
                gt.translation(),
            )
        })
        .collect();
    // Tri : par squelette (rig) puis Y monde décroissant dans le squelette.
    rows.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(b.3.y.partial_cmp(&a.3.y).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut json = String::with_capacity(4096);
    json.push_str("{\n");
    json.push_str("  \"id\": \"rig_bones\",\n");
    json.push_str(&format!(
        "  \"BUILD_MARKER\": \"{RIG_BONES_BUILD_MARKER}\",\n"
    ));
    json.push_str(&format!("  \"bone_count\": {},\n", rows.len()));
    json.push_str("  \"note\": \"groupé par 'rig' (squelette), trié Y décroissant. Rex = le rig avec des os tail_*. color=classe gizmo (green=spine). dy/dz vs bone précédent DU MÊME rig.\",\n");
    json.push_str("  \"bones\": [\n");
    let mut prev: Option<(u64, Vec3)> = None;
    for (i, (rig, name, color, p)) in rows.iter().enumerate() {
        let (ddy, ddz) = match prev {
            Some((prig, pp)) if prig == *rig => (p.y - pp.y, p.z - pp.z),
            _ => (0.0, 0.0),
        };
        prev = Some((*rig, *p));
        let comma = if i + 1 < rows.len() { "," } else { "" };
        json.push_str(&format!(
            "    {{\"rig\":{rig},\"name\":\"{name}\",\"color\":\"{color}\",\"world\":[{:.3},{:.3},{:.3}],\"dy_prev\":{:.3},\"dz_prev\":{:.3}}}{comma}\n",
            p.x, p.y, p.z, ddy, ddz
        ));
    }
    json.push_str("  ]\n}\n");

    if let Err(e) = std::fs::write(SENSOR_RIG_BONES_PATH, json) {
        warn!("[auto-rig] échec écriture {SENSOR_RIG_BONES_PATH}: {e}");
    }
}

/// Label court de classe (aligné `color_for_name`) pour le sensor.
fn class_label(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    if n.contains("thigh") || n.contains("shin") || n.contains("foot") || n.contains("leg") || n.contains("toe") {
        "leg"
    } else if n.contains("arm") || n.contains("forearm") || n.contains("clavicle") || n.contains("hand") {
        "arm"
    } else if n.contains("tail_") {
        "tail"
    } else if n.contains("head") || n.contains("neck") || n.contains("skull") {
        "head"
    } else if n.contains("spine") || n.contains("chest") || n.contains("hip") || n.contains("torso") {
        "spine"
    } else {
        "unknown"
    }
}

/// Classification heuristique par sous-chaîne du nom de bone. Aligné avec
/// les naming conventions des templates BipedLizard / Humanoid de Phase 1A
/// (hip, spine_*, thigh_L/R, shin_*, foot_*, arm_L/R, forearm_*, neck, head, tail_*).
fn color_for_name(name: &str) -> Color {
    let n = name.to_ascii_lowercase();
    if n.contains("thigh") || n.contains("shin") || n.contains("foot") || n.contains("leg") || n.contains("toe") {
        if n.contains("_l") || n.ends_with("l") {
            COLOR_LEG
        } else {
            // Right ou non-specifié → variante plus sombre
            Color::srgb(0.80, 0.20, 0.20)
        }
    } else if n.contains("arm")
        || n.contains("forearm")
        || n.contains("clavicle")
        || n.contains("hand")
    {
        if n.contains("_l") || n.ends_with("l") {
            COLOR_ARM
        } else {
            Color::srgb(0.20, 0.40, 0.80)
        }
    } else if n.contains("tail_") {
        COLOR_TAIL
    } else if n.contains("head") || n.contains("neck") || n.contains("skull") {
        COLOR_HEAD
    } else if n.contains("spine") || n.contains("chest") || n.contains("hip") || n.contains("torso")
    {
        COLOR_SPINE
    } else {
        COLOR_UNKNOWN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_for_name_classifies_leg() {
        let c = color_for_name("thigh_L");
        assert_eq!(c, COLOR_LEG);
    }

    #[test]
    fn color_for_name_classifies_arm() {
        let c = color_for_name("arm_L");
        assert_eq!(c, COLOR_ARM);
    }

    #[test]
    fn color_for_name_classifies_tail() {
        let c = color_for_name("tail_02");
        assert_eq!(c, COLOR_TAIL);
    }

    #[test]
    fn color_for_name_classifies_spine() {
        assert_eq!(color_for_name("spine_lower"), COLOR_SPINE);
        assert_eq!(color_for_name("hip"), COLOR_SPINE);
        assert_eq!(color_for_name("chest"), COLOR_SPINE);
    }

    #[test]
    fn color_for_name_classifies_head() {
        assert_eq!(color_for_name("head"), COLOR_HEAD);
        assert_eq!(color_for_name("neck"), COLOR_HEAD);
    }

    #[test]
    fn color_for_name_unknown_falls_back() {
        assert_eq!(color_for_name("mystery_bone"), COLOR_UNKNOWN);
    }
}
