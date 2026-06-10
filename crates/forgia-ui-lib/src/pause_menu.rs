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
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_audio::UserMasterVolume;
use forgia_core::prelude::*;
use forgia_player::{FpsCamera, MouseLookTuning};
use crate::style::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub mod prelude {
    pub use super::{ForgiaUiPauseMenuPlugin, PauseMenuState, UserSettings};
}

/// Ancien emplacement (≤ story-594) — encore LU pour migration, plus jamais écrit :
/// le dossier d'install est read-only en distribution (Program Files / Steam),
/// et les réglages user n'ont rien à faire dans les assets versionnés.
const LEGACY_SETTINGS_PATH: &str = "assets/user_settings.toml";

/// Chemin canonique : `%APPDATA%\Forgia\user_settings.toml` (story-595, M2-B1).
/// Fallback cwd legacy si APPDATA absent (environnement exotique/dev).
fn settings_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(|a| PathBuf::from(a).join("Forgia").join("user_settings.toml"))
        .unwrap_or_else(|| PathBuf::from(LEGACY_SETTINGS_PATH))
}

/// Presets de résolution proposés en mode fenêtré (16:9 courants Steam).
const RESOLUTION_PRESETS: [(u32, u32); 4] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];

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
    /// Volume principal 0.0..=1.0 — appliqué via forgia_audio::UserMasterVolume
    /// à tous les canaux kira (story-595, M2-B1 : « volume » n'existait nulle part).
    #[serde(default = "default_master_volume")]
    pub master_volume: f32,
    /// "windowed" | "borderless". Défaut = "windowed" : miroir EXACT du
    /// comportement pré-595 (WindowPlugin 1920×1080 sans mode) — zéro régression.
    #[serde(default = "default_window_mode")]
    pub window_mode: String,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
}

fn default_sensor_period() -> f32 {
    1.0
}
fn default_master_volume() -> f32 {
    1.0
}
fn default_window_mode() -> String {
    "windowed".to_string()
}
fn default_window_width() -> u32 {
    1920
}
fn default_window_height() -> u32 {
    1080
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.002,
            fov_deg: 90.0,
            sensor_period_secs: 1.0,
            master_volume: 1.0,
            window_mode: "windowed".to_string(),
            window_width: 1920,
            window_height: 1080,
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
    // Chemin canonique %APPDATA%, puis migration depuis l'ancien assets/ (lu
    // une fois ; le prochain Save écrira le canonique).
    let canonical = settings_path();
    let (content, source) = match fs::read_to_string(&canonical) {
        Ok(c) => (Some(c), canonical.display().to_string()),
        Err(_) => match fs::read_to_string(LEGACY_SETTINGS_PATH) {
            Ok(c) => (Some(c), format!("{LEGACY_SETTINGS_PATH} (legacy, migration au prochain Save)")),
            Err(_) => (None, String::new()),
        },
    };
    match content {
        Some(content) => match toml::from_str::<UserSettings>(&content) {
            Ok(loaded) => {
                *settings = loaded;
                info!(
                    "[pause-menu] settings chargés depuis {source} (sensibilité {:.4}, fov {:.0}°, volume {:.0}%)",
                    settings.mouse_sensitivity,
                    settings.fov_deg,
                    settings.master_volume * 100.0
                );
            }
            Err(e) => warn!("[pause-menu] user_settings.toml parse error: {e}"),
        },
        None => {
            info!("[pause-menu] aucun user_settings.toml — défauts");
        }
    }
}

/// Propage `UserSettings.master_volume` → `forgia_audio::UserMasterVolume`
/// (appliqué aux canaux kira par forgia-audio). Event-driven + sync initiale.
pub fn apply_settings_to_volume(
    settings: Res<UserSettings>,
    mut volume: ResMut<UserMasterVolume>,
    mut applied_once: Local<bool>,
) {
    if !settings.is_changed() && *applied_once {
        return;
    }
    volume.0 = settings.master_volume.clamp(0.0, 1.0);
    *applied_once = true;
}

