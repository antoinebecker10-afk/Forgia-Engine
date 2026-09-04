//! enemy_rig_debug.rs — story-636 : viz de contrôle du rig des ennemis.
//!
//! Deux aides au contrôle de l'animation, **togglables à chaud** via
//! `roguelite_enemy_anim.toml` (champs `rig_visible` / `mesh_alpha` / `gizmo_enabled`,
//! cf [`crate::enemy_anim::EnemyAnimConfig`]) :
//!
//! 1. **Transparence** : le mesh ennemi devient translucide (effet rayon-X) pour
//!    voir le squelette à travers. Observer `On<SceneInstanceReady>` posé sur le
//!    visuel (`waves.rs`) → clone **dédupliqué** des matériaux GLB (par `AssetId`
//!    d'origine) puis swap. **Piège** : muter le matériau GLB partagé toucherait
//!    TOUS les ennemis ; le clone par-source est volontairement partagé pour que le
//!    toggle reste global et bon marché (~6 clones : 2 mats × 3 GLB).
//! 2. **Gizmos de bones** : trace le rig (sphère par joint + segment vers le parent)
//!    depuis `SkinnedMesh.joints`, couleur par archétype.
//!
//! Caveat connu : le tri en profondeur des skinned meshes transparents qui se
//! chevauchent est imparfait (Bevy 0.18, tri par origine d'entité, pas par
//! triangle). C'est **acceptable et voulu** ici : le but est justement de voir le
//! rig à travers la chair. `rig_visible=false` → retour `AlphaMode::Opaque`, 0 artefact.

use crate::enemies::EnemyArchetype;
use crate::enemy_anim::EnemyAnimConfig;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use std::collections::HashMap;

/// Rayon des sphères de joint (interne rendu/debug — no-hardcode exempt).
const JOINT_GIZMO_RADIUS: f32 = 0.03;
/// Résolution basse des sphères de gizmo (coût : ~41 joints × N ennemis).
const JOINT_GIZMO_RESOLUTION: u32 = 6;

/// Registry des matériaux clonés (transparents) — dédup par `AssetId` d'origine.
/// `all` sert au toggle global (mute l'alpha de tous les clones d'un coup).
#[derive(Resource, Default)]
pub struct EnemyMatRegistry {
    pub clones: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
    pub all: Vec<Handle<StandardMaterial>>,
}

/// Applique l'état debug (transparent vs opaque) à un matériau.
fn apply_alpha(m: &mut StandardMaterial, visible: bool, alpha: f32) {
    if visible {
        m.base_color = m.base_color.with_alpha(alpha);
        m.alpha_mode = AlphaMode::Blend;
    } else {
        m.base_color = m.base_color.with_alpha(1.0);
        m.alpha_mode = AlphaMode::Opaque;
    }
}

/// Couleur de gizmo par archétype (debug — lisible par-dessus le toon).
fn arch_color(arch: EnemyArchetype) -> Color {
    match arch {
        EnemyArchetype::Tank => Color::srgb(1.0, 0.3, 0.3),
        EnemyArchetype::Runner => Color::srgb(1.0, 0.7, 0.2),
        EnemyArchetype::Sniper => Color::srgb(0.7, 0.4, 1.0),
        EnemyArchetype::Boss => Color::srgb(1.0, 0.2, 0.8),
    }
}

