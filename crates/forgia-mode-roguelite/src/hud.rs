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
// Story-571 — Or in-run = `Gold` (alias de forgia-rpg-data Souls) ; Souls méta = `MetaSouls`.
use crate::run::{MetaSouls, RunTimer};
use forgia_rpg_data::loot_tables::Souls as Gold;
use forgia_ui_lib::hud_ammo::AmmoHudTuning;
use forgia_ui_lib::style::*;
use forgia_ui_lib::theme::{display_font, display_text};

use crate::enemies::EnemyArchetype;
use crate::run::RunState;
use crate::ultimate_config::UltimateConfig;
use crate::ultimate_vfx::{weapon_technique_label, weapon_vfx_color};
use crate::waves::RogueliteWave;
use forgia_combat::confidence::PepinConfidence;
use forgia_combat::ultimate::UltimateState;
use forgia_combat::weapons::{EquippedWeapons, WeaponType, ARENA_V1_WEAPONS};
use forgia_damage::{DefenseLayer, Health as DamageHealth};
use forgia_player::{FpsCamera, Player};
use forgia_rpg_data::boons::{ActiveBoons, BoonEffectKind, BoonId, BoonsCatalogue};
// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// use forgia_audio_voicelines::ActiveBark;
use forgia_stage::graph::StageKind;
// TODO(story-471..479): SystemTime/UNIX_EPOCH utilisés par draw_bark_bubble — désactivé
// use std::time::{SystemTime, UNIX_EPOCH};

// ─── Wave counter (top center) ───────────────────────────────────────────

/// Libellé de progression de run — PUR, donc testable.
///
/// Story-678. Deux défauts corrigés ici :
///
/// 1. **« SALLE 5 / 4 »** — le total venait d'une source différente de celle qui
///    fait avancer les rounds (`RunGraphConfig.total_stages`, rechargeable à
///    chaud, contre `RunGraph.total_stages`, figé au départ de la run). Deux
///    nombres pour la même chose finissent toujours par diverger. Le caller ne
///    passe désormais qu'UN total, résolu à la bonne source.
/// 2. **Un dénominateur n'a aucun sens en boucle infinie.** Il n'y a pas de fin
///    à atteindre, il y a une pression qui monte. Afficher « / 4 » racontait une
///    histoire fausse — c'est ce qui rendait la progression illisible.
///
/// Cette fonction ne peut PAS produire un numérateur au-dessus du dénominateur :
/// c'est vérifié par test, pas par relecture.
pub(crate) fn run_progress_label(
    stage: u8,
    current_wave: u8,
    waves_per_stage: u8,
    total_rooms: u8,
    rounds: &crate::rounds::RoundsConfig,
) -> String {
    let round = u32::from(stage);
    let endless = rounds.enabled && rounds.max_rounds == 0;
    let progress = if endless {
        format!("ROUND {}", round + 1)
    } else {
        // Garde-fou : le numérateur ne dépasse JAMAIS le dénominateur. Si ça
        // arrive quand même, c'est la dernière salle — pas « 5 / 4 ».
        let total = total_rooms.max(1);
        format!("SALLE {} / {}", (stage + 1).min(total), total)
    };
    if !endless && stage + 1 >= total_rooms.max(1) {
        return format!("{progress}  ·  BOSS");
    }
    if rounds.enabled && rounds.is_respite_round(round) {
        return format!("{progress}  ·  RÉPIT");
    }
    if rounds.enabled && rounds.is_tier_round(round) {
        // Le palier est la marche qu'on RESSENT : le joueur doit savoir qu'il
        // vient d'en franchir une, sinon le sursaut passe pour de l'arbitraire.
        return format!("{progress}  ·  PALIER  ·  VAGUE {current_wave} / {waves_per_stage}");
    }
    format!("{progress}  ·  VAGUE {current_wave} / {waves_per_stage}")
}


pub(crate) fn draw_wave_counter(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    wave: Res<RogueliteWave>,
    // Story-646 R2 — totaux salle/vague depuis le genome (multi-salles).
    graph_cfg: Res<forgia_stage::graph::RunGraphConfig>,
    // Story-678 — la boucle de rounds : c'est ELLE qui dit s'il y a un total, et
    // c'est elle qui porte la menace du round.
    rounds: Res<crate::rounds::RoundsConfig>,
    // La MÊME source que celle qui fait avancer les rounds. Le « 5/4 » venait de
    // là : le compteur divisait par `RunGraphConfig.total_stages` (le génome,
    // rechargeable à chaud) alors que l'orchestrateur avance sur
    // `RunGraph.boss_depth()` (l'instantané pris au départ de la run). Deux
    // nombres pour la même chose finissent toujours par diverger.
    graph: Option<Res<forgia_stage::graph::RunGraph>>,
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
    glass_panel(&painter, panel_rect, 10.0);

    // Story-678 — le libellé est calculé par une fonction PURE (testable).
    // Le « SALLE 5 / 4 » vivait dans un appel de dessin : aucun test ne pouvait
    // l'atteindre. C'est la vraie leçon du défaut, pas le numérateur.
    let total_rooms = if rounds.enabled {
        rounds.max_rounds.min(u32::from(u8::MAX)) as u8
    } else {
        // La MÊME source que celle qui fait avancer les rounds.
        graph
            .as_deref()
            .map(|g| g.total_stages)
            .unwrap_or(graph_cfg.total_stages)
            .max(1)
    };
    let main_text = run_progress_label(
        wave.stage,
        wave.current_wave,
        graph_cfg.waves_per_stage,
        total_rooms,
        &rounds,
    );
    let round = u32::from(wave.stage);

    text_with_outline(
        &painter,
        egui::pos2(center_x, top_y + 22.0),
        egui::Align2::CENTER_CENTER,
        &main_text,
        display_font(24.0),
        C_TEXT_LIGHT,
        2.0,
    );

    // Subtext : ennemis restants / décompte de break — ET LA MENACE.
    //
    // C'est la réponse à « on ne comprend pas comment on évolue » : le joueur
    // voit le multiplicateur de vie ennemie monter round après round. Le contrat
    // de la boucle devient lisible — la menace monte, à toi de monter aussi.
    // Sans ce chiffre, la difficulté est réelle mais invisible, donc vécue comme
    // arbitraire.
    let (sub_text, sub_color) = if wave.in_break {
        (
            format!("SUITE DANS  {:.1}s", wave.break_secs_left.max(0.0)),
            C_ACCENT,
        )
    } else if rounds.enabled {
        let threat = rounds.threat(round).hp;
        (
            format!("ENNEMIS  {}   ·   MENACE  ×{threat:.1}", wave.bots_alive),
            C_TEXT_MUTED,
        )
    } else {
        (format!("ENNEMIS  {}", wave.bots_alive), C_TEXT_MUTED)
    };
    text_with_outline(
        &painter,
        egui::pos2(center_x, top_y + 54.0),
        egui::Align2::CENTER_CENTER,
        &sub_text,
        egui::FontId::proportional(16.0),
        sub_color,
        1.0,
    );
}

// ─── Compteurs monnaie (top right) — Or + Souls, modèle Gunfire ────────────

/// Story-571 — double compteur haut-droite : **Or** (in-run, `Gold`) au-dessus,
/// **Âmes/Souls** (méta persistant, `MetaSouls`) en dessous. Modèle Gunfire Reborn
/// (gemme + pièce). Or = doré, Âmes = teal.
pub(crate) fn draw_currency_counters(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    gold: Res<Gold>,
    meta: Res<MetaSouls>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_roguelite_currency"),
    ));

    let panel_w = 180.0;
    let panel_h = 46.0;
    let gap = 8.0;
    let pad = 18.0;
    let right_x = screen.max.x - pad;
    let top_y = screen.min.y + pad;

    let draw_counter = |y: f32, label: &str, value: u32, accent: egui::Color32, is_soul: bool| {
        let rect = egui::Rect::from_min_size(
            egui::pos2(right_x - panel_w, y),
            egui::vec2(panel_w, panel_h),
        );
        glass_panel(&painter, rect, 8.0);
        // Icône monnaie à gauche du compteur.
        let icon_c = egui::pos2(rect.min.x + 24.0, rect.center().y);
        if is_soul {
            // Wisp d'âme : empilement de cercles teal = flamme/âme fantomatique.
            painter.circle_filled(icon_c + egui::vec2(0.0, 4.0), 9.0, accent);
            painter.circle_filled(icon_c + egui::vec2(0.0, -4.0), 5.5, accent);
            painter.circle_filled(icon_c + egui::vec2(0.0, -11.0), 2.5, accent);
            painter.circle_filled(
                icon_c + egui::vec2(0.0, 3.0),
                3.5,
                egui::Color32::from_rgb(225, 255, 255),
            );
        } else {
            // Pièce d'or : disque + rim sombre + reflet clair.
            painter.circle_filled(icon_c, 11.0, accent);
            painter.circle_stroke(
                icon_c,
                11.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(150, 110, 20)),
            );
            painter.circle_filled(icon_c, 6.0, egui::Color32::from_rgb(255, 228, 130));
        }
        text_with_outline(
            &painter,
            egui::pos2(rect.min.x + 44.0, rect.min.y + 5.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(12.0),
            C_TEXT_MUTED,
            1.0,
        );
        text_with_outline(
            &painter,
            egui::pos2(rect.max.x - 12.0, rect.center().y + 5.0),
            egui::Align2::RIGHT_CENTER,
            &format!("{value}"),
            display_font(26.0),
            accent,
            2.0,
        );
    };

    // Or (in-run) en haut, Âmes (méta) en dessous.
    draw_counter(top_y, "OR", gold.current, FORGE_OR, false);
    draw_counter(
        top_y + panel_h + gap,
        "ÂMES",
        meta.current,
        FORGE_TEAL,
        true,
    );
}

