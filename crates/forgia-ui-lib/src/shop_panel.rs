//! shop_panel — Fenêtre vendeur style WoW (story-58x Phase 5), cartoon Forge.
//!
//! Visible tant que le Player a une `ShopSession`. Deux onglets : **Acheter**
//! (grille `ShopInventory.items` + prix, bouton grisé si or insuffisant) et
//! **Vendre** (slots de l'inventaire du joueur, vendus au `sell_ratio`).
//! Or du joueur en en-tête. Émet `BuyItem` / `SellItem` / `CloseShop`.
//!
//! Data : `ShopSession` / `ShopInventory` + `Inventory` / `Gold` + `ItemRegistry`.
//! Gating : `GameMode::Rpg`.

use bevy::prelude::*;
use bevy_egui::egui::Color32;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::GameMode;
use forgia_player::Player;
use forgia_rpg_data::gold::Gold;
use forgia_rpg_data::inventory::{Inventory, ItemId};
use forgia_rpg_data::items::ItemRegistry;
use forgia_rpg_data::shop::{BuyItem, CloseShop, SellItem, ShopInventory, ShopSession};

use crate::style::*;

pub mod prelude {
    pub use super::ForgiaUiShopPlugin;
}

/// Onglet actif du vendeur (`false` = Acheter, `true` = Vendre).
#[derive(Resource, Default)]
struct ShopTab {
    sell: bool,
}

pub struct ForgiaUiShopPlugin;

impl Plugin for ForgiaUiShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShopTab>().add_systems(
            EguiPrimaryContextPass,
            draw_shop.run_if(in_state(GameMode::Rpg)),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_shop(
    mut contexts: EguiContexts,
    mut tab: ResMut<ShopTab>,
    registry: Res<ItemRegistry>,
    players: Query<(Entity, &Inventory, Option<&Gold>, &ShopSession), With<Player>>,
    shops: Query<&ShopInventory>,
    mut buy_w: MessageWriter<BuyItem>,
    mut sell_w: MessageWriter<SellItem>,
    mut close_w: MessageWriter<CloseShop>,
) {
    let Ok((player, inv, gold, session)) = players.single() else {
        return;
    };
    let Ok(shop) = shops.get(session.npc) else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let gold_val = gold.map(|g| g.0).unwrap_or(0);

    let mut buy: Option<ItemId> = None;
    let mut sell: Option<usize> = None;
    let mut close = false;

    let frame = egui::Frame::new()
        .fill(FORGE_BOIS_CLAIR)
        .stroke(egui::Stroke::new(4.0, FORGE_OR))
        .corner_radius(egui::CornerRadius::same(16))
        .inner_margin(18);

    egui::Window::new("forgia_shop")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_min_width(440.0);
            // En-tête : titre + or + Fermer.
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Marchand")
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
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("✕ Fermer").size(15.0).color(FORGE_CHARBON),
                        )
                        .fill(FORGE_METAL_CHAUD)
                        .corner_radius(egui::CornerRadius::same(8)),
                    )
                    .clicked()
                {
                    close = true;
                }
            });
            ui.add_space(6.0);

            // Onglets Acheter / Vendre.
            ui.horizontal(|ui| {
                if tab_button(ui, "Acheter", !tab.sell) {
                    tab.sell = false;
                }
                if tab_button(ui, "Vendre", tab.sell) {
                    tab.sell = true;
                }
            });
            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    if !tab.sell {
                        // ── ACHETER ──
                        if shop.items.is_empty() {
                            ui.label(
                                egui::RichText::new("Rien à vendre pour l'instant.")
                                    .size(14.0)
                                    .color(FORGE_CHARBON),
                            );
                        }
                        for id in &shop.items {
                            let def = registry.get(id);
                            let name = def.map(|d| d.display_name.clone()).unwrap_or(id.0.clone());
                            let price = registry.buy_price_of(id);
                            let tier = def.map(|d| d.rarity.tier_index()).unwrap_or(0);
                            let afford = gold_val >= price;
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&name)
                                        .size(15.0)
                                        .strong()
                                        .color(rarity_color_tier(tier)),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let btn = egui::Button::new(
                                            egui::RichText::new(format!("Acheter · {price} or"))
                                                .size(14.0)
                                                .strong()
                                                .color(FORGE_CHARBON),
                                        )
                                        .fill(if afford { FORGE_OR } else { FORGE_METAL_CHAUD })
                                        .corner_radius(egui::CornerRadius::same(8))
                                        .min_size(egui::vec2(150.0, 30.0));
                                        if ui.add_enabled(afford, btn).clicked() {
                                            buy = Some(id.clone());
                                        }
                                    },
                                );
                            });
                        }
                    } else {
                        // ── VENDRE ──
                        let mut any = false;
                        for (idx, slot) in inv.slots().iter().enumerate() {
                            let Some(s) = slot else { continue };
                            let def = registry.get(&s.id);
                            let name =
                                def.map(|d| d.display_name.clone()).unwrap_or(s.id.0.clone());
                            let tier = def.map(|d| d.rarity.tier_index()).unwrap_or(0);
                            let unit = registry.buy_price_of(&s.id);
                            let value = ((unit as f32) * shop.sell_ratio).round() as u32;
                            let sellable = unit > 0;
                            any = true;
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{name} ×{}", s.count))
                                        .size(15.0)
                                        .strong()
                                        .color(rarity_color_tier(tier)),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let label = if sellable {
                                            format!("Vendre · {} or", value.max(1))
                                        } else {
                                            "Invendable".to_string()
                                        };
                                        let btn = egui::Button::new(
                                            egui::RichText::new(label)
                                                .size(14.0)
                                                .strong()
                                                .color(FORGE_CHARBON),
                                        )
                                        .fill(if sellable {
                                            FORGE_TEAL
                                        } else {
                                            FORGE_METAL_CHAUD
                                        })
                                        .corner_radius(egui::CornerRadius::same(8))
                                        .min_size(egui::vec2(150.0, 30.0));
                                        if ui.add_enabled(sellable, btn).clicked() {
                                            sell = Some(idx);
                                        }
                                    },
                                );
                            });
                        }
                        if !any {
                            ui.label(
                                egui::RichText::new("Ton sac est vide.")
                                    .size(14.0)
                                    .color(FORGE_CHARBON),
                            );
                        }
                    }
                });
        });

    if let Some(id) = buy {
        buy_w.write(BuyItem { player, id });
    }
    if let Some(slot) = sell {
        sell_w.write(SellItem { player, slot });
    }
    if close {
        close_w.write(CloseShop { player });
    }
}

/// Bouton d'onglet : surligné en or si actif. Renvoie `true` si cliqué.
fn tab_button(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let btn = egui::Button::new(
        egui::RichText::new(label)
            .size(16.0)
            .strong()
            .color(FORGE_CHARBON),
    )
    .fill(if active { FORGE_OR } else { FORGE_METAL_CHAUD })
    .corner_radius(egui::CornerRadius::same(8))
    .min_size(egui::vec2(120.0, 32.0));
    ui.add(btn).clicked()
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
