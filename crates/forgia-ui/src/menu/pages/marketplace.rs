//! Page MARKETPLACE — décors, couleurs, bras, musique, payés en Éclats.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_core::prelude::*;
use forgia_mode_roguelite::identity::IdentitySave;
use forgia_mode_roguelite::meta_shop::MetaShopSave;
use forgia_ui_lib::style::{
    glass_btn, C_PRIMARY, C_TEXT_MUTED, FORGE_CREME,
    FORGE_ECLAT, FORGE_OR, FORGE_PANEL,
};

use crate::currency_icons::{CurrencyIcons, CURRENCY_ICON};
use crate::menu::chrome::hub_section_panel;
use crate::menu::nav::MenuPage;

/// Colonnes du Marketplace — fixes, pour que les lignes s'alignent.
const DECOR_NAME_W: f32 = 290.0;
const DECOR_ACTION_W: f32 = 210.0;
const DECOR_ROW_H: f32 = 28.0;

/// Le MARKETPLACE — tout ce qui se porte et ne se joue pas (story-678).
///
/// Quatre familles sous un même toit : décors du menu, couleur du forgeron,
/// bras (ceux qu'on voit à chaque tir), musique du hub. Un onglet par famille,
/// une seule règle de possession (`cosmetics::OwnedCosmetics`).
///
/// ## Pourquoi ce n'est pas l'Enclume
///
/// L'Enclume vend de la PUISSANCE, en Âmes. Mêler du cosmétique à ses rangs
/// forcerait un arbitrage « je tape plus fort » contre « c'est plus joli » dans
/// la même liste — le pire endroit pour ce choix. Le Marketplace a donc sa
/// propre monnaie, les **Éclats**, gagnés à la profondeur atteinte.
///
/// ## Ce qui est possédé
///
/// Chaque famille est possédée dans le stock qui lui sert déjà ailleurs (les
/// couleurs dans `unlocked_colors`, que le panneau Forgeron filtre et que le
/// boot fait respecter). Le catalogue le sait ; cette page ne fait que
/// l'afficher.
pub(crate) fn sys_menu_marketplace(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut page: ResMut<MenuPage>,
    mut tab: Local<usize>,
    catalogue: Option<Res<forgia_mode_roguelite::cosmetics::CosmeticsConfig>>,
    mut identity: Option<ResMut<IdentitySave>>,
    mut meta_save: Option<ResMut<MetaShopSave>>,
    icons: Option<Res<CurrencyIcons>>,
) {
    use forgia_mode_roguelite::cosmetics::{self, CosmeticKind, CosmeticSource, OwnedCosmetics};

    if *app_state.get() != AppMode::Menu || *page != MenuPage::Marketplace {
        return;
    }
    let (Some(catalogue), Some(identity), Some(save)) =
        (catalogue.as_deref(), identity.as_mut(), meta_save.as_mut())
    else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let famille = CosmeticKind::ALL[(*tab).min(CosmeticKind::ALL.len() - 1)];
    let eclats = save.shards_total;
    let icone_eclat = icons.as_deref().and_then(|i| i.shards);

    // Instantané de la famille courante : (id, libellé, possédé, porté, prix,
    // chapitre requis). Pris AVANT de dessiner pour libérer l'emprunt sur
    // `identity`, que les actions muteront après la closure.
    let lignes: Vec<(String, String, bool, bool, Option<u32>, u32)> = {
        let owned = OwnedCosmetics {
            chapters_cleared: save.chapters_cleared,
            identity,
        };
        catalogue
            .of_kind(famille, &owned)
            .into_iter()
            .map(|(c, possede)| {
                let chapitre = match c.source {
                    CosmeticSource::Chapter(n) => n,
                    _ => 0,
                };
                (
                    c.id.clone(),
                    c.label.clone(),
                    possede,
                    cosmetics::is_equipped(identity, c),
                    c.source.price(),
                    chapitre,
                )
            })
            .collect()
    };
    let possedes = lignes.iter().filter(|l| l.2).count();

    let mut equiper: Option<String> = None;
    let mut acheter: Option<(String, u32)> = None;
    let mut retour = false;
    let mut nouvel_onglet: Option<usize> = None;

    hub_section_panel(
        ctx,
        "hub_sec_marketplace",
        MenuPage::Marketplace.section_title(),
        820.0,
        |ui| {
            // Bourse — dite en tête, parce que c'est ce qui décide de ce qu'on
            // peut faire sur cette page.
            ui.horizontal(|ui| {
                CurrencyIcons::show(ui, icone_eclat, CURRENCY_ICON);
                ui.label(
                    egui::RichText::new(format!("{eclats}  Éclats"))
                        .size(22.0)
                        .color(FORGE_ECLAT)
                        .strong(),
                );
            });
            ui.label(
                egui::RichText::new("Gagnés en descendant loin dans un chapitre.")
                    .size(13.0)
                    .color(C_TEXT_MUTED),
            );
            ui.add_space(12.0);

            // Onglets de famille.
            ui.horizontal(|ui| {
                for (i, k) in CosmeticKind::ALL.iter().enumerate() {
                    let actif = *k == famille;
                    let resp = ui.add_sized(
                        egui::vec2(150.0, 30.0),
                        egui::Button::selectable(
                            actif,
                            egui::RichText::new(k.tab_label())
                                .size(15.0)
                                .color(if actif { C_PRIMARY } else { FORGE_CREME })
                                .strong(),
                        ),
                    );
                    forgia_ui_lib::ui_sfx::instrument_hover(&resp);
                    if resp.clicked() && !actif {
                        nouvel_onglet = Some(i);
                        forgia_ui_lib::ui_sfx::push_ui_sfx(
                            &resp.ctx,
                            forgia_ui_lib::ui_sfx::UiSfxKind::Tab,
                        );
                    }
                }
            });
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(famille.tab_help())
                    .size(14.0)
                    .color(FORGE_CREME),
            );
            ui.label(
                egui::RichText::new(format!("{possedes} / {} débloqués", lignes.len()))
                    .size(13.0)
                    .color(C_TEXT_MUTED),
            );
            ui.add_space(12.0);

            for (id, label, possede, porte, prix, chapitre) in &lignes {
                ui.horizontal(|ui| {
                    // Pastille d'état : porté (or plein), possédé (or creux),
                    // verrouillé (gris). Se lit sans lire le texte.
                    let (dot, _) =
                        ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                    let centre = dot.center();
                    if *porte {
                        ui.painter().circle_filled(centre, 6.0, FORGE_OR);
                    } else if *possede {
                        ui.painter()
                            .circle_stroke(centre, 6.0, egui::Stroke::new(2.0, FORGE_OR));
                    } else {
                        ui.painter()
                            .circle_filled(centre, 5.0, egui::Color32::from_gray(80));
                    }

                    ui.add_sized(
                        egui::vec2(DECOR_NAME_W, DECOR_ROW_H),
                        egui::Label::new(
                            egui::RichText::new(label)
                                .size(16.0)
                                .color(if *possede { FORGE_CREME } else { C_TEXT_MUTED }),
                        )
                        .halign(egui::Align::LEFT),
                    );

                    if *porte {
                        ui.add_sized(
                            egui::vec2(DECOR_ACTION_W, DECOR_ROW_H),
                            egui::Label::new(
                                egui::RichText::new("PORTÉ")
                                    .size(13.0)
                                    .color(FORGE_OR)
                                    .strong(),
                            ),
                        );
                    } else if *possede {
                        let resp = ui.add_sized(
                            egui::vec2(DECOR_ACTION_W, DECOR_ROW_H),
                            egui::Button::new(
                                egui::RichText::new("Porter")
                                    .size(14.0)
                                    .color(FORGE_OR)
                                    .strong(),
                            )
                            .fill(FORGE_PANEL),
                        );
                        forgia_ui_lib::ui_sfx::instrument_hover(&resp);
                        if resp.clicked() {
                            equiper = Some(id.clone());
                        }
                    } else if let Some(p) = prix {
                        // Verrouillé mais achetable. Le bouton est DÉSACTIVÉ,
                        // pas caché, quand la bourse ne suit pas — « trop cher »
                        // et « inerte » ne doivent jamais se ressembler.
                        let assez = eclats >= *p;
                        let resp = ui
                            .add_enabled_ui(assez, |ui| {
                                ui.add_sized(
                                    egui::vec2(DECOR_ACTION_W, DECOR_ROW_H),
                                    egui::Button::new(
                                        egui::RichText::new(format!("Débloquer — {p}"))
                                            .size(14.0)
                                            .color(if assez { FORGE_ECLAT } else { C_TEXT_MUTED }),
                                    )
                                    .fill(FORGE_PANEL),
                                )
                            })
                            .inner;
                        forgia_ui_lib::ui_sfx::instrument_hover(&resp);
                        if resp.clicked() {
                            acheter = Some((id.clone(), *p));
                        }
                        if !assez {
                            ui.label(
                                egui::RichText::new(format!("il te manque {}", p - eclats))
                                    .size(12.0)
                                    .color(C_TEXT_MUTED),
                            );
                        }
                    } else {
                        let texte = if *chapitre > 0 {
                            format!("Bats le chapitre {chapitre}")
                        } else {
                            "Haut fait".to_string()
                        };
                        ui.add_sized(
                            egui::vec2(DECOR_ACTION_W, DECOR_ROW_H),
                            egui::Label::new(
                                egui::RichText::new(texte).size(13.0).color(C_TEXT_MUTED),
                            ),
                        );
                    }
                });
                ui.add_space(5.0);
            }

            ui.add_space(10.0);
            if glass_btn(ui, "‹  Retour").clicked() {
                retour = true;
            }
        },
    );

    if let Some(i) = nouvel_onglet {
        *tab = i;
    }
    if let Some(id) = equiper {
        if let Some(c) = catalogue.get(&id) {
            cosmetics::equip(identity, c);
        }
    }
    if let Some((id, prix)) = acheter {
        // Ordre voulu : possession → débit → déblocage. Débloquer avant de
        // débiter obligerait à défaire un déblocage déjà écrit sur le disque si
        // la bourse ne suivait pas.
        //
        // Les Éclats n'ont PAS de miroir vif (contrairement aux Âmes) : une
        // seule vérité, `shards_total`. C'est le miroir des Âmes qui avait
        // produit le défaut « achat sans effet ».
        let paye = match catalogue.get(&id) {
            Some(c)
                if save.shards_total >= prix && cosmetics::grant_and_equip(identity, c) =>
            {
                save.shards_total -= prix;
                save.save();
                true
            }
            _ => false,
        };
        let son = if paye {
            forgia_ui_lib::ui_sfx::UiSfxKind::Unlock
        } else {
            forgia_ui_lib::ui_sfx::UiSfxKind::Denied
        };
        forgia_ui_lib::ui_sfx::push_ui_sfx(ctx, son);
    }
    if retour {
        *page = MenuPage::Forgeron;
    }
}
