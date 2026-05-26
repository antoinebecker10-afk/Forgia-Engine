//! forgia-ui-dialogue — Modal egui panel that renders the active `DialogueSession`.
//!
//! Reads `DialogueSession` component on the player + `DialogueRegistry` Resource
//! to find the current node + choices. Player clicks a choice → sends
//! `ChooseDialogueOption` event to forgia-dialogue.
//!
//! Falls back gracefully if no DialogueTree is registered for the tree_id :
//! shows a debug panel with the raw tree_id + a "Quitter" button.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_rpg_data::dialogue::{ChooseDialogueOption, DialogueRegistry, DialogueSession, EndDialogue};

pub mod prelude {
    pub use super::ForgiaUiDialoguePlugin;
}

pub struct ForgiaUiDialoguePlugin;

impl Plugin for ForgiaUiDialoguePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, render_dialogue);
    }
}

fn render_dialogue(
    mut contexts: EguiContexts,
    sessions: Query<(Entity, &DialogueSession)>,
    registry: Res<DialogueRegistry>,
    mut choose: MessageWriter<ChooseDialogueOption>,
    mut end: MessageWriter<EndDialogue>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let Some((player, session)) = sessions.iter().next() else {
        return;
    };

    // Lookup tree + node
    let (speaker, line, choices): (String, String, Vec<(usize, String)>) =
        match registry.trees.get(&session.tree_id) {
            Some(tree) => match tree.nodes.get(&session.current_node) {
                Some(node) => (
                    node.speaker.clone(),
                    node.line.clone(),
                    node.choices
                        .iter()
                        .enumerate()
                        .map(|(i, c)| (i, c.text.clone()))
                        .collect(),
                ),
                None => (
                    "??? ".into(),
                    format!("[node missing: {:?}]", session.current_node),
                    vec![],
                ),
            },
            None => (
                "Forgia".into(),
                format!(
                    "Aucun arbre de dialogue n'est enregistré pour '{}'.\n\
                     (Phase 0 : NPCs greeted via log only. Register a DialogueTree to enable choices.)",
                    session.tree_id.0
                ),
                vec![],
            ),
        };

    // Bottom-center modal
    egui::TopBottomPanel::bottom("dialogue_panel")
        .resizable(false)
        .min_height(180.0)
        .show(ctx, |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.heading(egui::RichText::new(&speaker).size(20.0).strong());
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(&line).size(15.0));
                    ui.add_space(14.0);
                    if choices.is_empty() {
                        if ui.button("Quitter").clicked() {
                            end.write(EndDialogue { player });
                        }
                    } else {
                        for (i, text) in &choices {
                            if ui.button(format!("{}. {}", i + 1, text)).clicked() {
                                choose.write(ChooseDialogueOption {
                                    player,
                                    choice_index: *i,
                                });
                            }
                        }
                    }
                });
            });
            ui.add_space(12.0);
        });
}
