//! npc_pose.rs — Story-583 (2026-06-08). Pose idle « bras le long » des PNJ RPG.
//!
//! Les 4 PNJ on-brand (Dorin, Mira, l'Apprenti, le Maître Forgeron) sont déjà
//! auto-riggés (Pinocchio, `NeedsAutoRig::Template(Humanoid)`) mais restent en
//! **bind-pose T-pose** (bras horizontaux) faute d'`LocomotionTarget`. Ce module
//! les sort de la T-pose en composant la stance Humanoid (bras le long) par-dessus
//! la rest pose, **une fois** par PNJ (idempotent via [`NpcPosed`]).
//!
//! ## Pourquoi pas réutiliser `LocomotionTarget` / `procedural_locomotion`
//! Le cœur `procedural_locomotion` est mono-personnage (`q_cache.single()`,
//! `q_driver.single_mut()` — locomotion.rs:661/678). Ajouter `LocomotionTarget`
//! aux PNJ → >1 target → `.single()` échoue → **l'anim de Rex meurt**. On reste
//! donc à l'écart du cœur : pose statique séparée, zéro `LocomotionTarget`. Rex
//! n'a pas le component `Npc` → jamais matché par ce système.
//!
//! ## Stance
//! `StanceOffsets::humanoid_tpose()` = miroir EXACT de
//! `assets/genomes/skeleton_humanoid.toml` (`arm_l` Z+90°, `arm_r` Z-90°, reste
//! identité). Upgrade data-driven possible (lire le template via
//! `SkeletonTemplateRegistry`, cf `apply_stance_offsets_from_template`) si on
//! tune la stance des PNJ dans le TOML — non nécessaire pour ce 1er passage.

use bevy::prelude::*;
use forgia_anim_locomotion::StanceOffsets;
use forgia_rig_topology::analyze_rig_topology;

/// Marqueur idempotent : PNJ dont les os ont déjà été posés (pose 1-shot).
#[derive(Component)]
pub struct NpcPosed;

/// Pose statique des PNJ : compose la stance Humanoid (bras le long) par-dessus
/// la rest pose auto-rig. 1-shot par PNJ. Retry tant que les os ne sont pas
/// spawnés (auto-rig async). N'écrit JAMAIS sur Rex (pas de component `Npc`).
pub fn sys_pose_npcs_idle(
    mut commands: Commands,
    q_npcs: Query<Entity, (With<crate::Npc>, Without<NpcPosed>)>,
    q_children: Query<&Children>,
    q_names: Query<&Name>,
    mut q_transform: Query<&mut Transform>,
) {
    if q_npcs.is_empty() {
        return;
    }
    let stance = StanceOffsets::humanoid_tpose();
    // Collecte d'abord → libère le borrow de q_npcs avant le get_mut sur les os.
    let npcs: Vec<Entity> = q_npcs.iter().collect();
    for npc in npcs {
        // Classification géométrique (robuste aux conventions de noms —
        // forgia-rig-topology). Closures en bloc isolé : le borrow immutable de
        // q_transform se termine AVANT le get_mut d'application ci-dessous.
        let topo = {
            let children_of = |e: Entity| {
                q_children
                    .get(e)
                    .map(|c| c.iter().collect())
                    .unwrap_or_default()
            };
            let transform_of = |e: Entity| q_transform.get(e).ok().copied();
            let name_of = |e: Entity| q_names.get(e).ok().map(|n| n.to_string());
            analyze_rig_topology(npc, &children_of, &transform_of, &name_of)
        };
        // Os pas encore spawnés (auto-rig async pas fini) → retry la frame suivante.
        if topo.left_arm.is_none() && topo.right_arm.is_none() {
            continue;
        }
        // Compose la stance PAR-DESSUS la rest pose (tf.rotation courante = bind).
        // legs/spine = identité pour Humanoid (no-op) — gardés pour généralité.
        let pairs: [(Option<Entity>, Quat); 5] = [
            (topo.left_arm, stance.arm_l),
            (topo.right_arm, stance.arm_r),
            (topo.left_leg, stance.leg_l),
            (topo.right_leg, stance.leg_r),
            (topo.spine, stance.spine),
        ];
        for (bone, q) in pairs {
            if let Some(e) = bone {
                if let Ok(mut tf) = q_transform.get_mut(e) {
                    tf.rotation *= q;
                }
            }
        }
        commands.entity(npc).insert(NpcPosed);
        info!("[forgia-rpg::npc_pose] PNJ {npc:?} posé (bras le long, stance Humanoid)");
    }
}
