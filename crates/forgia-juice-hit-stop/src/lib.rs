//! # forgia-juice-hit-stop
//!
//! Hit-stop time pause — sur hit confirmé, ralentit `Time<Virtual>` brièvement
//! puis restaure. Pattern Apex / Doom Eternal pour la sensation d'impact.
//!
//! Source de vérité Forgia V2 — extrait depuis `forgia-combat::combat_juice`
//! (Tier 1D refacto fine-grained-crates, 2026-05-17).

use bevy::prelude::*;

pub mod prelude {
    pub use crate::{ForgiaJuiceHitStopPlugin, HitStopState};
}

/// Resource éphémère : insérée par un caller (fire system) pour déclencher
/// un hit-stop. Le system `hitstop_tick_system` la consomme et restaure la
/// vitesse `Time<Virtual>` à `restore_speed` quand `timer.is_finished()`.
#[derive(Resource)]
pub struct HitStopState {
    pub timer: Timer,
    pub restore_speed: f32,
}

pub struct ForgiaJuiceHitStopPlugin;

impl Plugin for ForgiaJuiceHitStopPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            hitstop_tick_system.run_if(resource_exists::<HitStopState>),
        );
    }
}

/// Tick le timer hit-stop. Restaure `Time<Virtual>` à `restore_speed` puis
/// retire la Resource quand fini.
pub fn hitstop_tick_system(
    mut commands: Commands,
    real_time: Res<Time<Real>>,
    mut time: ResMut<Time<Virtual>>,
    mut state: ResMut<HitStopState>,
) {
    state.timer.tick(real_time.delta());
    if state.timer.is_finished() {
        time.set_relative_speed(state.restore_speed);
        commands.remove_resource::<HitStopState>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaJuiceHitStopPlugin;
    }

    #[test]
    fn state_constructible() {
        let _s = HitStopState {
            timer: Timer::from_seconds(0.05, TimerMode::Once),
            restore_speed: 1.0,
        };
    }
}
