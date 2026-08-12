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
use bevy_egui::EguiContexts;
use bevy_kira_audio::prelude::Decibels;
use bevy_kira_audio::{
    AudioApp, AudioChannel, AudioControl, AudioInstance, AudioSource, AudioTween,
};
use bevy_rapier3d::prelude::KinematicCharacterControllerOutput;
use forgia_ai_arena_bot::ArenaBot;
use forgia_audio::prelude::{ForgiaAudioCorePlugin, UserAudioVolumes};
use forgia_combat::ammo::{AmmoChangeKind, AmmoChanged};
use forgia_combat::combat_juice::{CombatHitEvent, WeaponFiredEvent};
use forgia_combat::weapons::WeaponType;
use forgia_core::prelude::*;
use forgia_player::prelude::DashUsedEvent;
use forgia_player::{Player, PlayerLocomotion};
use forgia_rpg_data::boons::{BoonAppliedEvent, CoffrePickedEvent};
use forgia_rpg_data::loot_tables::Souls;
use forgia_ui_lib::ui_sfx::{drain_ui_sfx, UiSfxKind};
use serde::Deserialize;
use std::fs;
use std::time::SystemTime;

use crate::run::{EndRunEvent, MetaSouls, RunResult};
use crate::waves::{BossEnrageTriggeredEvent, RogueliteWave};

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_audio.toml";
const SENSOR_PATH: &str = "forgia2_roguelite_audio.json";
const POLL_PERIOD_SEC: f32 = 1.0;
/// Plafond des `volume` du genome.
///
/// Relevé 2.0 → 4.0 le 2026-08-05. Raison mesurée, pas de confort : les tirs du
/// pack ont une crête de **-6 à -10 dB** dans le fichier ; à 2.0 le plus discret
/// (Bourrasque, -10,0 dB) plafonnait à -4,9 dB une fois le master appliqué,
/// soit trop bas face à une bande-son à -17,8 dB de moyenne. Le plafond bornait
/// donc le mix AVANT que le niveau visé soit atteignable.
///
/// 4.0 (+12 dB) laisse la marge nécessaire sans rendre l'échelle absurde ; la
/// non-saturation reste garantie par les valeurs du genome, calculées pour que
/// `crête_fichier + volume + master` reste sous -2 dB.
const VOLUME_MAX: f32 = 4.0;

// ─── Channels kira (découplés du biome ambient RPG) ───────────────────────────

// Story-595 (M2-B1) : les marker types des canaux vivent désormais dans
// forgia-audio (foundation) pour que le volume master USER (settings) puisse
// s'appliquer à tous les canaux sans dépendance inverse. Re-export : les
// usages locaux (`AudioChannel<SfxChannel>`) restent inchangés.
pub use forgia_audio::{AmbienceChannel, MusicChannel, SfxChannel, VoiceChannel};

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
    ui_sfx_volume: Option<f32>,
    #[serde(default)]
    impact: Option<SoundEntryToml>,
    #[serde(default)]
    weakspot: Option<SoundEntryToml>,
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
    // Bande-son Suno (2026-08-05) : thème du hub (menu/lobby, hors run) +
    // une piste par CHAPITRE du Livre (index 0 = chapitre 1). Si un chapitre
    // n'a pas de piste, fallback music_combat.
    #[serde(default)]
    music_hub: Option<SoundEntryToml>,
    #[serde(default)]
    music_chapters: Option<Vec<SoundEntryToml>>,
    #[serde(default)]
    fire_pepin: Option<SoundEntryToml>,
    #[serde(default)]
    fire_bourrasque: Option<SoundEntryToml>,
    #[serde(default)]
    fire_lenoir: Option<SoundEntryToml>,
    #[serde(default)]
    fire_boucherie: Option<SoundEntryToml>,
    #[serde(default)]
    dash: Option<SoundEntryToml>,
    #[serde(default)]
    reload_start: Option<SoundEntryToml>,
    #[serde(default)]
    reload_complete: Option<SoundEntryToml>,
    #[serde(default)]
    weapon_switch: Option<SoundEntryToml>,
    #[serde(default)]
    boon: Option<SoundEntryToml>,
    #[serde(default)]
    chest: Option<SoundEntryToml>,
    #[serde(default)]
    wave_clear: Option<SoundEntryToml>,
    #[serde(default)]
    wave_start: Option<SoundEntryToml>,
    #[serde(default)]
    boss_enrage: Option<SoundEntryToml>,
    #[serde(default)]
    victory: Option<SoundEntryToml>,
    #[serde(default)]
    defeat: Option<SoundEntryToml>,
    #[serde(default)]
    ambience: Option<SoundEntryToml>,
    #[serde(default)]
    footstep_1: Option<SoundEntryToml>,
    #[serde(default)]
    footstep_2: Option<SoundEntryToml>,
    #[serde(default)]
    footstep_3: Option<SoundEntryToml>,
    #[serde(default)]
    footstep_4: Option<SoundEntryToml>,
    // Famille UI (story-678) — menu/hub, jouée aussi hors GameMode::Roguelite.
    #[serde(default)]
    ui_hover: Option<SoundEntryToml>,
    #[serde(default)]
    ui_click: Option<SoundEntryToml>,
    #[serde(default)]
    ui_tab: Option<SoundEntryToml>,
    #[serde(default)]
    ui_buy: Option<SoundEntryToml>,
    #[serde(default)]
    ui_unlock: Option<SoundEntryToml>,
    #[serde(default)]
    ui_denied: Option<SoundEntryToml>,
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
    /// Story-651 — « tink » weakspot (tête) : LE son pavlovien Gunfire-like,
    /// joué à CHAQUE headshot (en plus d'impact/kill), pitch fixe (signature).
    pub weakspot: SoundDef,
    pub kill: SoundDef,
    pub hurt: SoundDef,
    pub ding_gold: SoundDef,
    pub ding_souls: SoundDef,
    pub music_combat: SoundDef,
    pub music_break: SoundDef,
    /// Thème du hub (menu/lobby) — joué quand aucune vague n'existe (hors run).
    pub music_hub: SoundDef,
    /// Une piste par chapitre du Livre (index 0 = chapitre 1). Peut être plus
    /// courte que `CHAPTERS_PER_BOOK` : chapitre sans piste → music_combat.
    pub music_chapters: Vec<SoundDef>,
    // Sons de TIR par arme (slice B) — mappés depuis `WeaponType` (slot Digit1-4).
    pub fire_pepin: SoundDef,
    pub fire_bourrasque: SoundDef,
    pub fire_lenoir: SoundDef,
    pub fire_boucherie: SoundDef,
    pub dash: SoundDef,
    pub reload_start: SoundDef,
    pub reload_complete: SoundDef,
    pub weapon_switch: SoundDef,
    pub boon: SoundDef,
    pub chest: SoundDef,
    pub wave_clear: SoundDef,
    pub wave_start: SoundDef,
    pub boss_enrage: SoundDef,
    pub victory: SoundDef,
    pub defeat: SoundDef,
    pub ambience: SoundDef,
    pub footsteps: [SoundDef; 4],
    /// Trim global de la famille UI (multiplie chaque `ui_*.volume`).
    pub ui_sfx_volume: f32,
    pub ui_hover: SoundDef,
    pub ui_click: SoundDef,
    pub ui_tab: SoundDef,
    pub ui_buy: SoundDef,
    pub ui_unlock: SoundDef,
    pub ui_denied: SoundDef,
}

