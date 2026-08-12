//! equipment.rs — Pièces d'armure lootées, équipées au Lobby, actives en combat.
//!
//! Le personnage porte une combinaison de base et cinq pièces d'armure
//! (`assets/models/characters/trooper/`). Chaque pièce améliore UNE statistique
//! de combat ; sa **rareté**, donc sa **couleur**, dit de combien. La couleur
//! n'est pas décorative : c'est elle qu'on lit pour juger une pièce, et elle est
//! appliquée en teinte sur le matériau — visible sur le portrait de l'onglet
//! FORGE comme sur les gants en jeu.
//!
//! Tout le chiffrage vit dans `assets/genomes/roguelite/roguelite_equipment.toml`
//! (couche definition) : raretés, gains par slot, poids de tirage, cadrage du
//! portrait. Aucune valeur de gameplay n'est écrite ici.
//!
//! Les bonus n'atteignent PAS le combat par eux-mêmes : ils alimentent
//! [`EquipmentMods`], que `boons_apply::sys_recompute_boon_mods` compose dans
//! `PlayerCombatMods` au même titre que les boons, la méta et la Trempe. Un seul
//! endroit calcule les modificateurs du joueur.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_ui_lib::style::HAIR_GOLD_STRONG;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::run::{RunSeed, RunState};

const SAVE_FILE: &str = "equipment_save.toml";
const SAVE_VERSION: u32 = 1;
const CONFIG_PATH: &str = "assets/genomes/roguelite/roguelite_equipment.toml";
const SENSOR_PATH: &str = "forgia2_equipment.json";

// ── Couche definition ────────────────────────────────────────────────────────

/// Une rareté : sa couleur, ce qu'elle multiplie, sa fréquence de tirage.
#[derive(Deserialize, Clone, Debug)]
pub struct Rarity {
    pub id: String,
    pub label: String,
    pub rgb: [f32; 3],
    pub bonus_mul: f32,
    pub drop_weight: f32,
}

/// Un emplacement d'armure et la statistique qu'il améliore.
#[derive(Deserialize, Clone, Debug)]
pub struct SlotDef {
    pub id: String,
    pub label: String,
    /// `damage` | `fire_rate` | `reduction` | `crit` | `headshot`.
    pub stat: String,
    pub stat_label: String,
    /// Gain au rang le plus bas ; les raretés supérieures le multiplient.
    pub per_tier: f32,
    pub model: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DropRules {
    pub per_stage: u32,
    pub on_victory: u32,
    pub reroll_if_owned: u32,
    #[serde(default = "default_card_seconds")]
    pub card_seconds: f32,
}

fn default_card_seconds() -> f32 {
    4.0
}

impl Default for DropRules {
    fn default() -> Self {
        Self {
            per_stage: 1,
            on_victory: 1,
            reroll_if_owned: 8,
            card_seconds: default_card_seconds(),
        }
    }
}

/// Animation de l'avatar. Aligné sur `roguelite_enemy_anim.toml` (story-636) :
/// clips par NOM, fondu, seuils de vitesse et plafond anti-téléport. Le projet
/// n'a pas besoin de deux modèles d'animation de personnage.
#[derive(Deserialize, Clone, Debug)]
pub struct AvatarAnimCfg {
    pub idle_clip: String,
    pub walk_clip: String,
    pub run_clip: String,
    pub crossfade_ms: u64,
    pub walk_speed_min: f32,
    pub run_speed_min: f32,
    pub max_sane_speed: f32,
}

impl Default for AvatarAnimCfg {
    fn default() -> Self {
        Self {
            idle_clip: "idle".into(),
            walk_clip: "walk".into(),
            run_clip: "walk".into(),
            crossfade_ms: 150,
            walk_speed_min: 0.6,
            run_speed_min: 6.0,
            max_sane_speed: 30.0,
        }
    }
}

#[derive(Resource, Deserialize, Clone, Debug, Default)]
pub struct EquipmentConfig {
    #[serde(default)]
    pub animation: AvatarAnimCfg,
    /// Corps de base, toujours porté (l'aperçu 3D le montre seul quand aucune
    /// pièce n'est équipée).
    #[serde(default)]
    pub body_model: String,
    /// Emplacement dont la rareté teinte les bras du viewmodel — le seul visible
    /// en combat.
    #[serde(default)]
    pub viewmodel_slot: String,
    /// Bras vus en FPS. Déclarés avec le personnage plutôt que dans le
    /// préchargeur générique : le choix des bras appartient au personnage.
    /// Vides = on garde les bras d'origine.
    #[serde(default)]
    pub viewmodel_arm_left: String,
    #[serde(default)]
    pub viewmodel_arm_right: String,
    #[serde(default)]
    pub rarities: Vec<Rarity>,
    #[serde(default)]
    pub slots: Vec<SlotDef>,
    #[serde(default)]
    pub drops: DropRules,
}

impl EquipmentConfig {
    fn load() -> Self {
        // def_io et non std::fs : sur wasm le fs n'existe pas — sans pack
        // embarqué, 0 slot → aucun body_model → l'avatar du menu n'existe
        // jamais (bug testeur web 2026-08-11).
        match forgia_core::def_io::read_def_str(CONFIG_PATH) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_else(|e| {
                warn!("[equipment] {CONFIG_PATH} illisible: {e} — équipement désactivé");
                Self::default()
            }),
            Err(e) => {
                warn!("[equipment] {CONFIG_PATH} absent: {e} — équipement désactivé");
                Self::default()
            }
        }
    }

    pub fn rarity(&self, id: &str) -> Option<&Rarity> {
        self.rarities.iter().find(|r| r.id == id)
    }

