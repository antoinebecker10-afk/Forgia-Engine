//! # forgia-mode-roguelite
//!
//! 3e jeu Forgia V2 — roguelite FPS coop 1-3 joueurs (cible Steam Next Fest).
//! Story-468 (plan global) / Story-470 (M1 fondations).
//!
//! ## Scope M1 (cette release)
//!
//! - `RunState` SubStates de `GameMode::Roguelite` : Lobby / InRun / Boss / Defeat / Victory
//! - `StartRunEvent` / `EndRunEvent` (Bevy 0.18 `Message` derive)
//! - `RunSeed` Resource déterministe (xoshiro256**)
//! - Sensor `forgia2_roguelite_state.json` 1Hz
//!
//! Combat / loot / biome / coop / méta-progression : M2+ (voir story-468).
//!
//! ## Cleanup OnExit
//!
//! `RogueliteRunMarker` Component est exposé. Le système `sys_cleanup_run_markers`
//! qui despawne ces entités est géré par un **terminal parallèle dédié** — ce crate
//! ne contient PAS la logique de despawn pour éviter conflit merge.

use bevy::prelude::*;
use forgia_core::prelude::*;

pub mod run;
pub mod sensor;

pub use run::{
    EndRunEvent, RogueliteRunMarker, RunResult, RunSeed, RunState, StartRunEvent,
};
pub use sensor::RogueliteTelemetry;

pub mod prelude {
    pub use crate::{
        EndRunEvent, ForgiaModeRoguelitePlugin, RogueliteRunMarker, RunResult, RunSeed,
        RunState, StartRunEvent,
    };
}

pub struct ForgiaModeRoguelitePlugin;

impl Plugin for ForgiaModeRoguelitePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<sensor::RogueliteTelemetry>()
            .add_sub_state::<RunState>()
            .add_message::<StartRunEvent>()
            .add_message::<EndRunEvent>()
            .add_systems(OnEnter(GameMode::Roguelite), run::sys_spawn_roguelite_scene)
            .add_systems(
                Update,
                (run::sys_start_run, run::sys_end_run)
                    .chain()
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            // Sensor cross-mode : tourne en tout état (menu = run_state "none").
            // Telemetry tick counter en First pour capturer chaque frame.
            .add_systems(First, sensor::sys_update_roguelite_telemetry)
            .add_systems(
                Update,
                sensor::sys_write_roguelite_state.in_set(GameSet::Sensors),
            );
        // Cleanup OnExit(GameMode::Roguelite) géré par terminal parallèle (V7 cleanup
        // orchestration). Ne PAS dupliquer ici.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaModeRoguelitePlugin;
    }
}
