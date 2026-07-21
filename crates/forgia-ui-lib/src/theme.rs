//! # forgia-ui-lib::theme — Thème global egui « Forge »
//!
//! Story-596 Phase A. Avant : aucune font custom ni `set_style` dans tout le
//! workspace → font egui par défaut + widgets gris natifs (look prototype).
//!
//! Ici :
//! - **Fonts embarquées** (`include_bytes!`, Google Fonts OFL — ship OK) :
//!   - Lilita One → famille [`FAMILY_DISPLAY`] (titres cartoon chunky)
//!   - Poppins → tête de `Proportional` (body rond moderne, fallbacks emoji conservés)
//! - **Style global** [`forge_style`] : widgets natifs (boutons, sliders…) aux
//!   couleurs palette Forge (charbon chaud, hover or, corner_radius 12) — le
//!   menu principal, le pause menu et les settings héritent sans édition par-widget.
//! - [`ForgeThemePlugin`] : apply-once (Local<bool>) + sensor `forgia2_ui_theme.json`.
//!
//! Embed binaire = pas de call-site `asset_server.load()` (Lock L1 neutre).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPreUpdateSet};
use std::sync::Arc;

use crate::style::{FORGE_CHARBON, FORGE_CREME, FORGE_METAL_CHAUD, FORGE_OR, FORGE_PANEL};

/// Famille display cartoon (Lilita One). Utiliser via [`display_font`]/[`display_text`].
pub const FAMILY_DISPLAY: &str = "forge-display";

const FONT_DISPLAY_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/LilitaOne-Regular.ttf"
));
const FONT_BODY_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/Poppins-Regular.ttf"
));

/// Définitions fonts Forge : Poppins en tête de Proportional (fallbacks egui
/// conservés pour emoji/glyphes manquants), Lilita One en famille display
/// avec toute la chaîne Proportional en fallback.
pub fn forge_font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "lilita-one".to_owned(),
        Arc::new(egui::FontData::from_static(FONT_DISPLAY_BYTES)),
    );
    fonts.font_data.insert(
        "poppins".to_owned(),
        Arc::new(egui::FontData::from_static(FONT_BODY_BYTES)),
    );
    if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        prop.insert(0, "poppins".to_owned());
    }
    let mut display = vec!["lilita-one".to_owned()];
    if let Some(prop) = fonts.families.get(&egui::FontFamily::Proportional) {
        display.extend(prop.iter().cloned());
    }
    fonts
        .families
        .insert(egui::FontFamily::Name(FAMILY_DISPLAY.into()), display);
    fonts
}

/// `FontId` display (Lilita One) à la taille donnée.
pub fn display_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(FAMILY_DISPLAY.into()))
}

/// `RichText` pré-stylé display (titres, headings, compteurs).
pub fn display_text(text: impl Into<String>, size: f32, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text.into())
        .font(display_font(size))
        .color(color)
}

/// Applique la palette Forge au `Style` egui (widgets natifs).
pub fn forge_style(style: &mut egui::Style) {
    use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle};

    style.text_styles = [
        (TextStyle::Heading, display_font(30.0)),
        (TextStyle::Body, FontId::proportional(16.0)),
        (TextStyle::Button, FontId::proportional(18.0)),
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Monospace, FontId::monospace(14.0)),
    ]
    .into();
    style.spacing.button_padding = egui::vec2(18.0, 10.0);

    // Verre aubergine → hover éclairci → press or. Liserés métal/or, radius 12.
    // DA « Verre & Braise » (2026-07-20) : boutons natifs alignés sur les panneaux.
    let btn_fill = Color32::from_rgb(40, 32, 54);
    let btn_hover = Color32::from_rgb(58, 46, 78);
    let radius = CornerRadius::same(12);

    let v = &mut style.visuals;
    v.dark_mode = true;
    v.window_fill = FORGE_PANEL;
    v.window_stroke = Stroke::new(2.0, FORGE_OR);
    v.panel_fill = FORGE_PANEL;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(244, 196, 48, 90);

    v.widgets.noninteractive.bg_fill = FORGE_PANEL;
    v.widgets.noninteractive.weak_bg_fill = FORGE_PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(78, 68, 96));
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, FORGE_CREME);
    v.widgets.noninteractive.corner_radius = radius;

    v.widgets.inactive.bg_fill = btn_fill;
    v.widgets.inactive.weak_bg_fill = btn_fill;
    v.widgets.inactive.bg_stroke = Stroke::new(2.0, FORGE_METAL_CHAUD);
    v.widgets.inactive.fg_stroke = Stroke::new(1.2, FORGE_CREME);
    v.widgets.inactive.corner_radius = radius;

    v.widgets.hovered.bg_fill = btn_hover;
    v.widgets.hovered.weak_bg_fill = btn_hover;
    v.widgets.hovered.bg_stroke = Stroke::new(2.5, FORGE_OR);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, FORGE_CREME);
    v.widgets.hovered.corner_radius = radius;
    v.widgets.hovered.expansion = 2.0;

    v.widgets.active.bg_fill = FORGE_OR;
    v.widgets.active.weak_bg_fill = FORGE_OR;
    v.widgets.active.bg_stroke = Stroke::new(2.5, FORGE_CHARBON);
    v.widgets.active.fg_stroke = Stroke::new(1.5, FORGE_CHARBON);
    v.widgets.active.corner_radius = radius;

    v.widgets.open.bg_fill = btn_hover;
    v.widgets.open.weak_bg_fill = btn_hover;
    v.widgets.open.bg_stroke = Stroke::new(2.0, FORGE_OR);
    v.widgets.open.fg_stroke = Stroke::new(1.2, FORGE_CREME);
    v.widgets.open.corner_radius = radius;
}

