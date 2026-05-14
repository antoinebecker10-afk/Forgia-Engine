#![allow(dead_code, unused_imports)]
//! Port verbatim de `forgia-game/src/effects/arena_feedback.rs` (V1).
//! # Arena Feedback — SFX kill confirm + damage taken (story-427, 2026-05-12)
//!
//! Polish FPS Arena V1 : sound feedback bouclant la boucle gameplay.
//! - Bot killed by player → "thud" lourd au position du bot mort
//! - Player damaged by bot → "glass" léger pour feedback hit
//!
//! Patterns :
//! - **Découplé via events** (combat-code.md rule "JAMAIS d'import direct effects/")
//! - **AudioRegistry** (lazy load + variation cycling, pas L1 violation)
//! - **No-hardcode** : SFX IDs constants nommées + AUDIO_VARIANT_COUNT local

use bevy::prelude::*;

// TODO: port from V1 — audio_registry::AudioRegistry
// use forgia_audio_core::AudioRegistry;

// TODO: port from V1 — components::{BotKilledEvent, DamagePlayerEvent}
// use forgia_combat::events::{BotKilledEvent, DamagePlayerEvent};

// TODO: port from V1 — app_state::GameSet
// use forgia_core::app_state::GameSet;

/// Nombre de variantes audio dans `sfx/impacts/impactBell_heavy_NNN.ogg`
/// (000 à 004 → 5 variantes). Cycling Local<u32> évite la répétition.
const KILL_AUDIO_VARIANTS: u32 = 5;
/// Nombre de variantes audio dans `sfx/impacts/impactGlass_medium_NNN.ogg`
/// (000 à 004 → 5 variantes).
const DAMAGE_AUDIO_VARIANTS: u32 = 5;

/// Compteurs observables (sensor `forgia_arena_feedback.json`).
#[derive(Resource, Default, Debug, Clone)]
pub struct ArenaFeedbackStats {
    pub kill_sounds_played: u32,
    pub damage_sounds_played: u32,
    pub kill_audio_missing_count: u32,
    pub damage_audio_missing_count: u32,
}

pub struct ArenaFeedbackPlugin;

impl Plugin for ArenaFeedbackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ArenaFeedbackStats>()
            .add_systems(
                Update,
                (
                    // TODO: re-enable once BotKilledEvent + AudioRegistry ported
                    // arena_bot_kill_sound.in_set(GameSet::Effects),
                    // arena_player_damage_sound.in_set(GameSet::Effects),
                    arena_feedback_sensor_export,
                ),
            );
    }
}

// TODO: arena_bot_kill_sound requires AudioRegistry + BotKilledEvent (cross-crate)
// pub fn arena_bot_kill_sound(...)

// TODO: arena_player_damage_sound requires AudioRegistry + DamagePlayerEvent (cross-crate)
// pub fn arena_player_damage_sound(...)

/// Export `forgia_arena_feedback.json` toutes les 10s.
pub fn arena_feedback_sensor_export(
    time: Res<Time>,
    mut local_timer: Local<Timer>,
    stats: Res<ArenaFeedbackStats>,
) {
    if local_timer.duration().is_zero() {
        *local_timer = Timer::from_seconds(10.0, TimerMode::Repeating);
    }
    local_timer.tick(time.delta());
    if !local_timer.just_finished() {
        return;
    }
    let json = format!(
        "{{\n  \"timestamp_secs\": {:.1},\n  \"kill_sounds_played\": {},\n  \"damage_sounds_played\": {},\n  \"kill_audio_missing\": {},\n  \"damage_audio_missing\": {}\n}}",
        time.elapsed_secs(),
        stats.kill_sounds_played,
        stats.damage_sounds_played,
        stats.kill_audio_missing_count,
        stats.damage_audio_missing_count,
    );
    let _ = std::fs::write("forgia_arena_feedback.json", &json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_default_zero() {
        let s = ArenaFeedbackStats::default();
        assert_eq!(s.kill_sounds_played, 0);
        assert_eq!(s.damage_sounds_played, 0);
        assert_eq!(s.kill_audio_missing_count, 0);
        assert_eq!(s.damage_audio_missing_count, 0);
    }

    #[test]
    fn variant_count_constants_valid() {
        assert!(KILL_AUDIO_VARIANTS >= 1);
        assert!(DAMAGE_AUDIO_VARIANTS >= 1);
    }
}
