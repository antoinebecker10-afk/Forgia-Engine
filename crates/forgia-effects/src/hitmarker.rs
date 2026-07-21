//! # forgia-hitmarker
//!
//! Hit confirmation visual (4 segments diagonaux blancs fade 220ms autour crosshair)
//! déclenché par `CombatHitEvent`.
//!
//! Extrait de `forgia-ui` 2026-05-16 (règle `fine-grained-crates.md`).
//!
//! Pattern V1 : hud_hitmarker_duration_ms = 220ms.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::prelude::*;
use forgia_core::prelude::*;

pub mod prelude {
    pub use super::{ForgiaHitmarkerPlugin, HitmarkerState};
}

const HITMARKER_DURATION: f32 = 0.22;
// Story-528 phase 1 AC5 — cartoon hit feedback.
const CARTOON_STAR_DURATION: f32 = 0.20;
const FLASH_DURATION: f32 = 0.08;
// Story-528 phase 2 AC6 — headshot POW!.
const POW_DURATION: f32 = 0.45;

/// HitmarkerState — Resource pour fade visual après hit confirmé.
#[derive(Resource, Default)]
pub struct HitmarkerState {
    /// Time remaining (s). >0 = visible, fade linéaire.
    pub time_left: f32,
    /// Story-528 AC5 — étoiles cartoon ✨ fade 200ms.
    pub cartoon_star_time_left: f32,
    /// Story-528 AC5 — flash blanc plein écran 80ms.
    pub flash_time_left: f32,
    /// Story-528 AC6 — POW! text headshot 450ms (pop scale).
    pub pow_time_left: f32,
}

pub struct ForgiaHitmarkerPlugin;

impl Plugin for ForgiaHitmarkerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HitmarkerState>()
            .add_systems(
                EguiPrimaryContextPass,
                // story-659 — `draw_pow_text` RETIRÉ (demande user 2026-07-03 :
                // « les textes POW font trop avec les effets visuels ») : le
                // chime weakspot (story-651) + le tink visuel suffisent.
                (draw_hitmarker, draw_cartoon_stars, draw_white_flash),
            )
            .add_systems(Update, hitmarker_trigger.in_set(GameSet::UI));
    }
}

/// Lit `CombatHitEvent` → reset `HitmarkerState.time_left` à HITMARKER_DURATION.
/// Story-528 AC5 : reset aussi étoiles cartoon + flash blanc + incrémente
/// `FpsFeelMetrics.hit_feedbacks_total` (sensor forgia2_fps_feel.json).
fn hitmarker_trigger(
    mut hits: MessageReader<CombatHitEvent>,
    mut state: ResMut<HitmarkerState>,
    mut metrics: ResMut<FpsFeelMetrics>,
    time: Res<Time>,
) {
    let mut count = 0u64;
    let mut hs_count = 0u64;
    for ev in hits.read() {
        count += 1;
        if ev.is_headshot {
            hs_count += 1;
        }
    }
    if count > 0 {
        state.time_left = HITMARKER_DURATION;
        state.cartoon_star_time_left = CARTOON_STAR_DURATION;
        state.flash_time_left = FLASH_DURATION;
        metrics.hit_feedbacks_total = metrics.hit_feedbacks_total.saturating_add(count);
        if hs_count > 0 {
            // Story-528 AC6 — POW! pop + ding placeholder log (audio TBD).
            state.pow_time_left = POW_DURATION;
            // Lot A perf tir (audit 2026-07-20) : per-headshot → debug!.
            debug!("[forgia-effects::hitmarker] DING — POW! headshot ({hs_count})");
        }
    } else {
        let dt = time.delta_secs();
        if state.time_left > 0.0 {
            state.time_left = (state.time_left - dt).max(0.0);
        }
        if state.cartoon_star_time_left > 0.0 {
            state.cartoon_star_time_left = (state.cartoon_star_time_left - dt).max(0.0);
        }
        if state.flash_time_left > 0.0 {
            state.flash_time_left = (state.flash_time_left - dt).max(0.0);
        }
        if state.pow_time_left > 0.0 {
            state.pow_time_left = (state.pow_time_left - dt).max(0.0);
        }
    }
}

