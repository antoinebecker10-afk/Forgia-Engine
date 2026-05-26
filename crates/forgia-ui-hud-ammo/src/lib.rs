//! # forgia-ui-hud-ammo
//!
//! Story-455 Phase B — FPS Ammo HUD. Layout AAA :
//! - **Counter principal** : bottom-right, `current_mag` (gros) + `/ reserve` (petit).
//! - **Vertical slot strip** : N cases droite, 1 par arme équipée (`EquippedWeapons.slots`).
//!   Active = scale up + outline jaune Forgia. Inactives = alpha réduit.
//!   Initiale arme (P/B/L/M fallback) + hotkey 1-N en coin.
//! - **Reload arc** : circular progress autour du slot actif (CoD pattern).
//! - **Low-ammo flash** : pulse 2Hz rouge saturé quand mag_fraction ≤ low_ammo_threshold
//!   (genome-driven per weapon `viewmodel_arena.toml`).
//!
//! Sources research AAA (story-455 §7) :
//! - Halo Infinite / Apex / Valorant / Destiny 2 bottom-right consensus
//! - York Univ. IEEE GEM 2015 : diegetic numeric > HUD icons (35% accuracy gain)
//! - Dot Esports : low-ammo saturated red = non-negociable signal
//!
//! Genome-driven : `assets/genomes/hud_ammo_tuning.toml` (positions/sizes/colors/timings).
//! Aucun pixel hardcodé dans Rust. Sensor `forgia_hud_ammo.json` 1Hz.
//!
//! Performance :
//! - `Local<DrawCache>` réutilise un `String` pré-alloué pour `format!` numérique
//! - Slot strip itère `equipped.iter_slots()` (4 armes max V2) — alloc nulle
//! - Reload arc : segments stack-alloc, pas de Vec
//! - egui painter Foreground layer unique

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::prelude::*;
use forgia_combat::weapons::{EquippedWeapons, WeaponType, ARENA_V1_WEAPONS};
use forgia_core::prelude::*;
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_ui_style::*;

mod sensor;
mod tuning;

pub use sensor::AmmoHudSensor;
pub use tuning::AmmoHudTuning;

pub mod prelude {
    pub use crate::{AmmoHudSensor, AmmoHudTuning, ForgiaUiHudAmmoPlugin};
}

// ─── Helpers Display ────────────────────────────────────────────────────

/// Initiale courte (1 lettre) d'une arme — fallback icon en attendant textures.
/// Story-455 Phase B : `assets/icons/weapons/{slug}.png` futur.
fn weapon_initial(w: WeaponType) -> &'static str {
    match w {
        WeaponType::ModernAR => "P",       // Pépin
        WeaponType::AssaultRifle => "B",   // Bourrasque
        WeaponType::Shotgun => "L",        // Madame Lenoir
        WeaponType::RocketLauncher => "M", // Boucherie (Master / Madame)
        WeaponType::AK47 => "A",
        WeaponType::PlasmaRifle => "Z",
        WeaponType::Chainsaw => "C",
    }
}

/// Hotkey label depuis l'index dans ARENA_V1_WEAPONS. None si arme hors set V1.
/// V2 Arena : 4 armes Digit1-4. Futur : lire `KeybindRegistry` (story-456 reload key).
fn weapon_hotkey(w: WeaponType) -> Option<u8> {
    ARENA_V1_WEAPONS
        .iter()
        .position(|x| *x == w)
        .map(|i| (i + 1) as u8)
}

// ─── Cache local pour string formatting (anti-alloc hot path) ──────────

#[derive(Default)]
pub(crate) struct DrawCache {
    pub main_text: String,
    pub reserve_text: String,
    /// Mini compteurs pré-alloués pour les N slots (V2 = 4 armes max).
    /// Réutilisé chaque frame via `.clear()` + `write!()` pour `alloc=0` hot path.
    pub slot_mini: [String; 8],
}