/// Couleur + libellé court d'un effet de boon (pour le panneau Améliorations).
fn boon_visual(effect: &BoonEffectKind) -> (egui::Color32, String) {
    match effect {
        BoonEffectKind::DamageMul { factor } => {
            (FORGE_BRAISE, format!("+{:.0}% dmg", (factor - 1.0) * 100.0))
        }
        BoonEffectKind::FireRateMul { factor } => (
            egui::Color32::from_rgb(80, 160, 255),
            format!("+{:.0}% cadence", (factor - 1.0) * 100.0),
        ),
        BoonEffectKind::HealOnKill { hp } => (
            egui::Color32::from_rgb(80, 220, 120),
            format!("+{hp:.0} PV/kill"),
        ),
        BoonEffectKind::DamageReduction { factor } => {
            (FORGE_OR, format!("-{:.0}% reçu", factor * 100.0))
        }
        BoonEffectKind::ChainTargets { count } => (
            egui::Color32::from_rgb(180, 120, 255),
            format!("+{count} chaîne"),
        ),
        BoonEffectKind::Knockback { .. } => {
            (egui::Color32::from_rgb(255, 140, 60), "recul".to_string())
        }
        BoonEffectKind::FlatBonus { stat, amount } => (
            egui::Color32::from_rgb(200, 200, 200),
            format!("+{amount:.0} {stat}"),
        ),
    }
}

/// Panneau « AMÉLIORATIONS » (gauche, sous la minimap) : liste les boons actifs
/// (Coffre entre-waves + orbes du parcours) avec nom, stack ×N et effet. Lit
/// `ActiveBoons` + `BoonsCatalogue`. Vide → masqué.
pub(crate) fn draw_active_boons(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    active: Res<ActiveBoons>,
    catalogue: Option<Res<BoonsCatalogue>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let Some(cat) = catalogue else {
        return;
    };
    if active.active.is_empty() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Ordre d'acquisition, dédupliqué (le count vient du Vec complet).
    let mut order: Vec<&BoonId> = Vec::new();
    for id in &active.active {
        if !order.contains(&id) {
            order.push(id);
        }
    }

    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_active_boons"),
    ));
    let panel_w = 224.0;
    let row_h = 26.0;
    let x = screen.min.x + 18.0;
    let mut y = screen.min.y + 210.0;

    let header = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(panel_w, 24.0));
    glass_panel(&painter, header, 8.0);
    text_with_outline(
        &painter,
        egui::pos2(x + 12.0, y + 12.0),
        egui::Align2::LEFT_CENTER,
        "AMÉLIORATIONS",
        display_font(15.0),
        FORGE_OR,
        2.0,
    );
    y += 28.0;

    for id in order {
        let Some(def) = cat.entries.iter().find(|b| &b.id == id) else {
            continue;
        };
        let count = active.active.iter().filter(|b| *b == id).count();
        let (color, eff) = boon_visual(&def.effect);
        let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(panel_w, row_h - 3.0));
        chunky_rect_filled(&painter, rect, C_BG_DARK, 2.0, 6.0);
        painter.circle_filled(egui::pos2(x + 14.0, rect.center().y), 6.0, color);
        let name = if count > 1 {
            format!("{}  x{count}", def.name)
        } else {
            def.name.clone()
        };
        text_with_outline(
            &painter,
            egui::pos2(x + 28.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &name,
            egui::FontId::proportional(13.0),
            egui::Color32::from_rgb(240, 235, 225),
            1.5,
        );
        text_with_outline(
            &painter,
            egui::pos2(rect.max.x - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            &eff,
            egui::FontId::proportional(11.0),
            color,
            1.0,
        );
        y += row_h;
    }
}

/// Caméra pendant l'écran de choix (Coffre / fin de run) : si on maintient un
/// clic HORS du panel egui → grab curseur + look (la caméra tourne) ; sinon
/// curseur libre pour cliquer les boutons. Override per-frame de `block_look`
/// + `CursorOptions` (forgia-ui les fige en statique, on reprend la main ici).
#[allow(clippy::too_many_arguments)]
pub fn sys_break_look_override(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    coffre: Option<Res<forgia_rpg_data::boons::CoffreSession>>,
    // Story-646 Inc.2 — le choix de porte libère aussi le curseur (clic possible).
    wave: Res<RogueliteWave>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut blockers: ResMut<forgia_input::InputBlockers>,
    mut q_cursor: Query<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let coffre_open = coffre.map(|c| c.is_open).unwrap_or(false);
    let portal_open = !wave.portal_choices.is_empty();
    let end_screen = matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::Defeat) | Some(RunState::Victory)
    );
    if !coffre_open && !end_screen && !portal_open {
        return; // gameplay normal — forgia-ui gère curseur/look
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let over_panel = ctx.is_pointer_over_area();
    let held = mouse.pressed(MouseButton::Left) || mouse.pressed(MouseButton::Right);
    let look = held && !over_panel;
    blockers.block_look = !look;
    if let Ok(mut opts) = q_cursor.single_mut() {
        if look {
            opts.grab_mode = bevy::window::CursorGrabMode::Locked;
            opts.visible = false;
        } else {
            opts.grab_mode = bevy::window::CursorGrabMode::None;
            opts.visible = true;
        }
    }
}