/// Apply-once : fonts + style dès que le contexte egui primary existe.
/// Sensor one-shot `forgia2_ui_theme.json` (observability-required).
///
/// CRASH FIX 2026-06-12 : DOIT tourner en `PreUpdate` AVANT
/// `EguiPreUpdateSet::BeginPass` — `set_fonts` n'est appliqué qu'au
/// `begin_pass` suivant ; appliqué depuis `EguiPrimaryContextPass` (= pendant
/// la passe), la frame 1 dessinait `FontFamily::Name("forge-display")` avant
/// son enregistrement → panic epaint « not bound to any fonts »
/// (forgia2_crash.json 01:09). Ici : InitContexts crée le ctx → set_fonts →
/// BeginPass reconstruit les fonts → aucune passe ne voit la famille absente.
fn sys_apply_forge_theme(mut contexts: EguiContexts, mut applied: Local<bool>) {
    if *applied {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    ctx.set_fonts(forge_font_definitions());
    let mut style = (*ctx.style()).clone();
    forge_style(&mut style);
    ctx.set_style(style);
    *applied = true;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let json = format!(
        r#"{{"applied":true,"fonts":["lilita-one","poppins"],"display_family":"{FAMILY_DISPLAY}","timestamp_secs":{ts}}}"#
    );
    if let Err(e) = std::fs::write("forgia2_ui_theme.json", json) {
        warn!("[forge-theme] sensor write failed: {e}");
    }
    info!("[forge-theme] fonts + style Forge appliqués (Lilita One / Poppins)");
}

/// Plugin thème global — ajouté par `ForgiaUiHudPlugin` (guard idempotent).
pub struct ForgeThemePlugin;

impl Plugin for ForgeThemePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            sys_apply_forge_theme
                .after(EguiPreUpdateSet::InitContexts)
                .before(EguiPreUpdateSet::BeginPass),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_definitions_register_both_fonts() {
        let fonts = forge_font_definitions();
        assert!(fonts.font_data.contains_key("lilita-one"));
        assert!(fonts.font_data.contains_key("poppins"));
    }

    #[test]
    fn poppins_is_primary_proportional() {
        let fonts = forge_font_definitions();
        let prop = &fonts.families[&egui::FontFamily::Proportional];
        assert_eq!(prop[0], "poppins");
        // Fallbacks egui par défaut conservés derrière Poppins.
        assert!(prop.len() > 1);
    }

    #[test]
    fn display_family_has_lilita_then_fallbacks() {
        let fonts = forge_font_definitions();
        let display = &fonts.families[&egui::FontFamily::Name(FAMILY_DISPLAY.into())];
        assert_eq!(display[0], "lilita-one");
        assert!(display.contains(&"poppins".to_owned()));
    }

    #[test]
    fn forge_style_sets_display_heading_and_radius() {
        let mut style = egui::Style::default();
        forge_style(&mut style);
        let heading = &style.text_styles[&egui::TextStyle::Heading];
        assert_eq!(
            heading.family,
            egui::FontFamily::Name(FAMILY_DISPLAY.into())
        );
        assert_eq!(style.visuals.widgets.inactive.corner_radius.nw, 12);
        assert_eq!(style.visuals.widgets.active.weak_bg_fill, FORGE_OR);
    }
}
