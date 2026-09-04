//! [`MetricSample`] — échantillon numérique pour télémétrie continue.
//!
//! Pas de détecteur d'anomalie en P1 : le bus accumule, les détecteurs (z-score,
//! EWMA, threshold, growth_rate) sont implémentés en P2 dans `forgia-qa-telemetry`,
//! configurés par `config/genomes/qa_anomalies.toml`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Un échantillon numérique horodaté.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricSample {
    /// Timestamp UTC.
    pub timestamp: DateTime<Utc>,
    /// Frame moteur (0 si hors contexte ECS).
    pub frame: u64,
    /// Nom de la métrique. Convention : `<domain>.<sub>.<unit>` ex: `frame_time_ms`,
    /// `system.physics_step.duration_ms`, `entity_count.Projectile`.
    pub metric: String,
    /// Valeur observée.
    pub value: f64,
    /// Tags arbitraires (ex: `{ "biome": "Forest", "app_mode": "Play" }`).
    pub tags: HashMap<String, String>,
}

impl MetricSample {
    /// Construit un sample minimal (frame=0, tags vides).
    pub fn new(metric: impl Into<String>, value: f64) -> Self {
        Self {
            timestamp: Utc::now(),
            frame: 0,
            metric: metric.into(),
            value,
            tags: HashMap::new(),
        }
    }

    /// Builder : ajoute un tag.
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Builder : associe un numéro de frame.
    pub fn at_frame(mut self, frame: u64) -> Self {
        self.frame = frame;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_chains() {
        let s = MetricSample::new("frame_time_ms", 16.6)
            .at_frame(1234)
            .with_tag("app_mode", "Play")
            .with_tag("biome", "Forest");
        assert_eq!(s.frame, 1234);
        assert_eq!(s.tags.len(), 2);
        assert_eq!(s.tags["app_mode"], "Play");
    }

    #[test]
    fn ron_roundtrip() {
        let s = MetricSample::new("entity_count.Projectile", 42.0).at_frame(100);
        let r = ron::to_string(&s).unwrap();
        let back: MetricSample = ron::from_str(&r).unwrap();
        assert_eq!(s, back);
    }
}
