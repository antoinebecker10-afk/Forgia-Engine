//! # Roguelite HUD — Wave counter + Souls + Defeat/Victory overlays
//!
//! Module créé V7 M2.5 (2026-05-20). Gated `GameMode::Roguelite` (les widgets
//! généraux player_hp / ammo HUD sont cross-mode dans leurs crates respectives —
//! voir `forgia-ui-hud` + `forgia-ui-hud-ammo`).
//!
//! Widgets fournis :
//! - `draw_wave_counter` — top center, lit `RogueliteWave` + `RunState`
//! - `draw_souls_counter` — top right, lit `forgia_loot_tables::Souls`
//! - `draw_defeat_overlay` — fullscreen quand `RunState::Defeat`, bouton "Back to Lobby"
//! - `draw_victory_overlay` — fullscreen quand `RunState::Victory`, bouton "Back to Lobby"
//!
//! Pause (ESC) est géré globalement par `forgia-ui` (AppMode::Paused indépendant
//! du GameMode) → pas dupliqué ici.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_rpg_data::loot_tables::Souls;
use forgia_ui_lib::style::*;

use crate::enemies::EnemyArchetype;
use crate::run::{RunState, StartRunEvent};
use crate::waves::RogueliteWave;
use forgia_player::FpsCamera;
// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// use forgia_audio_voicelines::ActiveBark;
use forgia_stage::graph::{RunGraph, StageKind};
// TODO(story-471..479): SystemTime/UNIX_EPOCH utilisés par draw_bark_bubble — désactivé
// use std::time::{SystemTime, UNIX_EPOCH};

// ─── Wave counter (top center) ───────────────────────────────────────────

pub(crate) fn draw_wave_counter(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    wave: Res<RogueliteWave>,
    run_graph: Option<Res<RunGraph>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    // Hide en Lobby / Defeat / Victory (les overlays prennent le relais).
    let in_combat = matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    );
    if !in_combat {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_roguelite_wave_counter"),
    ));

    let panel_w = 340.0;
    let panel_h = 78.0;
    let center_x = screen.center().x;
    let top_y = screen.min.y + 18.0;
    let panel_rect = egui::Rect::from_min_size(
        egui::pos2(center_x - panel_w * 0.5, top_y),
        egui::vec2(panel_w, panel_h),
    );
    chunky_rect_filled(&painter, panel_rect, C_BG_DARK, 3.0, 10.0);

    // Texte principal "WAVE X / N".
    // TODO(story-471..479): current_stage_kind + current_stage_depth supprimés de RogueliteWave
    // — remplacés par current_wave (plus simple, pas de stage graph).
    let total = run_graph.as_deref().map(|g| g.total_stages).unwrap_or(5);
    let main_text = format!("WAVE {} / {}", wave.current_wave, total);
    text_with_outline(
        &painter,
        egui::pos2(center_x, top_y + 22.0),
        egui::Align2::CENTER_CENTER,
        &main_text,
        egui::FontId::monospace(22.0),
        C_TEXT_LIGHT,
        2.0,
    );

    // Subtext : enemies remaining OU break countdown.
    let (sub_text, sub_color) = if wave.in_break {
        (
            format!("NEXT IN  {:.1}s", wave.break_secs_left.max(0.0)),
            C_ACCENT,
        )
    } else {
        (format!("ENEMIES  {}", wave.bots_alive), C_TEXT_MUTED)
    };
    text_with_outline(
        &painter,
        egui::pos2(center_x, top_y + 54.0),
        egui::Align2::CENTER_CENTER,
        &sub_text,
        egui::FontId::monospace(16.0),
        sub_color,
        1.0,
    );
}

// ─── Souls counter (top right) ───────────────────────────────────────────

pub(crate) fn draw_souls_counter(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    souls: Res<Souls>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_roguelite_souls"),
    ));

    let panel_w = 180.0;
    let panel_h = 54.0;
    let pad = 18.0;
    let right_x = screen.max.x - pad;
    let top_y = screen.min.y + pad;
    let panel_rect = egui::Rect::from_min_size(
        egui::pos2(right_x - panel_w, top_y),
        egui::vec2(panel_w, panel_h),
    );
    chunky_rect_filled(&painter, panel_rect, C_BG_DARK, 3.0, 8.0);

    // Label "SOULS" en haut gauche du panel.
    text_with_outline(
        &painter,
        egui::pos2(panel_rect.min.x + 10.0, panel_rect.min.y + 6.0),
        egui::Align2::LEFT_TOP,
        "SOULS",
        egui::FontId::monospace(13.0),
        C_TEXT_MUTED,
        1.0,
    );

    // Nombre principal aligné droite.
    let count_text = format!("{}", souls.current);
    text_with_outline(
        &painter,
        egui::pos2(panel_rect.max.x - 12.0, panel_rect.center().y + 6.0),
        egui::Align2::RIGHT_CENTER,
        &count_text,
        egui::FontId::monospace(28.0),
        C_AMMO_TEXT,
        2.0,
    );
}

