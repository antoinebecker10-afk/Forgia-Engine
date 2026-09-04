//! Forgia QA Harness — `TestApp` builder + property tests gameplay (P5).
//!
//! Permet aux développeurs d'écrire des tests Bevy de haut niveau qui
//! bootent un App minimal avec QA infra activée, exécutent des scénarios
//! scriptés, et assertent l'état final via `BugBus` + custom assertions.
//!
//! ## Quick start
//!
//! ```ignore
//! use forgia_qa_harness::TestApp;
//!
//! #[test]
//! fn my_test() {
//!     let mut test = TestApp::new()
//!         .with_qa_core()
//!         .build();
//!     test.tick(60);
//!     test.expect_no_bug();
//! }
//! ```
//!
//! ## Avec replay scénario
//!
//! ```ignore
//! use forgia_qa_harness::TestApp;
//! use forgia_qa_replay::{InputEvent, RecordedSession};
//!
//! #[test]
//! fn replay_no_blocker() {
//!     let mut session = RecordedSession::new("test");
//!     session.record_input(InputEvent::KeyPressed { code: 65, frame: 0 });
//!     session.frame_count = 10;
//!
//!     let mut test = TestApp::new()
//!         .with_qa_core()
//!         .with_replay()
//!         .with_session(session)
//!         .build();
//!     test.run_to_completion();
//!     test.expect_no_blocker();
//! }
//! ```
//!
//! Cf. `docs/qa/PLAN_P5.md` pour plan complet.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod collecting_sink;
#[cfg(feature = "golden")]
pub mod golden_frame;
pub mod test_app;

pub use collecting_sink::{BugCollector, CollectingBugSink};
#[cfg(feature = "golden")]
pub use golden_frame::{
    capture, tick_deterministic, GoldenFrame, SensorCollector, SensorRegistry, SnapshotBundle,
};
pub use test_app::{TestApp, TestAppBuilder};
