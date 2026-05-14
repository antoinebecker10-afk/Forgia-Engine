//! forgia-rng
//!
//! Seeded deterministic RNG resource (bevy_rand wrapper)
//!
//! Category : foundation

use bevy::prelude::*;

/// Plugin Bevy. Add to App via pp.add_plugins(ForgiaRngPlugin).
pub struct ForgiaRngPlugin;

impl Plugin for ForgiaRngPlugin {
    fn build(&self, _app: &mut App) {
        // TODO: implement
    }
}
