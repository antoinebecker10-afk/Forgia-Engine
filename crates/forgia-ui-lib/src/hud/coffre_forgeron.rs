//! Coffre du Forgeron — story-529 Phase 2 (Mission 2.3 GDD AC3).
//!
//! UI modale 3 cartes après wave clear. Lit `CoffreSession` (set par
//! `forgia-rpg-data::boons::sys_handle_open_coffre`) + `BoonsCatalogue` pour
//! détails. Click → `CoffrePickedEvent` → apply system Phase 2.
//!
//! Style cartoon bible v1 : Maître Forgeron au-dessus, 3 cartes empilées
//! horizontalement, hover = surlignage orange Forgia. Pas de voiceline audio
//! (Tier 3 post-MVP) — `voiceline_preview` est affiché sous le nom comme
//! texte popup BD (cf reference_industry_3_gaps_forgia_roguelite gap #1).
//!
//! ## Gating
//!
//! Affiché uniquement `AppMode::InGame` + `GameMode::Roguelite`. Sinon
//! ignoré silencieusement (defense-in-depth — `is_open=false` est la garde
//! primaire).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_rpg_data::boons::{
    BoonEffectKind, BoonId, BoonRarity, BoonsCatalogue, CoffrePickedEvent, CoffreSession,
    OpenCoffreRequest,
};
use forgia_rpg_data::loot_tables::Souls;

/// Story-558 Phase 3 (2026-05-29) — coût Souls pour reforger l'offre.
/// Default 30 souls — kid-friendly (1-2 kills ennemis Tank), illimité par break.
/// Pattern Hadès Diamant reroll.
const REROLL_COST: u32 = 30;

use crate::style::*;

pub struct CoffreForgeronPlugin;

impl Plugin for CoffreForgeronPlugin {
    fn build(&self, app: &mut App) {
        // Defensive registration : si ForgiaBoonsPlugin pas encore wiré, on
        // garantit que MessageWriter<CoffrePickedEvent> est valide. Bevy
        // add_message est idempotent pour le même type.
        app.add_message::<CoffrePickedEvent>()
            .add_systems(EguiPrimaryContextPass, draw_coffre);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_coffre(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    mut session: Option<ResMut<CoffreSession>>,
    catalogue: Option<Res<BoonsCatalogue>>,
    souls: Option<ResMut<Souls>>,
    mut picked: MessageWriter<CoffrePickedEvent>,
    mut reroll: MessageWriter<OpenCoffreRequest>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    // Phase 2 defensive : si ForgiaBoonsPlugin n'est pas wiré côté
    // mode-roguelite (Phase 3), Resources absents → early-return silencieux.
    let (Some(session_res), Some(catalogue)) = (session.as_deref_mut(), catalogue) else {
        return;
    };
    if !session_res.is_open || session_res.candidates.is_empty() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let souls_current = souls.as_ref().map(|s| s.current).unwrap_or(0);
    let mut should_close = false;
    let mut should_reroll = false;

    egui::Area::new(egui::Id::new("forgia_coffre_forgeron"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            let cards_count = session_res.candidates.len();
            let card_w = 240.0;
            let card_h = 280.0;
            let gap = 24.0;
            let total_w = card_w * cards_count as f32 + gap * (cards_count as f32 - 1.0).max(0.0);
            let panel_w = total_w + 40.0;
            // +56 pour la barre de boutons Skip / Reroll en bas.
            let panel_h = card_h + 110.0 + 56.0;

            // Story-558 Phase 7 — cartoon kid-friendly : panel bois clair +
            // border or 5px + shadow stack 4 rects (Hadès dialogue pattern).
            let frame = egui::Frame::new()
                .fill(FORGE_BOIS_CLAIR)
                .stroke(egui::Stroke::new(5.0, FORGE_OR))
                .corner_radius(egui::CornerRadius::same(18))
                .inner_margin(22);

            frame.show(ui, |ui| {
                ui.set_min_size(egui::vec2(panel_w, panel_h));
                ui.vertical_centered(|ui| {
                    // Story-558 Phase 7 — header cartoon : titre charbon gros sur bois.
                    ui.label(
                        egui::RichText::new("⚒  LE MAÎTRE FORGERON")
                            .size(28.0)
                            .strong()
                            .color(FORGE_CHARBON),
                    );
                    let voiceline = if session_res.maitre_voiceline.is_empty() {
                        "Choisis ta bénédiction, petit forgeron !"
                    } else {
                        &session_res.maitre_voiceline
                    };
                    ui.label(
                        egui::RichText::new(format!("« {voiceline} »"))
                            .size(17.0)
                            .italics()
                            .color(FORGE_CHARBON),
                    );
                    ui.add_space(8.0);
                    // Story-571 — le Coffre dépense l'OR (in-run), pas les Souls méta.
                    ui.label(
                        egui::RichText::new(format!("🪙  {souls_current} or"))
                            .size(20.0)
                            .strong()
                            .color(FORGE_CHARBON)
                            .background_color(FORGE_OR),
                    );
                    ui.add_space(14.0);
                });

                ui.horizontal(|ui| {
                    for (i, id) in session_res.candidates.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(gap);
                        }
                        if let Some(picked_id) =
                            draw_card(ui, id, &catalogue, souls_current, card_w, card_h)
                        {
                            picked.write(CoffrePickedEvent { boon_id: picked_id });
                        }
                    }
                });

                ui.add_space(18.0);
                // AC5 — barre Skip + Reroll cartoon kid-friendly.
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    let skip_btn = egui::Button::new(
                        egui::RichText::new("✕  Passer")
                            .size(20.0)
                            .strong()
                            .color(FORGE_CHARBON),
                    )
                    .fill(FORGE_METAL_CHAUD)
                    .stroke(egui::Stroke::new(3.0, FORGE_CHARBON))
                    .corner_radius(egui::CornerRadius::same(12))
                    .min_size(egui::vec2(180.0, 48.0));
                    if ui.add(skip_btn).clicked() {
                        should_close = true;
                    }
                    ui.add_space(gap);
                    let can_reroll = souls_current >= REROLL_COST;
                    let (btn_fill, btn_text) = if can_reroll {
                        (FORGE_OR, FORGE_CHARBON)
                    } else {
                        (FORGE_METAL_CHAUD, FORGE_CHARBON)
                    };
                    let reroll_btn = egui::Button::new(
                        egui::RichText::new(format!("↻  Reforger  ({REROLL_COST} ◇)"))
                            .size(20.0)
                            .strong()
                            .color(btn_text),
                    )
                    .fill(btn_fill)
                    .stroke(egui::Stroke::new(3.0, FORGE_CHARBON))
                    .corner_radius(egui::CornerRadius::same(12))
                    .min_size(egui::vec2(240.0, 48.0));
                    if ui.add_enabled(can_reroll, reroll_btn).clicked() {
                        should_reroll = true;
                    }
                });
            });
        });

    // Apply Skip / Reroll après show pour éviter borrow conflict.
    if should_close {
        session_res.is_open = false;
        session_res.candidates.clear();
        session_res.maitre_voiceline.clear();
        session_res.source.clear();
        info!("[coffre] skipped by player");
    } else if should_reroll {
        if let Some(mut s) = souls {
            if s.current >= REROLL_COST {
                s.current = s.current.saturating_sub(REROLL_COST);
                // Ferme la session courante → sys_handle_open_coffre va la rouvrir
                // avec nouveaux candidates au prochain OpenCoffreRequest.
                session_res.is_open = false;
                session_res.candidates.clear();
                reroll.write(OpenCoffreRequest::wave_clear());
                info!(
                    "[coffre] reroll ({REROLL_COST} souls spent, remaining {})",
                    s.current
                );
            }
        }
    }
}

