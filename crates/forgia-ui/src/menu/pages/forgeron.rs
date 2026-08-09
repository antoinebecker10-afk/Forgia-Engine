//! Pages FORGERON et SAC — la fiche (poupée + sac + caractéristiques).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_core::prelude::*;
use forgia_mode_roguelite::equipment::{
    power_score, EquipmentConfig, EquipmentMods, EquipmentPanelShown, EquipmentSave,
};
use forgia_mode_roguelite::identity::{draw_identity_content, IdentityConfig, IdentitySave};
use forgia_ui_lib::style::{
    glass_btn, C_TEXT_MUTED, FORGE_CREME, FORGE_OR, HAIR_GOLD_STRONG,
};
use forgia_ui_lib::theme::display_text;

use crate::menu::chrome::hub_section_panel;
use crate::menu::nav::MenuPage;
use crate::{slot_glyph, weapon_preview};

/// LA FICHE — le personnage encadré par ses emplacements, le sac en grille à
/// côté, les caractéristiques dessous (structure de la référence « Classic RPG
/// UI », dans notre DA). « Sac » et « Forgeron » ouvrent ce MÊME écran : la
/// fiche et l'inventaire se regardent, les séparer obligerait à mémoriser ce
/// qu'on porte en changeant d'onglet.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sys_menu_forgeron(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut page: ResMut<MenuPage>,
    cfg: Res<IdentityConfig>,
    mut save: ResMut<IdentitySave>,
    mut arm_cosmetics: ResMut<ArmCosmetics>,
    mut editing: Local<bool>,
    eq_cfg: Res<EquipmentConfig>,
    mut eq_save: ResMut<EquipmentSave>,
    // Le bloc CARACTÉRISTIQUES lit les bonus agrégés du build.
    eq_mods: Res<EquipmentMods>,
    // La case choisie d'un simple clic (emplacement, rareté) — le double-clic
    // équipe. `Local` : c'est un état d'écran, rien à faire en Resource.
    mut selection: Local<Option<(String, String)>>,
    mut eq_shown: ResMut<EquipmentPanelShown>,
    rtt: Option<Res<weapon_preview::CharacterPreviewRtt>>,
) {
    if *app_state.get() != AppMode::Menu
        || !matches!(*page, MenuPage::Forgeron | MenuPage::Sac)
    {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    // Posé seulement une fois le panneau RÉELLEMENT dessiné : le capteur ne doit
    // pas annoncer « affiché » pour quelque chose d'invisible.
    eq_shown.0 = true;

    let character = rtt.as_ref().map(|r| r.tex_id);
    // Repli quand le rendu hors écran n'a pas encore spawné : la pastille de la
    // couleur portée, pour que la place ne soit jamais vide.
    let rgb = cfg
        .colors
        .iter()
        .find(|c| c.id == save.equipped_color)
        .map(|c| c.rgb)
        .unwrap_or([0.6, 0.6, 0.6]);
    let disc_col = egui::Color32::from_rgb(
        (rgb[0] * 255.0) as u8,
        (rgb[1] * 255.0) as u8,
        (rgb[2] * 255.0) as u8,
    );

    // Story-678 — chrome commun (titre/marges/transition standard). Le titre
    // suit l'onglet d'entrée : même écran, mais « TON SAC » quand on vient
    // pour l'inventaire.
    let titre = if *page == MenuPage::Sac {
        MenuPage::Sac.section_title()
    } else {
        MenuPage::Forgeron.section_title()
    };
    let mut back = false;
    let mut goto_decors = false;
    // Clic sur un emplacement de la poupée → SÉLECTIONNE sa pièce dans le sac
    // (surbrillance or). Même écran, donc pas de navigation : on guide l'œil.
    let mut choisir_slot: Option<String> = None;
    let mut pose: Option<(String, Option<String>)> = None;
    hub_section_panel(ctx, "hub_sec_forgeron", titre, 1000.0, |ui| {
        // ── La fiche, structurée d'après la référence « Classic RPG UI » :
        // le personnage ENCADRÉ par ses cellules d'équipement, l'inventaire en
        // grille à côté, les caractéristiques dessous. Chaque colonne a une
        // largeur FIXE : un `vertical_centered` lâché dans l'horizontal
        // réclamait toute la largeur restante et poussait le sac hors de
        // l'écran, écrasé en colonne de lettres (audit 2026-08-06, bloquant n°1).
        ui.horizontal_top(|ui| {
            // ── Haut du corps, collé au personnage ──
            ui.vertical(|ui| {
                ui.set_min_width(PAPERDOLL_COL_W);
                ui.set_max_width(PAPERDOLL_COL_W);
                for (i, slot) in eq_cfg.slots.iter().enumerate() {
                    if i % 2 != 0 {
                        continue;
                    }
                    if paperdoll_slot(ui, &eq_cfg, &eq_save, slot) {
                        choisir_slot = Some(slot.id.clone());
                    }
                }
            });
            ui.add_space(6.0);
            // ── Le personnage ──
            ui.vertical(|ui| {
                ui.set_min_width(280.0);
                ui.set_max_width(280.0);
                match character {
                    Some(tex) => {
                        ui.add(egui::Image::new(egui::load::SizedTexture::new(
                            tex,
                            egui::vec2(280.0, 340.0),
                        )));
                    }
                    None => {
                        let (r, _) = ui.allocate_exact_size(
                            egui::vec2(280.0, 340.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().circle_filled(r.center(), 60.0, disc_col);
                        ui.painter().circle_stroke(
                            r.center(),
                            60.0,
                            egui::Stroke::new(2.5, HAIR_GOLD_STRONG),
                        );
                    }
                }
                ui.vertical_centered(|ui| {
                    let p = power_score(&eq_cfg, &eq_save);
                    ui.label(display_text(format!("Puissance {p}"), 22.0, FORGE_OR).strong());
                    if eq_save.power_record > p {
                        ui.label(
                            egui::RichText::new(format!("record {}", eq_save.power_record))
                                .size(12.0)
                                .color(C_TEXT_MUTED),
                        );
                    }
                });
            });
            ui.add_space(6.0);
            // ── Bas du corps ──
            ui.vertical(|ui| {
                ui.set_min_width(PAPERDOLL_COL_W);
                ui.set_max_width(PAPERDOLL_COL_W);
                for (i, slot) in eq_cfg.slots.iter().enumerate() {
                    if i % 2 == 0 {
                        continue;
                    }
                    if paperdoll_slot(ui, &eq_cfg, &eq_save, slot) {
                        choisir_slot = Some(slot.id.clone());
                    }
                }
            });
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);
            // ── LE SAC, à côté de la fiche ──
            if let Some(a) = draw_sac(ui, &eq_cfg, &eq_save, &mut selection) {
                pose = Some(a);
            }
        });

        // ── CARACTÉRISTIQUES — le bloc « Characteristic » de la référence ──
        // La seule vue qui dit le PROFIL du build (où portent les bonus), là où
        // la Puissance n'en dit que la somme.
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let stats: [(&str, f32); 5] = [
                ("Dégâts", (eq_mods.damage_mul - 1.0) * 100.0),
                ("Cadence", (eq_mods.fire_rate_mul - 1.0) * 100.0),
                ("Blindage", eq_mods.damage_reduction * 100.0),
                ("Critique", eq_mods.crit_chance * 100.0),
                ("Visée", (eq_mods.headshot_bonus_mul - 1.0) * 100.0),
            ];
            for (nom, valeur) in stats {
                ui.vertical(|ui| {
                    ui.set_min_width(120.0);
                    ui.label(egui::RichText::new(nom).size(11.0).color(C_TEXT_MUTED));
                    let txt = egui::RichText::new(format!("+{valeur:.0}%")).size(16.0).strong();
                    ui.label(if valeur > 0.05 {
                        txt.color(FORGE_OR)
                    } else {
                        txt.color(C_TEXT_MUTED)
                    });
                });
            }
        });

        // ── Qui il est : nom, couleur, bras ──
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);
        draw_identity_content(ui, &cfg, &mut save, &mut arm_cosmetics, &mut editing);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if glass_btn(ui, "‹  Retour").clicked() {
                back = true;
            }
            // Le Marketplace est cosmétique : sa porte est sur la page
            // « qui je suis », en plus de la barre de navigation.
            if glass_btn(ui, "💰  Marketplace").clicked() {
                goto_decors = true;
            }
        });
    });
    if back {
        *page = MenuPage::Root;
    }
    if goto_decors {
        *page = MenuPage::Marketplace;
    }
    if let Some(slot_id) = choisir_slot {
        // La pièce montrée en priorité : celle qu'on porte, sinon la meilleure
        // possédée, sinon la première de l'échelle — jamais rien.
        let rarity = eq_save
            .equipped
            .get(&slot_id)
            .cloned()
            .or_else(|| {
                eq_save.owned.get(&slot_id).and_then(|v| {
                    v.iter()
                        .max_by_key(|r| eq_cfg.rarity_rank(r))
                        .cloned()
                })
            })
            .or_else(|| eq_cfg.rarities.first().map(|r| r.id.clone()));
        if let Some(r) = rarity {
            *selection = Some((slot_id, r));
        }
    }
    if let Some((slot_id, rarity)) = pose {
        match rarity {
            Some(r) => {
                eq_save.equipped.insert(slot_id, r);
                forgia_ui_lib::ui_sfx::push_ui_sfx(ctx, forgia_ui_lib::ui_sfx::UiSfxKind::Buy);
            }
            None => {
                eq_save.equipped.remove(&slot_id);
                forgia_ui_lib::ui_sfx::push_ui_sfx(ctx, forgia_ui_lib::ui_sfx::UiSfxKind::Tab);
            }
        }
        // 🚨 PERSISTER ICI, et NE PAS toucher `power_record`.
        //
        // Confirmé par l'audit du 2026-08-07 : `sys_track_power_record` est le
        // seul écrivain du disque pour cette sauvegarde, et sa condition est
        // « score > power_record ». En rehaussant le record moi-même juste
        // avant, je DÉSARMAIS son déclencheur : équiper au menu n'écrivait
        // jamais rien, et le choix était perdu au relancement. Équiper une
        // pièce MOINS bonne ne déclenchait rien non plus.
        //
        // Le suivi du record reste sa responsabilité — mutation + écriture
        // explicites ici, calcul du pic là-bas. Même classe que le défaut
        // « achat sans effet » côté Âmes : un garde d'autosave qui ne pousse
        // que les gains ne persiste pas les changements.
        eq_save.save();
    }
}

