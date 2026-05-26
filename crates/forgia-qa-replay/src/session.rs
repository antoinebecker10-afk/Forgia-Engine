//! Types canoniques `RecordedSession`, `InputEvent`, `RngState`.
//!
//! Schéma stable v1, sérialisable RON pour persistence
//! `target/forgia_qa/replays/<bug_id>.replay.ron`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION;

/// Événement d'input capturé pendant la session, ordonné par frame.
///
/// Convention : on capture les **state changes** (Pressed/Released) plutôt
/// que le held state continu pour limiter le storage. Mouse/gamepad axes
/// capturés avec delta plutôt que valeur absolue.
///
/// Cf. PLAN_P4 §8 risk "Input capture rate" — compression delta à 60Hz
/// donne ~3-5 events/sec usage normal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputEvent {
    /// Touche clavier appuyée. `code` = u32 stable cross-platform (KeyCode discriminant).
    KeyPressed {
        /// Code touche (KeyCode discriminant Bevy).
        code: u32,
        /// Frame où l'événement a eu lieu.
        frame: u64,
    },
    /// Touche clavier relâchée.
    KeyReleased {
        /// Code touche.
        code: u32,
        /// Frame.
        frame: u64,
    },
    /// Mouvement souris (delta dx/dy).
    MouseMoved {
        /// Delta x.
        dx: f32,
        /// Delta y.
        dy: f32,
        /// Frame.
        frame: u64,
    },
    /// Bouton souris pressé/relâché.
    MouseButton {
        /// Bouton (0=Left, 1=Right, 2=Middle, ...).
        button: u8,
        /// `true` = pressé, `false` = relâché.
        pressed: bool,
        /// Frame.
        frame: u64,
    },
    /// Axe gamepad. `value` ∈ [-1, 1].
    GamepadAxis {
        /// Axe (0=LeftX, 1=LeftY, 2=RightX, 3=RightY, ...).
        axis: u8,
        /// Valeur normalisée.
        value: f32,
        /// Frame.
        frame: u64,
    },
    /// Bouton gamepad pressé/relâché.
    GamepadButton {
        /// Bouton (0=South, 1=East, ...).
        button: u8,
        /// `true` = pressé.
        pressed: bool,
        /// Frame.
        frame: u64,
    },
}

impl InputEvent {
    /// Frame de l'événement (helper extraction).
    pub fn frame(&self) -> u64 {
        match self {
            Self::KeyPressed { frame, .. }
            | Self::KeyReleased { frame, .. }
            | Self::MouseMoved { frame, .. }
            | Self::MouseButton { frame, .. }
            | Self::GamepadAxis { frame, .. }
            | Self::GamepadButton { frame, .. } => *frame,
        }
    }
}

/// Snapshot de l'état d'un PRNG Xoshiro256** (4 × u64).
///
/// Forgia utilise Xoshiro/PCG via `rand 0.8` ; `thread_rng` (non-déterministe)
/// est banni dans la logique gameplay (cf. `docs/qa/DETERMINISM.md`).
///
/// Snapshot pris **par tick système qui consomme un random** + initial seed.
/// Pas de capture continue : on dépend du déterminisme inter-snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RngState {
    /// Seed initial de la session (rejouable via `Xoshiro256StarStar::seed_from_u64`).
    pub seed: u64,
    /// État interne du PRNG à ce snapshot (4 × u64 pour Xoshiro256**).
    pub state: [u64; 4],
    /// Frame du snapshot.
    pub frame: u64,
}

/// Pair (frame, state_hash u64) pour détection divergence replay.
///
/// `state_hash` = hash gameplay-only (player position rounded i32, NPC count,
/// inventory contents) calculé via `forgia-game/src/debug/sensors/
/// determinism_fingerprint.rs::compute_fingerprint`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FingerprintEntry {
    /// Frame du fingerprint.
    pub frame: u64,
    /// Hash u64 gameplay-only.
    pub state_hash: u64,
}

