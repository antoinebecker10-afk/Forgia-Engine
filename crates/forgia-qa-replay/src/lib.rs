//! Forgia QA Replay — replay déterministe gameplay-only (story-391 P4).
//!
//! Permet de capturer une session de jeu (inputs + RNG seeds + state hashes
//! par frame) et la rejouer 1:1 pour reproduire un `BugReport` localement.
//! Cible : time-to-reproduce **<2 min** (vs 30min-4h baseline).
//!
//! ## Frontière déterministe (Q2 décision)
//!
//! **Inclus** : inputs joueur, RNG seeds, time, gameplay state components
//! tagged via Reflect.
//!
//! **Exclus** : Rapier physics, bevy_hanabi VFX, bevy_kira_audio, rendering,
//! asset loading order. Ces domaines sont **non-déterministes assumés**
//! cross-platform (FP differences, GPU driver state, multi-threading order).
//!
//! Le fingerprint matche sur la **logique gameplay seule** (player position
//! rounded, NPC counts, inventory contents). Cf. `docs/qa/DETERMINISM.md`.
//!
//! ## Workflow
//!
//! 1. Run gameplay → bug détecté → `BugReport` émis avec
//!    `context.input_replay_path = "target/forgia_qa/replays/<bug_id>.replay.ron"`.
//! 2. `cargo run --bin forgia_repro -- <path.replay.ron>` charge la session.
//! 3. Le jeu démarre en mode replay, rejoue les inputs aux bons frames.
//! 4. Reproduction 1:1 → debug vs deviner.
//!
//! Cf. `docs/qa/PLAN_P4.md` pour plan complet et critères de sortie.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod player;
pub mod plugin;
pub mod recorder;
pub mod session;

pub use player::{advance_replay_cursor_system, ReplayPlayer};
pub use plugin::ForgiaQaReplayPlugin;
pub use recorder::{capture_keyboard_input_system, ReplayMode, ReplayRecorder};
pub use session::{FingerprintEntry, InputEvent, RecordedSession, RecordedSessionError, RngState};

/// Version courante du schéma `RecordedSession` sérialisé en RON.
/// À incrémenter sur breaking change (rename champ, suppression variant).
pub const SCHEMA_VERSION: u32 = 1;