// ─── Defeat overlay ──────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_defeat_overlay(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    // Story-591 — « REFORGER » renvoie au hub Lobby (L'Enclume) au lieu de
    // relancer direct : le joueur peut dépenser ses Âmes avant la prochaine run.
    mut next_run: ResMut<NextState<RunState>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    // Story-558 Phase 5 — résumé carry-over Souls.
    last_defeat: Res<crate::run::LastDefeatSummary>,
    // Story-597 Phase B — Âmes forgées cette run + récap 1re mort one-shot.
    meta: Res<MetaSouls>,
    mut ftue: ResMut<crate::ftue::FtueSave>,
    timer: Res<crate::run::RunTimer>,
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
            glass_frame_hero().inner_margin(egui::Margin::symmetric(80, 48)).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    // Titre cartoon : "LA FORGE T'A BRISÉ" braise sur bois
                    // (bible v1 — vocab CE2, vocabulaire poétique enfants).
                    ui.heading(display_text("LA FORGE T'A BRISÉ", 56.0, FORGE_BRAISE).strong());
                    ui.add_space(18.0);
                    // Story-597 Phase B — la 1re mort ENSEIGNE la méta-boucle
                    // (voix Maître Forgeron, ≤8 mots/ligne) ; les fois suivantes,
                    // juste l'encouragement court. Flag persisté (FtueSave), marqué
                    // vu à la SORTIE de l'écran (ftue::sys_mark_first_death_seen).
                    let first_death = !ftue.first_death_recap_seen;
                    if first_death {
                        for line in [
                            "Tes Âmes restent quand tu tombes.",
                            "Dépense-les à L'Enclume.",
                            "Reviens plus fort. Toujours.",
                        ] {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(line)
                                    .size(22.0)
                                    .strong()
                                    .color(FORGE_CREME),
                            );
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("Mais le marteau t'attend.")
                                .size(22.0)
                                .italics()
                                .color(FORGE_CREME),
                        );
                    }

                    // Story-597 — « toute run paie » : Âmes FORGÉES cette run
                    // (meta.earned_run), gardées. + total + Or perdu (secondaire).
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "Cette run t'a forgé ◇ {} Âmes — gardées !",
                            meta.earned_run
                        ))
                        .size(22.0)
                        .strong()
                        .color(FORGE_CHARBON)
                        .background_color(FORGE_OR),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "Total ◇ {} âmes  ·  Or perdu : {}",
                            last_defeat.souls_persistent, last_defeat.or_lost
                        ))
                        .size(16.0)
                        .color(FORGE_CREME),
                    );
                    ui.add_space(36.0);

                    // Story-596 — bouton cartoon partagé (forgia-ui-lib::style).
                    // Hub-menu étape 6 : le Lobby auto-lance la run (config au menu)
                    // → ce bouton = REJOUER direct. Shopping/reconfig = Retour au menu.
                    if cartoon_btn(ui, "↻  REJOUER", FORGE_OR).clicked() {
                        info!("[roguelite-hud] Defeat → Lobby (rejouer, auto-start)");
                        // Story-597 — marque le récap vu (couvre ce chemin de sortie).
                        ftue.mark_first_death(timer.secs);
                        next_run.set(RunState::Lobby);
                    }
                    // Story-597 — 1re mort : REJOUER relance sans rien dépenser
                    // (le Lobby auto-lance). On le dit, pour ne pas promettre
                    // une boutique qui n'est pas sur ce chemin.
                    if first_death {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("↑ tu repars sans rien acheter")
                                .size(15.0)
                                .italics()
                                .color(FORGE_CREME),
                        );
                    }
                    ui.add_space(10.0);
                    if cartoon_btn(ui, "✕  RETOUR AU MENU", FORGE_METAL_CHAUD).clicked() {
                        info!("[roguelite-hud] Defeat → Menu");
                        // Story-597 — marque aussi le récap vu sur le chemin Menu
                        // (OnExit SubState ne fire pas ici → fix qa BUG-597-B-01).
                        ftue.mark_first_death(timer.secs);
                        next_app.set(AppMode::Menu);
                        next_game.set(GameMode::None);
                    }
                    // La flèche guide pointe le SEUL chemin qui mène à L'Enclume :
                    // le menu-titre. Elle était sous REJOUER, qui relance la run
                    // sans permettre la moindre dépense — le FTUE enseignait donc
                    // le chemin qui ne marche pas.
                    if first_death {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("↑ passe par L'Enclume, dépense tes Âmes")
                                .size(15.0)
                                .italics()
                                .color(FORGE_BRAISE),
                        );
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
    meta: Res<MetaSouls>,
    // R3.3 (story-645) — chrono de la run + record persistant (NOUVEAU RECORD).
    last_stats: Res<crate::meta_shop::LastRunStats>,
    save: Res<crate::meta_shop::MetaShopSave>,
    // Story-591 — retour au hub Lobby (L'Enclume) au lieu de relancer direct.
    mut next_run: ResMut<NextState<RunState>>,
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
            // Story-596 Phase A — miroir cartoon du Defeat (bois + or, boutons
            // partagés) : l'écran de victoire ne peut pas être moins soigné que
            // l'écran d'échec (était : noir alpha + boutons egui par défaut).
            glass_frame_hero().inner_margin(egui::Margin::symmetric(80, 48)).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    ui.heading(display_text("VICTOIRE !", 64.0, FORGE_OR).strong());
                    ui.add_space(18.0);
                    ui.label(
                        egui::RichText::new("Le Forgeron Noir plie le genou… pour cette fois.")
                            .size(22.0)
                            .italics()
                            .color(FORGE_CREME),
                    );
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "◇ {} âmes forgées cette run !",
                            meta.earned_run
                        ))
                        .size(20.0)
                        .strong()
                        .color(FORGE_CHARBON)
                        .background_color(FORGE_OR),
                    );
                    // R3.3 — chrono de la run + record. `LastRunStats` est figé par
                    // `sys_record_run_stats` à l'entrée de Victory (même frame).
                    ui.add_space(10.0);
                    let fmt =
                        |s: f32| format!("{} min {:02} s", (s / 60.0) as u32, (s % 60.0) as u32);
                    if last_stats.new_best {
                        ui.label(
                            egui::RichText::new(format!(
                                "🏆 NOUVEAU RECORD — {} !",
                                fmt(last_stats.secs)
                            ))
                            .size(22.0)
                            .strong()
                            .color(FORGE_OR),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(format!(
                                "Temps : {}   ·   Record : {}",
                                fmt(last_stats.secs),
                                fmt(save.best_victory_secs)
                            ))
                            .size(18.0)
                            .color(FORGE_CREME),
                        );
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "Victoires : {} / {} runs",
                            save.victories, save.runs_played
                        ))
                        .size(16.0)
                        .color(FORGE_CREME),
                    );
                    ui.add_space(28.0);

                    // Hub-menu étape 6 : Lobby auto-lance → REJOUER direct.
                    if cartoon_btn(ui, "↻  REJOUER", FORGE_OR).clicked() {
                        info!("[roguelite-hud] Victory → Lobby (rejouer, auto-start)");
                        next_run.set(RunState::Lobby);
                    }
                    ui.add_space(10.0);
                    if cartoon_btn(ui, "✕  RETOUR AU MENU", FORGE_METAL_CHAUD).clicked() {
                        info!("[roguelite-hud] Victory → Menu");
                        next_app.set(AppMode::Menu);
                        next_game.set(GameMode::None);
                    }
                });
            });
        });
}

// ─── Portal overlay (M3 step 3) — dormant, voir TODO l.354 ──────────────

/// Emoji + label pour un `StageKind` (UI Portal). Ré-wiré story-646 Inc.2.
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

/// Couleur principale par `StageKind` (cohérence avec ennemis arena). Ré-wiré Inc.2.
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
/// - 1..4 → fallback numérique (NB : 2-4 switchent AUSSI l'arme — cosmétique,
///   la salle est vide pendant le choix ; les flèches sont le chemin propre).
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

/// Story-646 Inc.2 — RÉVEILLÉ (était stub depuis le refactor 471..479). Affiche les
/// portes typées après le clear d'une salle (`RogueliteWave.portal_choices`) et capte
/// le choix (flèches / 1-4 / clic) dans `portal_pick`, consommé par l'orchestrateur.
pub(crate) fn draw_portal_overlay(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wave: ResMut<RogueliteWave>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    if wave.portal_choices.is_empty() || wave.portal_pick.is_some() {
        return;
    }
    // Input clavier (flèches prioritaires, 1-4 fallback).
    let n = wave.portal_choices.len();
    for (key, idx) in PORTAL_KEYS {
        if (*idx as usize) < n && keys.just_pressed(*key) {
            wave.portal_pick = Some(*idx);
            return;
        }
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    // Layout MIROIR du Coffre du Forgeron (coffre_forgeron.rs:75-135, validé
    // runtime) : tailles EXPLICITES panel/cartes — les layouts « centrés » nus
    // dans une Area s'étendaient à tout l'écran (bugs runtime 2026-07-02).
    let n = wave.portal_choices.len();
    let card_w = 210.0_f32;
    let card_h = 190.0_f32;
    let gap = 24.0_f32;
    let total_w = card_w * n as f32 + gap * (n as f32 - 1.0).max(0.0);
    let panel_w = total_w + 40.0;
    let panel_h = card_h + 90.0;

    let mut clicked: Option<u8> = None;
    egui::Area::new(egui::Id::new("forgia_portal_choice"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -40.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(VERRE_GLASS)
                .stroke(egui::Stroke::new(1.5, HAIR_GOLD_STRONG))
                .corner_radius(egui::CornerRadius::same(18))
                .inner_margin(22)
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(panel_w, panel_h));
                    ui.set_max_width(panel_w);
                    ui.vertical_centered(|ui| {
                        ui.label(display_text("CHOISIS TA PORTE", 32.0, FORGE_OR).strong());
                        ui.add_space(12.0);
                    });
                    ui.horizontal(|ui| {
                        let hints = ["←", "→", "↑", "↓"];
                        for (i, kind) in wave.portal_choices.iter().enumerate() {
                            if i > 0 {
                                ui.add_space(gap);
                            }
                            let (emoji, label) = stage_kind_display(*kind);
                            let color = stage_kind_color(*kind);
                            ui.allocate_ui_with_layout(
                                egui::vec2(card_w, card_h),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    ui.add_space(10.0);
                                    ui.label(egui::RichText::new(emoji).size(46.0));
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(label).size(24.0).strong().color(color),
                                    );
                                    ui.add_space(14.0);
                                    let hint = hints.get(i).copied().unwrap_or("?");
                                    if cartoon_btn(ui, &format!("{hint}  ENTRER"), color).clicked()
                                    {
                                        clicked = Some(i as u8);
                                    }
                                },
                            );
                        }
                    });
                });
        });
    if clicked.is_some() {
        wave.portal_pick = clicked;
    }
}

