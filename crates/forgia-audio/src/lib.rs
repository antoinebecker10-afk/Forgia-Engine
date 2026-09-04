//! forgia-audio — Audio foundation + biome ambient (fused 2026-05-26).
//!
//! Issu de la fusion de `forgia-audio-core` (22 LOC) + `forgia-audio-biome` (135 LOC)
//! lors du nettoyage workspace 266→<69 crates.
//!
//! - [`ForgiaAudioCorePlugin`] — wrap `bevy_kira_audio::AudioPlugin` + volume user.
//! - [`biome::ForgiaAudioBiomePlugin`] — ambient OGG par BiomeType en RPG.
//! - [`SfxChannel`] / [`MusicChannel`] — canaux standard du moteur (story-595) :
//!   les marker types vivent ICI (foundation) pour que le volume user s'applique
//!   à tous les canaux sans dépendre des crates de mode. L'enregistrement
//!   (`add_audio_channel`) reste chez le propriétaire du canal (mode-roguelite).

use bevy::prelude::*;
use bevy_kira_audio::prelude::Decibels;
use bevy_kira_audio::{Audio, AudioChannel, AudioControl, AudioPlugin};

pub mod biome;

pub mod prelude {
    pub use crate::biome::{AudioSampleOffset, BiomeAmbientState, ForgiaAudioBiomePlugin};
    pub use crate::{
        AmbienceChannel, ForgiaAudioCorePlugin, MusicChannel, SfxChannel, UserAudioVolumes,
        UserMasterVolume, VoiceChannel,
    };
}

/// Channel SFX one-shot (impact/kill/hurt/ding/tirs). Type engine-standard —
/// enregistré par forgia-mode-roguelite (`add_audio_channel::<SfxChannel>()`).
#[derive(Resource)]
pub struct SfxChannel;

/// Channel musique (boucles combat/break). Type engine-standard — enregistré
/// par forgia-mode-roguelite (`add_audio_channel::<MusicChannel>()`).
#[derive(Resource)]
pub struct MusicChannel;

/// Boucles environnementales distinctes de la musique.
#[derive(Resource)]
pub struct AmbienceChannel;

/// Voix et barks, séparés pour réglage et ducking futurs.
#[derive(Resource)]
pub struct VoiceChannel;

/// Volume master UTILISATEUR (settings, 0.0..=1.0) — distinct du
/// `master_volume` genome (mix design-time de roguelite_audio.toml).
/// Appliqué au niveau des CANAUX kira → compose multiplicativement avec les
/// `with_volume` par-son existants, sans toucher les systèmes de lecture.
/// Écrit par forgia-ui-lib (slider Options), défaut 1.0 (story-595, M2-B1).
#[derive(Resource, Debug, Clone, Copy)]
pub struct UserMasterVolume(pub f32);

impl Default for UserMasterVolume {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Mix utilisateur par famille. Les gains sont linéaires et composés au niveau
/// de chaque instance, car kira ne compose pas toujours le volume du canal avec
/// `with_volume` (régression validée en jeu le 2026-07-03).
#[derive(Resource, Debug, Clone, Copy)]
pub struct UserAudioVolumes {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
    pub voice: f32,
    pub ambience: f32,
}

impl Default for UserAudioVolumes {
    fn default() -> Self {
        Self {
            master: 1.0,
            music: 0.75,
            sfx: 1.0,
            voice: 1.0,
            ambience: 0.8,
        }
    }
}

impl UserAudioVolumes {
    pub fn music_gain(&self) -> f32 {
        self.master.clamp(0.0, 1.0) * self.music.clamp(0.0, 1.0)
    }

    pub fn sfx_gain(&self) -> f32 {
        self.master.clamp(0.0, 1.0) * self.sfx.clamp(0.0, 1.0)
    }

    pub fn voice_gain(&self) -> f32 {
        self.master.clamp(0.0, 1.0) * self.voice.clamp(0.0, 1.0)
    }

