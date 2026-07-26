//! npcs_sensor.rs — Producteur `forgia2_npcs.json` (1Hz, cross-mode).
//!
//! Story-556 (2026-05-28) — comble blind spot NPCs/dialogue RPG.
//! forgia-rpg expose `Npc` Component + `InteractablePoint`.
//! forgia-rpg-data::dialogue expose `DialogueRegistry` Resource +
//! `DialogueSession` Component sur player quand conversation active.
//!
//! ## Schema JSON
//!
//! ```json
//! {
//!   "id": "npcs",
//!   "severity": "ok|warn",
//!   "next_step": "...",
//!   "timestamp_secs": 12.5,
//!   "npc_count_total": 12,
//!   "interactable_points_total": 18,
//!   "npcs_near_player_5m": 1,
//!   "npcs_near_player_20m": 3,
//!   "dialogue_registry_trees": 8,
//!   "dialogue_active": true,
//!   "dialogue_npc_name": "Aldric",
//!   "dialogue_tree_id": "forgeron",
//!   "dialogue_current_node": "greet"
//! }
//! ```
//!
//! Pas de "NPC AI state" (idle/wander/combat) — pas de mécanisme V2 à mesurer.
//! NE PAS fabriquer.

use bevy::prelude::*;
use forgia_player::Player;
use forgia_rpg::{InteractablePoint, Npc};
use forgia_rpg_data::dialogue::{DialogueRegistry, DialogueSession};

const NEAR_RADIUS_CLOSE: f32 = 5.0;
const NEAR_RADIUS_FAR: f32 = 20.0;
const SENSOR_PATH: &str = "forgia2_npcs.json";

#[derive(Resource, Default)]
pub struct NpcsSensorState {
    pub last_write_secs: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn sys_write_npcs_sensor(
    time: Res<Time>,
    registry: Option<Res<DialogueRegistry>>,
    q_npc: Query<(&Transform, &Npc)>,
    q_interact: Query<&InteractablePoint>,
    q_player: Query<&Transform, With<Player>>,
    q_session: Query<&DialogueSession, With<Player>>,
    q_npc_name_by_entity: Query<&Npc>,
    mut state: ResMut<NpcsSensorState>,
) {
    let now = time.elapsed_secs();
    if now - state.last_write_secs < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let npc_count_total = q_npc.iter().count();
    let interactable_points_total = q_interact.iter().count();
    let dialogue_registry_trees = registry.as_ref().map(|r| r.trees.len()).unwrap_or(0);

    let player_pos = q_player.iter().next().map(|tf| tf.translation);
    let mut npcs_near_close = 0u32;
    let mut npcs_near_far = 0u32;
    if let Some(p) = player_pos {
        for (tf, _) in &q_npc {
            let d2 = (tf.translation.xz() - p.xz()).length_squared();
            if d2 < NEAR_RADIUS_CLOSE * NEAR_RADIUS_CLOSE {
                npcs_near_close += 1;
            }
            if d2 < NEAR_RADIUS_FAR * NEAR_RADIUS_FAR {
                npcs_near_far += 1;
            }
        }
    }

    let session = q_session.iter().next();
    let dialogue_active = session.is_some();
    let (npc_name, tree_id, current_node) = if let Some(s) = session {
        let name = q_npc_name_by_entity
            .get(s.npc)
            .map(|n| n.name.clone())
            .unwrap_or_else(|_| "unknown".to_string());
        (name, s.tree_id.0.clone(), s.current_node.0.clone())
    } else {
        (String::new(), String::new(), String::new())
    };

    let (severity, next_step) = if npc_count_total == 0 && registry.is_some() {
        (
            "ok",
            "no Npc entities — Menu/Arena/Roguelite OK, RPG sinon vérifier village spawning",
        )
    } else if dialogue_registry_trees == 0 && npc_count_total > 0 {
        (
            "warn",
            "DialogueRegistry vide mais NPCs spawn — interactions impossibles, vérifier loader trees",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"npcs","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"npc_count_total":{npc_count_total},"interactable_points_total":{interactable_points_total},"npcs_near_player_5m":{npcs_near_close},"npcs_near_player_20m":{npcs_near_far},"dialogue_registry_trees":{dialogue_registry_trees},"dialogue_active":{dialogue_active},"dialogue_npc_name":"{}","dialogue_tree_id":"{}","dialogue_current_node":"{}"}}"#,
        now,
        sanitize(&npc_name),
        sanitize(&tree_id),
        sanitize(&current_node),
    );

    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[forgia-observability] npcs_sensor write failed: {e}");
    }
}

fn sanitize(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
