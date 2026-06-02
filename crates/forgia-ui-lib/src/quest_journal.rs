//! quest_journal — Journal de quêtes (touche J) + tracker bord d'écran (RPG).
//!
//! Story-58x Phase 2 — UI style WoW, habillage cartoon Forge.
//! - `draw_quest_journal` : panneau modal (J toggle) listant les quêtes actives,
//!   leurs objectifs + barres de progression, bouton « Suivre » → `TrackedQuest`.
//! - `draw_quest_tracker` : overlay compact RIGHT_TOP affichant la quête suivie
//!   (ou la 1re active) + progression live.
//!
//! Data : `QuestLog` / `TrackedQuest` (Components Player) + `QuestCatalogue` (Res).
//! Gating : `GameMode::Rpg`.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::GameMode;
use forgia_player::Player;
use forgia_rpg_data::dialogue::DialogueSession;
use forgia_rpg_data::quests::{QuestCatalogue, QuestId, QuestLog, QuestStatus, TrackedQuest};
use forgia_rpg_data::shop::ShopSession;

use crate::style::*;
use crate::RpgOpenPanel;

pub mod prelude {
    pub use super::ForgiaUiQuestPlugin;
}

pub struct ForgiaUiQuestPlugin;

impl Plugin for ForgiaUiQuestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RpgOpenPanel>().add_systems(
            EguiPrimaryContextPass,
            (draw_quest_journal, draw_quest_tracker).run_if(in_state(GameMode::Rpg)),
        );
    }
}

/// Objectif aplati pour le rendu : (label, courant, cible).
type ObjView = (String, u32, u32);

#[allow(clippy::type_complexity)]
fn draw_quest_journal(
    mut contexts: EguiContexts,
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: ResMut<RpgOpenPanel>,
    catalogue: Res<QuestCatalogue>,
    mut q: Query<(&QuestLog, &mut TrackedQuest), With<Player>>,
    blocking: Query<(), (With<Player>, Or<(With<ShopSession>, With<DialogueSession>)>)>,
) {
    let blocked = !blocking.is_empty();
    // Toggle gaté : pas d'ouverture pendant boutique/dialogue (BUG-review-#2b).
    if keys.just_pressed(KeyCode::KeyJ) && !blocked {
        *panel = if *panel == RpgOpenPanel::Journal {
            RpgOpenPanel::None
        } else {
            RpgOpenPanel::Journal
        };
    }
    // Vendeur / dialogue actif → forcer la fermeture (pas de réouverture auto).
    if blocked && *panel == RpgOpenPanel::Journal {
        *panel = RpgOpenPanel::None;
    }
    let show = *panel == RpgOpenPanel::Journal;
    if !show {
        return;
    }
    let Ok((log, mut tracked)) = q.single_mut() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    // Construire la liste (owned) des quêtes Active + Completed.
    let mut entries: Vec<(QuestId, String, Vec<ObjView>, bool)> = Vec::new();
    for (qid, state) in &log.entries {
        if !matches!(state.status, QuestStatus::Active | QuestStatus::Completed) {
            continue;
        }
        let Some(def) = catalogue.get(qid) else { continue };
        let completed = state.status == QuestStatus::Completed;
        let objs: Vec<ObjView> = def
            .objectives
            .iter()
            .enumerate()
            .map(|(i, o)| (o.label.clone(), state.progress.get(i).copied().unwrap_or(0), o.target))
            .collect();
        entries.push((qid.clone(), def.title.clone(), objs, completed));
    }
    entries.sort_by(|a, b| a.1.cmp(&b.1));

    let frame = egui::Frame::new()
        .fill(FORGE_BOIS_CLAIR)
        .stroke(egui::Stroke::new(4.0, FORGE_OR))
        .corner_radius(egui::CornerRadius::same(16))
        .inner_margin(18);

    egui::Window::new("forgia_quest_journal")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_min_width(440.0);
            ui.label(
                egui::RichText::new("Journal de quêtes")
                    .size(24.0)
                    .strong()
                    .color(FORGE_CHARBON),
            );
            ui.add_space(8.0);

            if entries.is_empty() {
                ui.label(
                    egui::RichText::new("Aucune quête active. Parle au Maître Forgeron !")
                        .size(15.0)
                        .color(FORGE_CHARBON),
                );
            }

            for (qid, title, objs, completed) in &entries {
                ui.separator();
                ui.label(
                    egui::RichText::new(format!("{} {title}", if *completed { "✔" } else { "•" }))
                        .size(18.0)
                        .strong()
                        .color(if *completed { FORGE_TEAL } else { FORGE_CHARBON }),
                );
                for (label, cur, target) in objs {
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(format!("{label}  {cur}/{target}"))
                                .size(14.0)
                                .color(FORGE_CHARBON),
                        );
                    });
                    let frac = if *target > 0 {
                        *cur as f32 / *target as f32
                    } else {
                        1.0
                    };
                    draw_progress_bar(ui, 400.0, frac, *completed);
                }
                ui.add_space(4.0);
                let is_tracked = tracked.0.as_ref() == Some(qid);
                let btn = egui::Button::new(
                    egui::RichText::new(if is_tracked { "✔ Suivie" } else { "Suivre" })
                        .size(14.0)
                        .strong()
                        .color(FORGE_CHARBON),
                )
                .fill(if is_tracked { FORGE_OR } else { FORGE_METAL_CHAUD })
                .stroke(egui::Stroke::new(2.0, FORGE_CHARBON))
                .corner_radius(egui::CornerRadius::same(8))
                .min_size(egui::vec2(130.0, 28.0));
                if ui.add(btn).clicked() {
                    tracked.0 = Some(qid.clone());
                }
            }

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("(J pour fermer)")
                    .size(12.0)
                    .italics()
                    .color(FORGE_CHARBON),
            );
        });
}

