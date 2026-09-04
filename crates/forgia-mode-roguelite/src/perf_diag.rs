//! perf_diag.rs — Capteur de charge combat (story-619). **Vision complète** pour
//! diagnostiquer les freezes en combat : il **corrèle** les spikes de frame avec
//! la charge réelle (ennemis / particules / lumières / auras de statut) au MÊME
//! instant — le trou des capteurs existants (`perf` donne le timing, mais rien ne
//! dit CE QUI était lourd quand ça a freezé).
//!
//! Écrit `forgia2_perf_diag.json` chaque seconde :
//! - **timing** : frame moyen + max sur la seconde, et **compte de spikes** par
//!   seuil (>30 / >45 / >60 ms) → on voit en une lecture combien de freezes et leur taille.
//! - **breakdown de charge** : entités totales + par catégorie suspecte
//!   (ennemis, auras de statut Hanabi, sparks d'élément, ParticleEffect totaux,
//!   PointLight, pickups, meshes, colliders).
//!
//! `severity = warn` dès qu'un spike >45 ms survient dans la seconde → le rapport
//! de santé pointe direct la seconde fautive, avec la charge associée.
//!
//! **Anneau de rétention** (`freezes`) : chaque seconde en `warn` est conservée
//! AVEC son breakdown complet (les `FREEZE_RING` dernières). Sans ça, une seconde
//! calme réécrirait le fichier et on perdrait la charge de la seconde du freeze —
//! le défaut de la v1. Lecture post-mortem : `forgia2_perf_diag.json::freezes[]`.
//!
//! Coût : accumulation par-frame triviale (delta + 3 comparaisons) ; les `count()`
//! ne tournent qu'**une fois par seconde** (négligeable même à 13k entités) ; le
//! push dans l'anneau (capped `FREEZE_RING`) ne se fait qu'en seconde de freeze.

use bevy::prelude::*;
use bevy::time::Real;
use bevy_rapier3d::prelude::Collider;
use forgia_core::prelude::*;
use forgia_effects::prelude::ParticleEffect;
use forgia_rpg_data::loot_tables::Pickup;

use crate::element_vfx::ElementSpark;
use crate::enemies::EnemyArchetype;
use crate::status_vfx::StatusVfxLink;

const SENSOR_PATH: &str = "forgia2_perf_diag.json";
const POLL_PERIOD_SEC: f32 = 1.0;
/// Seuils de spike (ms) — alignés sur `load_timing` (45 ms) + un cran au-dessus/dessous.
const SPIKE_30: f32 = 30.0;
const SPIKE_45: f32 = 45.0;
const SPIKE_60: f32 = 60.0;

/// Micro-freeze : 1 frame > 15 ms = hitch perceptible à haute cadence
/// (demande user 2026-07-20 : les micro-freezes < 30 ms passaient sous le radar).
const SPIKE_15: f32 = 15.0;

/// Nombre de secondes-freeze retenues dans l'anneau (post-hoc).
const FREEZE_RING: usize = 8;

/// Snapshot complet d'une seconde en freeze (timing + charge au même instant).
/// Retenu dans un anneau pour ne PAS perdre le breakdown quand une seconde calme
/// réécrit le fichier — le manque de la v1 (écrasement → seule la dernière seconde visible).
#[derive(Clone)]
struct FreezeSecond {
    t: f32,
    max_ms: f32,
    avg_ms: f32,
    spikes_15: u32,
    spikes_30: u32,
    spikes_45: u32,
    spikes_60: u32,
    enemies: usize,
    auras: usize,
    sparks: usize,
    particles: usize,
    lights: usize,
    total: usize,
}

/// Accumulateur par-seconde (timing) + compteurs de spikes + anneau de freezes.
#[derive(Resource, Default)]
pub struct PerfDiagState {
    accum: f32,
    frames: u32,
    sum_ms: f32,
    max_ms: f32,
    spikes_15: u32,
    spikes_30: u32,
    spikes_45: u32,
    spikes_60: u32,
    /// Secondes-freeze retenues (les `FREEZE_RING` dernières), pour lecture post-mortem.
    freezes: Vec<FreezeSecond>,
}