/// Largeur d'une colonne d'emplacements de la poupée.
const PAPERDOLL_COL_W: f32 = 96.0;
/// Côté du cadre d'un emplacement.
const PAPERDOLL_SLOT: f32 = 64.0;

/// Un emplacement de la poupée : la cellule (silhouette du type à la couleur de
/// la rareté portée), son nom dessous. Rend `true` au clic — l'appelant met la
/// pièce en surbrillance dans le sac.
///
/// Cellule VERTICALE compacte, comme la référence « Classic RPG UI » : les
/// colonnes se collent au personnage au lieu d'étaler un libellé à côté.
fn paperdoll_slot(
    ui: &mut egui::Ui,
    cfg: &EquipmentConfig,
    save: &EquipmentSave,
    slot: &forgia_mode_roguelite::equipment::SlotDef,
) -> bool {
    let porte = save.equipped.get(&slot.id);
    let col = porte
        .map(|id| cfg.color32(id))
        .unwrap_or(egui::Color32::from_gray(70));
    let mut clique = false;

    ui.vertical_centered(|ui| {
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(PAPERDOLL_SLOT, PAPERDOLL_SLOT),
            egui::Sense::click(),
        );
        let p = ui.painter();
        p.rect_filled(
            rect,
            egui::CornerRadius::same(10),
            egui::Color32::from_black_alpha(120),
        );
        // La SILHOUETTE du type, à la couleur de la rareté portée. Un
        // emplacement vide la garde, éteinte — la place se lit même libre.
        slot_glyph::draw(
            p,
            rect.shrink(10.0),
            &slot.id,
            if porte.is_some() {
                col
            } else {
                col.gamma_multiply(0.35)
            },
        );
        p.rect_stroke(
            rect,
            egui::CornerRadius::same(10),
            egui::Stroke::new(if resp.hovered() { 2.5 } else { 1.5 }, col),
            egui::StrokeKind::Inside,
        );
        // PARESSEUX : formatée seulement au survol (P1bis), comme au sac.
        let resp = resp.on_hover_ui(|ui| {
            let bulle = match porte {
                Some(id) => format!(
                    "{} — {}
{} +{:.0}%

Clic : voir dans le sac",
                    slot.label,
                    cfg.rarity(id).map(|r| r.label.as_str()).unwrap_or(id),
                    slot.stat_label,
                    cfg.rarity(id)
                        .map(|r| slot.per_tier * r.bonus_mul * 100.0)
                        .unwrap_or(0.0)
                ),
                None => format!(
                    "{} — vide
{}

Clic : voir dans le sac",
                    slot.label, slot.stat_label
                ),
            };
            ui.label(bulle);
        });
        if resp.clicked() {
            clique = true;
        }
        ui.label(
            egui::RichText::new(&slot.label)
                .size(12.0)
                .strong()
                .color(if porte.is_some() {
                    FORGE_CREME
                } else {
                    C_TEXT_MUTED
                }),
        );
        ui.label(
            egui::RichText::new(match porte {
                Some(id) => cfg.rarity(id).map(|r| r.label.as_str()).unwrap_or(id),
                None => "vide",
            })
            .size(10.0)
            .color(col),
        );
    });
    ui.add_space(8.0);
    clique
}

