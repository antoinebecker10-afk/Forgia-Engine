//! forgia-gauge — Thin wrapper around `bevy_gauge` 0.4 (attribute/stat system).
//!
//! Use cases : XP curves, HP/MP/Stamina, modifiers stacking, derived stats.

pub use bevy_gauge::*;

use bevy::prelude::*;

/// Forgia entry-point. Wraps the upstream `GaugePlugin`.
pub struct ForgiaGaugePlugin;

impl Plugin for ForgiaGaugePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_gauge::plugin::AttributesPlugin);
    }
}
