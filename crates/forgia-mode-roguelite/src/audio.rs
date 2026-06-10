//! Story-559 slice A (2026-06-04) — Audio Roguelite « le jeu n'est plus muet ».
//!
//! Audit V2 Trou #1 (jeu 100% silencieux) : on câble les retours sonores les plus
//! rentables, **sans toucher au pipeline de tir** (orthogonal multi-terminal) :
//!
//! - **SFX combat** : impact ennemi / kill / hurt joueur, lus depuis `CombatHitEvent`
//!   (déjà émis par forgia-fps + forgia-ai-arena-bot, consommé read-only ici).
//! - **Ding pickup** : « ding » Or (Souls in-run) + « ding » Âmes (MetaSouls méta),
//!   détecté par diff de Resource (front-detection Local, zéro édition cross-crate).
//! - **Musique** : boucle combat + bascule calme pendant le break 15s (tension/repos).
//!
//! Data-driven : `assets/genomes/roguelite/roguelite_audio.toml` (hot-reload mtime
//! 1Hz, Shift+F12). Sensor `forgia2_roguelite_audio.json` (règle observability-required).
//!
//! ⚠️ Le **« bang » du tir lui-même** (SFX à chaque shot, hit/miss) + muzzle flash =
//! slice B : il exige un `WeaponFiredEvent` (forgia-combat/forgia-fps) qu'on pairera
//! avec le muzzle flash (même hook). Ici, en arène cible-riche, l'impact joue à
//! chaque tir touché → feedback constant immédiat.

use bevy::prelude::*;
use bevy_kira_audio::prelude::Decibels;
use bevy_kira_audio::{AudioApp, AudioChannel, AudioControl, AudioSource};
use forgia_ai_arena_bot::ArenaBot;
use forgia_audio::prelude::ForgiaAudioCorePlugin;
use forgia_combat::combat_juice::{CombatHitEvent, WeaponFiredEvent};
use forgia_combat::weapons::WeaponType;
use forgia_core::prelude::*;
use forgia_player::Player;
use forgia_rpg_data::loot_tables::Souls;
use serde::Deserialize;
use std::fs;
use std::time::SystemTime;

use crate::run::MetaSouls;
use crate::waves::RogueliteWave;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_audio.toml";
const SENSOR_PATH: &str = "forgia2_roguelite_audio.json";
const POLL_PERIOD_SEC: f32 = 1.0;
const VOLUME_MAX: f32 = 2.0;

// ─── Channels kira (découplés du biome ambient RPG) ───────────────────────────

// Story-595 (M2-B1) : les marker types des canaux vivent désormais dans
// forgia-audio (foundation) pour que le volume master USER (settings) puisse
// s'appliquer à tous les canaux sans dépendance inverse. Re-export : les
// usages locaux (`AudioChannel<SfxChannel>`) restent inchangés.
pub use forgia_audio::{MusicChannel, SfxChannel};

// ─── Genome TOML ──────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
struct SoundEntryToml {
    path: String,
    #[serde(default)]
    volume: Option<f32>,
}

#[derive(Deserialize, Default)]
struct AudioGenomeToml {
    #[serde(default)]
    master_volume: Option<f32>,
    #[serde(default)]
    music_volume: Option<f32>,
    #[serde(default)]
    impact: Option<SoundEntryToml>,
    #[serde(default)]
    kill: Option<SoundEntryToml>,
    #[serde(default)]
    hurt: Option<SoundEntryToml>,
    #[serde(default)]
    ding_gold: Option<SoundEntryToml>,
    #[serde(default)]
    ding_souls: Option<SoundEntryToml>,
    #[serde(default)]
    music_combat: Option<SoundEntryToml>,
    #[serde(default)]
    music_break: Option<SoundEntryToml>,
    #[serde(default)]
    fire_pepin: Option<SoundEntryToml>,
    #[serde(default)]
    fire_bourrasque: Option<SoundEntryToml>,
    #[serde(default)]
    fire_lenoir: Option<SoundEntryToml>,
    #[serde(default)]
    fire_boucherie: Option<SoundEntryToml>,
}

// ─── Config résolue (Resource) ────────────────────────────────────────────────

#[derive(Clone)]
pub struct SoundDef {
    pub path: String,
    pub volume: f32,
}

