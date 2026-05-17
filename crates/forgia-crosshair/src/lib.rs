//! # forgia-crosshair
//!
//! Dynamic crosshair UI : croix hipfire blanche, red dot ADS, sniper scope fullscreen overlay.
//!
//! Pattern : Resource `CrosshairMode { ads_progress, sniper_fullscreen }` est piloté
//! depuis l'extérieur (e.g. `forgia-fps::ads::update_ads_progress`).
//! 2 systems rendent l'UI via egui :
//! - `draw_crosshair` : croix blanche hipfire / red dot ADS (lerp alpha)
//! - `draw_sniper_scope_overlay` : overlay fullscreen vignette + reticle CoD-style quand sniper
//!
//! Extrait de `forgia-ui` 2026-05-16 (règle `fine-grained-crates.md`).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;

pub mod prelude {
    pub use crate::{CrosshairMode, CrosshairTuning, ForgiaCrosshairPlugin};
}

/// Resource Tuning hot-reload pour TOUS les paramètres de rendu crosshair
/// (hipfire / ADS dot / sniper overlay). Push depuis fps_tuning.toml par forgia-fps.
/// Defaults = valeurs précédentes hardcodées (rétro-compat).
#[derive(Resource, Debug, Clone, Copy)]
pub struct CrosshairTuning {
    // Hipfire crosshair
    pub hipfire_cross_len: f32,
    pub hipfire_cross_stroke: f32,
    pub hipfire_cross_alpha: u8,
    // ADS red dot
    pub ads_dot_outer_radius: f32,
    pub ads_dot_inner_radius: f32,
    // Sniper overlay
    pub sniper_scope_radius_factor: f32,
    pub sniper_dim_alpha: u8,
    pub sniper_dim_color: [u8; 3],
    pub sniper_vignette_rings: u32,
    pub sniper_ring_thickness_factor: f32,
    pub sniper_ring_max_alpha: f32,
    pub sniper_ring_color: [u8; 3],
    pub sniper_ring_outer_extent: f32,
    pub sniper_border_width: f32,
    pub sniper_border_inner_width: f32,
    pub sniper_border_inner_offset: f32,
    pub sniper_reticle_gap: f32,
    pub sniper_reticle_line_factor: f32,
    pub sniper_reticle_color: [u8; 3],
    pub sniper_reticle_line_stroke: f32,
    pub sniper_reticle_tick_stroke: f32,
    pub sniper_reticle_tick_size: f32,
    pub sniper_red_dot_radius: f32,
    pub sniper_red_dot_color: [u8; 3],
}

impl Default for CrosshairTuning {
    fn default() -> Self {
        Self {
            hipfire_cross_len: 7.0,
            hipfire_cross_stroke: 2.0,
            hipfire_cross_alpha: 220,
            ads_dot_outer_radius: 4.0,
            ads_dot_inner_radius: 2.0,
            sniper_scope_radius_factor: 0.42,
            sniper_dim_alpha: 145,
            sniper_dim_color: [8, 8, 12],
            sniper_vignette_rings: 8,
            sniper_ring_thickness_factor: 0.18,
            sniper_ring_max_alpha: 90.0,
            sniper_ring_color: [15, 15, 18],
            sniper_ring_outer_extent: 1.5,
            sniper_border_width: 8.0,
            sniper_border_inner_width: 2.0,
            sniper_border_inner_offset: 4.0,
            sniper_reticle_gap: 8.0,
            sniper_reticle_line_factor: 0.85,
            sniper_reticle_color: [10, 10, 10],
            sniper_reticle_line_stroke: 1.5,
            sniper_reticle_tick_stroke: 1.0,
            sniper_reticle_tick_size: 4.0,
            sniper_red_dot_radius: 2.0,
            sniper_red_dot_color: [255, 30, 30],
        }
    }
}

/// Mode du crosshair : hipfire (croix blanche) ou ADS (red dot précis).
/// Written depuis l'extérieur (e.g. forgia-fps::ads::update_ads_progress).
#[derive(Resource, Default)]
pub struct CrosshairMode {
    /// 0.0 = hipfire complet, 1.0 = full ADS. Lerp continu.
    pub ads_progress: f32,
    /// Si true ET ads_progress > 0.5 → overlay sniper scope fullscreen :
    /// vignette semi-transparente (vision périphérique préservée) + gradient
    /// 8 anneaux concentriques fake-blur + reticle CoD/ACOG-style.
    pub sniper_fullscreen: bool,
}

