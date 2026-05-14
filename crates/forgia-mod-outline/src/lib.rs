//! forgia-mod-outline — Thin wrapper around `bevy_mod_outline` 0.10.
//!
//! Re-exports upstream + provides a Forgia-named Plugin.

pub use bevy_mod_outline::*;

use bevy::prelude::*;

/// Forgia entry-point. Wraps `OutlinePlugin` for naming consistency.
pub struct ForgiaModOutlinePlugin;

impl Plugin for ForgiaModOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(OutlinePlugin);
    }
}
