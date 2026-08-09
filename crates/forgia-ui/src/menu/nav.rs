//! Navigation du hub-menu : la pile `NavStack` (source de vérité de la page
//! affichée — sommet = page courante, Retour = pop), `MenuAction` (les demandes
//! de transition d'état), les pastilles `HubBadges` et la barre de navigation
//! horizontale.

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
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
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
    // Pour rouvrir l'onglet (depuis le registre, story-694 incr. 4) : restaurer
    // la variante ici + la garde `_EXHAUSTIVITE` + `TOUTES` ci-dessous, une
    // `PageDecl` dans `menu/registry.rs::PAGES`, et `draw_arena_test_section`
    // (historique git de l'ex-lib.rs).
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

// GARDE D'EXHAUSTIVITÉ (compile-time) — ce match est volontairement SANS
// wildcard : ajouter une variante à `MenuPage` casse la compilation ICI, à
// deux lignes de `TOUTES` qu'il faut compléter ; le test du registre
// (`chaque_variante_a_exactement_une_declaration`) exige ensuite sa `PageDecl`.
// Chaîne : compilateur → TOUTES → test → registre. Elle remplace l'exhaustivité
// que les ex-match `nav_label`/`section_title` offraient avant l'incrément 4.
const _EXHAUSTIVITE: () = match MenuPage::Root {
    MenuPage::Root
    | MenuPage::Forgeron
    | MenuPage::Armes
    | MenuPage::Talents
    | MenuPage::Enclume
    | MenuPage::Codex
    | MenuPage::Missions
    | MenuPage::Succes
    | MenuPage::Stats
    | MenuPage::Livre
    | MenuPage::Marketplace
    | MenuPage::Sac
    | MenuPage::Options => (),
};

impl MenuPage {
    // L'ex-`NAV: [MenuPage; 11]` et les deux match de libellés vivent
    // désormais dans LA table de `menu/registry.rs` (story-694 incr. 4) —
    // l'ordre des onglets est l'ordre du tableau, filtré `in_nav`. Le libellé
    // d'onglet se lit directement sur `PageDecl.nav_label` (son seul
    // consommateur, `draw_hub_nav`, itère la table).

    /// Les 13 variantes — maintenue à deux lignes de la garde `_EXHAUSTIVITE`
    /// qui force sa mise à jour. Consommée par le test de complétude du
    /// registre (d'où le cfg(test)) : chaque variante doit avoir exactement
    /// une `PageDecl`.
    #[cfg(test)]
    pub(crate) const TOUTES: [MenuPage; 13] = [
        MenuPage::Root,
        MenuPage::Forgeron,
        MenuPage::Armes,
        MenuPage::Talents,
        MenuPage::Enclume,
        MenuPage::Codex,
        MenuPage::Missions,
        MenuPage::Succes,
        MenuPage::Stats,
        MenuPage::Livre,
        MenuPage::Marketplace,
        MenuPage::Sac,
        MenuPage::Options,
    ];

    /// Titre display (Lilita) du panneau de section — délégué au registre.
    pub(crate) fn section_title(self) -> &'static str {
        crate::menu::registry::decl(self).section_title
    }
}

/// La PILE de navigation du hub (story-694, incrément 3) — LA source de vérité
/// de la page affichée. Le sommet est la page courante, le fond est TOUJOURS
/// `Root`. « D'où je viens » devient DÉRIVÉ (pop) au lieu d'être recopié par
/// chaque page — le Retour du Marketplace téléportait vers Forgeron même quand
/// on venait de la barre (constat UX n°5, audit 2026-08-07).
///
/// `MenuPage` n'est plus une Resource : retirer son derive a fait pointer le
/// compilateur sur chacun des 16 sites à convertir — aucun oubli possible.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(crate) struct NavStack {
    stack: Vec<MenuPage>,
}

impl Default for NavStack {
    fn default() -> Self {
        Self { stack: vec![MenuPage::Root] }
    }
}

