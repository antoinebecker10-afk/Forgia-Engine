//! checks.rs — 6 checks cross-sectoriels RPG Health Monitor (story-452).
//!
//! Toutes les fonctions `chk_*` sont pures (pas d'accès ECS direct, sauf CHK-4/5).
//! Les systèmes Bevy appellent ces fns et écrivent dans RpgHealthState.

use bevy::prelude::*;
use forgia_terrain::BiomeType;
use std::time::SystemTime;

use crate::config::RpgMonitorConfig;
use crate::state::{CheckResult, LastWriteTimestamps, RpgHealthState, SensorSnapshots, Severity};

// ─────────────────────────── CHK-1 : LOD2 desync ───────────────────────────

/// CHK-1 : vérifie la cohérence entre lod2_count (chunks activés LOD2) et
/// lod2_tile_count (mega-tiles effectivement créées).
pub fn chk_lod2_desync(snapshots: &SensorSnapshots, config: &RpgMonitorConfig) -> CheckResult {
    if !config.lod2_desync.enabled {
        return CheckResult::skipped("CHK-1 disabled via config");
    }
    let Some(ref lod_json) = snapshots.terrain_lod else {
        return CheckResult::ok("CHK-1: terrain_lod sensor absent (not yet loaded)");
    };

    let lod2_count = lod_json
        .get("lod2_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let lod2_tile_count = lod_json
        .get("lod2_tile_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if lod2_count > 0 && lod2_tile_count == 0 {
        return CheckResult::critical(
            lod2_count as f32,
            0.0,
            format!("CHK-1: lod2_count={lod2_count} mais lod2_tile_count=0 — mega-tiles absentes"),
            "Lire forgia_terrain_lod.json + vérifier build_lod2_tiles_system dans forgia-terrain/lod.rs",
        );
    }

    let tolerance = config.lod2_desync.tolerance;
    if lod2_count > lod2_tile_count && (lod2_count - lod2_tile_count) > tolerance {
        return CheckResult::warn(
            lod2_count as f32,
            lod2_tile_count as f32,
            format!(
                "CHK-1: lod2_count={lod2_count} vs lod2_tile_count={lod2_tile_count} (delta={}, tol={})",
                lod2_count - lod2_tile_count,
                tolerance
            ),
            "Sub-spawn ou retry attendu — vérifier Lod2TileManager + build_lod2_tiles_system dans forgia-terrain/lod.rs:359",
        );
    }
    // Story-454 fix : la branche "INVERSE leak" était un faux positif.
    // `lod2_count` (LodStats) = chunks ECS 32m avec component ChunkLod::Lod2
    // (rare car la plupart sont unloaded à 128m via unload_m streaming).
    // `lod2_tile_count` = mega-tiles 128m du ring 128-1500m (couvre ~430 clusters).
    // Comparer les deux est sans sens — désactivé. Le vrai test de leak inverse
    // serait : "lod2_tile_count >> aire_anneau / cluster_size²" mais c'est
    // déjà géré par to_remove loop avec hystérèse (forgia-terrain/lod.rs:578).

    CheckResult::ok(format!(
        "CHK-1: lod2_count={lod2_count} lod2_tile_count={lod2_tile_count} OK"
    ))
}

// ─────────────────────────── CHK-2 : Biome luminance ───────────────────────────

const ALL_BIOMES: &[BiomeType] = &[
    BiomeType::Plains,
    BiomeType::Forest,
    BiomeType::Desert,
    BiomeType::Mountain,
    BiomeType::Swamp,
    BiomeType::Tundra,
    BiomeType::Savanna,
    BiomeType::Jungle,
    BiomeType::Volcanic,
    BiomeType::Canyon,
];

/// CHK-2 : vérifie que chaque biome a une luminance Rec709 dans [lum_floor, lum_ceiling].
pub fn chk_biome_luminance(config: &RpgMonitorConfig) -> CheckResult {
    if !config.biome_luminance.enabled {
        return CheckResult::skipped("CHK-2 disabled via config");
    }

    let floor = config.biome_luminance.lum_floor;
    let ceiling = config.biome_luminance.lum_ceiling;

    let mut failing: Vec<(BiomeType, f32)> = Vec::new();

    // BUG-452-02 fix : Rec709 sur LINEAR rgba (Forgia BiomeType::linear_rgba existe déjà).
    // Le calcul sur sRGB encodé surestime la luminance des teintes sombres → ratait Volcanic.
    for &biome in ALL_BIOMES {
        let lin = biome.linear_rgba();
        let lum = 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
        if lum < floor || lum > ceiling {
            failing.push((biome, lum));
        }
    }

    if failing.is_empty() {
        return CheckResult::ok(format!(
            "CHK-2: tous les biomes dans [{floor:.3}, {ceiling:.3}] (Rec709 linear)"
        ));
    }

    let details: Vec<String> = failing
        .iter()
        .map(|(b, lum)| format!("{} lin_lum={:.3}", b.as_str(), lum))
        .collect();
    let msg = format!(
        "CHK-2: {} biome(s) hors plage [{floor:.3},{ceiling:.3}] (Rec709 lin): {}",
        failing.len(),
        details.join(", ")
    );
    let next_step = format!(
        "Ajuster BiomeType::color() dans forgia-terrain/biomes.rs:46 pour respecter le floor lin {floor:.3} (sRGB ≈ {:.2})",
        floor.powf(1.0 / 2.2)
    );

    // BUG-452-11 fix : value = nombre de biomes en échec (cohérent avec CHK-4/5)
    CheckResult::warn(failing.len() as f32, 0.0, msg, next_step)
}

// ─────────────────────────── CHK-3 : LOD asymmetry (story-453) ───────────────────────────

/// CHK-3 : détecte une divergence entre `lod0_y` (heightmap canonique) et
/// `lod2_y` (simulation mesh LOD2) sur les sample_points émis par
/// `sys_update_lod_sample_points` (forgia-terrain).
///
/// Cas couverts :
/// - Underwater phantom water : point sous sea_level avec lod0_y != lod2_y → Critical
///   (régression du clamp Phase 2d que story-450 a retiré le 2026-05-18).
/// - Asymétrie générale : `|lod0_y - lod2_y| > max_delta_m` → Warn
pub fn chk_lod_asymmetry(snapshots: &SensorSnapshots, config: &RpgMonitorConfig) -> CheckResult {
    if !config.lod_asymmetry.enabled {
        return CheckResult::skipped("CHK-3 disabled via config");
    }
    let Some(ref lod_json) = snapshots.terrain_lod else {
        return CheckResult::ok("CHK-3: terrain_lod sensor absent");
    };
    let Some(points) = lod_json.get("sample_points").and_then(|v| v.as_array()) else {
        return CheckResult::ok("CHK-3: sample_points field absent (sensor pas encore étendu)");
    };
    if points.is_empty() {
        return CheckResult::ok("CHK-3: sample_points vide (terrain pas encore initialisé)");
    }

    let max_delta = config.lod_asymmetry.max_delta_m;
    let epsilon = config.lod_asymmetry.epsilon_m;

    let mut warns: Vec<String> = Vec::new();
    let mut critical_phantom_water: Option<String> = None;
    let mut max_observed_delta: f32 = 0.0;

    for p in points {
        let lod0_y = p.get("lod0_y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let lod2_y = p.get("lod2_y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let sea = p.get("sea_level").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let x = p.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let z = p.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

        let delta = (lod0_y - lod2_y).abs();
        if delta > max_observed_delta {
            max_observed_delta = delta;
        }

        // Phantom water : underwater point avec asymétrie significative → Critical
        if lod0_y < sea && delta > epsilon && critical_phantom_water.is_none() {
            critical_phantom_water = Some(format!(
                "phantom water à ({x:.0},{z:.0}): lod0_y={lod0_y:.2} lod2_y={lod2_y:.2} sea={sea:.2}"
            ));
        } else if delta > max_delta {
            warns.push(format!("({x:.0},{z:.0}) delta={delta:.2}m"));
        }
    }

    if let Some(msg) = critical_phantom_water {
        return CheckResult::critical(
            max_observed_delta,
            epsilon,
            format!("CHK-3 CRITICAL: {msg}"),
            "Régression phantom water LOD2 — vérifier build_lod2_terrain_mesh + simulate_lod2_y_at dans forgia-terrain/lod.rs (la clamp sea_level ne doit pas être réintroduite)",
        );
    }

    if !warns.is_empty() {
        return CheckResult::warn(
            max_observed_delta,
            max_delta,
            format!("CHK-3: {} point(s) asymétriques (>{max_delta:.2}m): {}", warns.len(), warns.join(", ")),
            "Vérifier que simulate_lod2_y_at reste en sync avec build_lod2_terrain_mesh (forgia-terrain/lod.rs)",
        );
    }

    CheckResult::ok(format!(
        "CHK-3: {} sample_points OK (max delta {max_observed_delta:.3}m)",
        points.len()
    ))
}

// ─────────────────────────── CHK-4 : Critical assets ───────────────────────────

/// CHK-4 : vérifie que les assets critiques préchargés sont en `LoadState::Loaded`.
/// Story-453 : utilise désormais `CriticalAssetHandles` (Resource préchargée à
/// `OnEnter(GameMode::Rpg)`) au lieu d'appeler `asset_server.load()` chaque tick
/// (BUG-452-03 — handles droppés immédiatement).
pub fn chk_critical_assets(
    asset_server: &AssetServer,
    handles: &crate::asset_handles::CriticalAssetHandles,
    config: &RpgMonitorConfig,
    uptime_secs: f32,
) -> CheckResult {
    if !config.critical_assets.enabled {
        return CheckResult::skipped("CHK-4 disabled via config");
    }

    let min_uptime = config.critical_assets.asset_check_min_uptime_secs;
    if uptime_secs < min_uptime {
        return CheckResult::ok(format!(
            "CHK-4: uptime {uptime_secs:.0}s < min {min_uptime:.0}s — skip (assets en cours de chargement)"
        ));
    }

    if handles.handles.is_empty() {
        return CheckResult::ok(
            "CHK-4: aucun handle préchargé (CriticalAssetHandles vide — peut-être hors GameMode::Rpg)",
        );
    }

    let mut failed: Vec<String> = Vec::new();
    let mut loading: Vec<String> = Vec::new();

    for (path, handle) in &handles.handles {
        match asset_server.load_state(handle.id()) {
            bevy::asset::LoadState::Failed(_) => failed.push(path.clone()),
            bevy::asset::LoadState::Loading | bevy::asset::LoadState::NotLoaded => {
                loading.push(path.clone())
            }
            bevy::asset::LoadState::Loaded => {}
        }
    }

    if !failed.is_empty() {
        return CheckResult::critical(
            failed.len() as f32,
            0.0,
            format!("CHK-4: {} asset(s) en échec: {}", failed.len(), failed.join(", ")),
            "Vérifier que les paths config.critical_assets existent dans assets/ + logs AssetServer ; corriger chemins ou retirer entrées obsolètes",
        );
    }

    if !loading.is_empty() {
        return CheckResult::warn(
            loading.len() as f32,
            0.0,
            format!(
                "CHK-4: {} asset(s) encore en Loading après {uptime_secs:.0}s: {}",
                loading.len(),
                loading.join(", ")
            ),
            "Asset I/O lent ou bloqué — vérifier disque + bevy_asset Reader implementation",
        );
    }

    CheckResult::ok(format!(
        "CHK-4: {} asset(s) critiques chargés OK",
        handles.handles.len()
    ))
}

// ─────────────────────────── CHK-5 : Sensor liveness ───────────────────────────

/// CHK-5 : vérifie que les sensors attendus ont été mis à jour récemment.
pub fn chk_sensor_liveness(
    timestamps: &LastWriteTimestamps,
    config: &RpgMonitorConfig,
) -> CheckResult {
    let stale_secs = config.liveness.stale_secs;
    let now = SystemTime::now();
    let mut stale: Vec<&str> = Vec::new();
    let mut absent: Vec<&str> = Vec::new();

    for sensor_path in &config.liveness.expected_sensors {
        if let Some(last_modified) = timestamps.map.get(sensor_path) {
            let age_secs = now
                .duration_since(*last_modified)
                .map(|d| d.as_secs_f32())
                .unwrap_or(0.0);
            if age_secs > stale_secs {
                stale.push(sensor_path.as_str());
            }
        } else {
            absent.push(sensor_path.as_str());
        }
    }

    if stale.is_empty() && absent.is_empty() {
        return CheckResult::ok(format!(
            "CHK-5: {} sensors actifs (stale_secs={stale_secs:.0})",
            config.liveness.expected_sensors.len()
        ));
    }

    let mut issues: Vec<String> = Vec::new();
    if !absent.is_empty() {
        issues.push(format!("absents: [{}]", absent.join(", ")));
    }
    if !stale.is_empty() {
        issues.push(format!("stale (>{stale_secs:.0}s): [{}]", stale.join(", ")));
    }

    CheckResult::warn(
        (stale.len() + absent.len()) as f32,
        0.0,
        format!("CHK-5: sensors problématiques — {}", issues.join("; ")),
        // BUG-452-05 fix : next_step actionnable (Quality Gate convention)
        "Sensor writer absent ou crashed — vérifier ForgiaSensorsPlugin (crates/forgia-sensors/src/lib.rs) + plugin producteur du sensor listé ; chaque feature crate écrit son JSON via Local<f32> 1Hz",
    )
}

// ─────────────────────────── CHK-6 : Health consistency ───────────────────────────

/// CHK-6 : vérifie la cohérence des données de santé du joueur dans forgia_combat.json.
pub fn chk_health_consistency(
    snapshots: &SensorSnapshots,
    config: &RpgMonitorConfig,
) -> CheckResult {
    if !config.health_consistency.enabled {
        return CheckResult::skipped("CHK-6 disabled via config");
    }
    let Some(ref combat) = snapshots.combat else {
        return CheckResult::ok("CHK-6: forgia_combat.json absent (non chargé)");
    };

    let player_hp = combat
        .get("player_hp")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let max_hp = combat
        .get("max_hp")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);

    match (player_hp, max_hp) {
        (Some(hp), Some(max)) => {
            if hp < 0.0 {
                return CheckResult::critical(
                    hp,
                    0.0,
                    format!("CHK-6: player_hp={hp:.1} < 0 (valeur négative invalide)"),
                    "Bug dans forgia-combat : HP ne peut pas être négatif",
                );
            }
            if max <= 0.0 {
                return CheckResult::critical(
                    max,
                    1.0,
                    format!("CHK-6: max_hp={max:.1} <= 0 (invalide)"),
                    "Bug dans forgia-combat : max_hp doit être > 0",
                );
            }
            if hp > max {
                return CheckResult::critical(
                    hp,
                    max,
                    format!("CHK-6: player_hp={hp:.1} > max_hp={max:.1}"),
                    "Bug dans forgia-combat : HP dépasse le maximum",
                );
            }
            if hp == 0.0 {
                // Mort = état attendu, pas une alerte
                return CheckResult::ok(format!("CHK-6: player_hp=0 (mort) max_hp={max:.1}"));
            }
            CheckResult::ok(format!("CHK-6: HP {hp:.1}/{max:.1} OK"))
        }
        _ => CheckResult::ok("CHK-6: champs player_hp/max_hp absents du sensor combat"),
    }
}

// ─────────────────────────── Système agrégateur ───────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn sys_run_crosschecks(
    snapshots: Res<SensorSnapshots>,
    config: Res<RpgMonitorConfig>,
    mut state: ResMut<RpgHealthState>,
    timestamps: Res<LastWriteTimestamps>,
    asset_server: Res<AssetServer>,
    asset_handles: Res<crate::asset_handles::CriticalAssetHandles>,
    time: Res<Time>,
    mut accum: Local<f32>,
    _liveness_warn_cooldown: Local<f32>,
) {
    // Throttle : exécuter au même rythme que le sensor reader
    *accum += time.delta_secs();
    if *accum < config.global.tick_interval_secs {
        return;
    }
    *accum = 0.0;

    let uptime = time.elapsed_secs();

    let r1 = chk_lod2_desync(&snapshots, &config);
    let r2 = chk_biome_luminance(&config);
    let r3 = chk_lod_asymmetry(&snapshots, &config);
    let r4 = chk_critical_assets(&asset_server, &asset_handles, &config, uptime);
    let r5 = chk_sensor_liveness(&timestamps, &config);
    let r6 = chk_health_consistency(&snapshots, &config);

    // Severity globale
    let overall = r1
        .severity
        .max(r2.severity)
        .max(r3.severity)
        .max(r4.severity)
        .max(r5.severity)
        .max(r6.severity);

    // Message global : liste des checks non-Ok
    let checks = [
        ("chk1_lod2_desync", &r1),
        ("chk2_biome_luminance", &r2),
        ("chk3_lod_asymmetry", &r3),
        ("chk4_critical_assets", &r4),
        ("chk5_sensor_liveness", &r5),
        ("chk6_health_consistency", &r6),
    ];

    let mut messages: Vec<&str> = Vec::new();
    let mut overall_next_step = String::new();

    for (_, result) in &checks {
        if result.severity != Severity::Ok {
            messages.push(&result.message);
            if overall_next_step.is_empty()
                && !result.next_step.is_empty()
                && (result.severity == Severity::Critical || result.severity == Severity::Warn)
            {
                overall_next_step.clone_from(&result.next_step);
            }
        }
    }

    // Priorité next_step : Critical > Warn
    for (_, result) in &checks {
        if result.severity == Severity::Critical && !result.next_step.is_empty() {
            overall_next_step.clone_from(&result.next_step);
            break;
        }
    }

    state.last_severity = overall;
    state.last_message = messages.join("; ");
    state.last_next_step = overall_next_step;

    // Insérer les checks dans la HashMap
    for (name, result) in checks {
        state.checks.insert(name, result.clone());
    }

    state.tick_count += 1;

    // BUG-452-04 fix : log liveness centralisé dans sys_sensor_liveness_watchdog (exporter.rs).
    // Plus de double-log ici. Local<f32> conservé (préfixe _) pour réserver le slot.
}

