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
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct PlayerCombatMods {
    pub damage_mul: f32,
    pub fire_rate_mul: f32,
    pub damage_reduction: f32,
    pub crit_chance: f32,
    pub headshot_bonus_mul: f32,
    pub knockback_strength: f32,
    pub chain_extra_targets: u32,
    /// 2026-08-04 — **entretien** : vitesse de rechargement (>1 = plus rapide).
    ///
    /// Cette famille d'atouts ne buffe aucune arme : elle allonge le temps qu'on
    /// peut passer sur **la bonne**. C'est la seule qui renforce « toutes les
    /// armes vivantes, le choix vient de l'ennemi » au lieu de l'éroder — et
    /// c'est le moteur de DOOM Eternal, où la rareté des munitions FORCE la
    /// rotation d'armes.
    pub reload_speed_mul: f32,
    /// Taille du chargeur (et de la réserve). Même famille, même raison.
    ///
    /// ⚠️ À doser MODESTEMENT : en donner trop supprime la rotation au lieu de
    /// la servir. Il y a un optimum, pas une monotonie.
    pub mag_size_mul: f32,
    /// 2026-08-04 — **corps** : vitesse de déplacement (>1 = plus rapide).
    ///
    /// Famille NEUTRE pour le pilier : elle n'attache rien à une arme, donc elle
    /// ne crée aucune raison d'en préférer une. C'est un axe de build
    /// indépendant, et c'est précisément ce qui la rend sûre.
    ///
    /// ⚠️ Ne PAS étendre cette famille au saut. Les bots n'ont ni navmesh ni
    /// gravité : un joueur qui saute plus haut devient inatteignable
    /// (`spawn-clearance.md` §5). Ce n'est pas un atout, c'est un chantier IA.
    pub move_speed_mul: f32,
    /// 2026-08-04 — **récolte** : gain d'Or et d'Âmes (>1 = plus riche).
    ///
    /// ⚠️ Cette famille se multiplie avec elle-même : plus d'Or achète plus
    /// d'atouts, qui rapportent plus d'Or. Doser bas et rare, sinon c'est le
    /// snowball classique.
    pub loot_gain_mul: f32,
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
            reload_speed_mul: 1.0,
            mag_size_mul: 1.0,
            move_speed_mul: 1.0,
            loot_gain_mul: 1.0,
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
            reload_speed_mul: 1.8,
            mag_size_mul: 1.5,
            move_speed_mul: 1.4,
            loot_gain_mul: 1.3,
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
