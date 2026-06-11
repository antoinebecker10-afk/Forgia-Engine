//! meta_shop.rs — Story-591. L'Enclume des Âmes : méta-progression permanente.
//!
//! Le sink inter-run qui manquait : les Âmes (`MetaSouls`) s'accumulent mais
//! n'avaient nulle part où être dépensées ET n'étaient pas sauvées sur disque
//! (perdues au reboot). Ici : un **hub Lobby** (l'Enclume) où le joueur dépense
//! ses Âmes en upgrades PERMANENTS qui s'appliquent au début de chaque run et
//! **persistent sur disque** entre les sessions.
//!
//! ## Hooks (vérifiés sans édition cross-crate)
//! - **Vitalité** → `forgia_damage::Health.max` au run-start (miroir du reset HP).
//! - **Puissance** → `PlayerCombatMods.damage_mul` via [`PermanentPlayerMods`].
//! - **Armure** → `PlayerCombatMods.damage_reduction` (→ HealthGuard).
//! - **Pactole** → `Gold.current` de départ au run-start.
//!
//! (vitesse droppée : `MovementSpeedMultiplier` écrasé chaque frame par l'ADS.)
//!
//! ## Persistance
//! Pattern config Forgia (`fs` + `serde` + `toml`). Fichier `meta_shop_save.toml`
//! dans le dossier `config/`. Save événementiel (achat + OnExit + fin de run),
//! réconciliation `souls_total = MetaSouls.current` avant chaque write. Load au
//! Startup → `MetaSouls.current` (1×, évite l'écrasement au re-entry).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_ui_lib::style::{C_HP_HIGH, C_TEXT_MUTED, FORGE_OR, FORGE_PANEL, FORGE_TEAL};
use forgia_ui_lib::theme::display_text;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::run::{MetaSouls, RunState, StartRunEvent};

const SAVE_VERSION: u32 = 1;
const SAVE_FILE: &str = "meta_shop_save.toml";
const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_meta_shop.toml";
/// PV de base du joueur (= `DamageHealth::new(100)` dans forgia-player).
pub const BASE_PLAYER_HP: f32 = 100.0;

// ─── Effet d'un upgrade ─────────────────────────────────────────────────────

/// Effet permanent d'un upgrade, par rang (l'amount est ajouté `rank` fois).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MetaEffect {
    /// +N PV max par rang.
    MaxHp(f32),
    /// +N (fraction) au multiplicateur de dégâts par rang (0.08 = +8%).
    DamageMul(f32),
    /// +N (fraction) de réduction de dégâts par rang (cumul clampé à 0.85).
    DamageReduction(f32),
    /// +N Or de départ par rang.
    StartGold(u32),
}

impl MetaEffect {
    fn from_key(key: &str, amount: f32) -> Option<Self> {
        match key.trim().to_ascii_lowercase().as_str() {
            "max_hp" | "maxhp" | "pv" => Some(MetaEffect::MaxHp(amount)),
            "damage_mul" | "damage" | "degats" => Some(MetaEffect::DamageMul(amount)),
            "damage_reduction" | "armor" | "armure" => Some(MetaEffect::DamageReduction(amount)),
            "start_gold" | "gold" | "or" => Some(MetaEffect::StartGold(amount.max(0.0) as u32)),
            _ => None,
        }
    }
}

// ─── Catalogue (data-driven, miroir const) ──────────────────────────────────

#[derive(Clone, Debug)]
pub struct MetaUpgrade {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub effect: MetaEffect,
    /// Coût par rang ; `len()` = rang max.
    pub costs: Vec<u32>,
}

impl MetaUpgrade {
    pub fn max_rank(&self) -> u32 {
        self.costs.len() as u32
    }
    /// Coût pour passer de `rank` à `rank+1` (None = déjà au max).
    pub fn cost_for_next(&self, rank: u32) -> Option<u32> {
        self.costs.get(rank as usize).copied()
    }
}

#[derive(Resource, Clone, Debug)]
pub struct MetaShopCatalogue {
    pub upgrades: Vec<MetaUpgrade>,
}

impl Default for MetaShopCatalogue {
    fn default() -> Self {
        // Miroir EXACT de assets/genomes/roguelite/roguelite_meta_shop.toml.
        Self {
            upgrades: vec![
                MetaUpgrade {
                    id: "max_hp".into(),
                    name: "Vitalité".into(),
                    desc: "+15 PV max".into(),
                    effect: MetaEffect::MaxHp(15.0),
                    costs: vec![20, 40, 70, 110, 160],
                },
                MetaUpgrade {
                    id: "damage".into(),
                    name: "Puissance".into(),
                    desc: "+8% dégâts".into(),
                    effect: MetaEffect::DamageMul(0.08),
                    costs: vec![25, 50, 85, 130, 190],
                },
                MetaUpgrade {
                    id: "armor".into(),
                    name: "Armure".into(),
                    desc: "+5% réduction de dégâts".into(),
                    effect: MetaEffect::DamageReduction(0.05),
                    costs: vec![30, 60, 100, 150],
                },
                MetaUpgrade {
                    id: "gold".into(),
                    name: "Pactole".into(),
                    desc: "+50 Or de départ".into(),
                    effect: MetaEffect::StartGold(50),
                    costs: vec![15, 35, 60],
                },
            ],
        }
    }
}