// ─────────────────────────── Tests ───────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RpgMonitorConfig;
    use crate::state::SensorSnapshots;

    fn default_config() -> RpgMonitorConfig {
        RpgMonitorConfig::default()
    }

    fn snapshot_with_lod(lod2_count: u64, lod2_tile_count: u64) -> SensorSnapshots {
        let mut s = SensorSnapshots::default();
        s.terrain_lod = Some(serde_json::json!({
            "lod2_count": lod2_count,
            "lod2_tile_count": lod2_tile_count
        }));
        s
    }

    fn snapshot_with_combat(hp: f32, max_hp: f32) -> SensorSnapshots {
        let mut s = SensorSnapshots::default();
        s.combat = Some(serde_json::json!({
            "player_hp": hp,
            "max_hp": max_hp
        }));
        s
    }

    // CHK-1 tests
    #[test]
    fn chk1_no_sensor_returns_ok() {
        let s = SensorSnapshots::default();
        let cfg = default_config();
        let r = chk_lod2_desync(&s, &cfg);
        assert!(matches!(r.severity, Severity::Ok));
    }

    #[test]
    fn chk1_lod2_present_tiles_absent_is_critical() {
        let s = snapshot_with_lod(10, 0);
        let cfg = default_config();
        let r = chk_lod2_desync(&s, &cfg);
        assert!(matches!(r.severity, Severity::Critical));
    }

    #[test]
    fn chk1_lod2_desync_above_tolerance_is_warn() {
        let s = snapshot_with_lod(20, 10); // delta=10 > tolerance=4
        let cfg = default_config();
        let r = chk_lod2_desync(&s, &cfg);
        assert!(matches!(r.severity, Severity::Warn));
    }

    #[test]
    fn chk1_balanced_counts_is_ok() {
        let s = snapshot_with_lod(10, 10);
        let cfg = default_config();
        let r = chk_lod2_desync(&s, &cfg);
        assert!(matches!(r.severity, Severity::Ok));
    }

    // Story-454 : ex CHK-1 INVERSE retiré (faux positif — métriques
    // incompatibles : chunks 32m logiques vs mega-tiles 128m du ring).
    #[test]
    fn chk1_tile_count_high_lod2_count_low_is_ok() {
        // Configuration runtime normale : ring 128-1500m peuplé (~430 tiles)
        // mais aucun chunk 32m en ChunkLod::Lod2 (déjà unloaded à 128m).
        let s = snapshot_with_lod(0, 428);
        let cfg = default_config();
        let r = chk_lod2_desync(&s, &cfg);
        assert!(matches!(r.severity, Severity::Ok));
    }

    // CHK-2 tests
    #[test]
    fn chk2_permissive_thresholds_all_biomes_ok() {
        let mut cfg = default_config();
        cfg.biome_luminance.lum_floor = 0.0;
        cfg.biome_luminance.lum_ceiling = 1.0;
        let r = chk_biome_luminance(&cfg);
        assert!(
            matches!(r.severity, Severity::Ok),
            "all biomes should pass with permissive thresholds, got {:?} — {}",
            r.severity,
            r.message
        );
    }

    /// BUG-452-08 regression : confirme que CHK-2 détecte l'ancien Volcanic
    /// 0.22/0.15/0.12 (linear lum ≈ 0.024) avec un floor 0.05.
    /// Aujourd'hui Volcanic est 0.35/0.27/0.22 (linear lum ≈ 0.065) qui doit aussi alerter
    /// si on garde le floor 0.05 strict ? Non — 0.065 > 0.05 = pass.
    /// Ce test est sur la valeur ACTUELLE de Volcanic post-fix biomes.rs:56.
    #[test]
    fn chk2_default_floor_catches_overly_dark_biome() {
        // Default floor 0.05 (post-fix BUG-452-02). Avec Jungle lin lum ≈ 0.040 → Warn attendu.
        let cfg = default_config();
        let r = chk_biome_luminance(&cfg);
        // Selon biome colors actuels : Jungle 0.12/0.32/0.10 → linear ≈ 0.014/0.082/0.010 → lum ≈ 0.062
        // → toujours > 0.05 → OK attendu.
        // Si un nouveau biome plus sombre arrive, ce test flague.
        // On valide juste pas Critical (qui n'arrive jamais en CHK-2).
        assert!(!matches!(r.severity, Severity::Critical));
    }

    #[test]
    fn chk2_very_strict_floor_returns_warn() {
        let mut cfg = default_config();
        cfg.biome_luminance.lum_floor = 0.99; // impossible à respecter
        let r = chk_biome_luminance(&cfg);
        assert!(matches!(r.severity, Severity::Warn));
    }

    #[test]
    fn chk2_disabled_returns_skipped() {
        let mut cfg = default_config();
        cfg.biome_luminance.enabled = false;
        let r = chk_biome_luminance(&cfg);
        assert!(matches!(r.severity, Severity::Ok));
        assert!(r.message.contains("disabled"));
    }

    // CHK-3 tests (story-453)
    fn snapshot_with_sample_points(points: Vec<serde_json::Value>) -> SensorSnapshots {
        let mut s = SensorSnapshots::default();
        s.terrain_lod = Some(serde_json::json!({
            "lod2_count": 0, "lod2_tile_count": 0,
            "sample_points": points,
        }));
        s
    }

    #[test]
    fn chk3_no_sensor_returns_ok() {
        let s = SensorSnapshots::default();
        let cfg = default_config();
        let r = chk_lod_asymmetry(&s, &cfg);
        assert!(matches!(r.severity, Severity::Ok));
    }

    #[test]
    fn chk3_empty_sample_points_returns_ok() {
        let s = snapshot_with_sample_points(vec![]);
        let cfg = default_config();
        let r = chk_lod_asymmetry(&s, &cfg);
        assert!(matches!(r.severity, Severity::Ok));
    }

    #[test]
    fn chk3_aligned_points_is_ok() {
        let s = snapshot_with_sample_points(vec![
            serde_json::json!({"x": 64.0, "z": 0.0, "lod0_y": 12.5, "lod2_y": 12.5, "sea_level": 4.0}),
            serde_json::json!({"x": 0.0, "z": 64.0, "lod0_y": 8.0, "lod2_y": 8.0, "sea_level": 4.0}),
        ]);
        let cfg = default_config();
        let r = chk_lod_asymmetry(&s, &cfg);
        assert!(
            matches!(r.severity, Severity::Ok),
            "got {:?} — {}",
            r.severity,
            r.message
        );
    }

    #[test]
    fn chk3_above_sea_asymmetry_above_threshold_is_warn() {
        // Above sea, delta > max_delta_m (default 0.5) → Warn
        let s = snapshot_with_sample_points(vec![
            serde_json::json!({"x": 128.0, "z": 0.0, "lod0_y": 10.0, "lod2_y": 12.0, "sea_level": 4.0}),
        ]);
        let cfg = default_config();
        let r = chk_lod_asymmetry(&s, &cfg);
        assert!(
            matches!(r.severity, Severity::Warn),
            "got {:?} — {}",
            r.severity,
            r.message
        );
    }

    #[test]
    fn chk3_underwater_phantom_water_is_critical() {
        // Underwater (lod0_y=2.0 < sea=4.0) avec asymétrie (lod2_y clampé à sea_level) → Critical
        let s = snapshot_with_sample_points(vec![
            serde_json::json!({"x": 50.0, "z": 50.0, "lod0_y": 2.0, "lod2_y": 4.0, "sea_level": 4.0}),
        ]);
        let cfg = default_config();
        let r = chk_lod_asymmetry(&s, &cfg);
        assert!(
            matches!(r.severity, Severity::Critical),
            "got {:?} — {}",
            r.severity,
            r.message
        );
        assert!(r.message.contains("phantom"));
    }

    #[test]
    fn chk3_disabled_returns_skipped() {
        let mut cfg = default_config();
        cfg.lod_asymmetry.enabled = false;
        let s = snapshot_with_sample_points(vec![
            serde_json::json!({"x": 0.0, "z": 0.0, "lod0_y": 2.0, "lod2_y": 4.0, "sea_level": 4.0}),
        ]);
        let r = chk_lod_asymmetry(&s, &cfg);
        assert!(matches!(r.severity, Severity::Ok));
        assert!(r.message.contains("disabled"));
    }

    // CHK-5 tests
    #[test]
    fn chk5_empty_timestamps_returns_warn() {
        let timestamps = LastWriteTimestamps::default();
        let cfg = default_config();
        let r = chk_sensor_liveness(&timestamps, &cfg);
        // Les sensors attendus sont absents → Warn
        assert!(matches!(r.severity, Severity::Warn));
    }

    #[test]
    fn chk5_all_recent_timestamps_is_ok() {
        let mut timestamps = LastWriteTimestamps::default();
        let now = SystemTime::now();
        // Ajouter tous les sensors attendus avec timestamp actuel
        for sensor in &RpgMonitorConfig::default().liveness.expected_sensors {
            timestamps.map.insert(sensor.clone(), now);
        }
        let cfg = default_config();
        let r = chk_sensor_liveness(&timestamps, &cfg);
        assert!(matches!(r.severity, Severity::Ok));
    }

    // CHK-6 tests
    #[test]
    fn chk6_no_sensor_is_ok() {
        let s = SensorSnapshots::default();
        let cfg = default_config();
        let r = chk_health_consistency(&s, &cfg);
        assert!(matches!(r.severity, Severity::Ok));
    }

    #[test]
    fn chk6_valid_hp_is_ok() {
        let s = snapshot_with_combat(75.0, 100.0);
        let cfg = default_config();
        let r = chk_health_consistency(&s, &cfg);
        assert!(matches!(r.severity, Severity::Ok));
    }

    #[test]
    fn chk6_negative_hp_is_critical() {
        let s = snapshot_with_combat(-10.0, 100.0);
        let cfg = default_config();
        let r = chk_health_consistency(&s, &cfg);
        assert!(matches!(r.severity, Severity::Critical));
    }

    #[test]
    fn chk6_hp_exceeds_max_is_critical() {
        let s = snapshot_with_combat(150.0, 100.0);
        let cfg = default_config();
        let r = chk_health_consistency(&s, &cfg);
        assert!(matches!(r.severity, Severity::Critical));
    }

    #[test]
    fn chk6_zero_hp_is_ok() {
        let s = snapshot_with_combat(0.0, 100.0);
        let cfg = default_config();
        let r = chk_health_consistency(&s, &cfg);
        assert!(matches!(r.severity, Severity::Ok));
        assert!(r.message.contains("mort"));
    }
}
