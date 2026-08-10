//! forgia-dialogue — Branching dialogue trees with choices and effects.
//!
//! Each NPC has a `DialogueTree` indexed by node id. Choices link to next nodes
//! or trigger `DialogueEffect` (give item, start quest, increase reputation).
//!
//! UI rendering is in forgia-ui-dialogue (renders the active DialogueSession).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::gold::Gold;
use crate::inventory::{Inventory, ItemId, ItemStack};
use crate::items::ItemRegistry;
use crate::quests::{QuestCatalogue, QuestId, QuestLog, QuestProgress};
use crate::shop::OpenShop;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DialogueId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueTree {
    pub id: DialogueId,
    pub root: NodeId,
    pub nodes: HashMap<NodeId, DialogueNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueNode {
    pub speaker: String,
    pub line: String,
    pub choices: Vec<DialogueChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueChoice {
    pub text: String,
    pub next: Option<NodeId>,
    #[serde(default)]
    pub effects: Vec<DialogueEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DialogueEffect {
    GiveItem {
        id: String,
        count: u32,
    },
    StartQuest {
        id: String,
    },
    /// Increment a quest objective counter (no-combat progression hook).
    AdvanceQuest {
        tag: String,
        delta: u32,
    },
    /// Remet une quête `Completed` au donneur → `TurnedIn` (clôture la boucle).
    TurnInQuest {
        id: String,
    },
    /// Ouvre la boutique du PNJ courant (vendeur). Termine le dialogue.
    OpenShop,
    EndConversation,
}

/// Active dialogue session — attached to player when a conversation starts.
#[derive(Component, Debug)]
pub struct DialogueSession {
    pub tree_id: DialogueId,
    pub current_node: NodeId,
    pub npc: Entity,
}

#[derive(Resource, Default)]
pub struct DialogueRegistry {
    pub trees: HashMap<DialogueId, DialogueTree>,
}

#[derive(Message, Debug, Clone)]
pub struct StartDialogue {
    pub player: Entity,
    pub npc: Entity,
    pub tree_id: DialogueId,
}

#[derive(Message, Debug, Clone)]
pub struct ChooseDialogueOption {
    pub player: Entity,
    pub choice_index: usize,
}

#[derive(Message, Debug, Clone)]
pub struct EndDialogue {
    pub player: Entity,
}

pub struct ForgiaDialoguePlugin;

impl Plugin for ForgiaDialoguePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogueRegistry>()
            .add_message::<StartDialogue>()
            .add_message::<ChooseDialogueOption>()
            .add_message::<EndDialogue>()
            .add_systems(Update, (start_sessions, advance_sessions, end_sessions));
    }
}

fn start_sessions(
    mut events: MessageReader<StartDialogue>,
    mut commands: Commands,
    registry: Res<DialogueRegistry>,
) {
    for ev in events.read() {
        let Some(tree) = registry.trees.get(&ev.tree_id) else {
            continue;
        };
        commands.entity(ev.player).insert(DialogueSession {
            tree_id: ev.tree_id.clone(),
            current_node: tree.root.clone(),
            npc: ev.npc,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn advance_sessions(
    mut events: MessageReader<ChooseDialogueOption>,
    mut sessions: Query<&mut DialogueSession>,
    registry: Res<DialogueRegistry>,
    mut end_w: MessageWriter<EndDialogue>,
    catalogue: Res<QuestCatalogue>,
    item_registry: Res<ItemRegistry>,
    mut inventories: Query<&mut Inventory>,
    mut golds: Query<&mut Gold>,
    mut quest_logs: Query<&mut QuestLog>,
    mut quest_progress_w: MessageWriter<QuestProgress>,
    mut shop_w: MessageWriter<OpenShop>,
) {
    for ev in events.read() {
        let Ok(mut session) = sessions.get_mut(ev.player) else {
            continue;
        };
        let npc = session.npc;
        let Some(tree) = registry.trees.get(&session.tree_id) else {
            continue;
        };
        let Some(node) = tree.nodes.get(&session.current_node) else {
            continue;
        };
        let Some(choice) = node.choices.get(ev.choice_index) else {
            continue;
        };
        // Own the choice data before mutating session / other components (the
        // `choice` borrow points into the immutable DialogueRegistry resource).
        let effects = choice.effects.clone();
        let next = choice.next.clone();

        let mut should_end = false;
        for eff in &effects {
            match eff {
                DialogueEffect::EndConversation => should_end = true,
                DialogueEffect::GiveItem { id, count } => {
                    if id.as_str() == "gold" {
                        if let Ok(mut g) = golds.get_mut(ev.player) {
                            g.add(*count);
                        }
                    } else if let Ok(mut inv) = inventories.get_mut(ev.player) {
                        let max = item_registry.max_stack_of(&ItemId(id.clone()));
                        if let Some(left) = inv.add(ItemStack::new(id.clone(), *count, max)) {
                            warn!(
                                "[dialogue] inventaire plein : {} x{} non ajouté",
                                left.id.0, left.count
                            );
                        }
                    }
                }
                DialogueEffect::StartQuest { id } => {
                    let qid = QuestId(id.clone());
                    if let Some(def) = catalogue.get(&qid) {
                        if let Ok(mut log) = quest_logs.get_mut(ev.player) {
                            log.start(def);
                        }
                    } else {
                        warn!("[dialogue] StartQuest : quête inconnue '{}'", id);
                    }
                }
                DialogueEffect::AdvanceQuest { tag, delta } => {
                    quest_progress_w.write(QuestProgress {
                        tag: tag.clone(),
                        delta: *delta,
                    });
                }
                DialogueEffect::TurnInQuest { id } => {
                    if let Ok(mut log) = quest_logs.get_mut(ev.player) {
                        if log.turn_in(&QuestId(id.clone())) {
                            info!("[dialogue] quête '{}' rendue (TurnedIn)", id);
                        }
                    }
                }
                DialogueEffect::OpenShop => {
                    shop_w.write(OpenShop {
                        player: ev.player,
                        npc,
                    });
                    should_end = true;
                }
            }
        }
        if let Some(next) = next {
            session.current_node = next;
        } else {
            should_end = true;
        }
        if should_end {
            end_w.write(EndDialogue { player: ev.player });
        }
    }
}

fn end_sessions(mut events: MessageReader<EndDialogue>, mut commands: Commands) {
    for ev in events.read() {
        commands.entity(ev.player).remove::<DialogueSession>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_lookup() {
        let mut tree = DialogueTree {
            id: DialogueId("forgeron".into()),
            root: NodeId("greet".into()),
            nodes: HashMap::new(),
        };
        tree.nodes.insert(
            NodeId("greet".into()),
            DialogueNode {
                speaker: "Aldric".into(),
                line: "Bienvenue.".into(),
                choices: vec![DialogueChoice {
                    text: "Salutations.".into(),
                    next: None,
                    effects: vec![DialogueEffect::EndConversation],
                }],
            },
        );
        assert!(tree.nodes.contains_key(&NodeId("greet".into())));
    }
}
