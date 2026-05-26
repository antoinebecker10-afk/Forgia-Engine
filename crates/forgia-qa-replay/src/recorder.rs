//! `ReplayRecorder` Resource + capture systems Bevy.
//!
//! Active quand `ReplayMode::Recording`. Capture les InputEvents au fil
//! des frames et les pousse dans une `RecordedSession` détenue par le
//! recorder. À la demande (Ctrl+Shift+R ou bug detected), la session est
//! sauvegardée sur disque.

use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;

use crate::session::{InputEvent, RecordedSession};

/// Mode du système replay.
///
/// Source unique de vérité pour activer/désactiver Recording vs Replaying.
/// Les systems Capture / Player gating dessus.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub enum ReplayMode {
    /// Aucune capture ni replay actif (default).
    #[default]
    Disabled,
    /// Capture en cours : les InputEvents sont poussés dans `ReplayRecorder`.
    Recording,
    /// Replay en cours : `ReplayPlayer` lit la session et synthétise les inputs.
    /// Le path source est stocké pour log + closure.
    Replaying {
        /// Chemin vers le fichier `.replay.ron` chargé.
        source_path: String,
    },
}

impl ReplayMode {
    /// `true` si en mode Recording.
    pub const fn is_recording(&self) -> bool {
        matches!(self, Self::Recording)
    }

    /// `true` si en mode Replaying.
    pub const fn is_replaying(&self) -> bool {
        matches!(self, Self::Replaying { .. })
    }

    /// `true` si Disabled (aucun overhead).
    pub const fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }
}

/// Recorder Resource : détient la session en cours d'enregistrement.
///
/// Push events via `record_input` / `record_rng` / `record_fingerprint`.
/// Récupère la session finale via `take_session()` (drain pour save disk).
#[derive(Resource, Debug, Default)]
pub struct ReplayRecorder {
    session: Option<RecordedSession>,
    /// Compteur frame courant (incrémenté par `advance_frame()`).
    current_frame: u64,
}

impl ReplayRecorder {
    /// Démarre un nouvel enregistrement avec un build_hash. Reset toute
    /// session précédente non drainée.
    pub fn start(&mut self, build_hash: impl Into<String>) {
        self.session = Some(RecordedSession::new(build_hash));
        self.current_frame = 0;
    }

    /// Avance le compteur frame de 1 (à appeler en début de chaque frame
    /// par le système de capture).
    pub fn advance_frame(&mut self) {
        self.current_frame = self.current_frame.wrapping_add(1);
        if let Some(s) = self.session.as_mut() {
            s.frame_count = self.current_frame;
        }
    }

    /// Frame courante.
    pub const fn current_frame(&self) -> u64 {
        self.current_frame
    }

    /// `true` si une session est active (post-`start`, pre-`take_session`).
    pub const fn is_active(&self) -> bool {
        self.session.is_some()
    }

    /// Pousse un input event dans la session active. No-op si inactive.
    pub fn record_input(&mut self, event: InputEvent) {
        if let Some(s) = self.session.as_mut() {
            s.record_input(event);
        }
    }

    /// Drain la session courante (les checks `is_active` retourneront false).
    /// Caller responsabilité de save_to_path() après take.
    pub fn take_session(&mut self) -> Option<RecordedSession> {
        self.session.take()
    }

    /// Lecture seule de la session courante (utile pour tests + introspection).
    pub fn session(&self) -> Option<&RecordedSession> {
        self.session.as_ref()
    }
}

/// Système Bevy : capture les transitions clavier (Pressed/Released)
/// quand `ReplayMode::Recording`.
///
/// Détecte les `just_pressed` et `just_released` sur tous les `KeyCode`
/// connus de Bevy 0.18 et les push dans le recorder. La capture est
/// **state-change only** (pas de held state continu) pour limiter le storage.
///
/// Source/key encoding : on stocke le discriminant `KeyCode` cast en u32
/// stable (assumption Bevy 0.18 ne renumérote pas les variants).
pub fn capture_keyboard_input_system(
    mode: Res<ReplayMode>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut recorder: ResMut<ReplayRecorder>,
) {
    if !mode.is_recording() || !recorder.is_active() {
        return;
    }
    // Option<Res<>> permet au système de tourner sans panic dans contextes
    // headless (forgia_repro CLI utilise MinimalPlugins sans InputPlugin).
    let Some(keys) = keys else { return };
    recorder.advance_frame();
    let frame = recorder.current_frame();

    for key in keys.get_just_pressed() {
        let code = keycode_to_u32(*key);
        recorder.record_input(InputEvent::KeyPressed { code, frame });
    }
    for key in keys.get_just_released() {
        let code = keycode_to_u32(*key);
        recorder.record_input(InputEvent::KeyReleased { code, frame });
    }
}