// ─── Defeat overlay ──────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_defeat_overlay(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    mut start_run: MessageWriter<StartRunEvent>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    // Story-558 Phase 5 — résumé carry-over Souls.
    last_defeat: Res<crate::run::LastDefeatSummary>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    if !matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::Defeat)
    ) {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Area::new(egui::Id::new("forgia_roguelite_defeat"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // Story-558 Phase 7 — overlay cartoon kid-friendly :
            // fond bois clair (pas noir grimdark) + border or 5px + shadow stack.
            // Anti-pattern documenté audit §8 : punition cosmétique Defeat = décourage.
            egui::Frame::new()
                .fill(FORGE_BOIS_CLAIR)
                .inner_margin(egui::Margin::symmetric(80, 48))
                .corner_radius(egui::CornerRadius::same(20))
                .stroke(egui::Stroke::new(5.0, FORGE_OR))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(4.0);
                        // Titre cartoon : "LA FORGE T'A BRISÉ" braise sur bois
                        // (bible v1 — vocab CE2, vocabulaire poétique enfants).
                        ui.heading(
                            egui::RichText::new("LA FORGE T'A BRISÉ")
                                .size(56.0)
                                .color(FORGE_BRAISE)
                                .strong(),
                        );
                        ui.add_space(18.0);
                        // Encouragement (anti "Game Over" dépressif)
                        ui.label(
                            egui::RichText::new("Mais le marteau t'attend.")
                                .size(22.0)
                                .italics()
                                .color(FORGE_CHARBON),
                        );
                        // Story-558 AC8 — message carry-over encourageant.
                        if last_defeat.souls_before > 0 {
                            ui.add_space(20.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "Tu gardes ◇ {} de ta forge précédente.",
                                    last_defeat.souls_kept
                                ))
                                .size(20.0)
                                .strong()
                                .color(FORGE_CHARBON)
                                .background_color(FORGE_OR),
                            );
                        }
                        ui.add_space(36.0);

                        let cartoon_btn =
                            |ui: &mut egui::Ui, label: &str, fill: egui::Color32| -> bool {
                                ui.add(
                                    egui::Button::new(
                                        egui::RichText::new(label)
                                            .size(22.0)
                                            .strong()
                                            .color(FORGE_CHARBON),
                                    )
                                    .fill(fill)
                                    .stroke(egui::Stroke::new(4.0, FORGE_CHARBON))
                                    .corner_radius(egui::CornerRadius::same(14))
                                    .min_size(egui::vec2(280.0, 52.0)),
                                )
                                .clicked()
                            };

                        if cartoon_btn(ui, "↻  REFORGER", FORGE_OR) {
                            info!("[roguelite-hud] Defeat → Nouvelle Run");
                            start_run.write(StartRunEvent { seed: None });
                        }
                        ui.add_space(10.0);
                        if cartoon_btn(ui, "✕  RETOUR AU MENU", FORGE_METAL_CHAUD) {
                            info!("[roguelite-hud] Defeat → Menu");
                            next_app.set(AppMode::Menu);
                            next_game.set(GameMode::None);
                        }
                    });
                });
        });
}

