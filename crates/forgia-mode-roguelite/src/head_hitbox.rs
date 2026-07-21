//! head_hitbox.rs — story-652 : hitbox tête ennemis **suivie de l'os `head`**.
//!
//! Problème résolu (diagnostic 2026-07-02, sensor `forgia2_combat.json` : 105 tirs,
//! 0 `hit_zone_head`) : l'ancien proxy tête (fractions de capsule, story-517) était
//! strictement À L'INTÉRIEUR de la capsule body → jamais premier hit du raycast
//! (`forgia-fps` `cast_ray` premier-hit) → headshots impossibles. Et les capsules ne
//! collaient pas aux meshs KayKit (crâne du Runner 33 cm hors collider).
//!
//! Approche industrie : la sphère tête est dimensionnée sur le **crâne mesuré** du
//! GLB, dépasse de la capsule body (recalibrée aux épaules), et est **recollée sur
//! le joint `head` du rig animé** chaque frame — elle suit marche/course/attaque.
//!
//! Lag assumé : le joint est lu via `GlobalTransform` (propagé au PostUpdate
//! précédent) et Rapier synchronise en FixedUpdate → la hitbox retarde d'1-2 frames
//! sur le visuel (~3-7 cm en anim rapide). Standard du genre, imperceptible.
//!
//! Capteur : `forgia2_head_hitbox.json` (1 Hz) — proxies vs bound + next_step.

use bevy::prelude::*;
use forgia_core::prelude::*;
use std::fs;

const SENSOR_PATH: &str = "forgia2_head_hitbox.json";
const POLL_PERIOD_SEC: f32 = 1.0;
/// Garde-fou BFS binding (rig KayKit ≈ 60 nodes, armes incluses).
const BIND_BFS_MAX_NODES: u32 = 512;

// ── Mesures bind-pose des GLB KayKit ────────────────────────────────────────
// Scripts scratchpad `measure_glb_aabb.py` / `head_mesh_aabb.py` (2026-07-02).
// Les 3 rigs (Warrior/Minion/Mage) partagent le même squelette : crâne centré
// (0, 1.735, 0), AABB 0.87×0.85×0.91, joint `head` à y=1.241 (espace modèle).
// Invariants de l'asset (pas du tuning gameplay) → const nommées (no-hardcode §4).

/// Nom du node glTF du joint de tête dans les rigs KayKit.
pub const KAYKIT_HEAD_JOINT_NAME: &str = "head";
/// Centre du mesh crâne AU-DESSUS du joint `head` (espace modèle) : 1.735 − 1.241.
pub const SKULL_CENTER_ABOVE_JOINT_RAW: f32 = 0.494;
/// Centre du mesh crâne au-dessus des pieds (bind pose) — fallback avant binding.
pub const SKULL_CENTER_ABOVE_FEET_RAW: f32 = 1.735;
/// Demi-étendue max du mesh crâne (0.912 / 2) → rayon auto de la sphère tête.
pub const SKULL_HALF_EXTENT_RAW: f32 = 0.456;

// ── Placement (pur, testable) ───────────────────────────────────────────────

/// Rayon de la hitbox tête (m monde) : override genome sinon crâne mesuré × scale.
pub fn head_radius(override_m: Option<f32>, skeleton_scale: f32) -> f32 {
    override_m
        .unwrap_or(SKULL_HALF_EXTENT_RAW * skeleton_scale)
        .max(0.01)
}

/// Y parent-local du centre du crâne en bind pose (fallback pré-binding).
/// Le parent ennemi est au centre de la capsule ; les pieds sont à −(hh+r).
pub fn head_local_y_bind(
    capsule_half_height: f32,
    capsule_radius: f32,
    skeleton_scale: f32,
) -> f32 {
    SKULL_CENTER_ABOVE_FEET_RAW * skeleton_scale - (capsule_half_height + capsule_radius)
}

// ── Composants ──────────────────────────────────────────────────────────────

/// Sphère hitbox tête (`HitZoneTag(Head)`, enfant direct de la racine ennemi).
/// `joint` est résolu par `sys_bind_head_joints` quand la scène GLB est prête.
#[derive(Component)]
pub struct HeadProxy {
    /// Racine ennemi (RigidBody + EnemyArchetype) — référentiel parent-local.
    pub enemy_root: Entity,
    /// Entité du joint `head` du rig (None tant que non lié).
    pub joint: Option<Entity>,
}

// ── Systèmes ────────────────────────────────────────────────────────────────

/// Résout l'entité du joint `head` de chaque proxy non lié (BFS depuis la racine
/// ennemi, match exact du nom de node). Idempotent, retry chaque frame tant que la
/// scène n'est pas prête (miroir robustesse `sys_wire_enemy_anim`, story-636).
/// Hot path : les proxies liés sortent en 1 test ; le BFS ne tourne qu'au chargement.
pub fn sys_bind_head_joints(
    mut q_proxies: Query<&mut HeadProxy>,
    q_children: Query<&Children>,
    q_names: Query<&Name>,
    mut stack: Local<Vec<Entity>>,
) {
    for mut proxy in &mut q_proxies {
        if proxy.joint.is_some() {
            continue;
        }
        stack.clear();
        stack.push(proxy.enemy_root);
        let mut visited = 0u32;
        while let Some(e) = stack.pop() {
            visited += 1;
            if visited > BIND_BFS_MAX_NODES {
                break;
            }
            if e != proxy.enemy_root
                && q_names
                    .get(e)
                    .is_ok_and(|n| n.as_str() == KAYKIT_HEAD_JOINT_NAME)
            {
                proxy.joint = Some(e);
                break;
            }
            if let Ok(children) = q_children.get(e) {
                stack.extend(children.iter());
            }
        }
    }
}