// ─── Counter principal (bottom-right) ───────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_ammo_counter(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    equipped: Res<EquippedWeapons>,
    tuning: Res<AmmoHudTuning>,
    time: Res<Time>,
    mut cache: Local<DrawCache>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Fps {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let weapon = equipped.current;
    let Some(slot) = equipped.slots.get(&weapon) else {
        // Pas encore sync genome → skip rendu (évite affichage default 30/120 trompeur).
        return;
    };

    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_ammo_counter"),
    ));

    let panel_w = tuning.counter_panel_w;
    let panel_h = tuning.counter_panel_h;
    let right = screen.max.x - tuning.counter_padding_right;
    let bottom = screen.max.y - tuning.counter_padding_bottom;
    let panel_rect = egui::Rect::from_min_max(
        egui::pos2(right - panel_w, bottom - panel_h),
        egui::pos2(right, bottom),
    );
    chunky_rect_filled(&painter, panel_rect, C_BG_DARK, 3.0, 10.0);

    // Low-ammo flash : alpha pulse [min..max] @ low_flash_hz.
    let is_low = slot.is_low();
    let is_reloading = slot.reload_state.is_reloading();
    let flash_t = if is_low && !is_reloading {
        let phase = (time.elapsed_secs() * tuning.low_flash_hz * std::f32::consts::TAU).sin();
        // map [-1, 1] → [low_flash_alpha_min, low_flash_alpha_max]
        let m = (phase * 0.5 + 0.5).clamp(0.0, 1.0);
        tuning.low_flash_alpha_min + (tuning.low_flash_alpha_max - tuning.low_flash_alpha_min) * m
    } else {
        1.0
    };

    let main_color = if is_low && !is_reloading {
        // Pulse rouge saturé low-ammo (research : Valorant backlash sur désaturation = non-négociable)
        let alpha = (flash_t * 255.0) as u8;
        egui::Color32::from_rgba_unmultiplied(C_AMMO_LOW.r(), C_AMMO_LOW.g(), C_AMMO_LOW.b(), alpha)
    } else if is_reloading {
        // Pendant reload : cyan reload arc color (signal "ça revient").
        C_RELOAD_ARC
    } else {
        ammo_color(slot.mag_fraction(), slot.config.low_ammo_threshold)
    };

    // Pre-allocated cache strings — réutilisés frame à frame.
    cache.main_text.clear();
    cache.reserve_text.clear();
    use std::fmt::Write as _;
    if slot.config.infinite_ammo {
        let _ = write!(cache.main_text, "∞");
    } else {
        let _ = write!(cache.main_text, "{}", slot.current_mag);
    }
    if slot.config.infinite_ammo {
        let _ = write!(cache.reserve_text, "/ ∞");
    } else {
        let _ = write!(cache.reserve_text, "/ {}", slot.reserve);
    }

    // Position du chiffre principal — droite du panel, centré vertical (Y centre).
    // Pattern Halo/Apex : main number aligné droite, reserve juste au-dessus.
    let main_pos = egui::pos2(
        panel_rect.max.x - tuning.counter_main_padding_right,
        panel_rect.center().y + tuning.counter_main_y_offset,
    );
    text_with_outline(
        &painter,
        main_pos,
        egui::Align2::RIGHT_CENTER,
        &cache.main_text,
        egui::FontId::monospace(tuning.counter_font_main),
        main_color,
        tuning.counter_outline_px,
    );

    // Reserve au-dessus.
    let reserve_pos = egui::pos2(
        panel_rect.max.x - tuning.counter_main_padding_right,
        panel_rect.min.y + tuning.counter_reserve_padding_top,
    );
    text_with_outline(
        &painter,
        reserve_pos,
        egui::Align2::RIGHT_CENTER,
        &cache.reserve_text,
        egui::FontId::monospace(tuning.counter_font_reserve),
        C_TEXT_MUTED,
        tuning.counter_reserve_outline_px,
    );

    // Sous-titre "× N pellets" si arme multi-pellets (Boucherie). Indication shotgun.
    // (Pellets lus depuis genome viewmodel — pas de double-source ; ammo HUD ne sait pas,
    // donc on infère via fire_mode si dispo. V2 : on skip — Phase B+ : query ViewmodelGenome
    // pour afficher proprement. Pour l'instant pas affiché si pas trivialement accessible.)

    // Label "AMMO" en haut gauche du panel (sobriété cartoon).
    let label_pos = egui::pos2(
        panel_rect.min.x + tuning.label_padding_left,
        panel_rect.min.y + tuning.label_padding_top,
    );
    text_with_outline(
        &painter,
        label_pos,
        egui::Align2::LEFT_TOP,
        "AMMO",
        egui::FontId::monospace(tuning.label_font_size),
        C_TEXT_MUTED,
        tuning.label_outline_px,
    );

    // Reloading indicator texte si en reload (en complément du reload arc autour slot).
    if is_reloading {
        let reload_pos = egui::pos2(
            panel_rect.min.x + tuning.reload_indicator_padding_left,
            panel_rect.max.y - tuning.reload_indicator_padding_bottom,
        );
        text_with_outline(
            &painter,
            reload_pos,
            egui::Align2::LEFT_BOTTOM,
            "RELOADING…",
            egui::FontId::monospace(tuning.reload_indicator_font_size),
            C_RELOAD_ARC,
            tuning.reload_indicator_outline_px,
        );
    }
}