impl SoundDef {
    fn new(path: &str, volume: f32) -> Self {
        Self {
            path: path.to_string(),
            volume,
        }
    }
}

#[derive(Resource, Clone)]
pub struct RogueliteAudioConfig {
    pub master_volume: f32,
    pub music_volume: f32,
    pub impact: SoundDef,
    pub kill: SoundDef,
    pub hurt: SoundDef,
    pub ding_gold: SoundDef,
    pub ding_souls: SoundDef,
    pub music_combat: SoundDef,
    pub music_break: SoundDef,
    // Sons de TIR par arme (slice B) — mappés depuis `WeaponType` (slot Digit1-4).
    pub fire_pepin: SoundDef,
    pub fire_bourrasque: SoundDef,
    pub fire_lenoir: SoundDef,
    pub fire_boucherie: SoundDef,
}

impl Default for RogueliteAudioConfig {
    fn default() -> Self {
        // Placeholders V1-jingle (assets/audio-v1/) : audibles immédiatement.
        // À remplacer par des SFX CC0 punchy dédiés — data-driven, swap = hot-reload.
        Self {
            master_volume: 0.9,
            music_volume: 0.45,
            impact: SoundDef::new("audio-v1/music/jingles/jingles_HIT00.ogg", 0.6),
            kill: SoundDef::new("audio-v1/music/jingles/jingles_HIT12.ogg", 0.85),
            hurt: SoundDef::new("audio-v1/music/jingles/jingles_NES10.ogg", 0.5),
            ding_gold: SoundDef::new("audio-v1/music/jingles/jingles_PIZZI00.ogg", 0.7),
            ding_souls: SoundDef::new("audio-v1/music/jingles/jingles_PIZZI08.ogg", 0.7),
            // Musique continue du jeu = le morceau « entre les waves » (user 2026-06-05).
            // combat == break → handle-compare évite tout restart aux transitions.
            music_combat: SoundDef::new("audio-v1/music/emotional_01.ogg", 0.8),
            music_break: SoundDef::new("audio-v1/music/emotional_01.ogg", 0.8),
            // Sons de tir CC-BY 3.0 (Vincent Sevedge) — cf
            // assets/audio/roguelite/weapons/CREDITS.md. CZ-52/SKS/Mosin/Shotgun.
            fire_pepin: SoundDef::new("audio/roguelite/weapons/pepin.ogg", 0.75),
            fire_bourrasque: SoundDef::new("audio/roguelite/weapons/bourrasque.ogg", 0.6),
            fire_lenoir: SoundDef::new("audio/roguelite/weapons/lenoir.ogg", 0.9),
            fire_boucherie: SoundDef::new("audio/roguelite/weapons/boucherie.ogg", 0.85),
        }
    }
}

/// Handles préchargés (0 `load()` dans le hot path : tout chargé au Startup + reload).
#[derive(Resource, Default)]
pub struct AudioHandles {
    impact: Handle<AudioSource>,
    kill: Handle<AudioSource>,
    hurt: Handle<AudioSource>,
    ding_gold: Handle<AudioSource>,
    ding_souls: Handle<AudioSource>,
    music_combat: Handle<AudioSource>,
    music_break: Handle<AudioSource>,
    fire_pepin: Handle<AudioSource>,
    fire_bourrasque: Handle<AudioSource>,
    fire_lenoir: Handle<AudioSource>,
    fire_boucherie: Handle<AudioSource>,
}

/// Compteurs sensor (forgia2_roguelite_audio.json).
#[derive(Resource, Default)]
pub struct RogueliteAudioStats {
    pub fires: u32,
    pub impacts: u32,
    pub kills: u32,
    pub hurts: u32,
    pub dings_gold: u32,
    pub dings_souls: u32,
    pub sfx_played: u32,
    pub music_playing: bool,
}

/// État musique : None=stoppée, Some(false)=combat, Some(true)=break.
#[derive(Resource, Default)]
struct MusicTrack {
    current: Option<Handle<AudioSource>>,
}

