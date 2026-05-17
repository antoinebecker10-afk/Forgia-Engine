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
    pub use crate::{CrosshairMode, ForgiaCrosshairPlugin};
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
        app.init_resource::<CrosshairMode>().add_systems(
            EguiPrimaryContextPass,
            (draw_crosshair, draw_sniper_scope_overlay),
        );
    }
}

fn draw_crosshair(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mode: Res<CrosshairMode>,
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
        let alpha_hip = ((1.0 - p) * 220.0) as u8;
        let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha_hip);
        let len = 7.0;
        let stroke = egui::Stroke::new(2.0, color);
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
            4.0,
            egui::Color32::from_rgba_unmultiplied(255, 60, 40, alpha_ads / 4),
        );
        painter.circle_filled(
            center,
            2.0,
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

    let scope_radius = screen.width().min(screen.height()) * 0.42;
    // ★ Semi-transparent (alpha 145/255 ≈ 57%) au lieu de full opaque noir.
    // User voit la scène dimmée AUTOUR du scope → vision périphérique préservée.
    let dim_alpha = (p * 145.0) as u8;
    let dim = egui::Color32::from_rgba_unmultiplied(8, 8, 12, dim_alpha);

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

    // ★ Vignette gradient : 8 anneaux concentriques EXTÉRIEURS au scope.
    // Chaque anneau progressivement plus opaque vers le scope → effet "frosted
    // glass" qui simule un blur radial sans shader. Donne l'illusion de blur.
    let vignette_rings = 8;
    let ring_thickness = scope_radius * 0.18; // épaisseur cumulée hors scope
    for i in 0..vignette_rings {
        // Distance du scope (i=0 = collé au scope, i=N-1 = loin du scope)
        let t = i as f32 / (vignette_rings - 1) as f32;
        // Alpha décroissant vers l'extérieur (max au bord scope, min loin)
        let ring_alpha = ((1.0 - t) * 90.0 * p) as u8;
        let ring_color = egui::Color32::from_rgba_unmultiplied(15, 15, 18, ring_alpha);
        let r = scope_radius + ring_thickness * t * 1.5;
        painter.circle_stroke(center, r, egui::Stroke::new(ring_thickness * 0.35, ring_color));
    }

    // Bordure scope métallique (lecture immédiate du contour)
    let alpha = (p * 255.0) as u8;
    let ring_color = egui::Color32::from_rgba_unmultiplied(20, 20, 20, alpha);
    painter.circle_stroke(center, scope_radius, egui::Stroke::new(8.0, ring_color));
    let outer_ring = egui::Color32::from_rgba_unmultiplied(60, 60, 60, alpha);
    painter.circle_stroke(center, scope_radius - 4.0, egui::Stroke::new(2.0, outer_ring));

    // Reticle
    let reticle_alpha = ((p * 255.0).min(255.0)) as u8;
    let reticle_color = egui::Color32::from_rgba_unmultiplied(10, 10, 10, reticle_alpha);
    let gap = 8.0;
    let line_len = scope_radius * 0.85;
    painter.line_segment(
        [
            egui::pos2(center.x - line_len, center.y),
            egui::pos2(center.x - gap, center.y),
        ],
        egui::Stroke::new(1.5, reticle_color),
    );
    painter.line_segment(
        [
            egui::pos2(center.x + gap, center.y),
            egui::pos2(center.x + line_len, center.y),
        ],
        egui::Stroke::new(1.5, reticle_color),
    );
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - line_len),
            egui::pos2(center.x, center.y - gap),
        ],
        egui::Stroke::new(1.5, reticle_color),
    );
    painter.line_segment(
        [
            egui::pos2(center.x, center.y + gap),
            egui::pos2(center.x, center.y + line_len),
        ],
        egui::Stroke::new(1.5, reticle_color),
    );
    painter.circle_filled(
        center,
        2.0,
        egui::Color32::from_rgba_unmultiplied(255, 30, 30, reticle_alpha),
    );

    // Tick marks horizontales (graduations distance)
    let tick_y = center.y;
    for offset in [-line_len * 0.5, -line_len * 0.25, line_len * 0.25, line_len * 0.5] {
        painter.line_segment(
            [
                egui::pos2(center.x + offset, tick_y - 4.0),
                egui::pos2(center.x + offset, tick_y + 4.0),
            ],
            egui::Stroke::new(1.0, reticle_color),
        );
    }
}
