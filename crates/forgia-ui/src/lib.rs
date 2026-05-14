//! # forgia-ui
//!
//! Menu (Start + choix FPS/RPG + Pause + Settings) + HUD partagé.
//!
//! **Anti-traps V1 enforced** :
//! - 1 seul handler ESC
//! - `MenuCamera2d` isolé OnEnter(Menu)/OnExit(Menu)
//! - `Time<Real>` pour sensors UI

use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use forgia_combat::prelude::*;
use forgia_core::prelude::*;

pub mod prelude {
    pub use crate::ForgiaUiPlugin;
}

pub struct ForgiaUiPlugin;

impl Plugin for ForgiaUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        // MenuCamera2d permanente : spawn 1 fois Startup, JAMAIS despawn.
        // Ordre explicite high pour render egui par-dessus la Camera3d gameplay.
        // Anti-trap V1 : éviter le frame où aucune caméra n'existe (ESC bug).
        app.init_resource::<HitmarkerState>()
            .add_systems(Startup, spawn_menu_camera_permanent)
            .add_systems(OnEnter(AppMode::Menu), release_cursor)
            .add_systems(OnEnter(AppMode::InGame), grab_cursor)
            .add_systems(EguiPrimaryContextPass, (main_menu_ui, draw_crosshair, draw_hitmarker))
            .add_systems(Update, (escape_handler, hitmarker_trigger).in_set(GameSet::UI));
    }
}

#[derive(Component)]
struct MenuCamera2d;

/// MenuCamera2d permanente — spawn une fois au Startup, JAMAIS despawn.
/// Camera order=10 : render PAR-DESSUS la Camera3d gameplay (order default=0).
/// Egui s'attache à cette Camera2d. Quand AppMode != Menu, main_menu_ui early return,
/// donc rien ne dessine, mais la caméra existe toujours.
fn spawn_menu_camera_permanent(
    mut commands: Commands,
    q: Query<Entity, With<MenuCamera2d>>,
) {
    if q.is_empty() {
        commands.spawn((
            Camera2d,
            Camera {
                order: 10,
                // CRITICAL : None sinon Camera2d écrase tout le framebuffer Camera3d/Atmosphere
                // chaque frame avec ClearColor (Resource brun). Egui render par-dessus en transparent.
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
                ui.add_space(40.0);
                if ui.add(egui::Button::new(egui::RichText::new("Quitter").size(20.0)).min_size(egui::vec2(180.0, 40.0))).clicked() {
                    exit.write(AppExit::Success);
                }

                ui.add_space(80.0);
                ui.label(egui::RichText::new("Phase 1 — Hello World jouable").size(14.0).color(egui::Color32::GRAY));
            });
        });
}

/// Handler ESC unique — toggle InGame ↔ Menu.
/// Anti-trap V1 : ce handler est le SEUL qui touche Escape pour le state machine.
/// Lit `ButtonInput<KeyCode>` directement (pas leafwing) pour fonctionner même sans Player spawn.
/// Release le curseur IMMÉDIATEMENT (sans attendre OnEnter qui trigger le frame d'après).
fn escape_handler(
    keys: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        let from = app_state.get().clone();
        match &from {
            AppMode::InGame => {
                info!("[forgia-ui] ESC pressed (InGame → Menu)");
                next_app.set(AppMode::Menu);
                next_game.set(GameMode::None);
                // Release cursor immédiatement (anti-frame-delay)
                if let Ok(mut opts) = q_cursor.single_mut() {
                    opts.grab_mode = CursorGrabMode::None;
                    opts.visible = true;
                    info!("[forgia-ui] Cursor released (immediate)");
                }
            }
            AppMode::Paused => {
                info!("[forgia-ui] ESC pressed (Paused → InGame)");
                next_app.set(AppMode::InGame);
            }
            other => {
                info!("[forgia-ui] ESC pressed in {other:?} — no transition");
            }
        }
    }
}

/// HitmarkerState — Resource Local pour fade visual après hit confirmé.
/// Pattern V1 : 4 segments diagonaux blancs autour crosshair, fade 220ms.
#[derive(Resource, Default)]
pub struct HitmarkerState {
    /// Time remaining (s). >0 = visible, fade linéaire.
    pub time_left: f32,
}

const HITMARKER_DURATION: f32 = 0.22; // V1 hud_hitmarker_duration_ms = 220ms

/// Lit `CombatHitEvent` → reset `HitmarkerState.time_left` à HITMARKER_DURATION.
fn hitmarker_trigger(
    mut hits: MessageReader<CombatHitEvent>,
    mut state: ResMut<HitmarkerState>,
    time: Res<Time>,
) {
    if !hits.is_empty() {
        state.time_left = HITMARKER_DURATION;
        for _ in hits.read() {} // drain
    } else if state.time_left > 0.0 {
        state.time_left = (state.time_left - time.delta_secs()).max(0.0);
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
    let inner = 7.0; // gap autour crosshair
    let outer = 14.0; // longueur segment
    let stroke = egui::Stroke::new(2.5, color);
    // 4 segments diagonaux X
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

/// Crosshair simple — croix blanche 14px au centre de l'écran quand InGame.
/// Anti-trap V1 (memory `reference_egui_layer_intercept_2026_05_13.md`) :
/// dessiné via Painter sur layer Background (PAS Foreground qui intercept clics).
fn draw_crosshair(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
) {
    if *app_state.get() != AppMode::InGame {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let screen = ctx.content_rect();
    let center = screen.center();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("forgia_crosshair"),
    ));
    let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220);
    let len = 7.0;
    let thick = 2.0;
    let stroke = egui::Stroke::new(thick, color);
    // Croix horizontale
    painter.line_segment(
        [
            egui::pos2(center.x - len, center.y),
            egui::pos2(center.x + len, center.y),
        ],
        stroke,
    );
    // Croix verticale
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - len),
            egui::pos2(center.x, center.y + len),
        ],
        stroke,
    );
    // Point central
    painter.circle_filled(center, 1.5, egui::Color32::WHITE);
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
