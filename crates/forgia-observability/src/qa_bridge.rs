//! qa_bridge.rs — Pont santé → bus QA (story-622, Phase 0.2).
//!
//! Le bus QA (`forgia-qa-core::BugBus`) était branché (`ForgiaQaCorePlugin` dans
//! forgia-game) mais **muet** : zéro producteur `BugReport` dans tout le code →
//! `total_ingested` restait à 0. Ce module le réveille avec son premier producteur
//! réel : chaque check santé (`RpgHealthState` : RGL-1/2 en Roguelite, CHK-1..6 en
//! RPG) qui **passe** en Warn/Critical émet UN `BugReport`, source
//! `DetectionSource::HealthMonitor { check_name }` — le variant prévu exactement
//! pour ce pont (migration capteur legacy → bus typé).
//!
//! ## Discipline producteur
//! - **Edge-trigger** : émission sur le *front montant* de sévérité uniquement
//!   (Ok→Warn, Ok→Critical, Warn→Critical). Pas de réémission par frame.
//! - **Catégorie à champs stables** : la signature de dédup du bus hashe le
//!   `Debug` de la catégorie ; on met donc `detail = next_step` (texte **statique**
//!   de guidage, pas le message à compteurs variables) → signature stable par
//!   check → une alerte récurrente dédupe en `occurrences++` sur la fenêtre 5 min
//!   au lieu de flooder.
//! - **Direction de dépendance saine** : on lit `RpgHealthState`, déjà peuplé par
//!   les systèmes santé de cette crate — aucune dépendance vers le gameplay.
//!
//! ## Pré-requis
//! `MessageWriter<BugReport>` exige `Messages<BugReport>` enregistré : garanti par
//! `ForgiaQaCorePlugin` (toujours ajouté dans forgia-game). Le drain runtime est
//! gated `qa-runtime` (feature `qa` de forgia-game, ON par défaut) : sans elle le
//! drain est no-op et `bus_ingested` reste 0, mais `emitted_total` compte quand
//! même les fronts (utile en test).
//!
//! ## Observabilité
//! `forgia2_qa.json` (1Hz) expose l'activité du bus : `emitted_total`,
//! `bus_ingested`, `dedup_hits`, + sévérité/next_step de la dernière émission.

use std::collections::HashMap;

use bevy::prelude::*;

use forgia_qa_core::{BugBus, BugCategory, BugReport, DetectionSource, Severity as QaSeverity};

use crate::config::RpgMonitorConfig;
use crate::state::{CheckResult, RpgHealthState, Severity as ObsSeverity};

const QA_SENSOR_PATH: &str = "forgia2_qa.json";
/// Sensor crash legacy écrit par le panic hook (src/main.rs, story-592), hors `crates/`.
const CRASH_SENSOR_PATH: &str = "forgia2_crash.json";
/// Archive après consommation : évite la réémission au boot suivant. Matché par
/// `.gitignore forgia2_*.json` ; invisible à sensor-audit (rename, pas write).
const CRASH_SENSOR_ARCHIVE: &str = "forgia2_crash.previous.json";

/// État du pont : edge-detection par check + résumé pour `forgia2_qa.json`.
#[derive(Resource, Default)]
pub struct QaBridgeState {
    /// Dernière sévérité observée par check (détection de front montant).
    seen: HashMap<&'static str, ObsSeverity>,
    /// Nombre d'émissions edge-triggered vers le bus (cumulé).
    pub emitted_total: u64,
    /// Sévérité QA (repr courte) de la dernière émission.
    pub last_severity: &'static str,
    /// `next_step` de la dernière émission.
    pub last_next_step: String,
    /// `elapsed_secs` de la dernière émission (fenêtre « récent » du sensor).
    pub last_emit_secs: f32,
}

/// Rang ordinal de la sévérité observabilité (`Severity` obs ne dérive pas `Ord`).
fn obs_rank(s: ObsSeverity) -> u8 {
    match s {
        ObsSeverity::Ok => 0,
        ObsSeverity::Warn => 1,
        ObsSeverity::Critical => 2,
    }
}

