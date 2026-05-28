//! quests_sensor.rs — Producteur `forgia2_quests.json` (1Hz, cross-mode).
//!
//! Story-554 (2026-05-28) — comble blind spot quests RPG. forgia-rpg-data
//! expose QuestCatalogue + QuestLog (Component) + QuestStatus enum mais
//! aucun sensor n'expose l'état runtime.
//!
//! ## Schema JSON
//!
//! ```json
//! {
//!   "id": "quests",
//!   "severity": "ok|warn",
//!   "next_step": "...",
//!   "timestamp_secs": 12.5,
//!   "catalogue_total": 12,
//!   "log_present": true,
//!   "active_count": 2,
//!   "completed_count": 5,
//!   "turned_in_count": 3,
//!   "failed_count": 0,
//!   "active_quests": [
//!     {"id": "kill_wolves", "title": "Tuer 5 loups", "completion_percent": 60.0}
//!   ]
//! }
//! ```
//!
//! Limite `MAX_ACTIVE_QUESTS_DUMP = 10` pour borner taille JSON.
//! Pas de queries hot path — itération 1Hz over QuestLog active_ids.

use bevy::prelude::*;
use forgia_player::Player;
use forgia_rpg_data::quests::{QuestCatalogue, QuestLog, QuestStatus};

const MAX_ACTIVE_QUESTS_DUMP: usize = 10;
const SENSOR_PATH: &str = "forgia2_quests.json";

#[derive(Resource, Default)]
pub struct QuestsSensorState {
    pub last_write_secs: f32,
}

pub fn sys_write_quests_sensor(
    time: Res<Time>,
    catalogue: Option<Res<QuestCatalogue>>,
    q_player: Query<&QuestLog, With<Player>>,
    mut state: ResMut<QuestsSensorState>,
) {
    let now = time.elapsed_secs();
    if now - state.last_write_secs < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let catalogue_total = catalogue.as_ref().map(|c| c.defs.len()).unwrap_or(0);
    let log = q_player.iter().next();
    let log_present = log.is_some();

    let mut active = 0u32;
    let mut completed = 0u32;
    let mut turned_in = 0u32;
    let mut failed = 0u32;
    let mut active_entries_json: Vec<String> = Vec::new();

    if let Some(log) = log {
        for (qid, st) in &log.entries {
            match st.status {
                QuestStatus::Active => {
                    active += 1;
                    if active_entries_json.len() < MAX_ACTIVE_QUESTS_DUMP {
                        let (title, percent) = if let Some(def) =
                            catalogue.as_ref().and_then(|c| c.defs.get(qid))
                        {
                            let total_target: u32 =
                                def.objectives.iter().map(|o| o.target).sum::<u32>().max(1);
                            let total_progress: u32 = st.progress.iter().sum();
                            let pct = (total_progress as f32 / total_target as f32) * 100.0;
                            (def.title.clone(), pct)
                        } else {
                            (qid.0.clone(), 0.0)
                        };
                        let safe_id = sanitize_json_str(&qid.0);
                        let safe_title = sanitize_json_str(&title);
                        active_entries_json.push(format!(
                            r#"{{"id":"{safe_id}","title":"{safe_title}","completion_percent":{:.1}}}"#,
                            pct_clamp(percent)
                        ));
                    }
                }
                QuestStatus::Completed => completed += 1,
                QuestStatus::TurnedIn => turned_in += 1,
                QuestStatus::Failed => failed += 1,
                QuestStatus::Available => {}
            }
        }
    }

    let (severity, next_step) = if catalogue_total == 0 {
        (
            "warn",
            "QuestCatalogue vide — RPG progression dead, vérifier loader genome",
        )
    } else if !log_present {
        (
            "ok",
            "Player sans QuestLog component — Menu/Pause/AppNotInGame OK, RPG sinon attention",
        )
    } else if active > 20 {
        (
            "warn",
            "active_count > 20 — quest log overflow risk, vérifier completion logic",
        )
    } else {
        ("ok", "")
    };

    let active_json = active_entries_json.join(",");
    let json = format!(
        r#"{{"id":"quests","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"catalogue_total":{catalogue_total},"log_present":{log_present},"active_count":{active},"completed_count":{completed},"turned_in_count":{turned_in},"failed_count":{failed},"active_quests":[{active_json}]}}"#,
        now,
    );

    if let Err(e) = std::fs::write(SENSOR_PATH, &json) {
        warn!("[forgia-observability] quests_sensor write failed: {e}");
    }
}

fn sanitize_json_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn pct_clamp(v: f32) -> f32 {
    v.clamp(0.0, 100.0)
}
