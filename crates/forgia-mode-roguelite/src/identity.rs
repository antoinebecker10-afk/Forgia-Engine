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
use bevy_egui::egui;
use forgia_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
            name_presets: [
                "Forgeron Écarlate",
                "Petit Marteau",
                "Braise",
                "Enclumette",
                "Cendre",
            ]
            .iter()
            .map(|l| NamePreset {
                label: l.to_string(),
            })
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

    /// Teinte d'un preset. Publique : le HUD en jeu colore le pseudo avec.
    pub fn color_rgb(&self, id: &str) -> [f32; 3] {
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
    /// Cosmétique des bras procéduraux (onglet Forge, P3) : id couleur (réf
    /// `cfg.colors`) + style (`peau`/`gantelet`/`cyber`).
    #[serde(default = "default_arm_color")]
    pub arm_color: String,
    #[serde(default = "default_arm_style")]
    pub arm_style: String,
    /// Story-678 — le DÉCOR du menu affiché, id du catalogue `cosmetics`.
    ///
    /// Même forme que `equipped_color` / `unlocked_colors` juste au-dessus :
    /// c'est le patron des cosmétiques du projet, on ne s'en écarte pas.
    #[serde(default = "default_backdrop")]
    pub equipped_backdrop: String,
    /// Le morceau qui joue au hub — clé de piste (`hub`, `chapter_02`…).
    #[serde(default = "default_hub_music")]
    pub hub_music: String,
    /// Décors ACHETÉS ou accordés — seulement ceux-là.
    ///
    /// Les cosmétiques gagnés en battant un chapitre ne sont PAS stockés : ils
    /// se dérivent de `chapters_cleared` (cf. `cosmetics::OwnedCosmetics`). Les
    /// stocker ferait deux vérités de la même chose, qui divergeraient dès
    /// qu'on renumérote un chapitre.
    #[serde(default)]
    pub unlocked_backdrops: Vec<String>,
    /// Bras achetés ou accordés (ids d'article du catalogue).
    #[serde(default)]
    pub unlocked_arms: Vec<String>,
    /// Musiques de hub achetées ou accordées (ids d'article).
    #[serde(default)]
    pub unlocked_music: Vec<String>,
}

fn default_backdrop() -> String {
    crate::cosmetics::FALLBACK_BACKDROP.to_string()
}

fn default_hub_music() -> String {
    "hub".to_string()
}

fn default_arm_color() -> String {
    "default".to_string()
}
fn default_arm_style() -> String {
    "peau".to_string()
}

impl Default for IdentitySave {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            player_name: String::new(), // rempli au boot depuis IdentityConfig.default_name
            name_edited: false,
            equipped_color: "default".to_string(),
            unlocked_colors: vec!["default".to_string()],
            arm_color: default_arm_color(),
            arm_style: default_arm_style(),
            equipped_backdrop: default_backdrop(),
            hub_music: default_hub_music(),
            unlocked_backdrops: Vec::new(),
            unlocked_arms: Vec::new(),
            unlocked_music: Vec::new(),
        }
    }
}

impl IdentitySave {
    /// Écrit la sauvegarde sur le disque.
    ///
    /// Story-678 : exposé pour `cosmetics.rs`, qui mute les stocks cosmétiques
    /// (couleurs, décors, bras, musiques) et doit pouvoir les persister. La
    /// mécanique d'écriture, elle, reste ici — c'est ce module qui possède le
    /// fichier.
    pub fn persist(&self) {
        self.save();
    }

    fn save_path() -> PathBuf {
        crate::persist::save_dir().join(SAVE_FILE)
    }

    fn load_or_default() -> Self {
        crate::persist::load_toml_migrating(SAVE_FILE)
    }

    fn save(&self) {
        crate::persist::save_toml_atomic(&Self::save_path(), self, "identity");
    }
}

// ── Systèmes ─────────────────────────────────────────────────────────────────

