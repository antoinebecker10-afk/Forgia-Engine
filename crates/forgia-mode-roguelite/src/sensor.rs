//! sensor.rs — Producteur `forgia2_roguelite_state.json` (1Hz, V7 M1).
//!
//! Story-470 — sensor canonique 13/13 V5+V7 cible.

use crate::run::{RunSeed, RunState};
use bevy::prelude::*;

pub fn sys_write_roguelite_state(
    time: Res<Time>,
    mut accum: Local<f32>,
    run_state: Option<Res<State<RunState>>>,
    run_seed: Option<Res<RunSeed>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (state_str, stage) = match run_state.as_ref().map(|s| s.get().clone()) {
        Some(RunState::Lobby) => ("lobby", 0u8),
        Some(RunState::InRun { stage }) => ("in_run", stage),
        Some(RunState::Boss { stage }) => ("boss", stage),
        Some(RunState::Defeat) => ("defeat", 0),
        Some(RunState::Victory) => ("victory", 0),
        None => ("none", 0),
    };
    let seed = run_seed.as_ref().map(|s| s.seed).unwrap_or(0);
    let stage_count = run_seed.as_ref().map(|s| s.stage_count).unwrap_or(0);

    let json = format!(
        r#"{{"id":"roguelite_state","severity":"ok","next_step":"","timestamp_secs":{:.1},"run_state":"{state_str}","stage":{stage},"stage_count":{stage_count},"seed":{seed}}}"#,
        time.elapsed_secs()
    );

    if let Err(e) = std::fs::write("forgia2_roguelite_state.json", &json) {
        warn!("[forgia-mode-roguelite] sensor write failed: {e}");
    }
}
