//! `SmokeBot` — bot CI qui run un scénario `Fn(&mut TestApp)` et asserte
//! sur le résultat.
//!
//! Pattern : closure-driven scenario + auto-detect Pass/Fail via
//! `catch_unwind` autour des panics + inspection BugBus.

use std::panic::AssertUnwindSafe;
use std::time::Instant;

use forgia_qa_core::Severity;
use forgia_qa_harness::TestApp;

use crate::bot::{Bot, BotResult, BotStats};

/// Bot smoke test : exécute un scénario closure sur TestApp, retourne
/// Pass si pas de Blocker / panic, sinon Fail.
///
/// Le scénario reçoit une `&mut TestApp` pré-construite avec
/// `with_qa_core()` activé (BugBus + CollectingBugSink wired). Le scénario
/// est libre d'ajouter `with_replay()` ou systems custom via `test.app`.
pub struct SmokeBot {
    name: String,
    scenario: Box<dyn FnMut(&mut TestApp) + Send>,
}

impl SmokeBot {
    /// Construit un bot avec un nom + un scénario closure.
    ///
    /// Le scénario est exécuté dans un `catch_unwind` : si le closure panic,
    /// le bot retourne `Fail { reason: panic_message }`.
    pub fn new<F>(name: impl Into<String>, scenario: F) -> Self
    where
        F: FnMut(&mut TestApp) + Send + 'static,
    {
        Self {
            name: name.into(),
            scenario: Box::new(scenario),
        }
    }
}

impl Bot for SmokeBot {
    fn run(&mut self) -> BotResult {
        let started = Instant::now();
        let mut test = TestApp::new().with_qa_core().build();

        // Exécute le scénario dans catch_unwind. Si panic → Fail avec message.
        let scenario_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            (self.scenario)(&mut test);
        }));

        let elapsed = started.elapsed();
        let bugs = test.bugs();
        let bugs_emitted = bugs.len() as u64;

        let stats = BotStats {
            frames_consumed: 0, // TestApp ne expose pas frame counter publiquement
            bugs_emitted,
            elapsed,
        };

        match scenario_result {
            Err(panic_payload) => {
                let panic_msg = panic_payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic_payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "<unknown panic payload>".to_string());
                BotResult::Fail {
                    reason: format!("scenario panicked: {panic_msg}"),
                    bugs,
                    stats,
                }
            }
            Ok(()) => {
                // Pas de panic. Vérif Blocker dans les bugs collectés.
                let has_blocker = bugs.iter().any(|b| b.severity >= Severity::Blocker);
                if has_blocker {
                    let blocker_count = bugs
                        .iter()
                        .filter(|b| b.severity >= Severity::Blocker)
                        .count();
                    BotResult::Fail {
                        reason: format!("{blocker_count} Blocker BugReport(s) detected"),
                        bugs,
                        stats,
                    }
                } else {
                    BotResult::Pass { stats }
                }
            }
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_qa_core::{BugCategory, BugReport, DetectionSource};

    #[test]
    fn smoke_bot_pass_on_empty_scenario() {
        let mut bot = SmokeBot::new("empty", |_test| {
            // No-op scenario : devrait Pass.
        });
        let result = bot.run();
        assert!(
            result.is_pass(),
            "empty scenario should Pass, got {result:?}"
        );
    }

    #[test]
    fn smoke_bot_pass_on_tick_no_bug() {
        let mut bot = SmokeBot::new("tick_60", |test| {
            test.tick(60);
        });
        let result = bot.run();
        assert!(result.is_pass());
    }

    #[test]
    fn smoke_bot_fail_on_panic() {
        let mut bot = SmokeBot::new("panicker", |_test| {
            panic!("intentional panic for test");
        });
        let result = bot.run();
        assert!(result.is_fail(), "panic should produce Fail");
        if let BotResult::Fail { reason, .. } = result {
            assert!(reason.contains("scenario panicked"));
            assert!(reason.contains("intentional panic"));
        }
    }

    #[test]
    fn smoke_bot_fail_on_blocker_bug() {
        let mut bot = SmokeBot::new("emit_blocker", |test| {
            let bug = BugReport::new(
                BugCategory::Panic {
                    message: "boom".into(),
                    location: "x".into(),
                },
                Severity::Blocker,
                DetectionSource::UnitTest {
                    test_name: "smoke".into(),
                },
            );
            test.emit_bug(bug);
            test.tick(2); // drain Messages → BugBus → CollectingBugSink
        });
        let result = bot.run();
        assert!(result.is_fail(), "Blocker bug should produce Fail");
        if let BotResult::Fail {
            reason,
            bugs,
            stats,
        } = result
        {
            assert!(reason.contains("Blocker"));
            assert_eq!(bugs.len(), 1);
            assert_eq!(stats.bugs_emitted, 1);
        }
    }

    #[test]
    fn smoke_bot_pass_on_major_bug_only() {
        let mut bot = SmokeBot::new("emit_major", |test| {
            let bug = BugReport::new(
                BugCategory::FrameTimeSpike {
                    p99_ms: 50.0,
                    threshold_ms: 33.0,
                },
                Severity::Major,
                DetectionSource::UnitTest {
                    test_name: "smoke".into(),
                },
            );
            test.emit_bug(bug);
            test.tick(2);
        });
        let result = bot.run();
        assert!(
            result.is_pass(),
            "Major bug should still Pass (only Blocker fails)"
        );
        if let BotResult::Pass { stats } = result {
            assert_eq!(stats.bugs_emitted, 1);
        }
    }

    #[test]
    fn smoke_bot_name_accessor() {
        let bot = SmokeBot::new("my_bot", |_| {});
        assert_eq!(bot.name(), "my_bot");
    }

    #[test]
    fn smoke_bot_stats_elapsed_positive() {
        let mut bot = SmokeBot::new("elapsed", |test| {
            test.tick(5);
        });
        let result = bot.run();
        if let BotResult::Pass { stats } = result {
            // elapsed devrait être > 0 (au moins quelques µs pour App init).
            assert!(stats.elapsed.as_nanos() > 0);
        } else {
            panic!("expected Pass");
        }
    }
}