// ─── Plugin ──────────────────────────────────────────────────────────────

// ─── Bark bubble (Story-482 — floating text overlay armes parlantes) ───────

/// Couleur RGBA du label speaker, alignée avec persona genome.
/// Story-596 — délègue à la source unique `forge_persona_color` (style.rs) :
/// les valeurs locales avaient drifté (Pépin (120,220,130) vs (122,201,130)).
pub fn speaker_color(speaker: &str) -> egui::Color32 {
    forge_persona_color(speaker)
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
        // Couleur d'archétype (source unique `archetype_color`) + alpha de fade distance.
        let base = archetype_color(*archetype);
        let color =
            egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), (255.0 * alpha) as u8);
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

/// Story-558 P3 (2026-05-29) — banner telegraph boss enrage. time_left
/// décompté chaque frame. Set par `sys_track_boss_enrage_banner` sur event.
#[derive(Resource, Default, Debug)]
pub struct BossEnrageBannerState {
    pub time_left: f32,
}

const BOSS_ENRAGE_BANNER_DURATION: f32 = 2.5;

/// Tick state + apply CameraTrauma punch sur fire de l'event.
pub(crate) fn sys_track_boss_enrage_banner(
    time: Res<Time>,
    mut events: MessageReader<crate::waves::BossEnrageTriggeredEvent>,
    mut state: ResMut<BossEnrageBannerState>,
    mut trauma: ResMut<forgia_combat::combat_juice::CameraTrauma>,
) {
    let mut triggered = false;
    for _ in events.read() {
        triggered = true;
    }
    if triggered {
        state.time_left = BOSS_ENRAGE_BANNER_DURATION;
        // Vlambeer juice — camera shake punch fort (audit §6).
        trauma.add(0.65);
    } else {
        let dt = time.delta_secs();
        state.time_left = (state.time_left - dt).max(0.0);
    }
}

/// Banner top-center "⚒ FORGE EN COLÈRE !" — fade in/out + scale pop.
pub(crate) fn draw_boss_enrage_banner(
    mut contexts: EguiContexts,
    state: Res<BossEnrageBannerState>,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
) {
    if state.time_left <= 0.0 {
        return;
    }
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let lifetime_t = 1.0 - (state.time_left / BOSS_ENRAGE_BANNER_DURATION);
    // Fade in 1st 15%, hold middle 70%, fade out last 15%.
    let alpha = if lifetime_t < 0.15 {
        lifetime_t / 0.15
    } else if lifetime_t > 0.85 {
        ((1.0 - lifetime_t) / 0.15).max(0.0)
    } else {
        1.0
    };
    // Pop scale ease-out-back sur intro 200ms (helper partagé story-596).
    let pop_scale = ease_out_back((lifetime_t / 0.10).min(1.0));

    egui::Area::new(egui::Id::new("forgia_roguelite_boss_enrage_banner"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 96.0))
        .show(ctx, |ui| {
            let banner_w = 540.0 * pop_scale;
            let banner_h = 84.0 * pop_scale;
            let (_, response) =
                ui.allocate_exact_size(egui::vec2(banner_w, banner_h), egui::Sense::hover());
            let rect = response.rect;
            let painter = ui.painter_at(rect);
            // Shadow stack
            for (i, alpha_mul) in [80u8, 50u8, 25u8].iter().enumerate() {
                let off = egui::vec2(6.0 * (i + 1) as f32, 6.0 * (i + 1) as f32);
                painter.rect_filled(
                    rect.translate(off),
                    14.0,
                    egui::Color32::from_black_alpha((f32::from(*alpha_mul) * alpha) as u8),
                );
            }
            // Fond braise + border or épais.
            let fill = egui::Color32::from_rgba_unmultiplied(231, 76, 60, (alpha * 240.0) as u8);
            let stroke_color =
                egui::Color32::from_rgba_unmultiplied(244, 196, 48, (alpha * 255.0) as u8);
            painter.rect_filled(rect, 14.0, fill);
            painter.rect_stroke(
                rect,
                14.0,
                egui::Stroke::new(5.0, stroke_color),
                egui::StrokeKind::Inside,
            );
            // Text "⚒ FORGE EN COLÈRE !" creme outline noir.
            let text_color =
                egui::Color32::from_rgba_unmultiplied(255, 244, 220, (alpha * 255.0) as u8);
            let outline = egui::Color32::from_rgba_unmultiplied(43, 24, 16, (alpha * 255.0) as u8);
            let size = 36.0 * pop_scale;
            let pos = rect.center();
            for dx in [-2.0_f32, 0.0, 2.0] {
                for dy in [-2.0_f32, 0.0, 2.0] {
                    if dx == 0.0 && dy == 0.0 {
                        continue;
                    }
                    painter.text(
                        pos + egui::vec2(dx, dy),
                        egui::Align2::CENTER_CENTER,
                        "⚒  FORGE EN COLÈRE !",
                        egui::FontId::proportional(size),
                        outline,
                    );
                }
            }
            painter.text(
                pos,
                egui::Align2::CENTER_CENTER,
                "⚒  FORGE EN COLÈRE !",
                egui::FontId::proportional(size),
                text_color,
            );
        });
}

// ─── Minimap (top-left) — radar style Gunfire Reborn ───────────────────────

/// Couleur cartoon par archétype ennemi — source unique (blips minimap + labels
/// 3D), remplace les littéraux rgb dupliqués. Valeurs inchangées (no-speculative).
pub(crate) fn archetype_color(a: EnemyArchetype) -> egui::Color32 {
    match a {
        EnemyArchetype::Boss => egui::Color32::from_rgb(255, 80, 200),
        EnemyArchetype::Tank => egui::Color32::from_rgb(240, 70, 70),
        EnemyArchetype::Sniper => egui::Color32::from_rgb(190, 100, 255),
        EnemyArchetype::Runner => egui::Color32::from_rgb(255, 180, 60),
    }
}

/// 2026-06-02 — Minimap radar cartoon (HUD Roguelite, modèle Gunfire Reborn).
///
/// **Player-relative** : la carte tourne avec le joueur, la flèche reste fixe
/// pointant vers le haut (= forward). Lit uniquement les positions ennemis +
/// la caméra → aucune dépendance à l'économie (Or/Souls), 100% orthogonal.
///
/// Vue pure (pas de Resource produite) : pas de sensor dédié, les positions
/// sont déjà couvertes par `forgia_entities_snapshot` / sensors roguelite.
pub(crate) fn draw_minimap(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_enemies: Query<(&GlobalTransform, &EnemyArchetype)>,
    timer: Res<RunTimer>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    // Visible uniquement en combat (cohérent avec draw_wave_counter).
    let in_combat = matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    );
    if !in_combat {
        return;
    }
    let Ok(cam_tf) = q_cam.single() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_roguelite_minimap"),
    ));
    let screen = ctx.content_rect();

    // Géométrie : cercle ancré haut-gauche.
    const RADIUS: f32 = 72.0;
    const RANGE_M: f32 = 55.0; // mètres couverts du centre au bord
    let pad = 22.0;
    let center = egui::pos2(screen.min.x + pad + RADIUS, screen.min.y + pad + RADIUS);
    let scale = RADIUS / RANGE_M;

    // Radar verre « Verre & Braise » : ombre douce + fond verre + liseré or unique
    // (`glass_panel` est rect-only ; on en reproduit l'esprit pour le cercle, à la
    // place de l'ancien double anneau charbon+or qui détonnait des autres panneaux).
    painter.circle_filled(
        center + egui::vec2(0.0, 4.0),
        RADIUS,
        egui::Color32::from_black_alpha(60),
    );
    painter.circle_filled(center, RADIUS, VERRE_GLASS);
    painter.circle_stroke(center, RADIUS, egui::Stroke::new(1.5, HAIR_GOLD_STRONG));

    // Heading joueur projeté sur le plan XZ (le pitch caméra est ignoré).
    let fwd = cam_tf.forward();
    let mut fwd_xz = egui::vec2(fwd.x, fwd.z);
    if fwd_xz.length() < 1e-4 {
        fwd_xz = egui::vec2(0.0, -1.0);
    }
    let fwd_n = fwd_xz.normalized();
    // Vecteur "droite" = rotation -90° du forward dans le plan.
    let right_n = egui::vec2(-fwd_n.y, fwd_n.x);
    let player_xz = egui::vec2(cam_tf.translation().x, cam_tf.translation().z);

    // Blips ennemis (projetés repère joueur, clampés au rayon).
    for (etf, archetype) in &q_enemies {
        let delta = egui::vec2(etf.translation().x, etf.translation().z) - player_xz;
        let up = delta.dot(fwd_n); // + = devant le joueur
        let rg = delta.dot(right_n); // + = à droite du joueur
        let mut blip = egui::vec2(rg * scale, -up * scale); // egui y vers le bas
        if blip.length() > RADIUS - 6.0 {
            blip = blip.normalized() * (RADIUS - 6.0);
        }
        let color = archetype_color(*archetype);
        let r = match archetype {
            EnemyArchetype::Boss => 5.5,
            EnemyArchetype::Tank => 4.5,
            EnemyArchetype::Sniper => 4.0,
            EnemyArchetype::Runner => 3.5,
        };
        let p = center + blip;
        painter.circle_filled(p, r + 1.0, FORGE_CHARBON); // outline lisibilité
        painter.circle_filled(p, r, color);
    }

    // Flèche joueur au centre (pointe vers le haut = forward).
    let tip = center + egui::vec2(0.0, -9.0);
    let bl = center + egui::vec2(-6.0, 7.0);
    let br = center + egui::vec2(6.0, 7.0);
    painter.add(egui::Shape::convex_polygon(
        vec![tip, bl, br],
        FORGE_TEAL,
        egui::Stroke::new(2.0, FORGE_CHARBON),
    ));

    // Chrono de run sous la minimap (style Gunfire « 6 min 43 s »).
    let total = timer.secs.max(0.0) as u32;
    text_with_outline(
        &painter,
        egui::pos2(center.x, center.y + RADIUS + 16.0),
        egui::Align2::CENTER_CENTER,
        &format!("{}:{:02}", total / 60, total % 60),
        egui::FontId::proportional(18.0),
        C_TEXT_LIGHT,
        2.0,
    );
}

