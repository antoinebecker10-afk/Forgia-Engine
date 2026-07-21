//! forge_shop.rs — story-659 : **fenêtre unique du Forgeron Itinérant**.
//!
//! Remplace les deux panneaux séparés (marchand bas-centre + Trempe bas-gauche) par
//! UNE fenêtre modale centrée, ouverte en **parlant au gobelin (E)** :
//! - colonne GAUCHE = « LE FORGERON ITINÉRANT » (achats : munitions/soin/revive),
//! - colonne DROITE = « LA TREMPE » (monter l'arme équipée).
//!
//! ## Souris + curseur (pattern Coffre, vérifié)
//! Ouverte → curseur libéré (`CursorOptions`) + `InputBlockers{block_look,block_fire}`
//! (block_fire lu par `forgia-fps::fire_allowed`) → on clique les boutons sans tirer ni
//! tourner la caméra. Fermée → curseur re-grab + gameplay restauré. `block_fire` n'a
//! d'autre écrivain qu'aux transitions (pause/end) → aucun conflit en jeu normal.
//!
//! ## Événementiel (scalabilité « Events > mutation directe »)
//! Les boutons (et les touches 1-4, `merchant.rs`) émettent `PurchaseRequest` /
//! `TemperRequest` — appliqués par `merchant::sys_apply_purchase` / `trempe::sys_apply_temper`.
//!
//! ## Anim du gobelin
//! `Gobli.glb` est un **mesh statique sans rig** (inspection glTF) → anim SKELETTIQUE
//! impossible. Substitut procédural : balancement (yaw) + respiration (tilt X) en idle +
//! un « hop » à l'ouverture. **Rotation seule** → zéro conflit avec la calibration du
//! marchand (qui ne touche que scale + translation.y).

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::weapons::EquippedWeapons;
use forgia_core::prelude::*;
use forgia_input::InputBlockers;
use forgia_rpg_data::loot_tables::Souls as Gold;
use forgia_ui_lib::style::{C_TEXT_MUTED, FORGE_OR, FORGE_PANEL, FORGE_TEAL, HAIR_GOLD_STRONG};
use forgia_ui_lib::theme::display_text;

use crate::merchant::{
    Currency, ForgeShopOpen, MerchantCatalogue, MerchantStats, MerchantVendor, PurchaseRequest,
    ReviveTokens,
};
use crate::run::{MetaSouls, RunState};
use crate::trempe::{TemperRequest, TrempeConfig, WeaponTrempeState};

// ── Anim procédurale du gobelin (cosmétique, rotation seule) ─────────────────
const GOBLI_SWAY_FREQ: f32 = 1.1; // balancement yaw (Hz·τ)
const GOBLI_SWAY_AMP: f32 = 0.10; // rad (~5.7°)
const GOBLI_BREATHE_FREQ: f32 = 2.2;
const GOBLI_BREATHE_AMP: f32 = 0.05;
const GOBLI_HOP_DURATION: f32 = 0.45; // s
const GOBLI_HOP_AMP: f32 = 0.30;
const GOBLI_HOP_FREQ: f32 = 22.0;

/// True si on est en run active (InRun/Boss) et in-game Roguelite.
fn shop_context_ok(
    app_state: &State<AppMode>,
    game_mode: &State<GameMode>,
    run_state: Option<&State<RunState>>,
) -> bool {
    *app_state.get() == AppMode::InGame
        && *game_mode.get() == GameMode::Roguelite
        && matches!(
            run_state.map(|s| s.get()),
            Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
        )
}

/// E près du marchand → ouvre/ferme la fenêtre. Ferme de force si on s'éloigne, si la
/// run se termine, ou si on quitte le jeu (pause) — pour ne pas laisser le curseur libre.
pub fn sys_toggle_forge_shop(
    keys: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    stats: Option<Res<MerchantStats>>,
    mut shop: ResMut<ForgeShopOpen>,
) {
    let ok = shop_context_ok(&app_state, &game_mode, run_state.as_deref());
    let near = stats.map(|s| s.near_player).unwrap_or(false);
    if shop.0 && (!ok || !near) {
        shop.0 = false;
        return;
    }
    if !ok || !near {
        return;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        shop.0 = !shop.0;
    }
}

