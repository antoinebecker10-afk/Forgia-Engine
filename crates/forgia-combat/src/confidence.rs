//! Jauge de confiance de Pépin — story-531 AC9 (GDD §2.2 « confiance gauge »).
//!
//! > Jauge 0-10 : +1 par tir qui touche un ennemi, −1 par tir raté (mur ou vide).
//! > Pépin est timide — la confiance se gagne coup par coup et se perd vite.
//!
//! Ce module porte les TYPES PARTAGÉS (state + message + logique pure) :
//! - écrivain : `forgia-fps` (fire path émet [`ShotResolved`], système pepin.rs
//!   met à jour [`PepinConfidence`], payoff dégâts genome-driven) ;
//! - lecteur : `forgia-ui-lib` (HUD cœurs cyan discrets).
//!
//! Les boons Pépin (story-530 : « Premier vrai tir », « Pépin s'enhardit »,
//! « Crénom de Pépin ! ») se grefferont sur cette même Resource.

use bevy::prelude::*;

use crate::weapons::WeaponType;

/// Résolution d'UN tir (pas par-pellet) : émise par le fire path à chaque tir
/// engagé, qu'il touche ou non. `hit_enemy=false` couvre le tir dans le vide
/// ET le tir bloqué par le décor (GDD : « −1/miss » — rater le mur compte).
#[derive(Message, Debug, Clone, Copy)]
pub struct ShotResolved {
    pub weapon: WeaponType,
    pub hit_enemy: bool,
}

/// État de la jauge — Resource globale (1 joueur, arme Pépin unique).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PepinConfidence {
    /// Stacks courants, 0..=max (max = genome, défaut 10).
    pub stacks: u8,
    /// Pic atteint sur la run (sensor + futur boon).
    pub peak_run: u8,
    /// Compteurs de run (sensor : accuracy = hits/(hits+misses)).
    pub hits_run: u32,
    pub misses_run: u32,
    /// Horodatage du dernier changement (le HUD fait clignoter le cœur).
    pub last_change_secs: f32,
}

impl PepinConfidence {
    /// Remise à zéro entre les runs (OnEnter Roguelite).
    pub fn reset_run(&mut self) {
        *self = Self::default();
    }
}

/// Logique pure de la jauge (testable headless) : +1/hit, −1/miss, saturée 0..=max.
pub fn apply_shot(stacks: u8, hit_enemy: bool, max_stacks: u8) -> u8 {
    if hit_enemy {
        stacks.saturating_add(1).min(max_stacks)
    } else {
        stacks.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_increments_to_max() {
        assert_eq!(apply_shot(0, true, 10), 1);
        assert_eq!(apply_shot(9, true, 10), 10);
        assert_eq!(apply_shot(10, true, 10), 10, "saturé au max genome");
    }

    #[test]
    fn miss_decrements_to_zero() {
        assert_eq!(apply_shot(5, false, 10), 4);
        assert_eq!(apply_shot(0, false, 10), 0, "plancher 0, pas d'underflow");
    }

    #[test]
    fn max_stacks_genome_lower_clamps() {
        // Hot-reload d'un max plus bas : les nouveaux gains saturent dessous.
        assert_eq!(apply_shot(7, true, 5), 5);
    }

    #[test]
    fn reset_run_clears_everything() {
        let mut c = PepinConfidence {
            stacks: 7,
            peak_run: 9,
            hits_run: 42,
            misses_run: 13,
            last_change_secs: 99.0,
        };
        c.reset_run();
        assert_eq!(c.stacks, 0);
        assert_eq!(c.peak_run, 0);
        assert_eq!(c.hits_run, 0);
    }
}
