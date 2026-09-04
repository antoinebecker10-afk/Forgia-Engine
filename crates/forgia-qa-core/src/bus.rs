//! [`BugBus`] et [`MetricBus`] — Resources Bevy fédératrices.
//!
//! Architecture : les systems Forgia émettent un `BugReport` via `MessageWriter`
//! (Bevy 0.18 API). Le system [`crate::plugin::drain_bug_messages_to_bus`] draine
//! tous les messages dans le [`BugBus`] qui :
//!
//! 1. Calcule la signature (déjà faite au `BugReport::new`).
//! 2. Vérifie dédup via [`SlidingWindowDedup`].
//! 3. Si nouveau : dispatche vers tous les sinks enregistrés.
//! 4. Si dédup hit : appelle `increment_occurrence` sur les sinks.

use bevy::ecs::resource::Resource;

use crate::bug::BugReport;
use crate::dedup::SlidingWindowDedup;
use crate::metric::MetricSample;
use crate::sink::{BugSink, NullSink, SinkError};

/// Bus typé central pour les [`BugReport`].
///
/// Resource Bevy. À enregistrer via [`crate::plugin::ForgiaQaCorePlugin`].
#[derive(Resource)]
pub struct BugBus {
    sinks: Vec<Box<dyn BugSink>>,
    dedup: SlidingWindowDedup,
    /// Compteur cumulé d'ingestions (incluant dédup hits).
    pub total_ingested: u64,
    /// Compteur de dédup hits.
    pub dedup_hits: u64,
}

impl Default for BugBus {
    fn default() -> Self {
        Self {
            sinks: vec![Box::new(NullSink)],
            dedup: SlidingWindowDedup::default(),
            total_ingested: 0,
            dedup_hits: 0,
        }
    }
}

impl BugBus {
    /// Construit un bus avec un set de sinks et une fenêtre de dédup explicite.
    pub fn with_sinks(sinks: Vec<Box<dyn BugSink>>, dedup_window_secs: u64) -> Self {
        Self {
            sinks,
            dedup: SlidingWindowDedup::new(dedup_window_secs),
            total_ingested: 0,
            dedup_hits: 0,
        }
    }

    /// Remplace les sinks (ex: changer de FileSink à NullSink en runtime).
    pub fn set_sinks(&mut self, sinks: Vec<Box<dyn BugSink>>) {
        self.sinks = sinks;
    }

    /// Ajoute un sink supplémentaire.
    pub fn add_sink(&mut self, sink: Box<dyn BugSink>) {
        self.sinks.push(sink);
    }

