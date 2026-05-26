//! # forgia-ui-pause-menu
//!
//! Story-455 Phase G — Pause menu cliquable + settings panel persistant.
//!
//! - **Pause root** : 3 boutons Resume / Settings / Quit to Menu
//! - **Settings panel** (sub-state) :
//!   - Sensitivity slider (mutate `MouseLookTuning.base_sensitivity`)
//!   - FOV slider (mutate `Projection::Perspective.fov` sur FpsCamera)
//!   - Save Settings button → `assets/user_settings.toml`
//!   - Back button → retour root menu
//! - **Persistence** : `assets/user_settings.toml` chargé au boot, écrit sur Save click
//!
//! Remplace l'ancien `paused_overlay_ui` de `forgia-ui` (keyboard-only).
//! L'ancien handler ESC/Q reste dans `forgia-ui` pour la transition state.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_player::{FpsCamera, MouseLookTuning};
use crate::style::*;
use serde::{Deserialize, Serialize};
use std::fs;

pub mod prelude {
    pub use super::{ForgiaUiPauseMenuPlugin, PauseMenuState, UserSettings};
}

const USER_SETTINGS_PATH: &str = "assets/user_settings.toml";

// ─── Sub-state du pause menu ──────────────────────────────────────────

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseSubMenu {
    #[default]
    Root,
    Settings,
}

#[derive(Resource, Default)]
pub struct PauseMenuState {
    pub sub: PauseSubMenu,
    /// Dernière action loggée (sensor).
    pub last_action: String,
}

// ─── Persisted user settings ──────────────────────────────────────────

#[derive(Resource, Serialize, Deserialize, Debug, Clone)]
pub struct UserSettings {
    pub mouse_sensitivity: f32,
    pub fov_deg: f32,
    /// BUG-455-10 fix : sensor period genome-driven (cohérence convention Forgia).
    /// Stocké dans le même TOML que les autres user settings pour simplicité.
    #[serde(default = "default_sensor_period")]
    pub sensor_period_secs: f32,
}

fn default_sensor_period() -> f32 {
    1.0
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.002,
            fov_deg: 90.0,
            sensor_period_secs: 1.0,
        }
    }
}

#[derive(Resource, Default)]
pub struct PauseMenuSensor {
    pub last_write_secs: f32,
    pub open_count_session: u32,
    pub last_save_secs: f32,
    pub last_save_success: bool,
}

// ─── Load settings au startup ─────────────────────────────────────────

/// BUG-455-04 fix : ne touche **plus** `MouseLookTuning` ici. Charge `UserSettings`
/// uniquement. Le système `apply_settings_to_tuning` propage via `Changed<UserSettings>`.
/// Élimine la race init Startup non déterministe avec `init_resource::<MouseLookTuning>`.
pub fn load_user_settings_at_boot(mut settings: ResMut<UserSettings>) {
    match fs::read_to_string(USER_SETTINGS_PATH) {
        Ok(content) => match toml::from_str::<UserSettings>(&content) {
            Ok(loaded) => {
                *settings = loaded;
                info!(
                    "[pause-menu] user_settings.toml loaded (sensitivity {:.4}, fov {:.0}°)",
                    settings.mouse_sensitivity, settings.fov_deg
                );
            }
            Err(e) => warn!("[pause-menu] user_settings.toml parse error: {e}"),
        },
        Err(_) => {
            info!("[pause-menu] no user_settings.toml — using defaults");
        }
    }
}

/// BUG-455-04 fix : propagation `UserSettings → MouseLookTuning` event-driven.
/// Run sur `Changed<UserSettings>` ou au démarrage (forcé initial sync via `Local<bool>`).
pub fn apply_settings_to_tuning(
    settings: Res<UserSettings>,
    mut tuning: ResMut<MouseLookTuning>,
    mut applied_once: Local<bool>,
) {
    if !settings.is_changed() && *applied_once {
        return;
    }
    tuning.base_sensitivity = settings.mouse_sensitivity;
    *applied_once = true;
}

