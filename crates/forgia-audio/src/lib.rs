//! forgia-audio — Audio foundation + biome ambient (fused 2026-05-26).
//!
//! Issu de la fusion de `forgia-audio-core` (22 LOC) + `forgia-audio-biome` (135 LOC)
//! lors du nettoyage workspace 266→<69 crates.
//!
//! - [`ForgiaAudioCorePlugin`] — wrap `bevy_kira_audio::AudioPlugin`.
//! - [`biome::ForgiaAudioBiomePlugin`] — ambient OGG par BiomeType en RPG.

use bevy::prelude::*;
use bevy_kira_audio::AudioPlugin;

pub mod biome;

pub mod prelude {
    pub use crate::biome::{AudioSampleOffset, BiomeAmbientState, ForgiaAudioBiomePlugin};
    pub use crate::ForgiaAudioCorePlugin;
}

/// Foundation plugin — registers `bevy_kira_audio::AudioPlugin` once.
/// Sous-plugins (biome, music, sfx, …) doivent l'ajouter via `is_plugin_added::<AudioPlugin>` guard.
pub struct ForgiaAudioCorePlugin;

impl Plugin for ForgiaAudioCorePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<AudioPlugin>() {
            app.add_plugins(AudioPlugin);
        }
    }
}
