//! combat_mods.rs — Story-558 Phase 4 (2026-05-29).
//!
//! Resource `PlayerCombatMods` : multipliers cumulatifs appliqués au tir du
//! Player. Consommé par forgia-fps (fire system) et forgia-mode-roguelite
//! (boons apply + Coffre du Forgeron).
//!
//! Pattern : Resource neutre par défaut (1.0 / 0.0), mutée par les modes qui
//! veulent appliquer des bonus. Cross-crate safe car forgia-combat est dep
//! root de forgia-fps + forgia-mode-roguelite.

use bevy::prelude::*;

/// Multiplicateurs joueur cumulés.
///
/// Cibles d'application :
/// - `damage_mul` : multiplie `effective_dmg` dans forgia-fps hit system.
/// - `fire_rate_mul` : multiplie le taux de tir (cooldown divisé).
/// - `damage_reduction` : 0..1, fraction de dégâts évitée sur Player
///   (lue par forgia-damage::apply_damage ; Phase 4b).
///
/// Default neutre (= no-op) : `damage_mul=1.0, fire_rate_mul=1.0,
/// damage_reduction=0.0`. Recompute idempotent par les modes qui mutent.
#[derive(Resource, Debug, Clone, Copy)]
pub struct PlayerCombatMods {
    pub damage_mul: f32,
    pub fire_rate_mul: f32,
    pub damage_reduction: f32,
}

impl Default for PlayerCombatMods {
    fn default() -> Self {
        Self {
            damage_mul: 1.0,
            fire_rate_mul: 1.0,
            damage_reduction: 0.0,
        }
    }
}

impl PlayerCombatMods {
    /// Restaure les valeurs neutres (utilisé OnExit Roguelite).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_neutral() {
        let m = PlayerCombatMods::default();
        assert_eq!(m.damage_mul, 1.0);
        assert_eq!(m.fire_rate_mul, 1.0);
        assert_eq!(m.damage_reduction, 0.0);
    }

    #[test]
    fn reset_restores_neutral() {
        let mut m = PlayerCombatMods {
            damage_mul: 2.5,
            fire_rate_mul: 1.5,
            damage_reduction: 0.4,
        };
        m.reset();
        assert_eq!(m.damage_mul, 1.0);
        assert_eq!(m.fire_rate_mul, 1.0);
        assert_eq!(m.damage_reduction, 0.0);
    }
}