/// Front montant : émettre si la nouvelle sévérité est actionnable (> Ok) ET
/// strictement supérieure à la précédente (apparition ou escalade). Reste au même
/// niveau / redescend → pas de réémission (la dédup du bus couvre le résiduel).
fn is_rising_edge(prev: ObsSeverity, now: ObsSeverity) -> bool {
    obs_rank(now) > 0 && obs_rank(now) > obs_rank(prev)
}

/// Mappe la sévérité observabilité vers l'échelle QA (BugReport). `Ok` → rien.
/// `Warn` → `Major` (bug observable non-bloquant) ; `Critical` → `Critical`.
fn map_severity(obs: ObsSeverity) -> Option<QaSeverity> {
    match obs {
        ObsSeverity::Ok => None,
        ObsSeverity::Warn => Some(QaSeverity::Major),
        ObsSeverity::Critical => Some(QaSeverity::Critical),
    }
}

/// Repr **T0** (`ok|warn|critical`) de la sévérité observabilité, pour le champ
/// `severity` du sensor `forgia2_qa.json`. Distincte de l'échelle QA à 5 niveaux
/// (`major`/`blocker`…) qui n'est PAS valide pour le schéma sensor T0
/// (`xtask::VALID_SEVERITIES`). Le BugReport garde l'échelle QA ; le sensor T0.
fn obs_short(s: ObsSeverity) -> &'static str {
    match s {
        ObsSeverity::Ok => "ok",
        ObsSeverity::Warn => "warn",
        ObsSeverity::Critical => "critical",
    }
}

/// Purge les entrées edge-detection des checks disparus de `present` (ex: sortie
/// de mode → les clés `RGL-*` quittent `RpgHealthState.checks`). Sans ça, un check
/// resté Critical entre deux passages dans un mode ne réémettrait pas à la
/// ré-entrée (front Critical→Critical = pas de front).
fn prune_ghost_seen(
    seen: &mut HashMap<&'static str, ObsSeverity>,
    present: &HashMap<&'static str, CheckResult>,
) {
    seen.retain(|k, _| present.contains_key(k));
}

/// Système cross-mode — détecte les fronts montants des checks santé et émet un
/// `BugReport` par front. Tourne chaque frame mais n'agit que sur changement
/// (HashMap ~8 entrées, coût négligeable).
pub fn sys_qa_health_bridge(
    time: Res<Time>,
    config: Res<RpgMonitorConfig>,
    state: Res<RpgHealthState>,
    mut bridge: ResMut<QaBridgeState>,
    mut writer: MessageWriter<BugReport>,
) {
    if !config.qa_bridge.enabled {
        return;
    }
    let now = time.elapsed_secs();
    for (name, check) in &state.checks {
        let prev = bridge.seen.get(name).copied().unwrap_or(ObsSeverity::Ok);
        let cur = check.severity;
        if is_rising_edge(prev, cur) {
            if let Some(qa_sev) = map_severity(cur) {
                let report = BugReport::new(
                    BugCategory::LogicInvariantViolation {
                        tr_id: (*name).to_string(),
                        // detail = next_step (statique) → signature de dédup stable.
                        detail: check.next_step.clone(),
                    },
                    qa_sev,
                    DetectionSource::HealthMonitor {
                        check_name: (*name).to_string(),
                    },
                );
                writer.write(report);
                bridge.emitted_total += 1;
                // Sensor T0 : `warn`/`critical` (échelle obs), PAS `major` (échelle QA).
                bridge.last_severity = obs_short(cur);
                bridge.last_next_step = check.next_step.clone();
                bridge.last_emit_secs = now;
            }
        }
        bridge.seen.insert(*name, cur);
    }
    // Purge les fronts fantômes des checks disparus (changement de mode).
    prune_ghost_seen(&mut bridge.seen, &state.checks);
}