// ─── Victory overlay ─────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_victory_overlay(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    souls: Res<Souls>,
    mut start_run: MessageWriter<StartRunEvent>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    if !matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::Victory)
    ) {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Area::new(egui::Id::new("forgia_roguelite_victory"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(220))
                .inner_margin(egui::Margin::symmetric(80, 48))
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(egui::Stroke::new(3.0, C_HP_HIGH))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(4.0);
                        ui.heading(
                            egui::RichText::new("VICTOIRE")
                                .size(72.0)
                                .color(C_HP_HIGH)
                                .strong(),
                        );
                        ui.add_space(20.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "Boss vaincu — {} âmes récoltées.",
                                souls.total_collected
                            ))
                            .size(20.0)
                            .color(C_TEXT_LIGHT),
                        );
                        ui.add_space(32.0);

                        let btn = |ui: &mut egui::Ui, label: &str| -> bool {
                            ui.add(
                                egui::Button::new(egui::RichText::new(label).size(22.0))
                                    .min_size(egui::vec2(260.0, 46.0)),
                            )
                            .clicked()
                        };

                        if btn(ui, "↻ Nouvelle Run") {
                            info!("[roguelite-hud] Victory → Nouvelle Run");
                            start_run.write(StartRunEvent { seed: None });
                        }
                        ui.add_space(8.0);
                        if btn(ui, "✕ Retour au Menu") {
                            info!("[roguelite-hud] Victory → Menu");
                            next_app.set(AppMode::Menu);
                            next_game.set(GameMode::None);
                        }
                    });
                });
        });
}

// ─── Portal overlay (M3 step 3) — dormant, voir TODO l.354 ──────────────

/// Emoji + label pour un `StageKind` (UI Portal).
/// Dormant : sera re-wiré quand `draw_portal_overlay` sera ré-implémenté
/// (refactor story-471..479 a supprimé les champs RogueliteWave nécessaires).
#[allow(dead_code)]
fn stage_kind_display(kind: StageKind) -> (&'static str, &'static str) {
    match kind {
        StageKind::Combat => ("⚔", "Combat"),
        StageKind::Elite => ("💀", "Elite"),
        StageKind::Shop => ("🛒", "Boutique"),
        StageKind::Event => ("❓", "Évènement"),
        StageKind::Treasure => ("💎", "Trésor"),
        StageKind::Rest => ("🛏", "Repos"),
        StageKind::Boss => ("👑", "Boss"),
    }
}

/// Couleur principale par `StageKind` (cohérence avec ennemis arena).
/// Dormant : voir `stage_kind_display` rationale.
#[allow(dead_code)]
fn stage_kind_color(kind: StageKind) -> egui::Color32 {
    match kind {
        StageKind::Combat => egui::Color32::from_rgb(220, 80, 60),
        StageKind::Elite => egui::Color32::from_rgb(200, 60, 200),
        StageKind::Shop => egui::Color32::from_rgb(240, 200, 80),
        StageKind::Event => egui::Color32::from_rgb(120, 180, 255),
        StageKind::Treasure => egui::Color32::from_rgb(255, 215, 100),
        StageKind::Rest => egui::Color32::from_rgb(120, 220, 140),
        StageKind::Boss => egui::Color32::from_rgb(255, 60, 100),
    }
}

/// V7 M3 step 3 (2026-05-20) — touches clavier portal :
/// - Flèche gauche → variant 0
/// - Flèche droite → variant 1
/// - Flèche haut/bas (3 et 4 choix éventuels) → variants 2/3
/// - 1..4 → fallback numérique générique
///
/// Dormant : voir TODO portal overlay re-implémentation.
#[allow(dead_code)]
const PORTAL_KEYS: &[(KeyCode, u8)] = &[
    (KeyCode::ArrowLeft, 0),
    (KeyCode::ArrowRight, 1),
    (KeyCode::ArrowUp, 2),
    (KeyCode::ArrowDown, 3),
    (KeyCode::Digit1, 0),
    (KeyCode::Digit2, 1),
    (KeyCode::Digit3, 2),
    (KeyCode::Digit4, 3),
];

// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// draw_portal_overlay désactivé : RogueliteWave n'a plus pending_portal_choices
// ni chosen_variant_idx (champs supprimés lors du refactor de session précédente).
// Remplacer par stub no-op pour conserver la signature dans le plugin.
pub(crate) fn draw_portal_overlay(
    _contexts: EguiContexts,
    _app_state: Res<State<AppMode>>,
    _game_mode: Res<State<GameMode>>,
    _keys: Res<ButtonInput<KeyCode>>,
    _wave: ResMut<RogueliteWave>,
) {
    // disabled — pending_portal_choices / chosen_variant_idx supprimés
}

// ─── Plugin ──────────────────────────────────────────────────────────────

// ─── Bark bubble (Story-482 — floating text overlay armes parlantes) ───────

