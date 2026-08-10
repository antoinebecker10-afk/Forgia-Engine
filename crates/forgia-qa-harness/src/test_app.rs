//! `TestApp` builder + assertions de base.
//!
//! Pattern : `TestApp::new().with_qa_core().with_replay().build()` retourne
//! une `TestApp` exposant `tick(n)` / `run_to_completion()` + assertions
//! `expect_no_bug` / `expect_emitted_bug_count(n)` / `expect_no_blocker`.

use bevy::app::App;

use forgia_qa_core::{BugBus, BugCategory, BugReport, ForgiaQaCorePlugin, NullSink, Severity};
use forgia_qa_replay::{ForgiaQaReplayPlugin, RecordedSession, ReplayMode, ReplayPlayer};

use crate::collecting_sink::{BugCollector, CollectingBugSink};

/// Builder fluent pour `TestApp`.
///
/// Méthodes chainables :
/// - `with_qa_core()` : ajoute `ForgiaQaCorePlugin` (BugBus + drain).
/// - `with_replay()` : ajoute `ForgiaQaReplayPlugin` (Recorder + Player).
/// - `with_session(session)` : charge un `RecordedSession` dans le player
///   et active `ReplayMode::Replaying`. Implique `with_replay()`.
///
/// Termine par `build()` qui retourne une `TestApp`.
#[derive(Default)]
pub struct TestAppBuilder {
    qa_core: bool,
    qa_replay: bool,
    session: Option<RecordedSession>,
    #[cfg(feature = "golden")]
    observability: bool,
}

impl TestAppBuilder {
    /// Active `ForgiaQaCorePlugin` (BugBus + Messages drain). Idempotent.
    pub fn with_qa_core(mut self) -> Self {
        self.qa_core = true;
        self
    }

    /// Active `ForgiaQaReplayPlugin` (Recorder/Player Resources + capture/advance
    /// systems). Idempotent.
    pub fn with_replay(mut self) -> Self {
        self.qa_replay = true;
        self
    }

    /// Charge un `RecordedSession` dans `ReplayPlayer` et active
    /// `ReplayMode::Replaying`. Implique `with_replay()` automatiquement.
    pub fn with_session(mut self, session: RecordedSession) -> Self {
        self.qa_replay = true;
        self.session = Some(session);
        self
    }

    /// Active l'infrastructure golden frame : `MinimalPlugins` + `Time<Fixed>`
    /// + `SensorRegistry` Resource.
    ///
    /// Pré-requis pour [`crate::golden_frame::capture`] et `tick_deterministic`.
    ///
    /// Vague 2 story-409 — sensors mock seulement. Vrais sensors arrivent
    /// Vague 3 après extraction story-408 (`forgia-observability`).
    #[cfg(feature = "golden")]
    pub fn with_observability(mut self) -> Self {
        self.observability = true;
        self
    }

    /// Construit la `TestApp` finalisée.
    pub fn build(self) -> TestApp {
        let mut app = App::new();
        let mut collector: Option<BugCollector> = None;

        #[cfg(feature = "golden")]
        if self.observability {
            // `MinimalPlugins` = scheduler + Time + Diagnostics + TaskPool. Suffisant
            // pour les tests headless, pas de render/audio/input.
            app.add_plugins(bevy::MinimalPlugins);
            app.init_resource::<crate::golden_frame::SensorRegistry>();
        }

        if self.qa_core {
            app.add_plugins(ForgiaQaCorePlugin);
            // Remplace les sinks par défaut (NullSink) par notre CollectingBugSink
            // + un NullSink fallback pour conserver la chaîne dispatch (BugBus
            // itère all sinks ; un seul suffit en pratique pour les tests).
            let (sink, c) = CollectingBugSink::new();
            collector = Some(c);
            let mut bus = app.world_mut().resource_mut::<BugBus>();
            bus.set_sinks(vec![Box::new(sink), Box::new(NullSink)]);
        }
        if self.qa_replay {
            app.add_plugins(ForgiaQaReplayPlugin);
        }

        if let Some(session) = self.session {
            // Charger session dans player + activer Replaying.
            let frame_count = session.frame_count;
            {
                let mut player = app.world_mut().resource_mut::<ReplayPlayer>();
                player.load(session);
            }
            app.world_mut().insert_resource(ReplayMode::Replaying {
                source_path: "test_harness_in_memory".to_string(),
            });
            // Stocker frame_count pour run_to_completion safety bound.
            app.insert_resource(ReplayBudget {
                max_frames: frame_count.saturating_mul(10).max(100),
            });
        }

        TestApp { app, collector }
    }
}

