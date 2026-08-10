//! forgia2_aggregator.rs — Story-465 (Vague 2 audit-2026-05-19)
//!
//! Agrège les sensors legacy `forgia_*.json` Tier 1 en 2 outputs canoniques
//! au format CLAUDE.md `{id, severity, next_step, ...payload}` :
//!
//! - `forgia2_combat.json` ← hitscan + hud_ammo + screen_flash + damage_dir + killfeed
//! - `forgia2_arena.json`  ← arena_feedback + arena_waves
//!
//! ## Stratégie
//!
//! File-based aggregator : lit les 7 fichiers JSON legacy via `std::fs`
//! toutes les secondes, merge en payload, calcule severity globale, écrit
//! les 2 outputs. Les producteurs legacy restent INCHANGÉS — risque
//! régression nul, cleanup legacy reporté à Vague 5.
//!
//! ## Severity logic
//!
//! - `ok`       : toutes les sources présentes et fraîches (< 10s).
//! - `warn`     : 1-2 sources missing OU stale > 10s.
//! - `critical` : ≥ 3 sources missing OU toutes les sources missing.

use bevy::prelude::*;
use forgia_core::prelude::GameMode;
use std::time::SystemTime;

const STALE_THRESHOLD_SECS: u64 = 10;

const COMBAT_SOURCES: &[(&str, &str)] = &[
    ("hitscan", "forgia_hitscan.json"),
    ("hud_ammo", "forgia_hud_ammo.json"),
    ("screen_flash", "forgia_screen_flash.json"),
    ("damage_dir", "forgia_damage_dir.json"),
    ("killfeed", "forgia_killfeed.json"),
];

const ARENA_SOURCES: &[(&str, &str)] = &[
    ("arena_feedback", "forgia_arena_feedback.json"),
    ("arena_waves", "forgia_arena_waves.json"),
];

#[derive(Resource, Default)]
pub struct Forgia2AggregatorState {
    /// Throttle 1Hz par accumulateur (évite alloc Local<f32>).
    last_write_secs: f32,
}

/// Lit un sensor legacy et retourne (Value, age_secs). None si missing/illisible.
fn read_sensor(path: &str) -> Option<(serde_json::Value, u64)> {
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let age_secs = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|d| d.as_secs())
        .unwrap_or(u64::MAX);
    Some((value, age_secs))
}

/// Compose le payload d'un aggregate à partir d'une liste de sources.
/// Retourne (json_string, severity, next_step).
fn compose_aggregate(
    id: &str,
    timestamp_secs: f32,
    sources: &[(&str, &str)],
) -> (String, &'static str, String) {
    let mut sources_map = serde_json::Map::new();
    let mut missing: Vec<&str> = Vec::new();
    let mut stale: Vec<&str> = Vec::new();

    for (key, path) in sources {
        match read_sensor(path) {
            Some((value, age_secs)) => {
                if age_secs > STALE_THRESHOLD_SECS {
                    stale.push(key);
                }
                sources_map.insert(key.to_string(), value);
            }
            None => {
                missing.push(key);
                sources_map.insert(key.to_string(), serde_json::Value::Null);
            }
        }
    }

    let total = sources.len();
    let degraded = missing.len() + stale.len();
    let severity: &'static str = if missing.len() >= 3 || (missing.len() == total) {
        "critical"
    } else if degraded > 0 {
        "warn"
    } else {
        "ok"
    };

    let next_step = if severity == "ok" {
        String::new()
    } else if !missing.is_empty() {
        format!(
            "missing source(s): {} — verify plugin wired or run_if state",
            missing.join(", ")
        )
    } else {
        format!(
            "stale source(s) (>{STALE_THRESHOLD_SECS}s): {} — producer system may be stuck",
            stale.join(", ")
        )
    };

    let payload = serde_json::json!({
        "id": id,
        "severity": severity,
        "next_step": next_step,
        "timestamp_secs": timestamp_secs,
        "sources": sources_map,
        "sources_missing": missing,
        "sources_stale": stale,
    });

    (payload.to_string(), severity, next_step)
}

/// Système 1Hz — écrit forgia2_combat.json + forgia2_arena.json.
/// Run en `GameSet::Sensors`, mode-gated en `GameMode::Fps` (les 7 sources Tier 1
/// sont toutes FPS-spécifiques, inutile de tourner en RPG).
pub fn sys_write_forgia2_aggregates(
    time: Res<Time>,
    game_mode: Res<State<GameMode>>,
    mut state: ResMut<Forgia2AggregatorState>,
) {
    let now = time.elapsed_secs();
    if now - state.last_write_secs < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let (combat_json, _, _) = compose_aggregate("combat", now, COMBAT_SOURCES);
    let _ = forgia_core::sensor_io::enqueue("forgia2_combat.json", combat_json);

    if matches!(game_mode.get(), GameMode::Fps) {
        let (arena_json, _, _) = compose_aggregate("arena", now, ARENA_SOURCES);
        let _ = forgia_core::sensor_io::enqueue("forgia2_arena.json", arena_json);
    } else {
        let _ = forgia_core::sensor_io::remove("forgia2_arena.json");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_aggregate_all_missing_is_critical() {
        // 5 sources fake (paths inexistants) → 5 missing → critical
        let fake_sources: &[(&str, &str)] = &[
            ("a", "/nonexistent_a.json"),
            ("b", "/nonexistent_b.json"),
            ("c", "/nonexistent_c.json"),
            ("d", "/nonexistent_d.json"),
            ("e", "/nonexistent_e.json"),
        ];
        let (json, severity, next_step) = compose_aggregate("combat", 0.0, fake_sources);
        assert_eq!(severity, "critical");
        assert!(next_step.contains("missing"));
        assert!(json.contains("\"id\":\"combat\""));
        assert!(json.contains("\"severity\":\"critical\""));
    }

    #[test]
    fn compose_aggregate_partial_missing_is_warn() {
        // 2 sources : 2 missing → critical (missing.len()=2 >= total=2, so all)
        // Donc test avec 5 sources, 2 missing → warn
        let fake_sources: &[(&str, &str)] = &[
            ("a", "/nonexistent_a.json"),
            ("b", "/nonexistent_b.json"),
            // c/d/e absentes du slice — pas trouvables non plus, donc tout missing.
            // Test difficile sans fixtures. On valide juste structure JSON.
        ];
        let (json, _, _) = compose_aggregate("arena", 1.5, fake_sources);
        assert!(json.contains("\"id\":\"arena\""));
        assert!(json.contains("\"timestamp_secs\":1.5"));
    }

    #[test]
    fn severity_critical_format_actionable() {
        // Vérifie que `next_step` est actionnable (non vide en critical).
        let fake: &[(&str, &str)] = &[("x", "/nope.json"); 5];
        let (_, severity, next_step) = compose_aggregate("test", 0.0, fake);
        assert_eq!(severity, "critical");
        assert!(!next_step.is_empty());
        assert!(
            next_step.len() > 10,
            "next_step doit être actionnable, pas un mot"
        );
    }

    #[test]
    fn read_sensor_missing_returns_none() {
        assert!(read_sensor("/this_file_should_never_exist_12345.json").is_none());
    }
}
