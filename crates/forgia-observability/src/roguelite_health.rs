//! roguelite_health.rs — Checks santé Roguelite (story-621, Phase 0.6 A).
//!
//! `forgia2_health.json` était aveugle en Roguelite : les 6 checks RPG (CHK-*) sont
//! gatés `GameMode::Rpg` et surveillent terrain/biome/LOD — inapplicables. Ce module
//! ajoute 2 checks propres au Roguelite, gatés `GameMode::Roguelite`, qui peuplent le
//! même `RpgHealthState` (lu cross-mode par `health_sensor::sys_write_health_sensor`).
//!
//! - **RGL-1 écran vide** : Mesh3d présents mais 0 visible (ou 0 Camera3d active) →
//!   critical. Aurait attrapé le bug « map invisible » du 2026-06-24.
//! - **RGL-2 vague figée** : 0 bot vivant en run actif (hors break) depuis >N s → warn.
//!
//! Coût : 1 query Mesh3d + 1 lecture JSON, 1Hz. AUCUNE dépendance vers
//! `forgia-mode-roguelite` (on relit le sensor JSON qu'il produit déjà).

use bevy::prelude::*;

use crate::config::RpgMonitorConfig;
use crate::state::{CheckResult, RpgHealthState, Severity};

const STATE_SENSOR_PATH: &str = "forgia2_roguelite_state.json";

/// Rang ordinal pour agréger la pire sévérité (`Severity` ne dérive pas `Ord`).
fn rank(s: Severity) -> u8 {
    match s {
        Severity::Ok => 0,
        Severity::Warn => 1,
        Severity::Critical => 2,
    }
}

/// RGL-1 — écran vide : meshes présents mais aucun visible, ou aucune Camera3d active.
pub fn chk_render_blank(
    mesh_total: u64,
    mesh_visible: u64,
    cams_3d_active: u64,
    blank_mesh_floor: u64,
) -> CheckResult {
    if cams_3d_active == 0 {
        return CheckResult::critical(
            0.0,
            1.0,
            "RGL-1: aucune Camera3d active en Roguelite — rien ne rend (skybox/clear seuls)",
            "Vérifier is_active de la FpsCamera (résidu mode 3P / lobby ?) — voir forgia2_render.json.",
        );
    }
    if mesh_total > blank_mesh_floor && mesh_visible == 0 {
        return CheckResult::critical(
            mesh_visible as f32,
            blank_mesh_floor as f32,
            format!("RGL-1: {mesh_total} Mesh3d présents mais 0 visible — ECRAN VIDE"),
            "Culling (B0004 InheritedVisibility) / passe opaque / frustum — voir forgia2_render.json.",
        );
    }
    CheckResult::ok(format!(
        "RGL-1: {mesh_visible}/{mesh_total} Mesh3d visibles"
    ))
}

/// RGL-2 — vague figée : run actif, hors break, aucun bot vivant depuis trop longtemps.
pub fn chk_stuck_wave(
    run_state: &str,
    bots_alive: u64,
    in_break: bool,
    // Fix audit 2026-07-19 — ex-`victory` : latch de fin de run (posé aussi sur
    // Defeat), renommé `run_ended` côté producteur (sensor.rs) et ici.
    run_ended: bool,
    boss_defeated: bool,
    time_in_state_secs: f32,
    stuck_wave_secs: f32,
) -> CheckResult {
    let active = run_state == "in_run" || run_state == "boss";
    if !active || in_break || run_ended || boss_defeated {
        return CheckResult::ok(format!(
            "RGL-2: pas de combat actif à surveiller ({run_state})"
        ));
    }
    if bots_alive == 0 && time_in_state_secs > stuck_wave_secs {
        return CheckResult::warn(
            time_in_state_secs,
            stuck_wave_secs,
            format!(
                "RGL-2: 0 bot vivant depuis {time_in_state_secs:.0}s en `{run_state}` (hors break) — vague figée / ennemis non spawnés"
            ),
            "Voir forgia2_worldgen (SpawnQueue) + director de vagues : spawn bloqué ?",
        );
    }
    CheckResult::ok(format!(
        "RGL-2: {bots_alive} bot(s) vivant(s) en `{run_state}`"
    ))
}

/// Lit `forgia2_roguelite_state.json` (garde de staleness via `timestamp_secs`, même
/// horloge `Time::elapsed_secs` que le producteur).
fn read_stuck_wave(elapsed_secs: f32, stuck_wave_secs: f32, state_stale_secs: f32) -> CheckResult {
    let src = match std::fs::read_to_string(STATE_SENSOR_PATH) {
        Ok(s) => s,
        Err(_) => return CheckResult::skipped("RGL-2: forgia2_roguelite_state.json absent (skip)"),
    };
    let v: serde_json::Value = match serde_json::from_str(&src) {
        Ok(v) => v,
        Err(_) => return CheckResult::skipped("RGL-2: roguelite_state json illisible (skip)"),
    };
    let ts = v
        .get("timestamp_secs")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    if elapsed_secs - ts > state_stale_secs {
        return CheckResult::skipped("RGL-2: roguelite_state stale (skip)");
    }
    let run_state = v
        .get("run_state")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let bots_alive = v
        .get("bots_alive")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let in_break = v
        .get("in_break")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let run_ended = v
        .get("run_ended")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let boss_defeated = v
        .get("boss_defeated")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let time_in_state = v
        .get("time_in_state_secs")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    chk_stuck_wave(
        run_state,
        bots_alive,
        in_break,
        run_ended,
        boss_defeated,
        time_in_state,
        stuck_wave_secs,
    )
}