/// Gère le curseur + les blockers selon l'ouverture. Ouvert → curseur libre +
/// look/tir bloqués ; à la fermeture → restaure gameplay (re-grab + déblocage).
/// Ne touche RIEN quand la fenêtre est fermée (n'interfère pas avec forgia-ui).
pub fn sys_forge_shop_cursor(
    shop: Res<ForgeShopOpen>,
    mut blockers: ResMut<InputBlockers>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut was_open: Local<bool>,
) {
    if shop.0 {
        blockers.block_look = true;
        blockers.block_fire = true;
        if let Ok(mut o) = q_cursor.single_mut() {
            if o.grab_mode != CursorGrabMode::None {
                o.grab_mode = CursorGrabMode::None;
                o.visible = true;
            }
        }
        *was_open = true;
    } else if *was_open {
        blockers.block_look = false;
        blockers.block_fire = false;
        if let Ok(mut o) = q_cursor.single_mut() {
            o.grab_mode = CursorGrabMode::Locked;
            o.visible = false;
        }
        *was_open = false;
    }
}

/// Prompt « [E] Parler au forgeron » quand proche + fenêtre fermée.
pub fn draw_forge_prompt(
    mut contexts: EguiContexts,
    shop: Res<ForgeShopOpen>,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    stats: Option<Res<MerchantStats>>,
) {
    if shop.0 || !shop_context_ok(&app_state, &game_mode, run_state.as_deref()) {
        return;
    }
    if !stats.map(|s| s.near_player).unwrap_or(false) {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    egui::Area::new(egui::Id::new("forgia_forge_prompt"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -120.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(FORGE_PANEL)
                .inner_margin(egui::Margin::symmetric(20, 12))
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(egui::Stroke::new(2.0, FORGE_OR))
                .show(ui, |ui| {
                    ui.label(display_text("[E] Parler au forgeron", 20.0, FORGE_OR).strong());
                });
        });
}

/// Fenêtre unique centrée : Forgeron (gauche) + Trempe (droite), boutons cliquables.
#[allow(clippy::too_many_arguments)]
pub fn draw_forge_shop_window(
    mut contexts: EguiContexts,
    shop: Res<ForgeShopOpen>,
    cat: Res<MerchantCatalogue>,
    gold: Option<Res<Gold>>,
    meta: Res<MetaSouls>,
    revive: Res<ReviveTokens>,
    trempe_cfg: Res<TrempeConfig>,
    trempe_state: Res<WeaponTrempeState>,
    equipped: Option<Res<EquippedWeapons>>,
    mut ev_purchase: MessageWriter<PurchaseRequest>,
    mut ev_temper: MessageWriter<TemperRequest>,
) {
    if !shop.0 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let or = gold.as_ref().map(|g| g.current).unwrap_or(0);
    let ames = meta.current;
    let w = equipped.map(|e| e.current).unwrap_or_default();
    let t_level = trempe_state.level_of(w);
    let t_cost = trempe_cfg.cost_for_next(t_level);
    let t_maxed = t_level >= trempe_cfg.level_cap;
    let t_mul = trempe_cfg.damage_mul_for_level(t_level);
    let t_afford = or >= t_cost;

    egui::Area::new(egui::Id::new("forgia_forge_shop"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(FORGE_PANEL)
                .inner_margin(egui::Margin::symmetric(30, 24))
                .corner_radius(egui::CornerRadius::same(14))
                .stroke(egui::Stroke::new(1.5, HAIR_GOLD_STRONG))
                .show(ui, |ui| {
                    ui.set_max_width(780.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Or : {or}      ◇ Âmes : {ames}      Second souffle : {}",
                                revive.0
                            ))
                            .size(18.0)
                            .strong()
                            .color(FORGE_TEAL),
                        );
                    });
                    ui.add_space(12.0);
                    ui.columns(2, |cols| {
                        // ── Colonne gauche : Forgeron itinérant ──
                        cols[0].vertical(|ui| {
                            ui.heading(
                                display_text("LE FORGERON ITINÉRANT", 24.0, FORGE_OR).strong(),
                            );
                            ui.add_space(8.0);
                            for (i, item) in cat.items.iter().enumerate() {
                                let bal = match item.currency {
                                    Currency::Or => or,
                                    Currency::Ames => ames,
                                };
                                let afford = bal >= item.cost;
                                let label = format!(
                                    "[{}]  {}  ·  {} {}",
                                    i + 1,
                                    item.name,
                                    item.cost,
                                    item.currency.label()
                                );
                                let btn = egui::Button::new(
                                    egui::RichText::new(label).size(16.0).color(if afford {
                                        FORGE_OR
                                    } else {
                                        C_TEXT_MUTED
                                    }),
                                );
                                if ui
                                    .add_enabled(afford, btn)
                                    .on_hover_text(item.desc.clone())
                                    .clicked()
                                {
                                    ev_purchase.write(PurchaseRequest { index: i });
                                }
                                ui.add_space(4.0);
                            }
                        });
                        // ── Colonne droite : La Trempe ──
                        cols[1].vertical(|ui| {
                            ui.heading(
                                display_text(
                                    format!("⚒ TREMPE — {}", w.display_name()),
                                    24.0,
                                    FORGE_TEAL,
                                )
                                .strong(),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "Palier {}/{}   ·   dégâts ×{:.2}",
                                    t_level, trempe_cfg.level_cap, t_mul
                                ))
                                .size(16.0)
                                .strong()
                                .color(FORGE_OR),
                            );
                            ui.add_space(8.0);
                            if t_maxed {
                                ui.label(
                                    egui::RichText::new("Arme trempée à fond !")
                                        .size(16.0)
                                        .color(FORGE_OR),
                                );
                            } else {
                                let label = format!(
                                    "Tremper  ·  {t_cost} Or  (+{:.0}% dégâts)",
                                    trempe_cfg.damage_per_level * 100.0
                                );
                                let btn = egui::Button::new(
                                    egui::RichText::new(label).size(16.0).color(if t_afford {
                                        FORGE_OR
                                    } else {
                                        C_TEXT_MUTED
                                    }),
                                );
                                if ui
                                    .add_enabled(t_afford && trempe_cfg.enabled, btn)
                                    .clicked()
                                {
                                    ev_temper.write(TemperRequest);
                                }
                            }
                        });
                    });
                    ui.add_space(12.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Clique un bouton (ou touches 1-4)  ·  [E] Fermer")
                                .size(14.0)
                                .color(C_TEXT_MUTED),
                        );
                    });
                });
        });
}

