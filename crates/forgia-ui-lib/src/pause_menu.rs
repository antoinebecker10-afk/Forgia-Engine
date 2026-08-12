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

use crate::style::*;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_audio::{UserAudioVolumes, UserMasterVolume};
use forgia_core::prelude::*;
use forgia_player::{CameraFov, FpsCamera, MouseLookTuning};
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
    #[serde(default = "default_music_volume")]
    pub music_volume: f32,
    #[serde(default = "default_sfx_volume")]
    pub sfx_volume: f32,
    #[serde(default = "default_voice_volume")]
    pub voice_volume: f32,
    #[serde(default = "default_ambience_volume")]
    pub ambience_volume: f32,
    /// "windowed" | "borderless". Défaut = "borderless" depuis story-692 : la
    /// cible officielle (build-stack.md « 1920x1080 borderless ») — le défaut
    /// Windowed historique donnait une fenêtre que l'OS clampe à 1009 px de
    /// haut, jamais le 1080 du design. Un TOML explicite garde son choix.
    #[serde(default = "default_window_mode")]
    pub window_mode: String,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    /// Story-599 inc.1 — profil colorimétrique (tonemapping) :
    /// "tony" | "aces" | "agx" | "reinhard" | "none". Défaut = TonyMcMapface (défaut Bevy).
    #[serde(default = "default_tonemapping")]
    pub tonemapping: String,
    /// Story-599 inc.2 — niveau MSAA : 1 (Off) / 2 / 4 / 8. Défaut 4 (défaut Bevy).
    /// Appliqué UNIQUEMENT à la caméra FPS (Roguelite) : la caméra orbitale 3P
    /// (RPG/Cyber) casse son rendu si on change son MSAA à chaud (écran marron).
    #[serde(default = "default_msaa_samples")]
    pub msaa_samples: u32,
    /// Story-599 inc.3 — VSync (true = `PresentMode::AutoVsync`, false = `AutoNoVsync`). Défaut on.
    #[serde(default = "default_vsync")]
    pub vsync: bool,
    /// Story-678 Phase 2 — animations d'interface (transitions de sections,
    /// impulsions de boutons). Toggle accessibilité, miroité chaque frame vers
    /// `motion::set_motion_enabled`. Défaut on.
    #[serde(default = "default_ui_motion")]
    pub ui_motion_enabled: bool,
}

fn default_sensor_period() -> f32 {
    1.0
}
fn default_master_volume() -> f32 {
    1.0
}
fn default_music_volume() -> f32 {
    0.75
}
fn default_sfx_volume() -> f32 {
    1.0
}
fn default_voice_volume() -> f32 {
    1.0
}
fn default_ambience_volume() -> f32 {
    0.8
}
fn default_window_mode() -> String {
    "borderless".to_string()
}
fn default_window_width() -> u32 {
    1920
}
fn default_window_height() -> u32 {
    1080
}
fn default_tonemapping() -> String {
    "tony".to_string()
}
fn default_msaa_samples() -> u32 {
    4
}
fn default_vsync() -> bool {
    true
}
fn default_ui_motion() -> bool {
    true
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.002,
            fov_deg: 90.0,
            sensor_period_secs: 1.0,
            master_volume: 1.0,
            music_volume: default_music_volume(),
            sfx_volume: default_sfx_volume(),
            voice_volume: default_voice_volume(),
            ambience_volume: default_ambience_volume(),
            window_mode: default_window_mode(),
            window_width: 1920,
            window_height: 1080,
            tonemapping: "tony".to_string(),
            msaa_samples: 4,
            vsync: true,
            ui_motion_enabled: true,
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
    match read_user_settings_from_disk() {
        Some((loaded, source)) => {
            *settings = loaded;
            info!(
                "[pause-menu] settings chargés depuis {source} (sensibilité {:.4}, fov {:.0}°, volume {:.0}%)",
                settings.mouse_sensitivity,
                settings.fov_deg,
                settings.master_volume * 100.0
            );
        }
        None => {
            info!("[pause-menu] aucun user_settings.toml — défauts");
        }
    }
}

