//! Jauge de confiance Pépin — HUD discret (story-531 AC9, GDD : « HUD discret
//! cœur clignote »).
//!
//! Rangée de cœurs cyan (couleur Pépin, AC8) au-dessus de la barre d'énergie,
//! visible UNIQUEMENT quand Pépin (ModernAR) est en main en Roguelite. Le cœur
//! le plus récent clignote ~0,5 s après chaque changement de jauge.
//! Pattern miroir de `energy.rs` (painter Foreground + text_with_outline).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::confidence::PepinConfidence;
use forgia_combat::weapons::{EquippedWeapons, WeaponType};
use forgia_core::prelude::*;

use crate::style::*;

/// Cyan Pépin (AC8 — couleur dominante de la persona).
const C_PEPIN: egui::Color32 = egui::Color32::from_rgb(80, 220, 255);
const BLINK_SECS: f32 = 0.5;
/// Pépin = ModernAR (mapping slot Digit1, cf forgia-fps::pepin::PEPIN_WEAPON —
/// dupliqué ici car ui-lib ne dépend pas de forgia-fps).
const PEPIN_WEAPON: WeaponType = WeaponType::ModernAR;

pub(crate) fn draw_confidence_hearts(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    equipped: Option<Res<EquippedWeapons>>,
    conf: Option<Res<PepinConfidence>>,
    time: Res<Time>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let Some(equipped) = equipped else { return };
    if equipped.current != PEPIN_WEAPON {
        return;
    }
    let Some(conf) = conf else { return };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_pepin_confidence"),
    ));

    // Au-dessus de la rangée cœurs énergie (energy.rs : hearts_y = bottom-pad-bar_h-14).
    let pad = 24.0;
    let bar_h = 32.0;
    let base_y = screen.max.y - pad - bar_h - 38.0;
    let left_x = screen.min.x + pad;

    // Blink : pulse alpha sur le cœur de tête juste après un changement.
    let since_change = time.elapsed_secs() - conf.last_change_secs;
    let blinking = (0.0..BLINK_SECS).contains(&since_change);
    let pulse = if blinking {
        // 0→1→0 deux fois sur la fenêtre (clignote, GDD).
        ((since_change / BLINK_SECS) * std::f32::consts::TAU * 2.0).sin().abs()
    } else {
        1.0
    };

    let heart_spacing = 16.0;
    let stacks = usize::from(conf.stacks);
    for i in 0..10usize {
        let filled = i < stacks;
        let is_head = i + 1 == stacks;
        let alpha = if filled {
            if is_head && blinking {
                (80.0 + 175.0 * pulse) as u8
            } else {
                255
            }
        } else {
            55 // cœurs vides à peine visibles = « discret »
        };
        let color = egui::Color32::from_rgba_unmultiplied(
            C_PEPIN.r(),
            C_PEPIN.g(),
            C_PEPIN.b(),
            alpha,
        );
        text_with_outline(
            &painter,
            egui::pos2(left_x + i as f32 * heart_spacing, base_y),
            egui::Align2::LEFT_CENTER,
            if filled { "♥" } else { "♡" },
            egui::FontId::proportional(13.0),
            color,
            1.0,
        );
    }
}

pub struct ConfidenceHudPlugin;

impl Plugin for ConfidenceHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, draw_confidence_hearts);
    }
}