/// Suivi mtime du genome pour hot-reload.
#[derive(Resource)]
struct AudioGenomeWatch {
    last_mtime: Option<SystemTime>,
    accum: f32,
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct RogueliteAudioPlugin;

impl Plugin for RogueliteAudioPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<ForgiaAudioCorePlugin>() {
            app.add_plugins(ForgiaAudioCorePlugin);
        }
        app.add_audio_channel::<SfxChannel>()
            .add_audio_channel::<MusicChannel>()
            .init_resource::<RogueliteAudioConfig>()
            .init_resource::<AudioHandles>()
            .init_resource::<RogueliteAudioStats>()
            .init_resource::<MusicTrack>()
            .insert_resource(AudioGenomeWatch {
                last_mtime: None,
                accum: 0.0,
            })
            .add_systems(
                Startup,
                (sys_init_audio_genome, sys_load_audio_handles).chain(),
            )
            .add_systems(OnExit(GameMode::Roguelite), sys_stop_music)
            .add_systems(
                Update,
                (
                    sys_hot_reload_audio_genome,
                    sys_fire_sfx,
                    sys_sfx_on_combat_hit,
                    sys_ding_on_currency,
                    sys_music_update,
                )
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(Update, sys_write_audio_sensor.in_set(GameSet::Sensors));
    }
}

// ─── Genome parse / load ──────────────────────────────────────────────────────

fn parse_audio_genome(path: &str) -> Option<RogueliteAudioConfig> {
    let raw = fs::read_to_string(path).ok()?;
    let toml: AudioGenomeToml = toml::from_str(&raw).ok()?;
    let mut cfg = RogueliteAudioConfig::default();
    if let Some(v) = toml.master_volume {
        cfg.master_volume = v.clamp(0.0, VOLUME_MAX);
    }
    if let Some(v) = toml.music_volume {
        cfg.music_volume = v.clamp(0.0, VOLUME_MAX);
    }
    apply_entry(&mut cfg.impact, toml.impact);
    apply_entry(&mut cfg.kill, toml.kill);
    apply_entry(&mut cfg.hurt, toml.hurt);
    apply_entry(&mut cfg.ding_gold, toml.ding_gold);
    apply_entry(&mut cfg.ding_souls, toml.ding_souls);
    apply_entry(&mut cfg.music_combat, toml.music_combat);
    apply_entry(&mut cfg.music_break, toml.music_break);
    apply_entry(&mut cfg.fire_pepin, toml.fire_pepin);
    apply_entry(&mut cfg.fire_bourrasque, toml.fire_bourrasque);
    apply_entry(&mut cfg.fire_lenoir, toml.fire_lenoir);
    apply_entry(&mut cfg.fire_boucherie, toml.fire_boucherie);
    Some(cfg)
}

fn apply_entry(dst: &mut SoundDef, src: Option<SoundEntryToml>) {
    if let Some(s) = src {
        if !s.path.trim().is_empty() {
            dst.path = s.path;
        }
        if let Some(v) = s.volume {
            dst.volume = v.clamp(0.0, VOLUME_MAX);
        }
    }
}

fn load_handles(asset_server: &AssetServer, cfg: &RogueliteAudioConfig) -> AudioHandles {
    // 1 seul call-site `asset_server.load` (boucle) → drift Lock L1 minimal (+1, pas +7).
    let defs = [
        &cfg.impact,
        &cfg.kill,
        &cfg.hurt,
        &cfg.ding_gold,
        &cfg.ding_souls,
        &cfg.music_combat,
        &cfg.music_break,
        &cfg.fire_pepin,
        &cfg.fire_bourrasque,
        &cfg.fire_lenoir,
        &cfg.fire_boucherie,
    ];
    let h: Vec<Handle<AudioSource>> = defs
        .iter()
        .map(|d| asset_server.load(d.path.clone()))
        .collect();
    AudioHandles {
        impact: h[0].clone(),
        kill: h[1].clone(),
        hurt: h[2].clone(),
        ding_gold: h[3].clone(),
        ding_souls: h[4].clone(),
        music_combat: h[5].clone(),
        music_break: h[6].clone(),
        fire_pepin: h[7].clone(),
        fire_bourrasque: h[8].clone(),
        fire_lenoir: h[9].clone(),
        fire_boucherie: h[10].clone(),
    }
}

/// Amplitude linéaire (0..2, 1.0 = nominal) → décibels (API kira 0.10 :
/// `with_volume` attend des `Decibels`). 0 dB = nominal, -6 dB ≈ ×0.5.
fn amp_to_db(amp: f32) -> Decibels {
    let db = if amp <= 1.0e-4 {
        -80.0
    } else {
        20.0 * amp.log10()
    };
    Decibels::from(db)
}

