//! chapters.rs — le sélecteur de CHAPITRES du Lobby (2026-08-04).
//!
//! Le Livre s'ouvre chapitre par chapitre : on ne peut entrer dans le suivant
//! qu'après avoir battu le boss du précédent. Ce panneau montre les dix, dit
//! lesquels sont ouverts, et laisse choisir celui qu'on va jouer.
//!
//! ## Ce qu'il ne décide pas
//!
//! Rien. Toute la règle vit dans `meta_shop` (`chapter_unlocked`,
//! `record_chapter_cleared`, `SelectedChapter::clamped`) et y est testée sans
//! egui. Ce module ne fait que **montrer** cet état et écrire un choix — un
//! panneau qui déciderait par lui-même finirait par diverger de ce que la
//! sauvegarde autorise.
//!
//! ## Pourquoi il borne malgré tout
//!
//! `SelectedChapter` survit à la session. Une sélection héritée d'une partie où
//! la progression était plus avancée (ou un fichier de save édité à la main) ne
//! doit pas ouvrir un chapitre verrouillé : le panneau applique `clamped` à
//! l'affichage comme au clic. Ne faire confiance à personne coûte une ligne.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_core::prelude::*;
use forgia_ui_lib::style::{
    C_PRIMARY, C_TEXT_MUTED, FORGE_CREME, FORGE_OR, FORGE_PANEL, FORGE_PANEL_LIGHT,
    HAIR_GOLD_STRONG,
};
use forgia_ui_lib::theme::display_text;

use crate::decor_palettes::DecorPalettesConfig;
use crate::meta_shop::{chapter_unlocked, MetaShopSave, SelectedChapter, CHAPTERS_PER_BOOK};
use crate::RunState;

/// Le nom de la direction artistique d'un chapitre, pour que la liste dise où
/// l'on va — « CHAPITRE 6 » seul ne raconte rien.
///
/// Repli sur une chaîne vide plutôt que sur un nom inventé : un chapitre sans DA
/// déclarée doit se voir comme tel.
fn chapter_da_name(palettes: Option<&DecorPalettesConfig>, chapter: u32) -> String {
    palettes
        .and_then(|c| c.palette_id_for_chapter(chapter))
        .and_then(|id| palettes.and_then(|c| c.palette(id)))
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

/// Le contenu du Livre, dessiné dans un `Ui` déjà fourni.
///
/// ## Pourquoi il est séparé de son système
///
/// Le premier jet ne dessinait ce panneau qu'au **Lobby**, et il n'a jamais été
/// vu : le Lobby n'est plus un hub, il est traversé par un démarrage
/// automatique. **Le hub est au MENU** (`MenuPage`). Le contenu vit donc ici,
/// sans savoir où il est dessiné, et les deux emplacements l'appellent.
pub fn chapter_select_content(
    ui: &mut egui::Ui,
    save: &MetaShopSave,
    palettes: Option<&DecorPalettesConfig>,
    selected: &mut SelectedChapter,
) {
    // La sélection est bornée AVANT d'être affichée : ce qu'on montre est ce
    // qu'on jouera, même si la Resource porte une valeur héritée.
    let courant = selected.clamped(save.chapters_cleared);
    if selected.0 != courant {
        selected.0 = courant;
    }

    ui.set_min_width(520.0);
    ui.vertical_centered(|ui| {
        let ouverts = (save.chapters_cleared + 1).min(CHAPTERS_PER_BOOK);
        ui.label(display_text(
            format!("{ouverts} / {CHAPTERS_PER_BOOK} chapitres ouverts").as_str(),
            15.0,
            C_TEXT_MUTED,
        ));
        ui.add_space(12.0);
    });

    for chapitre in 1..=CHAPTERS_PER_BOOK {
        let ouvert = chapter_unlocked(chapitre, save.chapters_cleared);
        let battu = chapitre <= save.chapters_cleared;
        let actif = chapitre == courant;
        let da = chapter_da_name(palettes, chapitre);

        // Le verrou se LIT : un chapitre fermé garde sa ligne et son numéro, il
        // ne disparaît pas. On voit ce qui reste.
        let (marque, couleur) = if !ouvert {
            ("\u{1F512}", C_TEXT_MUTED)
        } else if battu {
            ("\u{2714}", FORGE_CREME)
        } else {
            ("\u{25B6}", C_PRIMARY)
        };
        let titre = if da.is_empty() {
            format!("{marque}  CHAPITRE {chapitre}")
        } else {
            format!("{marque}  CHAPITRE {chapitre}  \u{2014}  {da}")
        };

        let frame = egui::Frame::new()
            .fill(if actif { FORGE_PANEL_LIGHT } else { FORGE_PANEL })
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(10, 7));
        frame.show(ui, |ui| {
            ui.set_min_width(480.0);
            // Un chapitre verrouillé n'est pas cliquable : le panneau ne propose
            // jamais ce que la règle refuse.
            let resp = ui.add_enabled(
                ouvert,
                egui::Button::selectable(
                    actif,
                    egui::RichText::new(titre).size(17.0).color(couleur),
                )
                .min_size(egui::vec2(470.0, 0.0)),
            );
            if resp.clicked() {
                selected.0 = chapitre;
            }
        });
        ui.add_space(3.0);
    }

    ui.add_space(10.0);
    ui.vertical_centered(|ui| {
        ui.label(display_text(
            "Bats le boss d'un chapitre pour ouvrir le suivant.",
            13.0,
            C_TEXT_MUTED,
        ));
    });
}

