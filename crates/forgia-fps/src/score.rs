//! Arena score system : kill detection, popup "+points" world-space, scoreboard TAB.
//!
//! Architecture :
//! - `ArenaScore` Resource : cumul kills/deaths/score session
//! - `KillPopup` Component : Text2d billboard fade vers le haut (0.8s)
//! - `ScoreboardVisible` Resource : toggle UI TAB
//!
//! Pipeline kill : `CombatHitEvent.is_kill = true` (émis par fire_weapon_minimal)
//! → `record_kill_on_hit` incrémente score + spawn popup au hit position.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_combat::prelude::CombatHitEvent;
use forgia_core::prelude::*;

const KILL_POINTS: u32 = 100;
const POPUP_LIFETIME_SECS: f32 = 0.8;
const POPUP_RISE_SPEED: f32 = 1.5; // m/s vers le haut
const POPUP_COLOR: Color = Color::srgb(1.0, 0.85, 0.2); // jaune-or style CoD

#[derive(Resource, Default)]
pub struct ArenaScore {
    pub kills: u32,
    pub deaths: u32,
    pub score: u32,
}

#[derive(Resource, Default)]
pub struct ScoreboardVisible(pub bool);

#[derive(Component)]
pub struct KillPopup {
    pub timer: Timer,
}

/// System Update : lit CombatHitEvent et incrémente ArenaScore + spawn popup si is_kill.
pub fn record_kill_on_hit(
    mut commands: Commands,
    mut hit_events: MessageReader<CombatHitEvent>,
    mut score: ResMut<ArenaScore>,
    q_target: Query<&GlobalTransform>,
) {
    for ev in hit_events.read() {
        if !ev.is_kill {
            continue;
        }
        score.kills += 1;
        score.score += KILL_POINTS;

        // Spawn popup au-dessus de la cible
        let pos = q_target
            .get(ev.target)
            .map(|t| t.translation())
            .unwrap_or(Vec3::ZERO)
            + Vec3::new(0.0, 0.8, 0.0);
        commands.spawn((
            KillPopup {
                timer: Timer::from_seconds(POPUP_LIFETIME_SECS, TimerMode::Once),
            },
            Text2d::new(format!("+{KILL_POINTS}")),
            TextFont {
                font_size: 36.0,
                ..default()
            },
            TextColor(POPUP_COLOR),
            Transform::from_translation(pos),
            Name::new("KillPopup"),
        ));
        info!(
            "[score] KILL → +{} points (total: {} kills, {} score)",
            KILL_POINTS, score.kills, score.score
        );
    }
}

/// System Update : fait monter les popups + fade alpha + despawn à expiration.
pub fn tick_kill_popups(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut KillPopup, &mut Transform, &mut TextColor)>,
) {
    let dt = time.delta_secs();
    for (entity, mut popup, mut tf, mut color) in &mut q {
        popup.timer.tick(time.delta());
        if popup.timer.is_finished() {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
            continue;
        }
        // Monte vers le haut
        tf.translation.y += POPUP_RISE_SPEED * dt;
        // Fade alpha en fin de vie
        let frac = popup.timer.fraction();
        let alpha = (1.0 - frac).clamp(0.0, 1.0);
        let mut c = POPUP_COLOR.to_srgba();
        c.alpha = alpha;
        color.0 = c.into();
    }
}

/// System Update : TAB toggle scoreboard visibility.
pub fn scoreboard_toggle(keys: Res<ButtonInput<KeyCode>>, mut visible: ResMut<ScoreboardVisible>) {
    if keys.just_pressed(KeyCode::Tab) {
        visible.0 = !visible.0;
    }
}

/// System Update : egui Window affichage scoreboard (compact, KDA + score).
pub fn scoreboard_ui(
    mut contexts: EguiContexts,
    visible: Res<ScoreboardVisible>,
    score: Res<ArenaScore>,
) {
    if !visible.0 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    egui::Window::new("Score")
        .anchor(egui::Align2::CENTER_TOP, [0.0, 40.0])
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Arena Score");
            ui.separator();
            egui::Grid::new("score_grid")
                .num_columns(2)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Kills");
                    ui.label(format!("{}", score.kills));
                    ui.end_row();
                    ui.label("Deaths");
                    ui.label(format!("{}", score.deaths));
                    ui.end_row();
                    ui.label("Score");
                    ui.label(format!("{}", score.score));
                    ui.end_row();
                });
            ui.separator();
            ui.small("TAB pour fermer");
        });
}

/// Plugin assembling all score systems.
pub struct ArenaScorePlugin;

impl Plugin for ArenaScorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ArenaScore>()
            .init_resource::<ScoreboardVisible>()
            .add_systems(
                Update,
                (
                    record_kill_on_hit,
                    tick_kill_popups,
                    scoreboard_toggle,
                    scoreboard_ui,
                )
                    .run_if(in_state(GameMode::Fps)),
            );
    }
}
