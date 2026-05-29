//! boons_sensor.rs — Producteur `forgia2_boons.json` (1Hz, cross-mode).
//!
//! Story-529 Phase 1 (Mission 2 GDD) — observability boons system.
//!
//! Tant que `ForgiaBoonsPlugin` n'est pas wiré côté `forgia-mode-roguelite`
//! (Phase 2, après sync autre terminal), les resources `BoonsCatalogue` et
//! `ActiveBoons` sont absentes → sensor émet severity `ok` `enabled=false`
//! pour indiquer que la foundation tourne sans active gameplay.
//!
//! ## Schema JSON
//!
//! ```json
//! {
//!   "id": "boons",
//!   "severity": "ok|warn",
//!   "next_step": "...",
//!   "timestamp_secs": 12.5,
//!   "enabled": true,
//!   "catalogue_entries": 9,
//!   "catalogue_legendary": 4,
//!   "active_count": 3,
//!   "total_choices_session": 5,
//!   "unlocked_legendary": 1,
//!   "tag_counts": {"fire": 2, "chaos": 1},
//!   "active_ids": ["metal_chaud","metal_chaud","souffle_du_maitre"]
//! }
//! ```

use bevy::prelude::*;
use forgia_rpg_data::boons::{ActiveBoons, BoonsCatalogue, BoonRarity};

const SENSOR_PATH: &str = "forgia2_boons.json";
const ACTIVE_IDS_DUMP: usize = 16;

#[derive(Resource, Default)]
pub struct BoonsSensorState {
    pub last_write_secs: f32,
}

pub fn sys_write_boons_sensor(
    time: Res<Time>,
    catalogue: Option<Res<BoonsCatalogue>>,
    active: Option<Res<ActiveBoons>>,
    mut state: ResMut<BoonsSensorState>,
) {
    let now = time.elapsed_secs();
    if now - state.last_write_secs < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let enabled = catalogue.is_some() && active.is_some();
    let (entries_total, legendary_total) = catalogue
        .as_ref()
        .map(|c| (c.entries.len(), c.count_by_rarity(BoonRarity::Legendary)))
        .unwrap_or((0, 0));

    let (active_count, total_choices, unlocked_leg, tags_json, ids_json) =
        if let Some(active) = active.as_ref() {
            let mut tags: Vec<(String, u32)> = active
                .tag_counts
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            tags.sort_by(|a, b| b.1.cmp(&a.1));
            let tags_json = tags
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", sanitize_json_str(k), v))
                .collect::<Vec<_>>()
                .join(",");
            let ids_json = active
                .active
                .iter()
                .take(ACTIVE_IDS_DUMP)
                .map(|id| format!("\"{}\"", sanitize_json_str(&id.0)))
                .collect::<Vec<_>>()
                .join(",");
            (
                active.active.len(),
                active.total_choices_this_session,
                active.unlocked_legendary.len(),
                tags_json,
                ids_json,
            )
        } else {
            (0, 0, 0, String::new(), String::new())
        };

    let (severity, next_step) = if !enabled {
        (
            "ok",
            "ForgiaBoonsPlugin not yet wired (Phase 2 mode-roguelite) — foundation OK",
        )
    } else if entries_total == 0 {
        (
            "warn",
            "boons catalogue empty — assets/genomes/roguelite_boons.toml missing or parse fail",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"boons","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"enabled":{enabled},"catalogue_entries":{entries_total},"catalogue_legendary":{legendary_total},"active_count":{active_count},"total_choices_session":{total_choices},"unlocked_legendary":{unlocked_leg},"tag_counts":{{{tags_json}}},"active_ids":[{ids_json}]}}"#,
        now,
    );

    if let Err(e) = std::fs::write(SENSOR_PATH, &json) {
        warn!("[forgia-observability] boons_sensor write failed: {e}");
    }
}

fn sanitize_json_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
