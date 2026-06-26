//! Wave counter — top-center écran, font monospace chunky = effet arcade rétro.
//!
//! Affiche : `WAVE 3 / 5` + nombre enemies restants + countdown break entre vagues.
//! Color flash quand wave change (lerp 1s).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_mode_fps_arena::{WavePhase, WaveStartedEvent, WaveState};

use crate::style::*;

#[derive(Resource, Default)]
pub struct WaveCounterAnim {
    /// Secondes restantes de flash après wave change (0 = idle).
    pub flash_secs_left: f32,
}

pub(crate) fn tick_wave_anim(
    time: Res<Time>,
    mut anim: ResMut<WaveCounterAnim>,
    mut started: MessageReader<WaveStartedEvent>,
) {
    if started.read().next().is_some() {
        anim.flash_secs_left = 1.2;
    }
    anim.flash_secs_left = (anim.flash_secs_left - time.delta_secs()).max(0.0);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_wave_counter(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    wave: Option<Res<WaveState>>,
    anim: Res<WaveCounterAnim>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Fps {
        return;
    }
    let Some(wave) = wave else { return };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_wave_counter"),
    ));

    // Panel position : haut centre.
    let panel_w = 320.0;
    let panel_h = 78.0;
    let center_x = screen.center().x;
    let top_y = screen.min.y + 18.0;
    let panel_rect = egui::Rect::from_min_size(
        egui::pos2(center_x - panel_w * 0.5, top_y),
        egui::vec2(panel_w, panel_h),
    );

    // Flash color cycle au moment du wave change.
    let flash_t = (anim.flash_secs_left / 1.2).clamp(0.0, 1.0);
    let panel_fill = if flash_t > 0.0 {
        // Lerp orange→bg
        egui::Color32::from_rgba_unmultiplied(
            (15.0 + (255.0 - 15.0) * flash_t * 0.55) as u8,
            (18.0 + (122.0 - 18.0) * flash_t * 0.55) as u8,
            (24.0 + (26.0 - 24.0) * flash_t * 0.55) as u8,
            (200.0 + 30.0 * flash_t) as u8,
        )
    } else {
        C_BG_DARK
    };
    chunky_rect_filled(&painter, panel_rect, panel_fill, 3.0, 10.0);

    // Texte principal "WAVE 3 / 5"
    let main_text = match wave.phase {
        WavePhase::PreBoot => "WAVE — / —".to_string(),
        WavePhase::AllComplete => "VICTORY".to_string(),
        _ => format!("WAVE {} / {}", wave.current_wave, wave.final_wave_index),
    };
    let main_color = match wave.phase {
        WavePhase::AllComplete => C_HP_HIGH,
        _ if flash_t > 0.0 => C_PRIMARY,
        _ => C_TEXT_LIGHT,
    };
    text_with_outline(
        &painter,
        egui::pos2(center_x, top_y + 22.0),
        egui::Align2::CENTER_CENTER,
        &main_text,
        egui::FontId::monospace(26.0),
        main_color,
        2.0,
    );

    // Subtext : enemies remaining ou break countdown
    let sub_text = match wave.phase {
        WavePhase::PreBoot => "Get ready...".to_string(),
        WavePhase::Active => {
            format!("ENEMIES  {} / {}", wave.bots_alive, wave.bots_total_wave)
        }
        WavePhase::Break => {
            let secs = wave.break_timer.remaining_secs().max(0.0);
            format!("NEXT IN  {:.1}s", secs)
        }
        WavePhase::AllComplete => "Boss defeated.".to_string(),
    };
    let sub_color = match wave.phase {
        WavePhase::Break => C_ACCENT,
        WavePhase::AllComplete => C_HP_HIGH,
        _ => C_TEXT_MUTED,
    };
    text_with_outline(
        &painter,
        egui::pos2(center_x, top_y + 54.0),
        egui::Align2::CENTER_CENTER,
        &sub_text,
        egui::FontId::monospace(16.0),
        sub_color,
        1.0,
    );
}

pub struct WaveCounterPlugin;

impl Plugin for WaveCounterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaveCounterAnim>()
            .add_systems(Update, tick_wave_anim.run_if(in_state(GameMode::Fps)))
            .add_systems(
                EguiPrimaryContextPass,
                draw_wave_counter.run_if(gameplay_hud_visible),
            );
    }
}