fn sys_init_audio_genome(mut commands: Commands) {
    let cfg = parse_audio_genome(GENOME_PATH).unwrap_or_default();
    commands.insert_resource(cfg);
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(AudioGenomeWatch {
        last_mtime: mtime,
        accum: 0.0,
    });
}

fn sys_load_audio_handles(
    asset_server: Res<AssetServer>,
    cfg: Res<RogueliteAudioConfig>,
    mut handles: ResMut<AudioHandles>,
) {
    *handles = load_handles(&asset_server, &cfg);
}

fn sys_hot_reload_audio_genome(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut watch: ResMut<AudioGenomeWatch>,
    mut cfg: ResMut<RogueliteAudioConfig>,
    mut handles: ResMut<AudioHandles>,
    mut music_track: ResMut<MusicTrack>,
) {
    watch.accum += time.delta_secs();
    if watch.accum < POLL_PERIOD_SEC {
        return;
    }
    watch.accum = 0.0;
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    if mtime == watch.last_mtime {
        return;
    }
    watch.last_mtime = mtime;
    if let Some(new_cfg) = parse_audio_genome(GENOME_PATH) {
        *cfg = new_cfg;
        *handles = load_handles(&asset_server, &cfg);
        // Force le redémarrage de la musique avec le nouveau handle/volume.
        music_track.current = None;
        info!("[roguelite-audio] genome HOT-RELOADED ({GENOME_PATH})");
    }
}

// ─── SFX combat (lecture CombatHitEvent, read-only) ───────────────────────────

fn sys_sfx_on_combat_hit(
    sfx: Res<AudioChannel<SfxChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    mut stats: ResMut<RogueliteAudioStats>,
    mut events: MessageReader<CombatHitEvent>,
    q_enemy: Query<(), With<ArenaBot>>,
    q_player: Query<(), With<Player>>,
) {
    let master = cfg.master_volume;
    for ev in events.read() {
        if q_enemy.get(ev.target).is_ok() {
            // L'ennemi encaisse : impact, ou kill si HP=0.
            let def = if ev.is_kill { &cfg.kill } else { &cfg.impact };
            let handle = if ev.is_kill {
                handles.kill.clone()
            } else {
                handles.impact.clone()
            };
            sfx.play(handle).with_volume(amp_to_db(def.volume * master));
            if ev.is_kill {
                stats.kills += 1;
            } else {
                stats.impacts += 1;
            }
            stats.sfx_played += 1;
        } else if q_player.get(ev.target).is_ok() {
            // Le joueur encaisse : son de douleur.
            sfx.play(handles.hurt.clone())
                .with_volume(amp_to_db(cfg.hurt.volume * master));
            stats.hurts += 1;
            stats.sfx_played += 1;
        }
    }
}

// ─── SFX de tir par arme (lecture WeaponFiredEvent) ───────────────────────────

/// Mapping `WeaponType` → son de tir. Slot Digit1-4 = pistolet/mitraillette/
/// sniper/pompe (cf assets/audio/roguelite/weapons/CREDITS.md). Les noms de
/// variantes `WeaponType` (Shotgun/RocketLauncher) sont des labels hérités de
/// l'Arena ; ce qui compte est le slot d'arme Forgia (Lenoir=sniper, Boucherie=pompe).
fn weapon_fire_def(cfg: &RogueliteAudioConfig, w: WeaponType) -> &SoundDef {
    match w {
        WeaponType::ModernAR => &cfg.fire_pepin,
        WeaponType::AssaultRifle | WeaponType::AK47 => &cfg.fire_bourrasque,
        WeaponType::Shotgun => &cfg.fire_lenoir,
        WeaponType::RocketLauncher => &cfg.fire_boucherie,
        _ => &cfg.fire_pepin,
    }
}

fn weapon_fire_handle(h: &AudioHandles, w: WeaponType) -> Handle<AudioSource> {
    match w {
        WeaponType::ModernAR => h.fire_pepin.clone(),
        WeaponType::AssaultRifle | WeaponType::AK47 => h.fire_bourrasque.clone(),
        WeaponType::Shotgun => h.fire_lenoir.clone(),
        WeaponType::RocketLauncher => h.fire_boucherie.clone(),
        _ => h.fire_pepin.clone(),
    }
}