/// Session enregistrée complète. Persistée en RON pretty.
///
/// Charge via [`RecordedSession::load_from_path`], replay via
/// `forgia-qa-replay::ReplayPlayer` Resource (W2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedSession {
    /// Version schema (= [`SCHEMA_VERSION`] en écriture).
    pub schema_version: u32,
    /// Identifiant `BugId` lié si la session a déclenché un BugReport.
    /// Format : `Some(ulid_string)` (pas de cycle dep entre crates).
    #[serde(default)]
    pub bug_id: Option<String>,
    /// SHA git + flag dirty au moment de l'enregistrement.
    pub build_hash: String,
    /// Timestamp début session UTC.
    pub started_at: DateTime<Utc>,
    /// Compteur frames totales enregistrées.
    pub frame_count: u64,
    /// Tous les InputEvent ordonnés par frame croissant.
    pub inputs: Vec<InputEvent>,
    /// Snapshots RNG state (initial + checkpoints par tick système).
    pub rng_seeds: Vec<RngState>,
    /// Fingerprints gameplay-only par frame pour divergence detection.
    pub fingerprints: Vec<FingerprintEntry>,
}

impl RecordedSession {
    /// Construit une session vide avec `schema_version` courant et timestamps now.
    pub fn new(build_hash: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            bug_id: None,
            build_hash: build_hash.into(),
            started_at: Utc::now(),
            frame_count: 0,
            inputs: Vec::new(),
            rng_seeds: Vec::new(),
            fingerprints: Vec::new(),
        }
    }

    /// Lie cette session à un `BugReport` via son ULID.
    pub fn with_bug_id(mut self, bug_id_ulid: impl Into<String>) -> Self {
        self.bug_id = Some(bug_id_ulid.into());
        self
    }

    /// Sérialise en RON pretty.
    pub fn to_ron_pretty(&self) -> Result<String, RecordedSessionError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| RecordedSessionError::Ron(e.to_string()))
    }

    /// Désérialise depuis chaîne RON. Vérifie `schema_version`.
    pub fn parse_ron(text: &str) -> Result<Self, RecordedSessionError> {
        let session: RecordedSession = ron::from_str(text)
            .map_err(|e| RecordedSessionError::Ron(e.to_string()))?;
        if session.schema_version != SCHEMA_VERSION {
            return Err(RecordedSessionError::SchemaMismatch {
                found: session.schema_version,
                expected: SCHEMA_VERSION,
            });
        }
        Ok(session)
    }

    /// Charge depuis fichier disque.
    pub fn load_from_path<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, RecordedSessionError> {
        let text = std::fs::read_to_string(path).map_err(RecordedSessionError::Io)?;
        Self::parse_ron(&text)
    }

    /// Sauvegarde sur disque (RON pretty).
    pub fn save_to_path<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), RecordedSessionError> {
        let text = self.to_ron_pretty()?;
        std::fs::write(path, text).map_err(RecordedSessionError::Io)
    }

    /// Pousse un input event. Convention : `frame` croissant monotone (caller
    /// responsability — pas de check ici hot path).
    pub fn record_input(&mut self, event: InputEvent) {
        self.inputs.push(event);
    }

    /// Pousse un snapshot RNG.
    pub fn record_rng(&mut self, snap: RngState) {
        self.rng_seeds.push(snap);
    }

    /// Pousse un fingerprint frame.
    pub fn record_fingerprint(&mut self, entry: FingerprintEntry) {
        self.fingerprints.push(entry);
    }

    /// Retourne les inputs à rejouer pour une frame donnée (ordre stable).
    pub fn inputs_at_frame(&self, frame: u64) -> impl Iterator<Item = &InputEvent> {
        self.inputs.iter().filter(move |e| e.frame() == frame)
    }
}

/// Erreur de manipulation `RecordedSession`.
#[derive(Debug)]
pub enum RecordedSessionError {
    /// Erreur I/O.
    Io(std::io::Error),
    /// Erreur de parsing/sérialisation RON. Stockée comme String pour éviter
    /// les divergences entre `ron::Error` (ser) et `ron::de::SpannedError`
    /// (deser) qui ne partagent pas un type d'erreur commun.
    Ron(String),
    /// Schema version inattendue.
    SchemaMismatch {
        /// Version trouvée.
        found: u32,
        /// Version attendue.
        expected: u32,
    },
}

impl std::fmt::Display for RecordedSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Ron(e) => write!(f, "RON error: {e}"),
            Self::SchemaMismatch { found, expected } => {
                write!(f, "schema_version mismatch: found {found}, expected {expected}")
            }
        }
    }
}

