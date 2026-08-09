//! Navigation du hub-menu : la Resource `MenuPage` (quelle page est affichée),
//! `MenuAction` (les demandes de transition d'état), les pastilles `HubBadges`
//! et la barre de navigation horizontale.

use bevy::prelude::*;
use bevy_egui::egui;
use forgia_core::prelude::*;
use forgia_mode_roguelite::equipment::EquipmentSave;
use forgia_mode_roguelite::meta_shop::{MetaShopCatalogue, MetaShopSave};
use forgia_mode_roguelite::run::MetaSouls;
use forgia_ui_lib::style::{C_PRIMARY, FORGE_CREME, FORGE_PANEL, HAIR_GOLD_STRONG};

/// Sous-page du menu titre — devenu **hub roguelite complet** (story-menu-hub).
/// Navigation purement UI-locale : PAS un variant d'`AppMode` (qui vit dans
/// forgia-core et est partagé). Reset à `Root` sur OnEnter(AppMode::Menu).
///
/// `Root` = accueil (titre FORGIA + CONTINUER/NOUVELLE PARTIE). Les autres = les
/// sections du hub, navigables depuis la sidebar verre sans lancer de run.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MenuPage {
    #[default]
    Root,
    Forgeron,
    Armes,
    Talents,
    Enclume,
    Codex,
    Missions,
    Succes,
    Stats,
    // `ArenaTest` (banc de blockout, story-667) RETIRÉ DU MENU le 2026-07-30 —
    // temporaire, et rien n'est supprimé côté moteur : `GameMode::ArenaTest`, le
    // plugin, le génome, le générateur et les 26 tests restent en place.
    // Accès entre-temps : `FORGIA_BOOT_MODE=arena_test`.
    // Pour rouvrir l'onglet : restaurer la variante ici, son `nav_label`, son
    // `section_title`, son bras de `draw_page` et `draw_arena_test_section`
    // (voir l'historique git de ce fichier).
    /// Le Livre — les dix chapitres, leurs verrous, celui qu'on va jouer.
    ///
    /// Sa place est ICI et pas au Lobby : le Lobby est un gate de chargement
    /// traversé par le démarrage automatique, le hub est au menu.
    Livre,
    /// Le MARKETPLACE — décors, couleurs, bras, musique (story-678).
    ///
    /// Hors sidebar comme Forgeron et Livre : on y entre depuis le Forgeron,
    /// qui est la page « qui je suis ». Elle a son propre système parce qu'une
    /// page riche tient mal sous le plafond de 16 paramètres Bevy quand elle
    /// partage le sien.
    Marketplace,
    /// **Le SAC** — les pièces d'armure possédées (story-678).
    Sac,
    Options,
}

impl MenuPage {
    /// Ordre de la sidebar de navigation (Accueil en tête, Options en pied).
    // Livre + Forgeron RETIRÉS de la sidebar (2026-08-05, story-678) : leur
    // contenu vit sur l'ACCUEIL (écran de préparation, modèle Dicero!). Les
    // pages existent toujours — Forgeron via « Personnaliser », Livre absorbé
    // par le carrousel de chapitres.
    pub(crate) const NAV: [MenuPage; 11] = [
        MenuPage::Root,
        MenuPage::Armes,
        // Story-678 — le Sac et le Marketplace montent dans la barre : ce sont
        // les deux écrans où l'on PREND quelque chose, ils ne doivent pas être
        // enfouis derrière la page Forgeron.
        MenuPage::Sac,
        MenuPage::Marketplace,
        MenuPage::Talents,
        MenuPage::Enclume,
        MenuPage::Codex,
        MenuPage::Missions,
        MenuPage::Succes,
        MenuPage::Stats,
        // `MenuPage::ArenaTest` RETIRÉ DE LA NAVIGATION (2026-07-30, temporaire).
        // Le banc de blockout reste entièrement en place — page, section, mode,
        // génome, générateur, tests. Seule son entrée de menu est masquée, le
        // temps que le sujet redevienne d'actualité.
        // Pour le rouvrir : remettre `MenuPage::ArenaTest,` sur cette ligne.
        // Accès entre-temps : `FORGIA_BOOT_MODE=arena_test`.
        MenuPage::Options,
    ];

