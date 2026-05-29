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
///   (lue par forgia-damage::apply_damage via HealthGuard component).
/// - `crit_chance` (Phase 4b) : 0..1, probabilité crit (×2 damage) par tir.
/// - `headshot_bonus_mul` (Phase 4b) : multiplier additif appliqué au
///   `zone_mul` quand HitZone::Head (ex: +0.5 = headshot 1.5× plus fort).
/// - `knockback_strength` (Phase 4b) : impulse appliqué sur enemy au hit.
/// - `chain_extra_targets` (Phase 4b) : N raycasts cascade après 1er hit.
///
/// Default neutre (= no-op). Recompute idempotent par les modes qui mutent.
#[derive(Resource, Debug, Clone, Copy)]
pub struct PlayerCombatMods {
    pub damage_mul: f32,
    pub fire_rate_mul: f32,
    pub damage_reduction: f32,
    pub crit_chance: f32,
    pub headshot_bonus_mul: f32,
    pub knockback_strength: f32,
    pub chain_extra_targets: u32,
}

impl Default for PlayerCombatMods {
    fn default() -> Self {
        Self {
            damage_mul: 1.0,
            fire_rate_mul: 1.0,
            damage_reduction: 0.0,
            crit_chance: 0.0,
            headshot_bonus_mul: 0.0,
            knockback_strength: 0.0,
            chain_extra_targets: 0,
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
        assert_eq!(m.crit_chance, 0.0);
        assert_eq!(m.headshot_bonus_mul, 0.0);
        assert_eq!(m.knockback_strength, 0.0);
        assert_eq!(m.chain_extra_targets, 0);
    }

    #[test]
    fn reset_restores_neutral() {
        let mut m = PlayerCombatMods {
            damage_mul: 2.5,
            fire_rate_mul: 1.5,
            damage_reduction: 0.4,
            crit_chance: 0.3,
            headshot_bonus_mul: 0.5,
            knockback_strength: 25.0,
            chain_extra_targets: 3,
        };
        m.reset();
        assert_eq!(m.damage_mul, 1.0);
        assert_eq!(m.fire_rate_mul, 1.0);
        assert_eq!(m.damage_reduction, 0.0);
        assert_eq!(m.crit_chance, 0.0);
        assert_eq!(m.headshot_bonus_mul, 0.0);
        assert_eq!(m.knockback_strength, 0.0);
        assert_eq!(m.chain_extra_targets, 0);
    }
}
