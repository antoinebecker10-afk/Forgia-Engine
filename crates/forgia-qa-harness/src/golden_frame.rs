//! Golden frame capture/diff (story-409 Vague 1).
//!
//! Permet aux développeurs de figer un snapshot déterministe de l'état
//! d'observabilité (sensors JSON, HealthAlerts, frame state) et de le
//! comparer byte-for-byte à des références versionnées dans
//! `tests/snapshots/`.
//!
//! ## Pré-requis bloquant des stories 408 / 410
//!
//! Sans ces snapshots, l'extraction de `forgia-observability` ou de
//! `forgia-input` se ferait à l'aveugle : un changement subtil de format
//! JSON ou d'ordre de sérialisation passerait inaperçu jusqu'au playtest.
//!
//! ## Quick start
//!
//! ```ignore
//! use forgia_qa_harness::golden_frame::{GoldenFrame, capture};
//!
//! #[test]
//! fn lag_detector_golden() {
//!     let mut app = bevy::app::App::new();
//!     // ... add observability framework ...
//!     let frame = GoldenFrame::new(0xDEAD_BEEF, 60);
//!     let bundle = capture(&mut app, frame);
//!     bundle.assert_matches_golden("lag_detector_60_frames");
//! }
//! ```
//!
//! ## Déterminisme exigé
//!
//! La capture **doit** être bit-identique sur 3 runs consécutifs avec le
//! même `seed`. Toute volatilité (timestamps wall-clock, PIDs, paths
//! absolus) **doit** être redactée via [`SnapshotBundle::redact_volatile`].
//! Sinon la story-409 méta-test rejette le snapshot.
//!
//! ## Scope Vague 1
//!
//! Squelette + diff runner. La capture réelle des sensors arrive en
//! Vague 2 (extension `TestApp` avec `with_observability`) et Vague 3
//! (8 sensors pilotes).

#![cfg(feature = "golden")]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use bevy::app::App;
use bevy::ecs::resource::Resource;
use bevy::ecs::world::World;
use serde::{Deserialize, Serialize};

/// Type alias : un collector lit le `World` en lecture seule et retourne
/// du JSON. `Send + Sync` exigé pour stockage dans une `Resource`.
pub type SensorCollector = Arc<dyn Fn(&World) -> serde_json::Value + Send + Sync>;

/// Resource enregistrant les collectors de sensors actifs.
///
/// Initialisée par `TestApp::with_observability()`. Les sensors s'inscrivent
/// via [`SensorRegistry::register`] avant la capture. Pendant
/// [`capture`], tous les collectors enregistrés sont appelés.
///
/// `BTreeMap` pour ordre déterministe (jamais `HashMap`).
#[derive(Resource, Default)]
pub struct SensorRegistry {
    collectors: BTreeMap<String, SensorCollector>,
}

impl SensorRegistry {
    /// Enregistre un collector. Le `name` doit matcher le nom du sensor
    /// JSON sans extension (`forgia_lag_events`, pas `.json`).
    ///
    /// Si `name` existe déjà, l'ancien collector est remplacé (idempotent).
    pub fn register<F>(&mut self, name: impl Into<String>, collector: F)
    where
        F: Fn(&World) -> serde_json::Value + Send + Sync + 'static,
    {
        self.collectors.insert(name.into(), Arc::new(collector));
    }

    /// Itère sur les noms de sensors enregistrés (ordre déterministe).
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.collectors.keys().map(String::as_str)
    }

    /// Nombre de sensors enregistrés.
    pub fn len(&self) -> usize {
        self.collectors.len()
    }

    /// `true` si aucun collector n'est enregistré.
    pub fn is_empty(&self) -> bool {
        self.collectors.is_empty()
    }
}