/// Applique mode fenêtre + résolution à la fenêtre principale.
///
/// 🚨 Anti-crash (2 crashes wgpu 2026-06-10, forgia2_crash.json) : la race resize
/// Vulkan « Attachments have differing sizes » est FATALE, et Windows CLAMPE les
/// fenêtres trop hautes (1920×1080 demandé → 1920×1009 réel sur écran 1080p).
/// Comparer aux dimensions RÉELLES de la fenêtre re-déclenchait donc un resize
/// à chaque réveil (boucle demande→clamp→demande). Règle : on ne répond qu'aux
/// changements de SETTINGS (mémorisés dans `last_applied`) — ce que l'OS fait
/// de la fenêtre ensuite ne nous regarde pas.
pub fn apply_window_settings(
    settings: Res<UserSettings>,
    mut q_window: Query<&mut Window, With<PrimaryWindow>>,
    mut last_applied: Local<Option<(String, u32, u32)>>,
) {
    let want = (
        settings.window_mode.clone(),
        settings.window_width,
        settings.window_height,
    );
    if last_applied.as_ref() == Some(&want) {
        return;
    }
    let Ok(mut window) = q_window.single_mut() else {
        return;
    };
    match settings.window_mode.as_str() {
        "borderless" => {
            let mode = WindowMode::BorderlessFullscreen(MonitorSelection::Current);
            if window.mode != mode {
                window.mode = mode;
            }
        }
        _ => {
            if window.mode != WindowMode::Windowed {
                window.mode = WindowMode::Windowed;
            }
            // Au boot avec les défauts, la fenêtre est déjà 1920×1080 → vrai no-op.
            let (w, h) = (settings.window_width as f32, settings.window_height as f32);
            if (window.resolution.width() - w).abs() > 0.5
                || (window.resolution.height() - h).abs() > 0.5
            {
                window.resolution.set(w, h);
            }
        }
    }
    *last_applied = Some(want);
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
                        PauseSubMenu::Settings => {
                            // 🚨 Anti-crash (2026-06-10 17:51) : `&mut ResMut` =
                            // DerefMut = UserSettings flagué Changed À CHAQUE FRAME
                            // où le panneau est affiché → apply_window_settings se
                            // réveillait en boucle → resize → race wgpu fatale.
                            // bypass + set_changed() SEULEMENT si un contrôle a
                            // réellement été modifié.
                            let inner = settings.bypass_change_detection();
                            let dirty = draw_settings(
                                ui,
                                &mut state,
                                inner,
                                &mut sensor,
                                time.elapsed_secs(),
                            );
                            if dirty {
                                settings.set_changed();
                            }
                        }
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
        egui::RichText::new("PAUSE")
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

    if btn(ui, "▶ Reprendre") {
        info!("[pause-menu] Reprendre");
        next_app.set(AppMode::InGame);
        state.last_action = "resume".to_string();
    }
    ui.add_space(8.0);
    if btn(ui, "⚙ Options") {
        info!("[pause-menu] Options");
        state.sub = PauseSubMenu::Settings;
        state.last_action = "open_settings".to_string();
    }
    ui.add_space(8.0);
    if btn(ui, "✕ Quitter vers le menu") {
        info!("[pause-menu] Quitter vers le menu");
        next_app.set(AppMode::Menu);
        next_game.set(GameMode::None);
        state.last_action = "quit_to_menu".to_string();
    }
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new("ÉCHAP — Reprendre   |   Q — Quitter")
            .size(13.0)
            .color(egui::Color32::from_gray(180)),
    );
}

/// Retourne `true` si un réglage a RÉELLEMENT été modifié (le caller déclenche
/// alors `set_changed()` — voir le commentaire anti-crash dans draw_pause_menu).
fn draw_settings(
    ui: &mut egui::Ui,
    state: &mut PauseMenuState,
    settings: &mut UserSettings,
    sensor: &mut PauseMenuSensor,
    now: f32,
) -> bool {
    let mut dirty = false;
    ui.heading(
        egui::RichText::new("OPTIONS")
            .size(40.0)
            .color(C_PRIMARY)
            .strong(),
    );
    ui.add_space(16.0);

    // BUG-455-03/04 fix : les sliders mutent UserSettings ONLY. La propagation
    // (MouseLookTuning, FpsCamera::Projection, UserMasterVolume, Window) est
    // faite par les systèmes apply_* (event-driven Changed<UserSettings>, race-free).
    ui.label(egui::RichText::new("Sensibilité souris").size(16.0));
    let mut sens = settings.mouse_sensitivity;
    if ui
        .add(egui::Slider::new(&mut sens, 0.0005..=0.008).fixed_decimals(4))
        .changed()
    {
        settings.mouse_sensitivity = sens;
        dirty = true;
    }
    ui.add_space(10.0);

    ui.label(egui::RichText::new("Champ de vision (°)").size(16.0));
    let mut fov = settings.fov_deg;
    if ui
        .add(egui::Slider::new(&mut fov, 60.0..=120.0).fixed_decimals(0))
        .changed()
    {
        settings.fov_deg = fov;
        dirty = true;
    }
    ui.add_space(10.0);

    // Story-595 (M2-B1) : volume principal — applique en LIVE (canaux kira).
    ui.label(egui::RichText::new("Volume principal").size(16.0));
    let mut vol_pct = settings.master_volume * 100.0;
    if ui
        .add(egui::Slider::new(&mut vol_pct, 0.0..=100.0).fixed_decimals(0).suffix(" %"))
        .changed()
    {
        settings.master_volume = (vol_pct / 100.0).clamp(0.0, 1.0);
        dirty = true;
    }
    ui.add_space(10.0);

    // Story-595 : affichage — applique en LIVE (Window mutable runtime).
    ui.label(egui::RichText::new("Affichage").size(16.0));
    ui.horizontal(|ui| {
        let is_borderless = settings.window_mode == "borderless";
        if ui.selectable_label(!is_borderless, "Fenêtré").clicked() && is_borderless {
            settings.window_mode = "windowed".to_string();
            dirty = true;
        }
        if ui
            .selectable_label(is_borderless, "Plein écran (sans bordure)")
            .clicked()
            && !is_borderless
        {
            settings.window_mode = "borderless".to_string();
            dirty = true;
        }
    });
    if settings.window_mode == "windowed" {
        ui.horizontal(|ui| {
            for (w, h) in RESOLUTION_PRESETS {
                let selected = settings.window_width == w && settings.window_height == h;
                if ui.selectable_label(selected, format!("{w}×{h}")).clicked() && !selected {
                    settings.window_width = w;
                    settings.window_height = h;
                    dirty = true;
                }
            }
        });
    }
    ui.add_space(10.0);

    // Story-595 : touches affichées (lecture seule — réassignation = post-ship).
    ui.collapsing(egui::RichText::new("Touches (AZERTY)").size(16.0), |ui| {
        ui.label(
            egui::RichText::new(
                "Z Q S D — Se déplacer\n\
                 Espace — Sauter   ·   Espace ×2 — Bondir (dash)\n\
                 Maj gauche — Sprint\n\
                 Clic gauche — Tirer   ·   Clic droit — Viser\n\
                 R — Recharger   ·   E — Interagir\n\
                 Échap — Pause",
            )
            .size(14.0)
            .color(egui::Color32::from_gray(200)),
        );
        ui.label(
            egui::RichText::new("Réassignation des touches : à venir.")
                .size(12.0)
                .italics()
                .color(egui::Color32::from_gray(140)),
        );
    });
    ui.add_space(16.0);

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(egui::RichText::new("💾 Sauvegarder").size(18.0))
                    .min_size(egui::vec2(150.0, 36.0)),
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
                info!("[pause-menu] settings sauvés → {}", settings_path().display());
            }
        }
        if ui
            .add(
                egui::Button::new(egui::RichText::new("← Retour").size(18.0))
                    .min_size(egui::vec2(120.0, 36.0)),
            )
            .clicked()
        {
            state.sub = PauseSubMenu::Root;
            state.last_action = "settings_back".to_string();
        }
    });
    dirty
}