/// Sensor `forgia2_qa.json` (1Hz, cross-mode) — activité du bus QA.
pub fn sys_write_qa_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    config: Res<RpgMonitorConfig>,
    bridge: Res<QaBridgeState>,
    bus: Option<Res<BugBus>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (bus_ingested, dedup_hits) = bus.map(|b| (b.total_ingested, b.dedup_hits)).unwrap_or((0, 0));

    // Sévérité du sensor = celle de la dernière émission si « récente », sinon ok.
    let recent = bridge.emitted_total > 0
        && (time.elapsed_secs() - bridge.last_emit_secs) < config.qa_bridge.recent_secs;
    let severity = if recent { bridge.last_severity } else { "ok" };
    let next_step = if recent {
        bridge.last_next_step.as_str()
    } else {
        ""
    };

    let json = serde_json::json!({
        "id": "qa",
        "severity": severity,
        "next_step": next_step,
        "timestamp_secs": (time.elapsed_secs() * 10.0).round() / 10.0,
        "emitted_total": bridge.emitted_total,
        "bus_ingested": bus_ingested,
        "dedup_hits": dedup_hits,
    });

    if let Err(e) = forgia_core::sensor_io::enqueue(QA_SENSOR_PATH, json.to_string()) {
        warn!("[forgia-observability] qa sensor write failed: {e}");
    }
}

/// Parse le sensor crash legacy → `(message, location)`. `None` si JSON illisible
/// ou si ce n'est pas un sensor crash (`id != "crash"`). Pur/testable.
fn parse_crash_json(src: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(src).ok()?;
    if v.get("id").and_then(serde_json::Value::as_str) != Some("crash") {
        return None;
    }
    let message = v
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("panic inconnu")
        .to_string();
    let location = v
        .get("location")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("inconnue")
        .to_string();
    Some((message, location))
}