#[derive(Deserialize)]
struct UpgradeToml {
    id: String,
    name: String,
    desc: String,
    effect: String,
    amount: f32,
    costs: Vec<u32>,
}

#[derive(Deserialize)]
struct CatalogueToml {
    #[serde(default)]
    upgrades: Vec<UpgradeToml>,
}

impl MetaShopCatalogue {
    /// Pur — testable. Fallback `Default` si parse KO ou liste vide.
    pub fn parse_toml(content: &str) -> Self {
        let Ok(parsed) = toml::from_str::<CatalogueToml>(content) else {
            return Self::default();
        };
        let upgrades: Vec<MetaUpgrade> = parsed
            .upgrades
            .into_iter()
            .filter_map(|u| {
                MetaEffect::from_key(&u.effect, u.amount).map(|effect| MetaUpgrade {
                    id: u.id,
                    name: u.name,
                    desc: u.desc,
                    effect,
                    costs: u.costs,
                })
            })
            .collect();
        if upgrades.is_empty() {
            Self::default()
        } else {
            Self { upgrades }
        }
    }

    fn load_or_default() -> Self {
        match std::fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

// ─── Save disque (source de vérité des Âmes accumulées) ─────────────────────

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct MetaShopSave {
    pub version: u32,
    pub souls_total: u32,
    pub ranks: HashMap<String, u32>,
}

impl Default for MetaShopSave {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            souls_total: 0,
            ranks: HashMap::new(),
        }
    }
}

/// Walk-up depuis l'exe pour trouver `config/` (marqueur `config/biomes/`),
/// fallback `config/`. Réplique locale de `forgia_terrain::config_dir` (évite
/// une dépendance crate pour un simple chemin de save).
fn config_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let mut cursor: Option<&Path> = exe.parent();
        while let Some(d) = cursor {
            if d.join("config").join("biomes").exists() {
                return d.join("config");
            }
            cursor = d.parent();
        }
    }
    PathBuf::from("config")
}

impl MetaShopSave {
    pub fn rank(&self, id: &str) -> u32 {
        self.ranks.get(id).copied().unwrap_or(0)
    }

    fn save_path() -> PathBuf {
        config_dir().join(SAVE_FILE)
    }

