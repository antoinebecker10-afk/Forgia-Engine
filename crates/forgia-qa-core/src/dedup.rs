//! [`SlidingWindowDedup`] — fenêtre glissante par défaut 5 min.
//!
//! Évite la republication d'un même bug dont la signature reviendrait toutes les
//! frames (ex: PBR violation per-tick, lag spike récurrent). Le compteur d'occurrences
//! est maintenu, mais une seule entrée logique persiste sur la fenêtre.

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::bug::BugId;
use crate::signature::BugSignature;

/// Fenêtre glissante de dédup. Genome-driven en P2 via `qa_dedup_window_secs`.
#[derive(Debug)]
pub struct SlidingWindowDedup {
    window: Duration,
    entries: HashMap<BugSignature, Entry>,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    id: BugId,
    last_seen: DateTime<Utc>,
}

impl SlidingWindowDedup {
    /// Crée une fenêtre de dédup. `window_secs` doit être > 0.
    pub fn new(window_secs: u64) -> Self {
        Self {
            window: Duration::seconds(window_secs.max(1) as i64),
            entries: HashMap::new(),
        }
    }

    /// Si la signature est déjà connue dans la fenêtre, retourne le `BugId`
    /// originel. Sinon, retourne `None` (l'appelant doit alors enregistrer).
    pub fn check(&mut self, signature: BugSignature) -> Option<BugId> {
        let now = Utc::now();
        // Purge avant lookup pour ne pas servir une entrée expirée.
        self.purge(now);
        self.entries.get_mut(&signature).map(|e| {
            e.last_seen = now;
            e.id
        })
    }

    /// Enregistre une nouvelle signature. Idempotent (écrase si présent).
    pub fn record(&mut self, signature: BugSignature, id: BugId) {
        let now = Utc::now();
        self.purge(now);
        self.entries.insert(signature, Entry { id, last_seen: now });
    }

    /// Nombre d'entrées vivantes dans la fenêtre courante.
    pub fn live_count(&mut self) -> usize {
        self.purge(Utc::now());
        self.entries.len()
    }

    fn purge(&mut self, now: DateTime<Utc>) {
        let cutoff = now - self.window;
        self.entries.retain(|_, e| e.last_seen >= cutoff);
    }
}

impl Default for SlidingWindowDedup {
    /// Default 300 s (5 min).
    fn default() -> Self {
        Self::new(300)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_check_returns_none() {
        let mut d = SlidingWindowDedup::new(300);
        let sig = BugSignature(42);
        assert!(d.check(sig).is_none());
    }

    #[test]
    fn record_then_check_hits() {
        let mut d = SlidingWindowDedup::new(300);
        let sig = BugSignature(42);
        let id = BugId::new();
        d.record(sig, id);
        assert_eq!(d.check(sig), Some(id));
    }

    #[test]
    fn distinct_signatures_independent() {
        let mut d = SlidingWindowDedup::new(300);
        let id_a = BugId::new();
        let id_b = BugId::new();
        d.record(BugSignature(1), id_a);
        d.record(BugSignature(2), id_b);
        assert_eq!(d.check(BugSignature(1)), Some(id_a));
        assert_eq!(d.check(BugSignature(2)), Some(id_b));
        assert!(d.check(BugSignature(3)).is_none());
    }

    #[test]
    fn live_count_tracks_entries() {
        let mut d = SlidingWindowDedup::new(300);
        assert_eq!(d.live_count(), 0);
        d.record(BugSignature(1), BugId::new());
        d.record(BugSignature(2), BugId::new());
        assert_eq!(d.live_count(), 2);
    }

    // Note : tester l'expiration "réelle" (300s window) demanderait un trait Clock
    // injectable. Refacto P2 — pour P1, on valide juste la logique purge avec
    // une fenêtre de 0 (qui purge instantanément tout).
    #[test]
    fn zero_window_clamped_to_one_sec_min() {
        // Construit avec 0 → clamp interne à 1s, donc `record` puis `check`
        // immédiat hit (le purge cutoff est now - 1s, donc l'entrée fraîche tient).
        let mut d = SlidingWindowDedup::new(0);
        let id = BugId::new();
        d.record(BugSignature(7), id);
        assert_eq!(d.check(BugSignature(7)), Some(id));
    }
}