/// Lit `user_settings.toml` sur le disque, SANS ECS — la lecture unique que
/// partagent le boot Bevy (`load_user_settings_at_boot`) et la création de la
/// fenêtre initiale (story-692 : `forgia-game` lit le mode fenêtre AVANT de
/// bâtir la `Window`, sinon chaque lancement borderless flashe une fenêtre
/// 1920×1080 le temps qu'`apply_window_settings` rattrape). Chemin canonique
/// %APPDATA%, puis migration depuis l'ancien assets/ (le prochain Save écrira
/// le canonique). Rend aussi la source, pour les logs.
pub fn read_user_settings_from_disk() -> Option<(UserSettings, String)> {
    let canonical = settings_path();
    let (content, source) = match fs::read_to_string(&canonical) {
        Ok(c) => (c, canonical.display().to_string()),
        Err(_) => match fs::read_to_string(LEGACY_SETTINGS_PATH) {
            Ok(c) => (
                c,
                format!("{LEGACY_SETTINGS_PATH} (legacy, migration au prochain Save)"),
            ),
            Err(_) => return None,
        },
    };
    match toml::from_str::<UserSettings>(&content) {
        Ok(loaded) => Some((loaded, source)),
        Err(e) => {
            warn!("[pause-menu] user_settings.toml parse error: {e}");
            None
        }
    }
}

/// Le triplet (mode, largeur, hauteur) de la fenêtre INITIALE, dérivé des
/// settings disque — ou des défauts s'il n'y en a pas (story-692).
pub fn initial_window_config() -> (WindowMode, u32, u32) {
    let s = read_user_settings_from_disk()
        .map(|(s, _)| s)
        .unwrap_or_default();
    let mode = match s.window_mode.as_str() {
        "windowed" => WindowMode::Windowed,
        _ => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
    };
    (mode, s.window_width, s.window_height)
}