/// Panneau du Lobby — le même Livre, pour qui s'y arrête.
pub fn draw_chapter_select(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    save: Res<MetaShopSave>,
    palettes: Option<Res<DecorPalettesConfig>>,
    mut selected: ResMut<SelectedChapter>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    if !matches!(run_state.as_deref().map(State::get), Some(RunState::Lobby)) {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let palettes_ref = palettes.as_deref();

    egui::Area::new(egui::Id::new("forgia_chapter_select"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 40.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(FORGE_PANEL)
                .stroke(egui::Stroke::new(1.5, HAIR_GOLD_STRONG))
                .corner_radius(egui::CornerRadius::same(16))
                .inner_margin(20)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(display_text("LE LIVRE", 30.0, FORGE_OR).strong());
                    });
                    chapter_select_content(ui, &save, palettes_ref, &mut selected);
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decor_palettes::DecorPalettesConfig;

    /// Le panneau AFFICHE la règle, il ne la réinvente pas — donc la liste des
    /// chapitres cliquables doit coïncider exactement avec `chapter_unlocked`.
    #[test]
    fn the_panel_offers_exactly_what_the_rule_allows() {
        for cleared in 0..=CHAPTERS_PER_BOOK {
            let ouverts: Vec<u32> = (1..=CHAPTERS_PER_BOOK)
                .filter(|c| chapter_unlocked(*c, cleared))
                .collect();
            let attendu: Vec<u32> = (1..=(cleared + 1).min(CHAPTERS_PER_BOOK)).collect();
            assert_eq!(ouverts, attendu, "progression {cleared}");
        }
    }

    /// Une sélection héritée d'une progression plus avancée ne doit pas ouvrir un
    /// chapitre verrouillé — le panneau borne avant d'afficher.
    #[test]
    fn a_stale_selection_is_pulled_back_before_being_shown() {
        assert_eq!(SelectedChapter(10).clamped(0), 1);
        assert_eq!(SelectedChapter(10).clamped(3), 4);
        assert_eq!(SelectedChapter(2).clamped(6), 2, "un choix légitime est gardé");
    }

    /// Le nom de DA vient du génome ; s'il manque, la ligne reste lisible au lieu
    /// d'inventer un nom.
    #[test]
    fn a_chapter_without_a_declared_art_direction_shows_no_name() {
        let vide = DecorPalettesConfig::default();
        assert_eq!(chapter_da_name(Some(&vide), 6), "");
        assert_eq!(chapter_da_name(None, 1), "");
    }

    /// …et avec le génome livré, les dix chapitres ont tous un nom à montrer.
    #[test]
    fn the_shipped_book_names_all_ten_chapters() {
        let content =
            std::fs::read_to_string("assets/genomes/roguelite/roguelite_palettes.toml")
                .or_else(|_| {
                    std::fs::read_to_string(
                        "../../assets/genomes/roguelite/roguelite_palettes.toml",
                    )
                })
                .expect("roguelite_palettes.toml introuvable");
        let cfg = DecorPalettesConfig::parse_toml(&content);
        for chapitre in 1..=CHAPTERS_PER_BOOK {
            assert!(
                !chapter_da_name(Some(&cfg), chapitre).is_empty(),
                "chapitre {chapitre} sans nom de direction artistique"
            );
        }
    }
}