/// Boot : si aucun nom (1re partie), attribue le nom par défaut SILENCIEUSEMENT
/// (P7 : pas de prompt). Débloque les couleurs gratuites — c'est-à-dire celles
/// que le Marketplace ne gouverne pas (story-678).
fn sys_init_identity(
    mut save: ResMut<IdentitySave>,
    cfg: Res<IdentityConfig>,
    mut arm_cosmetics: ResMut<ArmCosmetics>,
    // Ordonné APRÈS `sys_init_cosmetics` (cf. le plugin) : sans le catalogue, on
    // retomberait sur l'ancien comportement et on offrirait tout.
    cosmetics: Option<Res<crate::cosmetics::CosmeticsConfig>>,
) {
    let mut dirty = false;
    if save.player_name.trim().is_empty() {
        save.player_name = cfg.default_name.clone();
        dirty = true;
    }
    // Les couleurs étaient TOUTES offertes au boot (« MVP : gratuites »).
    //
    // Story-678 : celles que le Marketplace vend ou conditionne à un chapitre ne
    // le sont plus — sinon l'onglet Couleurs afficherait un prix pour ce que
    // cette boucle vient de donner. Le catalogue est l'autorité ; une couleur
    // qu'il ne liste pas reste gratuite, donc aucune couleur existante ne
    // disparaît par simple omission. Et rien n'est RETIRÉ d'une sauvegarde : on
    // n'ajoute pas, on ne retranche jamais.
    for c in &cfg.colors {
        if cosmetics
            .as_deref()
            .is_some_and(|cat| cat.color_is_governed(&c.id))
        {
            continue;
        }
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
    // Applique la cosmétique bras persistée (couleur + style) au boot.
    arm_cosmetics.color = cfg.color_rgb(&save.arm_color);
    arm_cosmetics.style = ArmStyle::from_key(&save.arm_style);
}

/// Contenu du panneau d'identité (nom + couleurs + bras) rendu dans un `Ui` donné.
/// PARTAGÉ avec le hub-menu (`forgia-ui`) — zéro duplication. Le panneau Lobby
/// dédié (`draw_identity_panel`) a été retiré (story-694 : le Lobby est un gate
/// auto-start couvert d'un overlay de chargement, jamais vu par personne).
/// Mute `save` / `arm_cosmetics` + sauve sur disque. `editing` = état
/// (ouvert/fermé) de l'éditeur de nom (un `Local<bool>` par appelant).
pub fn draw_identity_content(
    ui: &mut egui::Ui,
    cfg: &IdentityConfig,
    save: &mut IdentitySave,
    arm_cosmetics: &mut ArmCosmetics,
    editing: &mut bool,
) {
    let rgb = cfg.color_rgb(&save.equipped_color);
    let name_col = egui::Color32::from_rgb(
        (rgb[0] * 255.0) as u8,
        (rgb[1] * 255.0) as u8,
        (rgb[2] * 255.0) as u8,
    );

    // Pastille couleur + nom coloré + bouton crayon (édition non-bloquante, P7).
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(r, egui::CornerRadius::same(4), name_col);
        ui.label(
            egui::RichText::new(save.player_name.as_str())
                .size(22.0)
                .strong()
                .color(name_col),
        );
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
        if ui
            .add(
                egui::TextEdit::singleline(&mut typed)
                    .hint_text("…ou tape le tien")
                    .char_limit(20),
            )
            .changed()
        {
            save.player_name = typed;
            save.name_edited = true;
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
            let (r, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
            ui.painter()
                .rect_filled(r, egui::CornerRadius::same(5), col);
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
                // La combinaison des bras suit la couleur du joueur : une seule
                // décision de couleur, appliquée partout où elle a du sens.
                save.arm_color = c.id.clone();
                arm_cosmetics.color = c.rgb;
                save.save();
            }
        }
    });

    // ── Cosmétique des BRAS — plus AUCUN sélecteur ici ──
    //
    // « Bras — couleur » avait déjà été retiré : les bras du viewmodel sont ceux
    // du personnage, dont les plaques prennent la rareté de l'équipement
    // (`ArmCosmetics.armor_rgb`). La couleur suit donc celle du joueur.
    //
    // « Bras — style » (Peau/Gantelet/Cyber) est retiré à son tour (2026-08-05,
    // demande en jeu) : sur un écran qui montre déjà le personnage ÉQUIPÉ, un
    // réglage de peau d'avant-bras est un choix qu'on ne voit pas et qui n'a
    // rien à décider. Le style reste piloté par la valeur persistée
    // (`IdentitySave.arm_style`), appliquée au boot par `sys_load_identity` —
    // le rendu (`forgia-viewmodel::arms`) est inchangé.
}

/// Flag « édition d'identité affichée » (pour le health check du sensor).
///
/// Posé par la fiche Forgeron du hub-menu (`forgia-ui`) — l'unique surface
/// d'édition depuis que le panneau Lobby a été retiré (story-694 : le Lobby
/// est un gate auto-start couvert d'un overlay, son panneau ne s'affichait
/// pour personne — c'était précisément ce que ce capteur détectait).
#[derive(Resource, Default)]
pub struct IdentityPanelShown(pub bool);

/// Sensor `forgia2_identity.json` 1Hz (observability-required). Health check :
/// `IDENTITY_EDIT_UNREACHABLE` si l'édition n'a jamais été montrée (fiche
/// Forgeron du menu) — PAS « jamais nommé » (garder le défaut est légitime, P7).
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
            "edition du nom/couleur jamais affichee (fiche Forgeron du menu) — \
             verifier MenuPage::Forgeron et sys_mark_identity_shown (forgia-ui)",
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
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

/// Miroir `IdentitySave` → `ArmCosmetics` (story-678).
///
/// Le panneau Forgeron applique ses changements directement (il tient déjà les
/// deux ressources). Le Marketplace, lui, n'écrit QUE la sauvegarde : sans ce
/// miroir, acheter des gantelets ne changerait rien à l'écran — l'article
/// serait payé et inerte.
///
/// Set-if-different plutôt qu'une écriture par frame : `ArmCosmetics` est lue
/// par le viewmodel, et la marquer changée à chaque frame ferait retravailler
/// tout ce qui l'observe pour rien.
fn sys_sync_arm_cosmetics(
    cfg: Res<IdentityConfig>,
    save: Res<IdentitySave>,
    mut arm: ResMut<ArmCosmetics>,
) {
    let color = cfg.color_rgb(&save.arm_color);
    let style = ArmStyle::from_key(&save.arm_style);
    if arm.color != color || arm.style != style {
        arm.color = color;
        arm.style = style;
    }
}

pub struct IdentityPlugin;

impl Plugin for IdentityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(IdentityConfig::load())
            .insert_resource(IdentitySave::load_or_default())
            .init_resource::<IdentityPanelShown>()
            .add_systems(Startup, sys_init_identity)
            // Story-678 — les bras suivent la sauvegarde, d'où qu'elle change.
            // NON gaté sur `GameMode::Roguelite` : le Marketplace vit au MENU,
            // qui tourne en `GameMode::None` (piège déjà payé sur la musique de
            // hub et les sons d'UI).
            .add_systems(Update, sys_sync_arm_cosmetics)
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
