//! load_timing.rs — Diagnostic d'audit freeze (réactivé 2026-06-24 pour confirmer
//! la cause des micro-lags story-615 follow-up).
//!
//! À chaque frame lente (`dt_real > FREEZE_MS`) en `AppMode::InGame`, attribue le
//! freeze à une **cause** via les deltas depuis la frame précédente :
//! - `entity_delta` élevé → instanciation de scène GLTF (SceneSpawner Bevy = spawn vague/boss).
//! - `collider_delta` élevé → génération de colliders Rapier (ComputedColliderShape).
//! - aucun delta → upload GPU / **compilation pipeline-shader au 1er usage** (VFX non pré-warmé).
//!
//! Ring des derniers freezes → `forgia2_load_timing.json` : un seul read donne
//! l'attribution complète (timestamp + cause par freeze), à croiser avec
//! `forgia2_lag_events.json` (mêmes `t`).
//!
//! Diagnostic temporaire — à retirer une fois la cause confirmée et corrigée.

use bevy::prelude::*;
use bevy::time::Real;
use bevy_rapier3d::prelude::Collider;
use forgia_core::prelude::*;

/// Seuil de frame « lente » (ms) — diagnostic interne. 45 ms cible les vrais
/// freezes (spikes 46→250 ms du cluster observé) sans noyer le ring sous les
/// frames 30-35 ms mineures déjà couvertes par `forgia2_lag_events.json`.
const FREEZE_MS: f32 = 45.0;
/// Taille du ring de freezes récents conservés dans le sensor.
const RING: usize = 40;
/// Seuils d'attribution (deltas) — au-delà = cause dominante de la frame.
const SCENE_DELTA: i64 = 50;
const COLLIDER_DELTA: i64 = 20;

#[derive(Clone, Copy)]
struct FreezeRec {
    t: f32,
    dt_ms: f32,
    entity_delta: i64,
    collider_delta: i64,
}

/// Attribue une cause à un freeze depuis ses deltas (heuristique).
fn cause_of(entity_delta: i64, collider_delta: i64) -> &'static str {
    if entity_delta > SCENE_DELTA {
        "scene_spawn_gltf"
    } else if collider_delta > COLLIDER_DELTA {
        "colliders_rapier"
    } else {
        "gpu_or_shader_compile"
    }
}

#[derive(Resource, Default)]
pub struct LoadTimingState {
    last_entities: usize,
    last_colliders: usize,
    recent: Vec<FreezeRec>,
    total_freezes: u64,
}

/// Mesure non-intrusive : compte entités + colliders chaque frame, attribue et
/// logue les pics, écrit le ring dans `forgia2_load_timing.json`.
pub fn sys_load_timing(
    time: Res<Time<Real>>,
    app_state: Res<State<AppMode>>,
    q_all: Query<Entity>,
    q_col: Query<(), With<Collider>>,
    mut st: ResMut<LoadTimingState>,
) {
    let entities = q_all.iter().count();
    let colliders = q_col.iter().count();
    let dt_ms = time.delta_secs() * 1000.0;
    let e_delta = entities as i64 - st.last_entities as i64;
    let c_delta = colliders as i64 - st.last_colliders as i64;
    st.last_entities = entities;
    st.last_colliders = colliders;

    if *app_state.get() != AppMode::InGame || dt_ms <= FREEZE_MS {
        return;
    }

    st.total_freezes += 1;
    let cause = cause_of(e_delta, c_delta);
    info!(
        "[load-timing] FREEZE dt={dt_ms:.0}ms cause={cause} | entities {entities} ({e_delta:+}) | colliders {colliders} ({c_delta:+})"
    );

    if st.recent.len() >= RING {
        st.recent.remove(0);
    }
    st.recent.push(FreezeRec {
        t: time.elapsed_secs(),
        dt_ms,
        entity_delta: e_delta,
        collider_delta: c_delta,
    });

    let mut events = String::from("[");
    for (i, r) in st.recent.iter().enumerate() {
        if i > 0 {
            events.push(',');
        }
        events.push_str(&format!(
            r#"{{"t":{:.2},"dt_ms":{:.0},"entity_delta":{},"collider_delta":{},"cause":"{}"}}"#,
            r.t,
            r.dt_ms,
            r.entity_delta,
            r.collider_delta,
            cause_of(r.entity_delta, r.collider_delta),
        ));
    }
    events.push(']');

    let json = format!(
        r#"{{"id":"load_timing","threshold_ms":{:.0},"total_freezes":{},"entities_now":{},"colliders_now":{},"recent":{}}}"#,
        FREEZE_MS, st.total_freezes, entities, colliders, events
    );
    let _ = std::fs::write("forgia2_load_timing.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cause_attribution() {
        assert_eq!(cause_of(400, 0), "scene_spawn_gltf");
        assert_eq!(cause_of(0, 30), "colliders_rapier");
        assert_eq!(cause_of(0, 0), "gpu_or_shader_compile");
        // entité domine collider si les deux montent (spawn GLB amène ses colliders).
        assert_eq!(cause_of(400, 30), "scene_spawn_gltf");
    }
}
