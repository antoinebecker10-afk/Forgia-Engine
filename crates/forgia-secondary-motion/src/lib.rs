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
            // Sans cet enregistrement, `SpringBone` est invisible pour BRP : le
            // réglage du tissu se faisait alors à l'aveugle, une relance par
            // essai. Enregistré, il se règle en direct pendant qu'on regarde.
            .register_type::<SpringBone>()
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

    /// Banc du solveur sur une chaîne au repos, taillé sur le rig de la cape.
    ///
    /// # ⚠️ CE QU'IL NE COUVRE PAS — à lire avant de s'en servir comme preuve
    ///
    /// Il a été écrit le 2026-08-21 pour reproduire un coude de 89° observé en
    /// jeu là où la pose de liaison en a 16. **Il n'y arrive pas** : il reste
    /// vert alors que le jeu est cassé. Quatre ingrédients du jeu y ont été
    /// portés un par un — géométrie du rig (translations locales selon −X,
    /// redressées par la rotation de la racine), échelle 0,01 de l'armature,
    /// temps réellement avancé (sans quoi `delta` vaut quelques microsecondes
    /// et le solveur n'intègre rien), racine animée en continu comme le buste.
    /// Aucun ne déclenche le défaut.
    ///
    /// Il vaut donc comme **non-régression du cœur du solveur** — une chaîne au
    /// repos ne doit pas se plier — et **pas** comme couverture du défaut de la
    /// cape, qui reste ouvert. Ne pas le citer pour dire « la cape est
    /// couverte ».
    #[test]
    fn le_coeur_du_solveur_ne_plie_pas_une_chaine_au_repos() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::transform::TransformPlugin);
        app.add_plugins(ForgiaSecondaryMotionPlugin);

        // La racine tourne le repère : le −X local des os devient le bas du
        // monde, comme `root.001` le fait pour la cape.
        let vers_le_bas = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
        let longueurs = [0.162_f32, 0.182, 0.161, 0.184, 0.164];

        // 🚨 L'ARMATURE DE CAPE EST A L'ECHELLE 0,01 dans le GLB (mesure sur
        // `root.001`). Le solveur travaille en MONDE et ecrit du LOCAL : c'est
        // exactement le genre de detail qui ne se devine pas, donc le banc le
        // porte.
        let armature = app
            .world_mut()
            .spawn((
                Transform::from_scale(Vec3::splat(0.01)),
                GlobalTransform::default(),
            ))
            .id();
        let racine = app
            .world_mut()
            .spawn((
                Transform::from_rotation(vers_le_bas),
                GlobalTransform::default(),
            ))
            .id();
        app.world_mut().entity_mut(armature).add_child(racine);

        let mut os = Vec::new();
        let mut parent = racine;
        for longueur in longueurs {
            let e = app
                .world_mut()
                .spawn((
                    // En unites d'armature : 100 fois la longueur monde, comme
                    // les translations du rig (−16,2 ; −18,2 ; …).
                    Transform::from_translation(Vec3::new(-longueur * 100.0, 0.0, 0.0)),
                    GlobalTransform::default(),
                    SpringBone {
                        stiffness: 0.0,
                        damping: 0.97,
                        gravity: Vec3::new(0.0, -4.0, 0.0),
                        angle_max_rad: 45.0_f32.to_radians(),
                        rayon_corps_m: 0.0,
                    },
                ))
                .id();
            app.world_mut().entity_mut(parent).add_child(e);
            os.push(e);
            parent = e;
        }
        app.world_mut()
            .entity_mut(racine)
            .insert(SpringBoneChain {
                bones: os.clone(),
                ..default()
            });

        // 🚨 LE TEMPS NE PASSE PAS TOUT SEUL DANS UN TEST.
        //
        // Sous `MinimalPlugins`, `delta` vaut quelques microsecondes : 120
        // updates ne simulent que quelques millisecondes, le solveur n'integre
        // rien et le test passe en decrivant... rien. On avance donc le temps
        // explicitement, d'une frame de jeu a chaque tour.
        let frame = core::time::Duration::from_micros(16_667);
        app.world_mut().resource_mut::<Time>().advance_by(frame);
        app.update();
        let angles_repos = angles_des_maillons(&app, racine, &os);
        for (i, angle) in angles_repos.iter().enumerate() {
            assert!(
                *angle < 5.0,
                "pose de liaison deja de travers au maillon {i} : {angle:.1} deg"
            );
        }

        // 🚨 LA RACINE BOUGE, comme le buste anime du personnage.
        //
        // Un test statique ne prouvait rien : le solveur melange des positions
        // du MOMENT avec des rotations de parent de la frame PRECEDENTE, et ce
        // decalage ne se voit que sur du mouvement. Le clip d'idle fait
        // respirer le torse en permanence — meme « au repos », la racine de la
        // chaine tourne. On reproduit ce balancement lent.
        for i in 0..240 {
            let t = i as f32 / 60.0;
            let respiration = Quat::from_rotation_y((t * std::f32::consts::TAU * 0.5).sin() * 0.15);
            if let Some(mut tf) = app.world_mut().entity_mut(racine).get_mut::<Transform>() {
                tf.rotation = respiration * vers_le_bas;
            }
            app.world_mut().resource_mut::<Time>().advance_by(frame);
            app.update();
        }
        let angles_apres = angles_des_maillons(&app, racine, &os);
        let pire = angles_apres.iter().cloned().fold(0.0_f32, f32::max);
        assert!(
            pire < 20.0,
            "le solveur a plie une chaine au repos — angles entre maillons : {angles_apres:?}"
        );
    }

    /// L'angle, en degrés, entre chaque maillon et le précédent.
    fn angles_des_maillons(app: &App, racine: Entity, os: &[Entity]) -> Vec<f32> {
        let position = |e: Entity| {
            app.world()
                .entity(e)
                .get::<GlobalTransform>()
                .map(GlobalTransform::translation)
                .unwrap_or(Vec3::ZERO)
        };
        let mut points = vec![position(racine)];
        points.extend(os.iter().map(|&e| position(e)));
        let mut angles = Vec::new();
        for i in 1..points.len().saturating_sub(1) {
            let a = (points[i] - points[i - 1]).normalize_or_zero();
            let b = (points[i + 1] - points[i]).normalize_or_zero();
            if a == Vec3::ZERO || b == Vec3::ZERO {
                continue;
            }
            angles.push(a.dot(b).clamp(-1.0, 1.0).acos().to_degrees());
        }
        angles
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