/// Propage `UserSettings.master_volume` → `forgia_audio::UserMasterVolume`
/// (appliqué aux canaux kira par forgia-audio). Event-driven + sync initiale.
pub fn apply_settings_to_volume(
    settings: Res<UserSettings>,
    mut volume: ResMut<UserMasterVolume>,
    mut mix: ResMut<UserAudioVolumes>,
    mut applied_once: Local<bool>,
) {
    if !settings.is_changed() && *applied_once {
        return;
    }
    volume.0 = settings.master_volume.clamp(0.0, 1.0);
    *mix = UserAudioVolumes {
        master: volume.0,
        music: settings.music_volume.clamp(0.0, 1.0),
        sfx: settings.sfx_volume.clamp(0.0, 1.0),
        voice: settings.voice_volume.clamp(0.0, 1.0),
        ambience: settings.ambience_volume.clamp(0.0, 1.0),
    };
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

/// Story-615 — propage `UserSettings.fov_deg` → `forgia_player::CameraFov` (FOV
/// hipfire). C'est désormais `apply_ads_camera_fov` (forgia-viewmodel) qui écrit le
/// `Projection` chaque frame en lerpant `CameraFov.hipfire_deg` → ADS. Avant, ce
/// système écrivait `Projection` directement et était clobbé chaque frame par l'ADS
/// (FOV joueur mort, bloqué à 45°). Event-driven (Changed) + sync initiale forcée.
pub fn apply_fov_to_camera(
    settings: Res<UserSettings>,
    mut camera_fov: ResMut<CameraFov>,
    mut applied_once: Local<bool>,
) {
    if !settings.is_changed() && *applied_once {
        return;
    }
    camera_fov.hipfire_deg = settings.fov_deg;
    *applied_once = true;
}

// ─── Story-599 inc.1 — profil colorimétrique (tonemapping) ────────────

/// Mappe la string genome-friendly → variante `Tonemapping`. Fallback = Forgia.
fn tonemapping_from_str(s: &str) -> Tonemapping {
    let wanted = match s {
        "aces" => Tonemapping::AcesFitted,
        "agx" => Tonemapping::AgX,
        "reinhard" => Tonemapping::ReinhardLuminance,
        "none" => Tonemapping::None,
        _ => Tonemapping::TonyMcMapface,
    };
    // wasm (story-695) : les tonemappers à LUT (KTX2 Rgba16Unorm, sans équivalent
    // WebGPU) sont neutralisés au boot par forgia-game — les appliquer quand même
    // rend l'image NOIRE. Résoudre ici, à la source du réglage : ce système
    // réapplique sa valeur à chaque frame sur toute Camera3d et gagnerait toute
    // bataille contre une garde aval (écran noir arène web, 2026-08-11).
    #[cfg(target_arch = "wasm32")]
    let wanted = match wanted {
        Tonemapping::TonyMcMapface | Tonemapping::AgX | Tonemapping::BlenderFilmic => {
            Tonemapping::AcesFitted
        }
        other => other,
    };
    wanted
}

/// `UserSettings.tonemapping` → composant `Tonemapping` sur toute `Camera3d`.
/// Idempotent (set-if-different) : couvre le changement de réglage ET les
/// caméras spawnées plus tard (par-mode), sans boucle de change-detection.
/// `Tonemapping` est déjà un composant requis de `Camera3d` (défaut
/// TonyMcMapface) — on ne fait que changer sa valeur, aucun ajout de pipeline.
pub fn apply_tonemapping_to_cameras(
    settings: Res<UserSettings>,
    mut q_cam: Query<&mut Tonemapping, With<Camera3d>>,
) {
    let target = tonemapping_from_str(&settings.tonemapping);
    for mut tm in &mut q_cam {
        if *tm != target {
            *tm = target;
        }
    }
}

// ─── Story-599 inc.2 — anti-aliasing MSAA (caméra FPS UNIQUEMENT) ──────

/// `UserSettings.msaa_samples` → composant `Msaa` sur la **caméra FPS** seule.
///
/// ⚠️ Gaté `With<FpsCamera>` (PAS `Camera3d`) : changer le MSAA de la caméra
/// orbitale 3P (RPG/Cyber) à chaud casse son rendu (écran marron — confirmé par
/// isolation 2026-06-16, même fragilité que TAA/SMAA/HDR-Bloom story-550). La
/// caméra FPS (mode ship Roguelite) tolère le changement. En RPG/Cyber la
/// FpsCamera est inactive → set sans effet visuel, l'orbitale garde son défaut.
/// Idempotent (set-if-different).
pub fn apply_msaa_to_cameras(
    settings: Res<UserSettings>,
    // `Without<ScreenSpaceAmbientOcclusion>` : ne PAS écraser le `Msaa::Off` forcé
    // par le SSAO (render_quality, Roguelite) — SSAO+MSAA = incompatible Bevy. Dès
    // que le SSAO est retiré (OnExit Roguelite / config off), la cam re-match ici
    // et le MSAA utilisateur est restauré automatiquement.
    mut q_cam: Query<&mut Msaa, (With<FpsCamera>, Without<ScreenSpaceAmbientOcclusion>)>,
) {
    let target = match settings.msaa_samples {
        1 => Msaa::Off,
        2 => Msaa::Sample2,
        8 => Msaa::Sample8,
        _ => Msaa::Sample4,
    };
    for mut msaa in &mut q_cam {
        if *msaa != target {
            *msaa = target;
        }
    }
}

// ─── Story-599 inc.3 — VSync ──────────────────────────────────────────

/// `UserSettings.vsync` → `Window.present_mode` (AutoVsync ↔ AutoNoVsync).
/// Touche la fenêtre, pas les caméras → aucun risque de casser un rendu de
/// caméra. Idempotent (set-if-different). Off = FPS débridé (tearing possible).
pub fn apply_vsync_to_window(
    settings: Res<UserSettings>,
    mut q_window: Query<&mut Window, With<PrimaryWindow>>,
) {
    let target = if settings.vsync {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    };
    if let Ok(mut window) = q_window.single_mut() {
        if window.present_mode != target {
            window.present_mode = target;
        }
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
            crate::style::glass_frame_hero()
                .fill(FORGE_PANEL)
                .inner_margin(egui::Margin::symmetric(56, 36))
                .corner_radius(egui::CornerRadius::same(18))
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
    ui.label(crate::theme::display_text("PAUSE", 52.0, C_PRIMARY));
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

/// Dessine les contrôles de réglages (sensibilité / FOV / volume / affichage /
/// tonemapping / MSAA / VSync / aide touches). Retourne `true` si un contrôle a
/// RÉELLEMENT changé — le caller déclenche alors `set_changed()` (cf anti-crash
/// dans draw_pause_menu). Partagé entre le pause menu (in-game) et la page
/// Options du menu titre (forgia-ui) → source unique des réglages (DRY).
pub fn draw_settings_controls(ui: &mut egui::Ui, settings: &mut UserSettings) -> bool {
    let mut dirty = false;

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

    for (label, value) in [
        ("Musique", &mut settings.music_volume),
        ("Effets sonores", &mut settings.sfx_volume),
        ("Voix", &mut settings.voice_volume),
        ("Ambiance", &mut settings.ambience_volume),
    ] {
        ui.label(egui::RichText::new(label).size(14.0));
        let mut pct = *value * 100.0;
        if ui
            .add(
                egui::Slider::new(&mut pct, 0.0..=100.0)
                    .fixed_decimals(0)
                    .suffix(" %"),
            )
            .changed()
        {
            *value = (pct / 100.0).clamp(0.0, 1.0);
            dirty = true;
        }
    }
    ui.add_space(10.0);

    // Story-595 (M2-B1) : volume principal — applique en LIVE (canaux kira).
    ui.label(egui::RichText::new("Volume principal").size(16.0));
    let mut vol_pct = settings.master_volume * 100.0;
    if ui
        .add(
            egui::Slider::new(&mut vol_pct, 0.0..=100.0)
                .fixed_decimals(0)
                .suffix(" %"),
        )
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

    // Story-599 inc.1 — profil colorimétrique (tonemapping). Composant déjà
    // présent sur Camera3d → on change juste la valeur (sûr, ≠ post-process).
    ui.label(egui::RichText::new("Profil colorimétrique").size(16.0));
    ui.horizontal(|ui| {
        for (label, key) in [
            ("Forgia", "tony"),
            ("ACES", "aces"),
            ("AgX", "agx"),
            ("Doux", "reinhard"),
            ("Neutre", "none"),
        ] {
            let selected = settings.tonemapping == key;
            if ui.selectable_label(selected, label).clicked() && !selected {
                settings.tonemapping = key.to_string();
                dirty = true;
            }
        }
    });
    ui.add_space(10.0);

    // Story-599 inc.2 — anti-aliasing (MSAA). Appliqué à la caméra FPS (Roguelite).
    ui.label(egui::RichText::new("Anti-aliasing (MSAA)").size(16.0));
    ui.horizontal(|ui| {
        for (label, samples) in [("Off", 1u32), ("2×", 2), ("4×", 4), ("8×", 8)] {
            let selected = settings.msaa_samples == samples;
            if ui.selectable_label(selected, label).clicked() && !selected {
                settings.msaa_samples = samples;
                dirty = true;
            }
        }
    });
    ui.add_space(10.0);

    // Story-599 inc.3 — VSync.
    ui.label(egui::RichText::new("VSync").size(16.0));
    ui.horizontal(|ui| {
        if ui.selectable_label(settings.vsync, "Activé").clicked() && !settings.vsync {
            settings.vsync = true;
            dirty = true;
        }
        if ui.selectable_label(!settings.vsync, "Désactivé").clicked() && settings.vsync {
            settings.vsync = false;
            dirty = true;
        }
    });
    ui.add_space(10.0);

    // Story-678 Phase 2 — accessibilité : le motion d'interface se coupe.
    // Le miroir vers la mémoire egui est immédiat (même frame), la persistance
    // passe par le même « Sauvegarder » que le reste.
    let mut motion = settings.ui_motion_enabled;
    if ui
        .checkbox(
            &mut motion,
            egui::RichText::new("Animations du menu (transitions, impulsions)").size(16.0),
        )
        .changed()
    {
        settings.ui_motion_enabled = motion;
        crate::motion::set_motion_enabled(ui.ctx(), motion);
        dirty = true;
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

    dirty
}

/// Panneau OPTIONS du pause menu : heading + contrôles partagés
/// (`draw_settings_controls`) + boutons Sauvegarder / Retour. Retourne `true`
/// si un contrôle a changé (le caller déclenche `set_changed()`).
fn draw_settings(
    ui: &mut egui::Ui,
    state: &mut PauseMenuState,
    settings: &mut UserSettings,
    sensor: &mut PauseMenuSensor,
    now: f32,
) -> bool {
    ui.heading(
        egui::RichText::new("OPTIONS")
            .size(40.0)
            .color(C_PRIMARY)
            .strong(),
    );
    ui.add_space(16.0);

    let dirty = draw_settings_controls(ui, settings);
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
                info!(
                    "[pause-menu] settings sauvés → {}",
                    settings_path().display()
                );
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

pub fn save_user_settings(settings: &UserSettings) -> bool {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match toml::to_string_pretty(settings) {
        // Écriture atomique tmp+rename (fix audit 2026-07-19) : un crash
        // mi-écriture ne doit jamais corrompre le seul save utilisateur non
        // versionné par run. Même pattern que `persist.rs` (roguelite).
        Ok(content) => {
            let tmp = path.with_extension("toml.tmp");
            if fs::write(&tmp, content).is_err() {
                return false;
            }
            match fs::rename(&tmp, &path) {
                Ok(()) => true,
                Err(e) => {
                    warn!("[pause-menu] save rename error: {e}");
                    let _ = fs::remove_file(&tmp);
                    false
                }
            }
        }
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
    // Story-599 inc.2 — MSAA RÉEL de la caméra FPS (≠ valeur du réglage) →
    // confirme que le réglage atteint le rendu (MSAA subtil à l'œil).
    q_cam_msaa: Query<&Msaa, With<FpsCamera>>,
    // Story-599 inc.3 — present_mode RÉEL de la fenêtre (confirme le VSync).
    q_window: Query<&Window, With<PrimaryWindow>>,
) {
    let now = time.elapsed_secs();
    if now - sensor.last_write_secs < settings.sensor_period_secs.max(0.1) {
        return;
    }
    sensor.last_write_secs = now;
    let cam_msaa: Vec<u32> = q_cam_msaa.iter().map(|m| m.samples()).collect();
    let present_mode = q_window
        .single()
        .map(|w| format!("{:?}", w.present_mode))
        .unwrap_or_else(|_| "Unknown".to_string());
    let json = format!(
        r#"{{"timestamp_secs":{:.2},"open":{},"sub_menu":"{:?}","open_count_session":{},"last_action":"{}","last_save_secs":{:.2},"last_save_success":{},"sensitivity":{:.4},"fov_deg":{:.1},"master_volume":{:.2},"window_mode":"{}","tonemapping":"{}","msaa_samples":{},"fps_camera_msaa_actual":{:?},"vsync":{},"present_mode_actual":"{}"}}"#,
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
        settings.tonemapping,
        settings.msaa_samples,
        cam_msaa,
        settings.vsync,
        present_mode,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia_pause_menu.json", json);
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
                    // Story-615 — écrit CameraFov (préf joueur). Plus de run_if InGame :
                    // doit être prêt avant le spawn caméra (apply_ads_camera_fov lit la base).
                    apply_fov_to_camera,
                    // Story-595 (M2-B1) : propagation volume + fenêtre, même
                    // pattern event-driven Changed<UserSettings>.
                    apply_settings_to_volume,
                    apply_window_settings,
                    // Story-599 inc.1 — profil colorimétrique (idempotent, couvre
                    // les caméras spawnées par-mode).
                    apply_tonemapping_to_cameras,
                    // Story-599 inc.2 — MSAA caméra FPS (idempotent).
                    apply_msaa_to_cameras,
                    // Story-599 inc.3 — VSync (Window.present_mode, idempotent).
                    apply_vsync_to_window,
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
        // Story-599 inc.1 : tonemapping absent du TOML legacy → défaut Forgia.
        assert_eq!(s.tonemapping, "tony", "défaut tonemapping = TonyMcMapface");
        // Story-599 inc.2 : msaa absent du TOML legacy → défaut 4×.
        assert_eq!(s.msaa_samples, 4, "défaut MSAA = 4×");
        // Story-599 inc.3 : vsync absent du TOML legacy → défaut on.
        assert!(s.vsync, "défaut VSync = on");
        assert_eq!(s.master_volume, 1.0, "défaut volume = plein");
        assert_eq!(s.music_volume, 0.75);
        assert_eq!(s.sfx_volume, 1.0);
        assert_eq!(s.voice_volume, 1.0);
        assert_eq!(s.ambience_volume, 0.8);
        // Story-692 : le défaut est la cible officielle (borderless) — le
        // Windowed pré-595 donnait une fenêtre clampée à 1009 px par l'OS.
        assert_eq!(s.window_mode, "borderless", "défaut = cible officielle");
        assert_eq!((s.window_width, s.window_height), (1920, 1080));
    }

    #[test]
    fn tonemapping_str_mapping_and_fallback() {
        assert_eq!(tonemapping_from_str("aces"), Tonemapping::AcesFitted);
        assert_eq!(tonemapping_from_str("agx"), Tonemapping::AgX);
        assert_eq!(
            tonemapping_from_str("reinhard"),
            Tonemapping::ReinhardLuminance
        );
        assert_eq!(tonemapping_from_str("none"), Tonemapping::None);
        assert_eq!(tonemapping_from_str("tony"), Tonemapping::TonyMcMapface);
        // Inconnu → fallback Forgia (TonyMcMapface), jamais un panic.
        assert_eq!(tonemapping_from_str("garbage"), Tonemapping::TonyMcMapface);
    }

    #[test]
    fn settings_roundtrip_toml() {
        let s = UserSettings {
            master_volume: 0.35,
            music_volume: 0.4,
            window_mode: "borderless".to_string(),
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&s).expect("serialize");
        let back: UserSettings = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(back.master_volume, 0.35);
        assert_eq!(back.music_volume, 0.4);
        assert_eq!(back.window_mode, "borderless");
    }

    /// Le chemin canonique vit sous APPDATA (jamais dans assets/ : read-only
    /// en install distribuée). Sous Windows de test, APPDATA est toujours set.
    #[test]
    fn settings_path_is_under_appdata_when_available() {
        if std::env::var_os("APPDATA").is_some() {
            let p = settings_path();
            assert!(
                p.ends_with("Forgia\\user_settings.toml")
                    || p.ends_with("Forgia/user_settings.toml")
            );
            assert!(!p.starts_with("assets"));
        }
    }
}
