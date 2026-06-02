//! inventory_panel — Sacs style WoW (touche I), habillage cartoon Forge.
//!
//! Story-58x Phase 3 — grille de slots, bordure rareté, pastille + initiales,
//! count ×N, tooltip au survol, or en en-tête.
//! Phase 3b — interaction WoW : **clic gauche** ramasse/pose/échange (objet tenu
//! sur le curseur), **clic droit** utilise (émet `UseItemRequest`).
//!
//! LOCK-INV-1 : la capacité n'est jamais modifiée (take/place/swap restent dans
//! les slots existants). Header montre `X/80`.
//!
//! Data : `Inventory` / `Gold` (Components Player) + `ItemRegistry` (Res).
//! Gating : `GameMode::Rpg`.

use bevy::prelude::*;
use bevy_egui::egui::Color32;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::GameMode;
use forgia_player::Player;
use forgia_rpg_data::dialogue::DialogueSession;
use forgia_rpg_data::gold::Gold;
use forgia_rpg_data::inventory::{Inventory, InventoryEvent, ItemStack, UseItemRequest};
use forgia_rpg_data::items::ItemRegistry;
use forgia_rpg_data::shop::ShopSession;

use crate::style::*;
use crate::RpgOpenPanel;

pub mod prelude {
    pub use super::ForgiaUiInventoryPlugin;
}

const COLS: usize = 8;
const SLOT: f32 = 46.0;

/// Objet « tenu sur le curseur » pour le déplacement WoW-like (clic gauche).
#[derive(Resource, Default)]
pub struct InventoryCursor {
    pub held: Option<ItemStack>,
}

pub struct ForgiaUiInventoryPlugin;

impl Plugin for ForgiaUiInventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryCursor>()
            .init_resource::<RpgOpenPanel>()
            .add_systems(
                EguiPrimaryContextPass,
                draw_inventory_panel.run_if(in_state(GameMode::Rpg)),
            );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_inventory_panel(
    mut contexts: EguiContexts,
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: ResMut<RpgOpenPanel>,
    registry: Res<ItemRegistry>,
    mut cursor: ResMut<InventoryCursor>,
    mut q: Query<(Entity, &mut Inventory, Option<&Gold>), With<Player>>,
    blocking: Query<(), (With<Player>, Or<(With<ShopSession>, With<DialogueSession>)>)>,
    mut use_w: MessageWriter<UseItemRequest>,
    mut inv_w: MessageWriter<InventoryEvent>,
) {
    let blocked = !blocking.is_empty();
    // Toggle gaté : pas d'ouverture pendant boutique/dialogue (BUG-review-#2b).
    if keys.just_pressed(KeyCode::KeyI) && !blocked {
        *panel = if *panel == RpgOpenPanel::Inventory {
            RpgOpenPanel::None
        } else {
            RpgOpenPanel::Inventory
        };
    }
    let Ok((player, mut inv, gold)) = q.single_mut() else {
        return;
    };
    // Vendeur / dialogue actif → forcer la fermeture (évite la réouverture auto
    // quand la boutique/dialogue se referme — BUG-review-#2b).
    if blocked && *panel == RpgOpenPanel::Inventory {
        *panel = RpgOpenPanel::None;
    }
    let show = *panel == RpgOpenPanel::Inventory;
    if !show {
        // En masquant, rendre au sac l'objet éventuellement tenu (anti-perte).
        // BUG-review-#1 : gérer le leftover si le sac est plein (swap puis close).
        if let Some(held) = cursor.held.take() {
            if let Some(left) = inv.add(held) {
                warn!(
                    "[inventory] objet tenu '{}' x{} non re-rangé (sac plein)",
                    left.id.0, left.count
                );
                inv_w.write(InventoryEvent::Full {
                    entity: player,
                    dropped: left,
                });
            }
        }
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let gold_val = gold.map(|g| g.0).unwrap_or(0);
    let used = inv.slots().iter().filter(|s| s.is_some()).count();
    let cap = inv.capacity();

    let mut left_click: Option<usize> = None;
    let mut right_click: Option<usize> = None;

    let frame = egui::Frame::new()
        .fill(FORGE_BOIS_CLAIR)
        .stroke(egui::Stroke::new(4.0, FORGE_OR))
        .corner_radius(egui::CornerRadius::same(16))
        .inner_margin(18);

    egui::Window::new("forgia_inventory")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(frame)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Sacs")
                        .size(24.0)
                        .strong()
                        .color(FORGE_CHARBON),
                );
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(format!("🪙 {gold_val} or"))
                        .size(18.0)
                        .strong()
                        .color(FORGE_CHARBON)
                        .background_color(FORGE_OR),
                );
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(format!("{used}/{cap}"))
                        .size(15.0)
                        .color(FORGE_CHARBON),
                );
            });
            ui.add_space(10.0);

            egui::Grid::new("forgia_bag_grid")
                .spacing(egui::vec2(6.0, 6.0))
                .show(ui, |ui| {
                    for (idx, slot) in inv.slots().iter().enumerate() {
                        let resp = draw_slot(ui, slot, &registry);
                        if resp.clicked() {
                            left_click = Some(idx);
                        }
                        if resp.secondary_clicked() {
                            right_click = Some(idx);
                        }
                        if (idx + 1) % COLS == 0 {
                            ui.end_row();
                        }
                    }
                });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("I fermer · clic gauche déplacer · clic droit utiliser")
                    .size(12.0)
                    .italics()
                    .color(FORGE_CHARBON),
            );
        });

    // ── Appliquer les clics APRÈS le rendu (évite l'emprunt mut pendant render).
    if let Some(idx) = left_click {
        match cursor.held.take() {
            None => cursor.held = inv.take(idx),       // ramasser
            Some(held) => cursor.held = inv.place(idx, held), // poser / échanger
        }
    }
    if let Some(idx) = right_click {
        // Utiliser : seulement si on ne tient rien et que le slot a un objet.
        if cursor.held.is_none() && inv.slot(idx).is_some() {
            use_w.write(UseItemRequest {
                entity: player,
                slot: idx,
            });
        }
    }

    // ── Dessiner l'objet tenu, attaché au curseur, par-dessus tout.
    if let Some(held) = &cursor.held {
        if let Some(ptr) = ctx.pointer_latest_pos() {
            let fg = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("forgia_inv_cursor"),
            ));
            let r = egui::Rect::from_center_size(ptr, egui::vec2(40.0, 40.0));
            let border = registry
                .get(&held.id)
                .map(|d| rarity_color_tier(d.rarity.tier_index()))
                .unwrap_or(FORGE_METAL_CHAUD);
            fg.rect_filled(r, 6.0, FORGE_CREME);
            fg.rect_stroke(
                r,
                6.0,
                egui::Stroke::new(3.0, border),
                egui::StrokeKind::Inside,
            );
            let name = registry
                .get(&held.id)
                .map(|d| d.display_name.as_str())
                .unwrap_or(held.id.0.as_str());
            let initials: String = name
                .chars()
                .filter(|c| c.is_alphanumeric())
                .take(2)
                .collect::<String>()
                .to_uppercase();
            text_with_outline(
                &fg,
                ptr,
                egui::Align2::CENTER_CENTER,
                &initials,
                egui::FontId::proportional(16.0),
                border,
                1.0,
            );
            if held.count > 1 {
                text_with_outline(
                    &fg,
                    r.max - egui::vec2(3.0, 2.0),
                    egui::Align2::RIGHT_BOTTOM,
                    &format!("{}", held.count),
                    egui::FontId::proportional(12.0),
                    FORGE_CREME,
                    1.0,
                );
            }
        }
    }
}

