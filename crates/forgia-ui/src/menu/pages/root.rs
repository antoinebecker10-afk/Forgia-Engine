//! Page ACCUEIL (racine) — titre, tableau de bord, stats, options.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_core::prelude::*;
use forgia_mode_roguelite::chapters::chapter_da_name;
use forgia_mode_roguelite::decor_palettes::DecorPalettesConfig;
use forgia_mode_roguelite::equipment::{
    power_score, EquipmentConfig, EquipmentSave,
};
use forgia_mode_roguelite::meta_shop::{
    chapter_unlocked, MetaShopSave,
    SelectedChapter, CHAPTERS_PER_BOOK,
};
use forgia_mode_roguelite::rounds::RoundsConfig;
use forgia_ui_lib::pause_menu::{draw_settings_controls, save_user_settings, UserSettings};
use forgia_ui_lib::style::{
    cartoon_btn, glass_btn, glass_frame_hero, C_PRIMARY, C_TEXT_MUTED, FORGE_CREME, FORGE_OR, FORGE_PANEL, HAIR_GOLD_STRONG,
};
use forgia_ui_lib::theme::display_text;

use crate::menu::chrome::{hub_content_top, hub_section_panel};
use crate::menu::nav::{HubBadges, MenuAction, MenuPage};

/// Accueil (page racine) : le titre de l'enseigne. Tout le reste (carrousel de
/// chapitre, dernière run, boutons de départ, équipement) est dessiné par
/// `sys_menu_root_dashboard`, et le personnage vit dans le FOND
/// (`arena_backdrop`), plus dans une carte.
///
/// Le titre était centré en haut ; la barre de navigation horizontale occupe
/// désormais cette place. Il passe donc à gauche, en tête de la colonne de
/// préparation qu'il coiffe — et il cesse d'être une bannière posée sur rien.
/// NOUVELLE PARTIE a fusionné avec CONTINUER ; les démos moteur RPG/Cyber
/// restent hors menu (2026-07-22), Hall de Forgia est dans la carte.
pub(crate) fn draw_root_landing(ctx: &egui::Context) -> MenuAction {
    let (opacity, slide) = forgia_ui_lib::motion::section_enter_anim(ctx, "menu_hub_root");
    egui::Area::new(egui::Id::new("menu_hub_root"))
        .anchor(
            egui::Align2::LEFT_TOP,
            egui::vec2(ROOT_COL_X + slide, hub_content_top()),
        )
        .show(ctx, |ui| {
            ui.set_opacity(opacity);
            // Même axe que la carte en dessous (emprise totale), et `extend()` :
            // un titre qui wrappe (« FORGI / A ») déborde de ROOT_TITLE_H et
            // passe sous la carte — il ne doit JAMAIS revenir à la ligne.
            ui.set_width(ROOT_CARD_INNER_W + 2.0 * ROOT_CARD_MARGIN_X);
            ui.vertical_centered(|ui| {
                ui.add(
                    egui::Label::new(display_text("FORGIA", 64.0, C_PRIMARY).strong()).extend(),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("ROGUELITE")
                            .size(19.0)
                            .color(FORGE_CREME)
                            .strong(),
                    )
                    .extend(),
                );
            });
        });
    MenuAction::None
}

/// Abscisse de la colonne de préparation (carte de chapitre + titre).
///
/// Source unique : le titre, la carte et le panneau d'équipement s'y réfèrent.
/// Le personnage se tient dans le tiers droit du FOND — cette colonne occupe
/// donc la gauche, et rien ne doit venir la chevaucher.
const ROOT_COL_X: f32 = 56.0;

/// Hauteur du bloc-titre, au-dessus de la carte de chapitre.
const ROOT_TITLE_H: f32 = 104.0;

/// Largeur intérieure de la carte de l'Accueil, et sa marge horizontale.
///
/// Grandeurs COUPLÉES : le titre FORGIA se centre sur l'emprise totale de la
/// carte (`inner + 2×margin`) pour partager son axe — deux littéraux séparés
/// avaient déjà donné un titre décalé de la carte qu'il coiffe (rapporté en
/// jeu le 2026-08-08).
const ROOT_CARD_INNER_W: f32 = 340.0;
const ROOT_CARD_MARGIN_X: f32 = 28.0;