    pub fn ambience_gain(&self) -> f32 {
        self.master.clamp(0.0, 1.0) * self.ambience.clamp(0.0, 1.0)
    }
}

/// Amplitude linéaire (0..1, 1.0 = nominal) → décibels (kira 0.10 attend des
/// `Decibels`). Miroir de la convention mode-roguelite : 0 dB nominal, plancher
/// -80 dB ≈ silence. Pur — testé headless.
pub fn amp_to_db(amp: f32) -> Decibels {
    let db = if amp <= 1.0e-4 {
        -80.0
    } else {
        20.0 * amp.log10()
    };
    Decibels::from(db)
}

/// Applique le volume user à TOUS les canaux connus (main + sfx + music +
/// biome ambient). `Option<Res<…>>` : un canal non enregistré (mode jamais
/// entré) est simplement ignoré. Live : kira répercute le volume de canal sur
/// les instances déjà en cours (musique/ambient inclus).
fn sys_apply_user_master_volume(
    user: Res<UserMasterVolume>,
    mix: Res<UserAudioVolumes>,
    main: Option<Res<Audio>>,
    sfx: Option<Res<AudioChannel<SfxChannel>>>,
    music: Option<Res<AudioChannel<MusicChannel>>>,
    ambience: Option<Res<AudioChannel<AmbienceChannel>>>,
    voice: Option<Res<AudioChannel<VoiceChannel>>>,
    ambient: Option<Res<AudioChannel<biome::BiomeAmbientChannel>>>,
    mut applied_once: Local<bool>,
) {
    if !user.is_changed() && *applied_once {
        return;
    }
    let db = amp_to_db(user.0.clamp(0.0, 1.0));
    if let Some(c) = main {
        c.set_volume(db);
    }
    if let Some(c) = sfx {
        c.set_volume(db);
    }
    if let Some(c) = music {
        c.set_volume(db);
    }
    if let Some(c) = ambience {
        c.set_volume(amp_to_db(mix.ambience_gain()));
    }
    if let Some(c) = voice {
        c.set_volume(amp_to_db(mix.voice_gain()));
    }
    if let Some(c) = ambient {
        c.set_volume(amp_to_db(mix.ambience_gain()));
    }
    *applied_once = true;
}

/// Sensor `forgia2_volume.json` (1Hz) — rend diagnosticable la chaîne
/// slider→`UserMasterVolume`→canaux. Ajouté 2026-07-03 (bug « les SFX ne
/// suivent pas le volume général » : la feature n'avait AUCUN sensor → rupture
/// invisible). Expose la valeur user + quels canaux sont enregistrés + le dB
/// qui serait appliqué. Localise la rupture en 1 run : si `user_master_volume`
/// ne suit pas le slider → bug propagation (pause_menu) ; s'il suit mais un
/// canal est absent → bug d'enregistrement ; s'il suit et canaux présents mais
/// SFX inchangés → sémantique kira.
fn sys_write_volume_sensor(
    time: Res<Time>,
    user: Res<UserMasterVolume>,
    mix: Res<UserAudioVolumes>,
    main: Option<Res<Audio>>,
    sfx: Option<Res<AudioChannel<SfxChannel>>>,
    music: Option<Res<AudioChannel<MusicChannel>>>,
    ambience: Option<Res<AudioChannel<AmbienceChannel>>>,
    voice: Option<Res<AudioChannel<VoiceChannel>>>,
    ambient: Option<Res<AudioChannel<biome::BiomeAmbientChannel>>>,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    let applied_db = amp_to_db(user.0.clamp(0.0, 1.0)).0;
    let json = format!(
        r#"{{"id":"volume","user_master_volume":{:.3},"applied_db":{:.2},"music":{:.3},"sfx":{:.3},"voice":{:.3},"ambience":{:.3},"main_present":{},"sfx_present":{},"music_present":{},"ambience_present":{},"voice_present":{},"biome_ambient_present":{}}}"#,
        user.0,
        applied_db,
        mix.music,
        mix.sfx,
        mix.voice,
        mix.ambience,
        main.is_some(),
        sfx.is_some(),
        music.is_some(),
        ambience.is_some(),
        voice.is_some(),
        ambient.is_some(),
    );
    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_volume.json", json) {
        warn!("[forgia-audio] volume sensor write failed: {e}");
    }
}

/// Foundation plugin — registers `bevy_kira_audio::AudioPlugin` once + couche
/// volume user (story-595).
/// Sous-plugins (biome, music, sfx, …) doivent l'ajouter via `is_plugin_added::<AudioPlugin>` guard.
pub struct ForgiaAudioCorePlugin;

impl Plugin for ForgiaAudioCorePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<AudioPlugin>() {
            app.add_plugins(AudioPlugin);
        }
        app.init_resource::<UserMasterVolume>()
            .init_resource::<UserAudioVolumes>()
            .add_systems(Update, sys_apply_user_master_volume)
            .add_systems(Update, sys_write_volume_sensor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_master_volume_default_full() {
        assert_eq!(UserMasterVolume::default().0, 1.0);
    }

    #[test]
    fn amp_to_db_nominal_is_zero_db() {
        let db = amp_to_db(1.0).0;
        assert!(db.abs() < 0.001, "1.0 amp = 0 dB nominal, got {db}");
    }

    #[test]
    fn amp_to_db_half_is_minus_six_db() {
        let db = amp_to_db(0.5).0;
        assert!((db + 6.02).abs() < 0.1, "0.5 amp ≈ -6 dB, got {db}");
    }

    #[test]
    fn amp_to_db_zero_is_silence_floor() {
        let db = amp_to_db(0.0).0;
        assert_eq!(db, -80.0, "0 amp = plancher -80 dB");
    }

    #[test]
    fn category_gains_compose_with_master() {
        let mix = UserAudioVolumes {
            master: 0.5,
            music: 0.4,
            ..default()
        };
        assert!((mix.music_gain() - 0.2).abs() < 0.001);
        assert!((mix.sfx_gain() - 0.5).abs() < 0.001);
    }
}
