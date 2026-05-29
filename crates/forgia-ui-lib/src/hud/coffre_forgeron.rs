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
};

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

fn draw_coffre(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    session: Option<Res<CoffreSession>>,
    catalogue: Option<Res<BoonsCatalogue>>,
    mut picked: MessageWriter<CoffrePickedEvent>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    // Phase 2 defensive : si ForgiaBoonsPlugin n'est pas wiré côté
    // mode-roguelite (Phase 3), Resources absents → early-return silencieux.
    let (Some(session), Some(catalogue)) = (session, catalogue) else {
        return;
    };
    if !session.is_open || session.candidates.is_empty() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Area::new(egui::Id::new("forgia_coffre_forgeron"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            let cards_count = session.candidates.len();
            let card_w = 240.0;
            let card_h = 280.0;
            let gap = 24.0;
            let total_w = card_w * cards_count as f32 + gap * (cards_count as f32 - 1.0).max(0.0);
            let panel_w = total_w + 40.0;
            let panel_h = card_h + 110.0;

            let frame = egui::Frame::new()
                .fill(egui::Color32::from_rgba_premultiplied(15, 18, 24, 230))
                .stroke(egui::Stroke::new(3.0, C_PRIMARY))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(20.0);

            frame.show(ui, |ui| {
                ui.set_min_size(egui::vec2(panel_w, panel_h));
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("⚒  Le Maître Forgeron").size(22.0).strong().color(C_PRIMARY),
                    );
                    let voiceline = if session.maitre_voiceline.is_empty() {
                        "Choisis bien !"
                    } else {
                        &session.maitre_voiceline
                    };
                    ui.label(
                        egui::RichText::new(format!("« {voiceline} »"))
                            .size(15.0)
                            .italics()
                            .color(C_TEXT_LIGHT),
                    );
                    ui.add_space(14.0);
                });

                ui.horizontal(|ui| {
                    for (i, id) in session.candidates.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(gap);
                        }
                        if let Some(picked_id) = draw_card(ui, id, &catalogue, card_w, card_h) {
                            picked.write(CoffrePickedEvent { boon_id: picked_id });
                        }
                    }
                });
            });
        });
}

/// Returns Some(id) if the player clicked this card.
fn draw_card(
    ui: &mut egui::Ui,
    id: &BoonId,
    catalogue: &BoonsCatalogue,
    w: f32,
    h: f32,
) -> Option<BoonId> {
    let def = catalogue.find(id);
    let (size, response) =
        ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click().union(egui::Sense::hover()));
    let rect = response.rect;
    let _ = size;

    let painter = ui.painter_at(rect);

    // Card background + rarity outline.
    let rarity_color = def.map(|d| rarity_color(d.rarity)).unwrap_or(C_TEXT_MUTED);
    let bg = if response.hovered() {
        egui::Color32::from_rgba_premultiplied(35, 40, 52, 240)
    } else {
        egui::Color32::from_rgba_premultiplied(22, 26, 34, 230)
    };
    painter.rect_filled(rect, 8.0, bg);
    let outline_w = if response.hovered() { 4.0 } else { 2.5 };
    painter.rect_stroke(
        rect,
        8.0,
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

    // Rarity ribbon (top).
    let ribbon_h = 28.0;
    let ribbon_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), ribbon_h));
    painter.rect_filled(
        ribbon_rect,
        egui::CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        },
        rarity_color,
    );
    painter.text(
        ribbon_rect.center(),
        egui::Align2::CENTER_CENTER,
        rarity_label(def.rarity),
        egui::FontId::proportional(13.0),
        C_OUTLINE,
    );

    // Name (large, centered top).
    let name_y = rect.min.y + ribbon_h + 18.0;
    text_with_outline(
        &painter,
        egui::pos2(rect.center().x, name_y),
        egui::Align2::CENTER_TOP,
        &def.name,
        egui::FontId::proportional(18.0),
        C_TEXT_LIGHT,
        1.5,
    );

    // Voiceline preview (italic, mid).
    let vl_y = name_y + 48.0;
    let voiceline = format!("« {} »", def.voiceline_preview);
    painter.text(
        egui::pos2(rect.center().x, vl_y),
        egui::Align2::CENTER_TOP,
        &voiceline,
        egui::FontId::proportional(13.0),
        C_TEXT_MUTED,
    );

    // Effect summary (highlighted).
    let effect_y = vl_y + 56.0;
    let effect_line = format_effect(&def.effect);
    text_with_outline(
        &painter,
        egui::pos2(rect.center().x, effect_y),
        egui::Align2::CENTER_TOP,
        &effect_line,
        egui::FontId::proportional(15.0),
        C_ACCENT,
        1.0,
    );

    // Tag chips (bottom).
    if !def.tags.is_empty() {
        let chip_y = rect.max.y - 28.0;
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

    response.clicked().then(|| def.id.clone())
}

fn rarity_color(r: BoonRarity) -> egui::Color32 {
    match r {
        BoonRarity::Common => C_TEXT_MUTED,
        BoonRarity::Uncommon => C_HP_HIGH,
        BoonRarity::Rare => C_ACCENT,
        BoonRarity::Legendary => C_PRIMARY,
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