impl NavStack {
    /// La page affichée = le sommet. La pile n'est jamais vide (invariant tenu
    /// par toutes les méthodes) ; le repli défensif rend `Root`.
    pub(crate) fn current(&self) -> MenuPage {
        self.stack.last().copied().unwrap_or(MenuPage::Root)
    }

    /// Clic d'onglet (barre horizontale ou LB/RB) : les onglets sont des FRÈRES,
    /// pas des enfants — la pile repart de `[Root, tab]`. ESC depuis un onglet
    /// remonte donc à l'Accueil, jamais dans l'historique des onglets visités.
    pub(crate) fn switch_tab(&mut self, tab: MenuPage) {
        self.stack.clear();
        self.stack.push(MenuPage::Root);
        if tab != MenuPage::Root {
            self.stack.push(tab);
        }
    }

    /// Entrée en profondeur (Personnaliser → Forgeron, fiche → Marketplace…) :
    /// empile. No-op si on y est déjà (double-clic).
    pub(crate) fn push(&mut self, page: MenuPage) {
        if self.current() != page {
            self.stack.push(page);
        }
    }

    /// Remonte d'un niveau. `false` au Root (rien à dépiler) — l'appelant
    /// décide : au menu on ne fait rien, Quitter est un bouton explicite.
    pub(crate) fn back(&mut self) -> bool {
        if self.stack.len() > 1 {
            self.stack.pop();
            true
        } else {
            false
        }
    }

    /// Retour à l'accueil (OnEnter(Menu)).
    pub(crate) fn reset(&mut self) {
        self.stack.clear();
        self.stack.push(MenuPage::Root);
    }

    /// Profondeur de la pile — publiée par le capteur menu_hub (`nav_depth`).
    pub(crate) fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Chemin lisible pour le capteur — « Root>Forgeron>Marketplace ».
    pub(crate) fn path(&self) -> String {
        self.stack
            .iter()
            .map(|p| format!("{p:?}"))
            .collect::<Vec<_>>()
            .join(">")
    }
}

