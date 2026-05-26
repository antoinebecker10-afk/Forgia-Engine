//! `ReplayPlayer` Resource + advance system Bevy.
//!
//! Active quand `ReplayMode::Replaying`. Lit une `RecordedSession` chargée
//! depuis disque et avance un cursor frame-by-frame, exposant les inputs
//! du frame courant via `peek_current_frame_inputs()`.
//!
//! Les inputs synthétiques sont **lus** par les systèmes consommateurs
//! (forgia-game/src/...) qui basculeront entre `ButtonInput<KeyCode>`
//! réel ou la liste replay selon `ReplayMode`. Cette intégration concrète
//! sera faite en P5 harness (P4 livre l'API d'accès, pas le pilotage du jeu).

use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, ResMut};

use crate::recorder::ReplayMode;
use crate::session::{InputEvent, RecordedSession};

/// Player Resource : détient une session chargée + cursor frame courant.
///
/// API pull-based : le caller (système consommateur) `peek_current_frame_inputs()`
/// pour récupérer les events à appliquer ce frame, puis `advance_frame()`
/// pour passer au suivant.
#[derive(Resource, Debug, Default)]
pub struct ReplayPlayer {
    session: Option<RecordedSession>,
    current_frame: u64,
    /// Index dans `session.inputs` du prochain événement à consommer.
    /// Optimisation : évite re-scan O(N) sur chaque frame.
    next_input_idx: usize,
    /// `true` si `current_frame >= session.frame_count` (replay terminé).
    finished: bool,
}

impl ReplayPlayer {
    /// Charge une session pour replay. Reset le cursor à frame 0.
    pub fn load(&mut self, session: RecordedSession) {
        self.session = Some(session);
        self.current_frame = 0;
        self.next_input_idx = 0;
        self.finished = false;
    }

    /// `true` si une session est chargée.
    pub const fn is_loaded(&self) -> bool {
        self.session.is_some()
    }

    /// Frame courante (cursor).
    pub const fn current_frame(&self) -> u64 {
        self.current_frame
    }

    /// `true` si replay terminé (cursor a dépassé frame_count).
    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    /// Lecture seule de la session chargée (introspection / tests).
    pub fn session(&self) -> Option<&RecordedSession> {
        self.session.as_ref()
    }

    /// Récupère les inputs à appliquer pour la frame courante.
    /// Stable order (tel qu'enregistré).
    ///
    /// # Coût
    ///
    /// O(N) où N = nombre d'inputs à cette frame (typiquement 0-5). Le
    /// scan complet du Vec inputs est évité grâce à `next_input_idx`
    /// qui avance avec le cursor.
    pub fn peek_current_frame_inputs(&self) -> Vec<InputEvent> {
        let Some(session) = self.session.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut idx = self.next_input_idx;
        while let Some(event) = session.inputs.get(idx) {
            if event.frame() < self.current_frame {
                idx += 1;
                continue;
            }
            if event.frame() > self.current_frame {
                break;
            }
            out.push(event.clone());
            idx += 1;
        }
        out
    }

    /// Avance le cursor d'une frame. Met à jour `next_input_idx` pour
    /// pointer après les events de la frame qu'on vient de consommer.
    pub fn advance_frame(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        // Skip les events à current_frame (consommés) → next_input_idx pointe
        // après eux, prêt pour le prochain frame.
        while let Some(event) = session.inputs.get(self.next_input_idx) {
            if event.frame() <= self.current_frame {
                self.next_input_idx += 1;
            } else {
                break;
            }
        }
        self.current_frame = self.current_frame.wrapping_add(1);
        if self.current_frame > session.frame_count {
            self.finished = true;
        }
    }

    /// Décharge la session (cleanup post-replay).
    pub fn unload(&mut self) -> Option<RecordedSession> {
        self.current_frame = 0;
        self.next_input_idx = 0;
        self.finished = false;
        self.session.take()
    }
}

