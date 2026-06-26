//! identity.rs — Identité joueur (story-623 Phase E, MVP) : **nom + couleur**.
//!
//! Le joueur a un nom (généré par défaut au boot, éditable SANS friction via un
//! bouton crayon non-bloquant) et une couleur cosmétique, choisis au Lobby APRÈS la
//! 1re run (principe P7 : jouer avant de nommer, jamais de modal forcé).
//!
//! Auto-contenu (coordination multi-terminal) : save SÉPARÉE `identity_save.toml`
//! (pattern `ftue.rs`, ne couple pas `MetaShopSave`), presets data-driven
//! `assets/genomes/roguelite/roguelite_identity.toml` (no-hardcode, hot-éditable),
//! panneau egui propre, sensor `forgia2_identity.json` (observability-required).
//!
//! Sélection, jamais création (P6) : pas de sliders, presets cliquables (kid-friendly,
//! pas de clavier obligatoire). Couleur = cosmétique pur (zéro stat, P6/P8).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::RunState;

const SAVE_FILE: &str = "identity_save.toml";
const SAVE_VERSION: u32 = 1;
const PRESETS_PATH: &str = "assets/genomes/roguelite/roguelite_identity.toml";
const SENSOR_PATH: &str = "forgia2_identity.json";

// ── Presets data-driven (no-hardcode P5) ────────────────────────────────────

/// Un preset de couleur cosmétique (RGB sRGB). `souls_cost` réservé backlog (MVP=0).
#[derive(Deserialize, Clone, Debug)]
pub struct ColorPreset {
    pub id: String,
    pub label: String,
    pub rgb: [f32; 3],
}

#[derive(Deserialize, Clone, Debug)]
pub struct NamePreset {
    pub label: String,
}

/// Catalogue identité chargé depuis TOML (noms + couleurs proposés au Lobby).
#[derive(Resource, Deserialize, Clone, Debug)]
pub struct IdentityConfig {
    pub default_name: String,
    #[serde(default)]
    pub name_presets: Vec<NamePreset>,
    #[serde(default)]
    pub colors: Vec<ColorPreset>,
}

impl Default for IdentityConfig {
    /// Fallback cosmétique si le TOML manque (contenu, pas du gameplay chiffré —
    /// exception layout/cosmétique de no-hardcode ; le TOML reste la source éditable).
    fn default() -> Self {
        Self {
            default_name: "Forgeron Écarlate".to_string(),
            name_presets: ["Forgeron Écarlate", "Petit Marteau", "Braise", "Enclumette", "Cendre"]
                .iter()
                .map(|l| NamePreset { label: l.to_string() })
                .collect(),
            colors: vec![
                ("default", "Apprenti", [0.80, 0.50, 0.20]),
                ("azur", "Azur", [0.20, 0.45, 0.90]),
                ("emeraude", "Émeraude", [0.20, 0.70, 0.40]),
                ("pourpre", "Pourpre", [0.70, 0.20, 0.55]),
            ]
            .into_iter()
            .map(|(id, label, rgb)| ColorPreset {
                id: id.to_string(),
                label: label.to_string(),
                rgb,
            })
            .collect(),
        }
    }
}

impl IdentityConfig {
    fn load() -> Self {
        match std::fs::read_to_string(PRESETS_PATH) {
            Ok(c) => toml::from_str(&c).unwrap_or_else(|e| {
                warn!("[identity] {PRESETS_PATH} parse error: {e} — défauts");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    fn color_rgb(&self, id: &str) -> [f32; 3] {
        self.colors
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.rgb)
            .unwrap_or([0.8, 0.5, 0.2])
    }
}

// ── Save persistée (séparée du shop, P10 profil unique) ──────────────────────

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct IdentitySave {
    pub version: u32,
    pub player_name: String,
    /// L'utilisateur a-t-il édité son nom volontairement ? (≠ garder le défaut, funnel)
    pub name_edited: bool,
    pub equipped_color: String,
    #[serde(default)]
    pub unlocked_colors: Vec<String>,
}

impl Default for IdentitySave {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            player_name: String::new(), // rempli au boot depuis IdentityConfig.default_name
            name_edited: false,
            equipped_color: "default".to_string(),
            unlocked_colors: vec!["default".to_string()],
        }
    }
}

fn config_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let mut cursor = exe.parent();
        while let Some(d) = cursor {
            if d.join("config").join("biomes").exists() {
                return d.join("config");
            }
            cursor = d.parent();
        }
    }
    PathBuf::from("config")
}

