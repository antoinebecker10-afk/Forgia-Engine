//! # forgia-ui
//!
//! Menu (Start + choix FPS/RPG + Pause + Settings) + HUD partagé.
//!
//! **Anti-traps V1 enforced** :
//! - 1 seul handler ESC
//! - `MenuCamera2d` isolé OnEnter(Menu)/OnExit(Menu)
//! - `Time<Real>` pour sensors UI
//!
//! Crates atomiques wire-up :
//! - `forgia-crosshair` : crosshair + sniper scope overlay
//! - `forgia-hitmarker` : hit confirm visual

use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use forgia_core::prelude::*;
// Re-exports backward compat (déplacés vers crates atomiques 2026-05-16)
pub use forgia_crosshair::CrosshairMode;
pub use forgia_hitmarker::HitmarkerState;

pub mod prelude {
    pub use crate::ForgiaUiPlugin;
    /// Re-export backward compat — préférer `forgia_crosshair::CrosshairMode` direct.
    pub use forgia_crosshair::CrosshairMode;
    /// Re-export backward compat — préférer `forgia_hitmarker::HitmarkerState` direct.
    pub use forgia_hitmarker::HitmarkerState;
}

pub struct ForgiaUiPlugin;

impl Plugin for ForgiaUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        // Crates atomiques (règle fine-grained-crates) — idempotent.
        if !app.is_plugin_added::<forgia_crosshair::ForgiaCrosshairPlugin>() {
            app.add_plugins(forgia_crosshair::ForgiaCrosshairPlugin);
        }
        if !app.is_plugin_added::<forgia_hitmarker::ForgiaHitmarkerPlugin>() {
            app.add_plugins(forgia_hitmarker::ForgiaHitmarkerPlugin);
        }
        // MenuCamera2d permanente : spawn 1 fois Startup, JAMAIS despawn.
        // Ordre explicite high pour render egui par-dessus la Camera3d gameplay.
        // Anti-trap V1 : éviter le frame où aucune caméra n'existe (ESC bug).
        app.add_systems(Startup, spawn_menu_camera_permanent)
            .add_systems(OnEnter(AppMode::Menu), release_cursor)
            .add_systems(OnEnter(AppMode::InGame), grab_cursor)
            .add_systems(OnEnter(AppMode::Paused), (release_cursor, pause_time))
            .add_systems(OnExit(AppMode::Paused), resume_time)
            // Story-455 Phase G — paused_overlay_ui retiré (remplacé par forgia-ui-pause-menu
            // cliquable Resume / Settings / Quit). Le handler ESC/Q reste ici (escape_handler).
            .add_systems(EguiPrimaryContextPass, main_menu_ui)
            .add_systems(Update, escape_handler.in_set(GameSet::UI));
    }
}

#[derive(Component)]
struct MenuCamera2d;

/// MenuCamera2d permanente — spawn une fois au Startup, JAMAIS despawn.
fn spawn_menu_camera_permanent(
    mut commands: Commands,
    q: Query<Entity, With<MenuCamera2d>>,
) {
    if q.is_empty() {
        commands.spawn((
            Camera2d,
            Camera {
                order: 10,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            MenuCamera2d,
            Name::new("MenuCamera2d (permanent)"),
        ));
        info!("[forgia-ui] MenuCamera2d spawned (permanent, order=10, clear=None)");
    }
}

fn main_menu_ui(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut exit: MessageWriter<AppExit>,
    mut start_run: MessageWriter<forgia_mode_roguelite::StartRunEvent>,
    mut last_state: Local<Option<AppMode>>,
) {
    let current = app_state.get().clone();
    if last_state.as_ref() != Some(&current) {
        info!("[forgia-ui] main_menu_ui state = {current:?}");
        *last_state = Some(current.clone());
    }
    if current != AppMode::Menu {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        warn!("[forgia-ui] main_menu_ui: egui ctx not found (no Camera2d?)");
        return;
    };
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(15, 15, 25)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                ui.heading(egui::RichText::new("FORGIA V2").size(64.0).color(egui::Color32::from_rgb(255, 140, 50)));
                ui.add_space(20.0);
                ui.label(egui::RichText::new("Choisis ton mode").size(24.0).color(egui::Color32::WHITE));
                ui.add_space(60.0);

                if ui.add(egui::Button::new(egui::RichText::new("⚔  FPS Arena").size(28.0)).min_size(egui::vec2(280.0, 60.0))).clicked() {
                    next_game.set(GameMode::Fps);
                    next_app.set(AppMode::InGame);
                }
                ui.add_space(20.0);
                if ui.add(egui::Button::new(egui::RichText::new("🗺  RPG OpenWorld").size(28.0)).min_size(egui::vec2(280.0, 60.0))).clicked() {
                    next_game.set(GameMode::Rpg);
                    next_app.set(AppMode::InGame);
                }
                ui.add_space(20.0);
                if ui.add(egui::Button::new(egui::RichText::new("🎲  Roguelite Run").size(28.0)).min_size(egui::vec2(280.0, 60.0))).clicked() {
                    next_game.set(GameMode::Roguelite);
                    next_app.set(AppMode::InGame);
                    start_run.write(forgia_mode_roguelite::StartRunEvent { seed: None });
                }
                ui.add_space(40.0);
                if ui.add(egui::Button::new(egui::RichText::new("Quitter").size(20.0)).min_size(egui::vec2(180.0, 40.0))).clicked() {
                    exit.write(AppExit::Success);
                }

                ui.add_space(80.0);
                ui.label(egui::RichText::new("Phase 1 — Hello World jouable").size(14.0).color(egui::Color32::GRAY));
            });
        });
}

