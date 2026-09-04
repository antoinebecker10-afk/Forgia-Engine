//! Catégorie Physics — Rapier world state.
//!
//! Lit `forgia2_physics.json` (story-549 session 2026-05-28). Comble le blind
//! spot Rapier : rigid bodies par type, colliders, sensors, KCC count, joints.
//! Diagnostics bug class : player stuck KCC (540), raycast self-hit (545),
//! collisions désynchronisées.

use super::{fmt_opt, fmt_opt_f64, DebugCategory};
use crate::snapshot::SensorSnapshot;
use bevy_egui::egui;

pub struct PhysicsCategory;

impl DebugCategory for PhysicsCategory {
    fn draw(&self, ui: &mut egui::Ui, snap: &SensorSnapshot) {
        let p = &snap.physics;
        ui.label(format!("gravity.y: {} m/s²", fmt_opt_f64(p.gravity_y, 2)));
        ui.separator();
        ui.label(format!(
            "rigid_bodies: {} total",
            fmt_opt(p.rigid_bodies_total)
        ));
        ui.label(format!(
            "  dynamic:        {}",
            fmt_opt(p.rigid_bodies_dynamic)
        ));
        ui.label(format!(
            "  kinematic_pos:  {}",
            fmt_opt(p.rigid_bodies_kinematic_pos)
        ));
        ui.label(format!(
            "  kinematic_vel:  {}",
            fmt_opt(p.rigid_bodies_kinematic_vel)
        ));
        ui.label(format!(
            "  fixed:          {}",
            fmt_opt(p.rigid_bodies_fixed)
        ));
        ui.separator();
        ui.label(format!("colliders: {}", fmt_opt(p.colliders_total)));
        ui.label(format!(
            "  sensors (no collision): {}",
            fmt_opt(p.colliders_sensor)
        ));
        ui.separator();
        ui.label(format!("KCC (player controller): {}", fmt_opt(p.kcc_count)));
        ui.label(format!("joints: {}", fmt_opt(p.joints_count)));
    }
}
