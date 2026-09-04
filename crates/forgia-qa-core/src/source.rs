//! Source de détection d'un [`crate::BugReport`].
//!
//! Énumère d'où provient l'info — test unitaire, intégration, runtime assert,
//! sensor télémétrie, soak bot, session joueur, analyse statique, oracle differential.
//! Couvre 100% des bugs des 200 derniers commits Forgia (cf. `docs/qa/AUDIT.md` §3).

use serde::{Deserialize, Serialize};

/// Origine d'un [`crate::BugReport`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DetectionSource {
    /// Test unitaire `#[test]` qui a échoué.
    UnitTest {
        /// Nom du test (chemin module + fonction).
        test_name: String,
    },
    /// Test d'intégration `TestApp` (P5).
    IntegrationTest {
        /// Nom du test.
        test_name: String,
        /// Scénario joué (ex: "spawn_50_enemies_combat_60s").
        scenario: String,
    },
    /// `proptest` qui a trouvé un cas, avec input shrinké.
    PropertyTest {
        /// Nom de la propriété testée.
        property: String,
        /// Représentation textuelle de l'input minimal trouvé par shrinking.
        shrunk_input: String,
    },
    /// `precondition!` / `postcondition!` / `invariant!` violé en runtime.
    RuntimeAssert {
        /// Fichier source.
        file: String,
        /// Numéro de ligne.
        line: u32,
    },
    /// `HealthMonitor` legacy (bridge migration P2).
    HealthMonitor {
        /// Nom du check (ex: "VEGETATION ZERO").
        check_name: String,
    },
    /// Détecteur statistique sur métrique (z-score, EWMA, threshold, growth_rate).
    TelemetryAnomaly {
        /// Nom de la métrique (ex: "frame_time_ms").
        metric: String,
        /// Score d'anomalie (z-score ou ratio dépassement).
        score: f32,
    },
    /// SoakBot 12h (P6).
    SoakTest {
        /// Identifiant unique du run.
        run_id: String,
        /// Temps écoulé en secondes au moment de la détection.
        elapsed_s: u64,
    },
    /// Session joueur réelle (différé post-shipping freemium).
    PlayerSession {
        /// Identifiant anonymisé de session.
        session_id: String,
    },
    /// Analyse statique (clippy, miri, hardcode_debt sensor).
    StaticAnalysis {
        /// Outil utilisé.
        tool: String,
        /// Règle déclenchée.
        rule: String,
    },
    /// Test différentiel (oracle vs implémentation testée).
    DifferentialTest {
        /// Nom de l'oracle de référence.
        oracle: String,
    },
    /// Sensor JSON legacy bridgé pendant la migration P2.
    LegacySensor {
        /// Nom du fichier JSON source (ex: "forgia_textures.json").
        sensor_file: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ron_roundtrip_variants() {
        let cases = vec![
            DetectionSource::UnitTest {
                test_name: "foo::bar".into(),
            },
            DetectionSource::RuntimeAssert {
                file: "combat.rs".into(),
                line: 42,
            },
            DetectionSource::TelemetryAnomaly {
                metric: "frame_time_ms".into(),
                score: 4.2,
            },
            DetectionSource::LegacySensor {
                sensor_file: "forgia_textures.json".into(),
            },
        ];
        for case in cases {
            let s = ron::to_string(&case).expect("serialize");
            let back: DetectionSource = ron::from_str(&s).expect("deserialize");
            assert_eq!(case, back);
        }
    }
}
