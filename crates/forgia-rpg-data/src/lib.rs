//! forgia-rpg-data — RPG data layer (fused 2026-05-26).
//!
//! Issu de la fusion de 5 single-file scaffold crates :
//! - `forgia-inventory` (199 LOC, LOCK-INV-1 80 slots)
//! - `forgia-quests` (203 LOC)
//! - `forgia-loot-tables` (129 LOC, drop tables genome-driven)
//! - `forgia-xp-curves` (146 LOC, level curves)
//! - `forgia-dialogue` (174 LOC, dialogue trees living_weapons)
//!
//! Toutes partageaient `bevy + forgia-core + serde` sans inter-dep. Coalescence
//! sous un même crate data-layer évite 5 unité-de-build distinctes pour 851 LOC.
//!
//! ## Modules
//!
//! Consommer via `forgia_rpg_data::dialogue::X`, `forgia_rpg_data::loot_tables::X`,
//! etc.

pub mod boons;
pub mod dialogue;
pub mod gold;
pub mod inventory;
pub mod items;
pub mod loot_tables;
pub mod quests;
pub mod shop;
pub mod xp_curves;

use bevy::prelude::*;

/// Meta-plugin agrégeant les 4 sous-systèmes data-layer RPG câblés runtime :
/// inventory + quests + xp_curves + dialogue. (loot_tables/boons sont Roguelite,
/// ajoutés par `forgia-mode-roguelite`.)
///
/// Story-570 (2026-06-02) : remplace l'ajout solo de `dialogue::ForgiaDialoguePlugin`
/// côté forgia-game pour activer la boucle dialogue → inventaire/quête → XP.
pub struct ForgiaRpgDataPlugin;

impl Plugin for ForgiaRpgDataPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            inventory::ForgiaInventoryPlugin,
            quests::ForgiaQuestsPlugin,
            xp_curves::ForgiaXpCurvesPlugin,
            dialogue::ForgiaDialoguePlugin,
            shop::ForgiaShopPlugin,
        ));
        // Story-58x Phase 0 : monnaie Or + catalogue d'objets data-driven.
        app.init_resource::<items::ItemRegistry>()
            .add_message::<gold::GoldChanged>()
            .add_systems(Startup, items::load_items_registry);
    }
}