/// Revient à la page racine du menu à chaque entrée dans le menu (retour jeu→menu).
pub(crate) fn reset_menu_page(mut nav: ResMut<NavStack>) {
    nav.reset();
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
    nav: Res<NavStack>,
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
        if matches!(nav.current(), MenuPage::Forgeron | MenuPage::Sac) {
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
pub(crate) fn draw_hub_nav(ctx: &egui::Context, nav: &mut NavStack, badges: HubBadges) {
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
                    // Onglets — LA table du registre, filtrée in_nav, dans
                    // l'ordre de déclaration (story-694 incr. 4).
                    let mut first = true;
                    for decl in crate::menu::registry::nav_tabs() {
                        let tab = decl.id;
                        // Espace ENTRE les onglets seulement : un espace traînant
                        // après le dernier décalait la barre de 3 px hors de son
                        // axe CENTER_TOP (rapporté « pas parfaitement centré »).
                        if !first {
                            ui.add_space(3.0);
                        }
                        first = false;
                        let selected = nav.current() == tab;
                        let resp = ui.add_sized(
                            egui::vec2(tab_width(ui, decl.nav_label), 34.0),
                            egui::Button::selectable(
                                selected,
                                egui::RichText::new(decl.nav_label)
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
                        // t'attend derrière cet onglet (état réel, jamais
                        // décoratif). QUELLE pastille éclaire QUEL onglet est
                        // déclaré dans la table (`PageDecl.badge`) — plus de
                        // matches! codé en dur ici.
                        let dot = decl.badge.is_some_and(|f| f(&badges));
                        if dot {
                            ui.painter().circle_filled(
                                egui::pos2(resp.rect.right() - 10.0, resp.rect.center().y),
                                4.0,
                                C_PRIMARY,
                            );
                        }
                        // Re-cliquer l'onglet courant reste un no-op : un
                        // switch_tab tronquerait la pile (et le chemin de
                        // retour d'une page profonde) sans rien changer à
                        // l'écran.
                        if resp.clicked() && nav.current() != tab {
                            forgia_ui_lib::ui_sfx::push_ui_sfx(
                                &resp.ctx,
                                forgia_ui_lib::ui_sfx::UiSfxKind::Tab,
                            );
                            nav.switch_tab(tab);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_pile_nait_sur_root() {
        let nav = NavStack::default();
        assert_eq!(nav.current(), MenuPage::Root);
        assert_eq!(nav.depth(), 1);
    }

    #[test]
    fn switch_tab_repart_de_root() {
        let mut nav = NavStack::default();
        nav.switch_tab(MenuPage::Armes);
        nav.switch_tab(MenuPage::Sac);
        // Les onglets sont des frères : pas d'historique d'onglets empilé.
        assert_eq!(nav.depth(), 2);
        assert_eq!(nav.current(), MenuPage::Sac);
        assert!(nav.back());
        assert_eq!(nav.current(), MenuPage::Root);
    }

    #[test]
    fn back_au_root_ne_fait_rien() {
        let mut nav = NavStack::default();
        assert!(!nav.back());
        assert_eq!(nav.current(), MenuPage::Root);
        assert_eq!(nav.depth(), 1);
    }

    /// AC3 — le Retour du Marketplace dépend d'OÙ l'on vient, il ne téléporte
    /// plus vers Forgeron en dur.
    #[test]
    fn le_retour_du_marketplace_depend_d_ou_l_on_vient() {
        // Entré depuis la barre : retour à l'Accueil.
        let mut nav = NavStack::default();
        nav.switch_tab(MenuPage::Marketplace);
        assert!(nav.back());
        assert_eq!(nav.current(), MenuPage::Root);
        // Entré depuis la fiche (Personnaliser → Forgeron → Marketplace) :
        // retour à la fiche, pas à l'Accueil.
        let mut nav = NavStack::default();
        nav.push(MenuPage::Forgeron);
        nav.push(MenuPage::Marketplace);
        assert!(nav.back());
        assert_eq!(nav.current(), MenuPage::Forgeron);
        assert!(nav.back());
        assert_eq!(nav.current(), MenuPage::Root);
    }

    #[test]
    fn push_ne_double_pas_la_page_courante() {
        let mut nav = NavStack::default();
        nav.push(MenuPage::Forgeron);
        nav.push(MenuPage::Forgeron);
        assert_eq!(nav.depth(), 2);
    }

    #[test]
    fn reset_ramene_a_l_accueil() {
        let mut nav = NavStack::default();
        nav.push(MenuPage::Forgeron);
        nav.push(MenuPage::Marketplace);
        nav.reset();
        assert_eq!(nav.current(), MenuPage::Root);
        assert_eq!(nav.depth(), 1);
    }

    /// Invariant « jamais vide » sous une rafale adversariale de back().
    #[test]
    fn une_rafale_de_back_ne_vide_jamais_la_pile() {
        let mut nav = NavStack::default();
        nav.push(MenuPage::Forgeron);
        nav.push(MenuPage::Marketplace);
        for _ in 0..10 {
            nav.back();
        }
        assert_eq!(nav.current(), MenuPage::Root);
        assert_eq!(nav.depth(), 1);
    }

    /// switch_tab depuis une pile PROFONDE tronque volontairement : changer
    /// d'onglet est un changement de contexte, l'historique du drill-in ne
    /// doit pas survivre sous ESC. (Le no-op « re-clic sur l'onglet courant »
    /// est un garde des APPELANTS — draw_hub_nav, gamepad_nav.)
    #[test]
    fn switch_tab_depuis_une_pile_profonde_tronque() {
        let mut nav = NavStack::default();
        nav.push(MenuPage::Forgeron);
        nav.push(MenuPage::Marketplace);
        nav.switch_tab(MenuPage::Armes);
        assert_eq!(nav.depth(), 2);
        assert!(nav.back());
        assert_eq!(nav.current(), MenuPage::Root);
    }

    #[test]
    fn le_chemin_du_capteur_est_lisible() {
        let mut nav = NavStack::default();
        nav.push(MenuPage::Forgeron);
        nav.push(MenuPage::Marketplace);
        assert_eq!(nav.path(), "Root>Forgeron>Marketplace");
    }
}
