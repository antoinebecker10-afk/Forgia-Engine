//! Échelle de sévérité unifiée pour [`crate::BugReport`].
//!
//! Aligne `.claude/rules/audit-protocol.md` (🔴 CRITICAL / 🟠 HIGH / 🟡 MEDIUM / 🟢 LOW)
//! avec un cinquième niveau `Blocker` réservé aux violations critiques (Stability Lock,
//! crash imminent, perte de données).

use serde::{Deserialize, Serialize};

/// Échelle ordonnée. `Blocker > Critical > Major > Minor > Cosmetic`.
///
/// Le `derive(Ord)` génère l'ordre selon la position des variants dans la
/// déclaration : c'est pourquoi les variants sont listés du moins critique au
/// plus critique. La sémantique attendue par les utilisateurs (Blocker > Critical)
/// est ainsi correcte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Style, cosmétique, doc rot.
    Cosmetic,
    /// Dette technique, anti-pattern, faux positif sensor.
    Minor,
    /// Bug observable non-bloquant, leak mesuré, perte 10-30%.
    Major,
    /// Bug user-visible actif, régression perf >30%. Pas de panic.
    Critical,
    /// Crash imminent, perte de données, Stability Lock violé. Sous `qa-runtime` debug,
    /// peut déclencher un panic. En release, dégradation gracieuse + report.
    Blocker,
}

impl Severity {
    /// `true` pour `Blocker | Critical | Major`. Utilisé pour filtrage SmokeBot CI.
    pub const fn is_actionable(self) -> bool {
        matches!(self, Self::Blocker | Self::Critical | Self::Major)
    }

    /// Représentation courte pour logs et noms de fichier.
    pub const fn as_short(self) -> &'static str {
        match self {
            Self::Blocker => "blocker",
            Self::Critical => "critical",
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Cosmetic => "cosmetic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_total() {
        assert!(Severity::Blocker > Severity::Critical);
        assert!(Severity::Critical > Severity::Major);
        assert!(Severity::Major > Severity::Minor);
        assert!(Severity::Minor > Severity::Cosmetic);
    }

    #[test]
    fn actionable_set() {
        assert!(Severity::Blocker.is_actionable());
        assert!(Severity::Critical.is_actionable());
        assert!(Severity::Major.is_actionable());
        assert!(!Severity::Minor.is_actionable());
        assert!(!Severity::Cosmetic.is_actionable());
    }

    #[test]
    fn short_repr_stable() {
        assert_eq!(Severity::Blocker.as_short(), "blocker");
        assert_eq!(Severity::Cosmetic.as_short(), "cosmetic");
    }
}