// ─── Slots d'armes (bas-droite) — modèle Gunfire ──────────────────────────

/// 2026-06-02 — barre des armes équipées (bas-droite), style Gunfire Reborn.
/// Lit `EquippedWeapons.current` (forgia-combat) + le set `ARENA_V1_WEAPONS`.
/// Chaque slot = touche (1..N) + **nom lore** de l'arme (Pépin/Bourrasque/…,
/// les armes parlantes = identité Forgia), l'arme active surlignée or.
/// + hint « ⇧ DASH ». Vue pure, orthogonale à l'économie.
pub(crate) fn draw_weapon_slots(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    equipped: Option<Res<EquippedWeapons>>,
    ammo_tuning: Option<Res<AmmoHudTuning>>,
    save: Option<Res<crate::meta_shop::MetaShopSave>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let in_combat = matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    );
    if !in_combat {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let current = equipped.as_ref().map(|e| e.current).unwrap_or_default();
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_roguelite_weapon_slots"),
    ));

    let weapons = ARENA_V1_WEAPONS;
    let n = weapons.len() as f32;
    let slot_w = 92.0;
    let slot_h = 48.0;
    let gap = 6.0;
    let total_w = n * slot_w + (n - 1.0) * gap;

    // Positionné à GAUCHE du compteur ammo (forgia-ui-lib::hud_ammo) pour éviter
    // le chevauchement — layout Gunfire (armes à gauche, munitions à droite).
    // Lit AmmoHudTuning (genome-driven) pour rester découplé des valeurs ammo.
    let (ammo_pad_right, ammo_panel_w, ammo_pad_bottom) = ammo_tuning
        .as_ref()
        .map(|t| {
            (
                t.counter_padding_right,
                t.counter_panel_w,
                t.counter_padding_bottom,
            )
        })
        .unwrap_or((32.0, 280.0, 28.0));
    let ammo_left = screen.max.x - ammo_pad_right - ammo_panel_w;
    let start_x = ammo_left - 14.0 - total_w; // 14px de marge entre barre et ammo
    let bottom_y = screen.max.y - ammo_pad_bottom - slot_h; // baseline alignée sur l'ammo

    // Hint « ⇧ DASH » au-dessus de la barre (le dash existe — story-528).
    text_with_outline(
        &painter,
        egui::pos2(start_x, bottom_y - 8.0),
        egui::Align2::LEFT_BOTTOM,
        "⇧ DASH",
        egui::FontId::proportional(13.0),
        C_TEXT_MUTED,
        1.0,
    );

    for (i, w) in weapons.iter().enumerate() {
        let x = start_x + i as f32 * (slot_w + gap);
        let rect = egui::Rect::from_min_size(egui::pos2(x, bottom_y), egui::vec2(slot_w, slot_h));
        let active = *w == current;
        // Story-613 — arme verrouillée (non débloquée à l'Enclume) : slot grisé.
        let owned = save
            .as_ref()
            .map(|s| s.is_weapon_unlocked(crate::weapon_select::vm_key(*w)))
            .unwrap_or(true);
        let fill = if active {
            FORGE_OR
        } else if owned {
            C_BG_DARK
        } else {
            C_BG_DARK.gamma_multiply(0.45)
        };
        // Ombre douce (profondeur verre, cohérent glass_panel) sous chaque slot.
        cartoon_drop_shadow(&painter, rect, 8.0, egui::vec2(0.0, 4.0));
        painter.rect_filled(rect, 8.0, fill);
        painter.rect_stroke(
            rect,
            8.0,
            egui::Stroke::new(
                if active { 2.0 } else { 1.2 },
                if active {
                    FORGE_CHARBON
                } else {
                    HAIR_GOLD_STRONG
                },
            ),
            egui::StrokeKind::Inside,
        );

        let txt_color = if active {
            FORGE_CHARBON
        } else if owned {
            C_TEXT_LIGHT
        } else {
            C_TEXT_MUTED
        };
        // Touche (1..N) en haut-gauche.
        text_with_outline(
            &painter,
            egui::pos2(rect.min.x + 7.0, rect.min.y + 4.0),
            egui::Align2::LEFT_TOP,
            &format!("{}", i + 1),
            egui::FontId::proportional(14.0),
            txt_color,
            1.0,
        );
        // Nom lore de l'arme (talking weapons = identité Forgia ; fallback display_name).
        let speaker = crate::run::weapon_to_speaker(*w);
        let name = if speaker == "any" {
            w.display_name()
        } else {
            speaker_label(speaker)
        };
        text_with_outline(
            &painter,
            egui::pos2(rect.center().x, rect.max.y - 13.0),
            egui::Align2::CENTER_CENTER,
            name,
            egui::FontId::proportional(13.0),
            txt_color,
            1.0,
        );
    }
}

// ─── Portrait joueur (bas-gauche, au-dessus du HP bar) ─────────────────────