/// Section Stats — synthèse de la méta-progression (records + compteurs de runs).
pub(crate) fn draw_stats_section(ui: &mut egui::Ui, save: Option<&MetaShopSave>, level: u32) {
    let (runs, wins, best, souls, weapons) = save
        .map(|s| {
            (
                s.runs_played,
                s.victories,
                s.best_victory_secs,
                s.souls_total,
                s.unlocked_weapons.len(),
            )
        })
        .unwrap_or((0, 0, 0.0, 0, 1));
    let win_rate = if runs > 0 {
        (wins as f32 / runs as f32 * 100.0).round() as u32
    } else {
        0
    };
    let best_str = if best > 0.0 {
        let m = (best / 60.0).floor() as u32;
        let s = (best % 60.0).floor() as u32;
        format!("{m} min {s:02} s")
    } else {
        "—".to_string()
    };
    stat_row(ui, "Runs jouées", runs.to_string());
    stat_row(ui, "Victoires", format!("{wins}  ({win_rate} %)"));
    stat_row(ui, "Meilleure victoire", best_str);
    stat_row(ui, "Âmes accumulées", souls.to_string());
    stat_row(ui, "Armes débloquées", weapons.to_string());
    stat_row(ui, "Niveau", level.to_string());
}

/// Une ligne « libellé … valeur » de la section Stats (verre + liseré or).
fn stat_row(ui: &mut egui::Ui, label: &str, value: String) {
    egui::Frame::new()
        .fill(FORGE_PANEL)
        .inner_margin(egui::Margin::symmetric(16, 9))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
        .show(ui, |ui| {
            ui.set_min_width(460.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).size(16.0).color(FORGE_CREME));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(value)
                            .size(18.0)
                            .color(FORGE_OR)
                            .strong(),
                    );
                });
            });
        });
    ui.add_space(8.0);
}

/// Page Options — réutilise les contrôles du pause menu (DRY), dans un panneau
/// verre centré. « Retour » ramène à l'accueil.
pub(crate) fn draw_options_page(
    ctx: &egui::Context,
    page: &mut ResMut<MenuPage>,
    settings: &mut ResMut<UserSettings>,
) {
    // Story-678 — chrome commun. Sauvegarde IMMÉDIATE au changement : un
    // réglage qu'on perd en quittant est un réglage cassé.
    let mut retour = false;
    hub_section_panel(
        ctx,
        "hub_sec_options",
        MenuPage::Options.section_title(),
        680.0,
        |ui| {
            if draw_settings_controls(ui, settings) {
                save_user_settings(settings);
            }
            ui.add_space(14.0);
            if glass_btn(ui, "‹  Retour").clicked() {
                retour = true;
            }
        },
    );
    if retour {
        **page = MenuPage::Root;
    }
}


