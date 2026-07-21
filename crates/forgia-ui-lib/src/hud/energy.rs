//! Energy overlay Roguelite — Story-528 AC4 (rename HP→Énergie + cœurs cartoon).
//!
//! Pattern : overlay non-destructif au-dessus de la HP bar `player_hp.rs`.
//! - Couvre le label "HP" par "ÉNERGIE" (texte foreground même position).
//! - Ajoute 3 cœurs ♥ cartoon à gauche du label (épuisement = cœurs vides).
//! - Fade warm-orange plein écran quand Énergie < 30% (tone fatigué bible v1).
//! - Voiceline placeholder Maître Forgeron à l'épuisement (HP = 0).
//!
//! Anti-canon respecté : aucun mot "die/death/blood/HP", uniquement "Énergie / Repos".
//! Cf `.claude/rules/no-speculative-fix.md` — `player_hp.rs` n'est PAS modifié.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_damage::Health as DamageHealth;
use forgia_player::Player;

use crate::style::*;

const ENERGY_LOW_THRESHOLD: f32 = 0.30;
const HEART_COUNT: usize = 3;

/// Track edge HP > 0 → HP == 0 pour émettre la voiceline une seule fois.
#[derive(Resource, Default)]
pub(crate) struct EnergyExhaustionLatch {
    pub already_exhausted: bool,
}

pub(crate) fn draw_energy_label_overlay(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    q_player: Query<&DamageHealth, With<Player>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let Ok(health) = q_player.single() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_energy_overlay"),
    ));

    // Position miroir player_hp.rs : bar bottom-left, pad 24, bar_h 32.
    let bar_h = 32.0;
    let pad = 24.0;
    let bottom_y = screen.max.y - pad - bar_h;
    let left_x = screen.min.x + pad;
    let bar_center_y = bottom_y + bar_h * 0.5;

    // 1. Cover label "HP" original (positionné left_x - 14, center_y, RIGHT_CENTER).
    //    On dessine un rect opaque sombre par-dessus puis "ÉNERGIE".
    let cover_rect = egui::Rect::from_min_max(
        egui::pos2(left_x - 130.0, bottom_y - 4.0),
        egui::pos2(left_x - 4.0, bottom_y + bar_h + 4.0),
    );
    painter.rect_filled(cover_rect, 4.0, C_BG_DARK);

    // 2. Label "ÉNERGIE" + cœurs au-dessus du bar (au-dessus du cover).
    let frac = health.fraction();
    text_with_outline(
        &painter,
        egui::pos2(left_x - 14.0, bar_center_y),
        egui::Align2::RIGHT_CENTER,
        "ÉNERGIE",
        egui::FontId::proportional(15.0),
        C_PRIMARY,
        1.5,
    );

    // 3. Cœurs cartoon ♥ au-dessus du bar (HEART_COUNT, plein si frac > i/HEART_COUNT).
    let hearts_y = bottom_y - 14.0;
    let heart_size = 11.0;
    let heart_spacing = 22.0;
    for i in 0..HEART_COUNT {
        let cx = left_x + heart_spacing * 0.5 + i as f32 * heart_spacing;
        let threshold = (i + 1) as f32 / HEART_COUNT as f32;
        let filled = frac >= threshold - 1.0 / (HEART_COUNT as f32 * 2.0);
        let color = if filled { C_HP_LOW } else { C_TEXT_MUTED };
        draw_cartoon_heart(&painter, cx, hearts_y, heart_size, color);
    }
}

