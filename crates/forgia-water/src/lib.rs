//! # forgia-water
//!
//! Thin wrapper autour de `bevy_water::WaterPlugin`. Maintient une nappe d'eau
//! au niveau de la mer, visible uniquement en `GameMode::Rpg` (cachée en Menu /
//! Arena pour ne pas noyer les scènes non concernées — trap V1).
//!
//! W1 vertical slice : pas de swim gate côté player (V1 lit `MapGenConfig.sea_level`
//! pour `is_swimming` ; à porter quand forgia-player aura un controller adapté).

use bevy::prelude::*;
use bevy_water::{WaterPlugin, WaterSettings, WaterTiles};
use forgia_core::prelude::*;

pub mod prelude {
    pub use crate::ForgiaWaterPlugin;
}

/// Hauteur monde de la nappe d'eau (matche `forgia-rpg::RPG_SEA_LEVEL`).
const SEA_LEVEL: f32 = 4.0;

pub struct ForgiaWaterPlugin;

impl Plugin for ForgiaWaterPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WaterSettings {
            height: SEA_LEVEL,
            ..default()
        })
        .add_plugins(WaterPlugin)
        .add_systems(OnEnter(GameMode::Rpg), show_water)
        .add_systems(OnExit(GameMode::Rpg), hide_water);
    }
}

fn show_water(mut q: Query<&mut Visibility, With<WaterTiles>>) {
    for mut v in &mut q { *v = Visibility::Visible; }
}

fn hide_water(mut q: Query<&mut Visibility, With<WaterTiles>>) {
    for mut v in &mut q { *v = Visibility::Hidden; }
}
