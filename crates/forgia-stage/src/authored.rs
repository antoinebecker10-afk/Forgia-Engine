//! authored.rs — Coquille d'arène **authored** data-driven (story-625, Tier 1).
//!
//! Modèle Returnal hybride : la STRUCTURE de l'arène est composée à la main dans
//! `assets/genomes/arena_layouts.toml` (pièces GLB atomiques Inferno/KayKit
//! placées par position/rotation/scale/rôle), instanciée par `forgia-prefab`.
//! Le procédural (`layout::place_modules`) reste fallback (stage sans layout) ou
//! overlay (loot/POIs). Si `suppress_procedural_modules`, le scatter de modules
//! est coupé — la composition authored prend la main.
//!
//! Voir `docs/audit/audit-2026-06-26-arena-render-quality-vs-parcours.md`.
//! Hot-reload Shift+F12 natif (genome via AssetServer). **0 coordonnée arène en
//! dur dans le Rust** — tout vit dans le TOML.

use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, ComputedColliderShape};
use forgia_anchor::AnchorKind;
use forgia_level_presets::parse_anchor_kind;
use serde::Deserialize;
use std::collections::HashMap;

// ─── Types genome (Deserialize depuis arena_layouts.toml) ────────────────────

/// Une pièce posée à la main dans l'arène (1 GLB atomique = 1 instance).
#[derive(Deserialize, Clone, Debug)]
pub struct ArenaPiece {
    /// Chemin GLB relatif à `assets/` (e.g. "models/environment/inferno/TowerBig_001.glb").
    pub prefab: String,
    /// Position monde `[x, y, z]`. Arène centrée sur l'origine (convention story-483).
    pub pos: [f32; 3],
    /// Yaw en degrés (rotation Y). 0 par défaut.
    #[serde(default)]
    pub rot_deg: f32,
    /// Échelle uniforme. 1.0 par défaut.
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Rôle gameplay → pose un `AnchorPoint` si reconnu. "decor"/"" = visuel pur.
    /// Valeurs reconnues = snake_case `AnchorKind` (melee_pit, sniper_perch,
    /// boss_pad, player_spawn, poi_slot, cover_low, cover_high, ...).
    #[serde(default)]
    pub role: String,
    /// Walkable → collider TriMesh (on marche dessus : perchoir, plateforme).
    #[serde(default)]
    pub walkable: bool,
    /// Blocker → collider TriMesh bloquant (mur, obstacle).
    #[serde(default)]
    pub blocker: bool,
    /// Section bible (organisationnel : sensor/debug). Optionnel.
    #[serde(default)]
    pub section: String,
}

fn default_scale() -> f32 {
    1.0
}

/// Layout authored d'un stage : liste de pièces + politique vis-à-vis du procédural.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct ArenaLayout {
    /// true → la structure est authored : on coupe le scatter `place_modules`
    /// (le procédural reste pour POIs/loot = overlay run-to-run).
    #[serde(default)]
    pub suppress_procedural_modules: bool,
    #[serde(default)]
    pub pieces: Vec<ArenaPiece>,
}

/// Genome `arena_layouts.toml` : `[layouts.<stage_id>] ArenaLayout`.
#[derive(Deserialize, Clone, Debug, Default, TypePath)]
pub struct ArenaLayoutsGenome {
    #[serde(default)]
    pub layouts: HashMap<String, ArenaLayout>,
}

// ─── Rôle → Anchor (pur, testable) ──────────────────────────────────────────

/// Pure : rôle → `Option<AnchorKind>`. "decor"/"" → None (pièce visuelle pure).
/// Réutilise `parse_anchor_kind` (forgia-level-presets) pour cohérence des noms.
pub fn role_to_anchor(role: &str) -> Option<AnchorKind> {
    if role.is_empty() || role == "decor" {
        return None;
    }
    parse_anchor_kind(role)
}

// ─── Collider différé (pièces walkable/blocker) ─────────────────────────────

/// Posé sur une pièce authored walkable/blocker en attente de collider. Le GLB
/// charge en différé → [`sys_collide_authored_pieces`] retente jusqu'à mesh prêt,
/// puis ajoute un collider TriMesh sur chaque mesh (épouse le visuel exact).
#[derive(Component, Debug, Clone, Copy)]
pub struct AuthoredNeedsCollider;

/// Walk récursif : collecte les entités `Mesh3d` sous `root`.
fn collect_mesh_descendants(
    root: Entity,
    q_children: &Query<&Children>,
    q_mesh: &Query<&Mesh3d>,
    out: &mut Vec<Entity>,
) {
    if q_mesh.contains(root) {
        out.push(root);
    }
    if let Ok(children) = q_children.get(root) {
        for c in children.iter() {
            collect_mesh_descendants(c, q_children, q_mesh, out);
        }
    }
}