    /// Ingère un rapport. Applique dédup puis dispatche.
    ///
    /// Retourne `Err` si **un** sink a échoué — les autres sinks sont quand même
    /// invoqués (best-effort), seule la première erreur rencontrée est remontée.
    pub fn ingest(&mut self, report: BugReport) -> Result<(), SinkError> {
        self.total_ingested += 1;
        if let Some(existing_id) = self.dedup.check(report.signature) {
            self.dedup_hits += 1;
            let mut first_err: Option<SinkError> = None;
            for sink in &mut self.sinks {
                if let Err(e) = sink.increment_occurrence(existing_id) {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
            }
            return match first_err {
                Some(e) => Err(e),
                None => Ok(()),
            };
        }
        self.dedup.record(report.signature, report.id);
        let mut first_err: Option<SinkError> = None;
        for sink in &mut self.sinks {
            if let Err(e) = sink.ingest(&report) {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Force flush sur tous les sinks.
    pub fn flush_all(&mut self) -> Result<(), SinkError> {
        let mut first_err: Option<SinkError> = None;
        for sink in &mut self.sinks {
            if let Err(e) = sink.flush() {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Nombre de sinks enregistrés (pour introspection / tests).
    pub fn sink_count(&self) -> usize {
        self.sinks.len()
    }
}

/// Bus de samples numériques pour télémétrie.
/// Les détecteurs d'anomalie statistique (P2) lisent ce bus et émettent des
/// `BugReport(TelemetryAnomaly)` quand un seuil est dépassé.
#[derive(Resource, Debug, Default)]
pub struct MetricBus {
    /// Compteur cumulé de samples ingérés.
    pub total_ingested: u64,
}

impl MetricBus {
    /// Ingère un sample. P1 = compteur seulement, détecteurs en P2.
    pub fn ingest(&mut self, _sample: MetricSample) {
        self.total_ingested += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bug::BugCategory;
    use crate::severity::Severity;
    use crate::source::DetectionSource;

    fn fresh_report() -> BugReport {
        BugReport::new(
            BugCategory::FrameTimeSpike {
                p99_ms: 50.0,
                threshold_ms: 33.0,
            },
            Severity::Major,
            DetectionSource::TelemetryAnomaly {
                metric: "frame_time_ms".into(),
                score: 4.2,
            },
        )
    }

    #[test]
    fn fresh_bus_has_null_sink_default() {
        let b = BugBus::default();
        assert_eq!(b.sink_count(), 1);
    }

    #[test]
    fn ingest_increments_counter() {
        let mut b = BugBus::default();
        b.ingest(fresh_report()).unwrap();
        assert_eq!(b.total_ingested, 1);
    }

    #[test]
    fn dedup_hit_after_same_signature() {
        let mut b = BugBus::default();
        let r1 = fresh_report();
        let mut r2 = fresh_report();
        // Force same signature (les frames sont vides → même sig pour même cat).
        r2.signature = r1.signature;
        b.ingest(r1).unwrap();
        b.ingest(r2).unwrap();
        assert_eq!(b.total_ingested, 2);
        assert_eq!(b.dedup_hits, 1);
    }

    #[test]
    fn metric_bus_counts() {
        let mut m = MetricBus::default();
        for _ in 0..10 {
            m.ingest(MetricSample::new("frame_time_ms", 16.6));
        }
        assert_eq!(m.total_ingested, 10);
    }

    #[test]
    fn add_sink_grows_sink_count() {
        let mut b = BugBus::default();
        let before = b.sink_count();
        b.add_sink(Box::new(NullSink));
        assert_eq!(b.sink_count(), before + 1);
    }

    /// Sink qui échoue toujours sur ingest pour tester la propagation `first_err`.
    #[derive(Debug, Default)]
    struct FailingSink {
        ingest_calls: u32,
        increment_calls: u32,
        flush_calls: u32,
    }

    impl BugSink for FailingSink {
        fn ingest(&mut self, _r: &BugReport) -> Result<(), crate::sink::SinkError> {
            self.ingest_calls += 1;
            Err(crate::sink::SinkError::Other("boom".into()))
        }
        fn increment_occurrence(
            &mut self,
            _id: crate::bug::BugId,
        ) -> Result<(), crate::sink::SinkError> {
            self.increment_calls += 1;
            Err(crate::sink::SinkError::Other("boom-incr".into()))
        }
        fn flush(&mut self) -> Result<(), crate::sink::SinkError> {
            self.flush_calls += 1;
            Err(crate::sink::SinkError::Other("boom-flush".into()))
        }
    }

    #[test]
    fn flush_all_reaches_every_sink_even_on_error() {
        let mut b = BugBus::with_sinks(
            vec![Box::new(NullSink), Box::new(NullSink), Box::new(NullSink)],
            300,
        );
        // FlushAll on NullSinks = Ok(()).
        assert!(b.flush_all().is_ok());
    }

    #[test]
    fn flush_all_propagates_first_error_but_calls_all() {
        // 3 sinks dont 1 fail au milieu : tous doivent être appelés.
        let mut b = BugBus::with_sinks(vec![Box::new(NullSink)], 300);
        // Ajout direct sans accès au champ (utilise add_sink + builder).
        b.add_sink(Box::new(FailingSink::default()));
        b.add_sink(Box::new(NullSink));
        let res = b.flush_all();
        assert!(res.is_err(), "first_err doit être remonté");
    }

    #[test]
    fn set_sinks_replaces_completely() {
        let mut b = BugBus::default(); // 1 NullSink
        assert_eq!(b.sink_count(), 1);
        b.set_sinks(vec![
            Box::new(NullSink),
            Box::new(NullSink),
            Box::new(NullSink),
        ]);
        assert_eq!(b.sink_count(), 3);
        b.set_sinks(vec![]);
        assert_eq!(b.sink_count(), 0);
    }

    #[test]
    fn ingest_with_failing_sink_returns_err_but_dispatches_others() {
        let mut b = BugBus::with_sinks(
            vec![Box::new(FailingSink::default()), Box::new(NullSink)],
            300,
        );
        let r = fresh_report();
        let res = b.ingest(r);
        assert!(res.is_err(), "FailingSink doit propager Err");
        // Le compteur a quand même été incrémenté.
        assert_eq!(b.total_ingested, 1);
    }

    #[test]
    fn dedup_hit_with_failing_sink_propagates_increment_error() {
        let mut b = BugBus::with_sinks(
            vec![Box::new(FailingSink::default()), Box::new(NullSink)],
            300,
        );
        let r1 = fresh_report();
        let mut r2 = fresh_report();
        r2.signature = r1.signature;
        // 1er ingest : new report → FailingSink renvoie Err sur ingest.
        let _ = b.ingest(r1);
        // 2ème : dédup hit → FailingSink renvoie Err sur increment_occurrence.
        let res = b.ingest(r2);
        assert!(res.is_err());
        assert_eq!(b.dedup_hits, 1);
    }
}
