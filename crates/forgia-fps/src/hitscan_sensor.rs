//! Hitscan sensor — diagnostic des raycasts armes (Forgia V2).
//!
//! **Pourquoi** : "j'aime parfaitement et ça ne fait pas de dégâts" est le
//! failure mode #1 d'un FPS. Le sensor enregistre chaque shot (origin,
//! direction, entité touchée, distance, et catégorisation Hit/Block/Miss)
//! pour qu'on puisse diagnostiquer sans guesswork.
//!
//! Source : research industry (GDC 2017 Overwatch Architecture Tim Ford,
//! pattern "log every cast" universel). Le sensor coût ~1µs / shot stocké
//! dans VecDeque borné + write JSON 1Hz.
//!
//! Output : `forgia_hitscan.json` à la racine workspace, format :
//! ```json
//! {
//!   "timestamp_secs": 12.34,
//!   "total_shots": 42,
//!   "hits_with_damage": 15,
//!   "hits_blocked_by_world": 20,
//!   "missed_no_hit": 7,
//!   "recent": [
//!     {
//!       "t": 12.30,
//!       "weapon": "ModernAR",
//!       "origin": [x, y, z],
//!       "dir": [x, y, z],
//!       "hit_entity_idx": 4123,
//!       "toi": 12.34,
//!       "category": "hit_zone_body" | "hit_zone_head" | "blocker" | "miss"
//!     },
//!     ...
//!   ]
//! }
//! ```

use bevy::prelude::*;
use forgia_combat::weapons::WeaponType;
use std::collections::VecDeque;
use std::fs;

/// Catégorisation du résultat d'un raycast — clé pour diagnostiquer "aiming on
/// target, no damage". Si `BlockerNonZone` apparaît alors qu'on visait le bot,
/// le ray est bloqué par autre chose AVANT le bot (mur, cover, player self).
#[derive(Debug, Clone, Copy)]
pub enum HitscanCategory {
    HitZoneHead,
    HitZoneBody,
    BlockerNonZone,
    Miss,
}

impl HitscanCategory {
    fn as_str(&self) -> &'static str {
        match self {
            Self::HitZoneHead => "hit_zone_head",
            Self::HitZoneBody => "hit_zone_body",
            Self::BlockerNonZone => "blocker",
            Self::Miss => "miss",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HitscanLogEntry {
    pub t: f32,
    pub weapon: WeaponType,
    pub origin: Vec3,
    pub dir: Vec3,
    pub hit_entity_idx: Option<u64>,
    pub hit_name: Option<String>,
    pub toi: Option<f32>,
    pub category: HitscanCategory,
}

#[derive(Resource)]
pub struct HitscanSensorState {
    pub recent: VecDeque<HitscanLogEntry>,
    pub total_shots: u32,
    pub hits_with_damage: u32,
    pub hits_blocked_by_world: u32,
    pub missed: u32,
    pub last_write_secs: f32,
}

impl Default for HitscanSensorState {
    fn default() -> Self {
        Self {
            recent: VecDeque::with_capacity(32),
            total_shots: 0,
            hits_with_damage: 0,
            hits_blocked_by_world: 0,
            missed: 0,
            last_write_secs: 0.0,
        }
    }
}

impl HitscanSensorState {
    pub fn push(&mut self, entry: HitscanLogEntry) {
        self.total_shots = self.total_shots.saturating_add(1);
        match entry.category {
            HitscanCategory::HitZoneHead | HitscanCategory::HitZoneBody => {
                self.hits_with_damage = self.hits_with_damage.saturating_add(1);
            }
            HitscanCategory::BlockerNonZone => {
                self.hits_blocked_by_world = self.hits_blocked_by_world.saturating_add(1);
            }
            HitscanCategory::Miss => {
                self.missed = self.missed.saturating_add(1);
            }
        }
        if self.recent.len() == 32 {
            self.recent.pop_front();
        }
        self.recent.push_back(entry);
    }
}

/// Système 1Hz : écrit `forgia_hitscan.json` à la racine workspace.
/// Story-455 Phase A — étendu avec `ammo_state` per weapon (current_mag/reserve/reloading/low).
pub fn write_hitscan_sensor(
    time: Res<Time>,
    mut state: ResMut<HitscanSensorState>,
    equipped: Res<forgia_combat::weapons::EquippedWeapons>,
) {
    let now = time.elapsed_secs();
    if now - state.last_write_secs < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let mut recent_json = String::with_capacity(state.recent.len() * 120);
    for (i, e) in state.recent.iter().enumerate() {
        if i > 0 {
            recent_json.push(',');
        }
        let hit_idx = match e.hit_entity_idx {
            Some(idx) => format!("{}", idx),
            None => "null".to_string(),
        };
        let hit_name = match &e.hit_name {
            Some(n) => format!(r#""{}""#, n.replace('"', "'")),
            None => "null".to_string(),
        };
        let toi_val = match e.toi {
            Some(t) => format!("{:.2}", t),
            None => "null".to_string(),
        };
        recent_json.push_str(&format!(
            r#"{{"t":{:.2},"weapon":"{:?}","origin":[{:.2},{:.2},{:.2}],"dir":[{:.3},{:.3},{:.3}],"hit_entity_idx":{},"hit_name":{},"toi":{},"category":"{}"}}"#,
            e.t,
            e.weapon,
            e.origin.x, e.origin.y, e.origin.z,
            e.dir.x, e.dir.y, e.dir.z,
            hit_idx,
            hit_name,
            toi_val,
            e.category.as_str(),
        ));
    }

    // ─── Ammo state per weapon (story-455 Phase A) ────────────────────────
    let mut ammo_json = String::with_capacity(equipped.slots.len() * 140);
    let mut first = true;
    for (weapon, slot) in equipped.iter_slots() {
        if !first {
            ammo_json.push(',');
        }
        first = false;
        let reload_progress = slot.reload_state.progress(slot.config.reload_time_secs);
        ammo_json.push_str(&format!(
            r#""{:?}":{{"current_mag":{},"mag_size":{},"reserve":{},"reserve_max":{},"reload_kind":"{:?}","reloading":{},"reload_progress":{:.3},"infinite":{},"is_low":{},"is_current":{}}}"#,
            weapon,
            slot.current_mag, slot.config.mag_size,
            slot.reserve, slot.config.reserve_max,
            slot.config.reload_kind,
            slot.reload_state.is_reloading(),
            reload_progress,
            slot.config.infinite_ammo,
            slot.is_low(),
            weapon == equipped.current,
        ));
    }

    let json = format!(
        r#"{{"timestamp_secs":{:.2},"total_shots":{},"hits_with_damage":{},"hits_blocked_by_world":{},"missed_no_hit":{},"current_weapon":"{:?}","ammo_state":{{{}}},"recent":[{}]}}"#,
        now,
        state.total_shots,
        state.hits_with_damage,
        state.hits_blocked_by_world,
        state.missed,
        equipped.current,
        ammo_json,
        recent_json,
    );

    let _ = fs::write("forgia_hitscan.json", json);
}