/// Startup — 2e producteur : bridge le crash de la session **précédente**
/// (`forgia2_crash.json`, écrit par le panic hook de src/main.rs qui tourne hors
/// ECS) vers le bus QA, UNE fois, puis archive le fichier (→ `.previous.json`)
/// pour ne pas réémettre au prochain boot. Le crash est la classe de bug la plus
/// précieuse : il survit au process qui meurt et remonte au démarrage suivant.
pub fn sys_crash_bridge_startup(
    config: Res<RpgMonitorConfig>,
    mut bridge: ResMut<QaBridgeState>,
    mut writer: MessageWriter<BugReport>,
) {
    if !config.qa_bridge.enabled {
        return;
    }
    let Ok(src) = std::fs::read_to_string(CRASH_SENSOR_PATH) else {
        return; // pas de crash en attente — cas nominal
    };
    let Some((message, location)) = parse_crash_json(&src) else {
        return;
    };
    writer.write(BugReport::new(
        BugCategory::Panic {
            message,
            location: location.clone(),
        },
        // Crash = échelle QA Blocker ; le sensor T0 affichera `critical`.
        QaSeverity::Blocker,
        DetectionSource::LegacySensor {
            sensor_file: CRASH_SENSOR_PATH.to_string(),
        },
    ));
    bridge.emitted_total += 1;
    bridge.last_severity = "critical";
    bridge.last_next_step =
        format!("crash session précédente @ {location} — voir {CRASH_SENSOR_ARCHIVE}");
    bridge.last_emit_secs = 0.0;
    // Consomme : archive le crash (best-effort, ne bloque jamais le boot).
    if let Err(e) = std::fs::rename(CRASH_SENSOR_PATH, CRASH_SENSOR_ARCHIVE) {
        warn!("[forgia-observability] crash archive failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rising_edge_from_ok() {
        assert!(is_rising_edge(ObsSeverity::Ok, ObsSeverity::Warn));
        assert!(is_rising_edge(ObsSeverity::Ok, ObsSeverity::Critical));
    }

    #[test]
    fn rising_edge_on_escalation() {
        assert!(is_rising_edge(ObsSeverity::Warn, ObsSeverity::Critical));
    }

    #[test]
    fn no_edge_when_steady() {
        assert!(!is_rising_edge(ObsSeverity::Warn, ObsSeverity::Warn));
        assert!(!is_rising_edge(ObsSeverity::Critical, ObsSeverity::Critical));
    }

    #[test]
    fn no_edge_on_descent() {
        // On n'émet que sur aggravation : la résolution (descente) ne réémet pas.
        assert!(!is_rising_edge(ObsSeverity::Critical, ObsSeverity::Warn));
        assert!(!is_rising_edge(ObsSeverity::Warn, ObsSeverity::Ok));
        assert!(!is_rising_edge(ObsSeverity::Critical, ObsSeverity::Ok));
    }

    #[test]
    fn ok_never_emits() {
        assert!(!is_rising_edge(ObsSeverity::Ok, ObsSeverity::Ok));
        assert_eq!(map_severity(ObsSeverity::Ok), None);
    }

    #[test]
    fn severity_mapping_warn_major_critical_critical() {
        assert_eq!(map_severity(ObsSeverity::Warn), Some(QaSeverity::Major));
        assert_eq!(map_severity(ObsSeverity::Critical), Some(QaSeverity::Critical));
    }

    #[test]
    fn sensor_severity_is_t0_compliant() {
        // Le champ `severity` du sensor DOIT rester dans le schéma T0
        // (xtask::VALID_SEVERITIES = ok|warn|critical|info), jamais `major`.
        const T0: [&str; 4] = ["ok", "warn", "critical", "info"];
        for s in [ObsSeverity::Ok, ObsSeverity::Warn, ObsSeverity::Critical] {
            assert!(T0.contains(&obs_short(s)), "{} hors T0", obs_short(s));
        }
        assert_eq!(obs_short(ObsSeverity::Warn), "warn");
        assert_eq!(obs_short(ObsSeverity::Critical), "critical");
    }

    #[test]
    fn ghost_entries_pruned_when_check_disappears() {
        // Sortie de Roguelite : RGL-1 quitte state.checks → doit quitter `seen`
        // pour qu'une ré-entrée directe en Critical réémette bien un BugReport.
        let mut seen = HashMap::new();
        seen.insert("RGL-1", ObsSeverity::Critical);
        seen.insert("CHK-1", ObsSeverity::Warn);
        let mut present: HashMap<&'static str, CheckResult> = HashMap::new();
        present.insert("CHK-1", CheckResult::ok("still here"));
        prune_ghost_seen(&mut seen, &present);
        assert!(!seen.contains_key("RGL-1"), "fantôme non purgé");
        assert!(seen.contains_key("CHK-1"), "clé présente purgée à tort");
    }

    #[test]
    fn parse_crash_extracts_message_and_location() {
        let src = r#"{"id":"crash","severity":"critical","message":"index out of bounds: len 3 index 7","location":"crates\\forgia-fps\\src\\lib.rs:612:9"}"#;
        let (m, l) = parse_crash_json(src).expect("crash valide");
        assert!(m.contains("index out of bounds"));
        assert!(l.contains("lib.rs:612"));
    }

    #[test]
    fn parse_crash_rejects_non_crash_sensor() {
        // Un autre sensor (ex: forgia2_qa) ne doit pas être pris pour un crash.
        assert!(parse_crash_json(r#"{"id":"qa","severity":"ok"}"#).is_none());
    }

    #[test]
    fn parse_crash_rejects_garbage() {
        assert!(parse_crash_json("pas du json").is_none());
        assert!(parse_crash_json("").is_none());
    }

    #[test]
    fn parse_crash_defaults_on_missing_fields() {
        let (m, l) = parse_crash_json(r#"{"id":"crash"}"#).expect("id crash suffit");
        assert_eq!(m, "panic inconnu");
        assert_eq!(l, "inconnue");
    }
}