pub struct ForgiaCrosshairPlugin;

impl Plugin for ForgiaCrosshairPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CrosshairMode>()
            .init_resource::<CrosshairTuning>()
            .add_systems(
                EguiPrimaryContextPass,
                (draw_crosshair, draw_sniper_scope_overlay),
            );
    }
}

fn draw_crosshair(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mode: Res<CrosshairMode>,
    tuning: Res<CrosshairTuning>,
) {
    if *app_state.get() != AppMode::InGame {
        return;
    }
    // En sniper ADS fullscreen, le crosshair classique est remplacé par l'overlay scope.
    if mode.sniper_fullscreen && mode.ads_progress > 0.5 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let screen = ctx.content_rect();
    let center = screen.center();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("forgia_crosshair"),
    ));
    let p = mode.ads_progress.clamp(0.0, 1.0);

    // Hipfire crosshair : croix blanche, fade out en ADS
    if p < 1.0 {
        let alpha_hip = ((1.0 - p) * f32::from(tuning.hipfire_cross_alpha)) as u8;
        let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha_hip);
        let len = tuning.hipfire_cross_len;
        let stroke = egui::Stroke::new(tuning.hipfire_cross_stroke, color);
        painter.line_segment(
            [
                egui::pos2(center.x - len, center.y),
                egui::pos2(center.x + len, center.y),
            ],
            stroke,
        );
        painter.line_segment(
            [
                egui::pos2(center.x, center.y - len),
                egui::pos2(center.x, center.y + len),
            ],
            stroke,
        );
        painter.circle_filled(
            center,
            1.5,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha_hip),
        );
    }

    // ADS red dot : point rouge précis, fade in en ADS
    if p > 0.0 {
        let alpha_ads = (p * 240.0) as u8;
        painter.circle_filled(
            center,
            tuning.ads_dot_outer_radius,
            egui::Color32::from_rgba_unmultiplied(255, 60, 40, alpha_ads / 4),
        );
        painter.circle_filled(
            center,
            tuning.ads_dot_inner_radius,
            egui::Color32::from_rgba_unmultiplied(255, 30, 30, alpha_ads),
        );
    }
}

