//! # forgia-killfeed
//!
//! Story-455 Phase D — Kill feed top-right + multi-kill sweeping banner.
//!
//! Pattern AAA (research §7) :
//! - **Top-right** Overwatch / CS2 / Apex
//! - **Slide-in** lateral droite→gauche (0.15-0.20s)
//! - **FIFO** capé à 5 entries (oldest fade-out)
//! - **Display ~5s** + fade 1s (Overwatch Workshop config + Blizzard forum thread)
//! - **Multi-kill** Halo-style sweeping banner CENTER_TOP ("DOUBLE KILL" / "TRIPLE KILL" / "RAMPAGE")
//!
//! Event-driven : consume `CombatHitEvent` (Phase C étendu avec attacker/weapon).
//! Genome-driven : `assets/genomes/killfeed_tuning.toml`. Sensor `forgia_killfeed.json`.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::prelude::*;
use forgia_combat::weapons::WeaponType;
use forgia_core::prelude::*;
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_player::Player;
use forgia_ui_style::*;
use std::collections::VecDeque;
use std::fs;

mod tuning;

pub use tuning::KillfeedTuning;

pub mod prelude {
    pub use crate::{ForgiaKillfeedPlugin, KillfeedTuning};
}

// ─── State ─────────────────────────────────────────────────────────────

/// Une entry du kill feed.
#[derive(Debug, Clone)]
pub struct KillFeedEntry {
    pub attacker_label: String,
    pub victim_label: String,
    pub weapon: Option<WeaponType>,
    pub is_headshot: bool,
    pub attacker_is_player: bool,
    /// Temps d'apparition (elapsed_secs).
    pub spawned_at: f32,
}

#[derive(Resource, Default)]
pub struct KillFeedState {
    pub entries: VecDeque<KillFeedEntry>,
}

#[derive(Resource, Default)]
pub struct KillStreakState {
    /// Compteur de kills consécutifs sous `multikill_window_secs`.
    pub streak: u32,
    pub last_kill_at: f32,
    /// Banner courant à afficher.
    pub banner_text: Option<String>,
    pub banner_started_at: f32,
}

#[derive(Resource, Default)]
pub struct KillfeedSensor {
    pub last_write_secs: f32,
    pub total_kills_session: u32,
    pub last_attacker_label: String,
    pub last_victim_label: String,
}

// ─── Helpers ───────────────────────────────────────────────────────────

/// Label affiché pour une entité. Player → "You", Bot → Name component ou "Bot".
/// World damage (attacker None) → "World".
fn entity_label(
    entity: Option<Entity>,
    q_name: &Query<&Name>,
    q_player_marker: &Query<(), With<Player>>,
) -> String {
    let Some(e) = entity else {
        return "World".to_string();
    };
    if q_player_marker.get(e).is_ok() {
        return "You".to_string();
    }
    q_name
        .get(e)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|_| "Bot".to_string())
}

fn weapon_short(w: WeaponType) -> &'static str {
    match w {
        WeaponType::ModernAR => "P",
        WeaponType::AssaultRifle => "B",
        WeaponType::Shotgun => "L",
        WeaponType::RocketLauncher => "M",
        WeaponType::AK47 => "A",
        WeaponType::PlasmaRifle => "Z",
        WeaponType::Chainsaw => "C",
    }
}

fn streak_label(streak: u32) -> Option<&'static str> {
    match streak {
        2 => Some("DOUBLE KILL"),
        3 => Some("TRIPLE KILL"),
        4 => Some("QUAD KILL"),
        5..=u32::MAX => Some("RAMPAGE"),
        _ => None,
    }
}

// ─── Ingest events ─────────────────────────────────────────────────────

pub(crate) fn ingest_kill_events(
    mut events: MessageReader<CombatHitEvent>,
    mut feed: ResMut<KillFeedState>,
    mut streak: ResMut<KillStreakState>,
    mut sensor: ResMut<KillfeedSensor>,
    tuning: Res<KillfeedTuning>,
    time: Res<Time>,
    q_name: Query<&Name>,
    q_player_marker: Query<(), With<Player>>,
) {
    let now = time.elapsed_secs();
    for hit in events.read() {
        if !hit.is_kill {
            continue;
        }
        let attacker_label = entity_label(hit.attacker, &q_name, &q_player_marker);
        let victim_label = entity_label(Some(hit.target), &q_name, &q_player_marker);
        let attacker_is_player = hit
            .attacker
            .map(|e| q_player_marker.get(e).is_ok())
            .unwrap_or(false);
        let entry = KillFeedEntry {
            attacker_label: attacker_label.clone(),
            victim_label: victim_label.clone(),
            weapon: hit.weapon,
            is_headshot: hit.is_headshot,
            attacker_is_player,
            spawned_at: now,
        };
        if feed.entries.len() >= tuning.max_entries.max(1) {
            feed.entries.pop_front();
        }
        feed.entries.push_back(entry);

        // BUG-455-05 fix : streak.last_kill_at update SEULEMENT pour les kills du player.
        // Sinon un kill bot-vs-bot intercale dans la fenêtre player et casse le streak counter.
        if attacker_is_player {
            if now - streak.last_kill_at <= tuning.multikill_window_secs {
                streak.streak += 1;
            } else {
                streak.streak = 1;
            }
            streak.last_kill_at = now;
        }
        if attacker_is_player && streak.streak >= 2 {
            if let Some(label) = streak_label(streak.streak) {
                streak.banner_text = Some(label.to_string());
                streak.banner_started_at = now;
            }
        }

        sensor.total_kills_session = sensor.total_kills_session.saturating_add(1);
        sensor.last_attacker_label = attacker_label;
        sensor.last_victim_label = victim_label;
    }
}