/// Système Bevy : avance le cursor du player chaque frame quand
/// `ReplayMode::Replaying`. Les systèmes consommateurs (P5+) appelleront
/// `peek_current_frame_inputs()` AVANT cette advance pour récupérer les
/// events de la frame courante.
///
/// Ordering : ce système doit tourner en `Last` (PostUpdate) pour que les
/// consommateurs (Update) aient déjà lu les inputs du frame courant.
pub fn advance_replay_cursor_system(
    mode: Res<ReplayMode>,
    mut player: ResMut<ReplayPlayer>,
) {
    if !mode.is_replaying() || !player.is_loaded() || player.is_finished() {
        return;
    }
    player.advance_frame();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_session_3_frames() -> RecordedSession {
        let mut s = RecordedSession::new("test");
        s.frame_count = 3;
        s.record_input(InputEvent::KeyPressed { code: 1, frame: 0 });
        s.record_input(InputEvent::KeyPressed { code: 2, frame: 1 });
        s.record_input(InputEvent::KeyPressed { code: 3, frame: 1 });
        s.record_input(InputEvent::KeyReleased { code: 1, frame: 2 });
        s
    }

    #[test]
    fn player_default_unloaded() {
        let p = ReplayPlayer::default();
        assert!(!p.is_loaded());
        assert!(!p.is_finished());
        assert_eq!(p.current_frame(), 0);
    }

    #[test]
    fn player_load_resets_cursor() {
        let mut p = ReplayPlayer::default();
        p.load(build_session_3_frames());
        assert!(p.is_loaded());
        assert_eq!(p.current_frame(), 0);
        assert!(!p.is_finished());
    }

    #[test]
    fn peek_at_frame_0_returns_one_event() {
        let mut p = ReplayPlayer::default();
        p.load(build_session_3_frames());
        let events = p.peek_current_frame_inputs();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].frame(), 0);
    }

    #[test]
    fn peek_at_frame_1_returns_two_events() {
        let mut p = ReplayPlayer::default();
        p.load(build_session_3_frames());
        p.advance_frame();
        let events = p.peek_current_frame_inputs();
        assert_eq!(events.len(), 2);
        for e in &events {
            assert_eq!(e.frame(), 1);
        }
    }

    #[test]
    fn peek_at_empty_frame_returns_empty() {
        let mut s = RecordedSession::new("test");
        s.frame_count = 5;
        s.record_input(InputEvent::KeyPressed { code: 1, frame: 3 });
        let mut p = ReplayPlayer::default();
        p.load(s);
        // Frame 0 : no events
        assert!(p.peek_current_frame_inputs().is_empty());
        p.advance_frame();
        assert!(p.peek_current_frame_inputs().is_empty());
    }

    #[test]
    fn advance_past_frame_count_marks_finished() {
        let mut p = ReplayPlayer::default();
        p.load(build_session_3_frames()); // frame_count=3
        for _ in 0..3 {
            p.advance_frame();
        }
        assert!(!p.is_finished(), "still running at frame 3 == frame_count");
        p.advance_frame();
        assert!(p.is_finished(), "finished after frame 4 > frame_count");
    }

    #[test]
    fn advance_unloaded_noop() {
        let mut p = ReplayPlayer::default();
        // No load → advance no-op (no panic)
        p.advance_frame();
        assert_eq!(p.current_frame(), 0);
    }

    #[test]
    fn unload_drains_session() {
        let mut p = ReplayPlayer::default();
        p.load(build_session_3_frames());
        p.advance_frame();
        let drained = p.unload().expect("session");
        assert_eq!(drained.frame_count, 3);
        assert!(!p.is_loaded());
        assert_eq!(p.current_frame(), 0);
    }

    #[test]
    fn next_input_idx_optimization_advances_cursor() {
        // Vérifie que l'optimisation next_input_idx fonctionne :
        // après advance, on ne re-scan pas les events des frames passées.
        let mut p = ReplayPlayer::default();
        p.load(build_session_3_frames());
        p.advance_frame();
        p.advance_frame();
        // Frame 2 : 1 event KeyReleased
        let events = p.peek_current_frame_inputs();
        assert_eq!(events.len(), 1);
        match &events[0] {
            InputEvent::KeyReleased { code, frame } => {
                assert_eq!(*code, 1);
                assert_eq!(*frame, 2);
            }
            other => panic!("expected KeyReleased, got {other:?}"),
        }
    }
}