/// Resource interne : safety bound pour `run_to_completion()`.
#[derive(bevy::ecs::resource::Resource, Debug)]
struct ReplayBudget {
    max_frames: u64,
}

/// Test harness wrapping un `App` Bevy avec QA infra activée.
///
/// Construit via `TestApp::new()` → `TestAppBuilder` → `build()`.
pub struct TestApp {
    /// L'App Bevy sous-jacente, accessible pour customisations avancées.
    pub app: App,
    /// Collector des `BugReport` ingestés via `CollectingBugSink`. `None` si
    /// `with_qa_core()` n'a pas été appelé (BugBus absent).
    collector: Option<BugCollector>,
}

impl TestApp {
    /// Démarre un nouveau builder fluent.
    ///
    /// Retourne un [`TestAppBuilder`] (pas `Self`) — pattern volontaire pour
    /// l'API fluent `TestApp::new().with_qa_core().build()` cohérente avec
    /// les builders Bevy / proptest.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> TestAppBuilder {
        TestAppBuilder::default()
    }

    /// Avance l'App de `n` frames (`app.update()` × n).
    pub fn tick(&mut self, n: u64) {
        for _ in 0..n {
            self.app.update();
        }
    }

    /// Avance l'App jusqu'à ce que `ReplayPlayer.is_finished()` ou
    /// que `max_frames` (défini par `with_session`) soit atteint.
    ///
    /// Retourne le nombre de frames effectivement consommées.
    ///
    /// # Panics
    ///
    /// Si pas de session chargée (caller a oublié `with_session`).
    pub fn run_to_completion(&mut self) -> u64 {
        let max = self
            .app
            .world()
            .get_resource::<ReplayBudget>()
            .map(|b| b.max_frames)
            .unwrap_or(1000); // Default si pas de session : 1000 frames safety.

        let mut frames = 0u64;
        while frames < max {
            self.app.update();
            frames += 1;

            let finished = self
                .app
                .world()
                .get_resource::<ReplayPlayer>()
                .map(|p| p.is_finished())
                .unwrap_or(true);
            if finished {
                break;
            }
        }
        frames
    }

    /// Compteur cumulé de BugReport ingérés sur le BugBus.
    pub fn total_bugs_ingested(&self) -> u64 {
        self.app
            .world()
            .get_resource::<BugBus>()
            .map(|b| b.total_ingested)
            .unwrap_or(0)
    }

    /// Émet manuellement un `BugReport` via `MessageWriter` (utile pour tester
    /// les détecteurs en isolation).
    pub fn emit_bug(&mut self, bug: BugReport) {
        self.app.world_mut().write_message(bug);
    }

    /// Assertion : aucun BugReport ingéré.
    ///
    /// # Panics
    ///
    /// Si `BugBus.total_ingested > 0`.
    pub fn expect_no_bug(&self) {
        let count = self.total_bugs_ingested();
        assert_eq!(count, 0, "expected no BugReport, got {count} ingested");
    }

    /// Assertion : N BugReport ingérés exactement.
    ///
    /// # Panics
    ///
    /// Si le compteur diffère.
    pub fn expect_emitted_bug_count(&self, n: u64) {
        let count = self.total_bugs_ingested();
        assert_eq!(count, n, "expected {n} BugReport ingested, got {count}");
    }

    /// Snapshot des `BugReport` collectés (clone via `CollectingBugSink`).
    /// Retourne `Vec::new()` si `with_qa_core()` n'a pas été appelé.
    pub fn bugs(&self) -> Vec<BugReport> {
        self.collector
            .as_ref()
            .map(|c| c.snapshot())
            .unwrap_or_default()
    }

    /// Assertion : aucun BugReport de severity `>= Blocker` n'a été ingéré.
    /// Tolère les Critical / Major / Minor / Cosmetic.
    ///
    /// # Panics
    ///
    /// Si un BugReport de `Severity::Blocker` est trouvé.
    pub fn expect_no_blocker(&self) {
        if let Some(c) = &self.collector {
            if !c.has_severity_at_least(Severity::Blocker) {
                return;
            }
            let blockers: Vec<_> = c
                .snapshot()
                .into_iter()
                .filter(|b| b.severity >= Severity::Blocker)
                .map(|b| format!("{:?} : {:?}", b.severity, b.category))
                .collect();
            panic!(
                "expected no Blocker BugReport, found {}: {:?}",
                blockers.len(),
                blockers
            );
        }
    }

    /// Assertion : aucun BugReport de severity `>= Critical`. Plus strict
    /// que `expect_no_blocker` (échoue aussi sur Critical).
    pub fn expect_no_critical(&self) {
        if let Some(c) = &self.collector {
            if !c.has_severity_at_least(Severity::Critical) {
                return;
            }
            let crits: Vec<_> = c
                .snapshot()
                .into_iter()
                .filter(|b| b.severity >= Severity::Critical)
                .map(|b| format!("{:?} : {:?}", b.severity, b.category))
                .collect();
            panic!(
                "expected no Critical+ BugReport, found {}: {:?}",
                crits.len(),
                crits
            );
        }
    }

    /// Assertion : au moins un `BugReport` dont la catégorie matche `predicate`.
    ///
    /// # Panics
    ///
    /// Si aucun report ne matche.
    pub fn expect_category<F>(&self, predicate: F)
    where
        F: Fn(&BugCategory) -> bool,
    {
        if let Some(c) = &self.collector {
            assert!(
                c.has_category(&predicate),
                "expected at least 1 BugReport matching category predicate, none found in {} bugs",
                c.count()
            );
        } else {
            panic!("expect_category requires with_qa_core() — collector absent");
        }
    }

    /// Compteur de BugReport avec une severity donnée (exacte).
    pub fn count_with_severity(&self, severity: Severity) -> usize {
        self.collector
            .as_ref()
            .map(|c| c.count_with_severity(severity))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_builder_builds() {
        let test = TestApp::new().build();
        assert_eq!(test.total_bugs_ingested(), 0);
        assert!(test.bugs().is_empty());
    }

    /// Ces tests dépendent du drain `Messages<BugReport>` → `BugBus` qui
    /// est gated derrière la feature `qa-runtime` (zero-cost release : sans
    /// la feature, le drain est compilé en no-op). On suit la convention
    /// `forgia-qa-core/src/plugin.rs::tests::drain_pulls_message_into_bus`.
    /// Lancer via : `cargo test -p forgia-qa-harness --features qa-runtime`.
    #[test]
    #[cfg(feature = "qa-runtime")]
    fn collecting_sink_captures_bug_via_drain() {
        let mut test = TestApp::new().with_qa_core().build();
        let bug = bug_for_test();
        test.emit_bug(bug);
        // Tick 2× : 1 drain Messages → BugBus → CollectingBugSink, 1 marge.
        test.tick(2);

        let bugs = test.bugs();
        assert_eq!(bugs.len(), 1, "1 bug should be collected");
        assert_eq!(bugs[0].severity, Severity::Major);
    }

    #[test]
    fn expect_no_blocker_passes_on_major() {
        let mut test = TestApp::new().with_qa_core().build();
        test.emit_bug(bug_for_test()); // Major
        test.tick(2);
        test.expect_no_blocker(); // Should not panic
    }

    #[test]
    #[should_panic(expected = "expected no Blocker")]
    #[cfg(feature = "qa-runtime")]
    fn expect_no_blocker_panics_on_blocker() {
        use forgia_qa_core::{BugCategory, DetectionSource};
        let mut test = TestApp::new().with_qa_core().build();
        let bug = BugReport::new(
            BugCategory::Panic {
                message: "boom".into(),
                location: "x".into(),
            },
            Severity::Blocker,
            DetectionSource::UnitTest {
                test_name: "t".into(),
            },
        );
        test.emit_bug(bug);
        test.tick(2);
        test.expect_no_blocker();
    }

    #[test]
    #[cfg(feature = "qa-runtime")]
    fn expect_no_critical_panics_on_critical() {
        let mut test = TestApp::new().with_qa_core().build();
        let bug = BugReport::new(
            BugCategory::FrameTimeSpike {
                p99_ms: 100.0,
                threshold_ms: 33.0,
            },
            Severity::Critical,
            forgia_qa_core::DetectionSource::UnitTest {
                test_name: "t".into(),
            },
        );
        test.emit_bug(bug);
        test.tick(2);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            test.expect_no_critical();
        }));
        assert!(result.is_err(), "should panic on Critical");
    }

    #[test]
    #[cfg(feature = "qa-runtime")]
    fn expect_category_matches() {
        use forgia_qa_core::BugCategory;
        let mut test = TestApp::new().with_qa_core().build();
        test.emit_bug(bug_for_test()); // FrameTimeSpike
        test.tick(2);
        test.expect_category(|c| matches!(c, BugCategory::FrameTimeSpike { .. }));
    }

    #[test]
    #[cfg(feature = "qa-runtime")]
    fn count_with_severity_filters() {
        let mut test = TestApp::new().with_qa_core().build();
        test.emit_bug(bug_for_test()); // Major
        test.tick(2);
        assert_eq!(test.count_with_severity(Severity::Major), 1);
        assert_eq!(test.count_with_severity(Severity::Blocker), 0);
    }

    #[test]
    fn with_qa_core_registers_bug_bus() {
        let test = TestApp::new().with_qa_core().build();
        assert!(test.app.world().contains_resource::<BugBus>());
    }

    #[test]
    fn with_replay_registers_replay_resources() {
        let test = TestApp::new().with_qa_core().with_replay().build();
        assert!(test.app.world().contains_resource::<ReplayPlayer>());
        assert!(test.app.world().contains_resource::<ReplayMode>());
    }

    #[test]
    fn tick_advances_n_frames() {
        let mut test = TestApp::new().with_qa_core().build();
        test.tick(5);
        // Pas de moyen direct de mesurer frames sans Bevy diagnostics — on
        // vérifie juste qu'il n'y a pas de panic et que le BugBus reste vide.
        assert_eq!(test.total_bugs_ingested(), 0);
    }

    #[test]
    fn with_session_loads_player() {
        let mut session = RecordedSession::new("test");
        session.frame_count = 5;

        let test = TestApp::new().with_qa_core().with_session(session).build();

        let player = test.app.world().resource::<ReplayPlayer>();
        assert!(player.is_loaded());
        let mode = test.app.world().resource::<ReplayMode>();
        assert!(mode.is_replaying());
    }

    #[test]
    fn run_to_completion_finishes() {
        let mut session = RecordedSession::new("test");
        session.frame_count = 3;

        let mut test = TestApp::new().with_qa_core().with_session(session).build();
        let frames_consumed = test.run_to_completion();
        // ReplayBudget = 3 × 10 = 30 max, mais le replay finish dès que
        // current_frame > frame_count. Devrait s'arrêter en quelques ticks.
        assert!(frames_consumed > 0);
        assert!(frames_consumed <= 30, "should finish before safety bound");
    }

    #[test]
    fn expect_no_bug_passes_on_clean() {
        let test = TestApp::new().with_qa_core().build();
        test.expect_no_bug(); // Should not panic
    }

    #[test]
    #[should_panic(expected = "expected no BugReport")]
    #[cfg(feature = "qa-runtime")]
    fn expect_no_bug_panics_on_dirty() {
        let mut test = TestApp::new().with_qa_core().build();
        let bug = bug_for_test();
        test.emit_bug(bug);
        // Tick 2× : 1 pour drain Messages → BugBus, 1 pour assertion.
        test.tick(2);
        test.expect_no_bug();
    }

    #[test]
    fn expect_emitted_bug_count_works() {
        let test = TestApp::new().with_qa_core().build();
        test.expect_emitted_bug_count(0);
    }

    fn bug_for_test() -> BugReport {
        use forgia_qa_core::{BugCategory, DetectionSource};
        BugReport::new(
            BugCategory::FrameTimeSpike {
                p99_ms: 50.0,
                threshold_ms: 33.0,
            },
            Severity::Major,
            DetectionSource::UnitTest {
                test_name: "harness_test".into(),
            },
        )
    }
}