fn draw_quest_tracker(
    mut contexts: EguiContexts,
    catalogue: Res<QuestCatalogue>,
    q: Query<(&QuestLog, &TrackedQuest), With<Player>>,
) {
    let Ok((log, tracked)) = q.single() else {
        return;
    };

    // Quête suivie si toujours valide, sinon 1re active.
    let qid = tracked
        .0
        .clone()
        .filter(|id| {
            log.entries
                .get(id)
                .map(|s| matches!(s.status, QuestStatus::Active | QuestStatus::Completed))
                .unwrap_or(false)
        })
        .or_else(|| log.active_ids().next().cloned());
    let Some(qid) = qid else {
        return;
    };
    let (Some(def), Some(state)) = (catalogue.get(&qid), log.entries.get(&qid)) else {
        return;
    };

    let title = def.title.clone();
    let completed = state.status == QuestStatus::Completed;
    let objs: Vec<ObjView> = def
        .objectives
        .iter()
        .enumerate()
        .map(|(i, o)| (o.label.clone(), state.progress.get(i).copied().unwrap_or(0), o.target))
        .collect();

    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Area::new(egui::Id::new("forgia_quest_tracker"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 80.0))
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(C_BG_DARK)
                .stroke(egui::Stroke::new(2.0, FORGE_OR))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(10);
            frame.show(ui, |ui| {
                ui.set_max_width(250.0);
                ui.label(
                    egui::RichText::new(title)
                        .size(15.0)
                        .strong()
                        .color(FORGE_OR),
                );
                for (label, cur, target) in &objs {
                    let done = *cur >= *target;
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {label}  {cur}/{target}",
                            if done { "✔" } else { "›" }
                        ))
                        .size(13.0)
                        .color(if done { FORGE_TEAL } else { C_TEXT_LIGHT }),
                    );
                }
                if completed {
                    ui.label(
                        egui::RichText::new("→ Retourne voir le Maître Forgeron")
                            .size(12.0)
                            .italics()
                            .color(FORGE_OR),
                    );
                }
            });
        });
}

/// Barre de progression cartoon (chunky outline noir, remplissage couleur).
fn draw_progress_bar(ui: &mut egui::Ui, width: f32, frac: f32, completed: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 14.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6.0, FORGE_METAL_CHAUD);
    let fw = rect.width() * frac.clamp(0.0, 1.0);
    let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fw, rect.height()));
    p.rect_filled(
        fill_rect,
        6.0,
        if completed { FORGE_TEAL } else { FORGE_OR },
    );
    p.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(2.0, C_OUTLINE),
        egui::StrokeKind::Inside,
    );
}
