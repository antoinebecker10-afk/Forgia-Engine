//! forgia-quests — Quest definitions + runtime state machine.
//!
//! `QuestDef` is the static genome-loaded definition. `QuestLog` is the per-player
//! runtime state tracking which quests are active/completed and objective progress.
//!
//! Objectives are simple counters that other systems increment via `QuestProgress`
//! messages (e.g. forgia-damage emits "killed_X" → quest "Kill 5 wolves" hears it).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuestId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestDef {
    pub id: QuestId,
    pub title: String,
    pub description: String,
    pub objectives: Vec<Objective>,
    pub xp_reward: u32,
    /// (item_id, count) — granted on completion.
    pub item_rewards: Vec<(String, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    /// Tag string. Other systems emit `QuestProgress { tag, delta }` to increment.
    pub tag: String,
    /// Human label shown in UI ("Tuer 5 loups").
    pub label: String,
    pub target: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestStatus {
    Available,
    Active,
    Completed,
    TurnedIn,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestState {
    pub status: QuestStatus,
    /// Per-objective progress (count).
    pub progress: Vec<u32>,
}

impl QuestState {
    pub fn new(def: &QuestDef) -> Self {
        Self {
            status: QuestStatus::Active,
            progress: vec![0; def.objectives.len()],
        }
    }

    pub fn is_complete(&self, def: &QuestDef) -> bool {
        def.objectives
            .iter()
            .zip(&self.progress)
            .all(|(o, p)| *p >= o.target)
    }
}

/// Catalogue of all known quests (genome-loaded). Add via plugin or hot-reload.
#[derive(Resource, Default)]
pub struct QuestCatalogue {
    pub defs: HashMap<QuestId, QuestDef>,
}

impl QuestCatalogue {
    pub fn register(&mut self, def: QuestDef) {
        self.defs.insert(def.id.clone(), def);
    }
    pub fn get(&self, id: &QuestId) -> Option<&QuestDef> {
        self.defs.get(id)
    }
}

/// Per-player quest log. Attach as component to player entity.
#[derive(Component, Default, Debug)]
pub struct QuestLog {
    pub entries: HashMap<QuestId, QuestState>,
}

impl QuestLog {
    pub fn start(&mut self, def: &QuestDef) {
        self.entries
            .entry(def.id.clone())
            .or_insert_with(|| QuestState::new(def));
    }

    pub fn active_ids(&self) -> impl Iterator<Item = &QuestId> {
        self.entries
            .iter()
            .filter(|(_, s)| s.status == QuestStatus::Active)
            .map(|(id, _)| id)
    }
}

#[derive(Message, Debug, Clone)]
pub struct QuestProgress {
    pub tag: String,
    pub delta: u32,
}

#[derive(Message, Debug, Clone)]
pub struct QuestCompleted {
    pub player: Entity,
    pub quest_id: QuestId,
}

pub struct ForgiaQuestsPlugin;

impl Plugin for ForgiaQuestsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestCatalogue>()
            .add_message::<QuestProgress>()
            .add_message::<QuestCompleted>()
            .add_systems(Update, advance_quests);
    }
}

fn advance_quests(
    mut progress: MessageReader<QuestProgress>,
    mut completed: MessageWriter<QuestCompleted>,
    catalogue: Res<QuestCatalogue>,
    mut logs: Query<(Entity, &mut QuestLog)>,
) {
    let events: Vec<_> = progress.read().cloned().collect();
    if events.is_empty() {
        return;
    }
    for (player, mut log) in &mut logs {
        let active_ids: Vec<QuestId> = log.active_ids().cloned().collect();
        for qid in active_ids {
            let Some(def) = catalogue.get(&qid) else {
                continue;
            };
            let Some(state) = log.entries.get_mut(&qid) else {
                continue;
            };
            if state.status != QuestStatus::Active {
                continue;
            }
            for ev in &events {
                for (i, obj) in def.objectives.iter().enumerate() {
                    if obj.tag == ev.tag {
                        state.progress[i] = (state.progress[i] + ev.delta).min(obj.target);
                    }
                }
            }
            if state.is_complete(def) {
                state.status = QuestStatus::Completed;
                completed.write(QuestCompleted {
                    player,
                    quest_id: qid.clone(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_quest() -> QuestDef {
        QuestDef {
            id: QuestId("kill_wolves".into()),
            title: "Tuer 5 loups".into(),
            description: "Brennus a besoin d'aide.".into(),
            objectives: vec![Objective {
                tag: "kill_wolf".into(),
                label: "Tuer 5 loups".into(),
                target: 5,
            }],
            xp_reward: 100,
            item_rewards: vec![("gold".into(), 50)],
        }
    }

    #[test]
    fn quest_completes_on_target() {
        let def = sample_quest();
        let mut state = QuestState::new(&def);
        assert!(!state.is_complete(&def));
        state.progress[0] = 5;
        assert!(state.is_complete(&def));
    }

    #[test]
    fn quest_log_starts_quest() {
        let def = sample_quest();
        let mut log = QuestLog::default();
        log.start(&def);
        assert!(log.entries.contains_key(&def.id));
        assert_eq!(log.entries[&def.id].status, QuestStatus::Active);
    }
}
