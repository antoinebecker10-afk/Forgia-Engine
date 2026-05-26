//! Trait `Bot` + types canoniques `BotResult` / `BotStats`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use forgia_qa_core::BugReport;

/// Trait commun pour tous les bots QA Forgia.
///
/// Implémentations : `SmokeBot` (W1), `SoakBot` (W3), futurs LLM
/// ExplorerBot (post-P6).
pub trait Bot: Send + 'static {
    /// Exécute le bot et retourne le résultat. Le bot peut allouer
    /// des Apps Bevy headless en interne.
    fn run(&mut self) -> BotResult;

    /// Nom du bot pour reporting CI (kebab-case stable).
    fn name(&self) -> &str;
}

/// Résultat d'un run de bot. 3 issues : Pass / Fail / Inconclusive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum BotResult {
    /// Le scénario a tourné sans panic ni blocker. `stats` contient les
    /// metrics agrégés.
    Pass {
        /// Stats agrégés.
        stats: BotStats,
    },
    /// Le scénario a échoué (Blocker BugReport détecté ou panic Rust).
    /// `reason` = message humain, `bugs` = liste des reports collectés.
    Fail {
        /// Message humain.
        reason: String,
        /// Liste des `BugReport` collectés via `CollectingBugSink`.
        bugs: Vec<BugReport>,
        /// Stats agrégés au moment de l'échec.
        stats: BotStats,
    },
    /// Le bot n'a pas pu conclure (timeout safety, configuration manquante).
    Inconclusive {
        /// Note humaine.
        note: String,
    },
}

impl BotResult {
    /// `true` si Pass.
    pub const fn is_pass(&self) -> bool {
        matches!(self, Self::Pass { .. })
    }

    /// `true` si Fail.
    pub const fn is_fail(&self) -> bool {
        matches!(self, Self::Fail { .. })
    }

    /// `true` si Inconclusive.
    pub const fn is_inconclusive(&self) -> bool {
        matches!(self, Self::Inconclusive { .. })
    }
}

/// Stats agrégés d'un run de bot. Sérialisable RON/JSON pour reporting CI.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BotStats {
    /// Nombre de frames consommées (= app.update() calls).
    pub frames_consumed: u64,
    /// Nombre de BugReport ingestés (toutes severities confondues).
    pub bugs_emitted: u64,
    /// Durée wall-clock du run.
    pub elapsed: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass_result() -> BotResult {
        BotResult::Pass {
            stats: BotStats {
                frames_consumed: 60,
                bugs_emitted: 0,
                elapsed: Duration::from_millis(42),
            },
        }
    }

    fn fail_result() -> BotResult {
        BotResult::Fail {
            reason: "blocker found".into(),
            bugs: Vec::new(),
            stats: BotStats::default(),
        }
    }

    #[test]
    fn is_pass_helper() {
        assert!(pass_result().is_pass());
        assert!(!pass_result().is_fail());
        assert!(!pass_result().is_inconclusive());
    }

    #[test]
    fn is_fail_helper() {
        assert!(fail_result().is_fail());
        assert!(!fail_result().is_pass());
    }

    #[test]
    fn is_inconclusive_helper() {
        let r = BotResult::Inconclusive { note: "no plugin".into() };
        assert!(r.is_inconclusive());
        assert!(!r.is_pass());
    }

    #[test]
    fn bot_stats_default_zero() {
        let s = BotStats::default();
        assert_eq!(s.frames_consumed, 0);
        assert_eq!(s.bugs_emitted, 0);
        assert_eq!(s.elapsed, Duration::ZERO);
    }

    #[test]
    fn bot_result_serde_roundtrip_pass() {
        let r = pass_result();
        let json = serde_json::to_string(&r).expect("serialize");
        let back: BotResult = serde_json::from_str(&json).expect("deserialize");
        assert!(back.is_pass());
    }

    #[test]
    fn bot_result_serde_roundtrip_fail() {
        let r = fail_result();
        let json = serde_json::to_string(&r).expect("serialize");
        assert!(json.contains("blocker found"));
        let back: BotResult = serde_json::from_str(&json).expect("deserialize");
        assert!(back.is_fail());
    }
}