impl Default for RogueliteAudioConfig {
    fn default() -> Self {
        // Pack original « forge fantastique cartoon », généré sans sample tiers.
        // Le genome reste prioritaire ; ces valeurs protègent le fallback de parsing.
        Self {
            master_volume: 0.9,
            music_volume: 0.45,
            impact: SoundDef::new("audio/forgia_original/combat/impact_forge.ogg", 0.45),
            weakspot: SoundDef::new("audio/forgia_original/combat/weakspot_chime.ogg", 0.8),
            kill: SoundDef::new("audio/forgia_original/combat/kill_stamp.ogg", 0.7),
            hurt: SoundDef::new("audio/forgia_original/combat/player_hurt.ogg", 0.6),
            ding_gold: SoundDef::new("audio/forgia_original/pickups/gold_spark.ogg", 0.6),
            ding_souls: SoundDef::new("audio/forgia_original/pickups/soul_echo.ogg", 0.55),
            // Musique continue du jeu = le morceau « entre les waves » (user 2026-06-05).
            // combat == break → handle-compare évite tout restart aux transitions.
            music_combat: SoundDef::new("audio/forgia_original/music/forged_destiny_loop.ogg", 0.8),
            music_break: SoundDef::new("audio/forgia_original/music/forged_destiny_loop.ogg", 0.8),
            // Bande-son Suno 2026-08-05 — miroir du genome (fallback de parsing).
            music_hub: SoundDef::new("audio/forgia_original/music/hub.ogg", 0.8),
            music_chapters: (1..=10)
                .map(|i| {
                    SoundDef::new(
                        &format!("audio/forgia_original/music/chapter_{i:02}.ogg"),
                        0.8,
                    )
                })
                .collect(),
            // Re-balance 2026-08-05, 2e passe (miroir du genome) : les tirs
            // montent encore — la bande-son Suno (-17,8 dB moyen) écrasait des
            // fichiers d'armes à ~-35 dB. Chaque valeur est calculée pour que
            // `crête_fichier + volume + master` reste sous -2 dB (aucune saturation).
            fire_pepin: SoundDef::new("audio/forgia_original/weapons/pepin_forge.ogg", 2.05),
            fire_bourrasque: SoundDef::new(
                "audio/forgia_original/weapons/bourrasque_gale.ogg",
                2.20,
            ),
            fire_lenoir: SoundDef::new("audio/forgia_original/weapons/lenoir_royal.ogg", 2.80),
            fire_boucherie: SoundDef::new(
                "audio/forgia_original/weapons/boucherie_furnace.ogg",
                1.75,
            ),
            // Le dash reste discret (ambiance), mais le RECHARGEMENT remonte :
            // c'est de l'information de combat, pas de la décoration.
            dash: SoundDef::new("audio/forgia_original/movement/dash_ember.ogg", 0.28),
            reload_start: SoundDef::new("audio/forgia_original/movement/reload_start.ogg", 0.70),
            reload_complete: SoundDef::new(
                "audio/forgia_original/movement/reload_complete.ogg",
                0.90,
            ),
            weapon_switch: SoundDef::new("audio/forgia_original/movement/weapon_switch.ogg", 0.20),
            boon: SoundDef::new("audio/forgia_original/events/boon_forged.ogg", 0.7),
            chest: SoundDef::new("audio/forgia_original/events/chest_open.ogg", 0.65),
            wave_clear: SoundDef::new("audio/forgia_original/events/wave_clear.ogg", 0.6),
            wave_start: SoundDef::new("audio/forgia_original/events/wave_start.ogg", 0.55),
            boss_enrage: SoundDef::new("audio/forgia_original/events/boss_enrage.ogg", 0.85),
            victory: SoundDef::new("audio/forgia_original/events/victory.ogg", 0.9),
            defeat: SoundDef::new("audio/forgia_original/events/defeat.ogg", 0.75),
            ambience: SoundDef::new("audio/forgia_original/ambience/forge_heart_loop.ogg", 0.28),
            footsteps: [
                // Le son le plus répété du jeu (~1000 lectures/session) : -6 dB.
                SoundDef::new("audio/forgia_original/footsteps/forge_stone_01.ogg", 0.14),
                SoundDef::new("audio/forgia_original/footsteps/forge_stone_02.ogg", 0.14),
                SoundDef::new("audio/forgia_original/footsteps/forge_stone_03.ogg", 0.14),
                SoundDef::new("audio/forgia_original/footsteps/forge_stone_04.ogg", 0.14),
            ],
            // Famille UI (story-678) — la plus discrète du pack, cf. générateur.
            ui_sfx_volume: 1.0,
            ui_hover: SoundDef::new("audio/forgia_original/ui/ui_hover.ogg", 0.35),
            ui_click: SoundDef::new("audio/forgia_original/ui/ui_click.ogg", 0.5),
            ui_tab: SoundDef::new("audio/forgia_original/ui/ui_tab.ogg", 0.45),
            ui_buy: SoundDef::new("audio/forgia_original/ui/ui_buy.ogg", 0.6),
            ui_unlock: SoundDef::new("audio/forgia_original/ui/ui_unlock.ogg", 0.65),
            ui_denied: SoundDef::new("audio/forgia_original/ui/ui_denied.ogg", 0.5),
        }
    }
}

/// Handles préchargés (0 `load()` dans le hot path : tout chargé au Startup + reload).
#[derive(Resource, Default)]
pub struct AudioHandles {
    impact: Handle<AudioSource>,
    weakspot: Handle<AudioSource>,
    kill: Handle<AudioSource>,
    hurt: Handle<AudioSource>,
    ding_gold: Handle<AudioSource>,
    ding_souls: Handle<AudioSource>,
    music_combat: Handle<AudioSource>,
    music_break: Handle<AudioSource>,
    music_hub: Handle<AudioSource>,
    fire_pepin: Handle<AudioSource>,
    fire_bourrasque: Handle<AudioSource>,
    fire_lenoir: Handle<AudioSource>,
    fire_boucherie: Handle<AudioSource>,
    dash: Handle<AudioSource>,
    reload_start: Handle<AudioSource>,
    reload_complete: Handle<AudioSource>,
    weapon_switch: Handle<AudioSource>,
    boon: Handle<AudioSource>,
    chest: Handle<AudioSource>,
    wave_clear: Handle<AudioSource>,
    wave_start: Handle<AudioSource>,
    boss_enrage: Handle<AudioSource>,
    victory: Handle<AudioSource>,
    defeat: Handle<AudioSource>,
    ambience: Handle<AudioSource>,
    footsteps: [Handle<AudioSource>; 4],
    ui_hover: Handle<AudioSource>,
    ui_click: Handle<AudioSource>,
    ui_tab: Handle<AudioSource>,
    ui_buy: Handle<AudioSource>,
    ui_unlock: Handle<AudioSource>,
    ui_denied: Handle<AudioSource>,
}

