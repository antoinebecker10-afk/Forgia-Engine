//! Tuning genome-driven du HUD ammo. Aucun pixel hardcodé en Rust.
//!
//! Source : `assets/genomes/hud_ammo_tuning.toml` (hot-reload via `Genome` asset).
//! Defaults dans `Default impl` si fichier absent (boot safety).

use bevy::prelude::*;
use bevy::reflect::TypePath;
use forgia_genome_core::Genome;
use serde::Deserialize;

#[derive(Resource, Deserialize, Clone, Debug, TypePath)]
pub struct AmmoHudTuning {
    // ─── Counter principal (bottom-right) ─────────────────────────
    /// Padding par rapport au bord bas-droit (px).
    pub counter_padding_right: f32,
    pub counter_padding_bottom: f32,
    /// Taille font du nombre courant (current_mag). Cartoon bold AAA = 64-96px.
    pub counter_font_main: f32,
    /// Taille font du reserve (plus petit, sous-titre).
    pub counter_font_reserve: f32,
    /// Largeur du panel ammo (px). Détermine zone fond.
    pub counter_panel_w: f32,
    pub counter_panel_h: f32,
    /// Outline thickness du texte counter (px).
    pub counter_outline_px: f32,
    /// Décalage du texte principal (current_mag) depuis le bord droit du panel (px).
    pub counter_main_padding_right: f32,
    /// Décalage Y du texte principal par rapport au centre vertical du panel (px positif = bas).
    pub counter_main_y_offset: f32,
    /// Décalage Y du texte reserve par rapport au haut du panel (px).
    pub counter_reserve_padding_top: f32,
    /// Padding interne du label "AMMO" (top-left).
    pub label_padding_left: f32,
    pub label_padding_top: f32,
    pub label_font_size: f32,
    pub label_outline_px: f32,
    /// Padding du texte "RELOADING…" (bottom-left du panel).
    pub reload_indicator_padding_left: f32,
    pub reload_indicator_padding_bottom: f32,
    pub reload_indicator_font_size: f32,
    pub reload_indicator_outline_px: f32,
    /// Reserve text outline thickness.
    pub counter_reserve_outline_px: f32,

    // ─── Slot strip vertical droite ────────────────────────────────
    /// Padding du strip par rapport au bord droit (px).
    pub slot_padding_right: f32,
    /// Y du PREMIER slot (top) (px depuis haut écran).
    pub slot_first_y: f32,
    /// Dimensions box d'un slot (px).
    pub slot_box_w: f32,
    pub slot_box_h: f32,
    /// Espacement vertical entre slots (px).
    pub slot_spacing: f32,
    /// Scale du slot actif (1.0 = identique inactif, 1.1 = +10%).
    pub slot_active_scale: f32,
    /// Alpha du slot inactif (0..1).
    pub slot_inactive_alpha: f32,
    /// Outline thickness slot inactif (px).
    pub slot_outline_inactive: f32,
    /// Outline thickness slot actif (px).
    pub slot_outline_active: f32,
    /// Taille font de l'initiale (P/B/L/M fallback icône). Cartoon.
    pub slot_initial_font: f32,
    /// Taille font hotkey (1/2/3/4) corner top-left.
    pub slot_hotkey_font: f32,
    /// Corner rounding des slots (px).
    pub slot_corner_rounding: f32,
    /// Padding hotkey label (corner top-left du slot).
    pub slot_hotkey_padding_x: f32,
    pub slot_hotkey_padding_y: f32,
    /// Padding bas du mini-compteur dans la box slot (px).
    pub slot_mini_counter_padding_bottom: f32,
    /// Font size du mini-compteur ammo dans le slot (base, multiplié par scale).
    pub slot_mini_counter_font: f32,
    /// Outline du mini-compteur.
    pub slot_mini_counter_outline_px: f32,
    /// Outline initiale arme dans le slot.
    pub slot_initial_outline_px: f32,

    // ─── Reload arc (autour slot actif) ────────────────────────────
    /// Rayon arc reload (px) — autour du centre du slot actif.
    pub reload_arc_radius: f32,
    /// Épaisseur trait arc reload (px).
    pub reload_arc_thickness: f32,
    /// Segments tessellation arc (resolution).
    pub reload_arc_segments: usize,