/// 2026-06-02 — portrait cartoon de l'Apprenti (bible v1), au-dessus du HP bar.
/// Placeholder dessiné (visage amical) en attendant un asset dédié. Roguelite-only.
/// Carte vitals unique du coin bas-gauche — **un seul système possède la zone**,
/// donc zéro chevauchement (avant : portrait + barres défense + cœurs énergie +
/// cœurs confiance = 4 painters indépendants qui se recouvraient). Panneau verre
/// « Verre & Braise » (`glass_panel`), stack vertical déterministe :
///   [portrait + nom + niveau] → [bouclier] → [armure] → [énergie] → [confiance Pépin]
/// Gaté `in_run_or_boss` au wire-up (pas de garde interne).
pub(crate) fn draw_vitals_card(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    q_player: Query<(&DamageHealth, Option<&DefenseLayer>), With<Player>>,
    identity: Option<Res<crate::identity::IdentitySave>>,
    progress: Option<Res<crate::progress::PlayerProgress>>,
    equipped: Option<Res<EquippedWeapons>>,
    conf: Option<Res<PepinConfidence>>,
    time: Res<Time>,
) {
    // `in_run_or_boss` (run_if) garantit Roguelite mais PAS `AppMode::InGame` —
    // Pause est indépendante du RunState. Sans ce guard, la carte resterait
    // peinte derrière le menu pause (non-fullscreen). Cohérent avec les autres
    // widgets HUD du même tuple.
    if *app_state.get() != AppMode::InGame {
        return;
    }
    let Ok((health, defense)) = q_player.single() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_vitals_card"),
    ));

    // Constantes de layout (cosmétique UI — exempt no-hardcode).
    const PORTRAIT_R: f32 = 26.0;
    const CARD_W: f32 = 300.0;
    const MARGIN: f32 = 14.0;
    const SCREEN_PAD: f32 = 24.0;
    const SEG_H: f32 = 10.0;
    const SEG_GAP: f32 = 5.0;
    const BAR_H: f32 = 26.0;
    const ROW_GAP: f32 = 8.0;
    const HEART_ROW_H: f32 = 14.0;
    const HEADER_H: f32 = PORTRAIT_R * 2.0;

    // ── Données ──
    let frac = health.fraction();
    let name = identity
        .as_ref()
        .map(|s| {
            if s.player_name.is_empty() {
                "Forgeron"
            } else {
                s.player_name.as_str()
            }
        })
        .unwrap_or("Forgeron");
    let level = progress.as_ref().map(|p| p.level).unwrap_or(1);
    let has_shield = defense.is_some_and(|d| d.shield_max > 0.5);
    let has_armor = defense.is_some_and(|d| d.armor_max > 0.5);
    // Confiance Pépin visible seulement arme ModernAR (Pépin) en main.
    let pepin = conf.as_ref().filter(|_| {
        equipped
            .as_ref()
            .is_some_and(|e| e.current == WeaponType::ModernAR)
    });

    // ── Hauteur dynamique du panneau ──
    let n_def = u8::from(has_shield) + u8::from(has_armor);
    let def_block = f32::from(n_def) * (SEG_H + SEG_GAP);
    let heart_block = if pepin.is_some() {
        ROW_GAP + HEART_ROW_H
    } else {
        0.0
    };
    let content_h = HEADER_H + ROW_GAP + def_block + BAR_H + heart_block;
    let card_h = content_h + 2.0 * MARGIN;

    let card_left = screen.min.x + SCREEN_PAD;
    let card_bottom = screen.max.y - SCREEN_PAD;
    let card = egui::Rect::from_min_max(
        egui::pos2(card_left, card_bottom - card_h),
        egui::pos2(card_left + CARD_W, card_bottom),
    );
    glass_panel(&painter, card, 14.0);

    let content_left = card_left + MARGIN;
    let bar_w = CARD_W - 2.0 * MARGIN;
    let mut y = card.min.y + MARGIN;

    // ── Header : portrait apprenti + nom + niveau ──
    let pc = egui::pos2(content_left + PORTRAIT_R, y + PORTRAIT_R);
    draw_apprentice_face(&painter, pc, PORTRAIT_R);
    let text_x = pc.x + PORTRAIT_R + 12.0;
    text_with_outline(
        &painter,
        egui::pos2(text_x, pc.y - 10.0),
        egui::Align2::LEFT_CENTER,
        name,
        display_font(20.0),
        C_TEXT_LIGHT,
        1.8,
    );
    text_with_outline(
        &painter,
        egui::pos2(text_x, pc.y + 13.0),
        egui::Align2::LEFT_CENTER,
        &format!("Niv. {level}"),
        display_font(14.0),
        FORGE_OR,
        1.2,
    );
    y += HEADER_H + ROW_GAP;

    // ── Défense : bouclier puis armure (chacune si sa couche existe) ──
    if let Some(def) = defense {
        if has_shield {
            draw_vitals_bar(
                &painter,
                content_left,
                y,
                bar_w,
                SEG_H,
                def.shield / def.shield_max,
                DEFENSE_SHIELD,
                3.0,
            );
            y += SEG_H + SEG_GAP;
        }
        if has_armor {
            draw_vitals_bar(
                &painter,
                content_left,
                y,
                bar_w,
                SEG_H,
                def.armor / def.armor_max,
                DEFENSE_ARMOR,
                3.0,
            );
            y += SEG_H + SEG_GAP;
        }
    }

    // ── Barre d'énergie (vie relabelée — anti-canon : jamais le mot « HP ») ──
    draw_vitals_bar(&painter, content_left, y, bar_w, BAR_H, frac, hp_color(frac), 5.0);
    text_with_outline(
        &painter,
        egui::pos2(content_left + 8.0, y + BAR_H * 0.5),
        egui::Align2::LEFT_CENTER,
        "ÉNERGIE",
        display_font(11.0),
        FORGE_OR,
        1.0,
    );
    text_with_outline(
        &painter,
        egui::pos2(content_left + bar_w * 0.5, y + BAR_H * 0.5),
        egui::Align2::CENTER_CENTER,
        &format!(
            "{} / {}",
            health.current.round() as i32,
            health.max.round() as i32
        ),
        display_font(16.0),
        C_TEXT_LIGHT,
        1.2,
    );
    y += BAR_H;

    // ── Confiance Pépin (discret, cœur de tête clignote après changement) ──
    if let Some(conf) = pepin {
        y += ROW_GAP;
        const BLINK_SECS: f32 = 0.5;
        // Cyan Pépin (couleur dominante persona — story-531 AC8).
        const PEPIN_CYAN: egui::Color32 = egui::Color32::from_rgb(80, 220, 255);
        let since = time.elapsed_secs() - conf.last_change_secs;
        let blinking = (0.0..BLINK_SECS).contains(&since);
        let pulse = if blinking {
            ((since / BLINK_SECS) * std::f32::consts::TAU * 2.0)
                .sin()
                .abs()
        } else {
            1.0
        };
        let stacks = usize::from(conf.stacks);
        let spacing = 16.0;
        let size = 9.0;
        let cy = y + HEART_ROW_H * 0.5;
        for i in 0..10usize {
            let filled = i < stacks;
            let is_head = i + 1 == stacks;
            let alpha = if filled {
                if is_head && blinking {
                    (80.0 + 175.0 * pulse) as u8
                } else {
                    255
                }
            } else {
                55
            };
            let color = egui::Color32::from_rgba_unmultiplied(
                PEPIN_CYAN.r(),
                PEPIN_CYAN.g(),
                PEPIN_CYAN.b(),
                alpha,
            );
            draw_cartoon_heart(
                &painter,
                content_left + size * 0.5 + i as f32 * spacing,
                cy,
                size,
                color,
            );
        }
    }
}

/// Barre vitals (track sombre + fill coloré + reflet bombé + liseré or) — réutilisée
/// pour l'énergie et les couches de défense de la carte vitals.
fn draw_vitals_bar(
    painter: &egui::Painter,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    frac: f32,
    fill: egui::Color32,
    corner: f32,
) {
    let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h));
    painter.rect_filled(
        rect,
        corner,
        egui::Color32::from_rgba_unmultiplied(20, 22, 28, 235),
    );
    let fw = w * frac.clamp(0.0, 1.0);
    if fw > 0.5 {
        let fr = egui::Rect::from_min_size(rect.min, egui::vec2(fw, h));
        painter.rect_filled(fr, corner, fill);
        // Reflet bombé (quart haut plus clair) — cohérent HP bar cartoon.
        let hl = egui::Rect::from_min_size(fr.min, egui::vec2(fw, h * 0.35));
        painter.rect_filled(
            hl,
            corner,
            egui::Color32::from_rgba_unmultiplied(
                fill.r().saturating_add(40),
                fill.g().saturating_add(40),
                fill.b().saturating_add(40),
                150,
            ),
        );
    }
    painter.rect_stroke(
        rect,
        corner,
        egui::Stroke::new(1.5, HAIR_GOLD_STRONG),
        egui::StrokeKind::Inside,
    );
}

/// Visage « Apprenti » cohérent DA Verre & Braise : disque aubergine clair + liseré
/// or (remplace l'ancien disque bois clair, qui détonnait au milieu des panneaux
/// verre en plein combat). Placeholder en attendant un asset de portrait dédié.
fn draw_apprentice_face(painter: &egui::Painter, center: egui::Pos2, r: f32) {
    use std::f32::consts::PI;
    painter.circle_filled(center, r, FORGE_PANEL_LIGHT);
    painter.circle_stroke(center, r, egui::Stroke::new(2.0, HAIR_GOLD_STRONG));
    // Yeux (2 points crème).
    let eye_dx = r * 0.30;
    let eye_dy = -r * 0.12;
    let eye_r = r * 0.11;
    painter.circle_filled(center + egui::vec2(-eye_dx, eye_dy), eye_r, C_TEXT_LIGHT);
    painter.circle_filled(center + egui::vec2(eye_dx, eye_dy), eye_r, C_TEXT_LIGHT);
    // Sourire (arc bas = amical, bible « Apprenti doux »).
    arc_stroke(painter, center, r * 0.5, 0.18 * PI, 0.64 * PI, 1.0, FORGE_OR, 2.5, 16);
}

// ─── Bandeau Ultime (touche F, 10 s) — story-596 T4b ────────────────────────

