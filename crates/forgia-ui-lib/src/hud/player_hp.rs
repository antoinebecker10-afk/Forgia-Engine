//! Player HP bar — bas-gauche écran, style OW chunky cartoon.
//!
//! Lit `forgia_damage::Health` sur l'entity tagguée `BotTarget` (player).
//! Vignette rouge fullscreen quand HP < 30%.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_damage::Health as DamageHealth;
use forgia_player::Player;

use crate::style::*;

// Marker `With<Player>` requis : 2026-05-27 bug HP bar disparaît début vague suivante.
// Les ArenaBots (forgia-ai-arena-bot) portent aussi `forgia_damage::Health` → query
// sans filtre = Err(MultipleEntities) → single() fail → bar invisible.
// Pattern miroir : forgia-mode-roguelite/src/run.rs:262 (déjà filtré With<Player>).
pub(crate) fn draw_player_hp(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    q_player: Query<&DamageHealth, With<Player>>,
) {
    if *app_state.get() != AppMode::InGame
        || !matches!(*game_mode.get(), GameMode::Fps | GameMode::Roguelite)
    {
        return;
    }
    let Ok(health) = q_player.single() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_player_hp"),
    ));

    // Story-455 Phase F : low-HP red vignette migré vers `forgia-juice-screen-flash`
    // (rule fine-grained-crates, ScreenFlashTuning genome-driven). Ici on garde
    // uniquement la HP bar UI, la vignette overlay vient d'un autre crate.
    let frac = health.fraction();

    // HP bar dimensions — chunky cartoon
    let bar_w = 380.0;
    let bar_h = 32.0;
    let pad = 24.0;
    let bottom_y = screen.max.y - pad - bar_h;
    let left_x = screen.min.x + pad;

    // Container avec outline + arrondi
    let outer = egui::Rect::from_min_size(
        egui::pos2(left_x - 6.0, bottom_y - 6.0),
        egui::vec2(bar_w + 12.0, bar_h + 12.0),
    );
    chunky_rect_filled(&painter, outer, C_BG_DARK, 3.0, 8.0);

    // Bar background (track noir)
    let bar_rect =
        egui::Rect::from_min_size(egui::pos2(left_x, bottom_y), egui::vec2(bar_w, bar_h));
    painter.rect_filled(
        bar_rect,
        4.0,
        egui::Color32::from_rgba_unmultiplied(20, 22, 28, 240),
    );

    // Fill colored par fraction
    let fill_w = bar_w * frac;
    if fill_w > 0.5 {
        let fill_rect = egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_w, bar_h));
        let fill_color = hp_color(frac);
        painter.rect_filled(fill_rect, 4.0, fill_color);

        // Highlight stripe (top quart, plus clair) — effet bombé cartoon
        let hl_rect = egui::Rect::from_min_size(fill_rect.min, egui::vec2(fill_w, bar_h * 0.35));
        let hl_color = egui::Color32::from_rgba_unmultiplied(
            fill_color.r().saturating_add(40),
            fill_color.g().saturating_add(40),
            fill_color.b().saturating_add(40),
            150,
        );
        painter.rect_filled(hl_rect, 4.0, hl_color);
    }

    // Outline final autour de la bar
    painter.rect_stroke(
        bar_rect,
        4.0,
        egui::Stroke::new(2.0, C_OUTLINE),
        egui::StrokeKind::Outside,
    );

    // HP number "75 / 100" centré sur la bar
    let hp_text = format!(
        "{} / {}",
        health.current.round() as i32,
        health.max.round() as i32
    );
    text_with_outline(
        &painter,
        bar_rect.center(),
        egui::Align2::CENTER_CENTER,
        &hp_text,
        egui::FontId::proportional(18.0),
        C_TEXT_LIGHT,
        1.0,
    );

    // Label "HP" à gauche de la bar
    text_with_outline(
        &painter,
        egui::pos2(left_x - 14.0, bar_rect.center().y),
        egui::Align2::RIGHT_CENTER,
        "HP",
        egui::FontId::monospace(20.0),
        C_PRIMARY,
        1.5,
    );
}

pub struct PlayerHpPlugin;

impl Plugin for PlayerHpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, draw_player_hp);
    }
}
