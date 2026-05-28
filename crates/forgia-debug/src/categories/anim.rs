//! Category 5 — Anim : rex bones, foot IK, walk pose, anim layer.

use super::{fmt_opt, fmt_opt_f64, fmt_opt_str, DebugCategory};
use crate::snapshot::SensorSnapshot;
use bevy_egui::egui;

pub struct AnimCategory;

impl DebugCategory for AnimCategory {
    fn draw(&self, ui: &mut egui::Ui, snap: &SensorSnapshot) {
        let a = &snap.anim;

        ui.label("— foot IK —");
        ui.label(format!(
            "bones_missing:  {}",
            a.foot_ik_bones_missing.map_or("n/a".into(), |b| b.to_string())
        ));
        ui.label(format!(
            "active:         {}",
            a.foot_ik_active.map_or("n/a".into(), |b| b.to_string())
        ));

        ui.separator();
        ui.label("— walk pose —");
        ui.label(format!("phase:          {}", fmt_opt_f64(a.walk_phase, 2)));
        ui.label(format!(
            "speed_mps:      {}",
            fmt_opt_f64(a.walk_speed_mps, 2)
        ));

        ui.separator();
        ui.label("— anim layer —");
        ui.label(format!(
            "active_layer:   {}",
            fmt_opt_str(a.anim_layer_active.as_deref())
        ));
        ui.label(format!(
            "blend:          {}",
            fmt_opt_f64(a.anim_layer_blend, 2)
        ));

        ui.separator();
        ui.label("— rex bones —");
        ui.label(format!("present:        {}", fmt_opt(a.rex_bones_present)));
    }
}