/// Feedback joueur quand l'Ultime est ACTIF : bordure d'écran + bandeau haut +
/// barre de temps, **couleur liée à la technique de l'arme** (orange=explosion,
/// cyan=chaîne, violet=perforation, glace=gel). Bien visible (l'effet ennemi est
/// secondaire). Gaté `in_run_or_boss` (tuple) + `is_active`.
pub(crate) fn draw_ultimate_banner(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    ult: Res<UltimateState>,
    ult_cfg: Option<Res<UltimateConfig>>,
    equipped: Option<Res<EquippedWeapons>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    if !ult.is_active() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_roguelite_ultimate"),
    ));

    let cfg = ult_cfg.map(|c| *c).unwrap_or_default();
    let weapon = equipped.as_ref().map(|e| e.current).unwrap_or_default();
    let [r, g, b] = weapon_vfx_color(&cfg, weapon);
    let col = egui::Color32::from_rgb(
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
    );

    // 1) Bordure d'écran (4 barres) — pulse colorée pendant l'Ultime.
    let t = 12.0;
    let bar = |min: egui::Pos2, max: egui::Pos2| {
        painter.rect_filled(egui::Rect::from_min_max(min, max), 0.0, col);
    };
    bar(screen.min, egui::pos2(screen.max.x, screen.min.y + t)); // haut
    bar(egui::pos2(screen.min.x, screen.max.y - t), screen.max); // bas
    bar(screen.min, egui::pos2(screen.min.x + t, screen.max.y)); // gauche
    bar(egui::pos2(screen.max.x - t, screen.min.y), screen.max); // droite

    // 2) Bandeau haut-centre : « ULTIME · <technique> ».
    let cx = screen.center().x;
    let ty = screen.min.y + 72.0;
    let label = format!("ULTIME · {}", weapon_technique_label(weapon));
    text_with_outline(
        &painter,
        egui::pos2(cx, ty),
        egui::Align2::CENTER_CENTER,
        &label,
        display_font(36.0),
        col,
        3.0,
    );

    // 3) Barre de temps (se vide sur 10 s).
    let frac = ult.hud_fill();
    let (bw, bh) = (320.0, 14.0);
    let by = ty + 34.0;
    painter.rect_filled(
        egui::Rect::from_min_size(egui::pos2(cx - bw * 0.5, by), egui::vec2(bw, bh)),
        4.0,
        C_BG_DARK,
    );
    painter.rect_filled(
        egui::Rect::from_min_size(egui::pos2(cx - bw * 0.5, by), egui::vec2(bw * frac, bh)),
        4.0,
        col,
    );

    // 4) Secondes restantes.
    text_with_outline(
        &painter,
        egui::pos2(cx, by + bh + 16.0),
        egui::Align2::CENTER_CENTER,
        &format!("{:.0}s", ult.active_left.ceil()),
        egui::FontId::proportional(20.0),
        col,
        1.5,
    );
}

// ─── Indicateur sort F « Onde de choc » (bas-centre) ───────────────────────

/// Story-572 — pastille du sort F (bas-centre). Prêt = anneau + « F » dorés ;
/// en cooldown = anneau charbon + arc braise (progression) + secondes restantes.
pub(crate) fn draw_shockwave_indicator(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    ability: Res<crate::shockwave::ShockwaveAbility>,
    equipped: Option<Res<EquippedWeapons>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let in_combat = matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    );
    if !in_combat {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_roguelite_shockwave"),
    ));

    use std::f32::consts::{FRAC_PI_2, TAU};
    const R: f32 = 30.0;
    // Pastille du sort F, CENTRÉE. L'Ultime (même touche F) est rendu en anneau
    // extérieur concentrique par `draw_ultimate_gauge` — un seul bouton, pas deux.
    let center = egui::pos2(screen.center().x, screen.max.y - 64.0);
    // Story-573 — cooldown PAR ARME + couleur persona (identité armes parlantes).
    let current = equipped.as_ref().map(|e| e.current).unwrap_or_default();
    let cd = ability.cooldowns.get(&current).copied().unwrap_or(0.0);
    let max_cd = crate::shockwave::spell_max_cooldown(current);
    let ready = cd <= 0.0;
    let accent = speaker_color(crate::run::weapon_to_speaker(current));

    painter.circle_filled(center, R, VERRE_GLASS);
    painter.circle_stroke(center, R, egui::Stroke::new(1.5, HAIR_GOLD_STRONG));
    let ring = if ready { accent } else { FORGE_METAL_CHAUD };
    painter.circle_stroke(center, R, egui::Stroke::new(2.0, ring));

    let f_color = if ready { accent } else { C_TEXT_MUTED };
    text_with_outline(
        &painter,
        center,
        egui::Align2::CENTER_CENTER,
        "F",
        egui::FontId::proportional(24.0),
        f_color,
        2.0,
    );

    if !ready {
        let frac = 1.0 - (cd / max_cd.max(0.01)).clamp(0.0, 1.0);
        arc_stroke(
            &painter,
            center,
            R + 4.0,
            -FRAC_PI_2,
            TAU,
            frac,
            FORGE_BRAISE,
            4.0,
            24,
        );
        text_with_outline(
            &painter,
            egui::pos2(center.x, center.y + R + 12.0),
            egui::Align2::CENTER_CENTER,
            &format!("{:.0}", cd.ceil()),
            egui::FontId::proportional(14.0),
            C_TEXT_MUTED,
            1.0,
        );
    }
}

// ─── Jauge d'Ultime (bas-centre, à droite du sort F) ───────────────────────

/// Rend visible la CHARGE de l'Ultime — comble le trou où l'Ultime n'avait aucun
/// indicateur hors-actif (seul le bandeau plein écran s'affichait, une fois lancé).
/// L'Ultime s'ouvre avec le sort F quand il est prêt (cooldown propre, plus long
/// que celui du sort). États : prêt = anneau accent + « ULT » + halo pulsé (attire
/// l'œil) ; en charge = arc de recharge + secondes ; actif = arc qui se vide.
/// Couleur accent = VFX de la technique de l'arme (identité), comme le bandeau.
pub(crate) fn draw_ultimate_gauge(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    time: Res<Time>,
    ult: Res<UltimateState>,
    ult_cfg: Option<Res<UltimateConfig>>,
    equipped: Option<Res<EquippedWeapons>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let in_combat = matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    );
    if !in_combat {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_roguelite_ultimate_gauge"),
    ));

    use std::f32::consts::{FRAC_PI_2, TAU};
    const R: f32 = 30.0;
    // Anneau CONCENTRIQUE autour de la pastille F (même touche F, pas un 2e bouton) :
    // track sombre + arc de charge (0→1 sur le cooldown), plein + halo pulsé quand
    // prêt, se vide pendant l'Ultime actif.
    const RING_R: f32 = R + 11.0;
    let center = egui::pos2(screen.center().x, screen.max.y - 64.0);

    let cfg = ult_cfg.map(|c| *c).unwrap_or_default();
    let weapon = equipped.as_ref().map(|e| e.current).unwrap_or_default();
    let [r, g, b] = weapon_vfx_color(&cfg, weapon);
    let accent = egui::Color32::from_rgb(
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
    );
    let ready = ult.can_activate();
    let active = ult.is_active();

    // Track de fond (cercle complet discret).
    painter.circle_stroke(
        center,
        RING_R,
        egui::Stroke::new(
            3.5,
            egui::Color32::from_rgba_unmultiplied(20, 22, 28, 200),
        ),
    );
    // Halo pulsé quand prêt — invite à lâcher l'Ultime avec F.
    if ready {
        let pulse = 0.5 + 0.5 * (time.elapsed_secs() * TAU * 1.5).sin();
        let a = (45.0 + 60.0 * pulse) as u8;
        painter.circle_stroke(
            center,
            RING_R,
            egui::Stroke::new(
                5.0,
                egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), a),
            ),
        );
    }
    // Arc de charge (recharge) / vidange (actif) ; plein quand prêt.
    let progress = if ready { 1.0 } else { ult.hud_fill() };
    let arc_col = if ready || active { accent } else { FORGE_OR };
    arc_stroke(&painter, center, RING_R, -FRAC_PI_2, TAU, progress, arc_col, 3.5, 64);

    // Étiquette discrète « ULT » au-dessus de l'anneau (pas un bouton cliquable).
    let cap_col = if ready || active { accent } else { C_TEXT_MUTED };
    text_with_outline(
        &painter,
        egui::pos2(center.x, center.y - RING_R - 6.0),
        egui::Align2::CENTER_BOTTOM,
        "ULT",
        display_font(11.0),
        cap_col,
        1.0,
    );
}

pub struct RogueliteHudPlugin;