    /// Libellé de l'onglet dans la sidebar (icône + nom court).
    pub(crate) fn nav_label(self) -> &'static str {
        match self {
            MenuPage::Root => "🏠  Accueil",
            MenuPage::Livre => "📕  Le Livre",
            MenuPage::Forgeron => "⚒  Forgeron",
            MenuPage::Armes => "🗡  Armes",
            MenuPage::Talents => "✨  Talents",
            MenuPage::Enclume => "🔨  Enclume",
            MenuPage::Codex => "📖  Codex",
            MenuPage::Missions => "🎯  Missions",
            MenuPage::Succes => "🏆  Succès",
            MenuPage::Stats => "📊  Stats",
            MenuPage::Marketplace => "💰  Marketplace",
            MenuPage::Sac => "🎒  Sac",
            MenuPage::Options => "⚙  Options",
        }
    }

    /// Titre display (Lilita) affiché en tête du panneau de section.
    pub(crate) fn section_title(self) -> &'static str {
        match self {
            MenuPage::Root => "FORGIA",
            MenuPage::Livre => "LE LIVRE",
            MenuPage::Forgeron => "TON FORGERON",
            MenuPage::Armes => "TES ARMES",
            MenuPage::Talents => "TALENTS",
            MenuPage::Enclume => "L'ENCLUME DES ÂMES",
            MenuPage::Codex => "CODEX · BESTIAIRE",
            MenuPage::Missions => "MISSIONS",
            MenuPage::Succes => "HAUTS FAITS",
            MenuPage::Stats => "STATISTIQUES",
            MenuPage::Marketplace => "MARKETPLACE",
            MenuPage::Sac => "TON SAC",
            MenuPage::Options => "OPTIONS",
        }
    }
}

/// Revient à la page racine du menu à chaque entrée dans le menu (retour jeu→menu).
pub(crate) fn reset_menu_page(mut page: ResMut<MenuPage>) {
    *page = MenuPage::Root;
}

/// Action de navigation d'état demandée par une section du hub-menu (appliquée par
/// `main_menu_ui`). Découple le rendu des sections des `NextState`/`MessageWriter`.
pub(crate) enum MenuAction {
    None,
    /// Entrer InGame dans le mode donné (Roguelite = run, Rpg/CyberCity = démos dev).
    Launch(GameMode),
    Quit,
}

/// Pastilles « quelque chose t'attend » du hub (story-678 Phase 4) — pilotées
/// par l'état RÉEL, jamais décoratives.
///
/// Reconstruites après l'incident de découpe du 2026-08-06 (une édition par
/// script a remplacé un span trop large ; ce fichier n'était pas commité).
#[derive(Resource, Clone, Copy, Default, PartialEq)]
pub(crate) struct HubBadges {
    /// Au moins un achat possible à l'Enclume avec les Âmes courantes.
    pub(crate) enclume: bool,
    /// Une pièce d'armure ramassée depuis la dernière visite de la fiche.
    pub(crate) forgeron: bool,
    /// Un chapitre battu que le Livre n'a pas encore montré.
    pub(crate) livre: bool,
}

/// Calcule les pastilles, et éteint celle de la fiche quand on la VISITE :
/// « vu » = la page a été ouverte. Écritures gardées — pas de churn à vide.
pub(crate) fn sys_hub_badges(
    app_state: Res<State<AppMode>>,
    page: Res<MenuPage>,
    cat: Option<Res<MetaShopCatalogue>>,
    souls: Option<Res<MetaSouls>>,
    meta: Option<Res<MetaShopSave>>,
    mut eq_save: Option<ResMut<EquipmentSave>>,
    mut badges: ResMut<HubBadges>,
) {
    if *app_state.get() != AppMode::Menu {
        return;
    }
    if let Some(eq) = eq_save.as_deref_mut() {
        if matches!(*page, MenuPage::Forgeron | MenuPage::Sac) {
            let total = eq.owned_total();
            if eq.seen_owned_total != total {
                eq.seen_owned_total = total;
                // Sans écriture disque, le marquage ne survit pas à la session :
                // la pastille éteinte se rallume au lancement suivant. Écriture
                // rare — une fois par visite qui découvre de nouvelles pièces.
                eq.save();
            }
        }
    }
    let next = HubBadges {
        enclume: match (cat.as_deref(), meta.as_deref(), souls.as_deref()) {
            (Some(c), Some(m), Some(s)) => {
                forgia_mode_roguelite::meta_shop::enclume_affordable(c, m, s.current)
            }
            _ => false,
        },
        forgeron: eq_save
            .as_deref()
            .map(|e| e.owned_total() > e.seen_owned_total)
            .unwrap_or(false),
        livre: meta
            .as_deref()
            .map(|m| m.chapters_cleared > m.seen_chapters_cleared)
            .unwrap_or(false),
    };
    if *badges != next {
        *badges = next;
    }
}

