//! forgia-navigator — Thin wrapper around `vleue_navigator` 0.15 (navmesh pathfinding).
//!
//! Alternative to `bevy_landmass` for static navmesh use cases.

pub use vleue_navigator::*;

use bevy::prelude::*;

pub struct ForgiaNavigatorPlugin;

impl Plugin for ForgiaNavigatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(vleue_navigator::VleueNavigatorPlugin);
    }
}