/// BUG-455-03 fix : apply FOV à la `FpsCamera` quand on entre InGame OU quand
/// `UserSettings.fov_deg` change. Run en `Update` avec `run_if(in_state(InGame))`
/// pour couvrir les 2 cas (spawn caméra + slider mutation) sans dépendre du timing
/// d'OnEnter (FpsCamera spawn pas garantie à OnEnter exact, async asset load).
pub fn apply_fov_to_camera(
    settings: Res<UserSettings>,
    mut q_cam: Query<&mut Projection, With<FpsCamera>>,
    mut last_applied: Local<f32>,
) {
    if !settings.is_changed() && (*last_applied - settings.fov_deg).abs() < 0.01 {
        return;
    }
    let Ok(mut proj) = q_cam.single_mut() else {
        return;
    };
    if let Projection::Perspective(ref mut p) = *proj {
        p.fov = settings.fov_deg.to_radians();
        *last_applied = settings.fov_deg;
    }
}

// ─── Reset sub-menu sur entrée Paused ─────────────────────────────────

pub fn reset_submenu_on_pause(
    mut state: ResMut<PauseMenuState>,
    mut sensor: ResMut<PauseMenuSensor>,
) {
    state.sub = PauseSubMenu::Root;
    state.last_action = "opened".to_string();
    sensor.open_count_session = sensor.open_count_session.saturating_add(1);
}

// ─── UI Pause Menu ────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_pause_menu(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut state: ResMut<PauseMenuState>,
    mut settings: ResMut<UserSettings>,
    mut sensor: ResMut<PauseMenuSensor>,
    time: Res<Time>,
) {
    if !matches!(app_state.get(), AppMode::Paused) {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Area::new(egui::Id::new("forgia_pause_menu"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(200))
                .inner_margin(egui::Margin::symmetric(56, 36))
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(egui::Stroke::new(2.5, C_PRIMARY))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| match state.sub {
                        PauseSubMenu::Root => {
                            draw_root(ui, &mut next_app, &mut next_game, &mut state)
                        }
                        PauseSubMenu::Settings => draw_settings(
                            ui,
                            &mut state,
                            &mut settings,
                            &mut sensor,
                            time.elapsed_secs(),
                        ),
                    });
                });
        });
}

fn draw_root(
    ui: &mut egui::Ui,
    next_app: &mut NextState<AppMode>,
    next_game: &mut NextState<GameMode>,
    state: &mut PauseMenuState,
) {
    ui.add_space(4.0);
    ui.heading(
        egui::RichText::new("PAUSED")
            .size(56.0)
            .color(C_PRIMARY)
            .strong(),
    );
    ui.add_space(24.0);

    let btn = |ui: &mut egui::Ui, label: &str| -> bool {
        ui.add(
            egui::Button::new(egui::RichText::new(label).size(24.0))
                .min_size(egui::vec2(260.0, 48.0)),
        )
        .clicked()
    };

    if btn(ui, "▶ Resume") {
        info!("[pause-menu] Resume clicked");
        next_app.set(AppMode::InGame);
        state.last_action = "resume".to_string();
    }
    ui.add_space(8.0);
    if btn(ui, "⚙ Settings") {
        info!("[pause-menu] Settings clicked");
        state.sub = PauseSubMenu::Settings;
        state.last_action = "open_settings".to_string();
    }
    ui.add_space(8.0);
    if btn(ui, "✕ Quit to Menu") {
        info!("[pause-menu] Quit clicked");
        next_app.set(AppMode::Menu);
        next_game.set(GameMode::None);
        state.last_action = "quit_to_menu".to_string();
    }
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new("ESC — Resume   |   Q — Quit")
            .size(13.0)
            .color(egui::Color32::from_gray(180)),
    );
}

