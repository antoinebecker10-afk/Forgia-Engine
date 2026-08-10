//! Le REGISTRE des pages du hub (story-694, incrément 4) — LA table unique que
//! la barre de navigation, les pastilles, le cycle LB/RB et le dispatch de
//! `main_menu_ui` itèrent.
//!
//! **Ajouter une page = 1 variante `MenuPage` + 1 `PageDecl` ici** (+ sa fn de
//! dessin inline, OU son système dédié auto-gaté sur `nav.current()` à
//! enregistrer dans lib.rs). Avant ce registre : 6 sites d'édition dont 2 non
//! couverts par le compilateur (constat n°7 de l'audit 2026-08-07).

use bevy_egui::egui;
use forgia_mode_roguelite::hub::{draw_codex_section, section_intro};
use forgia_mode_roguelite::meta_shop::MetaShopSave;
use forgia_ui_lib::style::{FORGE_CREME, FORGE_OR};
use forgia_ui_lib::theme::display_text;

use crate::nav::{HubBadges, MenuPage};
use crate::pages::root::draw_stats_section;

/// Données passées aux dessinateurs INLINE — extraites par `main_menu_ui` de
/// ses propres params. S'enrichit quand une nouvelle page inline en a besoin.
pub(crate) struct InlinePageCtx<'a> {
    /// Niveau du forgeron (somme des rangs d'Enclume, story-680).
    pub(crate) level: u32,
    /// Rangs d'Enclume restant à débloquer.
    pub(crate) ranks_remaining: u32,
    pub(crate) meta_save: Option<&'a MetaShopSave>,
}

/// Comment le contenu d'une page est produit.
#[derive(Clone, Copy)]
pub(crate) enum PageDraw {
    /// Dessinée par `main_menu_ui` dans le panneau de section standard.
    Inline {
        /// Id egui du panneau — stable : il cle l'animation d'entrée de section.
        panel_id: &'static str,
        panel_width: f32,
        draw: fn(&InlinePageCtx, &mut egui::Ui),
    },
    /// Dessinée par son propre système Bevy (plafond des 16 params), auto-gaté
    /// sur `nav.current()` et enregistré dans la chaîne EguiPrimaryContextPass.
    OwnSystem,
    /// Cas spéciaux câblés dans le shell — Root (landing + `MenuAction`) et
    /// Options (mute la pile) sont traités par des arms EXPLICITES de
    /// `main_menu_ui`, AVANT la consultation de la table : cette valeur est
    /// purement documentaire, aucun code ne la lit.
    Shell,
}

pub(crate) struct PageDecl {
    pub(crate) id: MenuPage,
    /// Libellé d'onglet (icône + nom court).
    pub(crate) nav_label: &'static str,
    /// Titre display (Lilita) du panneau de section.
    pub(crate) section_title: &'static str,
    /// Présente dans la barre horizontale — **l'ordre du tableau EST l'ordre
    /// des onglets** (et du cycle LB/RB).
    pub(crate) in_nav: bool,
    /// La pastille « quelque chose t'attend » de son onglet, s'il en a une.
    /// (Les pastilles Forgeron/Livre vivent sur les blocs de l'Accueil.)
    pub(crate) badge: Option<fn(&HubBadges) -> bool>,
    pub(crate) draw: PageDraw,
}