    pub fn load_or_default() -> Self {
        match std::fs::read_to_string(Self::save_path()) {
            Ok(c) => toml::from_str(&c).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        match toml::to_string_pretty(self) {
            Ok(s) => {
                if let Err(e) = std::fs::write(Self::save_path(), s) {
                    warn!("[meta-shop] save failed: {e}");
                }
            }
            Err(e) => warn!("[meta-shop] serialize failed: {e}"),
        }
    }

    // ── Bonus cumulés (lus au run-start) ──
    pub fn max_hp_bonus(&self, cat: &MetaShopCatalogue) -> f32 {
        cat.upgrades
            .iter()
            .filter_map(|u| match u.effect {
                MetaEffect::MaxHp(a) => Some(a * self.rank(&u.id) as f32),
                _ => None,
            })
            .sum()
    }
    pub fn damage_mul(&self, cat: &MetaShopCatalogue) -> f32 {
        1.0 + cat
            .upgrades
            .iter()
            .filter_map(|u| match u.effect {
                MetaEffect::DamageMul(a) => Some(a * self.rank(&u.id) as f32),
                _ => None,
            })
            .sum::<f32>()
    }
    pub fn damage_reduction(&self, cat: &MetaShopCatalogue) -> f32 {
        cat.upgrades
            .iter()
            .filter_map(|u| match u.effect {
                MetaEffect::DamageReduction(a) => Some(a * self.rank(&u.id) as f32),
                _ => None,
            })
            .sum::<f32>()
            .min(0.85)
    }
    pub fn start_gold(&self, cat: &MetaShopCatalogue) -> u32 {
        cat.upgrades
            .iter()
            .filter_map(|u| match u.effect {
                MetaEffect::StartGold(a) => Some(a * self.rank(&u.id)),
                _ => None,
            })
            .sum()
    }
}

/// Mods permanents (méta) composés dans `PlayerCombatMods` par boons_apply.
/// Séparés des boons (per-run) pour ne pas être écrasés au recompute.
#[derive(Resource, Debug, Clone, Copy)]
pub struct PermanentPlayerMods {
    pub damage_mul: f32,
    pub damage_reduction: f32,
}

impl Default for PermanentPlayerMods {
    fn default() -> Self {
        Self { damage_mul: 1.0, damage_reduction: 0.0 }
    }
}

// ─── Systems ─────────────────────────────────────────────────────────────────

/// Startup — charge le save disque (1×) → `MetaSouls.current` + insère les
/// Resources `MetaShopSave` / `MetaShopCatalogue`.
pub fn sys_load_meta_shop(mut commands: Commands, mut meta: ResMut<MetaSouls>) {
    let save = MetaShopSave::load_or_default();
    let cat = MetaShopCatalogue::load_or_default();
    meta.current = save.souls_total;
    info!(
        "[meta-shop] loaded — souls={} ranks={} upgrades={}",
        save.souls_total,
        save.ranks.len(),
        cat.upgrades.len()
    );
    commands.insert_resource(save);
    commands.insert_resource(cat);
}

/// Réconcilie + sauve (OnExit Roguelite + OnEnter Victory/Defeat).
pub fn sys_flush_meta_save(meta: Res<MetaSouls>, mut save: ResMut<MetaShopSave>) {
    save.souls_total = meta.current;
    save.save();
}

/// OnEnter Lobby — hub PROPRE : purge les ennemis survivants (après une Defeat
/// avec des bots vivants) et ressuscite le joueur (HP au max) pour qu'il puisse
/// shopper tranquillement avant de relancer.
pub fn sys_lobby_cleanup(
    mut commands: Commands,
    q_enemies: Query<Entity, With<forgia_ai_arena_bot::ArenaBot>>,
) {
    let mut purged = 0u32;
    for e in &q_enemies {
        commands.entity(e).despawn();
        purged += 1;
    }
    if purged > 0 {
        info!("[meta-shop] Lobby — purge {purged} ennemis restants");
    }
    commands.queue(|world: &mut World| {
        let mut q =
            world.query_filtered::<&mut forgia_damage::Health, With<forgia_player::Player>>();
        if let Ok(mut hp) = q.single_mut(world) {
            hp.current = hp.max;
        }
    });
}

/// Hub Lobby : touches 1-4 = achat, ENTRÉE = lancer la run. Clavier-only (pas de
/// curseur à libérer). Gaté `run_if(in_state(RunState::Lobby))`.
pub fn sys_meta_shop_input(
    keys: Res<ButtonInput<KeyCode>>,
    cat: Res<MetaShopCatalogue>,
    mut save: ResMut<MetaShopSave>,
    mut meta: ResMut<MetaSouls>,
    mut start_run: MessageWriter<StartRunEvent>,
) {
    // Lancer la run (ENTRÉE) — réconcilie + sauve d'abord.
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        save.souls_total = meta.current;
        save.save();
        start_run.write(StartRunEvent { seed: None });
        return;
    }
    // Achat 1..=4.
    let idx = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .iter()
    .position(|k| keys.just_pressed(*k));
    let Some(i) = idx else {
        return;
    };
    let Some(up) = cat.upgrades.get(i) else {
        return;
    };
    let rank = save.rank(&up.id);
    let Some(cost) = up.cost_for_next(rank) else {
        info!("[meta-shop] {} déjà au rang max", up.name);
        return;
    };
    if meta.current < cost {
        info!("[meta-shop] pas assez d'âmes pour {} ({}/{})", up.name, meta.current, cost);
        return;
    }
    meta.current -= cost;
    *save.ranks.entry(up.id.clone()).or_insert(0) += 1;
    save.souls_total = meta.current;
    save.save();
    info!(
        "[meta-shop] acheté {} rang {} (-{} âmes, reste {})",
        up.name,
        rank + 1,
        cost,
        meta.current
    );
}

