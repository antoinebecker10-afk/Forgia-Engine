//! entities_sensor.rs — Producteur `forgia2_entities.json` (1Hz, cross-mode).
//!
//! Lit :
//! - `EntityCountDiagnosticsPlugin` pour le total entities
//! - 4 Queries Marker pour le breakdown (Player, ArenaBot, NameplateRoot, TerrainChunkMarker)
//!
//! Severity heuristic :
//! - `warn`     : total > 50_000 (approche budget)
//! - `critical` : total > 100_000 (proxy leak)
//!
//! Coût mesuré : ~50-200 µs sur RPG 10k entities. Acceptable 1Hz.
//!
//! Story-467 — Vague 5 Phase 5b Session B.

use bevy::diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin};
use bevy::prelude::*;

use forgia_ai_arena_bot::ArenaBot;
use forgia_enemy_nameplate::NameplateRoot;
use forgia_player::Player;
use forgia_terrain::chunk::TerrainChunkMarker;

/// Pur — extrait pour tests headless.
pub fn severity_for_entities(total: u64) -> (&'static str, &'static str) {
    if total > 100_000 {
        (
            "critical",
            "entity total > 100k — check leaks (despawn missing, see forgia2_health)",
        )
    } else if total > 50_000 {
        (
            "warn",
            "entity total > 50k — approach budget, audit recent spawns",
        )
    } else {
        ("ok", "")
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sys_write_entities_sensor(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut accum: Local<f32>,
    q_player: Query<Entity, With<Player>>,
    q_bots: Query<Entity, With<ArenaBot>>,
    q_nameplates: Query<Entity, With<NameplateRoot>>,
    q_chunks: Query<Entity, With<TerrainChunkMarker>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let total = diagnostics
        .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .and_then(|d| d.value())
        .unwrap_or(0.0) as u64;

    let players = q_player.iter().count() as u64;
    let bots = q_bots.iter().count() as u64;
    let nameplates = q_nameplates.iter().count() as u64;
    let chunks = q_chunks.iter().count() as u64;

    let (severity, next_step) = severity_for_entities(total);

    let json = format!(
        r#"{{"id":"entities","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"total":{total},"players":{players},"arena_bots":{bots},"nameplates":{nameplates},"chunks_loaded":{chunks}}}"#,
        time.elapsed_secs(),
    );

    if let Err(e) = std::fs::write("forgia2_entities.json", &json) {
        warn!("[forgia-observability] entities sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_below_50k() {
        let (sev, next) = severity_for_entities(10_000);
        assert_eq!(sev, "ok");
        assert!(next.is_empty());
    }

    #[test]
    fn severity_warn_between_50k_100k() {
        let (sev, next) = severity_for_entities(60_000);
        assert_eq!(sev, "warn");
        assert!(next.contains("approach budget"));
    }

    #[test]
    fn severity_critical_above_100k() {
        let (sev, next) = severity_for_entities(200_000);
        assert_eq!(sev, "critical");
        assert!(next.contains("leak"));
    }

    #[test]
    fn severity_boundary_50k_is_ok() {
        assert_eq!(severity_for_entities(50_000).0, "ok");
        assert_eq!(severity_for_entities(50_001).0, "warn");
    }
}
