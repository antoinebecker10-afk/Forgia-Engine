//! # brazier_emissive
//!
//! Story-551 — override `StandardMaterial.emissive` sur prefabs Brazier
//! (commit 998098f Crypts of Anvil) pour exploiter le HDR pipeline
//! (story-549 `Bloom::NATURAL` + `TonyMcMapface`).
//!
//! ## Flow auto-détection (pas de cross-dep avec `forgia-stage`)
//! 1. [`detect_braziers`] watch `Added<SceneRoot>`. Si le path du handle
//!    contient "Brazier" → insère [`BrazierEmissive`] sur l'entity.
//! 2. [`apply_brazier_emissive`] poll les `Added<BrazierEmissive>` sans
//!    [`BrazierEmissiveApplied`], walk descendants `Children` récursif,
//!    mute `StandardMaterial.emissive` via `Assets::get_mut`, tag Applied.
//!
//! ## Pourquoi mute le material (pas remove+insert)
//!
//! Cf `reference_bevy_018_meshmaterial_insert_remove_quirk.md` : `insert` ne
//! replace pas le handle en 0.18. Mutation via `Assets::get_mut` est l'API
//! correcte (le handle reste, le contenu change).
//!
//! ## Hot path
//!
//! Non hot — `Added<...>` filters + tags Applied garantissent walk descendants
//! 1× par brazier max. Polling activé tant que `BrazierEmissiveApplied` non
//! posé (scène pas encore loaded au 1er Update).

use bevy::asset::AssetServer;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::scene::{Scene, SceneRoot};

/// Tag posé sur SceneRoot des prefabs Brazier (auto-détecté via path).
#[derive(Component, Default, Debug)]
pub struct BrazierEmissive;

/// Tag posé une fois l'override emissive appliqué (idempotence).
#[derive(Component, Default, Debug)]
pub struct BrazierEmissiveApplied;

/// Multiplicateur HDR sur émission orange (LinearRgba (2.5, 1.0, 0.2) × M).
/// 4.0 → bloom visible mais pas cramé. Tunable si trop fort/faible.
const EMISSIVE_MULTIPLIER: f32 = 4.0;

/// Plugin. Wire dans `ForgiaEffectsPlugin`.
pub struct BrazierEmissivePlugin;

impl Plugin for BrazierEmissivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (detect_braziers, apply_brazier_emissive).chain());
    }
}

/// Watch `Added<SceneRoot>`. Si le path du handle contient "Brazier" (case
/// insensitive), insère `BrazierEmissive`. Auto-détection sans cross-dep
/// avec le caller spawn (forgia-stage).
pub fn detect_braziers(
    mut commands: Commands,
    q_new: Query<(Entity, &SceneRoot), Added<SceneRoot>>,
    asset_server: Res<AssetServer>,
) {
    for (e, scene_root) in q_new.iter() {
        let handle: &Handle<Scene> = &scene_root.0;
        let Some(path) = asset_server.get_path(handle.id()) else {
            continue;
        };
        let path_str = path.path().to_string_lossy();
        if path_str.to_ascii_lowercase().contains("brazier") {
            commands.entity(e).insert(BrazierEmissive);
        }
    }
}

/// Walk descendants des `BrazierEmissive` non-`Applied`. Override
/// `StandardMaterial.emissive` sur chaque mesh trouvé. Tag Applied dès
/// qu'au moins 1 material a été muté (= scène chargée).
pub fn apply_brazier_emissive(
    mut commands: Commands,
    q_brazier: Query<Entity, (With<BrazierEmissive>, Without<BrazierEmissiveApplied>)>,
    q_children: Query<&Children>,
    q_material: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let base_emissive = LinearRgba::new(
        2.5 * EMISSIVE_MULTIPLIER,
        1.0 * EMISSIVE_MULTIPLIER,
        0.2 * EMISSIVE_MULTIPLIER,
        1.0,
    );

    for brazier_e in q_brazier.iter() {
        let mut mutated = 0u32;
        let mut stack: Vec<Entity> = vec![brazier_e];
        while let Some(e) = stack.pop() {
            if let Ok(mat_handle) = q_material.get(e) {
                if let Some(mat) = materials.get_mut(&mat_handle.0) {
                    mat.emissive = base_emissive;
                    mutated += 1;
                }
            }
            if let Ok(children) = q_children.get(e) {
                stack.extend(children.iter());
            }
        }
        if mutated > 0 {
            commands.entity(brazier_e).insert(BrazierEmissiveApplied);
        }
        // Si mutated == 0 : scène pas encore loaded, on retry au prochain Update.
    }
}