impl IdentitySave {
    fn save_path() -> PathBuf {
        config_dir().join(SAVE_FILE)
    }

    fn load_or_default() -> Self {
        match std::fs::read_to_string(Self::save_path()) {
            Ok(c) => toml::from_str(&c).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    fn save(&self) {
        crate::persist::save_toml_atomic(&Self::save_path(), self, "identity");
    }
}

// ── Systèmes ─────────────────────────────────────────────────────────────────

/// Boot : si aucun nom (1re partie), attribue le nom par défaut SILENCIEUSEMENT
/// (P7 : pas de prompt). Débloque toutes les couleurs MVP (gratuites).
fn sys_init_identity(mut save: ResMut<IdentitySave>, cfg: Res<IdentityConfig>) {
    let mut dirty = false;
    if save.player_name.trim().is_empty() {
        save.player_name = cfg.default_name.clone();
        dirty = true;
    }
    // MVP : toutes les couleurs sont gratuites → débloquées d'office.
    for c in &cfg.colors {
        if !save.unlocked_colors.contains(&c.id) {
            save.unlocked_colors.push(c.id.clone());
            dirty = true;
        }
    }
    if !save.unlocked_colors.contains(&save.equipped_color) {
        save.equipped_color = "default".to_string();
        dirty = true;
    }
    if dirty {
        save.save();
    }
}

/// Panneau d'identité au Lobby (non-bloquant, P7). Nom + bouton crayon (presets
/// cliquables + texte optionnel) + pastilles couleur. `Local<bool>` = éditeur ouvert.
fn draw_identity_panel(
    mut contexts: EguiContexts,
    run_state: Option<Res<State<RunState>>>,
    cfg: Res<IdentityConfig>,
    mut save: ResMut<IdentitySave>,
    mut editing: Local<bool>,
    mut shown: ResMut<IdentityPanelShown>,
) {
    if !matches!(run_state.as_deref().map(|s| s.get()), Some(RunState::Lobby)) {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    shown.0 = true;

    let rgb = cfg.color_rgb(&save.equipped_color);
    let name_col = egui::Color32::from_rgb(
        (rgb[0] * 255.0) as u8,
        (rgb[1] * 255.0) as u8,
        (rgb[2] * 255.0) as u8,
    );

    // Home-hub P2.1 — onglet FORGE : panneau CENTRÉ (le hub gère les onglets).
    // Titre « TON FORGERON » = onglet hub.
    egui::Area::new(egui::Id::new("forgia_identity_panel"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 10.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(180))
                .inner_margin(egui::Margin::symmetric(22, 18))
                .corner_radius(egui::CornerRadius::same(10))
                .show(ui, |ui| {
                    ui.set_min_width(320.0);
                    // Pastille couleur + nom coloré.
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
                        ui.painter().rect_filled(r, egui::CornerRadius::same(4), name_col);
                        ui.label(egui::RichText::new(&save.player_name).size(22.0).strong().color(name_col));
                        if ui.button("✏").on_hover_text("Changer le nom").clicked() {
                            *editing = !*editing;
                        }
                    });

                    // Éditeur de nom (presets cliquables + texte optionnel).
                    if *editing {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Choisis un nom :").size(13.0));
                        ui.horizontal_wrapped(|ui| {
                            for p in &cfg.name_presets {
                                if ui.button(&p.label).clicked() {
                                    save.player_name = p.label.clone();
                                    save.name_edited = true;
                                    save.save();
                                }
                            }
                        });
                        let mut typed = save.player_name.clone();
                        if ui.add(egui::TextEdit::singleline(&mut typed).hint_text("…ou tape le tien").char_limit(20)).changed() {
                            save.player_name = typed;
                            save.name_edited = true;
                            // Save léger : sur perte de focus serait idéal ; ici save sur changement (rare).
                            save.save();
                        }
                    }

                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Couleur :").size(13.0));
                    ui.horizontal_wrapped(|ui| {
                        // Clone des ids débloqués pour éviter l'emprunt simultané de `save`.
                        let unlocked = save.unlocked_colors.clone();
                        let equipped = save.equipped_color.clone();
                        for c in &cfg.colors {
                            if !unlocked.contains(&c.id) {
                                continue;
                            }
                            let col = egui::Color32::from_rgb(
                                (c.rgb[0] * 255.0) as u8,
                                (c.rgb[1] * 255.0) as u8,
                                (c.rgb[2] * 255.0) as u8,
                            );
                            let selected = c.id == equipped;
                            let size = if selected { 30.0 } else { 24.0 };
                            let (r, resp) =
                                ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
                            ui.painter().rect_filled(r, egui::CornerRadius::same(5), col);
                            if selected {
                                ui.painter().rect_stroke(
                                    r,
                                    egui::CornerRadius::same(5),
                                    egui::Stroke::new(2.5, egui::Color32::WHITE),
                                    egui::StrokeKind::Outside,
                                );
                            }
                            if resp.on_hover_text(&c.label).clicked() {
                                save.equipped_color = c.id.clone();
                                save.save();
                            }
                        }
                    });
                });
        });
}

