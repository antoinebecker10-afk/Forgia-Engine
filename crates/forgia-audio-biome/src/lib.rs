//! # forgia-audio-biome
//!
//! Ambient audio par biome — joue 1 loop OGG selon le biome courant du joueur.
//!
//! V2 vertical slice : assets V1 high-quality dispo via junction `audio-v1/`.
//! Mapping `BiomeType → asset_path`. Système Update détecte changement de biome
//! (via `BiomeMap.biome_at(player_pos + AudioSampleOffset)`), stop l'ancien et
//! démarre le nouveau. Pas de crossfade (V2 future).

use bevy::prelude::*;
use bevy_kira_audio::{AudioApp, AudioChannel, AudioControl, AudioInstance, AudioSource};
use forgia_audio_core::ForgiaAudioCorePlugin;
use forgia_core::prelude::*;
use forgia_terrain::{BiomeMap, BiomeType};

pub mod prelude {
    pub use crate::{AudioSampleOffset, ForgiaAudioBiomePlugin};
}

/// Channel dédié pour ambient biome — découplé du music/sfx.
#[derive(Resource)]
pub struct BiomeAmbientChannel;

/// Wrapper Resource pour aligner le sample biome avec le décalage RPG.
/// Posée par forgia-rpg (sample_offset = map_size/2). Fallback (0,0) si absent.
#[derive(Resource, Clone, Copy, Default)]
pub struct AudioSampleOffset {
    pub x: f32,
    pub z: f32,
}

#[derive(Resource, Default)]
pub struct BiomeAmbientState {
    current: Option<BiomeType>,
    instance: Option<Handle<AudioInstance>>,
}

impl BiomeAmbientState {
    /// Story-469 — read-only accessor pour forgia2_audio.json sensor.
    pub fn current_biome(&self) -> Option<BiomeType> {
        self.current
    }
}

pub struct ForgiaAudioBiomePlugin;

impl Plugin for ForgiaAudioBiomePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<ForgiaAudioCorePlugin>() {
            app.add_plugins(ForgiaAudioCorePlugin);
        }
        app.add_audio_channel::<BiomeAmbientChannel>()
            .init_resource::<BiomeAmbientState>()
            .add_systems(OnExit(GameMode::Rpg), stop_biome_ambient)
            .add_systems(
                Update,
                update_biome_ambient
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Rpg)),
            );
    }
}

/// Chemin OGG ambient par biome. Pointe sur junction `audio-v1/ambiance/...`.
fn biome_ambient_asset(biome: BiomeType) -> &'static str {
    match biome {
        BiomeType::Forest => "audio-v1/ambiance/forest/Forest Day.ogg",
        BiomeType::Jungle => "audio-v1/ambiance/forest/forest_ambience.mp3",
        BiomeType::Swamp => "audio-v1/ambiance/night/Forest Night.ogg",
        BiomeType::Plains => "audio-v1/ambiance/forest/birds.ogg",
        BiomeType::Savanna => "audio-v1/ambiance/wind/wind_whoosh_loop.ogg",
        BiomeType::Desert => "audio-v1/ambiance/wind/wind_whoosh_loop.ogg",
        BiomeType::Canyon => "audio-v1/ambiance/wind/wind_whoosh_loop.ogg",
        BiomeType::Mountain => "audio-v1/ambiance/wind/wind_whoosh_loop.ogg",
        BiomeType::Tundra => "audio-v1/ambiance/wind/wind_whoosh_loop.ogg",
        BiomeType::Volcanic => "audio-v1/ambiance/cave/Cave.ogg",
    }
}

fn update_biome_ambient(
    audio: Res<AudioChannel<BiomeAmbientChannel>>,
    asset_server: Res<AssetServer>,
    biome_map: Option<Res<BiomeMap>>,
    offset: Option<Res<AudioSampleOffset>>,
    mut state: ResMut<BiomeAmbientState>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    player_q: Query<&Transform>,
    mut frame_counter: Local<u32>,
) {
    *frame_counter = frame_counter.wrapping_add(1);
    // 1 check / 30 frames (~0.5s) — biome change pas critique per-frame.
    if !frame_counter.is_multiple_of(30) {
        return;
    }

    let Some(biome_map) = biome_map else { return };
    let Some(player_tf) = player_q.iter().next() else {
        return;
    };
    let off = offset.map(|o| (o.x, o.z)).unwrap_or((0.0, 0.0));

    let p = player_tf.translation;
    let biome = biome_map.biome_at(p.x + off.0, p.z + off.1);

    if state.current == Some(biome) {
        return;
    }

    // Stop l'instance précédente (pas de fade — V2 future).
    if let Some(handle) = state.instance.take() {
        if let Some(inst) = audio_instances.get_mut(&handle) {
            inst.stop(bevy_kira_audio::AudioTween::default());
        }
    }

    let path = biome_ambient_asset(biome);
    let src: Handle<AudioSource> = asset_server.load(path);
    let handle = audio.play(src).looped().handle();

    state.current = Some(biome);
    state.instance = Some(handle);
    info!(
        "[forgia-audio-biome] Switched ambient → {:?} ({})",
        biome, path
    );
}

fn stop_biome_ambient(
    audio: Res<AudioChannel<BiomeAmbientChannel>>,
    mut state: ResMut<BiomeAmbientState>,
) {
    audio.stop();
    state.current = None;
    state.instance = None;
}