/// Returns Some(id) if the player clicked this card AND can afford it.
/// Story-558 Phase 3 — cards disabled si `souls_current < effective_souls_cost`.
fn draw_card(
    ui: &mut egui::Ui,
    id: &BoonId,
    catalogue: &BoonsCatalogue,
    souls_current: u32,
    w: f32,
    h: f32,
) -> Option<BoonId> {
    let def = catalogue.find(id);
    let cost = def.map(|d| d.effective_souls_cost()).unwrap_or(u32::MAX);
    let affordable = souls_current >= cost;
    let (size, response) =
        ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click().union(egui::Sense::hover()));
    let rect = response.rect;
    let _ = size;

    let painter = ui.painter_at(rect);

    // Story-558 Phase 7 — card cartoon : fond crème + border rarity 5px.
    // Hover : shadow stack + border ×1.5 (lift cartoon Hearthstone).
    let rarity_color = def.map(|d| rarity_color(d.rarity)).unwrap_or(FORGE_METAL_CHAUD);
    if response.hovered() {
        cartoon_drop_shadow(&painter, rect, 14.0, egui::vec2(6.0, 6.0));
    }
    painter.rect_filled(rect, 14.0, FORGE_CREME);
    let outline_w = if response.hovered() { 6.0 } else { 4.0 };
    painter.rect_stroke(
        rect,
        14.0,
        egui::Stroke::new(outline_w, rarity_color),
        egui::StrokeKind::Inside,
    );

    let Some(def) = def else {
        // Catalogue missing — degraded card with id only.
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("???\n{}", id.0),
            egui::FontId::proportional(14.0),
            C_HP_LOW,
        );
        return response.clicked().then(|| id.clone());
    };

    // Rarity ribbon cartoon — Hearthstone Diablo color + text crème.
    let ribbon_h = 32.0;
    let ribbon_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), ribbon_h));
    painter.rect_filled(
        ribbon_rect,
        egui::CornerRadius {
            nw: 14,
            ne: 14,
            sw: 0,
            se: 0,
        },
        rarity_color,
    );
    painter.text(
        ribbon_rect.center(),
        egui::Align2::CENTER_CENTER,
        rarity_label(def.rarity),
        egui::FontId::proportional(15.0),
        FORGE_CREME,
    );

    // Story-558 Phase 7 — texte charbon sur crème (contraste 12:1 WCAG AAA).
    let name_y = rect.min.y + ribbon_h + 20.0;
    text_with_outline(
        &painter,
        egui::pos2(rect.center().x, name_y),
        egui::Align2::CENTER_TOP,
        &def.name,
        egui::FontId::proportional(20.0),
        FORGE_CHARBON,
        1.0,
    );

    // Voiceline preview (italic, mid).
    let vl_y = name_y + 50.0;
    let voiceline = format!("« {} »", def.voiceline_preview);
    painter.text(
        egui::pos2(rect.center().x, vl_y),
        egui::Align2::CENTER_TOP,
        &voiceline,
        egui::FontId::proportional(14.0),
        FORGE_CHARBON.linear_multiply(0.7),
    );

    // Effect summary (highlighted braise).
    let effect_y = vl_y + 56.0;
    let effect_line = format_effect(&def.effect);
    text_with_outline(
        &painter,
        egui::pos2(rect.center().x, effect_y),
        egui::Align2::CENTER_TOP,
        &effect_line,
        egui::FontId::proportional(16.0),
        FORGE_BRAISE,
        1.0,
    );

    // Tag chips (mid-bottom — laisser place au cost en bas).
    if !def.tags.is_empty() {
        let chip_y = rect.max.y - 54.0;
        let tags_text = def
            .tags
            .iter()
            .map(tag_label)
            .collect::<Vec<_>>()
            .join("  ·  ");
        painter.text(
            egui::pos2(rect.center().x, chip_y),
            egui::Align2::CENTER_CENTER,
            tags_text,
            egui::FontId::monospace(12.0),
            rarity_color,
        );
    }

    // Story-558 Phase 3+7 — souls_cost cartoon : badge or si abordable, rouge sinon.
    let cost_y = rect.max.y - 26.0;
    let cost_bg = if affordable { FORGE_OR } else { FORGE_BRAISE };
    // Badge rectangulaire centré
    let badge_w = 100.0;
    let badge_h = 36.0;
    let badge_rect = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, cost_y),
        egui::vec2(badge_w, badge_h),
    );
    painter.rect_filled(badge_rect, 8.0, cost_bg);
    painter.rect_stroke(
        badge_rect,
        8.0,
        egui::Stroke::new(2.5, FORGE_CHARBON),
        egui::StrokeKind::Inside,
    );
    painter.text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("◇ {cost}"),
        egui::FontId::proportional(22.0),
        FORGE_CHARBON,
    );

    // Greyout cartoon si pas abordable — desaturate + message kid-friendly.
    if !affordable {
        painter.rect_filled(
            rect,
            14.0,
            egui::Color32::from_rgba_premultiplied(80, 60, 40, 140),
        );
        painter.text(
            egui::pos2(rect.center().x, rect.center().y + 10.0),
            egui::Align2::CENTER_CENTER,
            "Pas encore !",
            egui::FontId::proportional(18.0),
            FORGE_CREME,
        );
    }

    // Click ne vaut que si abordable (AC3 gate UI-side ; sys_handle_coffre_pick
    // est defense-in-depth backend).
    if !affordable {
        return None;
    }
    response.clicked().then(|| def.id.clone())
}

