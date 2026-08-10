//! Page ARMES — carte d'arme + aperçu RTT 3D live (sys_menu_armes).

use bevy::prelude::*;
use bevy_egui::EguiContexts;
use forgia_core::prelude::*;
use forgia_mode_roguelite::elements::ElementConfig;
use forgia_mode_roguelite::meta_shop::{
    MetaShopCatalogue, MetaShopSave,
};
use forgia_mode_roguelite::run::MetaSouls;
use forgia_mode_roguelite::weapon_select::{
    draw_weapon_menu_panel, StartingWeaponChoice, WeaponCards,
};

use crate::chrome::hub_section_panel;
use crate::nav::MenuPage;
use crate::weapon_preview::WeaponPreviewRtt;

/// Section Armes au menu-titre — carte d'arme (stats / élément / matchup +
/// sélecteur ‹ › + déblocage) avec **aperçu 3D live** : l'image RTT de
/// `weapon_preview` (l'arme tourne). Réutilise `draw_weapon_menu_panel` de
/// forgia-mode-roguelite. Système séparé (params + `WeaponPreviewRtt`).
pub(crate) fn sys_menu_armes(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    nav: Res<crate::NavStack>,
    mut choice: ResMut<StartingWeaponChoice>,
    cards: Res<WeaponCards>,
    elem_cfg: Res<ElementConfig>,
    mut save: ResMut<MetaShopSave>,
    cat: Res<MetaShopCatalogue>,
    mut meta: ResMut<MetaSouls>,
    rtt: Option<Res<WeaponPreviewRtt>>,
) {
    if *app_state.get() != AppMode::Menu || nav.current() != MenuPage::Armes {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    // Image RTT de l'aperçu 3D (None si le plugin n'a pas encore spawné la caméra).
    let weapon_image = rtt.as_ref().map(|r| r.tex_id);
    // Story-678 — chrome commun (titre/marges/transition standard).
    hub_section_panel(
        ctx,
        "hub_sec_armes",
        MenuPage::Armes.section_title(),
        500.0,
        |ui| {
            draw_weapon_menu_panel(
                ui,
                &mut choice,
                &cards,
                &elem_cfg,
                &mut save,
                &cat,
                &mut meta,
                weapon_image,
                220.0,
            );
        },
    );
}