/// Anim procédurale du gobelin (rotation seule). Idle : balancement + respiration ;
/// « hop » (nod amorti) déclenché à l'ouverture de la fenêtre.
pub fn sys_animate_gobli(
    time: Res<Time>,
    shop: Res<ForgeShopOpen>,
    mut q: Query<&mut Transform, With<MerchantVendor>>,
    mut hop_t: Local<f32>,
    mut was_open: Local<bool>,
) {
    if shop.0 && !*was_open {
        *hop_t = GOBLI_HOP_DURATION; // déclenche le hop à l'ouverture
    }
    *was_open = shop.0;
    if *hop_t > 0.0 {
        *hop_t = (*hop_t - time.delta_secs()).max(0.0);
    }
    let t = time.elapsed_secs();
    let sway = (t * GOBLI_SWAY_FREQ).sin() * GOBLI_SWAY_AMP;
    let breathe = (t * GOBLI_BREATHE_FREQ).sin() * GOBLI_BREATHE_AMP;
    let hop = if *hop_t > 0.0 {
        (*hop_t / GOBLI_HOP_DURATION) * GOBLI_HOP_AMP * (*hop_t * GOBLI_HOP_FREQ).sin()
    } else {
        0.0
    };
    for mut tf in &mut q {
        tf.rotation = Quat::from_rotation_y(sway) * Quat::from_rotation_x(breathe + hop);
    }
}

// ─── Plugin ──────────────────────────────────────────────────────────────────

pub struct RogueliteForgeShopPlugin;

impl Plugin for RogueliteForgeShopPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sys_toggle_forge_shop, sys_forge_shop_cursor)
                .chain()
                .in_set(GameSet::UI)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_animate_gobli
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            EguiPrimaryContextPass,
            (draw_forge_shop_window, draw_forge_prompt).run_if(in_state(GameMode::Roguelite)),
        );
    }
}
