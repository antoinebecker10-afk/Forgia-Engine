//! [`ForgiaQaReplayPlugin`] — wiring Bevy 0.18.
//!
//! Enregistre :
//! - [`ReplayMode`] Resource (default `Disabled`).
//! - [`ReplayRecorder`] Resource.
//! - [`ReplayPlayer`] Resource.
//! - [`capture_keyboard_input_system`] dans `Update` (gated `ReplayMode::Recording`).
//! - [`advance_replay_cursor_system`] dans `PostUpdate` (gated `ReplayMode::Replaying`).
//!
//! ## Workflow utilisateur
//!
//! ```ignore
//! use forgia_qa_replay::{ForgiaQaReplayPlugin, ReplayMode, ReplayRecorder};
//!
//! // 1. Plugin add (forgia-game/main.rs)
//! app.add_plugins(ForgiaQaReplayPlugin);
//!
//! // 2. Démarrer enregistrement (e.g. au pressed F11 ou auto-start dev mode)
//! let mut recorder = world.resource_mut::<ReplayRecorder>();
//! recorder.start("git_sha_abc");
//! world.insert_resource(ReplayMode::Recording);
//!
//! // 3. Stopper + sauvegarder (e.g. au bug detected ou Shift+F11)
//! let session = recorder.take_session().unwrap();
//! session.save_to_path("target/forgia_qa/replays/my.replay.ron").unwrap();
//! world.insert_resource(ReplayMode::Disabled);
//! ```
//!
//! ## Replay
//!
//! ```ignore
//! use forgia_qa_replay::{ReplayPlayer, ReplayMode, RecordedSession};
//!
//! let session = RecordedSession::load_from_path("...").unwrap();
//! let mut player = world.resource_mut::<ReplayPlayer>();
//! player.load(session);
//! world.insert_resource(ReplayMode::Replaying { source_path: "...".into() });
//!
//! // Les systèmes consommateurs (P5+) appelleront
//! // `player.peek_current_frame_inputs()` chaque frame.
//! ```

use bevy::app::{App, Plugin, PostUpdate, Update};

use crate::player::{advance_replay_cursor_system, ReplayPlayer};
use crate::recorder::{capture_keyboard_input_system, ReplayMode, ReplayRecorder};

/// Plugin Bevy à ajouter dans `forgia-game/src/main.rs`.
#[derive(Debug, Default)]
pub struct ForgiaQaReplayPlugin;

impl Plugin for ForgiaQaReplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReplayMode>()
            .init_resource::<ReplayRecorder>()
            .init_resource::<ReplayPlayer>()
            .add_systems(Update, capture_keyboard_input_system)
            .add_systems(PostUpdate, advance_replay_cursor_system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_registers_resources() {
        let mut app = App::new();
        app.add_plugins(ForgiaQaReplayPlugin);
        let world = app.world();
        assert!(world.contains_resource::<ReplayMode>());
        assert!(world.contains_resource::<ReplayRecorder>());
        assert!(world.contains_resource::<ReplayPlayer>());
    }

    #[test]
    fn default_mode_is_disabled() {
        let mut app = App::new();
        app.add_plugins(ForgiaQaReplayPlugin);
        let mode = app.world().resource::<ReplayMode>();
        assert!(mode.is_disabled());
    }
}