/// Côté d'une case du sac.
const SAC_CASE: f32 = 52.0;

/// **LE SAC**, dessiné À CÔTÉ de la poupée (story-678, 2026-08-06).
///
/// Pas une page à part : la fiche de personnage et l'inventaire se regardent,
/// c'est tout l'intérêt. On voit ce qu'on porte et ce qu'on pourrait porter dans
/// le même écran — la convention Diablo, et la raison pour laquelle on n'a pas
/// à mémoriser ce qu'on portait en ouvrant un autre onglet.
///
/// **Double-clic pour équiper.** Le simple clic SÉLECTIONNE (et affiche la
/// comparaison) : sur une grille dense, un clic qui équipe transforme le moindre
/// survol maladroit en changement de build.
///
/// Rend l'action à appliquer, `None` si rien n'a été demandé. La mutation est
/// faite par l'appelant : muter la sauvegarde pendant qu'on itère dessus
/// emprunterait deux fois la même donnée.
fn draw_sac(
    ui: &mut egui::Ui,
    cfg: &EquipmentConfig,
    save: &EquipmentSave,
    selection: &mut Option<(String, String)>,
) -> Option<(String, Option<String>)> {
    let mut action = None;
    let possedees: usize = cfg
        .slots
        .iter()
        .map(|s| save.owned.get(&s.id).map_or(0, |v| v.len()))
        .sum();
    let total = cfg.slots.len() * cfg.rarities.len();

    ui.vertical(|ui| {
        ui.label(display_text("LE SAC", 22.0, FORGE_OR).strong());
        ui.label(
            egui::RichText::new(format!("{possedees} / {total} pièces trouvées"))
                .size(12.0)
                .color(C_TEXT_MUTED),
        );
        ui.label(
            egui::RichText::new("Double-clic pour équiper.")
                .size(12.0)
                .color(C_TEXT_MUTED),
        );
        ui.add_space(10.0);

        for slot in &cfg.slots {
            let porte = save.equipped.get(&slot.id).cloned();
            let porte_gain = porte
                .as_deref()
                .and_then(|id| cfg.rarity(id))
                .map(|r| slot.per_tier * r.bonus_mul)
                .unwrap_or(0.0);
            let porte_power = porte.as_deref().map(|id| cfg.power_of(id)).unwrap_or(0);

            ui.label(
                egui::RichText::new(format!("{}  ·  {}", slot.label, slot.stat_label))
                    .size(12.0)
                    .color(C_TEXT_MUTED),
            );
            ui.horizontal(|ui| {
                for rarity in &cfg.rarities {
                    let a_soi = save
                        .owned
                        .get(&slot.id)
                        .is_some_and(|v| v.iter().any(|r| r == &rarity.id));
                    let sur_soi = porte.as_deref() == Some(rarity.id.as_str());
                    // Sans clone : deux Strings par case et par frame juste
                    // pour COMPARER (audit 2026-08-07, P1bis).
                    let choisie = selection
                        .as_ref()
                        .is_some_and(|(s, r)| *s == slot.id && *r == rarity.id);
                    let sense = if a_soi {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    };
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(SAC_CASE, SAC_CASE), sense);
                    let col = cfg.color32(&rarity.id);

                    // Le fond dit la POSSESSION, la silhouette dit le TYPE, sa
                    // couleur dit la RARETÉ. Trois informations, aucun mot.
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::same(8),
                        if a_soi {
                            egui::Color32::from_black_alpha(150)
                        } else {
                            egui::Color32::from_black_alpha(70)
                        },
                    );
                    slot_glyph::draw(
                        ui.painter(),
                        rect.shrink(9.0),
                        &slot.id,
                        if a_soi {
                            col
                        } else {
                            // Non trouvée : la place est réservée et la couleur
                            // annoncée, mais l'absence se lit sans comparer deux
                            // listes.
                            col.gamma_multiply(0.22)
                        },
                    );
                    if sur_soi {
                        ui.painter().rect_stroke(
                            rect,
                            egui::CornerRadius::same(8),
                            egui::Stroke::new(2.5, egui::Color32::WHITE),
                            egui::StrokeKind::Outside,
                        );
                    } else if choisie {
                        ui.painter().rect_stroke(
                            rect,
                            egui::CornerRadius::same(8),
                            egui::Stroke::new(2.0, FORGE_OR),
                            egui::StrokeKind::Outside,
                        );
                    }

                    // PARESSEUX (`on_hover_ui`) : ces bulles étaient formatées
                    // pour CHAQUE case à CHAQUE frame (≈ 25 format! dont un
                    // seul peut servir — audit 2026-08-07, P1bis). Le format!
                    // ne s'exécute plus qu'au survol.
                    let resp = resp.on_hover_ui(|ui| {
                        let gain = slot.per_tier * rarity.bonus_mul;
                        let delta = cfg.power_of(&rarity.id) as i32 - porte_power as i32;
                        let bulle = if !a_soi {
                            format!(
                                "{} — {}\npas encore trouvée\n{} +{:.0}% si tu l'obtiens",
                                slot.label,
                                rarity.label,
                                slot.stat_label,
                                gain * 100.0
                            )
                        } else if sur_soi {
                            format!(
                                "{} — {}\nPORTÉE · {} +{:.0}%\n\nDouble-clic pour la retirer ({:+} Puissance)",
                                slot.label,
                                rarity.label,
                                slot.stat_label,
                                gain * 100.0,
                                -(cfg.power_of(&rarity.id) as i32)
                            )
                        } else {
                            format!(
                                "{} — {}\n{} +{:.0}%  ({:+.0}% vs portée)\n{delta:+} Puissance\n\nDouble-clic pour l'équiper",
                                slot.label,
                                rarity.label,
                                slot.stat_label,
                                gain * 100.0,
                                (gain - porte_gain) * 100.0
                            )
                        };
                        ui.label(bulle);
                    });
                    if resp.clicked() {
                        *selection = Some((slot.id.clone(), rarity.id.clone()));
                    }
                    if resp.double_clicked() {
                        action = Some((
                            slot.id.clone(),
                            if sur_soi {
                                None
                            } else {
                                Some(rarity.id.clone())
                            },
                        ));
                    }
                    ui.add_space(4.0);
                }
            });
            ui.add_space(8.0);
        }
    });
    action
}