/// Overlay PAUSED legacy (remplacé story-455 Phase G par forgia-ui-pause-menu cliquable).
/// Conservé en `#[allow(dead_code)]` pour référence courte ; à supprimer story-457.
#[allow(dead_code)]
fn paused_overlay_ui(
    app_state: Res<State<AppMode>>,
    mut ctx: EguiContexts,
) {
    if !matches!(app_state.get(), AppMode::Paused) {
        return;
    }
    let Ok(ctx) = ctx.ctx_mut() else { return };
    egui::Area::new(egui::Id::new("paused_overlay"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(180))
                .inner_margin(egui::Margin::symmetric(48, 32))
                .corner_radius(egui::CornerRadius::same(8))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(4.0);
                        ui.heading(
                            egui::RichText::new("PAUSED")
                                .size(56.0)
                                .color(egui::Color32::from_rgb(255, 230, 100))
                                .strong(),
                        );
                        ui.add_space(18.0);
                        ui.label(
                            egui::RichText::new("ESC — Resume")
                                .size(20.0)
                                .color(egui::Color32::WHITE),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Q   — Quit to Menu")
                                .size(20.0)
                                .color(egui::Color32::from_gray(200)),
                        );
                        ui.add_space(4.0);
                    });
                });
        });
}

/// Handler ESC unique :
///  - InGame  → Paused (pause gameplay, libère curseur)
///  - Paused  → InGame (resume)
///  - Paused + Q → Menu (quit to menu)
fn escape_handler(
    keys: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let from = app_state.get().clone();

    // Q during Paused = quit to Menu
    if matches!(from, AppMode::Paused) && keys.just_pressed(KeyCode::KeyQ) {
        info!("[forgia-ui] Q pressed (Paused → Menu)");
        next_app.set(AppMode::Menu);
        next_game.set(GameMode::None);
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        match &from {
            AppMode::InGame => {
                info!("[forgia-ui] ESC pressed (InGame → Paused)");
                next_app.set(AppMode::Paused);
                if let Ok(mut opts) = q_cursor.single_mut() {
                    opts.grab_mode = CursorGrabMode::None;
                    opts.visible = true;
                }
            }
            AppMode::Paused => {
                info!("[forgia-ui] ESC pressed (Paused → InGame)");
                next_app.set(AppMode::InGame);
                if let Ok(mut opts) = q_cursor.single_mut() {
                    opts.grab_mode = CursorGrabMode::Locked;
                    opts.visible = false;
                }
            }
            other => {
                info!("[forgia-ui] ESC pressed in {other:?} — no transition");
            }
        }
    }
    // Q en Paused → quitte au Menu.
    if keys.just_pressed(KeyCode::KeyQ) && matches!(from, AppMode::Paused) {
        info!("[forgia-ui] Q pressed (Paused → Menu)");
        next_app.set(AppMode::Menu);
        next_game.set(GameMode::None);
    }
}

/// Freeze `Time<Virtual>` quand on entre en Paused (animations + locomotion
/// + physics gated par Time<Virtual> stoppent). Pattern Bevy standard.
fn pause_time(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.pause();
    info!("[forgia-ui] Time<Virtual> paused");
}

/// Reprend le temps virtuel quand on sort de Paused.
fn resume_time(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.unpause();
    info!("[forgia-ui] Time<Virtual> resumed");
}

/// Lock cursor au centre + invisible quand on entre InGame (pour mouse_look).
fn grab_cursor(mut q: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut opts) = q.single_mut() {
        opts.grab_mode = CursorGrabMode::Locked;
        opts.visible = false;
        info!("[forgia-ui] Cursor grabbed (Locked + invisible)");
    } else {
        warn!("[forgia-ui] grab_cursor: PrimaryWindow CursorOptions not found");
    }
}

/// Release cursor (visible + free) quand on entre Menu.
fn release_cursor(mut q: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut opts) = q.single_mut() {
        opts.grab_mode = CursorGrabMode::None;
        opts.visible = true;
        info!("[forgia-ui] Cursor released (None + visible)");
    } else {
        warn!("[forgia-ui] release_cursor: PrimaryWindow CursorOptions not found");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaUiPlugin;
    }
}