/// Dessine un slot (vide ou plein) et renvoie sa `Response` (clics + survol).
fn draw_slot(ui: &mut egui::Ui, slot: &Option<ItemStack>, registry: &ItemRegistry) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(SLOT, SLOT), egui::Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6.0, FORGE_CREME);

    let Some(s) = slot else {
        p.rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(1.5, FORGE_METAL_CHAUD),
            egui::StrokeKind::Inside,
        );
        return resp;
    };

    let def = registry.get(&s.id);
    let border = def
        .map(|d| rarity_color_tier(d.rarity.tier_index()))
        .unwrap_or(FORGE_METAL_CHAUD);
    // Survol = surbrillance (lift cartoon).
    let outline_w = if resp.hovered() { 4.0 } else { 3.0 };
    p.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(outline_w, border),
        egui::StrokeKind::Inside,
    );

    let name = def.map(|d| d.display_name.as_str()).unwrap_or(s.id.0.as_str());
    let initials: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    text_with_outline(
        &p,
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &initials,
        egui::FontId::proportional(18.0),
        border,
        1.0,
    );
    if s.count > 1 {
        text_with_outline(
            &p,
            rect.max - egui::vec2(4.0, 2.0),
            egui::Align2::RIGHT_BOTTOM,
            &format!("{}", s.count),
            egui::FontId::proportional(13.0),
            FORGE_CREME,
            1.0,
        );
    }

    let (dn, desc, price, tier, usable) = match def {
        Some(d) => (
            d.display_name.clone(),
            d.description.clone(),
            d.buy_price,
            d.rarity.tier_index(),
            d.heal_amount > 0,
        ),
        None => (s.id.0.clone(), String::new(), 0, 0u8, false),
    };
    let count = s.count;
    resp.on_hover_ui(|ui| {
        ui.label(
            egui::RichText::new(dn)
                .size(15.0)
                .strong()
                .color(rarity_color_tier(tier)),
        );
        if !desc.is_empty() {
            ui.label(egui::RichText::new(desc).size(13.0).color(C_TEXT_LIGHT));
        }
        if price > 0 {
            ui.label(
                egui::RichText::new(format!("Valeur : {price} or"))
                    .size(12.0)
                    .color(FORGE_OR),
            );
        }
        ui.label(
            egui::RichText::new(format!("Quantité : {count}"))
                .size(12.0)
                .color(C_TEXT_MUTED),
        );
        if usable {
            ui.label(
                egui::RichText::new("Clic droit : utiliser")
                    .size(12.0)
                    .italics()
                    .color(FORGE_TEAL),
            );
        }
    })
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