/// Configuration d'une capture déterministe.
///
/// Le couple `(seed, fixed_timestep, frames)` doit déterminer entièrement
/// l'output : changer un seul de ces champs invalide le snapshot golden.
#[derive(Debug, Clone)]
pub struct GoldenFrame {
    /// Seed RNG fixe — alimenté dans `Entropy<WyRand>` côté forgia-engine.
    /// Choisir un magic number lisible (`0xDEAD_BEEF`) pour la traçabilité.
    pub seed: u64,
    /// Timestep fixe pour `Time::<Fixed>`. Default `16ms` (60 FPS).
    pub fixed_timestep: Duration,
    /// Nombre de ticks à exécuter avant capture.
    pub frames: u32,
    /// Noms des sensors JSON à capturer. Vide = capture rien (smoke test).
    /// Format attendu : `"forgia_lag_events"` (sans `.json`).
    pub sensors_to_capture: Vec<String>,
}

impl GoldenFrame {
    /// Constructeur minimal. `fixed_timestep` = 16ms, `sensors_to_capture` vide.
    pub fn new(seed: u64, frames: u32) -> Self {
        Self {
            seed,
            fixed_timestep: Duration::from_millis(16),
            frames,
            sensors_to_capture: Vec::new(),
        }
    }

    /// Builder : fixe le timestep.
    #[must_use]
    pub fn with_timestep(mut self, timestep: Duration) -> Self {
        self.fixed_timestep = timestep;
        self
    }

    /// Builder : ajoute des sensors à la liste de capture.
    #[must_use]
    pub fn with_sensors<I, S>(mut self, sensors: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.sensors_to_capture
            .extend(sensors.into_iter().map(Into::into));
        self
    }
}

/// Bundle capturé : sensors, alerts, état final. Sérialisable pour `insta`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SnapshotBundle {
    /// JSON de chaque sensor demandé (clé = `forgia_<sensor>`).
    /// `BTreeMap` pour ordre déterministe (jamais `HashMap`).
    pub sensors: BTreeMap<String, serde_json::Value>,
    /// `HealthAlert` émis pendant la run, **après** redaction des timestamps.
    pub health_alerts: Vec<serde_json::Value>,
    /// État final générique : entity count, archetype count, frame counter.
    pub final_state: serde_json::Value,
}

impl SnapshotBundle {
    /// Redacte les champs volatiles : timestamps wall-clock, PIDs, paths absolus.
    ///
    /// Modifie l'état en place. Idempotent (appelable plusieurs fois).
    ///
    /// # Champs redactés (whitelist explicite)
    ///
    /// | Pattern | Remplacé par |
    /// |---|---|
    /// | `"timestamp_secs": <f64>` | `"timestamp_secs": "[float]"` |
    /// | `"timestamp_ns": <u64>` | `"timestamp_ns": "[int]"` |
    /// | `"elapsed_ns": <u64>` | `"elapsed_ns": "[int]"` |
    /// | `"elapsed_secs": <f64>` | `"elapsed_secs": "[float]"` |
    /// | `"pid": <u32>` | `"pid": "[int]"` |
    /// | `"wall_clock_unix_ms": <u64>` | `"wall_clock_unix_ms": "[int]"` |
    ///
    /// Tout autre champ doit rester **strict** : un changement de format
    /// JSON casse le snapshot et déclenche la review.
    pub fn redact_volatile(&mut self) {
        for value in self.sensors.values_mut() {
            redact_volatile_in(value);
        }
        for alert in self.health_alerts.iter_mut() {
            redact_volatile_in(alert);
        }
        redact_volatile_in(&mut self.final_state);
    }

    /// Compare le bundle au snapshot golden versionné dans
    /// `tests/snapshots/<name>.snap.json`.
    ///
    /// # Panics
    ///
    /// Panique si le snapshot diffère. Update intentionnelle :
    /// `INSTA_UPDATE=auto cargo test --features golden`, puis commit avec
    /// préfixe `golden:` dans le message (ratchet validate-commit.sh
    /// Vague 5).
    pub fn assert_matches_golden(&self, name: &str) {
        insta::assert_json_snapshot!(name, self);
    }
}

