//! forgia-audio-core — wrapper bevy_kira_audio (foundation pour les sous-plugins audio).
//!
//! V2 vertical slice : register `bevy_kira_audio::AudioPlugin` une fois pour toutes,
//! les sous-plugins (forgia-audio-biome, forgia-audio-music-state, etc.) s'ajoutent
//! par-dessus avec leur propre logique.

use bevy::prelude::*;
use bevy_kira_audio::AudioPlugin;

pub mod prelude {
    pub use crate::ForgiaAudioCorePlugin;
}

pub struct ForgiaAudioCorePlugin;

impl Plugin for ForgiaAudioCorePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<AudioPlugin>() {
            app.add_plugins(AudioPlugin);
        }
    }
}