impl std::error::Error for RecordedSessionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_event_frame_extraction() {
        assert_eq!(
            InputEvent::KeyPressed { code: 42, frame: 100 }.frame(),
            100
        );
        assert_eq!(
            InputEvent::MouseMoved { dx: 1.0, dy: 2.0, frame: 200 }.frame(),
            200
        );
    }

    #[test]
    fn recorded_session_new_canonical() {
        let s = RecordedSession::new("abc123");
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert_eq!(s.build_hash, "abc123");
        assert_eq!(s.frame_count, 0);
        assert!(s.inputs.is_empty());
        assert!(s.rng_seeds.is_empty());
        assert!(s.fingerprints.is_empty());
        assert!(s.bug_id.is_none());
    }

    #[test]
    fn recorded_session_with_bug_id() {
        let s = RecordedSession::new("abc").with_bug_id("01HXYZ1234567890ABCDEFG");
        assert_eq!(s.bug_id.as_deref(), Some("01HXYZ1234567890ABCDEFG"));
    }

    #[test]
    fn record_input_appends() {
        let mut s = RecordedSession::new("test");
        s.record_input(InputEvent::KeyPressed { code: 1, frame: 0 });
        s.record_input(InputEvent::KeyReleased { code: 1, frame: 5 });
        assert_eq!(s.inputs.len(), 2);
    }

    #[test]
    fn inputs_at_frame_filters_correctly() {
        let mut s = RecordedSession::new("test");
        s.record_input(InputEvent::KeyPressed { code: 1, frame: 0 });
        s.record_input(InputEvent::KeyPressed { code: 2, frame: 5 });
        s.record_input(InputEvent::KeyPressed { code: 3, frame: 5 });
        s.record_input(InputEvent::KeyPressed { code: 4, frame: 10 });

        let at_5: Vec<_> = s.inputs_at_frame(5).collect();
        assert_eq!(at_5.len(), 2);
    }

    #[test]
    fn ron_roundtrip_empty_session() {
        let s = RecordedSession::new("hash123");
        let ron = s.to_ron_pretty().expect("serialize");
        let back = RecordedSession::parse_ron(&ron).expect("deserialize");
        assert_eq!(back.build_hash, "hash123");
        assert_eq!(back.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn ron_roundtrip_full_session() {
        let mut s = RecordedSession::new("hash456").with_bug_id("01HXYZ");
        s.record_input(InputEvent::KeyPressed { code: 65, frame: 0 });
        s.record_input(InputEvent::MouseMoved { dx: 5.0, dy: -3.0, frame: 1 });
        s.record_input(InputEvent::GamepadAxis { axis: 0, value: 0.5, frame: 2 });
        s.record_rng(RngState { seed: 42, state: [1, 2, 3, 4], frame: 0 });
        s.record_fingerprint(FingerprintEntry { frame: 1, state_hash: 0xDEADBEEF });
        s.frame_count = 100;

        let ron = s.to_ron_pretty().expect("serialize");
        let back = RecordedSession::parse_ron(&ron).expect("deserialize");

        assert_eq!(back.bug_id.as_deref(), Some("01HXYZ"));
        assert_eq!(back.frame_count, 100);
        assert_eq!(back.inputs.len(), 3);
        assert_eq!(back.rng_seeds.len(), 1);
        assert_eq!(back.fingerprints.len(), 1);
        assert_eq!(back.fingerprints[0].state_hash, 0xDEADBEEF);
    }

    #[test]
    fn schema_mismatch_rejected() {
        let bad = r#"(
    schema_version: 999,
    bug_id: None,
    build_hash: "x",
    started_at: "2026-05-04T00:00:00Z",
    frame_count: 0,
    inputs: [],
    rng_seeds: [],
    fingerprints: [],
)"#;
        match RecordedSession::parse_ron(bad) {
            Err(RecordedSessionError::SchemaMismatch { found: 999, expected: 1 }) => {}
            other => panic!("expected SchemaMismatch, got {other:?}"),
        }
    }

    #[test]
    fn save_and_load_roundtrip_disk() {
        let dir = std::env::temp_dir();
        let path = dir.join("forgia_qa_replay_test.ron");
        let mut s = RecordedSession::new("disk_test");
        s.record_input(InputEvent::KeyPressed { code: 1, frame: 0 });
        s.save_to_path(&path).expect("save");

        let back = RecordedSession::load_from_path(&path).expect("load");
        assert_eq!(back.build_hash, "disk_test");
        assert_eq!(back.inputs.len(), 1);

        let _ = std::fs::remove_file(&path);
    }
}