/// Compteurs sensor (forgia2_roguelite_audio.json).
#[derive(Resource, Default)]
pub struct RogueliteAudioStats {
    pub fires: u32,
    pub impacts: u32,
    pub weakspots: u32,
    pub kills: u32,
    pub hurts: u32,
    pub dings_gold: u32,
    pub dings_souls: u32,
    pub sfx_played: u32,
    pub music_playing: bool,
    pub ambience_playing: bool,
    pub dash: u32,
    pub reloads: u32,
    pub boons: u32,
    pub chests: u32,
    pub wave_cues: u32,
    pub boss_cues: u32,
    pub footsteps: u32,
    /// Sons d'UI joués (story-678) — cumul session, menu compris.
    pub ui_sfx: u32,
    /// Piste musicale en cours (chemin asset) — vide si stoppée. Rend la
    /// sélection hub/chapitre falsifiable d'une lecture capteur.
    pub music_current: String,
    /// L'asset de `music_current` est-il RÉELLEMENT chargé ?
    ///
    /// Sans ce champ, `music_playing: true` ment : kira accepte de jouer un
    /// handle dont le chargement a échoué, et le capteur affichait « ça joue »
    /// pendant un silence total (défaut du 2026-08-06). « Demandé » n'est pas
    /// « chargé », et « chargé » n'est pas « audible » — mais au moins les deux
    /// premiers se distinguent maintenant.
    pub music_loaded: bool,
}

/// État musique : None=stoppée, Some(false)=combat, Some(true)=break.
#[derive(Resource, Default)]
struct MusicTrack {
    current: Option<Handle<AudioSource>>,
    instance: Option<Handle<AudioInstance>>,
    gain: f32,
    /// La SEULE piste de chapitre vivante, chargée à la demande (index, handle).
    /// Remplacer ce slot relâche la précédente : c'est ce qui garde l'empreinte
    /// mémoire de la bande-son à une piste au lieu de onze (cf. `sys_music_update`).
    chapter_slot: Option<(usize, Handle<AudioSource>)>,
}

