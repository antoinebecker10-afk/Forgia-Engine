//! forgia-inventory — Grid inventory with LOCK-INV-1 80-slot cap.
//!
//! Per CLAUDE.md §5 : 80 slots max with pagination UI. No expansion beyond 80.
//!
//! Item stacking, transfer between containers handled here. Equip slots are in
//! sibling crate forgia-equipment, crafting in forgia-crafting.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// LOCK-INV-1 hard cap. Don't increase without changing the Lock.
pub const INVENTORY_MAX_SLOTS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: ItemId,
    pub count: u32,
    pub max_stack: u32,
}

impl ItemStack {
    pub fn new(id: impl Into<String>, count: u32, max_stack: u32) -> Self {
        Self {
            id: ItemId(id.into()),
            count: count.min(max_stack),
            max_stack,
        }
    }

    pub fn can_merge(&self, other: &ItemStack) -> bool {
        self.id == other.id && self.count < self.max_stack
    }

    pub fn try_merge(&mut self, other: &mut ItemStack) {
        if !self.can_merge(other) { return; }
        let take = (self.max_stack - self.count).min(other.count);
        self.count += take;
        other.count -= take;
    }

    pub fn is_empty(&self) -> bool { self.count == 0 }
}

#[derive(Component, Debug, Clone)]
pub struct Inventory {
    slots: Vec<Option<ItemStack>>,
    capacity: usize,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::with_capacity(INVENTORY_MAX_SLOTS)
    }
}

impl Inventory {
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.min(INVENTORY_MAX_SLOTS);
        Self {
            slots: vec![None; capacity],
            capacity,
        }
    }

    pub fn capacity(&self) -> usize { self.capacity }
    pub fn slots(&self) -> &[Option<ItemStack>] { &self.slots }
    pub fn slot(&self, idx: usize) -> Option<&ItemStack> {
        self.slots.get(idx)?.as_ref()
    }

    /// Try to add a stack. Returns leftover if inventory full.
    pub fn add(&mut self, mut stack: ItemStack) -> Option<ItemStack> {
        for s in self.slots.iter_mut().flatten() {
            if s.can_merge(&stack) {
                s.try_merge(&mut stack);
                if stack.is_empty() { return None; }
            }
        }
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(stack);
                return None;
            }
        }
        Some(stack)
    }

    /// Remove `count` of `id`. Returns actual amount removed.
    pub fn remove(&mut self, id: &ItemId, mut count: u32) -> u32 {
        let mut removed = 0;
        for slot in &mut self.slots {
            if count == 0 { break; }
            if let Some(s) = slot {
                if s.id == *id {
                    let take = s.count.min(count);
                    s.count -= take;
                    removed += take;
                    count -= take;
                    if s.is_empty() { *slot = None; }
                }
            }
        }
        removed
    }

    pub fn count_of(&self, id: &ItemId) -> u32 {
        self.slots.iter().flatten().filter(|s| s.id == *id).map(|s| s.count).sum()
    }

    pub fn is_full(&self) -> bool {
        self.slots.iter().all(|s| s.is_some())
            && self.slots.iter().flatten().all(|s| s.count >= s.max_stack)
    }
}

#[derive(Message, Debug, Clone)]
pub enum InventoryEvent {
    Added { entity: Entity, id: ItemId, count: u32 },
    Removed { entity: Entity, id: ItemId, count: u32 },
    Full { entity: Entity, dropped: ItemStack },
}

pub struct ForgiaInventoryPlugin;

impl Plugin for ForgiaInventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<InventoryEvent>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_inv_1_cap_80() {
        let inv = Inventory::with_capacity(200);
        assert_eq!(inv.capacity(), 80, "LOCK-INV-1 caps at 80");
    }

    #[test]
    fn add_merges_then_fills_empty() {
        let mut inv = Inventory::with_capacity(3);
        assert!(inv.add(ItemStack::new("wood", 5, 10)).is_none());
        assert_eq!(inv.count_of(&ItemId("wood".into())), 5);
        assert!(inv.add(ItemStack::new("wood", 8, 10)).is_none());
        assert_eq!(inv.count_of(&ItemId("wood".into())), 13);
    }

    #[test]
    fn full_inventory_returns_leftover() {
        let mut inv = Inventory::with_capacity(1);
        inv.add(ItemStack::new("a", 1, 1));
        assert!(inv.add(ItemStack::new("b", 1, 1)).is_some());
    }

    #[test]
    fn remove_partial_then_full() {
        let mut inv = Inventory::with_capacity(5);
        inv.add(ItemStack::new("ore", 10, 64));
        assert_eq!(inv.remove(&ItemId("ore".into()), 7), 7);
        assert_eq!(inv.count_of(&ItemId("ore".into())), 3);
        assert_eq!(inv.remove(&ItemId("ore".into()), 99), 3);
        assert_eq!(inv.count_of(&ItemId("ore".into())), 0);
    }
}
