//! Category 3 — Player : state, lifecycle, HP.
//!
//! Bug canonique : story-540 (player stuck KCC) — position/velocity figées,
//! grounded false continu.

use super::{fmt_opt, fmt_opt_f64, fmt_opt_str, fmt_opt_vec3, DebugCategory};
use crate::snapshot::SensorSnapshot;
use bevy_egui::egui;

pub struct PlayerCategory;

impl DebugCategory for PlayerCategory {
    fn draw(&self, ui: &mut egui::Ui, snap: &SensorSnapshot) {
        let p = &snap.player;

        ui.label(format!(
            "app_mode:      {}",
            fmt_opt_str(p.app_mode.as_deref())
        ));
        ui.label(format!(
            "game_mode:     {}",
            fmt_opt_str(p.game_mode.as_deref())
        ));
        ui.separator();
        ui.label("— transform —");
        ui.label(format!("position:      {}", fmt_opt_vec3(p.position)));
        ui.label(format!("velocity:      {}", fmt_opt_vec3(p.velocity)));
        ui.label(format!(
            "grounded:      {}",
            p.grounded.map_or("n/a".into(), |b| b.to_string())
        ));
        ui.separator();
        ui.label("— HP —");
        ui.label(format!(
            "hp:            {} / {}",
            fmt_opt_f64(p.last_hp_current, 0),
            fmt_opt_f64(p.last_hp_max, 0)
        ));
        ui.separator();
        ui.label("— lifecycle (last sec) —");
        ui.label(format!(
            "players + / − : {} / {}",
            fmt_opt(p.players_added),
            fmt_opt(p.players_removed)
        ));
        ui.label(format!(
            "bots    + / − : {} / {}",
            fmt_opt(p.bots_added),
            fmt_opt(p.bots_removed)
        ));
    }
}