    pub fn slot(&self, id: &str) -> Option<&SlotDef> {
        self.slots.iter().find(|s| s.id == id)
    }

    /// Rang d'une rareté (0 = la plus commune) — sert à trier l'affichage.
    pub fn rarity_rank(&self, id: &str) -> usize {
        self.rarities.iter().position(|r| r.id == id).unwrap_or(0)
    }

    /// Ce que vaut une pièce en points de Puissance : son rang + 1, donc 1 pour
    /// la plus commune et `rarities.len()` pour la plus rare. Un emplacement vide
    /// vaut 0 — porter du Commun est déjà un progrès sur ne rien porter.
    pub fn power_of(&self, rarity_id: &str) -> u32 {
        self.rarity(rarity_id)
            .map(|_| self.rarity_rank(rarity_id) as u32 + 1)
            .unwrap_or(0)
    }

    /// La Puissance d'un équipement parfait : tous les emplacements dans la
    /// rareté la plus haute. C'est le dénominateur du score, et il se DÉRIVE du
    /// genome — ajouter un emplacement ou une rareté déplace la cible toute
    /// seule, sans qu'aucun nombre ne soit à corriger ici.
    pub fn power_max(&self) -> u32 {
        (self.slots.len() * self.rarities.len()) as u32
    }

    pub fn color32(&self, rarity_id: &str) -> egui::Color32 {
        let rgb = self.rarity(rarity_id).map(|r| r.rgb).unwrap_or([0.5; 3]);
        egui::Color32::from_rgb(
            (rgb[0] * 255.0) as u8,
            (rgb[1] * 255.0) as u8,
            (rgb[2] * 255.0) as u8,
        )
    }
}

// ── Sauvegarde ───────────────────────────────────────────────────────────────

/// Ce que le joueur possède et ce qu'il porte.
///
/// `owned` associe un slot aux raretés déjà trouvées pour lui : une collection,
/// pas un inventaire — deux Casques Communs restent un seul Casque Commun.
#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct EquipmentSave {
    pub version: u32,
    /// Nombre de pièces tombées depuis toujours (santé du système de butin).
    ///
    /// 🚨 Les scalaires DOIVENT précéder les tables : en TOML, une valeur écrite
    /// après `[owned]` appartiendrait à `[owned]`. Un champ ajouté plus bas
    /// casserait silencieusement le rechargement du save.
    #[serde(default)]
    pub drops_total: u32,
    /// Meilleure Puissance jamais portée. C'est ce qui transforme le score en
    /// trace de progression : sans lui, retirer une pièce efface l'histoire.
    #[serde(default)]
    pub power_record: u32,
    /// Story-678 Phase 4 — total de pièces possédées la dernière fois que la
    /// page Forgeron a été OUVERTE (pastille « nouvelle pièce »). Scalaire :
    /// DOIT rester avant les tables (cf. 🚨 ci-dessus).
    #[serde(default)]
    pub seen_owned_total: u32,
    #[serde(default)]
    pub owned: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub equipped: HashMap<String, String>,
}

impl Default for EquipmentSave {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            drops_total: 0,
            power_record: 0,
            seen_owned_total: 0,
            owned: HashMap::default(),
            equipped: HashMap::default(),
        }
    }
}

impl EquipmentSave {
    /// Total de pièces possédées, tous slots confondus (pastille Forgeron).
    pub fn owned_total(&self) -> u32 {
        self.owned.values().map(|v| v.len() as u32).sum()
    }

    fn save_path() -> PathBuf {
        crate::persist::save_dir().join(SAVE_FILE)
    }

    fn load_or_default() -> Self {
        crate::persist::load_toml_migrating(SAVE_FILE)
    }

    /// Écrit la sauvegarde d'équipement sur le disque.
    ///
    /// 🚨 PUBLIC depuis le 2026-08-07 : `sys_track_power_record` était le SEUL
    /// appelant, et il n'écrit que sur un NOUVEAU RECORD. Équiper au menu ne
    /// touchait donc jamais le disque — le choix était perdu au relancement,
    /// sans le moindre signe. Tout site qui mute `equipped` doit persister.
    pub fn save(&self) {
        crate::persist::save_toml_atomic(&Self::save_path(), self, "equipment");
    }

    pub fn owns(&self, slot: &str, rarity: &str) -> bool {
        self.owned
            .get(slot)
            .is_some_and(|v| v.iter().any(|r| r == rarity))
    }

    /// Ajoute une pièce. Rend `false` si elle était déjà possédée.
    fn grant(&mut self, slot: &str, rarity: &str) -> bool {
        if self.owns(slot, rarity) {
            return false;
        }
        self.owned
            .entry(slot.to_string())
            .or_default()
            .push(rarity.to_string());
        // Une pièce trouvée alors que le slot est vide s'équipe seule : sans ça
        // le premier butin d'une partie n'a aucun effet tant qu'on n'ouvre pas
        // le menu, et le joueur ne fait pas le lien entre le drop et le gain.
        self.equipped
            .entry(slot.to_string())
            .or_insert_with(|| rarity.to_string());
        true
    }
}

// ── Modificateurs produits ───────────────────────────────────────────────────

/// Bonus cumulés des pièces équipées. Composé dans `PlayerCombatMods` par
/// `boons_apply::sys_recompute_boon_mods` — cette Resource ne s'applique pas
/// toute seule.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct EquipmentMods {
    pub damage_mul: f32,
    pub fire_rate_mul: f32,
    pub damage_reduction: f32,
    pub crit_chance: f32,
    pub headshot_bonus_mul: f32,
}

impl Default for EquipmentMods {
    fn default() -> Self {
        Self {
            damage_mul: 1.0,
            fire_rate_mul: 1.0,
            damage_reduction: 0.0,
            crit_chance: 0.0,
            headshot_bonus_mul: 0.0,
        }
    }
}