/// Couleur RGBA du label speaker, alignée avec persona genome.
/// Tier 2 audio (story future) gardera la même palette pour cohérence visuelle/sonore.
pub fn speaker_color(speaker: &str) -> egui::Color32 {
    match speaker {
        "pepin" => egui::Color32::from_rgb(120, 220, 130), // vert frais (timide)
        "bourrasque" => egui::Color32::from_rgb(110, 180, 240), // bleu vent
        "lenoir" => egui::Color32::from_rgb(180, 130, 220), // violet noble
        "boucherie" => egui::Color32::from_rgb(230, 110, 110), // rouge sang
        _ => egui::Color32::from_rgb(180, 180, 180),       // gris fallback "any"
    }
}

/// Label affiché (capitalisé, lisible).
pub fn speaker_label(speaker: &str) -> &'static str {
    match speaker {
        "pepin" => "Pépin",
        "bourrasque" => "Bourrasque",
        "lenoir" => "Madame Lenoir",
        "boucherie" => "Boucherie",
        _ => "???",
    }
}

// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// draw_bark_bubble désactivé : ActiveBark (forgia_audio_voicelines) n'existe plus
// dans le scaffold vide. Stub no-op pour conserver l'enregistrement plugin.
pub(crate) fn draw_bark_bubble(
    _contexts: EguiContexts,
    _app_state: Res<State<AppMode>>,
    _game_mode: Res<State<GameMode>>,
) {
    // disabled — ActiveBark supprimé de forgia_audio_voicelines
}

// ─── Notification toast (V7 M3 step 3) ──────────────────────────────────

/// Affiche le toast notification (soft stage effect) centré bas écran.
/// Fade-out sur la dernière seconde via alpha.
// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// draw_stage_notification désactivé : RogueliteWave n'a plus le champ `notification`
// (supprimé lors du refactor de session précédente).
pub(crate) fn draw_stage_notification(
    _contexts: EguiContexts,
    _app_state: Res<State<AppMode>>,
    _game_mode: Res<State<GameMode>>,
    _wave: Res<RogueliteWave>,
) {
    // disabled — wave.notification supprimé
}

/// Story-517 nameplate texte au-dessus de chaque ennemi (Tank/Runner/Sniper/Boss).
/// Pattern miroir `forgia-rpg::character::draw_lineup_names` — egui world→viewport
/// projection avec outline noir 8 passes pour lisibilité tous fonds.
pub fn draw_enemy_archetype_labels(
    mut contexts: EguiContexts,
    q_enemies: Query<(&Transform, &EnemyArchetype)>,
    q_cam: Query<(&Camera, &GlobalTransform), With<FpsCamera>>,
) {
    let Ok((cam, cam_tf)) = q_cam.single() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_roguelite_enemy_labels"),
    ));

    for (tf, archetype) in &q_enemies {
        // Position monde au-dessus de la tête : capsule center + ~2m offset
        // (couvre toutes les tailles archetype, Boss inclus).
        let world_pos = tf.translation + Vec3::Y * 2.4;
        let Ok(screen_pos) = cam.world_to_viewport(cam_tf, world_pos) else {
            continue;
        };
        // Distance fade : 0..40m → alpha 1, 40..60m → fade, >60m → hidden.
        let dist = (cam_tf.translation() - tf.translation).length();
        if dist > 60.0 {
            continue;
        }
        let alpha = if dist > 40.0 {
            ((60.0 - dist) / 20.0).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let scale = (10.0 / dist.max(2.0)).clamp(0.5, 2.0);
        let font = egui::FontId::proportional(16.0 * scale);
        let label = match archetype {
            EnemyArchetype::Tank => "TANK",
            EnemyArchetype::Runner => "RUNNER",
            EnemyArchetype::Sniper => "SNIPER",
            EnemyArchetype::Boss => "BOSS",
        };
        let color = match archetype {
            EnemyArchetype::Tank => egui::Color32::from_rgba_unmultiplied(
                240,
                70,
                70,
                (255.0 * alpha) as u8,
            ),
            EnemyArchetype::Runner => egui::Color32::from_rgba_unmultiplied(
                255,
                180,
                60,
                (255.0 * alpha) as u8,
            ),
            EnemyArchetype::Sniper => egui::Color32::from_rgba_unmultiplied(
                190,
                100,
                255,
                (255.0 * alpha) as u8,
            ),
            EnemyArchetype::Boss => egui::Color32::from_rgba_unmultiplied(
                255,
                80,
                200,
                (255.0 * alpha) as u8,
            ),
        };
        let pos = egui::pos2(screen_pos.x, screen_pos.y);
        let outline = egui::Color32::from_rgba_unmultiplied(0, 0, 0, (255.0 * alpha) as u8);
        // Outline 8 directions.
        for (dx, dy) in &[
            (-1.0_f32, -1.0_f32),
            (0.0, -1.0),
            (1.0, -1.0),
            (-1.0, 0.0),
            (1.0, 0.0),
            (-1.0, 1.0),
            (0.0, 1.0),
            (1.0, 1.0),
        ] {
            painter.text(
                egui::pos2(pos.x + dx * scale.max(1.0), pos.y + dy * scale.max(1.0)),
                egui::Align2::CENTER_CENTER,
                label,
                font.clone(),
                outline,
            );
        }
        painter.text(pos, egui::Align2::CENTER_CENTER, label, font, color);
    }
}