/// Capture un bundle déterministe.
///
/// # Pipeline
///
/// 1. Tick `frame.frames` fois via `app.update()`.
/// 2. Lit la [`SensorRegistry`] et appelle chaque collector qui matche
///    `frame.sensors_to_capture` (ou tous si la liste est vide).
/// 3. Redact les champs volatiles (timestamps, elapsed_ns, pid).
///
/// # Pré-requis
///
/// - L'App doit avoir été construit via `TestApp::new().with_observability()`
///   (Vague 2) **ou** avoir une `SensorRegistry` insérée manuellement.
/// - Sans registry, retourne un bundle vide (pas une erreur — utile pour
///   le smoke test Vague 1).
///
/// # Garanties
///
/// - **Déterminisme** : pour un même `seed` + `frames` + collectors
///   enregistrés, l'output est bit-identique sur 3 runs consécutifs.
///   Le méta-test [`tests::capture_is_deterministic_across_3_runs`]
///   prouve ce contrat.
/// - **Pas d'alloc** dans la boucle de tick.
/// - **Lecture seule** du World pendant la collecte (pas de mutation
///   pendant `capture()`).
pub fn capture(app: &mut App, frame: GoldenFrame) -> SnapshotBundle {
    for _ in 0..frame.frames {
        app.update();
    }

    let mut bundle = SnapshotBundle::default();
    let world = app.world();

    if let Some(registry) = world.get_resource::<SensorRegistry>() {
        // Cloner les Arc (cheap) pour libérer le borrow sur `registry`
        // avant d'appeler les collectors qui re-lisent le World.
        let snapshot: Vec<(String, SensorCollector)> = registry
            .collectors
            .iter()
            .filter(|(name, _)| {
                frame.sensors_to_capture.is_empty()
                    || frame.sensors_to_capture.iter().any(|s| s == name.as_str())
            })
            .map(|(name, collector)| (name.clone(), Arc::clone(collector)))
            .collect();

        for (name, collector) in snapshot {
            bundle.sensors.insert(name, collector(world));
        }
    } else {
        // Pas de registry : conserver la sémantique Vague 1 (réserve les
        // clés demandées avec `Null`) pour ne pas casser le smoke test.
        for sensor_name in &frame.sensors_to_capture {
            bundle
                .sensors
                .insert(sensor_name.clone(), serde_json::Value::Null);
        }
    }

    bundle.redact_volatile();
    bundle
}

/// Tick déterministe — wrapper autour de `n × app.update()`.
///
/// Vague 2 : alias direct sur `tick`. Vague 3+ peut spécialiser pour
/// fixer `Time::<Fixed>` ou seed RNG si nécessaire (les `MinimalPlugins`
/// suffisent actuellement à garantir le déterminisme tant qu'aucun
/// système non-déterministe n'est ajouté).
pub fn tick_deterministic(app: &mut App, n: u32) {
    for _ in 0..n {
        app.update();
    }
}

// ─── Internals ──────────────────────────────────────────────────────────

const REDACTED_FLOAT: &str = "[float]";
const REDACTED_INT: &str = "[int]";

/// Whitelist des champs volatiles à redacter récursivement.
const VOLATILE_FLOAT_FIELDS: &[&str] = &["timestamp_secs", "elapsed_secs"];
const VOLATILE_INT_FIELDS: &[&str] = &[
    "timestamp_ns",
    "elapsed_ns",
    "pid",
    "wall_clock_unix_ms",
];

