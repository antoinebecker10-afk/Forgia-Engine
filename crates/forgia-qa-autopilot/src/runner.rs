//! `BotRunner` — orchestrateur multi-bots avec rapport JSON agrégé.
//!
//! Pattern : `BotRunner::new().with_bot(bot1).with_bot(bot2).run()` retourne un
//! `RunReport` sérialisable (CI-friendly).

use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::bot::{Bot, BotResult};

/// Orchestrateur séquentiel de bots QA.
pub struct BotRunner {
    bots: Vec<Box<dyn Bot>>,
}

impl Default for BotRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl BotRunner {
    /// Crée un runner vide.
    pub fn new() -> Self {
        Self { bots: Vec::new() }
    }

    /// Ajoute un bot à la suite (ordre = ordre d'exécution).
    pub fn with_bot<B: Bot>(mut self, bot: B) -> Self {
        self.bots.push(Box::new(bot));
        self
    }

    /// Ajoute un bot déjà boxé.
    pub fn with_boxed_bot(mut self, bot: Box<dyn Bot>) -> Self {
        self.bots.push(bot);
        self
    }

    /// Nombre de bots enregistrés.
    pub fn len(&self) -> usize {
        self.bots.len()
    }

    /// `true` si aucun bot enregistré.
    pub fn is_empty(&self) -> bool {
        self.bots.is_empty()
    }

    /// Exécute tous les bots séquentiellement et retourne le rapport agrégé.
    pub fn run(mut self) -> RunReport {
        let started = Instant::now();
        let started_at = Utc::now().to_rfc3339();
        let mut entries = Vec::with_capacity(self.bots.len());

        for bot in &mut self.bots {
            let name = bot.name().to_string();
            let bot_started = Instant::now();
            let result = bot.run();
            let elapsed = bot_started.elapsed();
            entries.push(RunEntry { name, elapsed, result });
        }

        let total_elapsed = started.elapsed();
        let summary = RunSummary::from_entries(&entries);

        RunReport {
            started_at,
            total_elapsed,
            summary,
            entries,
        }
    }
}

/// Rapport agrégé d'un run multi-bots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    /// Timestamp ISO-8601 UTC de démarrage du run.
    pub started_at: String,
    /// Durée wall-clock totale (somme + overhead orchestration).
    pub total_elapsed: Duration,
    /// Comptes agrégés Pass/Fail/Inconclusive.
    pub summary: RunSummary,
    /// Résultats par bot (ordre = ordre d'exécution).
    pub entries: Vec<RunEntry>,
}

impl RunReport {
    /// `true` si tous les bots ont Pass (aucun Fail/Inconclusive).
    pub const fn all_pass(&self) -> bool {
        self.summary.fail == 0 && self.summary.inconclusive == 0
    }

    /// Exit code CI : 0 si all_pass, 1 si Fail, 2 si Inconclusive (mais pas Fail).
    pub fn exit_code(&self) -> u8 {
        if self.summary.fail > 0 {
            1
        } else if self.summary.inconclusive > 0 {
            2
        } else {
            0
        }
    }

    /// Sérialise le rapport en JSON pretty-printed.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Résumé compteurs Pass/Fail/Inconclusive.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunSummary {
    /// Bots avec issue Pass.
    pub pass: u32,
    /// Bots avec issue Fail.
    pub fail: u32,
    /// Bots avec issue Inconclusive.
    pub inconclusive: u32,
    /// Total bots exécutés.
    pub total: u32,
}

impl RunSummary {
    fn from_entries(entries: &[RunEntry]) -> Self {
        let mut s = Self::default();
        for e in entries {
            s.total += 1;
            match &e.result {
                BotResult::Pass { .. } => s.pass += 1,
                BotResult::Fail { .. } => s.fail += 1,
                BotResult::Inconclusive { .. } => s.inconclusive += 1,
            }
        }
        s
    }
}

/// Entrée individuelle dans le rapport (1 bot = 1 entrée).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEntry {
    /// Nom du bot.
    pub name: String,
    /// Durée wall-clock du run individuel.
    pub elapsed: Duration,
    /// Résultat détaillé.
    pub result: BotResult,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SmokeBot;

    #[test]
    fn empty_runner_reports_zero() {
        let report = BotRunner::new().run();
        assert_eq!(report.summary.total, 0);
        assert!(report.all_pass());
        assert_eq!(report.exit_code(), 0);
    }

    #[test]
    fn runner_aggregates_pass_results() {
        let report = BotRunner::new()
            .with_bot(SmokeBot::new("a", |_| {}))
            .with_bot(SmokeBot::new("b", |t| t.tick(2)))
            .run();
        assert_eq!(report.summary.total, 2);
        assert_eq!(report.summary.pass, 2);
        assert_eq!(report.summary.fail, 0);
        assert!(report.all_pass());
        assert_eq!(report.exit_code(), 0);
        assert_eq!(report.entries[0].name, "a");
        assert_eq!(report.entries[1].name, "b");
    }

    #[test]
    fn runner_aggregates_fail_via_panic() {
        let report = BotRunner::new()
            .with_bot(SmokeBot::new("ok", |_| {}))
            .with_bot(SmokeBot::new("panic_bot", |_| panic!("boom")))
            .run();
        assert_eq!(report.summary.total, 2);
        assert_eq!(report.summary.pass, 1);
        assert_eq!(report.summary.fail, 1);
        assert!(!report.all_pass());
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn run_report_json_roundtrip() {
        let report = BotRunner::new().with_bot(SmokeBot::new("x", |_| {})).run();
        let json = report.to_json_pretty().expect("serialize");
        assert!(json.contains("\"name\": \"x\""));
        assert!(json.contains("\"pass\": 1"));
        let back: RunReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.summary.total, 1);
    }

    #[test]
    fn runner_len_is_empty() {
        let r = BotRunner::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        let r = r.with_bot(SmokeBot::new("a", |_| {}));
        assert_eq!(r.len(), 1);
        assert!(!r.is_empty());
    }
}