/// Recolle chaque sphère tête sur le crâne du rig animé : centre du crâne = offset
/// mesuré au-dessus du joint, transformé par le `GlobalTransform` du joint (rotation
/// et skeleton_scale inclus → suit lean/anims), reprojeté en parent-local. Un joint
/// despawné (mort/cleanup) fige la sphère sur sa dernière position (cadavre : OK).
pub fn sys_track_head_proxies(
    mut q_proxies: Query<(&HeadProxy, &mut Transform)>,
    q_gt: Query<&GlobalTransform>,
) {
    for (proxy, mut tf) in &mut q_proxies {
        let Some(joint) = proxy.joint else { continue };
        let (Ok(joint_gt), Ok(root_gt)) = (q_gt.get(joint), q_gt.get(proxy.enemy_root)) else {
            continue;
        };
        let skull_world = joint_gt.transform_point(Vec3::Y * SKULL_CENTER_ABOVE_JOINT_RAW);
        let local = root_gt.affine().inverse().transform_point3(skull_world);
        if local.is_finite() {
            tf.translation = local;
        }
    }
}

// ── Sensor ──────────────────────────────────────────────────────────────────

/// Pur — severity/next_step du capteur.
pub fn severity_for_head_hitbox(proxies: u32, bound: u32) -> (&'static str, &'static str) {
    if proxies > 0 && bound == 0 {
        (
            "warn",
            "Proxies tête présents mais 0 lié au joint `head` — vérifier noms de bones GLB / binding.",
        )
    } else if bound < proxies {
        ("info", "Binding os en cours (scènes GLB en chargement).")
    } else {
        ("ok", "Hitbox tête suivies (os `head`).")
    }
}

pub fn sys_write_head_hitbox_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    q_proxies: Query<&HeadProxy>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let mut proxies = 0u32;
    let mut bound = 0u32;
    for p in &q_proxies {
        proxies += 1;
        if p.joint.is_some() {
            bound += 1;
        }
    }
    let (severity, next_step) = severity_for_head_hitbox(proxies, bound);
    let json = format!(
        r#"{{"id":"head_hitbox","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"proxies":{proxies},"bound":{bound}}}"#,
        time.elapsed_secs(),
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[head_hitbox] sensor write failed: {e}");
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct RogueliteHeadHitboxPlugin;

impl Plugin for RogueliteHeadHitboxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sys_bind_head_joints, sys_track_head_proxies)
                .chain()
                .in_set(GameSet::Combat)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_write_head_hitbox_sensor
                .in_set(GameSet::Sensors)
                .run_if(in_state(GameMode::Roguelite)),
        );
    }
}

// ── Tests purs ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enemies::{skeleton_scale, stats_for, EnemyArchetype};

    const ALL: [EnemyArchetype; 4] = [
        EnemyArchetype::Tank,
        EnemyArchetype::Runner,
        EnemyArchetype::Sniper,
        EnemyArchetype::Boss,
    ];

    /// Régression du bug story-652 : la sphère tête DOIT dépasser de la capsule
    /// (sinon le raycast premier-hit ne la touche jamais) ET la chevaucher (pas de
    /// trou « cou » où un tir ne toucherait rien).
    #[test]
    fn head_sphere_pokes_out_of_capsule_no_neck_gap() {
        for arch in ALL {
            let s = stats_for(arch);
            let scale = skeleton_scale(arch);
            let r_head = head_radius(s.head_radius, scale);
            let y = head_local_y_bind(s.capsule_half_height, s.capsule_radius, scale);
            let capsule_top = s.capsule_half_height + s.capsule_radius; // parent-local
            assert!(
                y + r_head > capsule_top + 0.15 * scale,
                "{arch:?}: sphère tête doit dépasser nettement de la capsule \
                 (top sphère {:.2} vs top capsule {:.2})",
                y + r_head,
                capsule_top
            );
            assert!(
                y - r_head < capsule_top,
                "{arch:?}: pas de trou au cou (bas sphère {:.2} vs top capsule {:.2})",
                y - r_head,
                capsule_top
            );
        }
    }

    /// La capsule + la sphère couvrent le mesh : le haut du crâne visuel
    /// (2.166 × scale au-dessus des pieds) est dans la sphère.
    #[test]
    fn skull_top_covered_by_head_sphere() {
        for arch in ALL {
            let s = stats_for(arch);
            let scale = skeleton_scale(arch);
            let r_head = head_radius(s.head_radius, scale);
            let y = head_local_y_bind(s.capsule_half_height, s.capsule_radius, scale);
            // Haut du crâne mesuré, converti en parent-local.
            let skull_top_local = 2.166 * scale - (s.capsule_half_height + s.capsule_radius);
            assert!(
                y + r_head >= skull_top_local - 0.02 * scale,
                "{arch:?}: le haut du crâne visuel doit être couvert"
            );
        }
    }

    #[test]
    fn head_radius_override_wins() {
        assert_eq!(head_radius(Some(0.5), 2.0), 0.5);
        let auto = head_radius(None, 2.0);
        assert!((auto - SKULL_HALF_EXTENT_RAW * 2.0).abs() < 1e-6);
        // Jamais nul/négatif même avec un override absurde.
        assert!(head_radius(Some(-1.0), 1.0) >= 0.01);
    }

    #[test]
    fn severity_paths() {
        assert_eq!(severity_for_head_hitbox(0, 0).0, "ok");
        assert_eq!(severity_for_head_hitbox(5, 0).0, "warn");
        assert_eq!(severity_for_head_hitbox(5, 3).0, "info");
        assert_eq!(severity_for_head_hitbox(5, 5).0, "ok");
    }
}
