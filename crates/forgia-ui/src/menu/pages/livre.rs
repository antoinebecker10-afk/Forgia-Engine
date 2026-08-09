//! Page LIVRE — la vue d'ensemble des dix chapitres (chapter_select_content).

use bevy::prelude::*;
use bevy_egui::EguiContexts;
use forgia_core::prelude::*;
use forgia_mode_roguelite::chapters::chapter_select_content;
use forgia_mode_roguelite::decor_palettes::DecorPalettesConfig;
use forgia_mode_roguelite::meta_shop::{
    MetaShopSave,
    SelectedChapter,
};
use forgia_ui_lib::style::glass_btn;

use crate::menu::chrome::hub_section_panel;
use crate::menu::nav::MenuPage;

/// Le LIVRE en page pleine — la même `chapter_select_content` que le carrousel
/// de l'accueil, pour la vue d'ensemble des dix chapitres.
pub(crate) fn sys_menu_livre(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut nav: ResMut<crate::NavStack>,
    save: Option<Res<MetaShopSave>>,
    palettes: Option<Res<DecorPalettesConfig>>,
    selected: Option<ResMut<SelectedChapter>>,
) {
    if *app_state.get() != AppMode::Menu || nav.current() != MenuPage::Livre {
        return;
    }
    let (Some(save), Some(mut selected)) = (save, selected) else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let mut retour = false;
    hub_section_panel(
        ctx,
        "hub_sec_livre",
        MenuPage::Livre.section_title(),
        640.0,
        |ui| {
            chapter_select_content(ui, &save, palettes.as_deref(), &mut selected);
            ui.add_space(12.0);
            if glass_btn(ui, "‹  Retour").clicked() {
                retour = true;
            }
        },
    );
    if retour {
        // Dérivé, plus recopié : la seule entrée du Livre est l'Accueil
        // (titre cliquable) — pop y ramène.
        nav.back();
    }
}