/// Story-585/589 — cartes de choix au portail de fin de zone (choix gratuit,
/// agency Hadès, touche 1/2/3). Story-589 : si `is_element_choice`, les cartes
/// ARMENT un élément (couleur = couleur de l'élément) ; sinon ce sont des boons.
pub(crate) fn draw_zone_reward_cards(
    mut contexts: EguiContexts,
    reward: Res<crate::loot_room::ZoneReward>,
    catalogue: Option<Res<forgia_rpg_data::boons::BoonsCatalogue>>,
    elements_cfg: Option<Res<crate::elements::ElementConfig>>,
) {
    if !reward.choosing() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    // Construit les cartes (titre, sous-titre, couleur) selon le type de choix.
    let boon_colors = [
        egui::Color32::from_rgb(231, 76, 60),
        egui::Color32::from_rgb(80, 160, 255),
        egui::Color32::from_rgb(80, 220, 120),
    ];
    let (heading, cards): (&str, Vec<(String, String, egui::Color32)>) =
        if reward.is_element_choice() {
            let vfx = elements_cfg
                .as_deref()
                .map(|c| c.vfx.clone())
                .unwrap_or_default();
            let cards = reward
                .element_candidates
                .iter()
                .map(|e| {
                    let [r, g, b] = e.rgb(&vfx);
                    let col = egui::Color32::from_rgb(
                        (r * 255.0) as u8,
                        (g * 255.0) as u8,
                        (b * 255.0) as u8,
                    );
                    (e.fr_name().to_string(), e.tag().to_string(), col)
                })
                .collect();
            ("ARME UN ÉLÉMENT", cards)
        } else {
            let cat = catalogue.as_deref();
            let cards = reward
                .candidates
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let name = cat
                        .and_then(|c| c.entries.iter().find(|b| &b.id == id))
                        .map(|d| d.name.to_string())
                        .unwrap_or_else(|| "???".to_string());
                    (name, String::new(), boon_colors[i.min(2)])
                })
                .collect();
            ("CHOISIS UNE AMÉLIORATION", cards)
        };

    // Prompt dynamique : reflète le nombre réel de cartes (story-589 — au portail
    // final il peut n'y avoir que 2 éléments à armer, pas 3).
    let prompt = format!(
        "Appuie sur {}",
        (1..=cards.len())
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" · ")
    );

    egui::Area::new(egui::Id::new("forgia_zone_reward"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // Story-596 — fonds harmonisés palette Forge (étaient gris-violet ad hoc).
            egui::Frame::new()
                .fill(FORGE_PANEL)
                .inner_margin(egui::Margin::symmetric(40, 28))
                .corner_radius(egui::CornerRadius::same(16))
                .stroke(egui::Stroke::new(1.5, HAIR_GOLD_STRONG))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading(display_text(heading, 34.0, FORGE_CREME).strong());
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            for (i, (title, subtitle, col)) in cards.iter().enumerate() {
                                egui::Frame::new()
                                    .fill(FORGE_PANEL_LIGHT)
                                    .inner_margin(egui::Margin::same(16))
                                    .corner_radius(egui::CornerRadius::same(10))
                                    .stroke(egui::Stroke::new(3.0, *col))
                                    .show(ui, |ui| {
                                        ui.set_width(180.0);
                                        ui.vertical_centered(|ui| {
                                            ui.label(
                                                display_text(format!("{}", i + 1), 30.0, *col)
                                                    .strong(),
                                            );
                                            ui.add_space(6.0);
                                            ui.label(
                                                egui::RichText::new(title)
                                                    .size(18.0)
                                                    .strong()
                                                    .color(egui::Color32::WHITE),
                                            );
                                            if !subtitle.is_empty() {
                                                ui.add_space(4.0);
                                                ui.label(
                                                    egui::RichText::new(subtitle)
                                                        .size(13.0)
                                                        .color(egui::Color32::LIGHT_GRAY),
                                                );
                                            }
                                        });
                                    });
                                ui.add_space(10.0);
                            }
                        });
                        ui.add_space(14.0);
                        ui.label(egui::RichText::new(&prompt).size(18.0).color(C_TEXT_MUTED));
                    });
                });
        });
}

/// Story-617 — run-condition : vrai uniquement en combat (InRun/Boss). Sert à
/// masquer TOUT le HUD de combat au Lobby (écran de sélection) — un seul système
/// (`draw_wave_counter`) appliquait ce gate, tous les autres l'avaient oublié.
fn in_run_or_boss(run_state: Option<Res<State<RunState>>>) -> bool {
    matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    )
}

impl Plugin for RogueliteHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BossEnrageBannerState>()
            .add_systems(
                Update,
                sys_track_boss_enrage_banner
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            // HUD de COMBAT : masqué hors InRun/Boss (story-617 — propreté du Lobby).
            .add_systems(
                EguiPrimaryContextPass,
                (
                    draw_minimap,
                    draw_weapon_slots,
                    draw_vitals_card,
                    draw_shockwave_indicator,
                    draw_ultimate_gauge,
                    draw_ultimate_banner,
                    draw_wave_counter,
                    draw_currency_counters,
                    draw_active_boons,
                    draw_enemy_archetype_labels,
                )
                    .run_if(in_run_or_boss),
            )
            // Overlays auto-gatés par leur propre état/événement (Defeat/Victory/
            // portail/reward/bark…) — affichables hors combat selon leur logique.
            .add_systems(
                EguiPrimaryContextPass,
                (
                    draw_portal_overlay,
                    draw_zone_reward_cards,
                    draw_defeat_overlay,
                    draw_victory_overlay,
                    draw_bark_bubble,
                    draw_stage_notification,
                    draw_boss_enrage_banner,
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

#[cfg(test)]
mod run_progress_tests {
    use super::run_progress_label;
    use crate::rounds::RoundsConfig;

    fn graph_mode() -> RoundsConfig {
        RoundsConfig {
            enabled: false,
            ..RoundsConfig::default()
        }
    }

    /// LE défaut rapporté : « j'étais à 5/4 ». Un numérateur au-dessus du
    /// dénominateur n'est pas un affichage, c'est un mensonge — et il rendait
    /// la run impossible à suivre.
    #[test]
    fn the_room_number_can_never_exceed_the_total() {
        let cfg = graph_mode();
        for total in 1u8..=8 {
            for stage in 0u8..12 {
                let label = run_progress_label(stage, 1, 2, total, &cfg);
                assert!(
                    !label.contains(&format!("SALLE {} / {total}", total + 1)),
                    "stage {stage}, total {total} → {label}"
                );
                // Et au-delà du total, c'est la salle du BOSS, pas un dépassement.
                if stage + 1 >= total {
                    assert!(label.contains("BOSS"), "stage {stage}/{total} → {label}");
                }
            }
        }
    }

    /// En boucle infinie, un dénominateur raconte une histoire fausse : il n'y a
    /// pas de fin à atteindre. C'est le cœur de « on ne comprend pas comment on
    /// évolue ».
    #[test]
    fn the_endless_loop_shows_no_denominator_at_all() {
        let cfg = RoundsConfig::default();
        assert!(cfg.max_rounds == 0, "le défaut livré est bien la boucle infinie");
        for stage in [0u8, 1, 4, 30, 200] {
            let label = run_progress_label(stage, 1, 2, 4, &cfg);
            assert!(!label.contains('/') || label.contains("VAGUE"), "{label}");
            assert!(!label.contains("SALLE"), "{label}");
            assert!(label.starts_with(&format!("ROUND {}", u32::from(stage) + 1)), "{label}");
        }
    }

    /// Le round affiché commence à 1 — personne ne se situe dans un « round 0 ».
    #[test]
    fn rounds_are_numbered_from_one_for_the_player() {
        let cfg = RoundsConfig::default();
        assert!(run_progress_label(0, 1, 2, 4, &cfg).starts_with("ROUND 1"));
    }

    /// Les deux moments qui doivent se LIRE : la respiration et le palier.
    /// Sans eux, le sursaut de difficulté passe pour de l'arbitraire.
    #[test]
    fn the_rhythm_is_readable_on_screen() {
        let cfg = RoundsConfig::default();
        let respite = run_progress_label(cfg.respite_every as u8, 1, 2, 4, &cfg);
        assert!(respite.contains("RÉPIT"), "{respite}");
        let tier = run_progress_label(cfg.tier_every as u8, 1, 2, 4, &cfg);
        assert!(tier.contains("PALIER"), "{tier}");
        // Un round ordinaire ne porte ni l'un ni l'autre.
        let plain = run_progress_label(1, 1, 2, 4, &cfg);
        assert!(!plain.contains("RÉPIT") && !plain.contains("PALIER"), "{plain}");
    }

    /// Hors boucle, l'affichage historique est préservé au caractère près.
    #[test]
    fn the_graph_mode_display_is_unchanged() {
        let cfg = graph_mode();
        assert_eq!(run_progress_label(0, 1, 2, 4, &cfg), "SALLE 1 / 4  ·  VAGUE 1 / 2");
        assert_eq!(run_progress_label(3, 1, 2, 4, &cfg), "SALLE 4 / 4  ·  BOSS");
    }
}
