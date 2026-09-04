//! Components pour les chaînes de spring bones (secondary motion).
//!
//! Une chaîne = un `SpringBoneChain` placé sur l'entité racine (le bone parent animé
//! normalement par un clip ou la pose statique), pointant vers une liste ordonnée
//! d'entités `SpringBone` (les bones suiveurs : queue, oreille, cape, cheveux).

use bevy::prelude::*;

/// Marqueur posé sur un bone qui doit être animé en secondary motion.
///
/// Le bone doit aussi avoir un `Transform` standard (parenté hiérarchique
/// Bevy classique). L'état runtime (positions Verlet) vit dans `SpringBoneState`,
/// inséré automatiquement par le solver au premier frame.
/// 🚨 `Reflect` + `register_type` ne sont pas décoratifs ici : un réglage de
/// tissu se juge à l'œil, et sans réflexion il fallait relancer le jeu à chaque
/// essai (le génome n'est lu qu'au démarrage). Réflexif, il se mute en direct
/// par BRP — `python tools/ai/brp.py call world.mutate_components …` — donc on
/// cherche la bonne valeur en regardant, puis on l'écrit au génome.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct SpringBone {
    /// Raideur du ressort. 0.0 = mou (suit la gravité), 1.0 = rigide (suit le parent).
    pub stiffness: f32,
    /// Amortissement de la vitesse Verlet. 0.0 = aucun (oscille longtemps), 1.0 = bloqué.
    pub damping: f32,
    /// Gravité monde appliquée chaque frame (m/s²). Permet d'avoir une cape qui tombe
    /// vs une oreille qui suit l'inertie sans tomber.
    pub gravity: Vec3,
    /// Ouverture maximale depuis la pose de repos, en RADIANS.
    ///
    /// Sans elle, une chaîne libre part à 180° au premier demi-tour : « elle
    /// réagit bien, trop bien même » (constaté à l'écran le 2026-08-21). Une
    /// étoffe cousue ne s'ouvre pas au-delà de quelques dizaines de degrés.
    /// `PI` = aucune limite.
    pub angle_max_rad: f32,
    /// Rayon du corps que l'os ne doit pas traverser, autour de l'axe vertical
    /// de la racine de chaîne (mètres). `0` = pas de volume.
    ///
    /// Un solveur sans collision fait passer la cape DANS le personnage dès
    /// qu'il tourne : le tissu suit l'inertie, le corps pivote, ils se croisent.
    pub rayon_corps_m: f32,
}

impl Default for SpringBone {
    fn default() -> Self {
        Self {
            stiffness: 0.4,
            damping: 0.7,
            gravity: Vec3::new(0.0, -9.81, 0.0),
            angle_max_rad: core::f32::consts::PI,
            rayon_corps_m: 0.0,
        }
    }
}

/// Chaîne de spring bones. À poser sur l'entité **parent** de la chaîne
/// (le bone "root" qui reçoit la pose du clip d'animation).
///
/// Les `bones` doivent être listés du plus proche du root au plus éloigné.
/// Chaque bone doit avoir un component `SpringBone`.
///
/// Les `rest_lengths` sont les distances **monde** initiales entre bones consécutifs
/// (root → bone[0], bone[0] → bone[1], etc.). Capturées au premier frame.
#[derive(Component, Clone, Debug, Default)]
pub struct SpringBoneChain {
    pub bones: Vec<Entity>,
    pub rest_lengths: Vec<f32>,
    /// Marqueur d'initialisation. Le solver capture les rest_lengths + positions
    /// initiales au premier passage si false.
    pub initialized: bool,
}

/// État runtime Verlet par bone. Inséré automatiquement par le solver.
///
/// Stocke la position monde actuelle + précédente pour intégration Verlet
/// `pos_new = 2*pos - pos_old + accel*dt²`.
#[derive(Component, Clone, Debug)]
pub struct SpringBoneState {
    pub pos_world: Vec3,
    pub prev_pos_world: Vec3,
}

impl SpringBoneState {
    pub fn new(pos: Vec3) -> Self {
        Self {
            pos_world: pos,
            prev_pos_world: pos,
        }
    }
}