/// LA table. Les 11 premières entrées `in_nav` reproduisent l'ordre de
/// l'ex-`MenuPage::NAV` ; Forgeron et Livre restent hors barre (leurs portes
/// sont sur l'Accueil — « Personnaliser » et le titre du Livre).
pub(crate) const PAGES: &[PageDecl] = &[
    PageDecl {
        id: MenuPage::Root,
        nav_label: "🏠  Accueil",
        section_title: "FORGIA",
        in_nav: true,
        badge: None,
        draw: PageDraw::Shell,
    },
    PageDecl {
        id: MenuPage::Armes,
        nav_label: "🗡  Armes",
        section_title: "TES ARMES",
        in_nav: true,
        badge: None,
        draw: PageDraw::OwnSystem,
    },
    // Story-678 — le Sac et le Marketplace dans la barre : ce sont les deux
    // écrans où l'on PREND quelque chose, pas des annexes du Forgeron.
    PageDecl {
        id: MenuPage::Sac,
        nav_label: "🎒  Sac",
        section_title: "TON SAC",
        in_nav: true,
        badge: None,
        draw: PageDraw::OwnSystem,
    },
    PageDecl {
        id: MenuPage::Marketplace,
        nav_label: "💰  Marketplace",
        section_title: "MARKETPLACE",
        in_nav: true,
        badge: None,
        draw: PageDraw::OwnSystem,
    },
    PageDecl {
        id: MenuPage::Talents,
        nav_label: "✨  Talents",
        section_title: "TALENTS",
        in_nav: true,
        badge: None,
        draw: PageDraw::Inline {
            panel_id: "hub_sec_talents",
            panel_width: 640.0,
            draw: draw_talents,
        },
    },
    PageDecl {
        id: MenuPage::Enclume,
        nav_label: "🔨  Enclume",
        section_title: "L'ENCLUME DES ÂMES",
        in_nav: true,
        // Pastille dorée : au moins un achat possible avec les Âmes courantes
        // (état réel calculé par `sys_hub_badges`, jamais décoratif).
        badge: Some(|b| b.enclume),
        draw: PageDraw::OwnSystem,
    },
    PageDecl {
        id: MenuPage::Codex,
        nav_label: "📖  Codex",
        section_title: "CODEX · BESTIAIRE",
        in_nav: true,
        badge: None,
        draw: PageDraw::Inline {
            panel_id: "hub_sec_codex",
            panel_width: 720.0,
            draw: draw_codex,
        },
    },
    PageDecl {
        id: MenuPage::Missions,
        nav_label: "🎯  Missions",
        section_title: "MISSIONS",
        in_nav: true,
        badge: None,
        draw: PageDraw::Inline {
            panel_id: "hub_sec_missions",
            panel_width: 640.0,
            draw: draw_missions,
        },
    },
    PageDecl {
        id: MenuPage::Succes,
        nav_label: "🏆  Succès",
        section_title: "HAUTS FAITS",
        in_nav: true,
        badge: None,
        draw: PageDraw::Inline {
            panel_id: "hub_sec_succes",
            panel_width: 640.0,
            draw: draw_succes,
        },
    },
    PageDecl {
        id: MenuPage::Stats,
        nav_label: "📊  Stats",
        section_title: "STATISTIQUES",
        in_nav: true,
        badge: None,
        draw: PageDraw::Inline {
            panel_id: "hub_sec_stats",
            panel_width: 560.0,
            draw: draw_stats,
        },
    },
    // `MenuPage::ArenaTest` RETIRÉ DE LA NAVIGATION (2026-07-30, temporaire) —
    // pour rouvrir le banc : restaurer la variante + une PageDecl ici.
    // Accès entre-temps : `FORGIA_BOOT_MODE=arena_test`.
    PageDecl {
        id: MenuPage::Options,
        nav_label: "⚙  Options",
        section_title: "OPTIONS",
        in_nav: true,
        badge: None,
        draw: PageDraw::Shell,
    },
    // ── Hors barre (portes sur l'Accueil, story-678 refonte Dicero) ──
    PageDecl {
        id: MenuPage::Forgeron,
        nav_label: "⚒  Forgeron",
        section_title: "TON FORGERON",
        in_nav: false,
        badge: None,
        draw: PageDraw::OwnSystem,
    },
    PageDecl {
        id: MenuPage::Livre,
        nav_label: "📕  Le Livre",
        section_title: "LE LIVRE",
        in_nav: false,
        badge: None,
        draw: PageDraw::OwnSystem,
    },
];

/// La déclaration d'une page. Le registre est complet par construction (test
/// d'exhaustivité ci-dessous) — un id absent est un bug de programmation.
pub(crate) fn decl(id: MenuPage) -> &'static PageDecl {
    PAGES
        .iter()
        .find(|d| d.id == id)
        .expect("MenuPage sans PageDecl — compléter menu/registry.rs::PAGES")
}

/// Les onglets de la barre, dans l'ordre du tableau (consommé par
/// `draw_hub_nav` et le cycle LB/RB de `gamepad_nav`).
pub(crate) fn nav_tabs() -> impl Iterator<Item = &'static PageDecl> {
    PAGES.iter().filter(|d| d.in_nav)
}

// ── Dessinateurs des pages INLINE ──

fn draw_codex(_ctx: &InlinePageCtx, ui: &mut egui::Ui) {
    draw_codex_section(ui);
}

