//! audio_sensor.rs — Producteur `forgia2_audio.json` (1Hz, cross-mode).
//!
//! Compte les `AudioInstance` actives (PlaybackState != Stopped) via
//! `Assets<AudioInstance>::iter()` + expose `BiomeAmbientState.current_biome()`.
//!
//! Story-469 — Vague 5 Phase 5b Session C.

use bevy::prelude::*;
use bevy_kira_audio::{AudioInstance, PlaybackState};
use forgia_audio::biome::BiomeAmbientState;

/// Pur — extrait pour tests headless.
pub fn severity_for_audio(active_instances: usize) -> (&'static str, &'static str) {
    if active_instances > 64 {
        (
            "warn",
            "active audio instances > 64 — possible handle leak (channel unbounded?)",
        )
    } else {
        ("ok", "")
    }
}

pub fn sys_write_audio_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    audio_instances: Res<Assets<AudioInstance>>,
    biome_state: Option<Res<BiomeAmbientState>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let active_count = audio_instances
        .iter()
        .filter(|(_, inst)| !matches!(inst.state(), PlaybackState::Stopped))
        .count();

    let current_biome = biome_state
        .as_ref()
        .and_then(|s| s.current_biome())
        .map(|b| format!("{b:?}"))
        .unwrap_or_else(|| "none".to_string());

    let (severity, next_step) = severity_for_audio(active_count);

    let json = format!(
        r#"{{"id":"audio","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"active_instances":{},"current_biome":"{}"}}"#,
        time.elapsed_secs(),
        active_count,
        current_biome,
    );

    if let Err(e) = std::fs::write("forgia2_audio.json", &json) {
        warn!("[forgia-observability] audio sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_below_64() {
        let (sev, next) = severity_for_audio(10);
        assert_eq!(sev, "ok");
        assert!(next.is_empty());
    }

    #[test]
    fn severity_warn_above_64() {
        let (sev, next) = severity_for_audio(100);
        assert_eq!(sev, "warn");
        assert!(next.contains("leak"));
    }

    #[test]
    fn severity_boundary_64_is_ok() {
        assert_eq!(severity_for_audio(64).0, "ok");
        assert_eq!(severity_for_audio(65).0, "warn");
    }
}