/// Système 1Hz gaté `GameMode::Roguelite` — peuple `RpgHealthState` avec RGL-1/RGL-2.
#[allow(clippy::type_complexity)]
pub fn sys_roguelite_health(
    time: Res<Time>,
    mut accum: Local<f32>,
    config: Res<RpgMonitorConfig>,
    mut state: ResMut<RpgHealthState>,
    q_cams: Query<(&Camera, Has<Camera3d>)>,
    q_meshes: Query<&ViewVisibility, With<Mesh3d>>,
) {
    let cfg = &config.roguelite_health;
    if !cfg.enabled {
        return;
    }
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    // RGL-1 — écran vide (même comptage que render_sensor).
    let mut mesh_total = 0u64;
    let mut mesh_visible = 0u64;
    for vv in q_meshes.iter() {
        mesh_total += 1;
        if vv.get() {
            mesh_visible += 1;
        }
    }
    let cams_3d_active = q_cams
        .iter()
        .filter(|(cam, is_3d)| *is_3d && cam.is_active)
        .count() as u64;
    let rgl1 = chk_render_blank(
        mesh_total,
        mesh_visible,
        cams_3d_active,
        cfg.blank_mesh_floor,
    );

    // RGL-2 — vague figée (lecture du sensor roguelite, garde de staleness).
    let rgl2 = read_stuck_wave(
        time.elapsed_secs(),
        cfg.stuck_wave_secs,
        cfg.state_stale_secs,
    );

    // En Roguelite, les checks RPG (CHK-*) ne tournent pas : on possède l'état santé.
    let worst = if rank(rgl1.severity) >= rank(rgl2.severity) {
        (rgl1.severity, rgl1.next_step.clone())
    } else {
        (rgl2.severity, rgl2.next_step.clone())
    };
    state.checks.clear();
    state.checks.insert("RGL-1", rgl1);
    state.checks.insert("RGL-2", rgl2);
    state.last_severity = worst.0;
    state.last_next_step = worst.1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgl1_blank_screen_is_critical() {
        // Le bug du 2026-06-24 : beaucoup de meshes, 0 visible, caméra active.
        let r = chk_render_blank(3829, 0, 2, 50);
        assert_eq!(r.severity, Severity::Critical);
    }

    #[test]
    fn rgl1_no_active_3d_camera_is_critical() {
        let r = chk_render_blank(100, 100, 0, 50);
        assert_eq!(r.severity, Severity::Critical);
    }

    #[test]
    fn rgl1_few_meshes_zero_visible_below_floor_is_ok() {
        // Sous le plancher (menu/transition), 0 visible n'est pas une alarme.
        let r = chk_render_blank(10, 0, 2, 50);
        assert_eq!(r.severity, Severity::Ok);
    }

    #[test]
    fn rgl1_healthy_is_ok() {
        let r = chk_render_blank(3829, 2130, 2, 50);
        assert_eq!(r.severity, Severity::Ok);
    }

    #[test]
    fn rgl2_stuck_wave_is_warn() {
        let r = chk_stuck_wave("in_run", 0, false, false, false, 12.0, 8.0);
        assert_eq!(r.severity, Severity::Warn);
    }

    #[test]
    fn rgl2_break_is_ok() {
        let r = chk_stuck_wave("in_run", 0, true, false, false, 30.0, 8.0);
        assert_eq!(r.severity, Severity::Ok);
    }

    #[test]
    fn rgl2_lobby_is_ok() {
        let r = chk_stuck_wave("lobby", 0, false, false, false, 300.0, 8.0);
        assert_eq!(r.severity, Severity::Ok);
    }

    #[test]
    fn rgl2_bots_alive_is_ok() {
        let r = chk_stuck_wave("in_run", 5, false, false, false, 30.0, 8.0);
        assert_eq!(r.severity, Severity::Ok);
    }

    #[test]
    fn rgl2_below_threshold_is_ok() {
        // 0 bot mais juste après le début de vague (< seuil) → pas encore une alarme.
        let r = chk_stuck_wave("in_run", 0, false, false, false, 3.0, 8.0);
        assert_eq!(r.severity, Severity::Ok);
    }

    #[test]
    fn boss_state_is_monitored_like_in_run() {
        let r = chk_stuck_wave("boss", 0, false, false, false, 20.0, 8.0);
        assert_eq!(r.severity, Severity::Warn);
    }
}
