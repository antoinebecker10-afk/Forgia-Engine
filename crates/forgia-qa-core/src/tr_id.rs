//! Newtype [`TrId`] : référence stable vers `docs/registry/tr-registry.yaml` v3.
//!
//! Règle invariante (cf. `tr-registry.yaml` header) : les TR-IDs sont **permanents**,
//! jamais renumérotés. Format `TR-<domain>-<NNN>` ou `TR-invariant-<surface><seq>`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Référence stable vers une entrée du `tr-registry.yaml`.
///
/// La crate `forgia-qa-core` ne valide pas que la chaîne existe dans le registre
/// (c'est le rôle de `forgia-qa-invariants` P3 + d'un xtask de validation).
/// Ici, on garantit seulement qu'elle est non vide et préfixée `TR-`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrId(String);

impl TrId {
    /// Construit un [`TrId`] depuis une chaîne. Retourne `None` si le format est
    /// invalide (vide, ou ne commence pas par `TR-`).
    pub fn new(raw: impl Into<String>) -> Option<Self> {
        let s = raw.into();
        if s.starts_with("TR-") && s.len() > 3 {
            Some(Self(s))
        } else {
            None
        }
    }

    /// Accès à la chaîne sous-jacente.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TrId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_prefix_accepted() {
        assert!(TrId::new("TR-lock-001").is_some());
        assert!(TrId::new("TR-invariant-G2").is_some());
        assert!(TrId::new("TR-budget-frame-001").is_some());
    }

    #[test]
    fn invalid_prefix_rejected() {
        assert!(TrId::new("").is_none());
        assert!(TrId::new("TR-").is_none());
        assert!(TrId::new("lock-001").is_none());
        assert!(TrId::new("LOCK-INV-1").is_none()); // ce n'est pas un TR-id, c'est un Lock
    }

    #[test]
    fn display_round_trip() {
        let id = TrId::new("TR-invariant-A6").unwrap();
        assert_eq!(format!("{id}"), "TR-invariant-A6");
        assert_eq!(id.as_str(), "TR-invariant-A6");
    }
}