/// Recalcule les bonus depuis les pièces portées. Idempotent.
pub fn compute_mods(cfg: &EquipmentConfig, save: &EquipmentSave) -> EquipmentMods {
    let mut mods = EquipmentMods::default();
    for (slot_id, rarity_id) in &save.equipped {
        let (Some(slot), Some(rarity)) = (cfg.slot(slot_id), cfg.rarity(rarity_id)) else {
            continue;
        };
        let gain = slot.per_tier * rarity.bonus_mul;
        match slot.stat.as_str() {
            "damage" => mods.damage_mul += gain,
            "fire_rate" => mods.fire_rate_mul += gain,
            "reduction" => mods.damage_reduction += gain,
            "crit" => mods.crit_chance += gain,
            "headshot" => mods.headshot_bonus_mul += gain,
            other => debug!("[equipment] stat inconnue dans le TOML: {other}"),
        }
    }
    mods
}

/// La **Puissance** : un seul nombre qui dit où en est l'équipement.
///
/// Somme des rangs des pièces portées, sur `power_max()`. Elle mesure le RANG et
/// non le gain de statistique, et c'est délibéré :
///
/// - c'est le rang que le joueur progresse — trouver plus rare est le geste du
///   jeu, alors qu'un dosage de `per_tier` est une décision de game design ;
/// - un score dérivé des pourcentages bougerait au moindre rééquilibrage, sans
///   qu'aucune pièce n'ait changé : le record d'hier deviendrait faux ;
/// - additionner cinq pourcentages qui portent sur des choses différentes
///   (dégâts, cadence, réduction…) ne veut rien dire physiquement.
///
/// Elle ne crée AUCUNE seconde vérité : elle ne pilote rien en combat, elle lit
/// le même `equipped` que `compute_mods`.
pub fn power_score(cfg: &EquipmentConfig, save: &EquipmentSave) -> u32 {
    save.equipped
        .iter()
        .filter(|(slot_id, _)| cfg.slot(slot_id).is_some())
        .map(|(_, rarity_id)| cfg.power_of(rarity_id))
        .sum()
}

/// Retient le pic de Puissance. Sans garde `is_changed` sur la lecture ce serait
/// une écriture de fichier par frame ; avec, un save chargé d'une version
/// antérieure se voit quand même mesuré à la première frame (la Resource compte
/// comme changée à son insertion).
fn sys_track_power_record(cfg: Res<EquipmentConfig>, mut save: ResMut<EquipmentSave>) {
    if !save.is_changed() {
        return;
    }
    let score = power_score(&cfg, &save);
    if score > save.power_record {
        info!(
            "[equipment] puissance record — {} → {score} / {}",
            save.power_record,
            cfg.power_max()
        );
        save.power_record = score;
        save.save();
    }
}

fn sys_recompute_equipment_mods(
    cfg: Res<EquipmentConfig>,
    save: Res<EquipmentSave>,
    mut mods: ResMut<EquipmentMods>,
) {
    // Pas de garde `is_changed` : les Resources sont insérées au build du plugin,
    // donc leurs ticks de changement peuvent être passés avant même qu'on entre
    // en Roguelite — un joueur qui reprend une partie avec des pièces déjà
    // portées se retrouverait alors sans aucun bonus. C'est exactement le bug
    // « boons inertes » corrigé dans `boons_apply` le 2026-06-28. Le calcul est
    // négligeable (≤ 5 pièces) ; seul le log est conditionné au changement.
    let next = compute_mods(&cfg, &save);
    if *mods != next {
        info!(
            "[equipment] bonus — dégâts ×{:.2} cadence ×{:.2} blindage {:.0}% critique {:.0}% visée +{:.2} ({} pièces)",
            next.damage_mul,
            next.fire_rate_mul,
            next.damage_reduction * 100.0,
            next.crit_chance * 100.0,
            next.headshot_bonus_mul,
            save.equipped.len()
        );
        *mods = next;
    }
}

/// Fait pointer les bras du viewmodel sur ceux du personnage.
///
/// `GameAssets` précharge des bras génériques au démarrage ; on les remplace
/// une fois par ceux déclarés dans le genome. Écrire ici plutôt que dans le
/// préchargeur garde le choix des bras avec le personnage — et le rend
/// data-driven au passage : vider les deux champs rend les bras d'origine, sans
/// toucher au code.
///
/// Une seule fois : `forgia_viewmodel::arms::spawn_arms` lit ces handles au
/// spawn des bras, qui survient en jeu, bien après le démarrage.
fn sys_point_viewmodel_arms_at_character(
    cfg: Res<EquipmentConfig>,
    assets: Res<AssetServer>,
    mut game_assets: ResMut<forgia_assets::GameAssets>,
    mut done: Local<bool>,
) {
    if *done || cfg.viewmodel_arm_left.is_empty() || cfg.viewmodel_arm_right.is_empty() {
        return;
    }
    *done = true;
    let scene = |p: &str| assets.load(GltfAssetLabel::Scene(0).from_asset(p.to_string()));
    game_assets.viewmodel_arm_left = scene(&cfg.viewmodel_arm_left);
    game_assets.viewmodel_arm_right = scene(&cfg.viewmodel_arm_right);
    info!("[equipment] bras du viewmodel = ceux du personnage");
}