// Story-680 cran 1 — cet écran ANNONÇAIT « Choisis ton style » et « N point(s)
// de talent en attente » alors que rien ne dépensait ces points. Un système
// vide est neutre ; un système qui promet ce qu'il ne fait pas est une
// déception programmée. On dit donc ce qui est vrai.
fn draw_talents(ctx: &InlinePageCtx, ui: &mut egui::Ui) {
    section_intro(
        ui,
        "Ton niveau",
        "Ton niveau est la somme de ce que tu as débloqué à l'Enclume.                          Rien à farmer : chaque niveau est un choix que tu as fait.",
    );
    ui.add_space(8.0);
    ui.label(display_text(format!("Niveau {}", ctx.level), 22.0, FORGE_OR));
    ui.add_space(4.0);
    ui.label(display_text(
        if ctx.ranks_remaining == 0 {
            "Enclume complète — tout est débloqué.".to_string()
        } else {
            format!(
                "{} amélioration(s) t'attendent à l'Enclume.",
                ctx.ranks_remaining
            )
        },
        16.0,
        FORGE_CREME,
    ));
}

// Même doctrine que Talents (story-680 cran 1) : ces deux pages promettaient
// des Âmes et des titres qu'aucun système ne verse.
fn draw_missions(_ctx: &InlinePageCtx, ui: &mut egui::Ui) {
    section_intro(
        ui,
        "Missions",
        "Rien à accomplir pour l'instant — les défis quotidiens et \
         hebdomadaires n'existent pas encore. Quand ils arriveront, \
         c'est ici qu'ils t'attendront.",
    );
}

fn draw_succes(_ctx: &InlinePageCtx, ui: &mut egui::Ui) {
    section_intro(
        ui,
        "Hauts faits",
        "Aucun haut fait n'est encore suivi. Cette page s'ouvrira \
         quand tes exploits seront comptés.",
    );
}

fn draw_stats(ctx: &InlinePageCtx, ui: &mut egui::Ui) {
    draw_stats_section(ui, ctx.meta_save, ctx.level);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La liste vient de `MenuPage::TOUTES`, gardée exhaustive PAR LE
    /// COMPILATEUR (const `_EXHAUSTIVITE` de nav.rs, match sans wildcard) —
    /// pas de liste miroir maintenue à la main ici.
    #[test]
    fn chaque_variante_a_exactement_une_declaration() {
        for v in MenuPage::TOUTES {
            assert_eq!(
                PAGES.iter().filter(|d| d.id == v).count(),
                1,
                "{v:?} doit avoir exactement une PageDecl"
            );
        }
        assert_eq!(PAGES.len(), MenuPage::TOUTES.len());
    }

    /// L'ordre des onglets reproduit l'ex-`MenuPage::NAV` — le cycle LB/RB et
    /// la barre en dépendent.
    #[test]
    fn l_ordre_des_onglets_est_celui_d_avant_le_registre() {
        let ordre: Vec<MenuPage> = nav_tabs().map(|d| d.id).collect();
        assert_eq!(
            ordre,
            [
                MenuPage::Root,
                MenuPage::Armes,
                MenuPage::Sac,
                MenuPage::Marketplace,
                MenuPage::Talents,
                MenuPage::Enclume,
                MenuPage::Codex,
                MenuPage::Missions,
                MenuPage::Succes,
                MenuPage::Stats,
                MenuPage::Options,
            ]
        );
    }

    #[test]
    fn les_libelles_et_ids_de_panneau_sont_uniques() {
        for (i, a) in PAGES.iter().enumerate() {
            for b in &PAGES[i + 1..] {
                assert_ne!(a.nav_label, b.nav_label);
                if let (
                    PageDraw::Inline { panel_id: pa, .. },
                    PageDraw::Inline { panel_id: pb, .. },
                ) = (&a.draw, &b.draw)
                {
                    assert_ne!(pa, pb, "deux pages partagent l'id egui {pa}");
                }
            }
        }
    }

    #[test]
    fn seule_l_enclume_porte_une_pastille_de_barre() {
        let avec_badge: Vec<MenuPage> = PAGES
            .iter()
            .filter(|d| d.badge.is_some())
            .map(|d| d.id)
            .collect();
        assert_eq!(avec_badge, [MenuPage::Enclume]);
        let b = HubBadges {
            enclume: true,
            forgeron: false,
            livre: false,
        };
        assert!(decl(MenuPage::Enclume).badge.unwrap()(&b));
    }
}
