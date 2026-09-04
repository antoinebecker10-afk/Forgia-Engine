//! Gate de chargement Lobby (étape 6 hub-menu) : le hub est au MENU, le Lobby
//! n'est qu'un écran de warmup qui auto-lance la run dès les pipelines prêts.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_mode_roguelite::pipeline_warmup::WarmupState;
use forgia_mode_roguelite::StartRunEvent;
use forgia_ui_lib::style::{FORGE_CREME, FORGE_OR};

/// Étape 6 hub-menu — lancement direct : le Lobby n'est plus un hub interactif mais
/// un **gate de chargement**. Dès que le warmup PBR est prêt (`WarmupState.done`),
/// auto-fire `StartRunEvent` → combat, sans action utilisateur (tout est configuré
/// au menu). L'anti-double-spawn de `sys_start_run` + `run_if(Lobby)` (qui coupe
/// après la transition InRun) bornent le tir. Replay (Defeat/Victory → Lobby) :
/// `done` reste vrai → lancement instantané.
pub(crate) fn sys_auto_start_when_warm(
    warmup: Option<Res<WarmupState>>,
    mut start_run: MessageWriter<StartRunEvent>,
) {
    if warmup.as_ref().is_some_and(|w| w.done) {
        start_run.write(StartRunEvent { seed: None });
    }
}

/// Overlay de chargement plein écran pendant le gate Lobby — couvre l'ancien hub
/// interactif le temps du warmup, avant l'auto-lancement. Layer Foreground (au-
/// dessus des panneaux du hub, ordre Middle).
pub(crate) fn sys_lobby_loading_overlay(
    mut contexts: EguiContexts,
    warmup: Option<Res<WarmupState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let ready = warmup.as_ref().is_some_and(|w| w.done);
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("lobby_loading_overlay"),
    ));
    painter.rect_filled(screen, 0.0, egui::Color32::from_rgb(12, 9, 16));
    painter.text(
        screen.center(),
        egui::Align2::CENTER_CENTER,
        if ready {
            "La Forge est prête…"
        } else {
            "Préparation de la Forge…"
        },
        egui::FontId::proportional(34.0),
        FORGE_CREME,
    );
    painter.text(
        screen.center() + egui::vec2(0.0, 44.0),
        egui::Align2::CENTER_CENTER,
        "◆ ◆ ◆",
        egui::FontId::proportional(18.0),
        FORGE_OR,
    );
}
