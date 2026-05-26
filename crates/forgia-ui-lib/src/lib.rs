//! forgia-ui-lib — Consolidated UI library (story-517 fusion).
//!
//! 6 modules ex-crates :
//! - `style` (palette + egui helpers)
//! - `hud` (HP/wave counter)
//! - `hud_ammo` (ammo counter + slot strip + reload arc)
//! - `pause_menu` (Resume/Settings/Quit + sliders)
//! - `damage_direction` (CoD-style arcs)
//! - `dialogue` (modal dialogue UI)

pub mod style;
pub mod hud;
pub mod hud_ammo;
pub mod pause_menu;
pub mod damage_direction;
pub mod dialogue;

use bevy::prelude::*;

/// Meta-plugin bundling all 6 UI sub-systems.
/// Use this if you want every UI feature. Otherwise add sub-plugins individually.
pub struct ForgiaUiLibPlugin;

impl Plugin for ForgiaUiLibPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            hud::prelude::ForgiaUiHudPlugin,
            hud_ammo::prelude::ForgiaUiHudAmmoPlugin,
            pause_menu::prelude::ForgiaUiPauseMenuPlugin,
            damage_direction::prelude::ForgiaUiDamageDirectionPlugin,
            dialogue::ForgiaUiDialoguePlugin,
        ));
    }
}
