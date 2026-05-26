//! # forgia-hitmarker
//!
//! Hit confirmation visual (4 segments diagonaux blancs fade 220ms autour crosshair)
//! déclenché par `CombatHitEvent`.
//!
//! Extrait de `forgia-ui` 2026-05-16 (règle `fine-grained-crates.md`).
//!
//! Pattern V1 : hud_hitmarker_duration_ms = 220ms.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::prelude::*;
use forgia_core::prelude::*;

pub mod prelude {
    pub use super::{ForgiaHitmarkerPlugin, HitmarkerState};
}

const HITMARKER_DURATION: f32 = 0.22;

/// HitmarkerState — Resource pour fade visual après hit confirmé.
#[derive(Resource, Default)]
pub struct HitmarkerState {
    /// Time remaining (s). >0 = visible, fade linéaire.
    pub time_left: f32,
}

pub struct ForgiaHitmarkerPlugin;

impl Plugin for ForgiaHitmarkerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HitmarkerState>()
            .add_systems(EguiPrimaryContextPass, draw_hitmarker)
            .add_systems(Update, hitmarker_trigger.in_set(GameSet::UI));
    }
}

/// Lit `CombatHitEvent` → reset `HitmarkerState.time_left` à HITMARKER_DURATION.
fn hitmarker_trigger(
    mut hits: MessageReader<CombatHitEvent>,
    mut state: ResMut<HitmarkerState>,
    time: Res<Time>,
) {
    if !hits.is_empty() {
        state.time_left = HITMARKER_DURATION;
        for _ in hits.read() {}
    } else if state.time_left > 0.0 {
        state.time_left = (state.time_left - time.delta_secs()).max(0.0);
    }
}

/// Dessine 4 segments diagonaux blancs autour du crosshair quand HitmarkerState actif.
fn draw_hitmarker(
    mut contexts: EguiContexts,
    state: Res<HitmarkerState>,
    app_state: Res<State<AppMode>>,
) {
    if *app_state.get() != AppMode::InGame || state.time_left <= 0.0 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let center = ctx.content_rect().center();
    let alpha_pct = state.time_left / HITMARKER_DURATION;
    let alpha = (alpha_pct * 220.0) as u8;
    let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("forgia_hitmarker"),
    ));
    let inner = 7.0;
    let outer = 14.0;
    let stroke = egui::Stroke::new(2.5, color);
    for &(dx, dy) in &[(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        painter.line_segment(
            [
                egui::pos2(center.x + dx * inner, center.y + dy * inner),
                egui::pos2(center.x + dx * outer, center.y + dy * outer),
            ],
            stroke,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaHitmarkerPlugin;
    }

    #[test]
    fn hitmarker_default_invisible() {
        let s = HitmarkerState::default();
        assert_eq!(s.time_left, 0.0);
    }
}
