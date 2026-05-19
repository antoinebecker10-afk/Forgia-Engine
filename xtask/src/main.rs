//! # xtask — Forgia V2 automation
//!
//! Tasks :
//! - `check-orphans` : détecte plugins définis non wirés, sensors sans producteur, fields FpsTuning jamais lus
//! - `schedule-dump` : dump Bevy schedule en .dot/.svg pour audit GameSet ordering
//! - `baseline-e1-e2` : génère asset_load_whitelist.txt baseline
//! - `verify-sensors-format` : CI gate validation 13 forgia2_*.json canoniques + format conforme

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");

    let exit_code = match cmd {
        "check-orphans" => { check_orphans(); 0 }
        "schedule-dump" => { schedule_dump(); 0 }
        "baseline-e1-e2" => { baseline_e1_e2(); 0 }
        "verify-sensors-format" => verify_sensors_format(),
        _ => { print_help(); 0 }
    };

    std::process::exit(exit_code);
}

fn print_help() {
    println!("xtask — Forgia V2 automation");
    println!();
    println!("Commands :");
    println!("  check-orphans            Detect plugins/sensors/fields orphans");
    println!("  schedule-dump            Dump Bevy schedule for GameSet audit");
    println!("  baseline-e1-e2           Regenerate asset_load_whitelist.txt baseline");
    println!("  verify-sensors-format    Validate forgia2_*.json canonical sensors");
}

fn check_orphans() {
    println!("[xtask] check-orphans — Phase 0 placeholder");
    // Phase 5 : scan workspace pour `impl Plugin for X` vs `add_plugins(X)` diff.
}

fn schedule_dump() {
    println!("[xtask] schedule-dump — Phase 0 placeholder");
    // Phase 1+ : utilise bevy_mod_debugdump si nécessaire.
}

fn baseline_e1_e2() {
    println!("[xtask] baseline-e1-e2 — Phase 0 placeholder");
    // Phase 3 : scan asset_server.load() call-sites + génère whitelist.
}

// ─────────────────────────── verify-sensors-format (Vague 5 Phase 5b/5c) ───────────────────────────

/// Liste canonique forgia2_*.json attendus à la racine workspace (cible Phase 5).
///
/// Phase 5b Session A — 2 sensors livrés (health + rpg_health renames).
/// Phase 5b Session A étape 2/3 — ajoute arena + chunks + combat (5 total).
/// Phase 5b Session B/C — ajoute perf + entities + memory + lifecycle + watchdog
///   + audio + input + sensor_health (13 total final).
///
/// Le binary tolère les sensors manquants pour ne pas bloquer CI pendant migration
/// progressive. Mode strict (`--strict` flag futur) vérifierait count == 13 exact.
const CANONICAL_SENSORS: &[&str] = &[
    // Tier 0 (Session A Étape 1, story-457+rename) — health sensors
    "forgia2_health.json",
    "forgia2_rpg_health.json",
    // Tier 1 (story-465 sensor fusion) — gameplay aggregators
    "forgia2_arena.json",
    "forgia2_combat.json",
    // Tier 1bis (Session A étape 3 — futur) :
    // "forgia2_chunks.json",
    // Tier 2 (Session B-C) :
    // "forgia2_perf.json",
    // "forgia2_lifecycle.json",
    // "forgia2_entities.json",
    // "forgia2_memory.json",
    // "forgia2_watchdog.json",
    // "forgia2_audio.json",
    // "forgia2_input.json",
    // "forgia2_sensor_health.json",
];

const VALID_SEVERITIES: &[&str] = &["ok", "warn", "critical", "info"];

/// Vérifie présence + format des sensors canoniques à la racine workspace.
/// Returns exit code (0 = success, 1 = fail).
fn verify_sensors_format() -> i32 {
    let workspace_root = Path::new(".");
    let mut errors: Vec<String> = Vec::new();
    let mut verified = 0;

    for sensor_name in CANONICAL_SENSORS {
        let path = workspace_root.join(sensor_name);
        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(val) => {
                    let Some(obj) = val.as_object() else {
                        errors.push(format!("{sensor_name}: not a JSON object"));
                        continue;
                    };
                    let mut sensor_ok = true;
                    for required in ["id", "severity", "next_step"] {
                        if !obj.contains_key(required) {
                            errors.push(format!("{sensor_name}: missing '{required}' field"));
                            sensor_ok = false;
                        }
                    }
                    if let Some(sev) = obj.get("severity").and_then(|v| v.as_str()) {
                        if !VALID_SEVERITIES.contains(&sev) {
                            errors.push(format!(
                                "{sensor_name}: invalid severity '{sev}' (expected: {VALID_SEVERITIES:?})"
                            ));
                            sensor_ok = false;
                        }
                    }
                    if sensor_ok {
                        verified += 1;
                    }
                }
                Err(e) => errors.push(format!("{sensor_name}: invalid JSON ({e})")),
            },
            Err(_) => errors.push(format!("{sensor_name}: file not found at workspace root")),
        }
    }

    if errors.is_empty() {
        println!(
            "[xtask] verify-sensors-format: OK ({verified}/{} canonical sensors validated)",
            CANONICAL_SENSORS.len()
        );
        0
    } else {
        eprintln!(
            "[xtask] verify-sensors-format: FAIL ({} errors, {verified}/{} passed)",
            errors.len(),
            CANONICAL_SENSORS.len()
        );
        for err in &errors {
            eprintln!("  - {err}");
        }
        1
    }
}