/// Observer posé sur le visuel ennemi (`waves.rs`). Au scene-ready, swap chaque
/// matériau descendant vers un clone transparent dédupliqué.
pub fn on_enemy_scene_ready(
    event: On<SceneInstanceReady>,
    children: Query<&Children>,
    mat_q: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut registry: ResMut<EnemyMatRegistry>,
    cfg: Option<Res<EnemyAnimConfig>>,
    mut commands: Commands,
) {
    let root = event.entity;
    let visible = cfg.as_ref().map(|c| c.rig_visible).unwrap_or(true);
    let alpha = cfg.as_ref().map(|c| c.mesh_alpha).unwrap_or(0.45);
    for desc in children.iter_descendants(root) {
        let Ok(mat_handle) = mat_q.get(desc) else {
            continue;
        };
        let orig_id = mat_handle.0.id();
        let clone_handle = if let Some(h) = registry.clones.get(&orig_id) {
            h.clone()
        } else {
            let Some(base) = materials.get(&mat_handle.0).cloned() else {
                continue; // matériau pas encore chargé — laisser l'original
            };
            let mut m = base;
            apply_alpha(&mut m, visible, alpha);
            let h = materials.add(m);
            registry.clones.insert(orig_id, h.clone());
            registry.all.push(h.clone());
            h
        };
        commands.entity(desc).insert(MeshMaterial3d(clone_handle));
    }
}

/// BUG-04 (qa story-636) — purge le registry de matériaux clonés entre runs.
/// Les ennemis despawnent `OnExit(Roguelite)` (DespawnOnExit) → on relâche aussi
/// les handles clonés pour éviter une croissance non bornée si un GLB est rechargé
/// (AssetId change → anciennes entrées orphelines sinon). Les clones sont
/// re-créés au prochain scene-ready.
pub fn sys_clear_enemy_mat_registry(mut registry: ResMut<EnemyMatRegistry>) {
    registry.clones.clear();
    registry.all.clear();
}

/// Applique le toggle transparence quand la config change (mute tous les clones).
pub fn sys_apply_rig_debug(
    cfg: Res<EnemyAnimConfig>,
    registry: Res<EnemyMatRegistry>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last: Local<Option<(bool, u32)>>,
) {
    let key = (cfg.rig_visible, cfg.mesh_alpha.to_bits());
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    for h in &registry.all {
        if let Some(m) = materials.get_mut(h) {
            apply_alpha(m, cfg.rig_visible, cfg.mesh_alpha);
        }
    }
}

/// Trace le rig (sphères + os) depuis `SkinnedMesh.joints` des ennemis.
pub fn sys_draw_enemy_bones(
    cfg: Res<EnemyAnimConfig>,
    enemies: Query<(Entity, &EnemyArchetype)>,
    children: Query<&Children>,
    skinned: Query<&SkinnedMesh>,
    g_tf: Query<&GlobalTransform>,
    child_of: Query<&ChildOf>,
    mut gizmos: Gizmos,
) {
    if !cfg.gizmo_enabled {
        return;
    }
    for (root, arch) in &enemies {
        let col = arch_color(*arch);
        for desc in children.iter_descendants(root) {
            let Ok(skin) = skinned.get(desc) else {
                continue;
            };
            for &joint in &skin.joints {
                let Ok(jt) = g_tf.get(joint) else {
                    continue;
                };
                let jp = jt.translation();
                gizmos
                    .sphere(Isometry3d::from_translation(jp), JOINT_GIZMO_RADIUS, col)
                    .resolution(JOINT_GIZMO_RESOLUTION);
                // Os = segment vers le joint parent (s'il fait partie du rig).
                if let Ok(co) = child_of.get(joint) {
                    let parent = co.parent();
                    if skin.joints.contains(&parent) {
                        if let Ok(pt) = g_tf.get(parent) {
                            gizmos.line(jp, pt.translation(), col);
                        }
                    }
                }
            }
            break; // 1 skinned mesh (corps) suffit — même squelette partagé.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arch_colors_distinct() {
        assert_ne!(
            arch_color(EnemyArchetype::Tank),
            arch_color(EnemyArchetype::Runner)
        );
        assert_ne!(
            arch_color(EnemyArchetype::Sniper),
            arch_color(EnemyArchetype::Boss)
        );
    }

    #[test]
    fn apply_alpha_toggles_mode() {
        let mut m = StandardMaterial::default();
        apply_alpha(&mut m, true, 0.4);
        assert!(matches!(m.alpha_mode, AlphaMode::Blend));
        apply_alpha(&mut m, false, 0.4);
        assert!(matches!(m.alpha_mode, AlphaMode::Opaque));
    }
}
