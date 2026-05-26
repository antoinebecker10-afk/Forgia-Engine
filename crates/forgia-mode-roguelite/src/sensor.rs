//! sensor.rs — Producteur `forgia2_roguelite_state.json` (1Hz, V7 M1).
//!
//! Story-470 — sensor canonique 13/13. Telemetry runtime augmentée (story-471 quick) :
//! - run_state, stage, stage_count, seed
//! - tick_count (compteur frames depuis insertion plugin)
//! - elapsed_secs (time depuis boot)
//! - time_in_state_secs (durée depuis dernière transition RunState)
//! - transitions_count (nombre total transitions RunState observées)
//! - severity warn si stuck >30min en InRun (proxy run gelée)

use crate::run::{RunSeed, RunState};
use crate::waves::{RogueliteWave, WAVES_TOTAL};
use bevy::prelude::*;
use forgia_rpg_data::loot_tables::Souls;

/// Telemetry runtime — incrémentée chaque frame par `sys_update_roguelite_telemetry`.
#[derive(Resource, Default, Debug, Clone)]
pub struct RogueliteTelemetry {
    pub tick_count: u64,
    pub time_in_state_secs: f32,
    pub transitions_count: u32,
    pub last_state_label: Option<&'static str>,
}

const STUCK_RUN_THRESHOLD_SECS: f32 = 30.0 * 60.0; // 30 min sans transition = warn

/// Pur — extrait pour tests headless.
pub fn severity_for_roguelite(
    time_in_state_secs: f32,
    state_label: &str,
) -> (&'static str, &'static str) {
    if state_label == "in_run" && time_in_state_secs > STUCK_RUN_THRESHOLD_SECS {
        (
            "warn",
            "InRun > 30min sans transition — run possiblement gelée (boss pas tué ?)",
        )
    } else {
        ("ok", "")
    }
}

fn state_label(rs: Option<&State<RunState>>) -> (&'static str, u8) {
    match rs.map(|s| s.get().clone()) {
        Some(RunState::Lobby) => ("lobby", 0u8),
        Some(RunState::InRun { stage }) => ("in_run", stage),
        Some(RunState::Boss { stage }) => ("boss", stage),
        Some(RunState::Defeat) => ("defeat", 0),
        Some(RunState::Victory) => ("victory", 0),
        None => ("none", 0),
    }
}

/// Tourne chaque frame — incrémente ticks + détecte transitions RunState.
pub fn sys_update_roguelite_telemetry(
    time: Res<Time>,
    run_state: Option<Res<State<RunState>>>,
    mut tel: ResMut<RogueliteTelemetry>,
) {
    tel.tick_count = tel.tick_count.saturating_add(1);

    let (current_label, _) = state_label(run_state.as_deref());
    let changed = tel.last_state_label != Some(current_label);
    if changed {
        tel.last_state_label = Some(current_label);
        tel.time_in_state_secs = 0.0;
        tel.transitions_count = tel.transitions_count.saturating_add(1);
    } else {
        tel.time_in_state_secs += time.delta_secs();
    }
}

/// Écrit le sensor JSON 1Hz.
pub fn sys_write_roguelite_state(
    time: Res<Time>,
    mut accum: Local<f32>,
    run_state: Option<Res<State<RunState>>>,
    run_seed: Option<Res<RunSeed>>,
    tel: Res<RogueliteTelemetry>,
    souls: Option<Res<Souls>>,
    wave: Option<Res<RogueliteWave>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (state_str, stage) = state_label(run_state.as_deref());
    let seed = run_seed.as_ref().map(|s| s.seed).unwrap_or(0);
    let stage_count = run_seed.as_ref().map(|s| s.stage_count).unwrap_or(0);
    let (severity, next_step) = severity_for_roguelite(tel.time_in_state_secs, state_str);
    let souls_current = souls.as_ref().map(|s| s.current).unwrap_or(0);
    let souls_total = souls.as_ref().map(|s| s.total_collected).unwrap_or(0);
    let current_wave = wave.as_ref().map(|w| w.current_wave).unwrap_or(0);
    let bots_alive = wave.as_ref().map(|w| w.bots_alive).unwrap_or(0);
    let break_secs_left = wave.as_ref().map(|w| w.break_secs_left).unwrap_or(0.0);
    let in_break = wave.as_ref().map(|w| w.in_break).unwrap_or(false);
    let victory = wave.as_ref().map(|w| w.victory_emitted).unwrap_or(false);

    let json = format!(
        r#"{{"id":"roguelite_state","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"run_state":"{state_str}","stage":{stage},"stage_count":{stage_count},"seed":{seed},"tick_count":{},"time_in_state_secs":{:.1},"transitions_count":{},"elapsed_secs":{:.1},"souls_current":{souls_current},"souls_total":{souls_total},"current_wave":{current_wave},"waves_total":{WAVES_TOTAL},"bots_alive":{bots_alive},"in_break":{in_break},"break_secs_left":{:.1},"victory":{victory}}}"#,
        time.elapsed_secs(),
        tel.tick_count,
        tel.time_in_state_secs,
        tel.transitions_count,
        time.elapsed_secs(),
        break_secs_left,
    );

    if let Err(e) = std::fs::write("forgia2_roguelite_state.json", &json) {
        warn!("[forgia-mode-roguelite] sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_in_lobby() {
        assert_eq!(severity_for_roguelite(0.0, "lobby").0, "ok");
        assert_eq!(severity_for_roguelite(99999.0, "lobby").0, "ok");
    }

    #[test]
    fn severity_ok_in_run_under_threshold() {
        assert_eq!(severity_for_roguelite(1000.0, "in_run").0, "ok");
        assert_eq!(
            severity_for_roguelite(STUCK_RUN_THRESHOLD_SECS, "in_run").0,
            "ok"
        );
    }

    #[test]
    fn severity_warn_in_run_stuck() {
        let (sev, next) = severity_for_roguelite(STUCK_RUN_THRESHOLD_SECS + 0.1, "in_run");
        assert_eq!(sev, "warn");
        assert!(next.contains("30min"));
    }

    #[test]
    fn telemetry_default_zero() {
        let t = RogueliteTelemetry::default();
        assert_eq!(t.tick_count, 0);
        assert_eq!(t.time_in_state_secs, 0.0);
        assert_eq!(t.transitions_count, 0);
        assert!(t.last_state_label.is_none());
    }
}