/// Story-678 Phase 3 — le tableau de bord de l'accueil : carte « CHAPITRE EN
/// COURS » (carrousel du Livre), bandeau « DERNIÈRE RUN » (gelé par
/// `sys_record_run_stats` aux deux sorties de run), boutons de départ, et le
/// panneau d'équipement (bas-droite).
///
/// Système à part (même motif que `sys_menu_livre`) : `main_menu_ui` frôle le
/// plafond de 16 paramètres Bevy. Toutes les Resources roguelite sont
/// optionnelles — l'accueil s'affiche même sans elles.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sys_menu_root_dashboard(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut page: ResMut<MenuPage>,
    // `ResMut` : actionner le carrousel vaut « j'ai vu les nouveaux chapitres »
    // et éteint la pastille du Livre (cf. `sys_hub_badges`).
    save: Option<ResMut<MetaShopSave>>,
    mut selected: Option<ResMut<SelectedChapter>>,
    palettes: Option<Res<DecorPalettesConfig>>,
    rounds_cfg: Option<Res<RoundsConfig>>,
    equip_cfg: Option<Res<EquipmentConfig>>,
    equip_save: Option<Res<EquipmentSave>>,
    badges: Res<HubBadges>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut exit: MessageWriter<AppExit>,
) {
    if *app_state.get() != AppMode::Menu || *page != MenuPage::Root {
        return;
    }
    let Some(mut save) = save else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let chapitre = selected
        .as_deref()
        .map(|s| s.clamped(save.chapters_cleared))
        .unwrap_or(1);
    let da = chapter_da_name(palettes.as_deref(), chapitre);
    let menace = rounds_cfg
        .map(|c| c.threat_at(chapitre, 0).hp)
        .unwrap_or(1.0);

    // Sorties de l'UI, appliquées APRÈS les closures (emprunts propres).
    let mut new_chap = chapitre;
    let mut action = MenuAction::None;
    let mut goto_forgeron = false;
    let mut goto_livre = false;
    let mut carrousel_actionne = false;

    let (opacity, slide) = forgia_ui_lib::motion::section_enter_anim(ctx, "menu_hub_root");

    // ── Colonne GAUCHE : LE LIVRE (carrousel) + dernière run + départ ──
    // Sous le bloc-titre, alignée sur la même abscisse : l'écran se lit en deux
    // temps — ce que je prépare à gauche, qui je suis à droite (dans le décor).
    egui::Area::new(egui::Id::new("menu_hub_root_dashboard"))
        .anchor(
            egui::Align2::LEFT_TOP,
            egui::vec2(ROOT_COL_X + slide, hub_content_top() + ROOT_TITLE_H),
        )
        .show(ctx, |ui| {
            ui.set_opacity(opacity);
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(ROOT_CARD_MARGIN_X as i8, 22))
                .show(ui, |ui| {
                    ui.set_width(ROOT_CARD_INNER_W);
                    ui.vertical_centered(|ui| {
                        // Pastille « un chapitre s'est ouvert » — posée sur le
                        // titre du bloc, qui est aussi la PORTE de la page Livre
                        // complète (même modèle que « Personnaliser » → Forgeron :
                        // hors barre de nav, accessible depuis le bloc de
                        // l'Accueil qui a repris son contenu — sans ce clic, la
                        // vue d'ensemble des chapitres était orpheline).
                        let titre = ui
                            .add(
                                egui::Label::new(display_text("LE LIVRE", 18.0, FORGE_OR))
                                    .sense(egui::Sense::click()),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text("Ouvrir le Livre — tous les chapitres");
                        forgia_ui_lib::ui_sfx::instrument_hover(&titre);
                        if titre.clicked() {
                            goto_livre = true;
                        }
                        if badges.livre {
                            ui.painter().circle_filled(
                                egui::pos2(titre.rect.right() + 12.0, titre.rect.center().y),
                                5.0,
                                C_PRIMARY,
                            );
                        }
                        ui.add_space(6.0);
                        // Carrousel ‹ CHAPITRE N › — borné aux chapitres ouverts.
                        ui.horizontal(|ui| {
                            let w = ui.available_width();
                            let side = 36.0;
                            let prev_ok = chapitre > 1;
                            let next_ok =
                                chapter_unlocked(chapitre + 1, save.chapters_cleared);
                            ui.add_space((w - ROOT_CARD_INNER_W).max(0.0) / 2.0);
                            let prev = ui.add_enabled(
                                prev_ok,
                                egui::Button::new(
                                    egui::RichText::new("<").size(24.0).color(FORGE_OR),
                                )
                                .fill(FORGE_PANEL)
                                .min_size(egui::vec2(side, side)),
                            );
                            ui.add_space(8.0);
                            ui.add_sized(
                                egui::vec2(220.0, side),
                                egui::Label::new(
                                    display_text(format!("CHAPITRE {chapitre}"), 28.0, FORGE_OR)
                                        .strong(),
                                ),
                            );
                            ui.add_space(8.0);
                            let next = ui.add_enabled(
                                next_ok,
                                egui::Button::new(
                                    egui::RichText::new(">").size(24.0).color(FORGE_OR),
                                )
                                .fill(FORGE_PANEL)
                                .min_size(egui::vec2(side, side)),
                            );
                            if prev.clicked() {
                                new_chap = chapitre - 1;
                            }
                            if next.clicked() {
                                new_chap = chapitre + 1;
                            }
                            // Feuilleter le Livre, c'est l'avoir consulté : la
                            // pastille « nouveau chapitre » s'éteint ici, et
                            // nulle part ailleurs sur cet écran (cf. le pourquoi
                            // dans `sys_hub_badges`).
                            if prev.clicked() || next.clicked() {
                                carrousel_actionne = true;
                            }
                        });
                        if !da.is_empty() {
                            ui.label(egui::RichText::new(da).size(15.0).color(FORGE_CREME));
                        }
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("Menace ×{menace:.2}"))
                                .size(13.0)
                                .color(C_TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} / {} chapitres battus",
                                save.chapters_cleared, CHAPTERS_PER_BOOK
                            ))
                            .size(13.0)
                            .color(C_TEXT_MUTED),
                        );
                        // ── Bandeau DERNIÈRE RUN ──
                        if let Some(run) = &save.last_run {
                            ui.add_space(12.0);
                            ui.separator();
                            ui.add_space(6.0);
                            ui.label(display_text("DERNIÈRE RUN", 16.0, FORGE_OR));
                            let (verdict, couleur) = if run.victory {
                                ("Victoire", FORGE_OR)
                            } else {
                                ("Défaite", C_TEXT_MUTED)
                            };
                            let mins = (run.duration_secs / 60.0) as u32;
                            let secs = (run.duration_secs % 60.0) as u32;
                            ui.label(
                                egui::RichText::new(format!(
                                    "Chapitre {} — {verdict} · round {} · {mins}:{secs:02}",
                                    run.chapter, run.rounds_reached
                                ))
                                .size(13.0)
                                .color(couleur),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "+{} Âmes rapportées",
                                    run.souls_earned
                                ))
                                .size(13.0)
                                .color(FORGE_CREME),
                            );
                            if run.new_best {
                                ui.label(
                                    egui::RichText::new("NOUVEAU RECORD !")
                                        .size(13.0)
                                        .color(FORGE_OR)
                                        .strong(),
                                );
                            }
                        }
                        // ── Boutons de départ (tout sur l'écran, modèle Dicero) ──
                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(10.0);
                        if cartoon_btn(ui, "▶  CONTINUER", FORGE_OR).clicked() {
                            action = MenuAction::Launch(GameMode::Roguelite);
                        }
                        ui.add_space(8.0);
                        if glass_btn(ui, "🏰  Hall de Forgia").clicked() {
                            action = MenuAction::Launch(GameMode::CastleHub);
                        }
                        ui.add_space(8.0);
                        if glass_btn(ui, "QUITTER").clicked() {
                            action = MenuAction::Quit;
                        }
                    });
                });
        });

    // ── Colonne DROITE : ce que porte le personnage ──
    //
    // Le portrait en carte a disparu : le personnage est DANS le décor du fond,
    // à droite de l'écran (`arena_backdrop`). Ne reste ici que ce qu'une image
    // ne dit pas : le score de puissance et la liste des pièces avec leur
    // rareté. Ancré en BAS à droite pour laisser le buste dégagé au-dessus.
    egui::Area::new(egui::Id::new("menu_hub_root_gear"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-56.0 - slide, -48.0))
        .show(ctx, |ui| {
            ui.set_opacity(opacity);
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(22, 16))
                .show(ui, |ui| {
                    ui.set_width(290.0);
                    ui.vertical_centered(|ui| {
                        if let (Some(cfg), Some(esave)) =
                            (equip_cfg.as_deref(), equip_save.as_deref())
                        {
                            let score = power_score(cfg, esave);
                            ui.label(
                                egui::RichText::new(format!(
                                    "Puissance {score}  ·  record {}",
                                    esave.power_record
                                ))
                                .size(14.0)
                                .color(FORGE_CREME),
                            );
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(6.0);
                            // Pièces portées : pastille couleur rareté + emplacement.
                            for slot in &cfg.slots {
                                let equipped = esave.equipped.get(&slot.id);
                                let (col, texte) = match equipped {
                                    Some(rid) => {
                                        let rgb =
                                            cfg.rarity(rid).map(|r| r.rgb).unwrap_or([0.5; 3]);
                                        (
                                            egui::Color32::from_rgb(
                                                (rgb[0] * 255.0) as u8,
                                                (rgb[1] * 255.0) as u8,
                                                (rgb[2] * 255.0) as u8,
                                            ),
                                            format!(
                                                "{} — {}",
                                                slot.label,
                                                cfg.rarity(rid)
                                                    .map(|r| r.label.as_str())
                                                    .unwrap_or(rid)
                                            ),
                                        )
                                    }
                                    None => (
                                        egui::Color32::from_gray(90),
                                        format!("{} — vide", slot.label),
                                    ),
                                };
                                ui.horizontal(|ui| {
                                    let (dot, _) = ui.allocate_exact_size(
                                        egui::vec2(14.0, 14.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().circle_filled(dot.center(), 5.0, col);
                                    ui.label(
                                        egui::RichText::new(texte).size(13.0).color(FORGE_CREME),
                                    );
                                });
                            }
                        }
                        ui.add_space(10.0);
                        // Pastille « une pièce que tu n'as jamais regardée » —
                        // sur le bouton qui mène à la fiche.
                        let perso = glass_btn(ui, "⚒  Personnaliser");
                        if badges.forgeron {
                            ui.painter().circle_filled(
                                egui::pos2(perso.rect.right() - 12.0, perso.rect.center().y),
                                5.0,
                                C_PRIMARY,
                            );
                        }
                        if perso.clicked() {
                            goto_forgeron = true;
                        }
                    });
                });
        });

    // ── Application des sorties ──
    if new_chap != chapitre {
        if let Some(sel) = selected.as_deref_mut() {
            sel.0 = new_chap;
        }
    }
    if carrousel_actionne && save.seen_chapters_cleared != save.chapters_cleared {
        save.seen_chapters_cleared = save.chapters_cleared;
        save.save();
    }
    if goto_forgeron {
        *page = MenuPage::Forgeron;
    }
    if goto_livre {
        *page = MenuPage::Livre;
    }
    match action {
        MenuAction::Launch(mode) => {
            next_game.set(mode);
            next_app.set(AppMode::InGame);
        }
        MenuAction::Quit => {
            exit.write(AppExit::Success);
        }
        MenuAction::None => {}
    }
}