/// Tourne chaque frame (accumule timing + spikes), écrit le breakdown chaque seconde.
#[allow(clippy::too_many_arguments)]
pub fn sys_perf_diag(
    time: Res<Time<Real>>,
    app_state: Res<State<AppMode>>,
    mut st: ResMut<PerfDiagState>,
    q_all: Query<()>,
    q_enemies: Query<(), With<EnemyArchetype>>,
    q_auras: Query<(), With<StatusVfxLink>>,
    q_sparks: Query<(), With<ElementSpark>>,
    q_particles: Query<(), With<ParticleEffect>>,
    q_lights: Query<(), With<PointLight>>,
    q_pickups: Query<(), With<Pickup>>,
    q_meshes: Query<(), With<Mesh3d>>,
    q_colliders: Query<(), With<Collider>>,
) {
    // Accumulation par-frame (toujours, hors menu pour ne pas polluer).
    if *app_state.get() != AppMode::InGame {
        return;
    }
    let dt_ms = time.delta_secs() * 1000.0;
    st.frames += 1;
    st.sum_ms += dt_ms;
    st.max_ms = st.max_ms.max(dt_ms);
    if dt_ms > SPIKE_15 {
        st.spikes_15 += 1;
    }
    if dt_ms > SPIKE_30 {
        st.spikes_30 += 1;
    }
    if dt_ms > SPIKE_45 {
        st.spikes_45 += 1;
    }
    if dt_ms > SPIKE_60 {
        st.spikes_60 += 1;
    }

    st.accum += time.delta_secs();
    if st.accum < POLL_PERIOD_SEC {
        return;
    }

    let avg = if st.frames > 0 {
        st.sum_ms / st.frames as f32
    } else {
        0.0
    };
    let total = q_all.iter().count();
    let enemies = q_enemies.iter().count();
    let auras = q_auras.iter().count();
    let sparks = q_sparks.iter().count();
    let particles = q_particles.iter().count();
    let lights = q_lights.iter().count();
    let pickups = q_pickups.iter().count();
    let meshes = q_meshes.iter().count();
    let colliders = q_colliders.iter().count();

    // Health check : un spike >45 ms cette seconde = freeze ressenti → warn + next-step.
    let (severity, next_step) = if st.spikes_45 > 0 {
        (
            "warn",
            "freeze cette seconde — corréler frame_max_ms avec auras/particles/lights/enemies ci-dessous",
        )
    } else if st.spikes_30 > 2 {
        (
            "warn",
            "stutter soutenu (>2 spikes 30ms/s) — charge par-frame trop haute",
        )
    } else if st.spikes_15 > 4 {
        (
            "warn",
            "micro-stutter (>4 frames 15ms+/s) — hitchs perceptibles sous le seuil 30ms",
        )
    } else {
        ("ok", "")
    };

    // Rétention : si cette seconde a freezé (severity warn), on la garde dans l'anneau
    // AVEC sa charge complète — pour que la lecture post-hoc retrouve le breakdown
    // même si des secondes calmes réécrivent ensuite le fichier.
    if severity == "warn" {
        // Capture les champs `st.*` AVANT le push (sinon double-borrow de `st`).
        let (t, max_ms, sp15, sp30, sp45, sp60) = (
            time.elapsed_secs(),
            st.max_ms,
            st.spikes_15,
            st.spikes_30,
            st.spikes_45,
            st.spikes_60,
        );
        if st.freezes.len() >= FREEZE_RING {
            st.freezes.remove(0);
        }
        st.freezes.push(FreezeSecond {
            t,
            max_ms,
            avg_ms: avg,
            spikes_15: sp15,
            spikes_30: sp30,
            spikes_45: sp45,
            spikes_60: sp60,
            enemies,
            auras,
            sparks,
            particles,
            lights,
            total,
        });
    }

    // Sérialise l'anneau de freezes retenus (chaque entrée = timing + charge de sa seconde).
    let mut freezes_json = String::from("[");
    for (i, f) in st.freezes.iter().enumerate() {
        if i > 0 {
            freezes_json.push(',');
        }
        freezes_json.push_str(&format!(
            r#"{{"t":{:.1},"max_ms":{:.1},"avg_ms":{:.2},"spikes_15":{},"spikes_30":{},"spikes_45":{},"spikes_60":{},"enemies":{},"status_auras":{},"element_sparks":{},"particle_effects":{},"point_lights":{},"total_entities":{}}}"#,
            f.t,
            f.max_ms,
            f.avg_ms,
            f.spikes_15,
            f.spikes_30,
            f.spikes_45,
            f.spikes_60,
            f.enemies,
            f.auras,
            f.sparks,
            f.particles,
            f.lights,
            f.total,
        ));
    }
    freezes_json.push(']');

    let json = format!(
        r#"{{"id":"perf_diag","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"frame_avg_ms":{:.2},"frame_max_ms":{:.1},"spikes_15":{},"spikes_30":{},"spikes_45":{},"spikes_60":{},"frames_this_sec":{},"load":{{"total_entities":{total},"enemies":{enemies},"status_auras":{auras},"element_sparks":{sparks},"particle_effects":{particles},"point_lights":{lights},"pickups":{pickups},"meshes":{meshes},"colliders":{colliders}}},"freeze_count":{},"freezes":{freezes_json}}}"#,
        time.elapsed_secs(),
        avg,
        st.max_ms,
        st.spikes_15,
        st.spikes_30,
        st.spikes_45,
        st.spikes_60,
        st.frames,
        st.freezes.len(),
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[perf-diag] sensor write failed: {e}");
    }

    // Reset de la fenêtre.
    st.accum = 0.0;
    st.frames = 0;
    st.sum_ms = 0.0;
    st.max_ms = 0.0;
    st.spikes_15 = 0;
    st.spikes_30 = 0;
    st.spikes_45 = 0;
    st.spikes_60 = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_default_zeroed() {
        let s = PerfDiagState::default();
        assert_eq!(s.frames, 0);
        assert_eq!(s.spikes_45, 0);
        assert_eq!(s.max_ms, 0.0);
        assert!(s.freezes.is_empty());
    }

    #[test]
    fn freeze_ring_caps_and_evicts_oldest() {
        let mk = |t: f32| FreezeSecond {
            t,
            max_ms: 70.0,
            avg_ms: 8.0,
            spikes_15: 2,
            spikes_30: 1,
            spikes_45: 1,
            spikes_60: 0,
            enemies: 12,
            auras: 30,
            sparks: 5,
            particles: 40,
            lights: 34,
            total: 13000,
        };
        let mut s = PerfDiagState::default();
        // Pousse FREEZE_RING+3 freezes en respectant la règle d'éviction du tick.
        for i in 0..(FREEZE_RING + 3) {
            if s.freezes.len() >= FREEZE_RING {
                s.freezes.remove(0);
            }
            s.freezes.push(mk(i as f32));
        }
        assert_eq!(s.freezes.len(), FREEZE_RING, "anneau borné à FREEZE_RING");
        // Le plus ancien (t=0,1,2) est évincé ; le plus récent est conservé.
        assert_eq!(s.freezes.first().unwrap().t, 3.0, "oldest évincé");
        assert_eq!(
            s.freezes.last().unwrap().t,
            (FREEZE_RING + 2) as f32,
            "newest conservé"
        );
    }
}
