//! forgia-rpg-data::items — Catalogue d'objets data-driven (`ItemRegistry`).
//!
//! `ItemId` (inventory.rs) est une String opaque. `ItemDef` lui associe nom
//! d'affichage, icône (optionnelle), max_stack, prix, rareté, catégorie,
//! description — nécessaires pour les sacs-à-icônes, tooltips, et le vendeur.
//!
//! Source : `config/items/items.toml` (loader serde) + fallback hardcodé si
//! absent (pattern genome V2). La rareté expose `tier_index()` (pas de couleur
//! egui ici — la couleur `FORGE_RARITY_*` est mappée côté forgia-ui-lib pour
//! éviter une dép egui dans le data layer).

use crate::inventory::ItemId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Rareté d'objet (5 paliers façon Diablo/Hearthstone). `tier_index()` 0..=4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// 0 (Common) .. 4 (Legendary). La couche UI mappe vers `FORGE_RARITY_*`.
    pub fn tier_index(&self) -> u8 {
        match self {
            Rarity::Common => 0,
            Rarity::Uncommon => 1,
            Rarity::Rare => 2,
            Rarity::Epic => 3,
            Rarity::Legendary => 4,
        }
    }
}

/// Catégorie fonctionnelle (filtre vendeur / onglets futurs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ItemCategory {
    #[default]
    Misc,
    Consumable,
    Weapon,
    Armor,
    Quest,
}

/// Définition statique d'un objet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: ItemId,
    pub display_name: String,
    /// Chemin d'icône (asset) — `None` => fallback couleur+initiales côté UI.
    #[serde(default)]
    pub icon: Option<String>,
    pub max_stack: u32,
    #[serde(default)]
    pub buy_price: u32,
    #[serde(default)]
    pub rarity: Rarity,
    #[serde(default)]
    pub category: ItemCategory,
    #[serde(default)]
    pub description: String,
    /// Soin appliqué quand l'objet est utilisé (clic droit). 0 = non consommable.
    #[serde(default)]
    pub heal_amount: u32,
}

/// Registre de tous les objets connus. Peuplé au Startup (TOML + fallback).
#[derive(Resource, Default)]
pub struct ItemRegistry {
    defs: HashMap<ItemId, ItemDef>,
}

impl ItemRegistry {
    pub fn register(&mut self, def: ItemDef) {
        self.defs.insert(def.id.clone(), def);
    }
    pub fn get(&self, id: &ItemId) -> Option<&ItemDef> {
        self.defs.get(id)
    }
    /// max_stack de l'objet, fallback 99 si inconnu (anti-crash data manquante).
    pub fn max_stack_of(&self, id: &ItemId) -> u32 {
        self.get(id).map(|d| d.max_stack).unwrap_or(99)
    }
    /// Prix d'achat (0 si inconnu / non vendu).
    pub fn buy_price_of(&self, id: &ItemId) -> u32 {
        self.get(id).map(|d| d.buy_price).unwrap_or(0)
    }
    pub fn len(&self) -> usize {
        self.defs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

#[derive(Debug, Deserialize)]
struct ItemsToml {
    #[serde(default)]
    items: Vec<ItemDef>,
}

const ITEMS_TOML_PATH: &str = "config/items/items.toml";

/// Startup : charge `config/items/items.toml`, fallback hardcodé si absent.
pub fn load_items_registry(mut registry: ResMut<ItemRegistry>) {
    match std::fs::read_to_string(ITEMS_TOML_PATH) {
        Ok(text) => {
            match toml::from_str::<ItemsToml>(&text) {
                Ok(parsed) => {
                    let n = parsed.items.len();
                    for def in parsed.items {
                        registry.register(def);
                    }
                    info!("[forgia-rpg-data] ItemRegistry : {n} objets chargés depuis {ITEMS_TOML_PATH}");
                }
                Err(e) => {
                    warn!("[forgia-rpg-data] items.toml parse échoué ({e}) — fallback hardcodé");
                    register_fallback(&mut registry);
                }
            }
        }
        Err(_) => {
            info!("[forgia-rpg-data] {ITEMS_TOML_PATH} absent — fallback hardcodé");
            register_fallback(&mut registry);
        }
    }
}

/// Catalogue minimal couvrant les objets référencés par les dialogues/quêtes
/// sample (story-570) — garantit un runtime sain même sans TOML.
fn register_fallback(registry: &mut ItemRegistry) {
    let defs = [
        ItemDef {
            id: ItemId("iron_dagger".into()),
            display_name: "Dague de fer".into(),
            icon: None,
            max_stack: 1,
            buy_price: 25,
            rarity: Rarity::Common,
            category: ItemCategory::Weapon,
            description: "Une lame simple mais fiable.".into(),
            heal_amount: 0,
        },
        ItemDef {
            id: ItemId("potion_heal".into()),
            display_name: "Potion de soin".into(),
            icon: None,
            max_stack: 10,
            buy_price: 10,
            rarity: Rarity::Common,
            category: ItemCategory::Consumable,
            description: "Restaure un peu de vitalité.".into(),
            heal_amount: 25,
        },
        ItemDef {
            id: ItemId("health_potion".into()),
            display_name: "Grande potion de soin".into(),
            icon: None,
            max_stack: 10,
            buy_price: 20,
            rarity: Rarity::Uncommon,
            category: ItemCategory::Consumable,
            description: "Restaure beaucoup de vitalité.".into(),
            heal_amount: 50,
        },
    ];
    for d in defs {
        registry.register(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_max_stack_and_price() {
        let mut reg = ItemRegistry::default();
        register_fallback(&mut reg);
        assert_eq!(reg.max_stack_of(&ItemId("iron_dagger".into())), 1);
        assert_eq!(reg.buy_price_of(&ItemId("potion_heal".into())), 10);
        // inconnu → fallback 99 / 0
        assert_eq!(reg.max_stack_of(&ItemId("unknown".into())), 99);
        assert_eq!(reg.buy_price_of(&ItemId("unknown".into())), 0);
    }

    #[test]
    fn rarity_tiers() {
        assert_eq!(Rarity::Common.tier_index(), 0);
        assert_eq!(Rarity::Legendary.tier_index(), 4);
    }
}
