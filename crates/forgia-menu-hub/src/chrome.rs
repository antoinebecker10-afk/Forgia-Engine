//! Chrome commun du hub-menu : constantes de layout (source unique
//! `HUB_TOP_BAR_H`), cadre verre des chips, chips d'état (Âmes, Forgeron)
//! et panneau de section centré.

use bevy_egui::egui;
use forgia_ui_lib::style::{
    glass_frame_hero, C_TEXT_MUTED, FORGE_AME, FORGE_CREME, FORGE_ECLAT, FORGE_OR, FORGE_PANEL,
    HAIR_GOLD_STRONG,
};
use forgia_ui_lib::theme::display_text;

use crate::currency_icons::{CurrencyIcons, CURRENCY_ICON, CURRENCY_ICON_SMALL};

/// Hauteur réservée en haut de l'écran par le chrome du hub (barre de
/// navigation horizontale + chips d'état qui l'encadrent).
///
/// **Source unique.** La barre, les chips et TOUTES les pages s'y réfèrent :
/// une page qui recopierait sa propre marge finirait par passer sous la barre
/// le jour où celle-ci change de taille (`feedback_une_grandeur_ecrite_deux_fois`).
pub(crate) const HUB_TOP_BAR_H: f32 = 84.0;

/// Respiration entre le chrome du haut et le contenu de la page.
const HUB_CONTENT_GAP: f32 = 18.0;

/// Ordonnée du haut de tout contenu de page.
pub(crate) fn hub_content_top() -> f32 {
    HUB_TOP_BAR_H + HUB_CONTENT_GAP
}

/// Cadre « chip » cohérent avec le hub (verre aubergine + liseré or).
fn hub_chip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(FORGE_PANEL)
        .inner_margin(egui::Margin::symmetric(14, 8))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
}

/// Bandeau des deux monnaies — haut-droite du hub-menu.
///
/// Story-678 : les **Éclats** (cosmétique) sont affichés SOUS les Âmes
/// (puissance). Les montrer ensemble est le seul moyen de faire comprendre
/// qu'elles ne se remplacent pas — une seule ligne « 1729 » laisserait croire
/// qu'un achat au Marketplace coûte des rangs d'Enclume.
pub(crate) fn draw_hub_souls_chip(
    ctx: &egui::Context,
    souls_n: u32,
    shards: u32,
    icons: Option<&CurrencyIcons>,
) {
    let (i_souls, i_shards) = icons.map_or((None, None), |i| (i.souls, i.shards));
    egui::Area::new(egui::Id::new("menu_hub_souls"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-24.0, 18.0))
        .show(ctx, |ui| {
            hub_chip_frame().show(ui, |ui| {
                ui.vertical(|ui| {
                    // Largeur réservée : sans elle le chip se dimensionne sur son
                    // contenu, et l'ajout de l'icône a fait passer « 1852 Âmes »
                    // à la ligne (vu à l'inspection). Assez large pour 5 chiffres
                    // + le mot + l'icône.
                    ui.set_min_width(150.0);
                    ui.horizontal(|ui| {
                        CurrencyIcons::show(ui, i_souls, CURRENCY_ICON);
                        ui.label(
                            // FORGE_AME et pas FORGE_OR : l'or est la couleur
                            // des pièces de run — le texte suit le saphir.
                            egui::RichText::new(format!("{souls_n}  Âmes"))
                                .size(20.0)
                                .color(FORGE_AME)
                                .strong(),
                        );
                    });
                    ui.horizontal(|ui| {
                        CurrencyIcons::show(ui, i_shards, CURRENCY_ICON_SMALL);
                        ui.label(
                            // FORGE_ECLAT et pas C_PRIMARY : celui-ci est un
                            // alias de FORGE_OR, les deux monnaies sortaient
                            // dans le même or.
                            egui::RichText::new(format!("{shards}  Éclats"))
                                .size(15.0)
                                .color(FORGE_ECLAT)
                                .strong(),
                        );
                    });
                });
            });
        });
}