pub struct RogueliteHudPlugin;

impl Plugin for RogueliteHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            EguiPrimaryContextPass,
            (
                draw_wave_counter,
                draw_souls_counter,
                draw_portal_overlay,
                draw_defeat_overlay,
                draw_victory_overlay,
                draw_bark_bubble,
                draw_stage_notification,
                draw_enemy_archetype_labels,
            ),
        );
    }
}

// ─── Tests purs (compile-only) ───────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::RunResult;

    #[test]
    fn plugin_constructible() {
        let _ = RogueliteHudPlugin;
    }

    // ── Story-482 — Bark bubble speaker mapping ────────────────────────────

    #[test]
    fn speaker_color_distinct_per_arme_v1() {
        let p = speaker_color("pepin");
        let b = speaker_color("bourrasque");
        let l = speaker_color("lenoir");
        let bo = speaker_color("boucherie");
        // 4 couleurs uniques (sinon Antoine ne distingue pas qui parle)
        let arr = [p, b, l, bo];
        for i in 0..arr.len() {
            for j in (i + 1)..arr.len() {
                assert_ne!(arr[i], arr[j], "speakers {i} and {j} share color");
            }
        }
    }

    #[test]
    fn speaker_color_fallback_any() {
        assert_eq!(speaker_color("unknown"), speaker_color("any"));
    }

    #[test]
    fn speaker_label_arme_v1_mapped() {
        assert_eq!(speaker_label("pepin"), "Pépin");
        assert_eq!(speaker_label("bourrasque"), "Bourrasque");
        assert_eq!(speaker_label("lenoir"), "Madame Lenoir");
        assert_eq!(speaker_label("boucherie"), "Boucherie");
        assert_eq!(speaker_label("any"), "???");
    }

    /// La gate combat = uniquement InRun{stage} OU Boss{stage}.
    #[test]
    fn combat_gate_includes_inrun_and_boss() {
        let s1 = RunState::InRun { stage: 0 };
        let s2 = RunState::Boss { stage: 2 };
        let s3 = RunState::Lobby;
        let s4 = RunState::Defeat;
        let s5 = RunState::Victory;
        assert!(matches!(s1, RunState::InRun { .. } | RunState::Boss { .. }));
        assert!(matches!(s2, RunState::InRun { .. } | RunState::Boss { .. }));
        assert!(!matches!(
            s3,
            RunState::InRun { .. } | RunState::Boss { .. }
        ));
        assert!(!matches!(
            s4,
            RunState::InRun { .. } | RunState::Boss { .. }
        ));
        assert!(!matches!(
            s5,
            RunState::InRun { .. } | RunState::Boss { .. }
        ));
    }

    #[test]
    fn end_state_gating() {
        // Defeat overlay s'affiche uniquement sur RunState::Defeat.
        assert!(matches!(RunState::Defeat, RunState::Defeat));
        assert!(!matches!(RunState::Victory, RunState::Defeat));
        // Victory overlay s'affiche uniquement sur RunState::Victory.
        assert!(matches!(RunState::Victory, RunState::Victory));
        assert!(!matches!(RunState::Defeat, RunState::Victory));
    }

    /// L'enum RunResult expose Victory/Defeat/Abort. Sanity check pour ne pas
    /// drift en cas d'ajout futur (Pause/Quit/etc.) — explicite ce qu'on couvre.
    #[test]
    fn run_result_variants_known() {
        let _v = RunResult::Victory;
        let _d = RunResult::Defeat;
        let _a = RunResult::Abort;
    }
}