    // ─── Low-ammo flash ────────────────────────────────────────────
    /// Fréquence pulse low-ammo (Hz). AAA standard 2.0 Hz.
    pub low_flash_hz: f32,
    /// Alpha min du pulse (0..1).
    pub low_flash_alpha_min: f32,
    /// Alpha max du pulse (0..1).
    pub low_flash_alpha_max: f32,

    // ─── Sensor ────────────────────────────────────────────────────
    /// Période d'écriture `forgia_hud_ammo.json` (secondes). 1.0 = 1Hz.
    pub sensor_period_secs: f32,
}

impl Default for AmmoHudTuning {
    fn default() -> Self {
        Self {
            counter_padding_right: 32.0,
            counter_padding_bottom: 28.0,
            counter_font_main: 78.0,
            counter_font_reserve: 28.0,
            counter_panel_w: 280.0,
            counter_panel_h: 120.0,
            counter_outline_px: 2.5,
            counter_main_padding_right: 28.0,
            counter_main_y_offset: 18.0,
            counter_reserve_padding_top: 18.0,
            counter_reserve_outline_px: 1.5,
            label_padding_left: 14.0,
            label_padding_top: 12.0,
            label_font_size: 14.0,
            label_outline_px: 1.0,
            reload_indicator_padding_left: 14.0,
            reload_indicator_padding_bottom: 22.0,
            reload_indicator_font_size: 13.0,
            reload_indicator_outline_px: 1.0,

            slot_padding_right: 24.0,
            slot_first_y: 220.0,
            slot_box_w: 56.0,
            slot_box_h: 56.0,
            slot_spacing: 10.0,
            slot_active_scale: 1.10,
            slot_inactive_alpha: 0.55,
            slot_outline_inactive: 2.0,
            slot_outline_active: 3.5,
            slot_initial_font: 28.0,
            slot_hotkey_font: 14.0,
            slot_corner_rounding: 6.0,
            slot_hotkey_padding_x: 6.0,
            slot_hotkey_padding_y: 4.0,
            slot_mini_counter_padding_bottom: 6.0,
            slot_mini_counter_font: 11.0,
            slot_mini_counter_outline_px: 0.8,
            slot_initial_outline_px: 1.5,

            reload_arc_radius: 36.0,
            reload_arc_thickness: 4.5,
            reload_arc_segments: 36,

            low_flash_hz: 2.0,
            low_flash_alpha_min: 0.4,
            low_flash_alpha_max: 1.0,

            sensor_period_secs: 1.0,
        }
    }
}

#[derive(Resource)]
pub struct AmmoHudTuningHandle(pub Handle<Genome<AmmoHudTuning>>);

/// Charge le genome au startup. Path : `genomes/hud_ammo_tuning.toml`.
pub fn load_ammo_hud_tuning(mut commands: Commands, server: Res<AssetServer>) {
    let handle: Handle<Genome<AmmoHudTuning>> = server.load("genomes/hud_ammo_tuning.toml");
    commands.insert_resource(AmmoHudTuningHandle(handle));
}

/// Sync genome → Resource AmmoHudTuning. Run sur Asset Created/Modified.
pub fn sync_ammo_hud_tuning(
    mut events: MessageReader<AssetEvent<Genome<AmmoHudTuning>>>,
    handle: Option<Res<AmmoHudTuningHandle>>,
    assets: Res<Assets<Genome<AmmoHudTuning>>>,
    mut tuning: ResMut<AmmoHudTuning>,
) {
    let Some(handle) = handle else { return };
    let mut should_sync = false;
    for ev in events.read() {
        match ev {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id }
                if *id == handle.0.id() =>
            {
                should_sync = true;
            }
            _ => {}
        }
    }
    if !should_sync {
        return;
    }
    if let Some(genome) = assets.get(&handle.0) {
        *tuning = genome.data.clone();
        info!(
            "[ammo-hud] tuning genome synced (counter {}px, slot {}px)",
            tuning.counter_font_main, tuning.slot_box_w
        );
    }
}