/// Dessine 4 segments diagonaux blancs autour du crosshair quand HitmarkerState actif.
fn draw_hitmarker(
    mut contexts: EguiContexts,
    state: Res<HitmarkerState>,
    app_state: Res<State<AppMode>>,
) {
    if *app_state.get() != AppMode::InGame || state.time_left <= 0.0 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let center = ctx.content_rect().center();
    let alpha_pct = state.time_left / HITMARKER_DURATION;
    let alpha = (alpha_pct * 220.0) as u8;
    let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("forgia_hitmarker"),
    ));
    let inner = 7.0;
    let outer = 14.0;
    let stroke = egui::Stroke::new(2.5, color);
    for &(dx, dy) in &[(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        painter.line_segment(
            [
                egui::pos2(center.x + dx * inner, center.y + dy * inner),
                egui::pos2(center.x + dx * outer, center.y + dy * outer),
            ],
            stroke,
        );
    }
}

/// Story-528 AC5 — dessine 4 étoiles ✨ jaune cartoon autour du crosshair,
/// fade linéaire CARTOON_STAR_DURATION.
fn draw_cartoon_stars(
    mut contexts: EguiContexts,
    state: Res<HitmarkerState>,
    app_state: Res<State<AppMode>>,
) {
    if *app_state.get() != AppMode::InGame || state.cartoon_star_time_left <= 0.0 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let center = ctx.content_rect().center();
    let alpha_pct = state.cartoon_star_time_left / CARTOON_STAR_DURATION;
    let alpha = (alpha_pct * 230.0) as u8;
    let yellow = egui::Color32::from_rgba_unmultiplied(255, 220, 60, alpha);
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_cartoon_stars"),
    ));
    // 4 étoiles aux 4 cardinaux autour du crosshair, distance 28 px.
    let radius = 28.0;
    let size_pct = 0.6 + 0.4 * alpha_pct; // shrink en fade.
    let positions = [
        (0.0, -radius),
        (0.0, radius),
        (-radius, 0.0),
        (radius, 0.0),
    ];
    for (dx, dy) in positions {
        draw_star_4_branches(&painter, center.x + dx, center.y + dy, 9.0 * size_pct, yellow);
    }
}

/// Dessine une étoile à 4 branches (✨) via 4 lignes diagonales croisées + cœur jaune.
fn draw_star_4_branches(
    painter: &egui::Painter,
    cx: f32,
    cy: f32,
    size: f32,
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(2.0, color);
    // Branches cardinales (croix +).
    painter.line_segment(
        [egui::pos2(cx, cy - size), egui::pos2(cx, cy + size)],
        stroke,
    );
    painter.line_segment(
        [egui::pos2(cx - size, cy), egui::pos2(cx + size, cy)],
        stroke,
    );
    // Branches diagonales courtes (sparkle).
    let half = size * 0.5;
    painter.line_segment(
        [
            egui::pos2(cx - half, cy - half),
            egui::pos2(cx + half, cy + half),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(cx - half, cy + half),
            egui::pos2(cx + half, cy - half),
        ],
        stroke,
    );
}

// story-659 — `draw_pow_text` (texte cartoon « POW! » headshot, story-528 AC6)
// SUPPRIMÉ à la demande user 2026-07-03 : redondant avec les VFX élémentaires +
// le chime weakspot (story-651). L'état `pow_time_left` reste tické (inoffensif)
// pour ne pas toucher la struct partagée.

/// Story-528 AC5 — flash blanc plein écran 80ms (overlay semi-transparent).
fn draw_white_flash(
    mut contexts: EguiContexts,
    state: Res<HitmarkerState>,
    app_state: Res<State<AppMode>>,
) {
    if *app_state.get() != AppMode::InGame || state.flash_time_left <= 0.0 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let alpha_pct = state.flash_time_left / FLASH_DURATION;
    // Pic 35 alpha, fade rapide.
    let alpha = (alpha_pct * 35.0) as u8;
    let white = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("forgia_hit_flash"),
    ));
    painter.rect_filled(rect, 0.0, white);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaHitmarkerPlugin;
    }

    #[test]
    fn hitmarker_default_invisible() {
        let s = HitmarkerState::default();
        assert_eq!(s.time_left, 0.0);
    }
}