/// Publie la teinte des PLAQUES d'armure des bras dans `ArmCosmetics`.
///
/// Deux couches distinctes : la rareté teinte l'armure, la couleur d'identité
/// (écrite par `identity.rs`) teinte la combinaison dessous. On garde ainsi la
/// cosmétique choisie au Forgeron sans rendre la rareté illisible.
///
/// Passer par `ArmCosmetics` plutôt que d'appeler le viewmodel évite une
/// dépendance de `forgia-viewmodel` vers ce crate : la Resource est déjà le
/// point de rendez-vous des deux couches.
fn sys_publish_arm_armor_tint(
    cfg: Res<EquipmentConfig>,
    save: Res<EquipmentSave>,
    mut cosmetics: ResMut<ArmCosmetics>,
) {
    if !save.is_changed() && !cfg.is_changed() {
        return;
    }
    let tint = save
        .equipped
        .get(&cfg.viewmodel_slot)
        .and_then(|r| cfg.rarity(r))
        // Teinte NORMALISÉE : `base_color` multiplie l'albédo, donc une rareté
        // sombre assombrirait le gantelet au lieu de le colorer.
        .map(|r| crate::avatar::rarity_tint(r.rgb).to_srgba())
        .map(|c| [c.red, c.green, c.blue])
        // Aucune pièce portée : blanc, donc aucune teinte.
        .unwrap_or([1.0, 1.0, 1.0]);
    if cosmetics.armor_rgb != tint {
        cosmetics.armor_rgb = tint;
    }
}

// ── Butin ────────────────────────────────────────────────────────────────────

/// Tire une rareté au poids déclaré.
fn roll_rarity<'a>(cfg: &'a EquipmentConfig, rng: &mut Xoshiro256StarStar) -> Option<&'a Rarity> {
    let total: f32 = cfg.rarities.iter().map(|r| r.drop_weight).sum();
    if total <= 0.0 {
        return None;
    }
    let mut pick = (rng.next_u32() as f32 / u32::MAX as f32) * total;
    for r in &cfg.rarities {
        pick -= r.drop_weight;
        if pick <= 0.0 {
            return Some(r);
        }
    }
    cfg.rarities.last()
}

/// Tire une pièce, en re-tirant un nombre borné de fois si elle est déjà
/// possédée. Rend `None` seulement si la configuration est vide.
fn roll_piece(
    cfg: &EquipmentConfig,
    save: &EquipmentSave,
    rng: &mut Xoshiro256StarStar,
) -> Option<(String, String)> {
    if cfg.slots.is_empty() {
        return None;
    }
    let mut last = None;
    for _ in 0..=cfg.drops.reroll_if_owned {
        let slot = &cfg.slots[(rng.next_u32() as usize) % cfg.slots.len()];
        let rarity = roll_rarity(cfg, rng)?;
        let pair = (slot.id.clone(), rarity.id.clone());
        if !save.owns(&pair.0, &pair.1) {
            return Some(pair);
        }
        last = Some(pair);
    }
    // Tous les tirages étaient des doublons : on rend le dernier, l'appelant
    // saura que rien n'a été ajouté. Ne jamais rendre « rien » silencieusement.
    last
}

/// La dernière pièce tombée, à annoncer à l'écran.
///
/// Le butin est le geste central du roguelite. Sans cette carte, une pièce tombe
/// pendant la run et le joueur ne l'apprend qu'en ouvrant un menu — le gain
/// existe dans les données mais pas dans l'expérience.
#[derive(Resource, Default)]
pub struct LootCard {
    slot: String,
    rarity: String,
    /// `None` = rien à montrer.
    since: Option<f32>,
    /// Doublon : la pièce était déjà possédée, on le dit plutôt que de faire
    /// croire à un gain.
    duplicate: bool,
    /// Puissance avant / après le butin, et ce qu'elle vaudrait si la pièce était
    /// portée. Une pièce qui tombe sur un emplacement déjà rempli ne s'équipe pas
    /// toute seule : sans le troisième nombre, la carte annoncerait « +0 » sur un
    /// butin qui est pourtant une amélioration.
    power_before: u32,
    power_after: u32,
    power_potential: u32,
}

/// Étage le plus profond déjà récompensé — évite de re-donner le butin d'un
/// étage à chaque changement d'état interne.
#[derive(Resource, Default)]
struct StageWatermark(Option<u8>);

/// Accorde le butin quand un étage est atteint, et à la victoire.
///
/// Le tirage est dérivé de `RunSeed` : deux runs de même graine donnent le même
/// butin, comme le reste du roguelite.
fn sys_grant_stage_drops(
    state: Res<State<RunState>>,
    seed: Option<Res<RunSeed>>,
    cfg: Res<EquipmentConfig>,
    mut save: ResMut<EquipmentSave>,
    mut mark: ResMut<StageWatermark>,
    mut card: ResMut<LootCard>,
    time: Res<Time>,
) {
    if !state.is_changed() {
        return;
    }
    let (stage, count) = match state.get() {
        RunState::InRun { stage } | RunState::Boss { stage } => (*stage, cfg.drops.per_stage),
        RunState::Victory => (u8::MAX, cfg.drops.on_victory),
        // Le Lobby remet le compteur : une nouvelle run doit pouvoir re-donner
        // le butin des mêmes étages.
        RunState::Lobby => {
            mark.0 = None;
            return;
        }
        RunState::Defeat => return,
    };
    if mark.0.is_some_and(|seen| stage <= seen) {
        return;
    }
    mark.0 = Some(stage);
    let Some(seed) = seed else {
        return;
    };
    let mut rng = Xoshiro256StarStar::seed_from_u64(seed.stage_seed(stage));
    let mut changed = false;
    for _ in 0..count {
        let Some((slot, rarity)) = roll_piece(&cfg, &save, &mut rng) else {
            continue;
        };
        save.drops_total += 1;
        changed = true;
        let power_before = power_score(&cfg, &save);
        let fresh = save.grant(&slot, &rarity);
        let power_after = power_score(&cfg, &save);
        // Ce que donnerait la pièce si on l'équipait : le score courant, moins ce
        // que vaut la pièce actuellement portée sur cet emplacement, plus celle-ci.
        let worn_now = save
            .equipped
            .get(&slot)
            .map(|id| cfg.power_of(id))
            .unwrap_or(0);
        let power_potential = power_after - worn_now + cfg.power_of(&rarity);
        if fresh {
            info!("[equipment] butin — {slot} {rarity} (puissance {power_before} → {power_after})");
        } else {
            info!("[equipment] butin — {slot} {rarity} (doublon, rien ajouté)");
        }
        *card = LootCard {
            slot: slot.clone(),
            rarity: rarity.clone(),
            since: Some(time.elapsed_secs()),
            duplicate: !fresh,
            power_before,
            power_after,
            power_potential,
        };
    }
    if changed {
        save.save();
    }
}

