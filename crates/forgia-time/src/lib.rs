//! forgia-time — Helpers around `Time<Real>` vs `Time<Virtual>`.
//!
//! V1 trap (story-2026-05-13) : sensors UI/menu using default `Time` (which is
//! `Time<Virtual>` and pauses with Bevy's `paused()`) gave structural false
//! positives in AppFlowTracker. Rule :
//!
//! - **UI / boot / load / menu** : `Time<Real>` (always advances)
//! - **Gameplay / AI / animation** : `Time<Virtual>` (pauses with game pause)
//!
//! Use the helpers below to be explicit and avoid mistakes.

use bevy::prelude::*;

/// Read wall-clock delta (never pauses). Use for UI, menu, loading screens.
pub fn real_delta(time: &Time<Real>) -> f32 {
    time.delta_secs()
}

/// Read gameplay delta (pauses with game). Use for movement, AI, anim.
pub fn virtual_delta(time: &Time<Virtual>) -> f32 {
    time.delta_secs()
}

/// Cumulative wall-clock elapsed since startup.
pub fn real_elapsed(time: &Time<Real>) -> f32 {
    time.elapsed_secs()
}

/// Cumulative virtual elapsed (pause-aware).
pub fn virtual_elapsed(time: &Time<Virtual>) -> f32 {
    time.elapsed_secs()
}

pub struct ForgiaTimePlugin;

impl Plugin for ForgiaTimePlugin {
    fn build(&self, _app: &mut App) {
        // Bevy already inserts Time<Real> and Time<Virtual> via DefaultPlugins.
        // This plugin exists for naming consistency + future Forgia-specific
        // time domains (e.g. day/night cycle separated from gameplay pause).
    }
}