/// Overlay warm-orange semi-transparent quand énergie < 30%. Pulse léger pour
/// signaler la fatigue sans masquer le gameplay.
pub(crate) fn draw_warm_orange_fade(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    time: Res<Time>,
    q_player: Query<&DamageHealth, With<Player>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let Ok(health) = q_player.single() else {
        return;
    };
    let frac = health.fraction();
    if frac > ENERGY_LOW_THRESHOLD {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    // Pulse alpha 30..55 sinusoidale 1.2Hz. Plus opaque quand frac bas.
    let intensity = 1.0 - (frac / ENERGY_LOW_THRESHOLD).clamp(0.0, 1.0);
    let pulse = 0.5 + 0.5 * (time.elapsed_secs() * std::f32::consts::TAU * 1.2).sin();
    let alpha = (intensity * (35.0 + 25.0 * pulse)) as u8;
    let warm = egui::Color32::from_rgba_unmultiplied(255, 107, 53, alpha);
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("forgia_energy_warm_fade"),
    ));
    painter.rect_filled(rect, 0.0, warm);
}

/// Système edge-trigger : émet voiceline Maître Forgeron à l'épuisement (HP=0).
/// Reset latch quand HP > 0 (respawn / retour hub).
pub(crate) fn sys_energy_exhaustion_voiceline(
    mut latch: ResMut<EnergyExhaustionLatch>,
    game_mode: Res<State<GameMode>>,
    q_player: Query<&DamageHealth, With<Player>>,
) {
    if *game_mode.get() != GameMode::Roguelite {
        latch.already_exhausted = false;
        return;
    }
    let Ok(health) = q_player.single() else {
        latch.already_exhausted = false;
        return;
    };
    let exhausted = health.current <= 0.0;
    if exhausted && !latch.already_exhausted {
        latch.already_exhausted = true;
        // Placeholder voiceline — audio pipeline TBD (cf ROADMAP_ROGUELITE Tier 3 TTS).
        info!("[forgia-ui-lib::energy] Maître Forgeron: «Repose-toi, Apprenti. La forge te rappellera.»");
    } else if !exhausted {
        latch.already_exhausted = false;
    }
}

/// Cœur cartoon stylisé : 2 demi-disques + triangle inversé. Approximation
/// avec painter.circle_filled + line_segment pour rester simple.
/// Partagé avec la jauge de confiance Pépin (confidence.rs) — les glyphes
/// texte ♥/♡ ne sont pas couverts par les polices egui, on dessine la forme.
pub(crate) fn draw_cartoon_heart(
    painter: &egui::Painter,
    cx: f32,
    cy: f32,
    size: f32,
    color: egui::Color32,
) {
    let r = size * 0.5;
    let outline = egui::Color32::BLACK;
    // 2 demi-cercles côte à côte (en haut).
    let left = egui::pos2(cx - r * 0.55, cy - r * 0.20);
    let right = egui::pos2(cx + r * 0.55, cy - r * 0.20);
    painter.circle_filled(left, r * 0.65, color);
    painter.circle_filled(right, r * 0.65, color);
    // Triangle pointe bas (3 sommets).
    let tri = [
        egui::pos2(cx - r * 1.0, cy - r * 0.05),
        egui::pos2(cx + r * 1.0, cy - r * 0.05),
        egui::pos2(cx, cy + r * 0.95),
    ];
    painter.add(egui::Shape::convex_polygon(
        tri.to_vec(),
        color,
        egui::Stroke::new(1.0, outline),
    ));
}

pub struct EnergyOverlayPlugin;

impl Plugin for EnergyOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnergyExhaustionLatch>()
            .add_systems(
                EguiPrimaryContextPass,
                (draw_energy_label_overlay, draw_warm_orange_fade)
                    .run_if(gameplay_hud_visible),
            )
            .add_systems(Update, sys_energy_exhaustion_voiceline);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latch_default_not_exhausted() {
        let l = EnergyExhaustionLatch::default();
        assert!(!l.already_exhausted);
    }

    #[test]
    fn plugin_constructible() {
        let _p = EnergyOverlayPlugin;
    }

    #[test]
    fn energy_low_threshold_is_30_pct() {
        assert!((ENERGY_LOW_THRESHOLD - 0.30).abs() < 1e-5);
    }

    #[test]
    fn heart_count_is_three() {
        assert_eq!(HEART_COUNT, 3);
    }
}
