//! castle_equipment_panel.rs — L'inventaire d'armure du Hall, touche **I**.
//!
//! Le Hall est le lieu où l'on se prépare : on y voit son personnage, on doit
//! donc pouvoir l'habiller sans repasser par le menu-titre. Le contenu est
//! exactement celui du menu (`draw_equipment_content`) — une seule mise en page
//! d'équipement dans tout le jeu, y compris l'échelle complète des raretés qui
//! montre ce qui reste à trouver pour les prochaines runs.
//!
//! 🚨 **Une touche, un seul gestionnaire.** `KeyI` n'est utilisée nulle part
//! ailleurs (vérifié) ; elle ne bascule que ce panneau, et seulement dans le
//! Hall. C'est l'anti-trap « 2 handlers ESC » du contrat.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_mode_roguelite::equipment::{
    draw_equipment_content, EquipmentConfig, EquipmentMods, EquipmentPanelShown, EquipmentSave,
};
use forgia_ui_lib::style::{HAIR_GOLD_STRONG, VERRE_GLASS};

/// Panneau ouvert ou non. Fermé à l'entrée comme à la sortie du Hall.
#[derive(Resource, Default)]
pub struct HallEquipmentOpen(pub bool);

fn sys_toggle_equipment_panel(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<HallEquipmentOpen>,
) {
    if keys.just_pressed(KeyCode::KeyI) {
        open.0 = !open.0;
    }
}

/// Rend la souris pendant que le panneau est ouvert, et la reprend en le
/// fermant. Sans ça on ne peut pas cliquer les pastilles : le Hall capture le
/// curseur pour la caméra.
fn sys_equipment_panel_cursor(
    open: Res<HallEquipmentOpen>,
    mut q: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !open.is_changed() {
        return;
    }
    let Ok(mut cursor) = q.single_mut() else {
        return;
    };
    if open.0 {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

fn draw_hall_equipment_panel(
    mut contexts: EguiContexts,
    open: Res<HallEquipmentOpen>,
    cfg: Res<EquipmentConfig>,
    mut save: ResMut<EquipmentSave>,
    mods: Res<EquipmentMods>,
    mut shown: ResMut<EquipmentPanelShown>,
) {
    if !open.0 || cfg.slots.is_empty() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    shown.0 = true;

    egui::Area::new(egui::Id::new("hall_equipment_panel"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(VERRE_GLASS)
                .inner_margin(egui::Margin::symmetric(26, 20))
                .corner_radius(egui::CornerRadius::same(12))
                .stroke(egui::Stroke::new(1.5, HAIR_GOLD_STRONG))
                .show(ui, |ui| {
                    ui.set_max_width(340.0);
                    draw_equipment_content(ui, &cfg, &mut save, &mods);
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Les contours sont les pièces à trouver.  [I] ferme")
                            .size(11.0)
                            .weak(),
                    );
                });
        });
}

/// Ferme le panneau en quittant le Hall — sinon il resterait ouvert au retour,
/// curseur libre, dans un mode qui ne le dessine pas.
fn sys_close_on_exit(
    mut open: ResMut<HallEquipmentOpen>,
    mut q: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !open.0 {
        return;
    }
    open.0 = false;
    if let Ok(mut cursor) = q.single_mut() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

pub struct CastleEquipmentPanelPlugin;

impl Plugin for CastleEquipmentPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HallEquipmentOpen>()
            .add_systems(
                Update,
                (sys_toggle_equipment_panel, sys_equipment_panel_cursor)
                    .chain()
                    .in_set(GameSet::UI)
                    .run_if(in_state(GameMode::CastleHub))
                    .run_if(in_state(AppMode::InGame)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                draw_hall_equipment_panel.run_if(in_state(GameMode::CastleHub)),
            )
            .add_systems(OnExit(GameMode::CastleHub), sys_close_on_exit);
    }
}