#[derive(Resource, Default)]
struct AmbienceTrack {
    playing: bool,
    instance: Option<Handle<AudioInstance>>,
    gain: f32,
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
            .add_audio_channel::<AmbienceChannel>()
            .add_audio_channel::<VoiceChannel>()
            .init_resource::<RogueliteAudioConfig>()
            .init_resource::<AudioHandles>()
            .init_resource::<RogueliteAudioStats>()
            .init_resource::<MusicTrack>()
            .init_resource::<AmbienceTrack>()
            .insert_resource(AudioGenomeWatch {
                last_mtime: None,
                accum: 0.0,
            })
            .add_systems(
                Startup,
                (sys_init_audio_genome, sys_load_audio_handles).chain(),
            )
            .add_systems(OnExit(GameMode::Roguelite), sys_stop_all_audio)
            // Le hub joue en GameMode::None (menu) : couper si on quitte la
            // famille None|Roguelite, sinon l'instance kira boucle dans les
            // autres modes (le gate arrête le SYSTÈME, pas la lecture en cours).
            .add_systems(OnEnter(GameMode::Fps), sys_stop_all_audio)
            .add_systems(OnEnter(GameMode::Rpg), sys_stop_all_audio)
            .add_systems(OnEnter(GameMode::CyberCity), sys_stop_all_audio)
            .add_systems(
                Update,
                (
                    sys_fire_sfx,
                    sys_sfx_on_combat_hit,
                    sys_ding_on_currency,
                    sys_event_sfx,
                    sys_footsteps,
                )
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            // Musique + ambiance HORS cage Roguelite (2026-08-05) : le thème du
            // HUB doit jouer au MENU (GameMode::None) — même leçon que sys_ui_sfx
            // (story-678). Gate None|Roguelite : pas de fuite en Fps/Rpg/CyberCity,
            // et OnExit(Roguelite)→None coupe puis relance proprement sur le hub.
            .add_systems(
                Update,
                (sys_music_update, sys_ambience_update)
                    .in_set(GameSet::Effects)
                    .run_if(
                        in_state(GameMode::None).or(in_state(GameMode::Roguelite)),
                    ),
            )
            // Hors gate Roguelite (story-678) : le hub sonore vit au MENU, et le
            // hot-reload du genome doit suivre les genes UI partout (règle genome).
            .add_systems(
                Update,
                (sys_hot_reload_audio_genome, sys_ui_sfx).in_set(GameSet::Effects),
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
    apply_entry(&mut cfg.weakspot, toml.weakspot);
    apply_entry(&mut cfg.kill, toml.kill);
    apply_entry(&mut cfg.hurt, toml.hurt);
    apply_entry(&mut cfg.ding_gold, toml.ding_gold);
    apply_entry(&mut cfg.ding_souls, toml.ding_souls);
    apply_entry(&mut cfg.music_combat, toml.music_combat);
    apply_entry(&mut cfg.music_break, toml.music_break);
    apply_entry(&mut cfg.music_hub, toml.music_hub);
    if let Some(chapters) = toml.music_chapters {
        // Le genome fait AUTORITÉ sur la liste : sa longueur remplace le défaut
        // (un chapitre retiré du TOML retombe sur music_combat, pas sur le défaut).
        cfg.music_chapters = chapters
            .into_iter()
            .map(|entry| {
                let mut def = SoundDef::new("", 0.8);
                apply_entry(&mut def, Some(entry));
                def
            })
            .filter(|d| !d.path.is_empty())
            .collect();
    }
    apply_entry(&mut cfg.fire_pepin, toml.fire_pepin);
    apply_entry(&mut cfg.fire_bourrasque, toml.fire_bourrasque);
    apply_entry(&mut cfg.fire_lenoir, toml.fire_lenoir);
    apply_entry(&mut cfg.fire_boucherie, toml.fire_boucherie);
    apply_entry(&mut cfg.dash, toml.dash);
    apply_entry(&mut cfg.reload_start, toml.reload_start);
    apply_entry(&mut cfg.reload_complete, toml.reload_complete);
    apply_entry(&mut cfg.weapon_switch, toml.weapon_switch);
    apply_entry(&mut cfg.boon, toml.boon);
    apply_entry(&mut cfg.chest, toml.chest);
    apply_entry(&mut cfg.wave_clear, toml.wave_clear);
    apply_entry(&mut cfg.wave_start, toml.wave_start);
    apply_entry(&mut cfg.boss_enrage, toml.boss_enrage);
    apply_entry(&mut cfg.victory, toml.victory);
    apply_entry(&mut cfg.defeat, toml.defeat);
    apply_entry(&mut cfg.ambience, toml.ambience);
    apply_entry(&mut cfg.footsteps[0], toml.footstep_1);
    apply_entry(&mut cfg.footsteps[1], toml.footstep_2);
    apply_entry(&mut cfg.footsteps[2], toml.footstep_3);
    apply_entry(&mut cfg.footsteps[3], toml.footstep_4);
    if let Some(v) = toml.ui_sfx_volume {
        cfg.ui_sfx_volume = v.clamp(0.0, VOLUME_MAX);
    }
    apply_entry(&mut cfg.ui_hover, toml.ui_hover);
    apply_entry(&mut cfg.ui_click, toml.ui_click);
    apply_entry(&mut cfg.ui_tab, toml.ui_tab);
    apply_entry(&mut cfg.ui_buy, toml.ui_buy);
    apply_entry(&mut cfg.ui_unlock, toml.ui_unlock);
    apply_entry(&mut cfg.ui_denied, toml.ui_denied);
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
    let mut defs: Vec<&SoundDef> = vec![
        &cfg.impact,
        &cfg.weakspot,
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
        &cfg.dash,
        &cfg.reload_start,
        &cfg.reload_complete,
        &cfg.weapon_switch,
        &cfg.boon,
        &cfg.chest,
        &cfg.wave_clear,
        &cfg.wave_start,
        &cfg.boss_enrage,
        &cfg.victory,
        &cfg.defeat,
        &cfg.ambience,
        &cfg.footsteps[0],
        &cfg.footsteps[1],
        &cfg.footsteps[2],
        &cfg.footsteps[3],
        &cfg.ui_hover,
        &cfg.ui_click,
        &cfg.ui_tab,
        &cfg.ui_buy,
        &cfg.ui_unlock,
        &cfg.ui_denied,
    ];
    // Le hub est préchargé (il joue dès le menu). Les pistes de CHAPITRE ne le
    // sont PAS : 37 minutes d'audio décodées d'un coup faisaient échouer une
    // partie des chargements (cf. `sys_music_update`). Elles se chargent à la
    // demande, une seule à la fois.
    let dynamic_base = defs.len();
    defs.push(&cfg.music_hub);
    let h: Vec<Handle<AudioSource>> = defs
        .iter()
        .map(|d| asset_server.load(d.path.clone()))
        .collect();
    AudioHandles {
        impact: h[0].clone(),
        weakspot: h[1].clone(),
        kill: h[2].clone(),
        hurt: h[3].clone(),
        ding_gold: h[4].clone(),
        ding_souls: h[5].clone(),
        music_combat: h[6].clone(),
        music_break: h[7].clone(),
        fire_pepin: h[8].clone(),
        fire_bourrasque: h[9].clone(),
        fire_lenoir: h[10].clone(),
        fire_boucherie: h[11].clone(),
        dash: h[12].clone(),
        reload_start: h[13].clone(),
        reload_complete: h[14].clone(),
        weapon_switch: h[15].clone(),
        boon: h[16].clone(),
        chest: h[17].clone(),
        wave_clear: h[18].clone(),
        wave_start: h[19].clone(),
        boss_enrage: h[20].clone(),
        victory: h[21].clone(),
        defeat: h[22].clone(),
        ambience: h[23].clone(),
        footsteps: [h[24].clone(), h[25].clone(), h[26].clone(), h[27].clone()],
        ui_hover: h[28].clone(),
        ui_click: h[29].clone(),
        ui_tab: h[30].clone(),
        ui_buy: h[31].clone(),
        ui_unlock: h[32].clone(),
        ui_denied: h[33].clone(),
        music_hub: h[dynamic_base].clone(),
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

/// Story-651 — variation de pitch ±5 % (anti-fatigue sur sons répétitifs).
/// Xorshift local, présentation only (PAS la sim — hors keystone déterminisme).
/// Retourne un playback_rate dans [0.95, 1.05].
fn next_pitch(seed: &mut u32) -> f64 {
    if *seed == 0 {
        *seed = 0x9E37_79B9; // seed non-nul (xorshift(0) resterait 0)
    }
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    0.95 + 0.10 * (f64::from(*seed) / f64::from(u32::MAX))
}

fn sys_sfx_on_combat_hit(
    sfx: Res<AudioChannel<SfxChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    user_vol: Res<UserAudioVolumes>,
    mut stats: ResMut<RogueliteAudioStats>,
    mut events: MessageReader<CombatHitEvent>,
    q_enemy: Query<(), With<ArenaBot>>,
    q_player: Query<(), With<Player>>,
    mut pitch_seed: Local<u32>,
) {
    // Fix 2026-07-03 : le volume user (slider ESC) est appliqué à l'INSTANCE. En
    // bevy_kira_audio 0.25 le set_volume de canal ne compose PAS avec un with_volume
    // par-son (prouvé via forgia2_volume.json) → il faut multiplier ici.
    let master = cfg.master_volume * user_vol.sfx_gain();
    for ev in events.read() {
        // story-698 — LE SON DE KILL NE PARTAIT QUASIMENT JAMAIS : 8 pour 77 morts,
        // mesuré le 2026-08-12. Ce test était `q_enemy.get(ev.target).is_ok()`, une
        // requête sur une cible **déjà despawnée** :
        //
        //     despawn_dead_cubes    → GameSet::Combat
        //     sys_sfx_on_combat_hit → GameSet::Effects   (Combat passe AVANT)
        //
        // Sur un coup fatal, l'entité disparaît dans la même frame, avant qu'on
        // arrive ici. La requête échouait, le son ne partait pas — et le compteur,
        // qui est à l'intérieur du bloc, ne comptait pas non plus. D'où un capteur
        // à 8 qu'on a longtemps lu comme « 8 morts » au lieu de « 8 sons joués ».
        // Les 8 rescapés sont les morts survenues APRÈS le set Combat (roquette,
        // DoT) : celles-là survivent jusqu'à la frame suivante.
        //
        // Réparé sans toucher à l'ordonnancement — le déplacer coupleraient deux
        // crates pour un son. On déduit : cible introuvable + coup fatal + attaquant
        // ≠ la cible ⇒ c'était un ennemi. Le joueur, lui, reste query-able quand il
        // encaisse (il n'est pas despawné à la mort, il respawn), donc sa branche
        // `hurt` ci-dessous n'est pas volée.
        let cible_disparue = q_enemy.get(ev.target).is_err() && q_player.get(ev.target).is_err();
        let ennemi_mort_ce_frame = ev.is_kill && cible_disparue && ev.attacker != Some(ev.target);
        if q_enemy.get(ev.target).is_ok() || ennemi_mort_ce_frame {
            // L'ennemi encaisse : impact, ou kill si HP=0. Pitch varié ±5 %
            // (story-651) : ces sons jouent jusqu'à 16×/s en full-auto.
            let def = if ev.is_kill { &cfg.kill } else { &cfg.impact };
            let handle = if ev.is_kill {
                handles.kill.clone()
            } else {
                handles.impact.clone()
            };
            sfx.play(handle)
                .with_volume(amp_to_db(def.volume * master))
                .with_playback_rate(next_pitch(&mut pitch_seed));
            if ev.is_kill {
                stats.kills += 1;
            } else {
                stats.impacts += 1;
            }
            stats.sfx_played += 1;
            // Story-651 — « tink » weakspot : couche ADDITIVE sur CHAQUE headshot
            // (même au kill — double récompense Gunfire Reborn). Pitch FIXE :
            // c'est la signature pavlovienne « vise la tête », elle doit être
            // reconnaissable entre mille, jamais variée.
            if ev.is_headshot {
                sfx.play(handles.weakspot.clone())
                    .with_volume(amp_to_db(cfg.weakspot.volume * master));
                stats.weakspots += 1;
                stats.sfx_played += 1;
            }
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

/// Mapping `WeaponType` → signature sonore de persona. Les noms de
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
    user_vol: Res<UserAudioVolumes>,
    mut stats: ResMut<RogueliteAudioStats>,
    mut events: MessageReader<WeaponFiredEvent>,
    mut pitch_seed: Local<u32>,
) {
    // Fix 2026-07-03 : le volume user (slider ESC) est appliqué à l'INSTANCE. En
    // bevy_kira_audio 0.25 le set_volume de canal ne compose PAS avec un with_volume
    // par-son (prouvé via forgia2_volume.json) → il faut multiplier ici.
    let master = cfg.master_volume * user_vol.sfx_gain();
    for ev in events.read() {
        let def = weapon_fire_def(&cfg, ev.weapon);
        let handle = weapon_fire_handle(&handles, ev.weapon);
        // Story-651 — pitch ±5 % : casse la répétition métronome en full-auto
        // (règle Vlambeer/Destiny sound design : jamais deux tirs identiques).
        sfx.play(handle)
            .with_volume(amp_to_db(def.volume * master))
            .with_playback_rate(next_pitch(&mut pitch_seed));
        stats.fires += 1;
        stats.sfx_played += 1;
    }
}

// ─── Ding pickup (diff Resource, front-detection) ─────────────────────────────

fn sys_ding_on_currency(
    sfx: Res<AudioChannel<SfxChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    user_vol: Res<UserAudioVolumes>,
    mut stats: ResMut<RogueliteAudioStats>,
    gold: Option<Res<Souls>>,
    meta: Option<Res<MetaSouls>>,
    mut last_gold: Local<Option<u32>>,
    mut last_souls: Local<Option<u32>>,
) {
    // Fix 2026-07-03 : le volume user (slider ESC) est appliqué à l'INSTANCE. En
    // bevy_kira_audio 0.25 le set_volume de canal ne compose PAS avec un with_volume
    // par-son (prouvé via forgia2_volume.json) → il faut multiplier ici.
    let master = cfg.master_volume * user_vol.sfx_gain();
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

// ─── SFX de boucle de jeu ───────────────────────────────────────────────────

fn play_sfx(
    channel: &AudioChannel<SfxChannel>,
    handle: &Handle<AudioSource>,
    def: &SoundDef,
    gain: f32,
) {
    channel
        .play(handle.clone())
        .with_volume(amp_to_db(def.volume * gain));
}

// ─── Sons d'UI (story-678 Phase 1) ──────────────────────────────────────────

/// Draine la file de sons d'UI poussée par les helpers egui (forgia-ui-lib) et
/// joue chaque son. NON gaté sur `GameMode::Roguelite` : le hub vit au MENU.
/// Le hover est déjà dédoublonné à la source (front montant par widget).
pub(crate) fn sys_ui_sfx(
    mut contexts: EguiContexts,
    sfx: Res<AudioChannel<SfxChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    user_vol: Res<UserAudioVolumes>,
    mut stats: ResMut<RogueliteAudioStats>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let queued = drain_ui_sfx(ctx);
    if queued.is_empty() {
        return;
    }
    // Volume user à l'INSTANCE (le set_volume de canal ne compose pas, cf. fix
    // 2026-07-03) ; trim famille `ui_sfx_volume` par-dessus le master.
    let gain = cfg.master_volume * cfg.ui_sfx_volume * user_vol.sfx_gain();
    for kind in queued {
        let (handle, def) = match kind {
            UiSfxKind::Hover => (&handles.ui_hover, &cfg.ui_hover),
            UiSfxKind::Click => (&handles.ui_click, &cfg.ui_click),
            UiSfxKind::Tab => (&handles.ui_tab, &cfg.ui_tab),
            UiSfxKind::Buy => (&handles.ui_buy, &cfg.ui_buy),
            UiSfxKind::Unlock => (&handles.ui_unlock, &cfg.ui_unlock),
            UiSfxKind::Denied => (&handles.ui_denied, &cfg.ui_denied),
        };
        play_sfx(&sfx, handle, def, gain);
        stats.ui_sfx += 1;
        stats.sfx_played += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn sys_event_sfx(
    sfx: Res<AudioChannel<SfxChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    mix: Res<UserAudioVolumes>,
    wave: Option<Res<RogueliteWave>>,
    mut stats: ResMut<RogueliteAudioStats>,
    mut dash_events: MessageReader<DashUsedEvent>,
    mut ammo_events: MessageReader<AmmoChanged>,
    mut boon_events: MessageReader<BoonAppliedEvent>,
    mut chest_events: MessageReader<CoffrePickedEvent>,
    mut boss_events: MessageReader<BossEnrageTriggeredEvent>,
    mut end_events: MessageReader<EndRunEvent>,
    mut last_wave: Local<Option<(u8, u8, bool)>>,
) {
    let gain = cfg.master_volume * mix.sfx_gain();

    for _ in dash_events.read() {
        play_sfx(&sfx, &handles.dash, &cfg.dash, gain);
        stats.dash = stats.dash.saturating_add(1);
        stats.sfx_played = stats.sfx_played.saturating_add(1);
    }

    for event in ammo_events.read() {
        let sound = match event.kind {
            AmmoChangeKind::Reload { transferred: 0 } => {
                Some((&handles.reload_start, &cfg.reload_start))
            }
            AmmoChangeKind::Reload { .. } => {
                stats.reloads = stats.reloads.saturating_add(1);
                Some((&handles.reload_complete, &cfg.reload_complete))
            }
            AmmoChangeKind::WeaponSwitch => Some((&handles.weapon_switch, &cfg.weapon_switch)),
            AmmoChangeKind::Pickup { .. } => Some((&handles.ding_gold, &cfg.ding_gold)),
            AmmoChangeKind::Fire { .. } | AmmoChangeKind::GenomeApplied => None,
        };
        if let Some((handle, def)) = sound {
            play_sfx(&sfx, handle, def, gain);
            stats.sfx_played = stats.sfx_played.saturating_add(1);
        }
    }

    for _ in boon_events.read() {
        play_sfx(&sfx, &handles.boon, &cfg.boon, gain);
        stats.boons = stats.boons.saturating_add(1);
        stats.sfx_played = stats.sfx_played.saturating_add(1);
    }
    for _ in chest_events.read() {
        play_sfx(&sfx, &handles.chest, &cfg.chest, gain);
        stats.chests = stats.chests.saturating_add(1);
        stats.sfx_played = stats.sfx_played.saturating_add(1);
    }
    for _ in boss_events.read() {
        play_sfx(&sfx, &handles.boss_enrage, &cfg.boss_enrage, gain);
        stats.boss_cues = stats.boss_cues.saturating_add(1);
        stats.sfx_played = stats.sfx_played.saturating_add(1);
    }
    for event in end_events.read() {
        let (handle, def) = match event.result {
            RunResult::Victory => (&handles.victory, &cfg.victory),
            RunResult::Defeat | RunResult::Abort => (&handles.defeat, &cfg.defeat),
        };
        play_sfx(&sfx, handle, def, gain);
        stats.sfx_played = stats.sfx_played.saturating_add(1);
    }

    if let Some(wave) = wave {
        let now = (wave.stage, wave.current_wave, wave.in_break);
        if let Some(previous) = *last_wave {
            if !previous.2 && now.2 {
                play_sfx(&sfx, &handles.wave_clear, &cfg.wave_clear, gain);
                stats.wave_cues = stats.wave_cues.saturating_add(1);
                stats.sfx_played = stats.sfx_played.saturating_add(1);
            } else if previous.2 && !now.2 {
                play_sfx(&sfx, &handles.wave_start, &cfg.wave_start, gain);
                stats.wave_cues = stats.wave_cues.saturating_add(1);
                stats.sfx_played = stats.sfx_played.saturating_add(1);
            }
        }
        *last_wave = Some(now);
    }
}

fn sys_footsteps(
    time: Res<Time>,
    sfx: Res<AudioChannel<SfxChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    mix: Res<UserAudioVolumes>,
    locomotion: Res<PlayerLocomotion>,
    q_ground: Query<&KinematicCharacterControllerOutput, With<Player>>,
    mut stats: ResMut<RogueliteAudioStats>,
    mut elapsed: Local<f32>,
    mut index: Local<usize>,
    mut pitch_seed: Local<u32>,
) {
    let grounded = q_ground.single().map(|o| o.grounded).unwrap_or(false);
    let speed = locomotion.horizontal_speed;
    if !grounded || speed < 0.6 {
        *elapsed = 0.0;
        return;
    }

    *elapsed += time.delta_secs();
    let cadence = (0.58 - (speed / 8.0).clamp(0.0, 1.0) * 0.30).clamp(0.25, 0.58);
    if *elapsed < cadence {
        return;
    }
    *elapsed %= cadence;
    let slot = *index % handles.footsteps.len();
    sfx.play(handles.footsteps[slot].clone())
        .with_volume(amp_to_db(
            cfg.footsteps[slot].volume * cfg.master_volume * mix.sfx_gain(),
        ))
        .with_playback_rate(next_pitch(&mut pitch_seed));
    *index = index.wrapping_add(1);
    stats.footsteps = stats.footsteps.saturating_add(1);
    stats.sfx_played = stats.sfx_played.saturating_add(1);
}

// ─── Musique combat/break ─────────────────────────────────────────────────────

/// Compte les flux LOGIQUES d'un conteneur OGG (pur, sans dépendance).
///
/// Pourquoi ça existe : un MP3 exporté depuis un service de génération musicale
/// embarque une **pochette**. Convertir sans `-vn` la transcode en piste
/// **Theora** dans l'OGG — le fichier reste parfaitement lisible par ffmpeg,
/// mais le décodeur audio de Bevy refuse un conteneur multi-flux et le
/// chargement échoue **en silence** (2026-08-06 : les 11 pistes de la bande-son
/// étaient muettes pour cette seule raison). Aucun outil du projet ne le voyait.
///
/// Un OGG déclare un flux logique par page **BOS** (bit 0x02 de `header_type`)
/// en tête de fichier : les compter suffit, et n'exige que les premiers octets.
pub fn ogg_logical_stream_count(bytes: &[u8]) -> usize {
    const HEADER_LEN: usize = 27;
    let mut streams = 0usize;
    let mut pos = 0usize;
    while pos + HEADER_LEN <= bytes.len() {
        if &bytes[pos..pos + 4] != b"OggS" {
            break;
        }
        let header_type = bytes[pos + 5];
        // Les pages BOS sont groupées en tête ; la première page non-BOS clôt
        // la déclaration des flux.
        if header_type & 0x02 == 0 {
            break;
        }
        streams += 1;
        let segments = bytes[pos + 26] as usize;
        if pos + HEADER_LEN + segments > bytes.len() {
            break;
        }
        let payload: usize = bytes[pos + HEADER_LEN..pos + HEADER_LEN + segments]
            .iter()
            .map(|&b| b as usize)
            .sum();
        pos += HEADER_LEN + segments + payload;
    }
    streams
}

/// Pur — la piste du CHAPITRE ne joue que pendant la run elle-même.
///
/// Extrait du système parce que c'est exactement cette règle qui a bugué : le
/// critère « il existe une vague » était vrai dès le **Lobby** (qui vit dans
/// `GameMode::Roguelite`), et la musique de niveau démarrait avant la run.
/// Un test sur une fonction pure empêche la rechute.
/// Index de piste de chapitre d'une clé de musique de hub — `None` pour le
/// thème de la Forge (`hub`) ou une clé inconnue.
///
/// PUR — testable. Les clés du catalogue de cosmétiques sont `hub` et
/// `chapter_NN` (1-indexé, comme les fichiers) ; `music_chapters` est 0-indexé.
/// C'est exactement le genre de décalage qui, non nommé, se paie en « la
/// mauvaise piste joue ».
pub fn hub_music_chapter_index(key: &str) -> Option<usize> {
    key.strip_prefix("chapter_")?
        .parse::<usize>()
        .ok()?
        .checked_sub(1)
}

pub fn wants_chapter_track(state: Option<&crate::run::RunState>) -> bool {
    matches!(
        state,
        Some(crate::run::RunState::InRun { .. }) | Some(crate::run::RunState::Boss { .. })
    )
}

fn sys_music_update(
    music: Res<AudioChannel<MusicChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    user_vol: Res<UserAudioVolumes>,
    wave: Option<Res<RogueliteWave>>,
    run_state: Option<Res<State<crate::run::RunState>>>,
    chapter: Option<Res<crate::meta_shop::SelectedChapter>>,
    // Story-678 — le morceau du hub est une COSMÉTIQUE : le joueur le choisit
    // au Marketplace. Optionnel : la musique doit jouer même sans sauvegarde.
    identity: Option<Res<crate::identity::IdentitySave>>,
    asset_server: Res<AssetServer>,
    mut track: ResMut<MusicTrack>,
    mut stats: ResMut<RogueliteAudioStats>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    // Bande-son 2026-08-05 : la piste du CHAPITRE ne joue que pendant la run
    // elle-même ; partout ailleurs c'est le thème du HUB.
    //
    // ⚠️ Le critère est `RunState`, PAS la présence de `RogueliteWave`. Le Lobby
    // vit DANS `GameMode::Roguelite` et la ressource de vague y existe déjà —
    // s'en servir faisait démarrer la musique de niveau dès le Lobby (défaut
    // constaté en jeu). Cf. [[reference_menu_hub_is_not_the_lobby]] : le hub et
    // le Lobby sont deux endroits distincts, et tous deux veulent le thème calme.
    //
    // Couverture : Menu (pas de SubState → None) · Lobby · Defeat · Victory →
    // hub ; InRun/Boss → chapitre. Chapitre sans piste dédiée (liste genome plus
    // courte) → repli sur combat/break, l'ancien comportement.
    let in_run = wants_chapter_track(run_state.as_deref().map(|s| s.get()));
    let in_break = wave.map(|w| w.in_break).unwrap_or(false);

    // ⚠️ Les pistes de chapitre sont chargées À LA DEMANDE, jamais toutes au
    // Startup (défaut du 2026-08-05, corrigé le 06-08). Kira décode les sons
    // ENTIÈREMENT en mémoire : les 11 pistes font 37 minutes, soit ~0,4-0,8 Go
    // décodés. Les précharger faisait échouer une partie des chargements — un
    // sous-ensemble DIFFÉRENT à chaque lancement, signature d'un épuisement de
    // ressources — et le hub tombait muet en silence (`music_playing: true` sur
    // un handle mort). Ici : une seule piste vivante à la fois ; remplacer le
    // slot libère la précédente (plus aucun handle fort → Bevy la décharge).
    let (handle, def): (Handle<AudioSource>, &SoundDef) = if !in_run {
        // Story-678 — le hub joue le morceau CHOISI. Un thème de chapitre porté
        // comme cosmétique passe par le même slot à la demande que pendant une
        // run : une seule piste vivante à la fois, la contrainte mémoire de kira
        // ne change pas parce qu'on est au menu.
        let choisi = identity
            .as_deref()
            .and_then(|i| hub_music_chapter_index(&i.hub_music))
            .filter(|idx| cfg.music_chapters.get(*idx).is_some());
        match choisi {
            Some(idx) => {
                let def = &cfg.music_chapters[idx];
                let stale = track
                    .chapter_slot
                    .as_ref()
                    .map(|(i, _)| *i != idx)
                    .unwrap_or(true);
                if stale {
                    track.chapter_slot = Some((idx, asset_server.load(def.path.clone())));
                }
                let h = track
                    .chapter_slot
                    .as_ref()
                    .map(|(_, h)| h.clone())
                    .expect("le slot vient d'être rempli");
                (h, def)
            }
            None => {
                // Thème de la Forge : on relâche la piste de chapitre, ~40-160 Mo rendus.
                track.chapter_slot = None;
                (handles.music_hub.clone(), &cfg.music_hub)
            }
        }
    } else {
        let idx = chapter.map(|c| c.0).unwrap_or(1).saturating_sub(1) as usize;
        match cfg.music_chapters.get(idx) {
            Some(d) => {
                let stale = track
                    .chapter_slot
                    .as_ref()
                    .map(|(i, _)| *i != idx)
                    .unwrap_or(true);
                if stale {
                    track.chapter_slot = Some((idx, asset_server.load(d.path.clone())));
                }
                // `expect` sûr : la branche ci-dessus vient de le remplir.
                let h = track
                    .chapter_slot
                    .as_ref()
                    .map(|(_, h)| h.clone())
                    .expect("chapter_slot rempli juste au-dessus");
                (h, d)
            }
            None if in_break => (handles.music_break.clone(), &cfg.music_break),
            None => (handles.music_combat.clone(), &cfg.music_combat),
        }
    };
    // Ne (re)démarre QUE si le morceau voulu diffère de l'actuel → pas de restart
    // quand combat et break pointent sur le MÊME fichier (musique continue ;
    // demande user : « le son entre les waves = la musique du jeu »).
    let gain = def.volume * cfg.music_volume * user_vol.music_gain();
    // Vérité de chargement, relue chaque frame : une piste peut échouer bien
    // après avoir été demandée (chargement asynchrone).
    stats.music_loaded = asset_server.is_loaded_with_dependencies(&handle);
    if track.current.as_ref() == Some(&handle) {
        if (track.gain - gain).abs() > 0.001 {
            if let Some(instance) = track.instance.as_ref().and_then(|h| instances.get_mut(h)) {
                instance.set_decibels(amp_to_db(gain), AudioTween::default());
            }
            track.gain = gain;
        }
        return;
    }
    music.stop();
    let instance = music
        .play(handle.clone())
        .looped()
        .with_volume(amp_to_db(gain))
        .handle();
    track.current = Some(handle);
    track.instance = Some(instance);
    track.gain = gain;
    stats.music_playing = true;
    stats.music_current = def.path.clone();
}

fn sys_ambience_update(
    ambience: Res<AudioChannel<AmbienceChannel>>,
    handles: Res<AudioHandles>,
    cfg: Res<RogueliteAudioConfig>,
    mix: Res<UserAudioVolumes>,
    mut track: ResMut<AmbienceTrack>,
    mut stats: ResMut<RogueliteAudioStats>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    let gain = cfg.ambience.volume * mix.ambience_gain();
    if track.playing {
        if (track.gain - gain).abs() > 0.001 {
            if let Some(instance) = track.instance.as_ref().and_then(|h| instances.get_mut(h)) {
                instance.set_decibels(amp_to_db(gain), AudioTween::default());
            }
            track.gain = gain;
        }
        return;
    }
    let instance = ambience
        .play(handles.ambience.clone())
        .looped()
        .with_volume(amp_to_db(gain))
        .handle();
    track.playing = true;
    track.instance = Some(instance);
    track.gain = gain;
    stats.ambience_playing = true;
}

fn sys_stop_all_audio(
    music: Res<AudioChannel<MusicChannel>>,
    ambience: Res<AudioChannel<AmbienceChannel>>,
    mut track: ResMut<MusicTrack>,
    mut ambience_track: ResMut<AmbienceTrack>,
    mut stats: ResMut<RogueliteAudioStats>,
) {
    music.stop();
    ambience.stop();
    track.current = None;
    track.instance = None;
    ambience_track.playing = false;
    ambience_track.instance = None;
    stats.music_playing = false;
    stats.ambience_playing = false;
    stats.music_current.clear();
    stats.music_loaded = false;
    // Relâche aussi la piste de chapitre : quitter le mode doit rendre sa
    // mémoire, pas la garder au chaud pour une run qui ne viendra peut-être pas.
    track.chapter_slot = None;
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

    // La panne la plus vicieuse d'abord : une piste DEMANDÉE mais jamais
    // chargée. Kira la « joue » quand même → silence total avec un capteur au
    // vert. C'est exactement ce qui est arrivé au thème du hub le 2026-08-06.
    let (severity, next_step) = if stats.music_playing && !stats.music_loaded {
        (
            "warn",
            "MUSIQUE MUETTE : la piste est jouée mais son asset n'est pas chargé. Vérifier le chemin dans roguelite_audio.toml, et forgia2_assets.json::event_failed_paths (un préchargement trop gros fait échouer des sons au hasard).",
        )
    } else if !stats.music_playing && stats.sfx_played == 0 {
        (
            "info",
            "Entre en Roguelite + tire/encaisse/ramasse pour générer du son (slice A : impact/kill/ding/musique).",
        )
    } else {
        ("ok", "-")
    };
    let json = format!(
        r#"{{"id":"roguelite_audio","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"fires":{},"impacts":{},"weakspots":{},"kills":{},"hurts":{},"dings_gold":{},"dings_souls":{},"dash":{},"reloads":{},"boons":{},"chests":{},"wave_cues":{},"boss_cues":{},"footsteps":{},"ui_sfx":{},"sfx_played":{},"music_playing":{},"music_loaded":{},"ambience_playing":{},"music_current":"{}","master_volume":{:.2},"music_volume":{:.2}}}"#,
        time.elapsed_secs(),
        stats.fires,
        stats.impacts,
        stats.weakspots,
        stats.kills,
        stats.hurts,
        stats.dings_gold,
        stats.dings_souls,
        stats.dash,
        stats.reloads,
        stats.boons,
        stats.chests,
        stats.wave_cues,
        stats.boss_cues,
        stats.footsteps,
        stats.ui_sfx,
        stats.sfx_played,
        stats.music_playing,
        stats.music_loaded,
        stats.ambience_playing,
        stats.music_current,
        cfg.master_volume,
        cfg.music_volume,
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_variation_stays_in_bounds_and_varies() {
        // ±5 % : toujours dans [0.95, 1.05], et pas constant (anti-métronome).
        let mut seed = 0u32; // le helper doit s'auto-seeder (xorshift(0) = piège)
        let mut prev = next_pitch(&mut seed);
        let mut varied = false;
        for _ in 0..64 {
            let p = next_pitch(&mut seed);
            assert!((0.95..=1.05).contains(&p), "pitch hors bornes : {p}");
            if (p - prev).abs() > 1e-9 {
                varied = true;
            }
            prev = p;
        }
        assert!(varied, "le pitch doit varier entre les tirs");
    }

    /// LE garde-fou du 2026-08-06 : aucun asset audio ne doit être multi-flux.
    ///
    /// Une pochette d'album embarquée devient une piste Theora à la conversion,
    /// et Bevy refuse alors de charger le son — sans erreur visible en jeu. Ce
    /// test scanne les fichiers RÉELS : c'est un ratchet, pas une vérification
    /// de logique. Il échouera à la seconde où quelqu'un réimportera une piste
    /// sans `-vn`.
    #[test]
    fn no_audio_asset_carries_a_cover_art_stream() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/audio/forgia_original");
        if !root.exists() {
            // Pas d'assets sous la main (checkout partiel) : on ne prétend pas
            // avoir vérifié — un contrôle qui ne mesure rien n'est pas vert.
            eprintln!("assets absents, contrôle non exécuté");
            return;
        }
        let mut checked = 0usize;
        let mut offenders: Vec<String> = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("ogg") {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                checked += 1;
                let streams = ogg_logical_stream_count(&bytes);
                if streams != 1 {
                    offenders.push(format!(
                        "{} ({streams} flux)",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ));
                }
            }
        }
        assert!(
            checked > 0,
            "aucun .ogg scanné — le contrôle serait aveugle"
        );
        assert!(
            offenders.is_empty(),
            "{checked} fichiers scannés, {} multi-flux (Bevy ne les chargera pas) : {:?}\n\
             Remède : réencoder avec `-vn -map 0:a:0`.",
            offenders.len(),
            offenders
        );
    }

    #[test]
    fn ogg_stream_counter_reads_the_bos_pages() {
        // Une page BOS minimale : magie + version + header_type 0x02 + 22 octets
        // d'en-tête, 0 segment.
        let mut page = Vec::new();
        page.extend_from_slice(b"OggS");
        page.push(0); // version
        page.push(0x02); // BOS
        page.extend_from_slice(&[0u8; 20]); // granule + serial + seq + crc
        page.push(0); // 0 segment
        assert_eq!(ogg_logical_stream_count(&page), 1);

        // Deux flux déclarés = le cas Theora+Vorbis qui casse le chargement.
        let mut two = page.clone();
        two.extend_from_slice(&page);
        assert_eq!(ogg_logical_stream_count(&two), 2);

        // Ce qui n'est pas de l'OGG ne compte aucun flux (pas de panique).
        assert_eq!(ogg_logical_stream_count(b"pas un ogg"), 0);
        assert_eq!(ogg_logical_stream_count(&[]), 0);
    }

    #[test]
    fn the_lobby_keeps_the_hub_theme() {
        use crate::run::RunState;
        // Le défaut constaté en jeu : la musique de niveau démarrait dès le
        // Lobby. Le Lobby n'est PAS la run — il garde le thème calme, comme le
        // menu. Seuls InRun et Boss réclament la piste du chapitre.
        assert!(!wants_chapter_track(Some(&RunState::Lobby)), "Lobby → hub");
        assert!(!wants_chapter_track(None), "menu (pas de SubState) → hub");
        assert!(
            !wants_chapter_track(Some(&RunState::Defeat)),
            "défaite → hub"
        );
        assert!(
            !wants_chapter_track(Some(&RunState::Victory)),
            "victoire → hub"
        );
        assert!(wants_chapter_track(Some(&RunState::InRun { stage: 0 })));
        assert!(wants_chapter_track(Some(&RunState::Boss { stage: 3 })));
    }

    #[test]
    fn la_musique_de_hub_choisie_pointe_la_bonne_piste() {
        // Story-678 — le morceau du hub est une cosmétique. Les clés du
        // catalogue sont 1-indexées (`chapter_02`, comme les fichiers) alors que
        // `music_chapters` est 0-indexé : c'est LE décalage qui, non nommé, se
        // paie en « ce n'est pas la bonne musique qui joue ».
        assert_eq!(hub_music_chapter_index("chapter_01"), Some(0));
        assert_eq!(hub_music_chapter_index("chapter_10"), Some(9));
        // Le thème de la Forge n'est pas une piste de chapitre.
        assert_eq!(hub_music_chapter_index("hub"), None);
        // Rien d'inventé sur une clé absurde — repli sur le thème de la Forge.
        assert_eq!(hub_music_chapter_index(""), None);
        assert_eq!(hub_music_chapter_index("chapter_zero"), None);
        // `chapter_00` n'existe pas : 1-indexé, donc pas de soustraction sous zéro.
        assert_eq!(hub_music_chapter_index("chapter_00"), None);
    }

    #[test]
    fn weapon_volumes_stay_below_clipping_after_master() {
        // Garde-fou de mix : les volumes du pack sont calculés à partir des
        // crêtes MESURÉES des fichiers ; si quelqu'un les relève à l'aveugle,
        // ce test dit à partir d'où ça sature. Crêtes mesurées (ffmpeg
        // volumedetect, fichiers du 2026-08-05).
        let cfg = RogueliteAudioConfig::default();
        let peaks_db = [
            (&cfg.fire_pepin, -7.5_f32),
            (&cfg.fire_bourrasque, -10.0),
            (&cfg.fire_lenoir, -9.9),
            (&cfg.fire_boucherie, -5.9),
        ];
        for (def, peak) in peaks_db {
            let final_db = peak + 20.0 * def.volume.log10() + 20.0 * cfg.master_volume.log10();
            assert!(
                final_db < -1.0,
                "{} saturerait : crête finale {:.1} dB",
                def.path,
                final_db
            );
        }
    }

    #[test]
    fn reload_is_audible_but_stays_under_the_shots() {
        // Le rechargement est de l'INFORMATION : il doit s'entendre, sans
        // jamais passer devant le tir. L'ordre du mix est un invariant.
        let cfg = RogueliteAudioConfig::default();
        assert!(
            cfg.reload_complete.volume > cfg.reload_start.volume,
            "c'est la FIN du rechargement qui rend la main"
        );
        assert!(
            cfg.reload_complete.volume < cfg.fire_pepin.volume,
            "le rechargement ne passe jamais devant le tir"
        );
        assert!(
            cfg.reload_start.volume > cfg.footsteps[0].volume,
            "un rechargement s'entend mieux qu'un pas"
        );
    }

    #[test]
    fn default_soundtrack_covers_hub_and_the_ten_chapters() {
        // La bande-son 2026-08-05 : 1 hub + 1 piste par chapitre du Livre.
        // Garde la convention de nommage chapter_XX (zero-padded, 1-indexé).
        let cfg = RogueliteAudioConfig::default();
        assert_eq!(cfg.music_hub.path, "audio/forgia_original/music/hub.ogg");
        assert_eq!(cfg.music_chapters.len(), 10);
        assert_eq!(
            cfg.music_chapters[0].path,
            "audio/forgia_original/music/chapter_01.ogg"
        );
        assert_eq!(
            cfg.music_chapters[9].path,
            "audio/forgia_original/music/chapter_10.ogg"
        );
    }

    #[test]
    fn music_chapters_toml_deserializes_as_array_of_tables() {
        // Le genome fait autorité sur la LISTE : 2 entrées déclarées = 2 pistes
        // (les chapitres suivants retomberont sur music_combat au runtime).
        let toml: AudioGenomeToml = toml::from_str(
            r#"
            [music_hub]
            path = "audio/x/hub.ogg"

            [[music_chapters]]
            path = "audio/x/c1.ogg"
            volume = 0.7

            [[music_chapters]]
            path = "audio/x/c2.ogg"
            "#,
        )
        .expect("TOML valide");
        let chapters = toml.music_chapters.expect("array of tables présent");
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].path, "audio/x/c1.ogg");
        assert_eq!(chapters[0].volume, Some(0.7));
        assert_eq!(toml.music_hub.expect("hub présent").path, "audio/x/hub.ogg");
    }

    #[test]
    fn weakspot_default_points_to_original_asset() {
        let cfg = RogueliteAudioConfig::default();
        assert_eq!(
            cfg.weakspot.path,
            "audio/forgia_original/combat/weakspot_chime.ogg"
        );
        assert!(cfg.weakspot.volume > 0.0);
    }
}