/// Ajoute un collider TriMesh aux meshes des pièces authored walkable/blocker
/// (Collider sans RigidBody = statique, marchable, épouse le visuel — même
/// pattern que `boss_portal::solidify_dais`). Idempotent : retire le marqueur
/// quand tous les meshes sont chargés + collidérés.
pub fn sys_collide_authored_pieces(
    mut commands: Commands,
    q_pieces: Query<Entity, With<AuthoredNeedsCollider>>,
    q_children: Query<&Children>,
    q_mesh: Query<&Mesh3d>,
    q_has_col: Query<(), With<Collider>>,
    meshes: Res<Assets<Mesh>>,
) {
    for root in &q_pieces {
        let mut mesh_ents = Vec::new();
        collect_mesh_descendants(root, &q_children, &q_mesh, &mut mesh_ents);
        if mesh_ents.is_empty() {
            continue; // scène pas encore peuplée → retry
        }
        let all_loaded = mesh_ents
            .iter()
            .all(|e| q_mesh.get(*e).ok().and_then(|m| meshes.get(&m.0)).is_some());
        if !all_loaded {
            continue; // meshes pas tous chargés → retry
        }
        for &e in &mesh_ents {
            if q_has_col.get(e).is_ok() {
                continue; // déjà collideré
            }
            if let Ok(m3d) = q_mesh.get(e) {
                if let Some(mesh) = meshes.get(&m3d.0) {
                    if let Some(col) =
                        Collider::from_bevy_mesh(mesh, &ComputedColliderShape::TriMesh(default()))
                    {
                        commands.entity(e).try_insert(col);
                    }
                }
            }
        }
        commands.entity(root).remove::<AuthoredNeedsCollider>();
    }
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_decor_and_empty_yield_no_anchor() {
        assert_eq!(role_to_anchor("decor"), None);
        assert_eq!(role_to_anchor(""), None);
        assert_eq!(role_to_anchor("inconnu"), None);
    }

    #[test]
    fn role_gameplay_maps_to_anchor() {
        assert_eq!(role_to_anchor("melee_pit"), Some(AnchorKind::MeleePit));
        assert_eq!(
            role_to_anchor("sniper_perch"),
            Some(AnchorKind::SniperPerch)
        );
        assert_eq!(role_to_anchor("boss_pad"), Some(AnchorKind::BossPad));
        assert_eq!(
            role_to_anchor("player_spawn"),
            Some(AnchorKind::PlayerSpawn)
        );
    }

    #[test]
    fn parse_layout_genome_minimal() {
        let toml_src = r#"
[layouts.crypts_of_anvil]
suppress_procedural_modules = true

[[layouts.crypts_of_anvil.pieces]]
prefab = "models/environment/inferno/TowerBig_001.glb"
pos = [-46.0, 0.0, 28.0]
rot_deg = 90.0
role = "sniper_perch"
walkable = true
section = "perchoir"
"#;
        let g: ArenaLayoutsGenome = toml::from_str(toml_src).expect("parse");
        let layout = g.layouts.get("crypts_of_anvil").expect("stage present");
        assert!(layout.suppress_procedural_modules);
        assert_eq!(layout.pieces.len(), 1);
        let p = &layout.pieces[0];
        assert_eq!(p.pos, [-46.0, 0.0, 28.0]);
        assert_eq!(p.rot_deg, 90.0);
        assert!((p.scale - 1.0).abs() < 1e-6, "scale default = 1.0");
        assert!(p.walkable);
        assert_eq!(role_to_anchor(&p.role), Some(AnchorKind::SniperPerch));
    }

    #[test]
    fn parse_empty_genome_ok() {
        let g: ArenaLayoutsGenome = toml::from_str("").expect("parse empty");
        assert!(g.layouts.is_empty());
    }

    #[test]
    fn piece_defaults_applied() {
        let toml_src = r#"
[[layouts.s.pieces]]
prefab = "a.glb"
pos = [1.0, 2.0, 3.0]
"#;
        let g: ArenaLayoutsGenome = toml::from_str(toml_src).expect("parse");
        let p = &g.layouts["s"].pieces[0];
        assert_eq!(p.rot_deg, 0.0);
        assert!((p.scale - 1.0).abs() < 1e-6);
        assert!(!p.walkable && !p.blocker);
        assert_eq!(p.role, "");
        assert_eq!(role_to_anchor(&p.role), None);
    }
}