// ─── Tick entries (fade + cleanup) ─────────────────────────────────────

pub(crate) fn tick_killfeed(
    time: Res<Time>,
    mut feed: ResMut<KillFeedState>,
    mut streak: ResMut<KillStreakState>,
    tuning: Res<KillfeedTuning>,
) {
    let now = time.elapsed_secs();
    // Cleanup entries trop vieilles.
    feed.entries
        .retain(|e| now - e.spawned_at < tuning.display_secs);
    // Clear banner expiré.
    if streak.banner_text.is_some() && now - streak.banner_started_at > tuning.multikill_banner_secs
    {
        streak.banner_text = None;
    }
    // Reset streak si fenêtre dépassée (au-delà de window, le prochain kill repart à 1).
    if now - streak.last_kill_at > tuning.multikill_window_secs {
        streak.streak = 0;
    }
}

// ─── Draw feed ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_killfeed(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    feed: Res<KillFeedState>,
    tuning: Res<KillfeedTuning>,
    time: Res<Time>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Fps {
        return;
    }
    if feed.entries.is_empty() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_killfeed"),
    ));
    let now = time.elapsed_secs();
    let right = screen.max.x - tuning.padding_right;
    let mut cursor_y = screen.min.y + tuning.first_entry_y;
    let fade_start = tuning.display_secs - tuning.fade_out_secs;

    for entry in &feed.entries {
        let age = now - entry.spawned_at;
        // Slide-in : entrée glisse de droite → position finale durant slide_in_secs.
        let slide_t = (age / tuning.slide_in_secs).clamp(0.0, 1.0);
        let slide_offset = (1.0 - slide_t) * tuning.slide_in_distance;
        // Fade-out : alpha lerp 1.0 → 0.0 durant fade_out_secs final.
        let alpha = if age > fade_start {
            ((tuning.display_secs - age) / tuning.fade_out_secs).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let alpha_u8 = (alpha * 255.0) as u8;
        let entry_rect = egui::Rect::from_min_size(
            egui::pos2(right - tuning.entry_width + slide_offset, cursor_y),
            egui::vec2(tuning.entry_width, tuning.entry_height),
        );
        let bg = egui::Color32::from_rgba_unmultiplied(
            C_BG_DARK.r(),
            C_BG_DARK.g(),
            C_BG_DARK.b(),
            (200.0 * alpha) as u8,
        );
        chunky_rect_filled(
            &painter,
            entry_rect,
            bg,
            tuning.entry_outline_px,
            tuning.entry_corner_rounding,
        );

        // Couleurs attacker / victim selon team (player vs bot).
        let attacker_color = if entry.attacker_is_player {
            with_alpha(C_HP_HIGH, alpha_u8)
        } else {
            with_alpha(C_HP_LOW, alpha_u8)
        };
        let victim_color = if entry.attacker_is_player {
            with_alpha(C_HP_LOW, alpha_u8)
        } else {
            with_alpha(C_HP_HIGH, alpha_u8)
        };

        // Layout horizontal : attacker | weapon_icon | → | victim | [★]
        let pad = tuning.entry_pad_inner;
        let center_y = entry_rect.center().y;
        text_with_outline(
            &painter,
            egui::pos2(entry_rect.min.x + pad, center_y),
            egui::Align2::LEFT_CENTER,
            &entry.attacker_label,
            egui::FontId::monospace(tuning.font_attacker),
            attacker_color,
            tuning.text_outline_px,
        );

        // Weapon icon (initiale fallback en attendant atlas).
        let icon_x = entry_rect.min.x + pad + tuning.icon_offset_x;
        if let Some(w) = entry.weapon {
            text_with_outline(
                &painter,
                egui::pos2(icon_x, center_y),
                egui::Align2::CENTER_CENTER,
                weapon_short(w),
                egui::FontId::monospace(tuning.font_weapon_icon),
                with_alpha(C_PRIMARY, alpha_u8),
                1.5,
            );
        }

        // Arrow.
        text_with_outline(
            &painter,
            egui::pos2(icon_x + tuning.arrow_offset_x_from_icon, center_y),
            egui::Align2::CENTER_CENTER,
            ">",
            egui::FontId::monospace(tuning.font_arrow),
            with_alpha(C_TEXT_LIGHT, alpha_u8),
            1.0,
        );

        // Victim.
        text_with_outline(
            &painter,
            egui::pos2(icon_x + tuning.victim_offset_x_from_icon, center_y),
            egui::Align2::LEFT_CENTER,
            &entry.victim_label,
            egui::FontId::monospace(tuning.font_victim),
            victim_color,
            tuning.text_outline_px,
        );

        // Headshot star (yellow).
        if entry.is_headshot {
            text_with_outline(
                &painter,
                egui::pos2(entry_rect.max.x - pad, center_y),
                egui::Align2::RIGHT_CENTER,
                "*",
                egui::FontId::monospace(tuning.font_attacker + tuning.headshot_font_bonus),
                with_alpha(C_AMMO_TEXT, alpha_u8),
                1.5,
            );
        }

        cursor_y += tuning.entry_height + tuning.entry_spacing;
    }
}

