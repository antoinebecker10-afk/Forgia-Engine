//! arms_sensor.rs — Producteur `forgia2_viewmodel_arms.json` (1 Hz, story-661).
//!
//! Observabilité du mode bras (GLB cartoon vs poings procéduraux) : sans ce
//! capteur, un GLB manquant du dist = bras invisibles sans aucun diagnostic
//! (audit qa-lead 2026-07-20). Expose : mode actif, états de chargement des
//! 2 GLB, comptes d'entités.
//!
//! Severity :
//! - `critical` : `use_glb` actif et au moins un GLB en échec de chargement
//! - `warn`     : bras activés mais aucun root spawné (après période de grâce)
//! - `ok`       : sinon

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::arms::{ArmsGlbMode, ViewmodelArms, ViewmodelArmsTuning, ViewmodelHand};
use crate::arms::{ARM_GLB_LEFT, ARM_GLB_RIGHT};

/// Pur — testable sans App. `grace_over` = période de boot passée.
pub fn severity_for_arms(
    enabled: bool,
    use_glb: bool,
    glb_failed: bool,
    roots: usize,
    grace_over: bool,
) -> (&'static str, &'static str) {
    if enabled && use_glb && glb_failed {
        return (
            "critical",
            "GLB bras en échec — vérifier assets/models/arms/fps_arm_L|R.glb (présents dans le dist ?)",
        );
    }
    if enabled && roots == 0 && grace_over {
        return (
            "warn",
            "bras activés mais non spawnés — FpsCamera présente ? voir spawn_arms",
        );
    }
    ("ok", "")
}

fn load_state_str(asset_server: &AssetServer, path: &'static str) -> &'static str {
    let Some(id) = asset_server.get_path_id(path) else {
        return "not_requested";
    };
    match asset_server.get_load_state(id) {
        Some(LoadState::Loaded) => "loaded",
        Some(LoadState::Loading) => "loading",
        Some(LoadState::Failed(_)) => "failed",
        Some(LoadState::NotLoaded) => "not_loaded",
        None => "unknown",
    }
}

/// Écrit le JSON à 1 Hz. Gated `GameMode::Fps|Roguelite` (comme les systèmes bras).
pub fn write_arms_sensor(
    time: Res<Time>,
    mut acc: Local<f32>,
    mut ticks: Local<u32>,
    tuning: Res<ViewmodelArmsTuning>,
    asset_server: Res<AssetServer>,
    q_roots: Query<&ArmsGlbMode, With<ViewmodelArms>>,
    q_hands: Query<(), With<ViewmodelHand>>,
) {
    *acc += time.delta_secs();
    if *acc < 1.0 {
        return;
    }
    *acc = 0.0;
    *ticks += 1;

    let state_l = load_state_str(&asset_server, ARM_GLB_LEFT);
    let state_r = load_state_str(&asset_server, ARM_GLB_RIGHT);
    let glb_failed = state_l == "failed" || state_r == "failed";
    let roots = q_roots.iter().count();
    let hands = q_hands.iter().count();
    let mode_active = match q_roots.iter().next() {
        Some(ArmsGlbMode(true)) => "glb",
        Some(ArmsGlbMode(false)) => "procedural",
        None => "none",
    };
    // Grâce de 5 ticks (~5 s) : laisse le boot spawner la caméra + les bras.
    let (severity, next_step) =
        severity_for_arms(tuning.enabled, tuning.use_glb, glb_failed, roots, *ticks > 5);

    let json = format!(
        r#"{{"id":"viewmodel_arms","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"enabled":{},"use_glb":{},"glb_scale":{:.2},"mode_active":"{mode_active}","arms_roots":{roots},"hands":{hands},"glb_state_l":"{state_l}","glb_state_r":"{state_r}"}}"#,
        time.elapsed_secs(),
        tuning.enabled,
        tuning.use_glb,
        tuning.glb_scale,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_viewmodel_arms.json", json) {
        warn!("[forgia-viewmodel] arms sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_when_glb_failed() {
        let (sev, step) = severity_for_arms(true, true, true, 1, true);
        assert_eq!(sev, "critical");
        assert!(step.contains("fps_arm"));
    }

    #[test]
    fn warn_when_no_roots_after_grace() {
        assert_eq!(severity_for_arms(true, true, false, 0, true).0, "warn");
        // Pendant la grâce : pas d'alerte.
        assert_eq!(severity_for_arms(true, true, false, 0, false).0, "ok");
    }

    #[test]
    fn ok_nominal_and_when_disabled() {
        assert_eq!(severity_for_arms(true, true, false, 1, true).0, "ok");
        // Désactivé : jamais d'alerte, même sans root.
        assert_eq!(severity_for_arms(false, true, true, 0, true).0, "ok");
    }

    #[test]
    fn procedural_mode_ignores_glb_failure() {
        // use_glb=false : un échec GLB (asset jamais demandé/cassé) n'alerte pas.
        assert_eq!(severity_for_arms(true, false, true, 1, true).0, "ok");
    }
}