// ── Panneau d'équipement (onglet FORGE) ──────────────────────────────────────

/// Vrai dès que le panneau a été affiché au moins une fois (santé du capteur).
#[derive(Resource, Default)]
pub struct EquipmentPanelShown(pub bool);

/// Contenu du panneau : une ligne par emplacement, les raretés possédées en
/// pastilles cliquables. Sélection, jamais saisie — on clique une couleur.
pub fn draw_equipment_content(
    ui: &mut egui::Ui,
    cfg: &EquipmentConfig,
    save: &mut EquipmentSave,
    mods: &EquipmentMods,
) {
    ui.label(
        egui::RichText::new("ÉQUIPEMENT")
            .size(18.0)
            .strong()
            .color(HAIR_GOLD_STRONG),
    );
    let (worn, total) = (save.equipped.len(), cfg.slots.len());
    ui.label(
        egui::RichText::new(format!("{worn} / {total} emplacements"))
            .size(11.0)
            .weak(),
    );
    ui.add_space(6.0);

    // La Puissance en tête, en gros : c'est le seul nombre qui se compare d'une
    // run à l'autre. Les cinq pourcentages du bilan disent le PROFIL du build ;
    // celui-ci dit s'il vaut mieux que le précédent, ce qu'aucun d'eux ne peut
    // dire seul.
    let power = power_score(cfg, save);
    let power_max = cfg.power_max().max(1);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("PUISSANCE")
                .size(11.0)
                .weak()
                .color(HAIR_GOLD_STRONG),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if save.power_record > power {
                ui.label(
                    egui::RichText::new(format!("record {}", save.power_record))
                        .size(11.0)
                        .weak(),
                );
            }
            ui.label(
                egui::RichText::new(format!("/ {power_max}"))
                    .size(12.0)
                    .weak(),
            );
            ui.label(
                egui::RichText::new(power.to_string())
                    .size(24.0)
                    .strong()
                    .color(HAIR_GOLD_STRONG),
            );
        });
    });
    // Jauge de collection : le vide à droite est la part encore à trouver, et le
    // liseré du record dit d'où l'on vient quand on a retiré une pièce.
    {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(3),
            egui::Color32::from_gray(48),
        );
        if save.power_record > power {
            let mut mark = rect;
            mark.set_width(
                rect.width() * (save.power_record as f32 / power_max as f32).clamp(0.0, 1.0),
            );
            ui.painter().rect_filled(
                mark,
                egui::CornerRadius::same(3),
                HAIR_GOLD_STRONG.gamma_multiply(0.28),
            );
        }
        let mut filled = rect;
        filled.set_width(rect.width() * (power as f32 / power_max as f32).clamp(0.0, 1.0));
        ui.painter()
            .rect_filled(filled, egui::CornerRadius::same(3), HAIR_GOLD_STRONG);
    }
    ui.add_space(8.0);

    // Le clic est appliqué APRÈS la boucle : muter `save` pendant qu'on itère
    // dessus emprunterait deux fois la même donnée.
    let mut pending: Option<(String, Option<String>)> = None;
    for slot in &cfg.slots {
        let equipped = save.equipped.get(&slot.id).cloned();
        let mut owned = save.owned.get(&slot.id).cloned().unwrap_or_default();
        owned.sort_by_key(|r| cfg.rarity_rank(r));
        // Ce que la pièce portée rapporte — la référence de toute comparaison.
        let worn_gain = equipped
            .as_deref()
            .and_then(|id| cfg.rarity(id))
            .map(|r| slot.per_tier * r.bonus_mul)
            .unwrap_or(0.0);
        let worn_power = equipped.as_deref().map(|id| cfg.power_of(id)).unwrap_or(0);

        // Ligne d'emplacement : un liseré de la couleur portée, le nom, et à
        // droite ce que ça rapporte. La couleur du liseré porte l'information
        // « qualité » sans un mot — c'est tout l'intérêt de la convention.
        ui.horizontal(|ui| {
            let accent = equipped
                .as_deref()
                .map(|id| cfg.color32(id))
                .unwrap_or(egui::Color32::from_gray(60));
            let (bar, _) = ui.allocate_exact_size(egui::vec2(4.0, 18.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(bar, egui::CornerRadius::same(2), accent);
            ui.add_space(4.0);
            ui.label(egui::RichText::new(&slot.label).size(13.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if worn_gain > 0.0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} +{:.0}%",
                            slot.stat_label,
                            worn_gain * 100.0
                        ))
                        .size(11.0)
                        .color(accent),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(slot.stat_label.as_str())
                            .size(11.0)
                            .weak(),
                    );
                }
            });
        });

        // On affiche TOUTE l'échelle de raretés, pas seulement ce qu'on possède :
        // les pastilles pleines sont à soi, les contours sont à trouver. Le
        // joueur voit d'un coup ce qui lui manque, donc ce qu'il peut viser aux
        // prochaines runs — sans écran séparé.
        ui.horizontal_wrapped(|ui| {
            ui.add_space(8.0);
            for rarity in &cfg.rarities {
                let rarity_id = &rarity.id;
                let has = owned.iter().any(|r| r == rarity_id);
                let selected = equipped.as_deref() == Some(rarity_id.as_str());
                let size = if selected { 26.0 } else { 20.0 };
                let sense = if has {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                };
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), sense);
                let col = cfg.color32(rarity_id);
                if has {
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(4), col);
                } else {
                    // Contour seul : la place est réservée, la couleur annoncée,
                    // mais l'absence se lit sans avoir à comparer deux listes.
                    ui.painter().rect_stroke(
                        rect.shrink(2.0),
                        egui::CornerRadius::same(4),
                        egui::Stroke::new(1.0, col.gamma_multiply(0.45)),
                        egui::StrokeKind::Inside,
                    );
                }
                if selected {
                    ui.painter().rect_stroke(
                        rect,
                        egui::CornerRadius::same(4),
                        egui::Stroke::new(2.5, egui::Color32::WHITE),
                        egui::StrokeKind::Outside,
                    );
                }
                // Comparaison au survol : on ne montre pas seulement ce que vaut
                // la pièce, mais ce qu'on GAGNE ou PERD à l'échanger. Sans le
                // delta, choisir demande un calcul mental.
                let gain = slot.per_tier * rarity.bonus_mul;
                // Le delta de Puissance rend comparables deux pièces d'emplacements
                // DIFFÉRENTS — ce que « Cadence +8 % » contre « Blindage +6 % » ne
                // permet pas. C'est là que le score unique gagne sa place.
                let power_delta = cfg.power_of(rarity_id) as i32 - worn_power as i32;
                let hover = if !has {
                    format!(
                        "{} — pas encore trouvé\n{} +{:.0}% si tu l'obtiens",
                        rarity.label,
                        slot.stat_label,
                        gain * 100.0
                    )
                } else if selected {
                    format!(
                        "{} — porté\n{} +{:.0}%\n\nClic pour retirer ({:+} Puissance)",
                        rarity.label,
                        slot.stat_label,
                        gain * 100.0,
                        -(cfg.power_of(rarity_id) as i32)
                    )
                } else {
                    format!(
                        "{} — {} +{:.0}%\n{} {:+.0}% en l'équipant\n{power_delta:+} Puissance",
                        rarity.label,
                        slot.stat_label,
                        gain * 100.0,
                        slot.stat_label,
                        (gain - worn_gain) * 100.0
                    )
                };
                if resp.on_hover_text(hover).clicked() {
                    pending = Some((
                        slot.id.clone(),
                        if selected {
                            None
                        } else {
                            Some(rarity_id.clone())
                        },
                    ));
                }
            }
        });
        ui.add_space(6.0);
    }

    if let Some((slot_id, choice)) = pending {
        match choice {
            Some(rarity) => {
                save.equipped.insert(slot_id, rarity);
            }
            None => {
                save.equipped.remove(&slot_id);
            }
        }
        save.save();
    }

    ui.separator();
    ui.add_space(2.0);
    // Bilan : uniquement les statistiques réellement modifiées. Une colonne de
    // « +0 % » n'apprend rien et noie les deux lignes qui comptent.
    // Bilan en colonnes avec barres : une ligne de texte se lit comme un journal
    // et ne dit pas OÙ l'on est fort. Les barres montrent d'un coup le profil du
    // build — donc ce qu'on cherche à la prochaine run. Toutes les statistiques
    // sont listées, même à zéro : un creux visible est une information.
    let stats = [
        ("Dégâts", (mods.damage_mul - 1.0) * 100.0),
        ("Cadence", (mods.fire_rate_mul - 1.0) * 100.0),
        ("Blindage", mods.damage_reduction * 100.0),
        ("Critique", mods.crit_chance * 100.0),
        ("Visée", mods.headshot_bonus_mul * 100.0),
    ];
    // Échelle commune, dérivée du meilleur : les barres se comparent entre elles
    // plutôt que contre un maximum arbitraire.
    let peak = stats.iter().map(|(_, v)| *v).fold(1.0_f32, f32::max);
    for (label, value) in stats {
        ui.horizontal(|ui| {
            ui.add_sized(
                [72.0, 14.0],
                egui::Label::new(egui::RichText::new(label).size(11.0)),
            );
            let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 8.0), egui::Sense::hover());
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(3),
                egui::Color32::from_gray(48),
            );
            if value > 0.05 {
                let mut filled = rect;
                filled.set_width(rect.width() * (value / peak).clamp(0.0, 1.0));
                ui.painter()
                    .rect_filled(filled, egui::CornerRadius::same(3), HAIR_GOLD_STRONG);
            }
            ui.add_space(6.0);
            let txt = egui::RichText::new(format!("+{value:.0}%")).size(11.0);
            ui.label(if value > 0.05 {
                txt.color(HAIR_GOLD_STRONG)
            } else {
                txt.weak()
            });
        });
    }
}

