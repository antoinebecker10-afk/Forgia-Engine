//! # forgia-anim-debug
//!
//! Observability unifiée pour la Forgia Anim Layer (story-437/438).
//!
//! ## Producteurs attendus
//!
//! - `forgia-secondary-motion` → spring_chains_active, spring_bones_total
//! - `forgia-rpg::character` → locomotion stats (gait_phase, speed, cache_ready)
//! - `forgia-ik` (Phase 2) → ik_chains_active, ik_iterations_avg
//! - `forgia-cloth` (Phase 2) → cloth_particles_total, cloth_constraints_avg
//!
//! ## Sortie
//!
//! - `forgia_anim_layer.json` à la racine workspace, refresh 2s.
//! - `forgia_anim_layer_health.json` si overrun budget sustained.
//!
//! ## Health alert
//!
//! Si `frame_time_us > BUDGET_US` pendant > 5s consécutifs : flag `warning`
//! avec next-step `Read forgia_anim_layer.json` (rule quality-gate alerte→next-step).
//!
//! ## Convention
//!
//! Pas de serde — manual format!() pour zéro dep + lisibilité.

use bevy::prelude::*;
use forgia_core::prelude::*;
use std::time::Instant;

pub mod prelude {
    pub use crate::{AnimLayerStats, ForgiaAnimDebugPlugin};
}

/// Budget cible total Anim Layer par frame (microsecondes).
/// Spring (500µs) + locomotion (50µs) + IK futur (200µs) + cloth futur (1ms) = ~2ms hard cap.
pub const ANIM_LAYER_BUDGET_US: u128 = 2000;
/// Intervalle d'écriture du sensor JSON (secondes).
pub const SENSOR_INTERVAL_S: f32 = 2.0;
/// Seuil d'overrun sustained avant flag health (secondes).
pub const HEALTH_OVERRUN_THRESHOLD_S: f32 = 5.0;

/// Stats agrégées de la couche anim, lues par le sensor writer + health monitor.
/// Mises à jour par chaque producteur (spring, locomotion, IK, cloth).
#[derive(Resource, Default, Debug)]
pub struct AnimLayerStats {
    // — Secondary motion (spring bones)
    pub spring_chains_active: u32,
    pub spring_bones_total: u32,
    pub spring_solver_us: u128,

    // — Locomotion procédurale
    pub locomotion_active: bool,
    pub locomotion_cache_ready: bool,
    pub locomotion_speed: f32,
    pub locomotion_gait_phase: f32,
    pub locomotion_is_moving: bool,
    pub locomotion_us: u128,

    // — IK (Phase 2 reserved)
    pub ik_chains_active: u32,
    pub ik_iterations_avg: f32,
    pub ik_solver_us: u128,

    // — Cloth (Phase 2 reserved)
    pub cloth_particles_total: u32,
    pub cloth_constraint_iters: u32,
    pub cloth_solver_us: u128,

    // — Total (calculé par sensor writer)
    pub total_us: u128,
}

impl AnimLayerStats {
    pub fn reset_per_frame_counters(&mut self) {
        // Compteurs qui se reset à chaque frame (vs flags persistants comme cache_ready)
        self.spring_solver_us = 0;
        self.locomotion_us = 0;
        self.ik_solver_us = 0;
        self.cloth_solver_us = 0;
    }
}

/// Mesure manuelle de durée d'un système. Producteurs l'utilisent pour fournir leur us.
///
/// ```ignore
/// let t = AnimTimer::start();
/// // ... travail ...
/// stats.spring_solver_us = t.elapsed_us();
/// ```
pub struct AnimTimer(Instant);
impl AnimTimer {
    pub fn start() -> Self {
        Self(Instant::now())
    }
    pub fn elapsed_us(&self) -> u128 {
        self.0.elapsed().as_micros()
    }
}

/// État interne du sensor writer (timer accumulé).
#[derive(Resource, Default)]
struct SensorTimer {
    accum_s: f32,
    overrun_accum_s: f32,
}

pub struct ForgiaAnimDebugPlugin;

impl Plugin for ForgiaAnimDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnimLayerStats>()
            .init_resource::<SensorTimer>()
            .add_systems(
                Update,
                (reset_stats_each_frame, write_sensor_and_health)
                    .chain()
                    .in_set(GameSet::Sensors),
            );
    }
}

/// Reset les compteurs frame-locaux au DÉBUT de chaque frame (avant que les producteurs
/// remplissent). Ordering : ce système doit run AVANT les producteurs.
/// On le met dans `Sensors` puis on s'appuie sur l'ordering interne via `.chain()` —
/// mais ici on veut reset AVANT que les producteurs (qui sont dans Movement/Effects) écrivent.
/// Solution simple : reset à la FIN du frame précédent. Producteurs lisent le frame suivant
/// avec compteurs déjà à zéro. Le sensor writer lit les compteurs JUSTE écrits par les
/// producteurs du même frame, puis reset_for_next_frame.
fn reset_stats_each_frame(_stats: ResMut<AnimLayerStats>) {
    // Phase 1 : pas de reset agressif — les producteurs écrasent leurs fields à chaque
    // frame de toute façon (assignment direct, pas accumulation). Reset placeholder
    // pour Phase 2 quand on aura des moyennes glissantes.
}