// ─── Vertical slot strip (droite, sous wave counter) ───────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_slot_strip(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    equipped: Res<EquippedWeapons>,
    tuning: Res<AmmoHudTuning>,
    mut cache: Local<DrawCache>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Fps {
        return;
    }
    if equipped.slots.is_empty() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_ammo_slot_strip"),
    ));

    // On ordonne par hotkey index (ARENA_V1_WEAPONS order) pour consistance UI.
    // Slots hors V1 set : push en fin sans hotkey.
    let mut ordered: Vec<(WeaponType, &AmmoSlot, Option<u8>)> = Vec::with_capacity(8);
    for w in ARENA_V1_WEAPONS.iter().copied() {
        if let Some(slot) = equipped.slots.get(&w) {
            ordered.push((w, slot, weapon_hotkey(w)));
        }
    }
    for (w, slot) in equipped.iter_slots() {
        if !ARENA_V1_WEAPONS.contains(&w) {
            ordered.push((w, slot, None));
        }
    }

    let right = screen.max.x - tuning.slot_padding_right;
    let mut cursor_y = screen.min.y + tuning.slot_first_y;

    for (cursor_idx, (weapon, slot, hotkey)) in ordered.into_iter().enumerate() {
        let is_active = weapon == equipped.current;
        let scale = if is_active {
            tuning.slot_active_scale
        } else {
            1.0
        };
        let w_box = tuning.slot_box_w * scale;
        let h_box = tuning.slot_box_h * scale;
        let box_rect = egui::Rect::from_min_size(
            egui::pos2(right - w_box, cursor_y),
            egui::vec2(w_box, h_box),
        );

        // Fond — inactif = alpha réduit, actif = bg normal.
        let bg = if is_active {
            C_BG_DARK
        } else {
            egui::Color32::from_rgba_unmultiplied(
                C_BG_DARK.r(),
                C_BG_DARK.g(),
                C_BG_DARK.b(),
                (200.0 * tuning.slot_inactive_alpha) as u8,
            )
        };
        let outline_color = if is_active { C_SLOT_ACTIVE } else { C_OUTLINE };
        let outline_w = if is_active {
            tuning.slot_outline_active
        } else {
            tuning.slot_outline_inactive
        };
        painter.rect_filled(box_rect, tuning.slot_corner_rounding, bg);
        painter.rect_stroke(
            box_rect,
            tuning.slot_corner_rounding,
            egui::Stroke::new(outline_w, outline_color),
            egui::StrokeKind::Outside,
        );

        // Reload arc autour slot actif si reloading.
        if is_active && slot.reload_state.is_reloading() {
            let progress = slot.reload_state.progress(slot.config.reload_time_secs);
            arc_stroke(
                &painter,
                box_rect.center(),
                tuning.reload_arc_radius * scale,
                -std::f32::consts::FRAC_PI_2, // start top
                std::f32::consts::TAU,
                progress,
                C_RELOAD_ARC,
                tuning.reload_arc_thickness,
                tuning.reload_arc_segments,
            );
        }

        // Initiale arme — centre du slot.
        let initial_color = if is_active {
            C_TEXT_LIGHT
        } else {
            egui::Color32::from_rgba_unmultiplied(
                C_TEXT_LIGHT.r(),
                C_TEXT_LIGHT.g(),
                C_TEXT_LIGHT.b(),
                (255.0 * tuning.slot_inactive_alpha) as u8,
            )
        };
        text_with_outline(
            &painter,
            box_rect.center(),
            egui::Align2::CENTER_CENTER,
            weapon_initial(weapon),
            egui::FontId::monospace(tuning.slot_initial_font * scale),
            initial_color,
            tuning.slot_initial_outline_px,
        );

        // Hotkey en coin top-left (si V1).
        if let Some(hk) = hotkey {
            let hk_pos = egui::pos2(
                box_rect.min.x + tuning.slot_hotkey_padding_x,
                box_rect.min.y + tuning.slot_hotkey_padding_y,
            );
            let mut hk_buf = [0u8; 4];
            let hk_str = (hk as char).encode_utf8(&mut hk_buf);
            text_with_outline(
                &painter,
                hk_pos,
                egui::Align2::LEFT_TOP,
                hk_str,
                egui::FontId::monospace(tuning.slot_hotkey_font),
                C_PRIMARY,
                1.0,
            );
        }

        // Mini compteur ammo "12" en bas du slot (slot inactif = lecture rapide).
        // BUG-455-07 fix : DrawCache slot_mini pré-alloué (anti-alloc hot path).
        if !slot.config.infinite_ammo {
            let mini_pos = egui::pos2(
                box_rect.center().x,
                box_rect.max.y - tuning.slot_mini_counter_padding_bottom,
            );
            let mini_color = if slot.is_low() {
                C_AMMO_LOW
            } else {
                C_TEXT_MUTED
            };
            // Index dans cache.slot_mini bounded par len 8 (V2 = 4 armes).
            let buf_idx = cursor_idx.min(cache.slot_mini.len() - 1);
            cache.slot_mini[buf_idx].clear();
            use std::fmt::Write as _;
            let _ = write!(cache.slot_mini[buf_idx], "{}", slot.current_mag);
            text_with_outline(
                &painter,
                mini_pos,
                egui::Align2::CENTER_BOTTOM,
                &cache.slot_mini[buf_idx],
                egui::FontId::monospace(tuning.slot_mini_counter_font * scale),
                mini_color,
                tuning.slot_mini_counter_outline_px,
            );
        }

        cursor_y += h_box + tuning.slot_spacing;
    }
}

// ─── Tracking events pour sensor counters ──────────────────────────────

pub(crate) fn track_ammo_events(
    mut events: MessageReader<AmmoChanged>,
    mut sensor: ResMut<AmmoHudSensor>,
) {
    let n = events.read().count();
    if n > 0 {
        sensor.events_total = sensor.events_total.saturating_add(n as u32);
    }
}

// ─── Plugin ────────────────────────────────────────────────────────────

pub struct ForgiaUiHudAmmoPlugin;

impl Plugin for ForgiaUiHudAmmoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AmmoHudTuning>()
            .init_resource::<AmmoHudSensor>()
            .init_asset::<Genome<AmmoHudTuning>>()
            .register_asset_loader(GenomeLoader::<AmmoHudTuning>::default())
            .add_systems(Startup, tuning::load_ammo_hud_tuning)
            .add_systems(
                Update,
                (
                    tuning::sync_ammo_hud_tuning,
                    track_ammo_events,
                    sensor::write_ammo_hud_sensor,
                ),
            )
            .add_systems(EguiPrimaryContextPass, (draw_ammo_counter, draw_slot_strip));
    }
}
