//! # forgia-ui-hud
//!
//! HUD gameplay FPS arena : player HP bar, floating bot HP + damage popups,
//! wave counter retro arcade. Style cartoon Fortnite/Overwatch — couleurs
//! saturées, outlines chunky, monospace pour effet arcade rétro.
//!
//! Plug : `app.add_plugins(ForgiaUiHudPlugin)`.

use bevy::prelude::*;

mod bot_hp_floaters;
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
            bot_hp_floaters::BotHpFloatersPlugin,
            wave_counter::WaveCounterPlugin,
        ));
    }
}