/// Sniper scope fullscreen overlay (style CoD/Halo ACOG).
///
/// Quand `sniper_fullscreen = true` et `ads_progress > 0` :
/// - 4 quadrants **semi-transparents** aux coins (alpha 145) — user voit
///   la scène dimmée autour (vision périphérique préservée)
/// - **Vignette gradient** : 8 anneaux concentriques externes au scope,
///   alpha progressif → effet "frosted glass" sans shader (fake gaussian)
/// - Cercle bordure scope (anneau métallique)
/// - Reticle croix fine au centre + dot rouge + tick marks
///
/// Backlog : remplacer par vraie gaussian blur post-process (WGSL shader +
/// render-to-texture) pour effet AAA pro (ACOG / CoD scope). ~2-3h effort.
fn draw_sniper_scope_overlay(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mode: Res<CrosshairMode>,
    tuning: Res<CrosshairTuning>,
) {
    if *app_state.get() != AppMode::InGame {
        return;
    }
    if !mode.sniper_fullscreen || mode.ads_progress <= 0.01 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let screen = ctx.content_rect();
    let center = screen.center();
    let p = mode.ads_progress.clamp(0.0, 1.0);

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_sniper_scope"),
    ));

    let scope_radius = screen.width().min(screen.height()) * tuning.sniper_scope_radius_factor;
    // Semi-transparent (vision périphérique préservée).
    let dim_alpha = (p * f32::from(tuning.sniper_dim_alpha)) as u8;
    let dim = egui::Color32::from_rgba_unmultiplied(
        tuning.sniper_dim_color[0],
        tuning.sniper_dim_color[1],
        tuning.sniper_dim_color[2],
        dim_alpha,
    );

    // 4 rectangles vignette (haut, bas, gauche, droite — laissent passer la scène).
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(screen.min.x, screen.min.y),
            egui::pos2(screen.max.x, center.y - scope_radius),
        ),
        0.0,
        dim,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(screen.min.x, center.y + scope_radius),
            egui::pos2(screen.max.x, screen.max.y),
        ),
        0.0,
        dim,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(screen.min.x, center.y - scope_radius),
            egui::pos2(center.x - scope_radius, center.y + scope_radius),
        ),
        0.0,
        dim,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(center.x + scope_radius, center.y - scope_radius),
            egui::pos2(screen.max.x, center.y + scope_radius),
        ),
        0.0,
        dim,
    );

    // Vignette gradient : N anneaux concentriques (Tuning) extérieurs au scope.
    let vignette_rings = tuning.sniper_vignette_rings.max(1);
    let ring_thickness = scope_radius * tuning.sniper_ring_thickness_factor;
    for i in 0..vignette_rings {
        let t = if vignette_rings > 1 {
            i as f32 / (vignette_rings - 1) as f32
        } else {
            0.0
        };
        let ring_alpha = ((1.0 - t) * tuning.sniper_ring_max_alpha * p) as u8;
        let ring_color = egui::Color32::from_rgba_unmultiplied(
            tuning.sniper_ring_color[0],
            tuning.sniper_ring_color[1],
            tuning.sniper_ring_color[2],
            ring_alpha,
        );
        let r = scope_radius + ring_thickness * t * tuning.sniper_ring_outer_extent;
        painter.circle_stroke(center, r, egui::Stroke::new(ring_thickness * 0.35, ring_color));
    }

    // Bordure scope métallique
    let alpha = (p * 255.0) as u8;
    let ring_color = egui::Color32::from_rgba_unmultiplied(20, 20, 20, alpha);
    painter.circle_stroke(
        center,
        scope_radius,
        egui::Stroke::new(tuning.sniper_border_width, ring_color),
    );
    let outer_ring = egui::Color32::from_rgba_unmultiplied(60, 60, 60, alpha);
    painter.circle_stroke(
        center,
        scope_radius - tuning.sniper_border_inner_offset,
        egui::Stroke::new(tuning.sniper_border_inner_width, outer_ring),
    );

    // Reticle (depuis Tuning)
    let reticle_alpha = (p * 255.0).min(255.0) as u8;
    let reticle_color = egui::Color32::from_rgba_unmultiplied(
        tuning.sniper_reticle_color[0],
        tuning.sniper_reticle_color[1],
        tuning.sniper_reticle_color[2],
        reticle_alpha,
    );
    let gap = tuning.sniper_reticle_gap;
    let line_len = scope_radius * tuning.sniper_reticle_line_factor;
    let line_stroke = egui::Stroke::new(tuning.sniper_reticle_line_stroke, reticle_color);
    painter.line_segment(
        [
            egui::pos2(center.x - line_len, center.y),
            egui::pos2(center.x - gap, center.y),
        ],
        line_stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x + gap, center.y),
            egui::pos2(center.x + line_len, center.y),
        ],
        line_stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - line_len),
            egui::pos2(center.x, center.y - gap),
        ],
        line_stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x, center.y + gap),
            egui::pos2(center.x, center.y + line_len),
        ],
        line_stroke,
    );
    painter.circle_filled(
        center,
        tuning.sniper_red_dot_radius,
        egui::Color32::from_rgba_unmultiplied(
            tuning.sniper_red_dot_color[0],
            tuning.sniper_red_dot_color[1],
            tuning.sniper_red_dot_color[2],
            reticle_alpha,
        ),
    );

    // Tick marks distance (4 offsets, depuis Tuning size)
    let tick_y = center.y;
    let tick_size = tuning.sniper_reticle_tick_size;
    let tick_stroke = egui::Stroke::new(tuning.sniper_reticle_tick_stroke, reticle_color);
    for offset in [-line_len * 0.5, -line_len * 0.25, line_len * 0.25, line_len * 0.5] {
        painter.line_segment(
            [
                egui::pos2(center.x + offset, tick_y - tick_size),
                egui::pos2(center.x + offset, tick_y + tick_size),
            ],
            tick_stroke,
        );
    }
}
