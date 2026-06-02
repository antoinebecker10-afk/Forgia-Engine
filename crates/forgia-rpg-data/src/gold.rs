//! forgia-rpg-data::gold — Monnaie Or du RPG (story-58x, fondation vendeur).
//!
//! `Gold` est un **Component sur le Player** (pas une Resource), cohérent avec
//! `Inventory` / `QuestLog` / `XpProgress` déjà components, et N-joueurs-safe.
//!
//! NB : distinct des `Souls` Roguelite (méta-progression hub) — pas de fusion.
//! Cf memory `project_roguelite_dual_currency_or_souls`.

use bevy::prelude::*;

/// Or du joueur (RPG). Greffé sur le Player en `GameMode::Rpg`.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Gold(pub u32);

impl Gold {
    /// Débite `amount` si solde suffisant. Retourne `true` si la transaction passe.
    pub fn try_spend(&mut self, amount: u32) -> bool {
        if self.0 >= amount {
            self.0 -= amount;
            true
        } else {
            false
        }
    }

    /// Crédite `amount` (saturating, pas d'overflow).
    pub fn add(&mut self, amount: u32) {
        self.0 = self.0.saturating_add(amount);
    }
}

/// Émis quand l'or d'un joueur change (UI / sensor / feedback).
#[derive(Message, Debug, Clone, Copy)]
pub struct GoldChanged {
    pub entity: Entity,
    pub new_total: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_spend_respects_balance() {
        let mut g = Gold(100);
        assert!(g.try_spend(40));
        assert_eq!(g.0, 60);
        assert!(!g.try_spend(99)); // insuffisant
        assert_eq!(g.0, 60); // inchangé
    }

    #[test]
    fn add_saturates() {
        let mut g = Gold(u32::MAX - 1);
        g.add(10);
        assert_eq!(g.0, u32::MAX);
    }
}
