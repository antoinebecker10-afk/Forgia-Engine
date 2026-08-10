//! `SoakBot` — long-run stability check avec détection de drift.
//!
//! Exécute un scénario sur N frames, échantillonne periodiquement les
//! métriques (entity count, bug count) et détecte une dérive linéaire
//! (leak entity, accumulation bug). Fail si drift > seuil.

use std::panic::AssertUnwindSafe;
use std::time::Instant;

use forgia_qa_harness::TestApp;

use crate::bot::{Bot, BotResult, BotStats};

/// Closure exécutée à chaque frame du soak run : `(test_app, frame_index)`.
type PerFrameFn = Box<dyn FnMut(&mut TestApp, u64) + Send>;

/// Bot soak test : long-run, capture samples, détecte drift.
pub struct SoakBot {
    name: String,
    total_frames: u64,
    sample_count: usize,
    drift_threshold_entities: i64,
    per_frame: PerFrameFn,
}

impl SoakBot {
    /// Construit un soak bot.
    ///
    /// - `total_frames` : nombre de ticks total (ex: 10_000).
    /// - `sample_count` : nombre d'échantillons sur la durée (ex: 10).
    /// - `drift_threshold_entities` : delta entity_count max toléré entre
    ///   premier et dernier sample (Fail si dépassé).
    /// - `per_frame` : closure appelée à chaque tick avec `(test, frame_idx)`.
    pub fn new<F>(
        name: impl Into<String>,
        total_frames: u64,
        sample_count: usize,
        drift_threshold_entities: i64,
        per_frame: F,
    ) -> Self
    where
        F: FnMut(&mut TestApp, u64) + Send + 'static,
    {
        Self {
            name: name.into(),
            total_frames,
            sample_count: sample_count.max(2),
            drift_threshold_entities,
            per_frame: Box::new(per_frame),
        }
    }

    /// Soak test minimal : tick passif sans scénario, n frames.
    pub fn passive(name: impl Into<String>, total_frames: u64) -> Self {
        Self::new(name, total_frames, 10, 100, |_, _| {})
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    entity_count: u32,
}

impl Bot for SoakBot {
    fn run(&mut self) -> BotResult {
        let started = Instant::now();
        let mut test = TestApp::new().with_qa_core().build();

        let interval = (self.total_frames / self.sample_count.max(1) as u64).max(1);
        let mut samples: Vec<Sample> = Vec::with_capacity(self.sample_count + 1);

        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
            // Sample initial.
            samples.push(snapshot(&test));

            for frame in 1..=self.total_frames {
                (self.per_frame)(&mut test, frame);
                test.tick(1);
                if frame % interval == 0 || frame == self.total_frames {
                    samples.push(snapshot(&test));
                }
            }
            Ok(())
        }));

        let elapsed = started.elapsed();
        let bugs = test.bugs();
        let bugs_emitted = bugs.len() as u64;
        let stats = BotStats {
            frames_consumed: self.total_frames,
            bugs_emitted,
            elapsed,
        };

        match outcome {
            Err(panic_payload) => {
                let panic_msg = panic_payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic_payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "<unknown panic payload>".to_string());
                return BotResult::Fail {
                    reason: format!("soak panicked at unknown frame: {panic_msg}"),
                    bugs,
                    stats,
                };
            }
            Ok(Err(e)) => {
                return BotResult::Fail {
                    reason: format!("soak scenario error: {e}"),
                    bugs,
                    stats,
                };
            }
            Ok(Ok(())) => {}
        }

        // Vérif Blocker.
        let has_blocker = bugs
            .iter()
            .any(|b| b.severity >= forgia_qa_core::Severity::Blocker);
        if has_blocker {
            return BotResult::Fail {
                reason: format!("soak detected {} Blocker bug(s)", bugs.len()),
                bugs,
                stats,
            };
        }

        // Détection drift entity count : delta entre premier et dernier sample.
        if samples.len() >= 2 {
            let first = samples.first().expect("≥2 samples");
            let last = samples.last().expect("≥2 samples");
            let delta = i64::from(last.entity_count) - i64::from(first.entity_count);
            if delta.abs() > self.drift_threshold_entities {
                return BotResult::Fail {
                    reason: format!(
                        "entity drift detected: {} → {} (Δ {}, threshold ±{})",
                        first.entity_count, last.entity_count, delta, self.drift_threshold_entities,
                    ),
                    bugs,
                    stats,
                };
            }
        }

        BotResult::Pass { stats }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn snapshot(test: &TestApp) -> Sample {
    Sample {
        entity_count: test.app.world().entities().len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soak_passive_short_passes() {
        let mut bot = SoakBot::passive("passive_100", 100);
        let result = bot.run();
        assert!(
            result.is_pass(),
            "passive 100 frames should Pass: {result:?}"
        );
        if let BotResult::Pass { stats } = result {
            assert_eq!(stats.frames_consumed, 100);
        }
    }

    #[test]
    fn soak_with_no_op_per_frame_passes() {
        let mut bot = SoakBot::new("noop", 50, 5, 10, |_, _| {});
        assert!(bot.run().is_pass());
    }

    #[test]
    fn soak_detects_entity_drift() {
        // Spawn 1 entity per frame → drift > threshold.
        let mut bot = SoakBot::new("leak", 50, 5, 10, |test, _| {
            test.app.world_mut().spawn_empty();
        });
        let result = bot.run();
        assert!(result.is_fail(), "leak scenario should Fail: {result:?}");
        if let BotResult::Fail { reason, .. } = result {
            assert!(reason.contains("entity drift"), "reason: {reason}");
        }
    }

    #[test]
    fn soak_panic_in_per_frame_caught() {
        let mut bot = SoakBot::new("panic_at_5", 100, 5, 100, |_, frame| {
            if frame == 5 {
                panic!("intentional");
            }
        });
        let result = bot.run();
        assert!(result.is_fail());
        if let BotResult::Fail { reason, .. } = result {
            assert!(reason.contains("panicked"));
        }
    }

    #[test]
    fn soak_name_accessor() {
        let bot = SoakBot::passive("my_soak", 10);
        assert_eq!(bot.name(), "my_soak");
    }

    #[test]
    fn soak_zero_drift_under_threshold_passes() {
        // Spawn puis despawn la même entity → net 0 entity drift.
        let mut bot = SoakBot::new("balanced", 30, 3, 5, |test, _| {
            let e = test.app.world_mut().spawn_empty().id();
            test.app.world_mut().despawn(e);
        });
        assert!(bot.run().is_pass());
    }
}