/// Annonce la pièce qui vient de tomber, en plein jeu.
///
/// Haut-centre : hors du réticule et hors du HUD des coins, donc lisible sans
/// masquer le combat. Fond à la couleur de la rareté — c'est elle l'information,
/// le texte ne fait que la confirmer.
fn draw_loot_card(
    mut contexts: EguiContexts,
    cfg: Res<EquipmentConfig>,
    card: Res<LootCard>,
    time: Res<Time>,
) {
    let Some(since) = card.since else {
        return;
    };
    let age = time.elapsed_secs() - since;
    if age > cfg.drops.card_seconds {
        return;
    }
    let (Some(slot), Some(rarity)) = (cfg.slot(&card.slot), cfg.rarity(&card.rarity)) else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    // Fondu sur le dernier quart : une carte qui disparaît d'un coup se lit
    // comme un bug d'affichage.
    let fade = ((cfg.drops.card_seconds - age) / (cfg.drops.card_seconds * 0.25)).clamp(0.0, 1.0);
    let alpha = (fade * 255.0) as u8;
    let col = cfg.color32(&card.rarity);
    let accent = egui::Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha);

    egui::Area::new(egui::Id::new("forgia_loot_card"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 96.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(
                    14,
                    11,
                    18,
                    alpha.min(220),
                ))
                .inner_margin(egui::Margin::symmetric(20, 12))
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(egui::Stroke::new(2.0, accent))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(if card.duplicate {
                                "DÉJÀ POSSÉDÉ"
                            } else {
                                "PIÈCE TROUVÉE"
                            })
                            .size(12.0)
                            .color(egui::Color32::from_rgba_unmultiplied(200, 195, 190, alpha)),
                        );
                        ui.label(
                            egui::RichText::new(format!("{} {}", slot.label, rarity.label))
                                .size(22.0)
                                .strong()
                                .color(accent),
                        );
                        if !card.duplicate {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} +{:.0}%",
                                    slot.stat_label,
                                    slot.per_tier * rarity.bonus_mul * 100.0
                                ))
                                .size(14.0)
                                .color(egui::Color32::from_rgba_unmultiplied(230, 225, 220, alpha)),
                            );
                            // Le pourcentage dit ce que la pièce fait ; la Puissance
                            // dit où l'on en est. C'est le second qui se retient
                            // d'une run à l'autre.
                            let power_line = if card.power_after > card.power_before {
                                Some(format!(
                                    "Puissance {} → {}",
                                    card.power_before, card.power_after
                                ))
                            } else if card.power_potential > card.power_after {
                                Some(format!(
                                    "Puissance {} → {} si équipée",
                                    card.power_after, card.power_potential
                                ))
                            } else {
                                None
                            };
                            if let Some(line) = power_line {
                                ui.label(
                                    egui::RichText::new(line).size(12.0).strong().color(accent),
                                );
                            }
                        }
                    });
                });
        });
}

