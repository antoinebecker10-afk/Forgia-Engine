//! Layout sensor — story-485 phase 5/6.
//!
//! Lit `LayoutResult` Resource + écrit `forgia2_stage_layout.json` 1Hz avec
//! métriques placement modules + severity + next_step.
//!
//! Pattern : `[[reference-pattern-genome-driven-plugin-with-sensor]]`.

use crate::layout::{count_by_kind, SIGHTLINE_MAX_M};
use crate::LayoutResult;
use bevy::prelude::*;
use forgia_level_presets::ModuleKind;
use serde::Serialize;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

const SENSOR_PATH: &str = "forgia2_stage_layout.json";
const SENSOR_WRITE_PERIOD_SEC: f64 = 1.0;

// ─── Severity / Next-step (purs, testables headless) ────────────────────────

/// Severity layout — sourcé story §5.5 :
/// - `error` si `longest_sightline_m > 40.0` OR `min_cover_spacing_m < 3.0`
///   OR `cover_low_count == 0` quand palette inclut cover_low_cluster
/// - `warn` si `longest_sightline_m > 35.0` (proche limite) OR
///   `instances_skipped > 0`
/// - `info` si layout vide (transitoire startup)
/// - `ok` sinon
pub fn severity_for_layout(
    longest_sightline_m: f32,
    min_cover_spacing_m: f32,
    cover_low_count: u32,
    palette_expects_cover_low: bool,
    instances_skipped: u32,
    modules_placed: usize,
) -> &'static str {
    if modules_placed == 0 {
        return "info";
    }
    if longest_sightline_m > SIGHTLINE_MAX_M {
        return "error";
    }
    if min_cover_spacing_m.is_finite() && min_cover_spacing_m < 3.0 {
        return "error";
    }
    if palette_expects_cover_low && cover_low_count == 0 {
        return "error";
    }
    if longest_sightline_m > 35.0 {
        return "warn";
    }
    if instances_skipped > 0 {
        return "warn";
    }
    "ok"
}

/// Next-step actionnable — toujours pointer vers TOML edit + sensor.
pub fn next_step_for_layout(
    longest_sightline_m: f32,
    min_cover_spacing_m: f32,
    cover_low_count: u32,
    palette_expects_cover_low: bool,
    instances_skipped: u32,
    modules_placed: usize,
) -> &'static str {
    if modules_placed == 0 {
        return "No modules placed yet. If stage Ready and palette non-empty, check `forgia2_stage.json` state — palette may be empty in `roguelite_stages.toml`.";
    }
    if longest_sightline_m > SIGHTLINE_MAX_M {
        return "Sight-line PlayerSpawn↔BossPad exceeds 40m (COD WW2 engagement comfort). Increase `cover_high_wall.count` in current stage palette in `assets/genomes/roguelite_stages.toml`.";
    }
    if min_cover_spacing_m.is_finite() && min_cover_spacing_m < 3.0 {
        return "Cover spacing violated (< 3.0m, Level Design Book rule). Reduce `cover_low_cluster.count` or increase `arena_extent_m` in stage def.";
    }
    if palette_expects_cover_low && cover_low_count == 0 {
        return "Palette expected cover_low but 0 placed. Check `forgia2_stage_layout.json instances_skipped` — palette may be too dense for extent.";
    }
    if longest_sightline_m > 35.0 {
        return "Sight-line approaching 40m limit. Consider adding 1 CoverHigh between PlayerSpawn and BossPad — edit `assets/genomes/roguelite_stages.toml`.";
    }
    if instances_skipped > 0 {
        return "Some module instances failed placement (dart-throw exhausted). Reduce `count` or increase `arena_extent_m`.";
    }
    "Layout healthy. Stage spatial identity active."
}

// ─── Sensor writer 1Hz ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct LayoutSensorJson<'a> {
    id: &'a str,
    severity: &'a str,
    timestamp_secs: f64,
    stage_id: &'a str,
    modules_placed: usize,
    cover_low_count: u32,
    cover_high_count: u32,
    sniper_perch_count: u32,
    melee_pit_count: u32,
    flank_route_count: u32,
    longest_sightline_m: f32,
    min_cover_spacing_m: f32,
    instances_skipped: u32,
    module_palette_used: &'a [String],
    next_step: &'a str,
}