/// Écrit le sensor JSON + le health alert toutes les SENSOR_INTERVAL_S.
fn write_sensor_and_health(
    time: Res<Time>,
    mut timer: ResMut<SensorTimer>,
    mut stats: ResMut<AnimLayerStats>,
) {
    let dt = time.delta_secs();
    timer.accum_s += dt;

    // Total time-budget tracking
    stats.total_us = stats.spring_solver_us
        + stats.locomotion_us
        + stats.ik_solver_us
        + stats.cloth_solver_us;

    // Overrun tracking
    if stats.total_us > ANIM_LAYER_BUDGET_US {
        timer.overrun_accum_s += dt;
    } else {
        // Décroissance — on ne reset pas à zéro brutalement pour éviter flapping
        timer.overrun_accum_s = (timer.overrun_accum_s - dt * 0.5).max(0.0);
    }

    if timer.accum_s < SENSOR_INTERVAL_S {
        return;
    }
    timer.accum_s = 0.0;

    let alert_severity = if timer.overrun_accum_s > HEALTH_OVERRUN_THRESHOLD_S {
        "warning"
    } else {
        "ok"
    };

    // ── Sensor JSON principal ────────────────────────────────────────────────
    let json = format!(
        r#"{{
  "timestamp_secs": {:.1},
  "spring": {{
    "chains_active": {},
    "bones_total": {},
    "solver_us": {}
  }},
  "locomotion": {{
    "active": {},
    "cache_ready": {},
    "speed_m_s": {:.2},
    "gait_phase_rad": {:.2},
    "is_moving": {},
    "system_us": {}
  }},
  "ik": {{
    "chains_active": {},
    "iterations_avg": {:.2},
    "solver_us": {}
  }},
  "cloth": {{
    "particles_total": {},
    "constraint_iters": {},
    "solver_us": {}
  }},
  "budget": {{
    "total_us": {},
    "budget_us": {},
    "overrun_sustained_s": {:.2},
    "severity": "{}"
  }}
}}"#,
        time.elapsed_secs(),
        stats.spring_chains_active,
        stats.spring_bones_total,
        stats.spring_solver_us,
        stats.locomotion_active,
        stats.locomotion_cache_ready,
        stats.locomotion_speed,
        stats.locomotion_gait_phase,
        stats.locomotion_is_moving,
        stats.locomotion_us,
        stats.ik_chains_active,
        stats.ik_iterations_avg,
        stats.ik_solver_us,
        stats.cloth_particles_total,
        stats.cloth_constraint_iters,
        stats.cloth_solver_us,
        stats.total_us,
        ANIM_LAYER_BUDGET_US,
        timer.overrun_accum_s,
        alert_severity,
    );
    if let Err(e) = std::fs::write("forgia_anim_layer.json", json) {
        warn!("[forgia-anim-debug] sensor write failed: {e}");
    }

    // ── Health alert side-file ───────────────────────────────────────────────
    // BUG-ANIMQA-07 fix : convention V1 forgia_health.json = side-file écrit
    // SEULEMENT si severity != "ok". Permet detection "fichier absent = OK"
    // côté Claude / dev. Si severity retombe à ok, on supprime le file existant.
    if alert_severity == "ok" {
        // Best-effort delete (fail silencieux si fichier n'existe pas)
        let _ = std::fs::remove_file("forgia_anim_layer_health.json");
    } else {
        let health_json = format!(
            r#"{{
  "timestamp_secs": {:.1},
  "severity": "{}",
  "message": "Anim Layer budget {} us (cap {} us), overrun sustained {:.1}s",
  "next_step": "Read forgia_anim_layer.json — identifier quel sub-system (spring/locomotion/ik/cloth) dépasse, puis profile via budget.total_us breakdown",
  "exclusion_known": "Or: 1er frame BFS bone discovery cher (~10ms one-shot, reset après 1 frame). À ignorer si overrun < 2s."
}}"#,
            time.elapsed_secs(),
            alert_severity,
            stats.total_us,
            ANIM_LAYER_BUDGET_US,
            timer.overrun_accum_s,
        );
        if let Err(e) = std::fs::write("forgia_anim_layer_health.json", health_json) {
            warn!("[forgia-anim-debug] health write failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_default_zero() {
        let s = AnimLayerStats::default();
        assert_eq!(s.spring_chains_active, 0);
        assert_eq!(s.locomotion_us, 0);
    }

    #[test]
    fn timer_measures_nonzero() {
        let t = AnimTimer::start();
        std::thread::sleep(std::time::Duration::from_micros(100));
        assert!(t.elapsed_us() >= 100);
    }

    #[test]
    fn plugin_builds_without_panic() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        // ForgiaCorePlugin requires StatesPlugin (init_state panics sinon)
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.add_plugins(forgia_core::ForgiaCorePlugin);
        app.add_plugins(ForgiaAnimDebugPlugin);
    }
}
