//! Overlay perf live — **FPS + frame time (ms)** affichés en continu en jeu.
//!
//! Lecture directe de `FrameTimeDiagnosticsPlugin` (Bevy 0.18) via `DiagnosticsStore` :
//! - FPS lissé  (`FrameTimeDiagnosticsPlugin::FPS`)
//! - frame time lissé en ms (`FrameTimeDiagnosticsPlugin::FRAME_TIME`)
//!
//! Chip verre « Verre & Braise » en haut-gauche, décalé à droite de la minimap
//! roguelite (zone vide dans tous les modes). Visible tant qu'on est en jeu
//! (`InGame` ou `Paused`). Le plugin ajoute `FrameTimeDiagnosticsPlugin` de façon
//! idempotente (Bevy 0.18 ignore le double-add) → autonome même sans
//! forgia-observability wiré.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;

use crate::style::*;
use crate::theme::display_font;

fn draw_perf_overlay(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    diagnostics: Res<DiagnosticsStore>,
) {
    // Continu tant qu'on joue — inclut la Pause (indépendante du GameMode).
    if !matches!(*app_state.get(), AppMode::InGame | AppMode::Paused) {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);

    // Couleur FPS : vert ≥ 60, or 30..60, rouge < 30 (lecture immédiate de la santé).
    let fps_col = if fps >= 60.0 {
        STATUS_OK
    } else if fps >= 30.0 {
        FORGE_OR
    } else {
        STATUS_DANGER
    };

    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_perf_overlay"),
    ));

    // Haut-gauche, décalé à droite de la minimap roguelite (R=72 @ x≈22..166).
    let left = screen.min.x + 180.0;
    let top = screen.min.y + 10.0;
    let (w, h) = (176.0, 34.0);
    let rect = egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(w, h));
    glass_panel(&painter, rect, 8.0);

    let cy = rect.center().y;
    // FPS (valeur colorée + label discret).
    text_with_outline(
        &painter,
        egui::pos2(left + 12.0, cy),
        egui::Align2::LEFT_CENTER,
        &format!("{fps:.0}"),
        display_font(20.0),
        fps_col,
        1.3,
    );
    text_with_outline(
        &painter,
        egui::pos2(left + 64.0, cy),
        egui::Align2::LEFT_CENTER,
        "FPS",
        display_font(12.0),
        C_TEXT_MUTED,
        1.0,
    );
    // Frame time (droite).
    text_with_outline(
        &painter,
        egui::pos2(rect.max.x - 12.0, cy),
        egui::Align2::RIGHT_CENTER,
        &format!("{ms:.1} ms"),
        display_font(15.0),
        C_TEXT_LIGHT,
        1.2,
    );
}

pub struct PerfOverlayPlugin;

impl Plugin for PerfOverlayPlugin {
    fn build(&self, app: &mut App) {
        // Source des diagnostics — idempotent (Bevy 0.18 ignore le double-add ;
        // forgia-observability l'ajoute déjà, on reste autonome sinon).
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.add_systems(EguiPrimaryContextPass, draw_perf_overlay);
    }
}