fn draw_settings(
    ui: &mut egui::Ui,
    state: &mut PauseMenuState,
    settings: &mut UserSettings,
    sensor: &mut PauseMenuSensor,
    now: f32,
) {
    ui.heading(
        egui::RichText::new("SETTINGS")
            .size(40.0)
            .color(C_PRIMARY)
            .strong(),
    );
    ui.add_space(16.0);

    // BUG-455-03/04 fix : le slider mute UserSettings ONLY. La propagation vers
    // MouseLookTuning et FpsCamera::Projection est faite par apply_settings_to_tuning
    // et apply_fov_to_camera (event-driven Changed<UserSettings>, race-free).
    ui.label(egui::RichText::new("Mouse Sensitivity").size(16.0));
    let mut sens = settings.mouse_sensitivity;
    if ui
        .add(egui::Slider::new(&mut sens, 0.0005..=0.008).fixed_decimals(4))
        .changed()
    {
        settings.mouse_sensitivity = sens;
    }
    ui.add_space(10.0);

    ui.label(egui::RichText::new("Field of View (°)").size(16.0));
    let mut fov = settings.fov_deg;
    if ui
        .add(egui::Slider::new(&mut fov, 60.0..=120.0).fixed_decimals(0))
        .changed()
    {
        settings.fov_deg = fov;
    }
    ui.add_space(20.0);

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(egui::RichText::new("💾 Save").size(18.0))
                    .min_size(egui::vec2(120.0, 36.0)),
            )
            .clicked()
        {
            let saved = save_user_settings(settings);
            sensor.last_save_secs = now;
            sensor.last_save_success = saved;
            state.last_action = if saved {
                "saved".to_string()
            } else {
                "save_failed".to_string()
            };
            if saved {
                info!("[pause-menu] settings saved to {USER_SETTINGS_PATH}");
            }
        }
        if ui
            .add(
                egui::Button::new(egui::RichText::new("← Back").size(18.0))
                    .min_size(egui::vec2(120.0, 36.0)),
            )
            .clicked()
        {
            state.sub = PauseSubMenu::Root;
            state.last_action = "settings_back".to_string();
        }
    });
}

fn save_user_settings(settings: &UserSettings) -> bool {
    match toml::to_string_pretty(settings) {
        Ok(content) => fs::write(USER_SETTINGS_PATH, content).is_ok(),
        Err(e) => {
            warn!("[pause-menu] save serialize error: {e}");
            false
        }
    }
}

// ─── Sensor ───────────────────────────────────────────────────────────

pub fn write_pause_menu_sensor(
    time: Res<Time>,
    state: Res<PauseMenuState>,
    settings: Res<UserSettings>,
    app_state: Res<State<AppMode>>,
    mut sensor: ResMut<PauseMenuSensor>,
) {
    let now = time.elapsed_secs();
    if now - sensor.last_write_secs < settings.sensor_period_secs.max(0.1) {
        return;
    }
    sensor.last_write_secs = now;
    let json = format!(
        r#"{{"timestamp_secs":{:.2},"open":{},"sub_menu":"{:?}","open_count_session":{},"last_action":"{}","last_save_secs":{:.2},"last_save_success":{},"sensitivity":{:.4},"fov_deg":{:.1}}}"#,
        now,
        matches!(app_state.get(), AppMode::Paused),
        state.sub,
        sensor.open_count_session,
        state.last_action.replace('"', "'"),
        sensor.last_save_secs,
        sensor.last_save_success,
        settings.mouse_sensitivity,
        settings.fov_deg,
    );
    let _ = fs::write("forgia_pause_menu.json", json);
}

// ─── Plugin ───────────────────────────────────────────────────────────

pub struct ForgiaUiPauseMenuPlugin;

impl Plugin for ForgiaUiPauseMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseMenuState>()
            .init_resource::<UserSettings>()
            .init_resource::<PauseMenuSensor>()
            .add_systems(Startup, load_user_settings_at_boot)
            .add_systems(OnEnter(AppMode::Paused), reset_submenu_on_pause)
            .add_systems(EguiPrimaryContextPass, draw_pause_menu)
            // BUG-455-03/04 fix : propagation event-driven UserSettings → tuning/camera.
            // Race-free vs init order Startup.
            .add_systems(
                Update,
                (
                    apply_settings_to_tuning,
                    apply_fov_to_camera.run_if(in_state(AppMode::InGame)),
                    write_pause_menu_sensor,
                ),
            );
    }
}