pub fn write_layout_sensor(layout: Res<LayoutResult>, mut last_write: Local<f64>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    if now - *last_write < SENSOR_WRITE_PERIOD_SEC {
        return;
    }
    *last_write = now;

    let placed = &layout.placements;
    let cover_low_count = count_by_kind(placed, ModuleKind::CoverCluster);
    let cover_high_count = count_by_kind(placed, ModuleKind::CoverWall);
    let sniper_perch_count = count_by_kind(placed, ModuleKind::SniperPerch);
    let melee_pit_count = count_by_kind(placed, ModuleKind::MeleePit);
    let flank_route_count = count_by_kind(placed, ModuleKind::FlankRoute);

    // BUG-485-06 — Heuristique substring : on considère qu'un id de module
    // contenant "cover_low" promet d'émettre des anchors CoverCluster. C'est un
    // **contrat de nommage** (convention `cover_low_*` dans level_modules.toml),
    // pas une dérivation depuis `ModuleDef.anchor_kinds_emitted`. Refactor vers
    // une dérivation stricte coûterait passer `LevelModulesGenome` au sensor —
    // disproportionné tant que le set de modules reste petit (~4 en V7). Si la
    // palette TOML explose (>20 modules) ou si un module "cover_low_pillar_*"
    // n'émet PAS un CoverCluster, migrer vers query du genome.
    let palette_expects_cover_low = layout
        .module_palette_used
        .iter()
        .any(|s| s.contains("cover_low"));

    let min_spacing_finite = if layout.min_cover_spacing_m.is_finite() {
        layout.min_cover_spacing_m
    } else {
        -1.0 // sentinel JSON-friendly pour "< 2 CoverLow placés"
    };

    let severity = severity_for_layout(
        layout.longest_sightline_m,
        layout.min_cover_spacing_m,
        cover_low_count,
        palette_expects_cover_low,
        layout.instances_skipped,
        placed.len(),
    );
    let next_step = next_step_for_layout(
        layout.longest_sightline_m,
        layout.min_cover_spacing_m,
        cover_low_count,
        palette_expects_cover_low,
        layout.instances_skipped,
        placed.len(),
    );

    let payload = LayoutSensorJson {
        id: "stage_layout",
        severity,
        timestamp_secs: now,
        stage_id: &layout.stage_id,
        modules_placed: placed.len(),
        cover_low_count,
        cover_high_count,
        sniper_perch_count,
        melee_pit_count,
        flank_route_count,
        longest_sightline_m: layout.longest_sightline_m,
        min_cover_spacing_m: min_spacing_finite,
        instances_skipped: layout.instances_skipped,
        module_palette_used: &layout.module_palette_used,
        next_step,
    };

    if let Ok(json) = serde_json::to_string_pretty(&payload) {
        let _ = fs::write(SENSOR_PATH, json);
    }
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_empty_layout_is_info() {
        assert_eq!(
            severity_for_layout(0.0, f32::INFINITY, 0, false, 0, 0),
            "info"
        );
    }

    #[test]
    fn severity_healthy_layout_is_ok() {
        assert_eq!(severity_for_layout(25.0, 4.5, 6, true, 0, 9), "ok");
    }

    #[test]
    fn severity_sightline_over_40_is_error() {
        assert_eq!(severity_for_layout(42.0, 4.0, 6, true, 0, 9), "error");
    }

    #[test]
    fn severity_sightline_35_to_40_is_warn() {
        assert_eq!(severity_for_layout(37.5, 4.0, 6, true, 0, 9), "warn");
    }

    #[test]
    fn severity_cover_spacing_under_3_is_error() {
        assert_eq!(severity_for_layout(25.0, 2.5, 6, true, 0, 9), "error");
    }

    #[test]
    fn severity_palette_expects_cover_low_but_zero_is_error() {
        assert_eq!(
            severity_for_layout(25.0, f32::INFINITY, 0, true, 2, 5),
            "error"
        );
    }

    #[test]
    fn severity_instances_skipped_is_warn() {
        assert_eq!(severity_for_layout(25.0, 4.0, 6, true, 3, 9), "warn");
    }

    #[test]
    fn next_step_sightline_over_40_mentions_toml() {
        let s = next_step_for_layout(42.0, 4.0, 6, true, 0, 9);
        assert!(s.contains("roguelite_stages.toml"));
        assert!(s.contains("cover_high"));
    }

    #[test]
    fn next_step_spacing_violation_mentions_count_or_extent() {
        let s = next_step_for_layout(25.0, 2.5, 6, true, 0, 9);
        assert!(s.contains("count") || s.contains("extent"));
    }

    #[test]
    fn next_step_empty_layout_mentions_stage_sensor() {
        let s = next_step_for_layout(0.0, f32::INFINITY, 0, false, 0, 0);
        assert!(s.contains("forgia2_stage.json") || s.contains("palette"));
    }
}