/// Flag « panneau identité affiché » (pour le health check du sensor).
#[derive(Resource, Default)]
pub struct IdentityPanelShown(pub bool);

/// Sensor `forgia2_identity.json` 1Hz (observability-required). Health check :
/// `IDENTITY_EDIT_UNREACHABLE` si le panneau n'a jamais été montré (édition
/// inaccessible) — PAS « jamais nommé » (garder le défaut est légitime, P7).
fn sys_write_identity_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    save: Res<IdentitySave>,
    cfg: Res<IdentityConfig>,
    shown: Res<IdentityPanelShown>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    let named = !save.player_name.trim().is_empty();
    let (severity, next_step) = if named && !shown.0 && time.elapsed_secs() > 60.0 {
        (
            "warn",
            "panneau identite jamais affiche au Lobby — edition du nom/couleur inaccessible",
        )
    } else {
        ("ok", "")
    };
    let json = format!(
        r#"{{"id":"identity","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"named":{},"name_edited":{},"name_len":{},"equipped_color":"{}","unlocked_colors_count":{},"colors_total":{},"panel_shown":{}}}"#,
        time.elapsed_secs(),
        named,
        save.name_edited,
        save.player_name.chars().count(),
        save.equipped_color,
        save.unlocked_colors.len(),
        cfg.colors.len(),
        shown.0,
    );
    let _ = std::fs::write(SENSOR_PATH, json);
}

pub struct IdentityPlugin;

impl Plugin for IdentityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(IdentityConfig::load())
            .insert_resource(IdentitySave::load_or_default())
            .init_resource::<IdentityPanelShown>()
            .add_systems(Startup, sys_init_identity)
            // Hub à onglets (P2) : le panneau forgeron ne s'affiche que sur l'onglet FORGE.
            .add_systems(
                EguiPrimaryContextPass,
                draw_identity_panel.run_if(crate::hub::on_forge_tab),
            )
            .add_systems(Update, sys_write_identity_sensor.in_set(GameSet::Sensors));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_default_unnamed_then_filled() {
        let s = IdentitySave::default();
        assert!(s.player_name.is_empty());
        assert_eq!(s.equipped_color, "default");
        assert!(s.unlocked_colors.contains(&"default".to_string()));
        assert_eq!(s.version, SAVE_VERSION);
    }

    #[test]
    fn config_default_has_presets_and_colors() {
        let c = IdentityConfig::default();
        assert!(!c.default_name.is_empty());
        assert!(c.name_presets.len() >= 3);
        assert!(c.colors.iter().any(|c| c.id == "default"));
    }

    #[test]
    fn color_rgb_lookup_with_fallback() {
        let c = IdentityConfig::default();
        assert_eq!(c.color_rgb("azur"), [0.20, 0.45, 0.90]);
        // id inconnu → fallback (pas de panic).
        let fb = c.color_rgb("inconnu");
        assert_eq!(fb, [0.8, 0.5, 0.2]);
    }

    #[test]
    fn save_roundtrip_toml() {
        let mut s = IdentitySave::default();
        s.player_name = "Braise".into();
        s.name_edited = true;
        s.equipped_color = "azur".into();
        s.unlocked_colors.push("azur".into());
        let ser = toml::to_string_pretty(&s).unwrap();
        let de: IdentitySave = toml::from_str(&ser).unwrap();
        assert_eq!(de.player_name, "Braise");
        assert!(de.name_edited);
        assert_eq!(de.equipped_color, "azur");
    }

    /// Migration : un save legacy sans `unlocked_colors` charge avec défaut (serde default).
    #[test]
    fn legacy_save_missing_unlocked_colors() {
        let legacy = "version = 1\nplayer_name = \"Cendre\"\nname_edited = false\nequipped_color = \"default\"\n";
        let s: IdentitySave = toml::from_str(legacy).unwrap();
        assert_eq!(s.player_name, "Cendre");
        assert!(s.unlocked_colors.is_empty()); // défaut serde ; sys_init_identity repeuplera
    }
}
