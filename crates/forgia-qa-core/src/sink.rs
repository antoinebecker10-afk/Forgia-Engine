//! [`BugSink`] trait + implémentations [`FileSink`], [`StdoutSink`], [`NullSink`].
//!
//! Convention paths P1 :
//! - `target/forgia_qa/inbox/<bug_id>.bug.ron` — humain-readable
//! - `target/forgia_qa/index.ron` — index dédup courant (P2+)

use std::fs;
use std::path::PathBuf;

use thiserror::Error;

use crate::bug::{BugId, BugReport};

/// Format de sérialisation supporté par les sinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerFormat {
    /// RON pretty (humans-first).
    Ron,
    // MessagePack arrive P1 W6 (rmp-serde).
}

/// Erreurs que peut retourner un sink.
#[derive(Debug, Error)]
pub enum SinkError {
    /// Erreur I/O (création dir, write fichier).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Erreur de sérialisation RON.
    #[error("ron serialization error: {0}")]
    Ron(#[from] ron::Error),
    /// Erreur générique avec message.
    #[error("sink error: {0}")]
    Other(String),
}

/// Trait de persistance pour [`BugReport`].
pub trait BugSink: Send + Sync + std::fmt::Debug {
    /// Persiste un nouveau rapport.
    fn ingest(&mut self, report: &BugReport) -> Result<(), SinkError>;
    /// Incrémente le compteur d'occurrences pour un bug existant (dédup hit).
    /// L'implémentation par défaut est un no-op — chaque sink raffine selon son
    /// modèle de stockage.
    fn increment_occurrence(&mut self, _id: BugId) -> Result<(), SinkError> {
        Ok(())
    }
    /// Force le flush buffers/disque. Appelé sur shutdown / exit.
    fn flush(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// FileSink
// ----------------------------------------------------------------------------

/// Sink fichier RON. Écrit un fichier par bug dans `<dir>/<bug_id>.bug.ron`.
#[derive(Debug)]
pub struct FileSink {
    dir: PathBuf,
    format: SerFormat,
}

impl FileSink {
    /// Construit un sink fichier. Crée le répertoire `dir` si absent.
    pub fn new(dir: impl Into<PathBuf>, format: SerFormat) -> Result<Self, SinkError> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir, format })
    }

    /// Convention par défaut : `target/forgia_qa/inbox/`.
    pub fn default_inbox() -> Result<Self, SinkError> {
        Self::new(PathBuf::from("target/forgia_qa/inbox"), SerFormat::Ron)
    }

    fn file_path_for(&self, report: &BugReport) -> PathBuf {
        let ext = match self.format {
            SerFormat::Ron => "bug.ron",
        };
        self.dir.join(format!("{}.{ext}", report.id))
    }
}

impl BugSink for FileSink {
    fn ingest(&mut self, report: &BugReport) -> Result<(), SinkError> {
        let path = self.file_path_for(report);
        let s = match self.format {
            SerFormat::Ron => {
                ron::ser::to_string_pretty(report, ron::ser::PrettyConfig::default())?
            }
        };
        fs::write(path, s)?;
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// StdoutSink
// ----------------------------------------------------------------------------

/// Sink stdout. Imprime une ligne `[QA] <severity> <category_kind> <id>` par bug.
#[derive(Debug, Default)]
pub struct StdoutSink;

impl BugSink for StdoutSink {
    fn ingest(&mut self, report: &BugReport) -> Result<(), SinkError> {
        // Catégorie via discriminant Debug (premier mot après `{ kind:`).
        let cat_short = format!("{:?}", report.category);
        let short = cat_short.split_whitespace().next().unwrap_or("?");
        println!(
            "[QA] {} {} id={} occ={}",
            report.severity.as_short(),
            short,
            report.id,
            report.occurrences
        );
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// NullSink
// ----------------------------------------------------------------------------

/// Sink no-op. Utile pour tests ou builds où la collecte est désactivée.
#[derive(Debug, Default)]
pub struct NullSink;

impl BugSink for NullSink {
    fn ingest(&mut self, _report: &BugReport) -> Result<(), SinkError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bug::BugCategory;
    use crate::severity::Severity;
    use crate::source::DetectionSource;

    fn sample() -> BugReport {
        BugReport::new(
            BugCategory::Panic {
                message: "test boom".into(),
                location: "lib.rs:1".into(),
            },
            Severity::Blocker,
            DetectionSource::UnitTest {
                test_name: "test_panic".into(),
            },
        )
    }

    #[test]
    fn null_sink_swallows() {
        let mut s = NullSink;
        let r = sample();
        s.ingest(&r).unwrap();
        s.increment_occurrence(r.id).unwrap();
        s.flush().unwrap();
    }

    #[test]
    fn stdout_sink_does_not_error() {
        let mut s = StdoutSink;
        s.ingest(&sample()).unwrap();
    }

    #[test]
    fn file_sink_writes_then_reads_back() {
        let tmp = std::env::temp_dir().join(format!("forgia_qa_test_{}", std::process::id()));
        let mut sink = FileSink::new(&tmp, SerFormat::Ron).unwrap();
        let r = sample();
        sink.ingest(&r).unwrap();
        let path = tmp.join(format!("{}.bug.ron", r.id));
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("test boom"));
        let back: BugReport = ron::from_str(&content).unwrap();
        assert_eq!(back.id, r.id);
        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