// ── Capteur ──────────────────────────────────────────────────────────────────

/// `forgia2_equipment.json` 1 Hz.
///
/// Alerte `EQUIPMENT_NO_DROPS` : des étages ont été franchis mais aucune pièce
/// n'est jamais tombée — le butin est muet, et sans ce signal ça ne se voit pas,
/// puisqu'un joueur sans pièce ressemble à un joueur qui débute.
fn sys_write_equipment_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cfg: Res<EquipmentConfig>,
    save: Res<EquipmentSave>,
    mods: Res<EquipmentMods>,
    shown: Res<EquipmentPanelShown>,
    mark: Res<StageWatermark>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let owned_total: usize = save.owned.values().map(Vec::len).sum();
    let (severity, next_step) = if cfg.slots.is_empty() {
        (
            "warn",
            "aucun slot charge — verifier assets/genomes/roguelite/roguelite_equipment.toml",
        )
    } else if mark.0.is_some() && save.drops_total == 0 {
        (
            "warn",
            "etage atteint sans aucun drop — verifier drop_weight et sys_grant_stage_drops",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"equipment","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"slots_total":{},"rarities_total":{},"owned_total":{},"equipped_total":{},"drops_total":{},"power":{},"power_max":{},"power_record":{},"panel_shown":{},"damage_mul":{:.3},"fire_rate_mul":{:.3},"damage_reduction":{:.3},"crit_chance":{:.3},"headshot_bonus_mul":{:.3}}}"#,
        time.elapsed_secs(),
        cfg.slots.len(),
        cfg.rarities.len(),
        owned_total,
        save.equipped.len(),
        save.drops_total,
        power_score(&cfg, &save),
        cfg.power_max(),
        save.power_record,
        shown.0,
        mods.damage_mul,
        mods.fire_rate_mul,
        mods.damage_reduction,
        mods.crit_chance,
        mods.headshot_bonus_mul,
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

// ── Plugin ───────────────────────────────────────────────────────────────────

pub struct EquipmentPlugin;

impl Plugin for EquipmentPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EquipmentConfig::load())
            .insert_resource(EquipmentSave::load_or_default())
            .init_resource::<EquipmentMods>()
            .init_resource::<EquipmentPanelShown>()
            .init_resource::<StageWatermark>()
            .init_resource::<LootCard>()
            .add_systems(
                EguiPrimaryContextPass,
                draw_loot_card.run_if(in_state(GameMode::Roguelite)),
            )
            // Le recompute tourne PARTOUT, y compris au menu : c'est là qu'on
            // équipe, et le bilan du panneau doit refléter ce qu'on vient de
            // cliquer. Le gater sur Roguelite affichait « aucun bonus actif »
            // avec trois pièces portées — l'écran mentait, comme un capteur.
            .add_systems(
                Update,
                (
                    sys_recompute_equipment_mods,
                    sys_track_power_record,
                    sys_publish_arm_armor_tint,
                    sys_point_viewmodel_arms_at_character,
                ),
            )
            // 🚨 Le butin, lui, lit `RunState` — un SubState dont la Resource
            // N'EXISTE PAS hors de `GameMode::Roguelite`. Sans cette garde le
            // système est écarté à chaque frame avec un avertissement.
            .add_systems(
                Update,
                sys_grant_stage_drops.run_if(in_state(GameMode::Roguelite)),
            )
            // Ni le panneau ni l'aperçu 3D ne vivent ici. Le Lobby n'est plus un
            // hub interactif mais un gate de chargement, dont l'overlay plein
            // écran (`forgia_ui::sys_lobby_loading_overlay`, `Order::Foreground`)
            // recouvre tout — et le menu n'a pas de scène 3D. Les deux vivent
            // donc dans `forgia-ui` : `sys_menu_equipement` pour le panneau,
            // `weapon_preview` pour l'aperçu rendu hors écran.
            .add_systems(Update, sys_write_equipment_sensor.in_set(GameSet::Sensors));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> EquipmentConfig {
        toml::from_str(
            &std::fs::read_to_string("../../assets/genomes/roguelite/roguelite_equipment.toml")
                .expect("le genome d'équipement doit être lisible depuis la crate"),
        )
        .expect("le genome d'équipement doit parser")
    }

    #[test]
    fn genome_declares_five_slots_and_five_rarities() {
        let c = cfg();
        assert_eq!(c.slots.len(), 5);
        assert_eq!(c.rarities.len(), 5);
    }

    /// Chaque modèle déclaré doit exister — corps compris. Un chemin qui ne
    /// pointe rien donne un aperçu vide, et rien ne le signale au runtime.
    #[test]
    fn every_model_exists_on_disk() {
        let c = cfg();
        let assets = std::path::Path::new("../../assets");
        assert!(
            !c.body_model.is_empty(),
            "le corps de base doit être déclaré"
        );
        assert!(
            assets.join(&c.body_model).exists(),
            "corps absent : {}",
            c.body_model
        );
        for slot in c.slots {
            assert!(
                assets.join(&slot.model).exists(),
                "modèle absent pour {}: {}",
                slot.id,
                slot.model
            );
        }
    }

    /// Chaque `stat` du TOML doit être une statistique que `compute_mods` sait
    /// appliquer — sinon la pièce ne fait rien et personne ne le voit.
    #[test]
    fn every_slot_stat_is_applied() {
        let c = cfg();
        for slot in &c.slots {
            let mut save = EquipmentSave::default();
            let top = c.rarities.last().expect("au moins une rareté");
            save.equipped.insert(slot.id.clone(), top.id.clone());
            let mods = compute_mods(&c, &save);
            assert_ne!(
                mods,
                EquipmentMods::default(),
                "la stat {:?} du slot {} n'est appliquée nulle part",
                slot.stat,
                slot.id
            );
        }
    }

    #[test]
    fn rarity_scales_the_gain() {
        let c = cfg();
        let slot = c.slot("legs").expect("slot legs");
        let mut low = EquipmentSave::default();
        low.equipped.insert("legs".into(), "commun".into());
        let mut high = EquipmentSave::default();
        high.equipped.insert("legs".into(), "mythique".into());
        let (a, b) = (compute_mods(&c, &low), compute_mods(&c, &high));
        assert!(b.damage_mul > a.damage_mul);
        let commun = c.rarity("commun").expect("rareté commun").bonus_mul;
        let mythique = c.rarity("mythique").expect("rareté mythique").bonus_mul;
        assert!(
            ((b.damage_mul - 1.0) / (a.damage_mul - 1.0) - mythique / commun).abs() < 1e-4,
            "le gain doit suivre exactement bonus_mul"
        );
        assert!(slot.per_tier > 0.0);
    }

    #[test]
    fn granting_twice_is_a_duplicate() {
        let mut save = EquipmentSave::default();
        assert!(save.grant("helmet", "rare"));
        assert!(!save.grant("helmet", "rare"));
        assert_eq!(save.owned["helmet"].len(), 1);
    }

    /// Une pièce trouvée pour un slot vide s'équipe seule ; une seconde ne
    /// remplace pas le choix du joueur.
    #[test]
    fn first_piece_auto_equips_but_never_overrides() {
        let mut save = EquipmentSave::default();
        save.grant("boots", "commun");
        assert_eq!(
            save.equipped.get("boots").map(String::as_str),
            Some("commun")
        );
        save.grant("boots", "mythique");
        assert_eq!(
            save.equipped.get("boots").map(String::as_str),
            Some("commun")
        );
    }

    #[test]
    fn drops_are_deterministic_for_a_given_seed() {
        let c = cfg();
        let save = EquipmentSave::default();
        let roll = |seed: u64| {
            let mut rng = Xoshiro256StarStar::seed_from_u64(seed);
            roll_piece(&c, &save, &mut rng)
        };
        assert_eq!(roll(42), roll(42));
    }

    /// Le tirage ne doit jamais rendre « rien » quand la table est peuplée.
    #[test]
    fn rolling_always_yields_a_piece() {
        let c = cfg();
        let save = EquipmentSave::default();
        for seed in 0..64u64 {
            let mut rng = Xoshiro256StarStar::seed_from_u64(seed);
            assert!(roll_piece(&c, &save, &mut rng).is_some(), "seed {seed}");
        }
    }

    #[test]
    fn save_roundtrip_toml() {
        let mut s = EquipmentSave::default();
        s.grant("chest", "epique");
        s.drops_total = 3;
        let ser = toml::to_string_pretty(&s).expect("serialise");
        let de: EquipmentSave = toml::from_str(&ser).expect("deserialise");
        assert!(de.owns("chest", "epique"));
        assert_eq!(de.drops_total, 3);
    }

    /// Un save d'une version antérieure, sans aucun champ d'équipement, doit
    /// charger sans paniquer (le joueur ne perd pas sa progression).
    #[test]
    fn legacy_save_without_fields_loads() {
        let s: EquipmentSave = toml::from_str("version = 1\n").expect("charge");
        assert!(s.owned.is_empty());
        assert_eq!(s.drops_total, 0);
    }
}