fn rarity_color(r: BoonRarity) -> egui::Color32 {
    // Story-558 Phase 7 — convention Hearthstone/Diablo kid-friendly.
    // Universellement lisible enfants (color-blind safe via icon + texte).
    match r {
        BoonRarity::Common => FORGE_RARITY_COMMON,
        BoonRarity::Uncommon => FORGE_RARITY_UNCOMMON,
        BoonRarity::Rare => FORGE_RARITY_RARE,
        BoonRarity::Legendary => FORGE_RARITY_LEGENDARY,
    }
}

fn rarity_label(r: BoonRarity) -> &'static str {
    match r {
        BoonRarity::Common => "COMMUN",
        BoonRarity::Uncommon => "RARE",
        BoonRarity::Rare => "ÉPIQUE",
        BoonRarity::Legendary => "LÉGENDAIRE",
    }
}

fn tag_label(t: &forgia_rpg_data::boons::BoonTag) -> String {
    use forgia_rpg_data::boons::BoonTag::*;
    match t {
        Fire => "feu",
        Ricochet => "ricochet",
        Knockback => "souffle",
        Chain => "chaîne",
        Precision => "précision",
        Chaos => "chaos",
        Other => "?",
    }
    .into()
}

fn format_effect(e: &BoonEffectKind) -> String {
    match e {
        BoonEffectKind::DamageMul { factor } => format!("Dégâts ×{factor:.2}"),
        BoonEffectKind::HealOnKill { hp } => format!("+{hp:.0} énergie par élimination"),
        BoonEffectKind::FireRateMul { factor } => format!("Cadence ×{factor:.2}"),
        BoonEffectKind::ChainTargets { count } => format!("+{count} cibles chaîne"),
        BoonEffectKind::Knockback { strength } => format!("Souffle {strength:.0}"),
        BoonEffectKind::DamageReduction { factor } => {
            format!("-{:.0}% dégâts subis", factor * 100.0)
        }
        BoonEffectKind::FlatBonus { stat, amount } => format!("+{amount:.2} {stat}"),
    }
}