fn redact_volatile_in(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                if VOLATILE_FLOAT_FIELDS.contains(&key.as_str()) {
                    *v = serde_json::Value::String(REDACTED_FLOAT.to_string());
                } else if VOLATILE_INT_FIELDS.contains(&key.as_str()) {
                    *v = serde_json::Value::String(REDACTED_INT.to_string());
                } else {
                    redact_volatile_in(v);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                redact_volatile_in(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_app_capture_is_default_bundle() {
        let mut app = App::new();
        let frame = GoldenFrame::new(0xDEAD_BEEF, 0);
        let bundle = capture(&mut app, frame);

        assert!(bundle.sensors.is_empty());
        assert!(bundle.health_alerts.is_empty());
        assert_eq!(bundle.final_state, serde_json::Value::Null);
    }

    #[test]
    fn redact_volatile_replaces_whitelisted_fields() {
        let mut value = serde_json::json!({
            "timestamp_secs": 123.456,
            "timestamp_ns": 9_876_543_210u64,
            "elapsed_ns": 42u64,
            "pid": 12345,
            "stable_field": "untouched",
            "nested": {
                "elapsed_secs": 0.5,
                "untouched_int": 7,
            },
            "array": [
                { "wall_clock_unix_ms": 1_000_000u64 },
                { "stable": true },
            ],
        });

        redact_volatile_in(&mut value);

        assert_eq!(value["timestamp_secs"], "[float]");
        assert_eq!(value["timestamp_ns"], "[int]");
        assert_eq!(value["elapsed_ns"], "[int]");
        assert_eq!(value["pid"], "[int]");
        assert_eq!(value["stable_field"], "untouched");
        assert_eq!(value["nested"]["elapsed_secs"], "[float]");
        assert_eq!(value["nested"]["untouched_int"], 7);
        assert_eq!(value["array"][0]["wall_clock_unix_ms"], "[int]");
        assert_eq!(value["array"][1]["stable"], true);
    }

    #[test]
    fn redact_volatile_is_idempotent() {
        let mut value = serde_json::json!({ "elapsed_ns": 100u64 });
        redact_volatile_in(&mut value);
        let after_first = value.clone();
        redact_volatile_in(&mut value);
        assert_eq!(value, after_first);
    }

    #[test]
    fn frame_builder_chains() {
        let frame = GoldenFrame::new(1, 60)
            .with_timestep(Duration::from_millis(8))
            .with_sensors(["forgia_lag_events", "forgia_health"]);

        assert_eq!(frame.seed, 1);
        assert_eq!(frame.frames, 60);
        assert_eq!(frame.fixed_timestep, Duration::from_millis(8));
        assert_eq!(frame.sensors_to_capture.len(), 2);
    }

    #[test]
    fn capture_preallocates_requested_sensor_keys() {
        let mut app = App::new();
        let frame = GoldenFrame::new(0, 0).with_sensors(["forgia_a", "forgia_b"]);
        let bundle = capture(&mut app, frame);

        assert_eq!(bundle.sensors.len(), 2);
        assert!(bundle.sensors.contains_key("forgia_a"));
        assert!(bundle.sensors.contains_key("forgia_b"));
    }

    /// Smoke test insta : un App vide produit un SnapshotBundle stable
    /// que `assert_matches_golden` accepte sans échec.
    ///
    /// Génère le snapshot de référence dans
    /// `tests/snapshots/golden_frame__tests__empty_capture_smoke.snap.json`
    /// au premier run via `INSTA_UPDATE=auto cargo test --features golden`.
    #[test]
    fn empty_capture_smoke_via_insta() {
        let mut app = App::new();
        let frame = GoldenFrame::new(0xDEAD_BEEF, 0).with_sensors(["forgia_smoke"]);
        let bundle = capture(&mut app, frame);
        bundle.assert_matches_golden("empty_capture_smoke");
    }

    /// Vague 2 : registry vide ⇒ pas de sensors collectés, juste les
    /// pré-réservations Vague 1.
    #[test]
    fn registry_can_be_inserted_and_is_empty() {
        let mut app = App::new();
        app.insert_resource(SensorRegistry::default());

        let registry = app.world().resource::<SensorRegistry>();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert_eq!(registry.names().count(), 0);
    }

    /// Vague 2 : un sensor enregistré est appelé pendant `capture()`
    /// et son output apparaît dans le bundle.
    #[test]
    fn capture_invokes_registered_sensor() {
        let mut app = App::new();
        let mut registry = SensorRegistry::default();
        registry.register("forgia_mock", |_world| {
            serde_json::json!({ "stable_value": 42 })
        });
        app.insert_resource(registry);

        let frame = GoldenFrame::new(0, 0);
        let bundle = capture(&mut app, frame);

        assert_eq!(bundle.sensors.len(), 1);
        assert_eq!(
            bundle.sensors["forgia_mock"],
            serde_json::json!({ "stable_value": 42 })
        );
    }

    /// Vague 2 : `sensors_to_capture` filtre la collecte (whitelist).
    #[test]
    fn capture_respects_sensors_whitelist() {
        let mut app = App::new();
        let mut registry = SensorRegistry::default();
        registry.register("forgia_a", |_| serde_json::json!({"a": 1}));
        registry.register("forgia_b", |_| serde_json::json!({"b": 2}));
        registry.register("forgia_c", |_| serde_json::json!({"c": 3}));
        app.insert_resource(registry);

        let frame = GoldenFrame::new(0, 0).with_sensors(["forgia_a", "forgia_c"]);
        let bundle = capture(&mut app, frame);

        assert_eq!(bundle.sensors.len(), 2);
        assert!(bundle.sensors.contains_key("forgia_a"));
        assert!(bundle.sensors.contains_key("forgia_c"));
        assert!(!bundle.sensors.contains_key("forgia_b"));
    }

    /// Vague 2 — méta-test déterminisme : 3 runs consécutifs avec la
    /// même configuration et le même seed produisent EXACTEMENT le même
    /// bundle JSON (modulo les champs redactés). Critère d'arrêt Vague 2.
    #[test]
    fn capture_is_deterministic_across_3_runs() {
        fn run() -> SnapshotBundle {
            let mut app = App::new();
            let mut registry = SensorRegistry::default();
            registry.register("forgia_mock", |_world| {
                // State purement déterministe : pas de wall-clock, pas de RNG.
                serde_json::json!({
                    "stable_value": 42,
                    "list": [1, 2, 3],
                    "elapsed_ns": 99_999u64, // sera redacté
                })
            });
            app.insert_resource(registry);
            let frame = GoldenFrame::new(0xDEAD_BEEF, 0);
            capture(&mut app, frame)
        }

        let run_1 = run();
        let run_2 = run();
        let run_3 = run();

        let json_1 = serde_json::to_string(&run_1).unwrap();
        let json_2 = serde_json::to_string(&run_2).unwrap();
        let json_3 = serde_json::to_string(&run_3).unwrap();

        assert_eq!(json_1, json_2, "run #1 vs run #2 diverge — déterminisme cassé");
        assert_eq!(json_2, json_3, "run #2 vs run #3 diverge — déterminisme cassé");
    }

    /// Vague 2 — golden snapshot via insta sur sensor mock.
    /// Prouve l'intégration end-to-end registry → capture → redact → insta.
    #[test]
    fn mock_sensor_golden_via_insta() {
        let mut app = App::new();
        let mut registry = SensorRegistry::default();
        registry.register("forgia_mock", |_| {
            serde_json::json!({
                "stable_value": 42,
                "elapsed_ns": 12345u64, // redacté
                "list": [1, 2, 3],
            })
        });
        app.insert_resource(registry);

        let frame = GoldenFrame::new(0xDEAD_BEEF, 0).with_sensors(["forgia_mock"]);
        let bundle = capture(&mut app, frame);
        bundle.assert_matches_golden("mock_sensor_capture");
    }

    /// Vague 2 — `tick_deterministic` est l'équivalent de `tick(n)` mais
    /// expose l'API publique du module golden_frame.
    #[test]
    fn tick_deterministic_advances_app() {
        let mut app = App::new();
        // 0 frame : no-op, juste vérifier l'API compile.
        tick_deterministic(&mut app, 0);
        tick_deterministic(&mut app, 3);
    }
}