/// Joue le son de tir propre à l'arme à chaque `WeaponFiredEvent` (hit OU miss).
fn sys_fire_sfx(
    sfx: Res<AudioChannel<SfxChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    mut stats: ResMut<RogueliteAudioStats>,
    mut events: MessageReader<WeaponFiredEvent>,
) {
    let master = cfg.master_volume;
    for ev in events.read() {
        let def = weapon_fire_def(&cfg, ev.weapon);
        let handle = weapon_fire_handle(&handles, ev.weapon);
        sfx.play(handle).with_volume(amp_to_db(def.volume * master));
        stats.fires += 1;
        stats.sfx_played += 1;
    }
}

// ─── Ding pickup (diff Resource, front-detection) ─────────────────────────────

fn sys_ding_on_currency(
    sfx: Res<AudioChannel<SfxChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    mut stats: ResMut<RogueliteAudioStats>,
    gold: Option<Res<Souls>>,
    meta: Option<Res<MetaSouls>>,
    mut last_gold: Local<Option<u32>>,
    mut last_souls: Local<Option<u32>>,
) {
    let master = cfg.master_volume;
    if let Some(g) = gold {
        let cur = g.total_collected;
        if matches!(*last_gold, Some(prev) if cur > prev) {
            sfx.play(handles.ding_gold.clone())
                .with_volume(amp_to_db(cfg.ding_gold.volume * master));
            stats.dings_gold += 1;
            stats.sfx_played += 1;
        }
        *last_gold = Some(cur);
    }
    if let Some(m) = meta {
        let cur = m.earned_run;
        if matches!(*last_souls, Some(prev) if cur > prev) {
            sfx.play(handles.ding_souls.clone())
                .with_volume(amp_to_db(cfg.ding_souls.volume * master));
            stats.dings_souls += 1;
            stats.sfx_played += 1;
        }
        *last_souls = Some(cur);
    }
}

// ─── Musique combat/break ─────────────────────────────────────────────────────

fn sys_music_update(
    music: Res<AudioChannel<MusicChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    wave: Option<Res<RogueliteWave>>,
    mut track: ResMut<MusicTrack>,
    mut stats: ResMut<RogueliteAudioStats>,
) {
    let want_break = wave.map(|w| w.in_break).unwrap_or(false);
    let (handle, def) = if want_break {
        (&handles.music_break, &cfg.music_break)
    } else {
        (&handles.music_combat, &cfg.music_combat)
    };
    // Ne (re)démarre QUE si le morceau voulu diffère de l'actuel → pas de restart
    // quand combat et break pointent sur le MÊME fichier (musique continue ;
    // demande user : « le son entre les waves = la musique du jeu »).
    if track.current.as_ref() == Some(handle) {
        return;
    }
    music.stop();
    music
        .play(handle.clone())
        .looped()
        .with_volume(amp_to_db(def.volume * cfg.music_volume));
    track.current = Some(handle.clone());
    stats.music_playing = true;
}

fn sys_stop_music(
    music: Res<AudioChannel<MusicChannel>>,
    mut track: ResMut<MusicTrack>,
    mut stats: ResMut<RogueliteAudioStats>,
) {
    music.stop();
    track.current = None;
    stats.music_playing = false;
}

// ─── Sensor ───────────────────────────────────────────────────────────────────

fn sys_write_audio_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    stats: Res<RogueliteAudioStats>,
    cfg: Res<RogueliteAudioConfig>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (severity, next_step) = if !stats.music_playing && stats.sfx_played == 0 {
        (
            "info",
            "Entre en Roguelite + tire/encaisse/ramasse pour générer du son (slice A : impact/kill/ding/musique).",
        )
    } else {
        ("ok", "-")
    };
    let json = format!(
        r#"{{"id":"roguelite_audio","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"fires":{},"impacts":{},"kills":{},"hurts":{},"dings_gold":{},"dings_souls":{},"sfx_played":{},"music_playing":{},"master_volume":{:.2},"music_volume":{:.2}}}"#,
        time.elapsed_secs(),
        stats.fires,
        stats.impacts,
        stats.kills,
        stats.hurts,
        stats.dings_gold,
        stats.dings_souls,
        stats.sfx_played,
        stats.music_playing,
        cfg.master_volume,
        cfg.music_volume,
    );
    let _ = fs::write(SENSOR_PATH, json);
}
