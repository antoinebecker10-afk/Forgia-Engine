//! Floating HP bars + damage popups au-dessus des bots — style Fortnite.
//!
//! - Lit `CombatHitEvent` pour register show-timer (1.5s fade) et damage popups.
//! - Chaque frame : `Camera.world_to_viewport(parent_translation + Y up)` → screen pos.
//! - HP bar : rouge chunky avec outline, taille adaptative à la distance.
//! - Popups : montent + fadent, jaune cartoon + outline. Headshot = rouge plus gros.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::Health as CombatHealth;
use forgia_combat::prelude::CombatHitEvent;
use forgia_core::prelude::*;
use forgia_mode_fps_arena::TargetCube;
use std::collections::HashMap;

use crate::style::*;

const FLOATER_DURATION_SECS: f32 = 2.0;
const POPUP_DURATION_SECS: f32 = 1.2;
const POPUP_RISE_SPEED: f32 = 1.6; // m/s monte dans le monde

/// Per-bot fade timer pour la bar HP au-dessus.
#[derive(Resource, Default)]
pub struct HpFloaterTimers {
    /// Map entity → seconds restantes avant fade complet.
    pub map: HashMap<Entity, f32>,
}

/// Popup damage "−42" rising.
#[derive(Debug, Clone)]
pub struct DamagePopup {
    pub world_pos: Vec3,
    pub amount: f32,
    pub age_secs: f32,
    pub is_kill: bool,
}

#[derive(Resource, Default)]
pub struct DamagePopups {
    pub popups: Vec<DamagePopup>,
}

/// Lit CombatHitEvent : reset le timer pour l'entity touchée + spawn un popup.
pub(crate) fn collect_hit_events(
    mut hits: MessageReader<CombatHitEvent>,
    mut timers: ResMut<HpFloaterTimers>,
    mut popups: ResMut<DamagePopups>,
    q_target: Query<&Transform, With<TargetCube>>,
) {
    for hit in hits.read() {
        timers.map.insert(hit.target, FLOATER_DURATION_SECS);
        if let Ok(tf) = q_target.get(hit.target) {
            popups.popups.push(DamagePopup {
                world_pos: tf.translation + Vec3::Y * 1.7,
                amount: hit.damage,
                age_secs: 0.0,
                is_kill: hit.is_kill,
            });
        }
    }
}

/// Decrement timers + age popups.
pub(crate) fn tick_floaters(
    time: Res<Time>,
    mut timers: ResMut<HpFloaterTimers>,
    mut popups: ResMut<DamagePopups>,
) {
    let dt = time.delta_secs();
    timers.map.retain(|_, t| {
        *t -= dt;
        *t > 0.0
    });
    for p in &mut popups.popups {
        p.age_secs += dt;
        p.world_pos.y += POPUP_RISE_SPEED * dt;
    }
    popups.popups.retain(|p| p.age_secs < POPUP_DURATION_SECS);
}

/// Draw HP bars + popups. World→viewport via Camera3d.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_bot_hp_and_popups(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    timers: Res<HpFloaterTimers>,
    popups: Res<DamagePopups>,
    q_bots: Query<(&Transform, &CombatHealth), With<TargetCube>>,
    q_cam: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Fps {
        return;
    }
    let Ok((cam, cam_tf)) = q_cam.single() else { return };
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_bot_hp_floaters"),
    ));

    // HP bars
    for (entity, timer) in &timers.map {
        let Ok((tf, hp)) = q_bots.get(*entity) else { continue };
        let world_pos = tf.translation + Vec3::Y * 2.2;
        let Ok(screen_pos) = cam.world_to_viewport(cam_tf, world_pos) else { continue };

        // Distance-based scale (plus loin = plus petit, mais reste lisible)
        let dist = (cam_tf.translation() - world_pos).length();
        let scale = (10.0 / dist.max(2.0)).clamp(0.4, 1.6);

        let bar_w = 80.0 * scale;
        let bar_h = 9.0 * scale;
        let alpha_fade = (timer / FLOATER_DURATION_SECS).clamp(0.0, 1.0);
        let alpha_byte = (alpha_fade * 235.0) as u8;

        let bar_rect = egui::Rect::from_center_size(
            egui::pos2(screen_pos.x, screen_pos.y),
            egui::vec2(bar_w, bar_h),
        );

        // Fond noir translucide
        painter.rect_filled(
            bar_rect,
            2.0,
            egui::Color32::from_rgba_unmultiplied(15, 18, 24, (alpha_fade * 200.0) as u8),
        );

        // Fill rouge chunky
        let frac = hp.current / hp.max.max(0.01);
        let fill_w = bar_w * frac.clamp(0.0, 1.0);
        if fill_w > 0.5 {
            let fill_rect = egui::Rect::from_min_size(
                bar_rect.min,
                egui::vec2(fill_w, bar_h),
            );
            painter.rect_filled(
                fill_rect,
                2.0,
                egui::Color32::from_rgba_unmultiplied(
                    C_BOT_HP.r(),
                    C_BOT_HP.g(),
                    C_BOT_HP.b(),
                    alpha_byte,
                ),
            );
        }

        // Outline noir chunky
        painter.rect_stroke(
            bar_rect,
            2.0,
            egui::Stroke::new(
                (1.5 * scale).max(1.0),
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, alpha_byte),
            ),
            egui::StrokeKind::Outside,
        );
    }

    // Damage popups
    for popup in &popups.popups {
        let Ok(screen_pos) = cam.world_to_viewport(cam_tf, popup.world_pos) else { continue };
        let t = popup.age_secs / POPUP_DURATION_SECS;
        let alpha = ((1.0 - t).clamp(0.0, 1.0) * 255.0) as u8;
        let dist = (cam_tf.translation() - popup.world_pos).length();
        let scale = (12.0 / dist.max(2.0)).clamp(0.5, 2.2);

        let (text, color, font_size) = if popup.is_kill {
            (format!("KILL  -{:.0}", popup.amount), C_HEADSHOT_NUMBER, 26.0 * scale)
        } else if popup.amount > 50.0 {
            (format!("-{:.0}", popup.amount), C_HEADSHOT_NUMBER, 22.0 * scale)
        } else {
            (format!("-{:.0}", popup.amount), C_DAMAGE_NUMBER, 18.0 * scale)
        };

        let color_alpha = egui::Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            alpha,
        );

        text_with_outline(
            &painter,
            egui::pos2(screen_pos.x, screen_pos.y),
            egui::Align2::CENTER_CENTER,
            &text,
            egui::FontId::monospace(font_size),
            color_alpha,
            (1.5 * scale).max(1.0),
        );
    }
}

pub struct BotHpFloatersPlugin;

impl Plugin for BotHpFloatersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HpFloaterTimers>()
            .init_resource::<DamagePopups>()
            .add_systems(
                Update,
                (collect_hit_events, tick_floaters)
                    .chain()
                    .run_if(in_state(GameMode::Fps)),
            )
            .add_systems(EguiPrimaryContextPass, draw_bot_hp_and_popups);
    }
}
