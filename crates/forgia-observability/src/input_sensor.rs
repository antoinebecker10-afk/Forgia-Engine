//! input_sensor.rs — Producteur `forgia2_input.json` (1Hz, cross-mode).
//!
//! Accumule `KeyboardInput` messages + `ActionState<PlayerAction>::get_just_pressed()`
//! chaque frame (sinon Bevy purge les messages après 2 frames), flush JSON 1Hz.
//!
//! Bevy 0.18 : `EventReader` → `MessageReader` (KeyboardInput est `#[derive(Message)]`).
//!
//! Story-469 — Vague 5 Phase 5b Session C.

use bevy::ecs::message::MessageReader;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use forgia_input::PlayerAction;
use leafwing_input_manager::action_state::ActionState;

#[derive(Default)]
pub struct InputSensorAccum {
    pub keys_pressed: u32,
    pub keys_released: u32,
    pub actions_just_pressed: u32,
    pub elapsed_secs: f32,
}

/// Pur — extrait pour tests headless.
pub fn severity_for_input(keys_pressed_per_sec: u32) -> (&'static str, &'static str) {
    if keys_pressed_per_sec > 200 {
        (
            "warn",
            "keys_pressed > 200/s — input flood (stuck key? autoclicker?)",
        )
    } else {
        ("ok", "")
    }
}

pub fn sys_track_input_accum(
    mut events: MessageReader<KeyboardInput>,
    action_q: Query<&ActionState<PlayerAction>>,
    mut accum: Local<InputSensorAccum>,
    time: Res<Time>,
) {
    // Accumulate every frame to avoid Bevy 2-frame message purge.
    for ev in events.read() {
        match ev.state {
            ButtonState::Pressed => accum.keys_pressed = accum.keys_pressed.saturating_add(1),
            ButtonState::Released => accum.keys_released = accum.keys_released.saturating_add(1),
        }
    }
    for state in &action_q {
        accum.actions_just_pressed = accum
            .actions_just_pressed
            .saturating_add(state.get_just_pressed().len() as u32);
    }

    accum.elapsed_secs += time.delta_secs();
    if accum.elapsed_secs < 1.0 {
        return;
    }
    accum.elapsed_secs = 0.0;

    let (severity, next_step) = severity_for_input(accum.keys_pressed);

    let json = format!(
        r#"{{"id":"input","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"keys_pressed_per_sec":{},"keys_released_per_sec":{},"actions_just_pressed_per_sec":{}}}"#,
        time.elapsed_secs(),
        accum.keys_pressed,
        accum.keys_released,
        accum.actions_just_pressed,
    );

    if let Err(e) = std::fs::write("forgia2_input.json", &json) {
        warn!("[forgia-observability] input sensor write failed: {e}");
    }

    accum.keys_pressed = 0;
    accum.keys_released = 0;
    accum.actions_just_pressed = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_below_200() {
        let (sev, next) = severity_for_input(50);
        assert_eq!(sev, "ok");
        assert!(next.is_empty());
    }

    #[test]
    fn severity_warn_above_200() {
        let (sev, next) = severity_for_input(300);
        assert_eq!(sev, "warn");
        assert!(next.contains("flood"));
    }

    #[test]
    fn input_accum_default_zero() {
        let a = InputSensorAccum::default();
        assert_eq!(a.keys_pressed, 0);
        assert_eq!(a.keys_released, 0);
        assert_eq!(a.actions_just_pressed, 0);
        assert_eq!(a.elapsed_secs, 0.0);
    }
}
