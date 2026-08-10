//! Page ENCLUME — les cartes d'upgrade méta, achat via apply_meta_purchase.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_core::prelude::*;
use forgia_mode_roguelite::meta_shop::{
    apply_meta_purchase, draw_enclume_panel, MetaShopCatalogue, MetaShopSave,
};
use forgia_mode_roguelite::run::MetaSouls;
use forgia_ui_lib::style::FORGE_CREME;

use crate::chrome::hub_section_panel;
use crate::nav::MenuPage;

/// L'ENCLUME au menu — les cartes cliquables de `draw_enclume_panel`, l'achat
/// appliqué par `apply_meta_purchase` (une seule règle de dépense).
pub(crate) fn sys_menu_enclume(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    nav: Res<crate::NavStack>,
    cat: Option<Res<MetaShopCatalogue>>,
    save: Option<ResMut<MetaShopSave>>,
    meta: Option<ResMut<MetaSouls>>,
) {
    if *app_state.get() != AppMode::Menu || nav.current() != MenuPage::Enclume {
        return;
    }
    let (Some(cat), Some(mut save), Some(mut meta)) = (cat, save, meta) else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let souls = meta.current;
    let mut purchase = None;
    hub_section_panel(
        ctx,
        "hub_sec_enclume",
        MenuPage::Enclume.section_title(),
        640.0,
        |ui| {
            ui.label(
                egui::RichText::new("Dépense tes Âmes en améliorations permanentes.")
                    .size(16.0)
                    .color(FORGE_CREME),
            );
            ui.add_space(10.0);
            purchase = draw_enclume_panel(ui, &cat, &save, souls);
        },
    );
    if let Some(p) = purchase {
        let ok = apply_meta_purchase(&cat, &mut save, &mut meta, p);
        forgia_ui_lib::ui_sfx::push_ui_sfx(
            ctx,
            if ok {
                forgia_ui_lib::ui_sfx::UiSfxKind::Buy
            } else {
                forgia_ui_lib::ui_sfx::UiSfxKind::Denied
            },
        );
    }
}
