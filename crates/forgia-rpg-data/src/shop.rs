//! forgia-rpg-data::shop — Commerce PNJ (vendeur style WoW), story-58x Phase 5.
//!
//! `ShopInventory` (Component sur le PNJ vendeur) liste les objets à l'achat +
//! ratio de revente. `ShopSession` (Component sur le Player) = boutique ouverte.
//! Transactions via events `BuyItem` / `SellItem` → débit/crédit `Gold` +
//! `Inventory` (cap-safe via add/consume_one → LOCK-INV-1 respecté).

use bevy::prelude::*;

use crate::gold::Gold;
use crate::inventory::{Inventory, ItemId, ItemStack};
use crate::items::ItemRegistry;

/// Étal d'un marchand. Greffé sur le PNJ vendeur (ex Mira).
#[derive(Component, Debug, Clone)]
pub struct ShopInventory {
    /// Objets proposés à l'achat (prix lu depuis `ItemRegistry`).
    pub items: Vec<ItemId>,
    /// Fraction du prix d'achat reversée au joueur quand il vend (0.0..=1.0).
    pub sell_ratio: f32,
}

impl Default for ShopInventory {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            sell_ratio: 0.33,
        }
    }
}

/// Boutique ouverte — greffée sur le Player tant que l'UI vendeur est active.
#[derive(Component, Debug, Clone, Copy)]
pub struct ShopSession {
    pub npc: Entity,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct OpenShop {
    pub player: Entity,
    pub npc: Entity,
}

#[derive(Message, Debug, Clone)]
pub struct BuyItem {
    pub player: Entity,
    pub id: ItemId,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct SellItem {
    pub player: Entity,
    pub slot: usize,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct CloseShop {
    pub player: Entity,
}

pub struct ForgiaShopPlugin;

impl Plugin for ForgiaShopPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OpenShop>()
            .add_message::<BuyItem>()
            .add_message::<SellItem>()
            .add_message::<CloseShop>()
            .add_systems(
                Update,
                (open_shop_sessions, close_shop_sessions, process_buy, process_sell),
            );
    }
}

fn open_shop_sessions(mut events: MessageReader<OpenShop>, mut commands: Commands) {
    for ev in events.read() {
        commands
            .entity(ev.player)
            .insert(ShopSession { npc: ev.npc });
    }
}

fn close_shop_sessions(mut events: MessageReader<CloseShop>, mut commands: Commands) {
    for ev in events.read() {
        commands.entity(ev.player).remove::<ShopSession>();
    }
}

fn process_buy(
    mut events: MessageReader<BuyItem>,
    registry: Res<ItemRegistry>,
    mut q: Query<(&mut Inventory, &mut Gold)>,
) {
    for ev in events.read() {
        let Ok((mut inv, mut gold)) = q.get_mut(ev.player) else {
            continue;
        };
        let price = registry.buy_price_of(&ev.id);
        if price == 0 {
            continue; // objet non vendu
        }
        if !gold.try_spend(price) {
            continue; // or insuffisant
        }
        let max = registry.max_stack_of(&ev.id);
        if let Some(_leftover) = inv.add(ItemStack::new(ev.id.0.clone(), 1, max)) {
            // Inventaire plein : transaction annulée, on rembourse.
            gold.add(price);
        }
    }
}

fn process_sell(
    mut events: MessageReader<SellItem>,
    registry: Res<ItemRegistry>,
    shops: Query<&ShopInventory>,
    mut q: Query<(&mut Inventory, &mut Gold, &ShopSession)>,
) {
    for ev in events.read() {
        let Ok((mut inv, mut gold, session)) = q.get_mut(ev.player) else {
            continue;
        };
        let ratio = shops.get(session.npc).map(|s| s.sell_ratio).unwrap_or(0.33);
        let Some(stack) = inv.slot(ev.slot) else {
            continue;
        };
        let unit_price = registry.buy_price_of(&stack.id);
        if unit_price == 0 {
            continue; // objet sans valeur marchande (quête, etc.)
        }
        let value = ((unit_price as f32) * ratio).round() as u32;
        inv.consume_one(ev.slot);
        gold.add(value.max(1)); // au moins 1 or
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shop_inventory_default_ratio() {
        let s = ShopInventory::default();
        assert!((s.sell_ratio - 0.33).abs() < 1e-6);
        assert!(s.items.is_empty());
    }
}
