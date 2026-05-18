//! Killfeed tuning genome-driven. Aucun pixel hardcodé Rust.
//!
//! Source : `assets/genomes/killfeed_tuning.toml`. Hot-reload via Bevy Asset.

use bevy::prelude::*;
use bevy::reflect::TypePath;
use forgia_genome_core::Genome;
use serde::Deserialize;

#[derive(Resource, Deserialize, Clone, Debug, TypePath)]
pub struct KillfeedTuning {
    // ─── Layout feed (top-right) ───────────────────────────────
    /// Padding par rapport au bord droit (px).
    pub padding_right: f32,
    /// Y du premier entry (top) — typiquement sous wave counter.
    pub first_entry_y: f32,
    /// Largeur d'un entry (px).
    pub entry_width: f32,
    /// Hauteur d'un entry (px).
    pub entry_height: f32,
    /// Espacement vertical entre entries.
    pub entry_spacing: f32,
    /// Cap : nombre max d'entries visibles. Overwatch/CS2 ≈ 5.
    pub max_entries: usize,

    // ─── Timings ───────────────────────────────────────────────
    /// Durée totale d'affichage d'un entry (secondes). AAA 5-6s.
    pub display_secs: f32,
    /// Durée du fade-out final (secondes).
    pub fade_out_secs: f32,
    /// Durée du slide-in lateral droite→gauche (secondes).
    pub slide_in_secs: f32,
    /// Distance du slide-in (px) — l'entry vient de la droite et glisse à gauche.
    pub slide_in_distance: f32,

    // ─── Fonts ─────────────────────────────────────────────────
    pub font_attacker: f32,
    pub font_arrow: f32,
    pub font_victim: f32,
    pub font_weapon_icon: f32,
    pub text_outline_px: f32,
    /// Layout interne d'un entry (depuis bord gauche).
    pub entry_pad_inner: f32,         // padding interne gauche/droite (texte attacker depuis L)
    pub icon_offset_x: f32,           // icon position depuis (L + pad)
    pub arrow_offset_x_from_icon: f32,// arrow depuis icon position
    pub victim_offset_x_from_icon: f32,// victim depuis icon position
    pub entry_corner_rounding: f32,   // corner rounding entry
    pub entry_outline_px: f32,        // outline thickness entry
    pub headshot_font_bonus: f32,     // bonus size pour le star headshot

    // ─── Multi-kill banner (Halo-style sweeping CENTER_TOP) ────
    /// Fenêtre temporelle pour comptabiliser un multi-kill (secondes).
    /// 2 kills < window → DOUBLE. 3 < window → TRIPLE.
    pub multikill_window_secs: f32,
    /// Durée d'affichage du banner multi-kill (secondes).
    pub multikill_banner_secs: f32,
    /// Padding Y du banner (depuis top).
    pub multikill_banner_y: f32,
    /// Taille font du banner.
    pub multikill_font: f32,

    // ─── Sensor ────────────────────────────────────────────────
    pub sensor_period_secs: f32,
}

impl Default for KillfeedTuning {
    fn default() -> Self {
        Self {
            padding_right: 32.0,
            first_entry_y: 110.0,
            entry_width: 360.0,
            entry_height: 34.0,
            entry_spacing: 6.0,
            max_entries: 5,

            display_secs: 5.0,
            fade_out_secs: 1.0,
            slide_in_secs: 0.18,
            slide_in_distance: 60.0,

            font_attacker: 18.0,
            font_arrow: 22.0,
            font_victim: 18.0,
            font_weapon_icon: 20.0,
            text_outline_px: 1.5,
            entry_pad_inner: 14.0,
            icon_offset_x: 90.0,
            arrow_offset_x_from_icon: 24.0,
            victim_offset_x_from_icon: 48.0,
            entry_corner_rounding: 6.0,
            entry_outline_px: 2.0,
            headshot_font_bonus: 2.0,

            multikill_window_secs: 4.0,
            multikill_banner_secs: 1.8,
            multikill_banner_y: 130.0,
            multikill_font: 56.0,

            sensor_period_secs: 1.0,
        }
    }
}

#[derive(Resource)]
pub struct KillfeedTuningHandle(pub Handle<Genome<KillfeedTuning>>);

pub fn load_killfeed_tuning(mut commands: Commands, server: Res<AssetServer>) {
    let handle: Handle<Genome<KillfeedTuning>> = server.load("genomes/killfeed_tuning.toml");
    commands.insert_resource(KillfeedTuningHandle(handle));
}

pub fn sync_killfeed_tuning(
    mut events: MessageReader<AssetEvent<Genome<KillfeedTuning>>>,
    handle: Option<Res<KillfeedTuningHandle>>,
    assets: Res<Assets<Genome<KillfeedTuning>>>,
    mut tuning: ResMut<KillfeedTuning>,
) {
    let Some(handle) = handle else { return };
    let mut should = false;
    for ev in events.read() {
        match ev {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id } => {
                if *id == handle.0.id() {
                    should = true;
                }
            }
            _ => {}
        }
    }
    if !should {
        return;
    }
    if let Some(g) = assets.get(&handle.0) {
        *tuning = g.data.clone();
        info!("[killfeed] tuning synced (display {}s, max {})", tuning.display_secs, tuning.max_entries);
    }
}
