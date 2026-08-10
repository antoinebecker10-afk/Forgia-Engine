//! # forgia-secondary-motion
//!
//! Secondary motion (queues, oreilles, capes, cheveux) via spring/Verlet sur les bones
//! Bevy natifs. **Forgia Anim Layer Phase 1.**
//!
//! ## Usage
//!
//! ```ignore
//! use forgia_secondary_motion::{ForgiaSecondaryMotionPlugin, SpringBone, SpringBoneChain};
//!
//! app.add_plugins(ForgiaSecondaryMotionPlugin);
//!
//! // Sur une entité : placer SpringBoneChain au root, SpringBone sur chaque bone suiveur
//! commands.entity(tail_root).insert(SpringBoneChain {
//!     bones: vec![tail_01, tail_02, tail_03, tail_04],
//!     ..default()
//! });
//! for bone in [tail_01, tail_02, tail_03, tail_04] {
//!     commands.entity(bone).insert(SpringBone::default());
//! }
//! ```
//!
//! ## Architecture
//!
//! - `spring_bone.rs` — Components `SpringBone`, `SpringBoneChain`, `SpringBoneState`
//! - `solver.rs`     — System `update_spring_bones` (Verlet + contraintes de distance)
//!
//! ## Timing
//!
//! Le solver tourne dans `PostUpdate` APRÈS l'animation des clips et AVANT
//! `TransformSystem::TransformPropagate`. Garantit que les bones parents ont leur
//! pose finale du frame avant qu'on simule les suiveurs.
//!
//! ## Hot path discipline
//!
//! - `Local<Vec<Vec3>>` scratch buffer réutilisé
//! - `run_if(any_with_component::<SpringBoneChain>)` skip complet sans chaîne
//! - Budget cible : 500 µs/frame pour 12 chaînes × 4 bones

use bevy::prelude::*;
use bevy::transform::TransformSystems;

pub mod solver;
pub mod spring_bone;

pub use spring_bone::{SpringBone, SpringBoneChain, SpringBoneState};

pub mod prelude {
    pub use crate::spring_bone::{SpringBone, SpringBoneChain, SpringBoneState};
    pub use crate::ForgiaSecondaryMotionPlugin;
}

/// Plugin Bevy. À ajouter via `app.add_plugins(ForgiaSecondaryMotionPlugin)`.
pub struct ForgiaSecondaryMotionPlugin;

impl Plugin for ForgiaSecondaryMotionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<forgia_anim_debug::AnimLayerStats>()
            .add_systems(
                PostUpdate,
                solver::update_spring_bones
                    .before(TransformSystems::Propagate)
                    .run_if(any_with_component::<SpringBoneChain>),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_builds_without_panic() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::transform::TransformPlugin);
        app.add_plugins(ForgiaSecondaryMotionPlugin);
    }

    #[test]
    fn empty_chain_does_nothing() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::transform::TransformPlugin);
        app.add_plugins(ForgiaSecondaryMotionPlugin);

        let root = app
            .world_mut()
            .spawn((
                Transform::default(),
                GlobalTransform::default(),
                SpringBoneChain::default(),
            ))
            .id();

        app.update();

        // Ne doit pas paniquer même avec une chaîne vide
        let chain = app.world().entity(root).get::<SpringBoneChain>().unwrap();
        assert!(chain.bones.is_empty());
    }
}