/// Dessine l'Enclume au Lobby (EguiPrimaryContextPass).
pub fn draw_meta_shop_lobby(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    cat: Res<MetaShopCatalogue>,
    save: Res<MetaShopSave>,
    meta: Res<MetaSouls>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    if !matches!(run_state.as_deref().map(|s| s.get()), Some(RunState::Lobby)) {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Area::new(egui::Id::new("forgia_meta_shop"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // Story-596 — couleurs palette Forge partagée (étaient des littéraux
            // locaux dupliquant FORGE_OR & co) + titre display font.
            egui::Frame::new()
                .fill(FORGE_PANEL)
                .inner_margin(egui::Margin::symmetric(44, 30))
                .corner_radius(egui::CornerRadius::same(14))
                .stroke(egui::Stroke::new(4.0, FORGE_OR))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading(display_text("L'ENCLUME DES ÂMES", 40.0, FORGE_OR).strong());
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(format!("◇ Âmes : {}", meta.current))
                                .size(24.0)
                                .strong()
                                .color(FORGE_TEAL),
                        );
                        ui.add_space(16.0);
                        for (i, up) in cat.upgrades.iter().enumerate() {
                            let rank = save.rank(&up.id);
                            let max = up.max_rank();
                            let (text, col) = match up.cost_for_next(rank) {
                                Some(cost) => {
                                    let afford = meta.current >= cost;
                                    (
                                        format!(
                                            "[{}]  {} — {}  (rang {}/{})  ·  {} âmes",
                                            i + 1,
                                            up.name,
                                            up.desc,
                                            rank,
                                            max,
                                            cost
                                        ),
                                        if afford { FORGE_OR } else { C_TEXT_MUTED },
                                    )
                                }
                                None => (
                                    format!(
                                        "[—]  {} — {}  (MAX {}/{})",
                                        up.name, up.desc, max, max
                                    ),
                                    C_HP_HIGH,
                                ),
                            };
                            ui.label(egui::RichText::new(text).size(19.0).color(col));
                            ui.add_space(4.0);
                        }
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new("Touches 1-4 = acheter   ·   ENTRÉE = lancer la run")
                                .size(18.0)
                                .color(C_TEXT_MUTED),
                        );
                    });
                });
        });
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

pub struct MetaShopPlugin;

impl Plugin for MetaShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MetaShopSave>();
        app.init_resource::<MetaShopCatalogue>();
        app.init_resource::<PermanentPlayerMods>();
        // Charge le disque une fois au boot (écrase les Default).
        app.add_systems(Startup, sys_load_meta_shop);
        // Hub Lobby : achats + lancement.
        app.add_systems(
            Update,
            sys_meta_shop_input
                .in_set(GameSet::UI)
                .run_if(in_state(RunState::Lobby)),
        );
        app.add_systems(EguiPrimaryContextPass, draw_meta_shop_lobby);
        // Hub propre : purge ennemis + revive joueur en entrant au Lobby.
        app.add_systems(OnEnter(RunState::Lobby), sys_lobby_cleanup);
        // Flush save aux moments-clés (réconciliation Âmes → disque).
        app.add_systems(OnExit(GameMode::Roguelite), sys_flush_meta_save);
        app.add_systems(OnEnter(RunState::Victory), sys_flush_meta_save);
        app.add_systems(OnEnter(RunState::Defeat), sys_flush_meta_save);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cumulative_bonuses_scale_with_rank() {
        let cat = MetaShopCatalogue::default();
        let mut save = MetaShopSave::default();
        save.ranks.insert("max_hp".into(), 3);
        save.ranks.insert("damage".into(), 2);
        save.ranks.insert("armor".into(), 1);
        save.ranks.insert("gold".into(), 2);
        assert_eq!(save.max_hp_bonus(&cat), 45.0); // 15 × 3
        assert!((save.damage_mul(&cat) - 1.16).abs() < 1e-5); // 1 + 0.08×2
        assert!((save.damage_reduction(&cat) - 0.05).abs() < 1e-5); // 0.05×1
        assert_eq!(save.start_gold(&cat), 100); // 50 × 2
    }

    #[test]
    fn no_ranks_means_neutral() {
        let cat = MetaShopCatalogue::default();
        let save = MetaShopSave::default();
        assert_eq!(save.max_hp_bonus(&cat), 0.0);
        assert_eq!(save.damage_mul(&cat), 1.0);
        assert_eq!(save.damage_reduction(&cat), 0.0);
        assert_eq!(save.start_gold(&cat), 0);
    }

    #[test]
    fn damage_reduction_clamped() {
        let cat = MetaShopCatalogue::default();
        let mut save = MetaShopSave::default();
        save.ranks.insert("armor".into(), 100); // absurde
        assert!(save.damage_reduction(&cat) <= 0.85);
    }

    #[test]
    fn cost_and_max_rank() {
        let cat = MetaShopCatalogue::default();
        let vit = &cat.upgrades[0];
        assert_eq!(vit.max_rank(), 5);
        assert_eq!(vit.cost_for_next(0), Some(20));
        assert_eq!(vit.cost_for_next(4), Some(160));
        assert_eq!(vit.cost_for_next(5), None); // maxed
    }

    #[test]
    fn parse_garbage_falls_back_to_default() {
        let c = MetaShopCatalogue::parse_toml("pas du toml [[[");
        assert_eq!(c.upgrades.len(), MetaShopCatalogue::default().upgrades.len());
    }

    #[test]
    fn save_roundtrip_toml() {
        let mut save = MetaShopSave::default();
        save.souls_total = 123;
        save.ranks.insert("max_hp".into(), 2);
        let s = toml::to_string_pretty(&save).unwrap();
        let back: MetaShopSave = toml::from_str(&s).unwrap();
        assert_eq!(back.souls_total, 123);
        assert_eq!(back.rank("max_hp"), 2);
    }
}