/// Chip forgeron (haut-GAUCHE) : nom + niveau + avancement de l'Enclume.
///
/// Il vivait en tête de la sidebar verticale. La barre de navigation est passée
/// à l'horizontale (2026-08-06) et ne peut plus porter trois lignes de texte :
/// l'identité prend donc son propre chip, en miroir du chip Âmes à droite. Le
/// haut de l'écran se lit alors comme un bandeau : QUI je suis · OÙ je vais ·
/// CE QUE j'ai.
pub(crate) fn draw_hub_smith_chip(ctx: &egui::Context, name: &str, level: u32, remaining: u32, frac: f32) {
    egui::Area::new(egui::Id::new("menu_hub_smith"))
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(24.0, 18.0))
        .show(ctx, |ui| {
            hub_chip_frame().show(ui, |ui| {
                ui.set_min_width(180.0);
                ui.label(
                    egui::RichText::new(name)
                        .size(17.0)
                        .color(FORGE_CREME)
                        .strong(),
                );
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Niveau {level}"))
                            .size(12.0)
                            .color(C_TEXT_MUTED),
                    );
                });
                ui.add_space(3.0);
                // 14 px de haut, pas 8 : la barre porte un texte de 9 px, et
                // une barre plus basse que son propre libellé le coupe en deux
                // (vu à l'inspection du 2026-08-06).
                ui.add_sized(
                    egui::vec2(180.0, 14.0),
                    egui::ProgressBar::new(frac).fill(FORGE_OR).text(
                        egui::RichText::new(if remaining == 0 {
                            "COMPLET".to_string()
                        } else {
                            format!("{remaining} À DÉBLOQUER")
                        })
                        .size(10.0)
                        // Texte SOMBRE : il est posé sur le remplissage doré de
                        // la barre, et le crème d'origine s'y noyait.
                        .color(egui::Color32::from_rgb(40, 26, 10))
                        .strong(),
                    ),
                );
            });
        });
}


/// Panneau de section centré (verre + liseré or) — chrome commun aux sections
/// data du hub.
///
/// Ancré en HAUT depuis le passage de la nav à l'horizontale : centré
/// verticalement, une page haute remontait sous la barre. Il part donc sous
/// [`hub_content_top`] et descend — la seule ancre qui garantit qu'aucune
/// section ne passe derrière le chrome, quelle que soit sa hauteur.
pub(crate) fn hub_section_panel(
    ctx: &egui::Context,
    id: &'static str,
    title: &str,
    max_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    // Story-678 Phase 2 — entrée de section en fondu + glissement (150 ms).
    // La clé = l'id de section : naviguer relance l'anim, re-render non.
    let (opacity, slide) = forgia_ui_lib::motion::section_enter_anim(ctx, id);
    // ── Plafond de hauteur + défilement (audit 2026-08-06, bloquant n°2) ──
    // Une `Area` dont le contenu dépasse l'écran est REMONTÉE par egui : le
    // titre passait par-dessus la barre de navigation, et le panneau invisible
    // MANGEAIT ses clics — depuis Options, la moitié des onglets ne répondait
    // plus. Le contenu défile désormais dans le panneau ; le panneau, lui, ne
    // dépasse jamais.
    // ⚠ viewport_rect, PAS content_rect : le fond vidéo du menu est un
    // CentralPanel qui CONSOMME l'espace du contexte — content_rect mesurait
    // ce qui restait APRÈS lui, et le panneau s'arrêtait à mi-écran (vu le
    // 2026-08-06, « plus adapté à la taille de mon écran »). Les Areas
    // flottent au-dessus des panels : c'est bien le viewport qui fait foi.
    let ecran_h = ctx
        .data(|d| d.get_temp::<f32>(egui::Id::new("forgia_viewport_h")))
        .unwrap_or(1080.0);
    let max_h = (ecran_h - hub_content_top() - 24.0).max(220.0);
    // Réserve du chrome interne : titre (40 px) + respirations + marges du cadre.
    let scroll_h = max_h - 130.0;
    egui::Area::new(egui::Id::new(id))
        .anchor(
            egui::Align2::CENTER_TOP,
            egui::vec2(slide, hub_content_top()),
        )
        .show(ctx, |ui| {
            ui.set_opacity(opacity);
            // 🚨 LA LIGNE QUI MANQUAIT (mesurée le 2026-08-07, sonde ci-dessous).
            //
            // Une `Area` egui ne devine pas la place qu'on veut lui laisser :
            // elle crée son `Ui` avec un `max_rect` de 400 px de haut, quoi
            // qu'on calcule par ailleurs. Mon plafond de 824 n'était donc
            // JAMAIS atteint — trois correctifs successifs (content_rect,
            // viewport_rect, hauteur de fenêtre Bevy) ont réparé un chiffre
            // qui n'a jamais été le limiteur. `set_max_height` assigne le
            // `max_rect` : il ÉLARGIT autant qu'il restreint.
            ui.set_max_height(max_h);
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(40, 30))
                .show(ui, |ui| {
                    ui.set_max_width(max_width);
                    ui.vertical_centered(|ui| {
                        ui.label(display_text(title, 40.0, FORGE_OR).strong());
                    });
                    ui.add_space(16.0);
                    egui::ScrollArea::vertical()
                        .max_height(scroll_h)
                        .show(ui, |ui| {
                            ui.set_max_width(max_width);
                            ui.vertical_centered(|ui| add_contents(ui));
                        });
                });
        });
}

