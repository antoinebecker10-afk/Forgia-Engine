//! `CollectingBugSink` — `BugSink` qui collecte les `BugReport` ingestés
//! dans un `Vec<>` partagé pour inspection par les assertions `TestApp`.
//!
//! Pattern : `Arc<Mutex<Vec<BugReport>>>` partagé entre le sink (poussé dans
//! `BugBus`) et la `TestApp` (lecture pour assertions). Permet `expect_no_blocker`
//! / `expect_no_critical` / `bugs_with_severity` etc.
//!
//! Coût : 1 `Mutex::lock()` par `ingest`. Acceptable pour usage test, jamais
//! dans hot path runtime production (le sink runtime est `FileSink` ou
//! `NullSink`).

use std::sync::{Arc, Mutex};

use forgia_qa_core::{BugCategory, BugId, BugReport, BugSink, Severity, SinkError};

/// Sink qui collecte les `BugReport` ingestés dans un Vec partagé.
///
/// Construit via `CollectingBugSink::new()` qui retourne `(Self, BugCollector)`.
/// Le sink (Self) est inséré dans `BugBus`, et le collector exposé à l'API
/// publique permet à `TestApp` de lire les bugs collectés.
#[derive(Debug)]
pub struct CollectingBugSink {
    bugs: Arc<Mutex<Vec<BugReport>>>,
}

/// Handle public sur la collection partagée. Cloneable, thread-safe.
#[derive(Debug, Clone, Default)]
pub struct BugCollector {
    bugs: Arc<Mutex<Vec<BugReport>>>,
}

impl CollectingBugSink {
    /// Construit un sink + son collector handle.
    ///
    /// Le sink est `Box<dyn BugSink>`-able et s'insère dans `BugBus` via
    /// `BugBus::with_sinks` ou `add_sink`. Le collector est conservé par
    /// `TestApp` pour les assertions.
    pub fn new() -> (Self, BugCollector) {
        let bugs: Arc<Mutex<Vec<BugReport>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Self { bugs: Arc::clone(&bugs) };
        let collector = BugCollector { bugs };
        (sink, collector)
    }
}

impl BugSink for CollectingBugSink {
    fn ingest(&mut self, report: &BugReport) -> Result<(), SinkError> {
        let mut guard = self
            .bugs
            .lock()
            .map_err(|e| SinkError::Other(format!("mutex poisoned: {e}")))?;
        guard.push(report.clone());
        Ok(())
    }

    fn increment_occurrence(&mut self, id: BugId) -> Result<(), SinkError> {
        let mut guard = self
            .bugs
            .lock()
            .map_err(|e| SinkError::Other(format!("mutex poisoned: {e}")))?;
        for bug in guard.iter_mut() {
            if bug.id == id {
                bug.occurrences = bug.occurrences.saturating_add(1);
                bug.last_seen = chrono::Utc::now();
                return Ok(());
            }
        }
        // ID inconnu — peut arriver si dédup hit avec autre sink. No-op.
        Ok(())
    }
}

impl BugCollector {
    /// Snapshot des `BugReport` collectés (clone pour éviter rétention du lock).
    pub fn snapshot(&self) -> Vec<BugReport> {
        self.bugs
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Compteur total (incluant toutes les severities).
    pub fn count(&self) -> usize {
        self.bugs.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Filtre par severity exacte.
    pub fn count_with_severity(&self, severity: Severity) -> usize {
        self.bugs
            .lock()
            .map(|g| g.iter().filter(|b| b.severity == severity).count())
            .unwrap_or(0)
    }

    /// `true` si au moins un report avec severity `>= min_severity`.
    /// `Severity::Blocker > Critical > Major > Minor > Cosmetic`.
    pub fn has_severity_at_least(&self, min_severity: Severity) -> bool {
        self.bugs
            .lock()
            .map(|g| g.iter().any(|b| b.severity >= min_severity))
            .unwrap_or(false)
    }

    /// `true` si au moins un report dont la catégorie matche le predicate.
    pub fn has_category<F>(&self, predicate: F) -> bool
    where
        F: Fn(&BugCategory) -> bool,
    {
        self.bugs
            .lock()
            .map(|g| g.iter().any(|b| predicate(&b.category)))
            .unwrap_or(false)
    }

    /// Vide la collection (utile pour tests qui réutilisent le même TestApp).
    pub fn clear(&self) {
        if let Ok(mut g) = self.bugs.lock() {
            g.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_qa_core::DetectionSource;

    fn make_bug(severity: Severity) -> BugReport {
        BugReport::new(
            BugCategory::FrameTimeSpike { p99_ms: 50.0, threshold_ms: 33.0 },
            severity,
            DetectionSource::UnitTest { test_name: "harness".into() },
        )
    }

    #[test]
    fn collect_then_snapshot() {
        let (mut sink, collector) = CollectingBugSink::new();
        sink.ingest(&make_bug(Severity::Major)).unwrap();
        sink.ingest(&make_bug(Severity::Critical)).unwrap();

        let snap = collector.snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(collector.count(), 2);
    }

    #[test]
    fn count_with_severity_filters() {
        let (mut sink, collector) = CollectingBugSink::new();
        sink.ingest(&make_bug(Severity::Major)).unwrap();
        sink.ingest(&make_bug(Severity::Major)).unwrap();
        sink.ingest(&make_bug(Severity::Blocker)).unwrap();

        assert_eq!(collector.count_with_severity(Severity::Major), 2);
        assert_eq!(collector.count_with_severity(Severity::Blocker), 1);
        assert_eq!(collector.count_with_severity(Severity::Cosmetic), 0);
    }

    #[test]
    fn has_severity_at_least_blocker() {
        let (mut sink, collector) = CollectingBugSink::new();
        sink.ingest(&make_bug(Severity::Major)).unwrap();
        assert!(!collector.has_severity_at_least(Severity::Blocker));

        sink.ingest(&make_bug(Severity::Blocker)).unwrap();
        assert!(collector.has_severity_at_least(Severity::Blocker));
        assert!(collector.has_severity_at_least(Severity::Major));
    }

    #[test]
    fn has_category_predicate() {
        let (mut sink, collector) = CollectingBugSink::new();
        sink.ingest(&make_bug(Severity::Major)).unwrap();

        assert!(collector.has_category(|c| matches!(c, BugCategory::FrameTimeSpike { .. })));
        assert!(!collector.has_category(|c| matches!(c, BugCategory::Panic { .. })));
    }

    #[test]
    fn clear_resets_collection() {
        let (mut sink, collector) = CollectingBugSink::new();
        sink.ingest(&make_bug(Severity::Major)).unwrap();
        assert_eq!(collector.count(), 1);

        collector.clear();
        assert_eq!(collector.count(), 0);
    }

    #[test]
    fn shared_arc_thread_safe_via_clone() {
        let (mut sink, collector) = CollectingBugSink::new();
        sink.ingest(&make_bug(Severity::Major)).unwrap();

        let collector_clone = collector.clone();
        // Les 2 handles partagent le même Vec via Arc.
        assert_eq!(collector.count(), 1);
        assert_eq!(collector_clone.count(), 1);
    }

    #[test]
    fn increment_occurrence_finds_existing() {
        let (mut sink, collector) = CollectingBugSink::new();
        let bug = make_bug(Severity::Major);
        let id = bug.id;
        sink.ingest(&bug).unwrap();

        sink.increment_occurrence(id).unwrap();
        sink.increment_occurrence(id).unwrap();

        let snap = collector.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].occurrences, 3); // 1 init + 2 increments
    }
}