/// Barre de navigation HORIZONTALE, centrée en haut. `page` est muté au clic.
///
/// Elle était verticale à gauche et mangeait 228 px de large sur toute la
/// hauteur — dont l'écran de préparation avait besoin pour respirer, et qui
/// obligeait chaque page à se décaler de +100 px pour ne pas passer dessous.
/// À l'horizontale elle coûte [`HUB_TOP_BAR_H`] px de haut, une seule fois, et
/// les pages retrouvent le centre de l'écran.
pub(crate) fn draw_hub_nav(ctx: &egui::Context, page: &mut MenuPage, badges: HubBadges) {
    egui::Area::new(egui::Id::new("menu_hub_nav"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 18.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(FORGE_PANEL)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .corner_radius(egui::CornerRadius::same(14))
                .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                    // Onglets.
                    let mut first = true;
                    for tab in MenuPage::NAV {
                        // Espace ENTRE les onglets seulement : un espace traînant
                        // après le dernier décalait la barre de 3 px hors de son
                        // axe CENTER_TOP (rapporté « pas parfaitement centré »).
                        if !first {
                            ui.add_space(3.0);
                        }
                        first = false;
                        let selected = *page == tab;
                        let resp = ui.add_sized(
                            egui::vec2(tab_width(ui, tab.nav_label()), 34.0),
                            egui::Button::selectable(
                                selected,
                                egui::RichText::new(tab.nav_label())
                                    .size(16.0)
                                    .color(if selected { C_PRIMARY } else { FORGE_CREME })
                                    .strong(),
                            )
                            // Jamais de retour à la ligne dans un onglet — même
                            // règle que le titre FORGIA (un libellé wrappé dans
                            // un bouton de 34 px de haut est illisible).
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                        // La sidebar est dessinée avec des `Button::selectable`
                        // bruts, donc hors des helpers de style qui sonnent —
                        // elle était MUETTE au survol (retour en jeu 2026-08-05).
                        // Seul le survol est instrumenté : le clic joue déjà son
                        // `Tab` plus bas.
                        forgia_ui_lib::ui_sfx::instrument_hover(&resp);
                        // Story-678 Phase 4 — pastille dorée : quelque chose
                        // t'attend derrière cet onglet (état réel, jamais décoratif).
                        //
                        // Seule l'Enclume est encore concernée ICI : Livre et
                        // Forgeron ont quitté la sidebar avec la refonte Dicero,
                        // et leurs pastilles vivent désormais sur les blocs de
                        // l'Accueil qui ont repris leur contenu (carrousel du
                        // Livre, bouton « Personnaliser »). Les tester ici était
                        // du code mort — donc deux signaux calculés que rien
                        // n'affichait.
                        let dot = matches!(tab, MenuPage::Enclume) && badges.enclume;
                        if dot {
                            ui.painter().circle_filled(
                                egui::pos2(resp.rect.right() - 10.0, resp.rect.center().y),
                                4.0,
                                C_PRIMARY,
                            );
                        }
                        if resp.clicked() {
                            if *page != tab {
                                forgia_ui_lib::ui_sfx::push_ui_sfx(
                                    &resp.ctx,
                                    forgia_ui_lib::ui_sfx::UiSfxKind::Tab,
                                );
                            }
                            *page = tab;
                        }
                    }
                    });
                });
        });
}

/// Largeur d'un onglet de la barre horizontale, MESURÉE sur son libellé.
///
/// Une largeur fixe (les 200 px de la sidebar) donnait une barre de 1 800 px
/// pour neuf onglets — plus large que l'écran. Et l'approximation qui a suivi
/// (`CHAR_W = 9.0` × nombre de caractères) comptait un emoji comme un « i » et
/// cassait en silence au premier changement de fonte (audit 2026-08-07). On
/// mesure donc le galley RÉEL, au même corps que le rendu, avec un plancher
/// pour que les libellés courts restent cliquables.
fn tab_width(ui: &egui::Ui, label: &str) -> f32 {
    const PADDING: f32 = 22.0;
    const MIN_W: f32 = 84.0;
    let text_w = ui.fonts_mut(|f| {
        f.layout_no_wrap(
            label.to_string(),
            egui::FontId::proportional(16.0),
            egui::Color32::WHITE,
        )
        .size()
        .x
    });
    (text_w + PADDING).max(MIN_W)
}
