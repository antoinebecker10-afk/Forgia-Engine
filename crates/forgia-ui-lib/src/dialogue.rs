//! forgia-ui-dialogue — Modal egui panel that renders the active `DialogueSession`.
//!
//! Reads `DialogueSession` component on the player + `DialogueRegistry` Resource
//! to find the current node + choices. Player clicks a choice → sends
//! `ChooseDialogueOption` event to forgia-dialogue.
//!
//! Falls back gracefully if no DialogueTree is registered for the tree_id :
//! shows a debug panel with the raw tree_id + a "Quitter" button.

use bevy::prelude::*;
use bevy_egui::egui::Color32;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_rpg_data::dialogue::{
    ChooseDialogueOption, DialogueEffect, DialogueRegistry, DialogueSession, EndDialogue,
};
use forgia_rpg_data::inventory::ItemId;
use forgia_rpg_data::items::ItemRegistry;
use forgia_rpg_data::quests::{QuestCatalogue, QuestId};

use crate::style::*;

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
    quest_catalogue: Res<QuestCatalogue>,
    item_registry: Res<ItemRegistry>,
    mut choose: MessageWriter<ChooseDialogueOption>,
    mut end: MessageWriter<EndDialogue>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let Some((player, session)) = sessions.iter().next() else {
        return;
    };

    // ── Extraction (owned) avant rendu ────────────────────────────────────
    let node = registry
        .trees
        .get(&session.tree_id)
        .and_then(|t| t.nodes.get(&session.current_node));

    let (speaker, line) = match node {
        Some(n) => (n.speaker.clone(), n.line.clone()),
        None => (
            "Forgia".to_string(),
            format!("Aucun dialogue enregistré pour '{}'.", session.tree_id.0),
        ),
    };

    // Choix : (index, texte, is_accept = contient un StartQuest).
    let choices: Vec<(usize, String, bool)> = node
        .map(|n| {
            n.choices
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let is_accept = c
                        .effects
                        .iter()
                        .any(|e| matches!(e, DialogueEffect::StartQuest { .. }));
                    (i, c.text.clone(), is_accept)
                })
                .collect()
        })
        .unwrap_or_default();

    // Aperçu récompense : 1er StartQuest du node → def quête (XP + objets).
    let mut reward_chips: Vec<(String, Color32)> = Vec::new();
    if let Some(n) = node {
        'outer: for c in &n.choices {
            for e in &c.effects {
                if let DialogueEffect::StartQuest { id } = e {
                    if let Some(def) = quest_catalogue.get(&QuestId(id.clone())) {
                        if def.xp_reward > 0 {
                            reward_chips.push((format!("{} XP", def.xp_reward), FORGE_OR));
                        }
                        for (iid, count) in &def.item_rewards {
                            if iid.as_str() == "gold" {
                                reward_chips.push((format!("🪙 {count} or"), FORGE_OR));
                            } else if let Some(idef) = item_registry.get(&ItemId(iid.clone())) {
                                let label = if *count > 1 {
                                    format!("{} ×{count}", idef.display_name)
                                } else {
                                    idef.display_name.clone()
                                };
                                reward_chips
                                    .push((label, rarity_color_tier(idef.rarity.tier_index())));
                            } else {
                                reward_chips.push((iid.clone(), FORGE_METAL_CHAUD));
                            }
                        }
                    }
                    break 'outer;
                }
            }
        }
    }

    let initial = speaker
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let portrait = portrait_color(&speaker);

    // ── Rendu : gossip frame cartoon ancré bas-centre ─────────────────────
    egui::Area::new(egui::Id::new("forgia_gossip_frame"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -40.0))
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(FORGE_BOIS_CLAIR)
                .stroke(egui::Stroke::new(4.0, FORGE_OR))
                .corner_radius(egui::CornerRadius::same(16))
                .inner_margin(18);
            frame.show(ui, |ui| {
                ui.set_min_width(600.0);
                ui.set_max_width(600.0);

                // En-tête : portrait (cercle + initiale) + nom + ligne.
                ui.horizontal_top(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover());
                    let p = ui.painter_at(rect);
                    let c = rect.center();
                    p.circle_filled(c, 34.0, portrait);
                    p.circle_stroke(c, 34.0, egui::Stroke::new(3.0, C_OUTLINE));
                    text_with_outline(
                        &p,
                        c,
                        egui::Align2::CENTER_CENTER,
                        &initial,
                        egui::FontId::proportional(32.0),
                        FORGE_CREME,
                        1.5,
                    );
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.set_max_width(480.0);
                        ui.label(
                            egui::RichText::new(&speaker)
                                .size(22.0)
                                .strong()
                                .color(FORGE_CHARBON),
                        );
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(&line).size(16.0).color(FORGE_CHARBON));
                    });
                });

                // Récompenses (si une quête est proposée dans ce node).
                if !reward_chips.is_empty() {
                    ui.add_space(10.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new("Récompense :")
                                .size(15.0)
                                .strong()
                                .color(FORGE_CHARBON),
                        );
                        for (label, color) in &reward_chips {
                            ui.label(
                                egui::RichText::new(label)
                                    .size(15.0)
                                    .strong()
                                    .color(FORGE_CHARBON)
                                    .background_color(*color),
                            );
                        }
                    });
                }

                ui.add_space(14.0);

                // Choix (vert = accepte une quête) ou bouton Quitter.
                if choices.is_empty() {
                    let btn = egui::Button::new(
                        egui::RichText::new("Quitter")
                            .size(18.0)
                            .strong()
                            .color(FORGE_CHARBON),
                    )
                    .fill(FORGE_METAL_CHAUD)
                    .stroke(egui::Stroke::new(3.0, FORGE_CHARBON))
                    .corner_radius(egui::CornerRadius::same(12))
                    .min_size(egui::vec2(564.0, 40.0));
                    if ui.add(btn).clicked() {
                        end.write(EndDialogue { player });
                    }
                } else {
                    for (i, text, is_accept) in &choices {
                        let (fill, tcol) = if *is_accept {
                            (FORGE_TEAL, FORGE_CREME)
                        } else {
                            (FORGE_METAL_CHAUD, FORGE_CHARBON)
                        };
                        let btn = egui::Button::new(
                            egui::RichText::new(format!("{}. {}", i + 1, text))
                                .size(17.0)
                                .strong()
                                .color(tcol),
                        )
                        .fill(fill)
                        .stroke(egui::Stroke::new(3.0, FORGE_CHARBON))
                        .corner_radius(egui::CornerRadius::same(12))
                        .min_size(egui::vec2(564.0, 40.0));
                        if ui.add(btn).clicked() {
                            choose.write(ChooseDialogueOption {
                                player,
                                choice_index: *i,
                            });
                        }
                        ui.add_space(6.0);
                    }
                }
            });
        });
}

/// Mappe un palier de rareté (0..=4) vers la couleur `FORGE_RARITY_*`.
fn rarity_color_tier(tier: u8) -> Color32 {
    match tier {
        0 => FORGE_RARITY_COMMON,
        1 => FORGE_RARITY_UNCOMMON,
        2 => FORGE_RARITY_RARE,
        3 => FORGE_RARITY_EPIC,
        _ => FORGE_RARITY_LEGENDARY,
    }
}

/// Couleur de portrait déterministe à partir du nom (fallback sans asset).
fn portrait_color(name: &str) -> Color32 {
    const PALETTE: [Color32; 5] = [
        Color32::from_rgb(198, 120, 70),
        Color32::from_rgb(120, 150, 90),
        Color32::from_rgb(150, 110, 160),
        Color32::from_rgb(90, 140, 160),
        Color32::from_rgb(190, 150, 80),
    ];
    let sum: usize = name.bytes().map(|b| b as usize).sum();
    PALETTE[sum % PALETTE.len()]
}
