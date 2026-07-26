//! sensor.rs — Producteur forgia_combat.json (1Hz).
//!
//! Écrit à la racine du workspace toutes les secondes :
//! - player_hp / max_hp si un Player + Health sont présents
//! - active_weapon (WeaponType display name)
//! - shots_fired_last_sec / hits_landed_last_sec (compteurs reset/frame via CombatSensorCounters)
//!
//! Format : JSON plat sans serde (0 dep supplémentaire).

use bevy::prelude::*;

use crate::{weapons::EquippedWeapons, Health};

/// Compteurs de combat mis à jour par les systèmes de tir/hit (optionnels, défaut 0).
/// Doivent être reset à chaque tick sensor (1Hz) — le sensor writer s'en charge.
#[derive(Resource, Default)]
pub struct CombatSensorCounters {
    pub shots_fired: u32,
    pub hits_landed: u32,
}

/// Marker component : identifie l'entité joueur local portant un Health.
/// Si forgia-player n'est pas disponible, le sensor cherche n'importe quel Health.
#[derive(Component)]
pub struct LocalPlayerMarker;

/// Système sensor 1Hz — écrit forgia_combat.json.
#[allow(clippy::too_many_arguments)]
pub fn sys_write_combat_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    equipped: Res<EquippedWeapons>,
    counters: Option<ResMut<CombatSensorCounters>>,
    // Cherche d'abord un joueur marqué LocalPlayerMarker, sinon le premier Health disponible
    q_player: Query<&Health, With<LocalPlayerMarker>>,
    q_any_health: Query<&Health, Without<LocalPlayerMarker>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    // Résoudre hp : priorité marqueur, sinon premier health, sinon absent
    let (player_hp_str, max_hp_str, severity, next_step) = if let Ok(h) = q_player.single() {
        let severity = if h.current <= 0.0 {
            "critical"
        } else if h.current < h.max * 0.25 {
            "warn"
        } else {
            "ok"
        };
        let next_step = if h.current <= 0.0 {
            "Player mort — vérifier respawn system"
        } else {
            ""
        };
        (
            format!("{:.1}", h.current),
            format!("{:.1}", h.max),
            severity,
            next_step,
        )
    } else if let Some(h) = q_any_health.iter().next() {
        (
            format!("{:.1}", h.current),
            format!("{:.1}", h.max),
            "ok",
            "",
        )
    } else {
        // Pas de joueur encore spawné (menu, pré-spawn Arena)
        (
            "null".to_string(),
            "null".to_string(),
            "ok",
            "no player yet",
        )
    };

    let weapon_name = equipped.current.display_name();

    let (shots, hits) = if let Some(ref c) = counters {
        (c.shots_fired, c.hits_landed)
    } else {
        (0, 0)
    };

    // Reset compteurs après lecture
    if let Some(mut c) = counters {
        c.shots_fired = 0;
        c.hits_landed = 0;
    }

    let json = format!(
        r#"{{
  "id": "combat",
  "severity": "{}",
  "next_step": "{}",
  "timestamp_secs": {:.1},
  "player_hp": {},
  "max_hp": {},
  "active_weapon": "{}",
  "shots_fired_last_sec": {},
  "hits_landed_last_sec": {}
}}"#,
        severity,
        next_step,
        time.elapsed_secs(),
        player_hp_str,
        max_hp_str,
        weapon_name,
        shots,
        hits,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue("forgia_combat.json", json) {
        warn!("[forgia-combat] sensor write failed: {e}");
    }
}