/// Encode un `KeyCode` en u32 stable. Utilise le discriminant Debug
/// (string-based) hashé pour stabilité cross-version Bevy.
///
/// Note : Bevy 0.18 `KeyCode` est un enum non-`#[repr(u32)]`, donc on ne
/// peut pas faire `*key as u32` directement. On utilise un hash du nom.
fn keycode_to_u32(key: KeyCode) -> u32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{key:?}").hash(&mut hasher);
    (hasher.finish() & 0xFFFF_FFFF) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_mode_default_disabled() {
        let m = ReplayMode::default();
        assert!(m.is_disabled());
        assert!(!m.is_recording());
        assert!(!m.is_replaying());
    }

    #[test]
    fn replay_mode_recording_helpers() {
        let m = ReplayMode::Recording;
        assert!(m.is_recording());
        assert!(!m.is_disabled());
    }

    #[test]
    fn replay_mode_replaying_helpers() {
        let m = ReplayMode::Replaying { source_path: "x.ron".into() };
        assert!(m.is_replaying());
        assert!(!m.is_recording());
    }

    #[test]
    fn recorder_start_creates_session() {
        let mut r = ReplayRecorder::default();
        assert!(!r.is_active());
        r.start("build_abc");
        assert!(r.is_active());
        assert_eq!(r.session().unwrap().build_hash, "build_abc");
        assert_eq!(r.current_frame(), 0);
    }

    #[test]
    fn recorder_advance_frame_increments() {
        let mut r = ReplayRecorder::default();
        r.start("test");
        r.advance_frame();
        r.advance_frame();
        r.advance_frame();
        assert_eq!(r.current_frame(), 3);
        assert_eq!(r.session().unwrap().frame_count, 3);
    }

    #[test]
    fn recorder_record_input_appends() {
        let mut r = ReplayRecorder::default();
        r.start("test");
        r.record_input(InputEvent::KeyPressed { code: 1, frame: 0 });
        r.record_input(InputEvent::KeyReleased { code: 1, frame: 5 });
        assert_eq!(r.session().unwrap().inputs.len(), 2);
    }

    #[test]
    fn recorder_record_input_inactive_noop() {
        let mut r = ReplayRecorder::default();
        // Pas de start → inactive
        r.record_input(InputEvent::KeyPressed { code: 1, frame: 0 });
        assert!(!r.is_active());
        assert!(r.session().is_none());
    }

    #[test]
    fn recorder_take_session_drains() {
        let mut r = ReplayRecorder::default();
        r.start("test");
        r.record_input(InputEvent::KeyPressed { code: 1, frame: 0 });
        let s = r.take_session().expect("session");
        assert_eq!(s.inputs.len(), 1);
        assert!(!r.is_active());
    }

    #[test]
    fn keycode_to_u32_deterministic() {
        let a = keycode_to_u32(KeyCode::KeyA);
        let b = keycode_to_u32(KeyCode::KeyA);
        assert_eq!(a, b, "same key → same code");
    }

    #[test]
    fn keycode_to_u32_distinct_keys() {
        let a = keycode_to_u32(KeyCode::KeyA);
        let b = keycode_to_u32(KeyCode::KeyB);
        assert_ne!(a, b, "different keys → different codes");
    }

    #[test]
    fn capture_system_inactive_when_disabled() {
        use bevy::app::App;
        let mut app = App::new();
        app.init_resource::<ReplayMode>()
            .init_resource::<ReplayRecorder>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(bevy::app::Update, capture_keyboard_input_system);

        // Mode default = Disabled
        app.update();
        let recorder = app.world().resource::<ReplayRecorder>();
        // Le système doit no-op (pas d'advance frame) car Disabled.
        assert_eq!(recorder.current_frame(), 0);
        assert!(!recorder.is_active());
    }

    #[test]
    fn capture_system_advances_frame_when_recording() {
        use bevy::app::App;
        let mut app = App::new();
        app.init_resource::<ReplayMode>()
            .init_resource::<ReplayRecorder>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(bevy::app::Update, capture_keyboard_input_system);

        // Démarrer le recording
        app.world_mut().resource_mut::<ReplayRecorder>().start("test");
        app.world_mut().insert_resource(ReplayMode::Recording);

        // Tick 1 frame
        app.update();
        let frame_after_1 = app.world().resource::<ReplayRecorder>().current_frame();
        assert_eq!(frame_after_1, 1, "1 update → frame=1");

        // Tick 2 frames
        app.update();
        app.update();
        let frame_after_3 = app.world().resource::<ReplayRecorder>().current_frame();
        assert_eq!(frame_after_3, 3, "3 updates → frame=3");
    }

    #[test]
    fn capture_system_no_panic_without_input_resource() {
        // Vérifie que le système tourne sans panic dans un contexte headless
        // (MinimalPlugins sans InputPlugin) où ButtonInput<KeyCode> n'existe pas.
        use bevy::app::App;
        let mut app = App::new();
        app.init_resource::<ReplayMode>()
            .init_resource::<ReplayRecorder>()
            .add_systems(bevy::app::Update, capture_keyboard_input_system);

        app.world_mut().resource_mut::<ReplayRecorder>().start("test");
        app.world_mut().insert_resource(ReplayMode::Recording);

        // Pas d'InputPlugin → ButtonInput<KeyCode> absent.
        // Le système doit early-return sans panic.
        app.update();
        let recorder = app.world().resource::<ReplayRecorder>();
        // advance_frame() pas appelé car keys absent.
        assert_eq!(recorder.current_frame(), 0);
    }

    #[test]
    fn replay_mode_replaying_path_captured() {
        let m = ReplayMode::Replaying { source_path: "/tmp/foo.ron".into() };
        match m {
            ReplayMode::Replaying { source_path } => {
                assert_eq!(source_path, "/tmp/foo.ron");
            }
            _ => panic!("expected Replaying variant"),
        }
    }
}
