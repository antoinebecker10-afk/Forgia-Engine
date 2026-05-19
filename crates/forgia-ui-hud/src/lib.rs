//! # forgia-ui-hud
//!
//! HUD gameplay FPS arena : player HP bar, floating bot HP + damage popups,
//! wave counter retro arcade. Style cartoon Fortnite/Overwatch — couleurs
//! saturées, outlines chunky, monospace pour effet arcade rétro.
//!
//! Plug : `app.add_plugins(ForgiaUiHudPlugin)`.

use bevy::prelude::*;

// Story-457 (2026-05-19) : `bot_hp_floaters` retiré au profit du crate dédié
// `forgia-enemy-nameplate` (3D billboard world-space, custom Material possible).
// L'egui screen-space ne pouvait pas occluder derrière les murs ni gérer
// les distances en world units.
mod player_hp;
mod style;
mod wave_counter;

pub mod prelude {
    pub use crate::ForgiaUiHudPlugin;
}

pub struct ForgiaUiHudPlugin;

impl Plugin for ForgiaUiHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            player_hp::PlayerHpPlugin,
            wave_counter::WaveCounterPlugin,
        ));
    }
}
