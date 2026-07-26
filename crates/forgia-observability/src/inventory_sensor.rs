//! inventory_sensor.rs — Producteur `forgia2_inventory.json` (1Hz, cross-mode).
//!
//! Story-555 (2026-05-28) — comble blind spot inventory + audit LOCK-INV-1.
//! forgia-rpg-data::inventory expose Inventory (Component) avec cap 80 hard
//! mais 0 sensor exposait l'état runtime.
//!
//! ## Schema JSON
//!
//! ```json
//! {
//!   "id": "inventory",
//!   "severity": "ok|warn",
//!   "next_step": "...",
//!   "timestamp_secs": 12.5,
//!   "inventory_present": true,
//!   "capacity": 80,
//!   "slots_used": 23,
//!   "slots_free": 57,
//!   "is_full": false,
//!   "total_items": 145,
//!   "unique_item_types": 12,
//!   "top_items": [
//!     {"id": "wood", "count": 64, "max_stack": 64}
//!   ]
//! }
//! ```
//!
//! Pas de `weight` / `equipped` — concepts pas implémentés V2.
//! NE PAS fabriquer (cf rule no-speculative-fix).
//!
//! ## LOCK-INV-1 check
//!
//! Si `capacity != INVENTORY_MAX_SLOTS` (=80) → severity `warn` flag.
//! Bug class : expansion silencieuse au-delà 80 (interdite par CLAUDE.md §5).

use bevy::prelude::*;
use forgia_player::Player;
use forgia_rpg_data::gold::Gold;
use forgia_rpg_data::inventory::{Inventory, ItemId, INVENTORY_MAX_SLOTS};
use std::collections::HashMap;

const TOP_ITEMS_DUMP: usize = 5;
const SENSOR_PATH: &str = "forgia2_inventory.json";

#[derive(Resource, Default)]
pub struct InventorySensorState {
    pub last_write_secs: f32,
}

pub fn sys_write_inventory_sensor(
    time: Res<Time>,
    q_player: Query<(&Inventory, Option<&Gold>), With<Player>>,
    mut state: ResMut<InventorySensorState>,
) {
    let now = time.elapsed_secs();
    if now - state.last_write_secs < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let player = q_player.iter().next();
    let inv = player.map(|(inv, _)| inv);
    let gold = player.and_then(|(_, g)| g).map(|g| g.0).unwrap_or(0);
    let inventory_present = inv.is_some();

    let mut capacity = 0usize;
    let mut slots_used = 0usize;
    let mut is_full = false;
    let mut total_items = 0u64;
    let mut counts: HashMap<ItemId, (u32, u32)> = HashMap::new(); // id → (count, max_stack)

    if let Some(inv) = inv {
        capacity = inv.capacity();
        is_full = inv.is_full();
        for slot in inv.slots().iter().flatten() {
            slots_used += 1;
            total_items += u64::from(slot.count);
            counts
                .entry(slot.id.clone())
                .and_modify(|(c, _)| *c += slot.count)
                .or_insert((slot.count, slot.max_stack));
        }
    }
    let slots_free = capacity.saturating_sub(slots_used);
    let unique_item_types = counts.len();

    let mut top: Vec<(ItemId, u32, u32)> =
        counts.into_iter().map(|(id, (c, m))| (id, c, m)).collect();
    top.sort_by_key(|t| std::cmp::Reverse(t.1));
    let top_json: String = top
        .iter()
        .take(TOP_ITEMS_DUMP)
        .map(|(id, c, m)| {
            let safe_id = sanitize_json_str(&id.0);
            format!(r#"{{"id":"{safe_id}","count":{c},"max_stack":{m}}}"#)
        })
        .collect::<Vec<_>>()
        .join(",");

    let (severity, next_step) = if !inventory_present {
        (
            "ok",
            "no Inventory Component — Menu/Pause/AppNotInGame OK, RPG sinon attention",
        )
    } else if capacity != INVENTORY_MAX_SLOTS {
        (
            "warn",
            "LOCK-INV-1 violation — capacity != 80, expansion interdite par CLAUDE.md §5",
        )
    } else if is_full {
        (
            "warn",
            "Inventory full — prochains pickups droppés au sol (InventoryEvent::Full)",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"inventory","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"inventory_present":{inventory_present},"gold":{gold},"capacity":{capacity},"slots_used":{slots_used},"slots_free":{slots_free},"is_full":{is_full},"total_items":{total_items},"unique_item_types":{unique_item_types},"top_items":[{top_json}]}}"#,
        now,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[forgia-observability] inventory_sensor write failed: {e}");
    }
}

fn sanitize_json_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
