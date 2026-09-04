//! hub.rs — Reliquats du hub Lobby à onglets (story-694 : le hub à onglets a été
//! RETIRÉ). Le Lobby (`RunState::Lobby`) est un gate auto-start qui se ferme sous
//! un overlay de chargement opaque (`pipeline_warmup`) — son UI n'était jamais vue
//! par personne, et son chrome dupliquait celui du menu-titre (`forgia-ui`).
//!
//! Ce module ne garde que : le masquage du HUD de gameplay pendant le Lobby, et
//! les deux helpers de contenu partagés avec le hub-menu (`section_intro`,
//! `draw_codex_section`).

use bevy::prelude::*;
use bevy_egui::egui;
use forgia_core::prelude::*;
use forgia_ui_lib::style::{
    C_PRIMARY, C_TEXT_MUTED, FORGE_CREME, FORGE_OR, FORGE_PANEL_LIGHT, HAIR_GOLD_STRONG,
};
use forgia_ui_lib::theme::display_text;

use crate::RunState;

/// Au Lobby : masque tout le HUD de gameplay (ammo/PV/énergie/confiance/viewmodel)
/// pour ne garder que l'UI de menu du hub. Le HUD partagé (forgia-ui-lib /
/// forgia-viewmodel) lit `GameplayHudVisible` (forgia-core) sans dépendre de ce crate.
fn lobby_hide_gameplay_hud(mut v: ResMut<GameplayHudVisible>) {
    v.0 = false;
}

/// À la sortie du Lobby (run lancée OU retour menu) : ré-affiche le HUD de gameplay
/// + coupe l'aperçu bras forcé (onglet Forge).
fn lobby_show_gameplay_hud(
    mut v: ResMut<GameplayHudVisible>,
    mut forced: ResMut<ViewmodelForcedVisible>,
) {
    v.0 = true;
    forced.0 = false;
}

/// Intro d'une section « à venir » (Talents / Missions / Succès) : titre display +
/// pitch + tag discret. Le vrai contenu gameplay vient en stories suivantes.
///
/// `pub` : réutilisé par le hub-menu (`forgia-ui`) pour rendre les mêmes sections
/// data au menu-titre (story-menu-hub, pas de duplication).
pub fn section_intro(ui: &mut egui::Ui, title: &str, desc: &str) {
    ui.label(display_text(title, 30.0, C_PRIMARY).strong());
    ui.add_space(12.0);
    ui.label(egui::RichText::new(desc).size(16.0).color(FORGE_CREME));
    ui.add_space(14.0);
    ui.label(
        egui::RichText::new("Contenu à venir")
            .size(13.0)
            .italics()
            .color(C_TEXT_MUTED),
    );
}

/// Codex · Bestiaire — cartes des 4 archétypes ennemis. Contenu réel (comportement
/// aligné sur `enemies.rs`), pas un placeholder. Textes UI cosmétiques.
///
/// `pub` : réutilisé par le hub-menu (`forgia-ui`) pour rendre le Codex au
/// menu-titre (story-menu-hub, pas de duplication).
pub fn draw_codex_section(ui: &mut egui::Ui) {
    ui.label(display_text("Bestiaire", 28.0, C_PRIMARY).strong());
    ui.add_space(14.0);
    const ENTRIES: [(&str, &str); 4] = [
        (
            "Tank",
            "Gros, lent, lourdement blindé. Encaisse et charge dans le tas.",
        ),
        (
            "Coureur",
            "Rapide et fragile. Fonce sur toi, souvent en meute.",
        ),
        (
            "Tireur",
            "Se tient à distance et te canarde. Garde ses distances.",
        ),
        (
            "Boss — le Forgeron Noir",
            "S'enrage sous 50 % de PV. Le combat final de la run.",
        ),
    ];
    for (name, desc) in ENTRIES {
        egui::Frame::new()
            .fill(FORGE_PANEL_LIGHT)
            .inner_margin(egui::Margin::symmetric(16, 10))
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
            .show(ui, |ui| {
                ui.set_min_width(560.0);
                ui.vertical(|ui| {
                    ui.label(display_text(name, 18.0, FORGE_OR).strong());
                    ui.label(egui::RichText::new(desc).size(14.0).color(FORGE_CREME));
                });
            });
        ui.add_space(8.0);
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct HubPlugin;

impl Plugin for HubPlugin {
    fn build(&self, app: &mut App) {
        app
            // Masque le HUD de gameplay à l'entrée Lobby (OnEnter(Lobby) ne tire
            // qu'en GameMode::Roguelite : RunState est un SubState de Roguelite).
            .add_systems(OnEnter(RunState::Lobby), lobby_hide_gameplay_hud)
            // Sortie Lobby (run lancée ou retour menu) → ré-affiche le HUD de gameplay.
            .add_systems(OnExit(RunState::Lobby), lobby_show_gameplay_hud);
    }
}