fn save_user_settings(settings: &UserSettings) -> bool {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match toml::to_string_pretty(settings) {
        Ok(content) => fs::write(&path, content).is_ok(),
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
        r#"{{"timestamp_secs":{:.2},"open":{},"sub_menu":"{:?}","open_count_session":{},"last_action":"{}","last_save_secs":{:.2},"last_save_success":{},"sensitivity":{:.4},"fov_deg":{:.1},"master_volume":{:.2},"window_mode":"{}"}}"#,
        now,
        matches!(app_state.get(), AppMode::Paused),
        state.sub,
        sensor.open_count_session,
        state.last_action.replace('"', "'"),
        sensor.last_save_secs,
        sensor.last_save_success,
        settings.mouse_sensitivity,
        settings.fov_deg,
        settings.master_volume,
        settings.window_mode,
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
                    // Story-595 (M2-B1) : propagation volume + fenêtre, même
                    // pattern event-driven Changed<UserSettings>.
                    apply_settings_to_volume,
                    apply_window_settings,
                    write_pause_menu_sensor,
                ),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Régression story-595 : un TOML legacy (pré-595, sans les nouveaux champs)
    /// doit charger avec les défauts — pas d'erreur, pas de valeur fantôme.
    #[test]
    fn legacy_settings_toml_parses_with_defaults() {
        let legacy = "mouse_sensitivity = 0.003\nfov_deg = 100.0\n";
        let s: UserSettings = toml::from_str(legacy).expect("TOML legacy valide");
        assert_eq!(s.mouse_sensitivity, 0.003);
        assert_eq!(s.fov_deg, 100.0);
        assert_eq!(s.master_volume, 1.0, "défaut volume = plein");
        assert_eq!(s.window_mode, "windowed", "défaut = comportement pré-595");
        assert_eq!((s.window_width, s.window_height), (1920, 1080));
    }

    #[test]
    fn settings_roundtrip_toml() {
        let mut s = UserSettings::default();
        s.master_volume = 0.35;
        s.window_mode = "borderless".to_string();
        let toml_str = toml::to_string_pretty(&s).expect("serialize");
        let back: UserSettings = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(back.master_volume, 0.35);
        assert_eq!(back.window_mode, "borderless");
    }

    /// Le chemin canonique vit sous APPDATA (jamais dans assets/ : read-only
    /// en install distribuée). Sous Windows de test, APPDATA est toujours set.
    #[test]
    fn settings_path_is_under_appdata_when_available() {
        if std::env::var_os("APPDATA").is_some() {
            let p = settings_path();
            assert!(p.ends_with("Forgia\\user_settings.toml") || p.ends_with("Forgia/user_settings.toml"));
            assert!(!p.starts_with("assets"));
        }
    }
}