// ─── Draw multi-kill banner ────────────────────────────────────────────

pub(crate) fn draw_multikill_banner(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    streak: Res<KillStreakState>,
    tuning: Res<KillfeedTuning>,
    time: Res<Time>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Fps {
        return;
    }
    let Some(banner) = &streak.banner_text else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let now = time.elapsed_secs();
    let age = now - streak.banner_started_at;
    if age > tuning.multikill_banner_secs {
        return;
    }
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_multikill_banner"),
    ));
    // Sweep effect : alpha pop initial puis fade.
    let t = age / tuning.multikill_banner_secs;
    let alpha = if t < 0.2 {
        (t / 0.2).clamp(0.0, 1.0)
    } else {
        ((1.0 - t) / 0.8).clamp(0.0, 1.0)
    };
    // Scale pop initial.
    let scale = if t < 0.15 {
        0.6 + 0.4 * (t / 0.15)
    } else {
        1.0
    };
    let center = egui::pos2(screen.center().x, screen.min.y + tuning.multikill_banner_y);
    let color = egui::Color32::from_rgba_unmultiplied(
        C_PRIMARY.r(),
        C_PRIMARY.g(),
        C_PRIMARY.b(),
        (alpha * 255.0) as u8,
    );
    text_with_outline(
        &painter,
        center,
        egui::Align2::CENTER_CENTER,
        banner,
        egui::FontId::monospace(tuning.multikill_font * scale),
        color,
        3.0,
    );
}

// ─── Sensor JSON ───────────────────────────────────────────────────────

pub(crate) fn write_killfeed_sensor(
    time: Res<Time>,
    tuning: Res<KillfeedTuning>,
    feed: Res<KillFeedState>,
    streak: Res<KillStreakState>,
    mut sensor: ResMut<KillfeedSensor>,
) {
    let now = time.elapsed_secs();
    if now - sensor.last_write_secs < tuning.sensor_period_secs.max(0.1) {
        return;
    }
    sensor.last_write_secs = now;
    let banner_str = streak
        .banner_text
        .as_deref()
        .map(|s| format!(r#""{}""#, s.replace('"', "'")))
        .unwrap_or_else(|| "null".to_string());
    let json = format!(
        r#"{{"timestamp_secs":{:.2},"active_entries":{},"max_entries":{},"total_kills_session":{},"streak_current":{},"banner":{},"last_attacker":"{}","last_victim":"{}"}}"#,
        now,
        feed.entries.len(),
        tuning.max_entries,
        sensor.total_kills_session,
        streak.streak,
        banner_str,
        sensor.last_attacker_label.replace('"', "'"),
        sensor.last_victim_label.replace('"', "'"),
    );
    let _ = fs::write("forgia_killfeed.json", json);
}

// ─── Helper alpha ──────────────────────────────────────────────────────

fn with_alpha(c: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

// ─── Plugin ────────────────────────────────────────────────────────────

pub struct ForgiaKillfeedPlugin;

impl Plugin for ForgiaKillfeedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KillFeedState>()
            .init_resource::<KillStreakState>()
            .init_resource::<KillfeedSensor>()
            .init_resource::<KillfeedTuning>()
            .init_asset::<Genome<KillfeedTuning>>()
            .register_asset_loader(GenomeLoader::<KillfeedTuning>::default())
            .add_systems(Startup, tuning::load_killfeed_tuning)
            .add_systems(
                Update,
                (
                    tuning::sync_killfeed_tuning,
                    ingest_kill_events,
                    tick_killfeed,
                    write_killfeed_sensor,
                )
                    .chain(),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (draw_killfeed, draw_multikill_banner),
            );
    }
}
