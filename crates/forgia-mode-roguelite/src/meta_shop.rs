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

/// Déblocage permanent d'une arme (story-613). Pépin = gratuit (jamais listé).
#[derive(Clone, Debug)]
pub struct WeaponUnlock {
    /// Clé genome viewmodel (pepin/bourrasque/madame_lenoir/boucherie).
    pub key: String,
    pub name: String,
    pub cost: u32,
}

#[derive(Resource, Clone, Debug)]
pub struct MetaShopCatalogue {
    pub upgrades: Vec<MetaUpgrade>,
    /// Armes déblocables en Âmes (story-613).
    pub weapon_unlocks: Vec<WeaponUnlock>,
    /// Paliers d'atouts (boons) déblocables en Âmes (story-616) — réutilise key/name/cost.
    pub boon_tier_unlocks: Vec<WeaponUnlock>,
}

impl MetaShopCatalogue {
    /// Coût/nom de déblocage d'une arme par clé genome (None = Pépin / inconnue).
    pub fn weapon_unlock(&self, key: &str) -> Option<&WeaponUnlock> {
        self.weapon_unlocks.iter().find(|w| w.key == key)
    }
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
            // Miroir EXACT des [[weapon_unlocks]] du genome (story-613).
            weapon_unlocks: vec![
                WeaponUnlock { key: "bourrasque".into(), name: "Bourrasque".into(), cost: 60 },
                WeaponUnlock { key: "madame_lenoir".into(), name: "Madame Lenoir".into(), cost: 150 },
                WeaponUnlock { key: "boucherie".into(), name: "Boucherie".into(), cost: 250 },
            ],
            // Miroir EXACT des [[boon_tier_unlocks]] du genome (story-616).
            boon_tier_unlocks: vec![
                WeaponUnlock { key: "uncommon".into(), name: "Atouts Peu communs".into(), cost: 80 },
                WeaponUnlock { key: "rare".into(), name: "Atouts Rares".into(), cost: 200 },
                WeaponUnlock { key: "legendary".into(), name: "Atouts Légendaires".into(), cost: 400 },
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
struct WeaponUnlockToml {
    key: String,
    name: String,
    cost: u32,
}

#[derive(Deserialize)]
struct CatalogueToml {
    #[serde(default)]
    upgrades: Vec<UpgradeToml>,
    #[serde(default)]
    weapon_unlocks: Vec<WeaponUnlockToml>,
    #[serde(default)]
    boon_tier_unlocks: Vec<WeaponUnlockToml>,
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
        let weapon_unlocks: Vec<WeaponUnlock> = parsed
            .weapon_unlocks
            .into_iter()
            .map(|w| WeaponUnlock { key: w.key, name: w.name, cost: w.cost })
            .collect();
        let boon_tier_unlocks: Vec<WeaponUnlock> = parsed
            .boon_tier_unlocks
            .into_iter()
            .map(|w| WeaponUnlock { key: w.key, name: w.name, cost: w.cost })
            .collect();
        // Fallback PAR CHAMP : un genome partiel ne perd pas les autres listes.
        let d = Self::default();
        Self {
            upgrades: if upgrades.is_empty() { d.upgrades } else { upgrades },
            weapon_unlocks: if weapon_unlocks.is_empty() {
                d.weapon_unlocks
            } else {
                weapon_unlocks
            },
            boon_tier_unlocks: if boon_tier_unlocks.is_empty() {
                d.boon_tier_unlocks
            } else {
                boon_tier_unlocks
            },
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
    /// Armes débloquées en permanence (clés genome). Story-613 — défaut = Pépin seul.
    #[serde(default = "default_unlocked_weapons")]
    pub unlocked_weapons: Vec<String>,
    /// Paliers d'atouts débloqués (uncommon/rare/legendary). Story-616 — défaut vide (Common only).
    #[serde(default)]
    pub unlocked_boon_tiers: Vec<String>,
    /// Niveau de maîtrise par arme (clé genome → niveau). P3 — défaut 1, +1 par run
    /// terminée avec l'arme. Vide = toutes niveau 1.
    #[serde(default)]
    pub weapon_levels: HashMap<String, u32>,
}

/// Pépin = arme de départ : toujours débloquée (story-613).
fn default_unlocked_weapons() -> Vec<String> {
    vec!["pepin".to_string()]
}

impl Default for MetaShopSave {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            souls_total: 0,
            ranks: HashMap::new(),
            unlocked_weapons: default_unlocked_weapons(),
            unlocked_boon_tiers: Vec::new(),
            weapon_levels: HashMap::new(),
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

    /// Niveau de maîtrise de l'arme (P3) — défaut 1.
    pub fn weapon_level(&self, key: &str) -> u32 {
        self.weapon_levels.get(key).copied().unwrap_or(1)
    }

    /// +1 niveau de maîtrise (run terminée avec l'arme). Idempotent par appel.
    pub fn level_up_weapon(&mut self, key: &str) {
        let lvl = self.weapon_levels.entry(key.to_string()).or_insert(1);
        *lvl = lvl.saturating_add(1);
    }

    /// Pépin toujours débloquée ; les autres selon le save (story-613).
    pub fn is_weapon_unlocked(&self, key: &str) -> bool {
        key == "pepin" || self.unlocked_weapons.iter().any(|k| k == key)
    }

    /// Débloque une arme (idempotent). Story-613.
    pub fn unlock_weapon(&mut self, key: &str) {
        if !self.is_weapon_unlocked(key) {
            self.unlocked_weapons.push(key.to_string());
        }
    }

    /// Palier d'atouts débloqué ? Story-616 (Common toujours offert ailleurs).
    pub fn is_boon_tier_unlocked(&self, key: &str) -> bool {
        self.unlocked_boon_tiers.iter().any(|k| k == key)
    }

    /// Débloque un palier d'atouts (idempotent). Story-616.
    pub fn unlock_boon_tier(&mut self, key: &str) {
        if !self.is_boon_tier_unlocked(key) {
            self.unlocked_boon_tiers.push(key.to_string());
        }
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
        crate::persist::save_toml_atomic(&Self::save_path(), self, "meta-shop");
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

/// Story-616 — propage les paliers d'atouts débloqués (`MetaShopSave`) vers la
/// Resource `forgia_rpg_data::boons::UnlockedBoonTiers`, que le roll de boons lit
/// pour filtrer les candidats. Écrit seulement si différent (pas de churn change-detection).
pub fn sys_sync_unlocked_boon_tiers(
    save: Res<MetaShopSave>,
    mut tiers: ResMut<forgia_rpg_data::boons::UnlockedBoonTiers>,
) {
    let want = forgia_rpg_data::boons::UnlockedBoonTiers {
        uncommon: save.is_boon_tier_unlocked("uncommon"),
        rare: save.is_boon_tier_unlocked("rare"),
        legendary: save.is_boon_tier_unlocked("legendary"),
    };
    if *tiers != want {
        *tiers = want;
    }
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
    // Déblocage paliers d'atouts (Digit5/6/7) — story-616.
    let tier_idx = [KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7]
        .iter()
        .position(|k| keys.just_pressed(*k));
    if let Some(ti) = tier_idx {
        if let Some(bt) = cat.boon_tier_unlocks.get(ti) {
            if save.is_boon_tier_unlocked(&bt.key) {
                info!("[meta-shop] palier d'atouts {} déjà débloqué", bt.name);
            } else if meta.current < bt.cost {
                info!(
                    "[meta-shop] pas assez d'âmes pour {} ({}/{})",
                    bt.name, meta.current, bt.cost
                );
            } else {
                meta.current -= bt.cost;
                save.unlock_boon_tier(&bt.key);
                save.souls_total = meta.current;
                save.save();
                info!(
                    "[meta-shop] palier d'atouts débloqué : {} (-{} âmes, reste {})",
                    bt.name, bt.cost, meta.current
                );
            }
        }
        return;
    }
    // Achat upgrades 1..=4.
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

    // Home-hub P2.1 — onglet ENCLUME : panneau CENTRÉ (le hub gère les onglets,
    // un seul panneau visible à la fois). Titre « L'ENCLUME DES ÂMES » = onglet hub.
    egui::Area::new(egui::Id::new("forgia_meta_shop"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 10.0))
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
                        // Paliers d'atouts (boons) — story-616 (touches 5/6/7).
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("— ATOUTS (paliers de boons) —")
                                .size(16.0)
                                .strong()
                                .color(FORGE_TEAL),
                        );
                        ui.add_space(4.0);
                        for (i, bt) in cat.boon_tier_unlocks.iter().enumerate() {
                            let owned = save.is_boon_tier_unlocked(&bt.key);
                            let (text, col) = if owned {
                                (format!("[—]  {} — DÉBLOQUÉ", bt.name), C_HP_HIGH)
                            } else {
                                let afford = meta.current >= bt.cost;
                                (
                                    format!("[{}]  {}  ·  {} âmes", i + 5, bt.name, bt.cost),
                                    if afford { FORGE_OR } else { C_TEXT_MUTED },
                                )
                            };
                            ui.label(egui::RichText::new(text).size(19.0).color(col));
                            ui.add_space(4.0);
                        }
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new("1-4 stats · 5-7 atouts · ENTRÉE = lancer")
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
        // Hub à onglets (P2) : L'Enclume ne s'affiche que sur l'onglet ENCLUME.
        app.add_systems(
            EguiPrimaryContextPass,
            draw_meta_shop_lobby.run_if(crate::hub::on_enclume_tab),
        );
        // Story-616 — propage les paliers d'atouts débloqués vers forgia-rpg-data
        // (le roll de boons filtre alors les candidats par palier débloqué).
        app.add_systems(
            Update,
            sys_sync_unlocked_boon_tiers.run_if(in_state(GameMode::Roguelite)),
        );
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

    // ── Story-613 — déblocage permanent des armes ──

    #[test]
    fn default_unlocks_only_pepin() {
        let save = MetaShopSave::default();
        assert!(save.is_weapon_unlocked("pepin"));
        assert!(!save.is_weapon_unlocked("bourrasque"));
        assert!(!save.is_weapon_unlocked("madame_lenoir"));
        assert!(!save.is_weapon_unlocked("boucherie"));
    }

    #[test]
    fn unlock_weapon_is_idempotent() {
        let mut save = MetaShopSave::default();
        save.unlock_weapon("bourrasque");
        save.unlock_weapon("bourrasque");
        assert!(save.is_weapon_unlocked("bourrasque"));
        assert_eq!(
            save.unlocked_weapons.iter().filter(|k| *k == "bourrasque").count(),
            1
        );
    }

    #[test]
    fn catalogue_has_three_weapon_unlocks_with_costs() {
        let cat = MetaShopCatalogue::default();
        assert_eq!(cat.weapon_unlocks.len(), 3);
        assert_eq!(cat.weapon_unlock("bourrasque").map(|w| w.cost), Some(60));
        assert!(cat.weapon_unlock("pepin").is_none()); // Pépin jamais listée
    }

    #[test]
    fn old_save_without_field_defaults_to_pepin() {
        // Un save d'avant story-613 (sans `unlocked_weapons`) doit retomber sur Pépin.
        let old = r#"version = 1
souls_total = 500
[ranks]
max_hp = 3
"#;
        let save: MetaShopSave = toml::from_str(old).unwrap();
        assert!(save.is_weapon_unlocked("pepin"));
        assert!(!save.is_weapon_unlocked("boucherie"));
    }

    #[test]
    fn save_roundtrip_preserves_unlocks() {
        let mut save = MetaShopSave::default();
        save.unlock_weapon("madame_lenoir");
        let s = toml::to_string_pretty(&save).unwrap();
        let back: MetaShopSave = toml::from_str(&s).unwrap();
        assert!(back.is_weapon_unlocked("madame_lenoir"));
        assert!(back.is_weapon_unlocked("pepin"));
        assert!(!back.is_weapon_unlocked("boucherie"));
    }

    // ── Story-616 — paliers d'atouts (boons) ──

    #[test]
    fn boon_tiers_default_locked_and_unlock() {
        let mut save = MetaShopSave::default();
        assert!(!save.is_boon_tier_unlocked("uncommon"));
        save.unlock_boon_tier("uncommon");
        save.unlock_boon_tier("uncommon"); // idempotent
        assert!(save.is_boon_tier_unlocked("uncommon"));
        assert!(!save.is_boon_tier_unlocked("legendary"));
        assert_eq!(save.unlocked_boon_tiers.len(), 1);
    }

    #[test]
    fn catalogue_has_three_boon_tier_unlocks() {
        let cat = MetaShopCatalogue::default();
        assert_eq!(cat.boon_tier_unlocks.len(), 3);
        assert_eq!(cat.boon_tier_unlocks[0].key, "uncommon");
        assert_eq!(cat.boon_tier_unlocks[2].cost, 400);
    }

    #[test]
    fn old_save_defaults_boon_tiers_empty() {
        // Un save d'avant 616 (sans `unlocked_boon_tiers`) → Common only.
        let old = r#"version = 1
souls_total = 500
unlocked_weapons = ["pepin"]
[ranks]
"#;
        let save: MetaShopSave = toml::from_str(old).unwrap();
        assert!(!save.is_boon_tier_unlocked("uncommon"));
    }
}
