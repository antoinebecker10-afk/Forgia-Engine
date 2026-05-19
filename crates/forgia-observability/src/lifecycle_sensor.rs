//! lifecycle_sensor.rs — Producteur `forgia2_lifecycle.json` (1Hz, cross-mode).
//!
//! Compte les Add/Remove/Insert sur 3 markers clés via Observer hooks Bevy 0.18 :
//! - `Player` (forgia-player) : Add + Remove
//! - `TargetCube` (forgia-mode-fps-arena) : Add + Remove
//! - `NameplateRoot` (forgia-enemy-nameplate) : Insert
//!
//! Les compteurs sont des **deltas par-seconde** : reset à chaque write 1Hz.
//!
//! ## Bevy 0.18 syntax (research 2026-05-19)
//!
//! `On<Add, C>` / `On<Remove, C>` / `On<Insert, C>` — renommé depuis
//! `Trigger<OnAdd, C>` en 0.18 (PR #19596). Imports : `bevy::prelude::*`.
//!
//! Story-467 → Story-469 — Vague 5 Phase 5b Session C.

use bevy::prelude::*;
use forgia_ai_arena_bot::ArenaBot;
use forgia_enemy_nameplate::NameplateRoot;
use forgia_mode_fps_arena::TargetCube;
use forgia_player::Player;

#[derive(Resource, Default)]
pub struct LifecycleCounter {
    pub players_added: u32,
    pub players_removed: u32,
    pub target_cubes_added: u32,
    pub target_cubes_removed: u32,
    pub nameplates_inserted: u32,
    pub arena_bots_added: u32,
    pub arena_bots_removed: u32,
}

/// Pur — extrait pour tests headless.
pub fn severity_for_lifecycle(players_removed: u32) -> (&'static str, &'static str) {
    if players_removed > 0 {
        (
            "warn",
            "player removed during last second — unexpected outside mode switch",
        )
    } else {
        ("ok", "")
    }
}

pub fn obs_player_added(_: On<Add, Player>, mut c: ResMut<LifecycleCounter>) {
    c.players_added += 1;
}
pub fn obs_player_removed(_: On<Remove, Player>, mut c: ResMut<LifecycleCounter>) {
    c.players_removed += 1;
}
pub fn obs_target_cube_added(_: On<Add, TargetCube>, mut c: ResMut<LifecycleCounter>) {
    c.target_cubes_added += 1;
}
pub fn obs_target_cube_removed(_: On<Remove, TargetCube>, mut c: ResMut<LifecycleCounter>) {
    c.target_cubes_removed += 1;
}
pub fn obs_nameplate_inserted(_: On<Insert, NameplateRoot>, mut c: ResMut<LifecycleCounter>) {
    c.nameplates_inserted += 1;
}
pub fn obs_arena_bot_added(_: On<Add, ArenaBot>, mut c: ResMut<LifecycleCounter>) {
    c.arena_bots_added += 1;
}
pub fn obs_arena_bot_removed(_: On<Remove, ArenaBot>, mut c: ResMut<LifecycleCounter>) {
    c.arena_bots_removed += 1;
}

pub fn sys_write_lifecycle_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut counter: ResMut<LifecycleCounter>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (severity, next_step) = severity_for_lifecycle(counter.players_removed);

    let json = format!(
        r#"{{"id":"lifecycle","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"players_added":{},"players_removed":{},"target_cubes_added":{},"target_cubes_removed":{},"nameplates_inserted":{},"arena_bots_added":{},"arena_bots_removed":{}}}"#,
        time.elapsed_secs(),
        counter.players_added,
        counter.players_removed,
        counter.target_cubes_added,
        counter.target_cubes_removed,
        counter.nameplates_inserted,
        counter.arena_bots_added,
        counter.arena_bots_removed,
    );

    if let Err(e) = std::fs::write("forgia2_lifecycle.json", &json) {
        warn!("[forgia-observability] lifecycle sensor write failed: {e}");
    }

    // Reset deltas après écriture — cumul par-seconde.
    *counter = LifecycleCounter::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_when_no_player_removed() {
        let (sev, next) = severity_for_lifecycle(0);
        assert_eq!(sev, "ok");
        assert!(next.is_empty());
    }

    #[test]
    fn severity_warn_when_player_removed() {
        let (sev, next) = severity_for_lifecycle(1);
        assert_eq!(sev, "warn");
        assert!(next.contains("player removed"));
    }

    #[test]
    fn lifecycle_counter_default_zero() {
        let c = LifecycleCounter::default();
        assert_eq!(c.players_added, 0);
        assert_eq!(c.target_cubes_added, 0);
        assert_eq!(c.nameplates_inserted, 0);
    }
}
