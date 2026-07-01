//! elements.rs — Story-582 Phase A (2026-06-07). Système d'éléments par-arme.
//!
//! Chaque arme du loadout Roguelite a un **élément signature** (feu/poison/
//! électrique/perforant) avec :
//! - une **efficacité par type d'ennemi** (matchups Tank/Runner/Sniper/Boss),
//! - des **status effects** (burn = DoT plat, poison = DoT stackant + shred,
//!   shock = marque de vulnérabilité),
//! - de l'**arc électrique** (splash Shock), de l'**exécution** (perforant :
//!   one-shot tank) et des **réactions** (Combustion/Miasma/Surcharge) via un
//!   moteur générique data-driven ([`ReactionTable`], story-641).
//!
//! ## Décision d'architecture (critique)
//!
//! Les ennemis portent `forgia_combat::Health` (PAS `forgia_damage::Health`, qui
//! est sur le joueur). Donc `forgia_damage::DamageEvent` → `apply_damage` est un
//! **no-op sur les ennemis**. Ce module mute donc **`forgia_combat::Health`
//! directement** ; `despawn_dead_cubes` (forgia-fps) fait le pont vers
//! `DeathEvent` quand `current ≤ 0` (→ loot/heal/defeat). Pattern d'entrée =
//! `CombatHitEvent` (miroir de `boons_apply::sys_apply_chain_targets`).
//!
//! 100 % data-driven : `assets/genomes/roguelite/roguelite_elements.toml`
//! (hot-reload mtime, Shift+F12-like). Le `Default` Rust est le miroir exact du
//! TOML livré (zéro régression si le fichier disparaît).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_combat::weapons::{EquippedWeapons, WeaponType};
use forgia_combat::Health;
use forgia_damage::{DefenseLayer, ElementAffinity, Vulnerability};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::enemies::EnemyArchetype;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_elements.toml";
const POLL_PERIOD_SEC: f32 = 1.0;
const SENSOR_PATH: &str = "forgia2_elements.json";

/// Intervalle de tick des DoT (s) — on groupe les dégâts par tick (vs par frame)
/// pour des floating numbers lisibles + moins de mutations.
pub const STATUS_TICK_INTERVAL: f32 = 0.5;

// ─── Élément ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    /// Feu — burn DoT plat. Identité SMG (Bourrasque).
    Fire,
    /// Poison/Corrosif — DoT stackant + shred armure. Identité pompe (Boucherie).
    Poison,
    /// Électrique (Shock) — arc qui saute aux voisins + conditionne les réactions
    /// (Miasma/Surcharge) + marque `StatusShock` (+vulnérabilité). Identité pistolet (Pépin).
    /// (Ex-`Explosive` : remap story-641, l'AOE devient un arc électrique.)
    Shock,
    /// Perforant — gros bonus vs Tank + exécution sous seuil. Identité sniper (Lenoir).
    ArmorPierce,
}

impl Element {
    pub fn label(self) -> &'static str {
        match self {
            Element::Fire => "fire",
            Element::Poison => "poison",
            Element::Shock => "shock",
            Element::ArmorPierce => "armor_pierce",
        }
    }

    /// Parse une clé TOML (tolérante FR/EN, accents + casse) vers un élément.
    /// `to_lowercase` (Unicode) — pas `to_ascii_lowercase` — pour plier « É »→« é ».
    pub fn from_key(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "fire" | "feu" => Some(Element::Fire),
            "poison" | "corrosive" | "corrosif" => Some(Element::Poison),
            "shock" | "electric" | "electrique" | "électrique" | "elec" => Some(Element::Shock),
            "armor_pierce" | "perforant" | "pierce" => Some(Element::ArmorPierce),
            _ => None,
        }
    }

    /// Index stable `0..4` — pour indexer les handles VFX par élément
    /// ([`crate::element_vfx::ElementVfxAssets`]).
    pub fn idx(self) -> usize {
        match self {
            Element::Fire => 0,
            Element::Poison => 1,
            Element::Shock => 2,
            Element::ArmorPierce => 3,
        }
    }

    /// Couleur RGB (linéaire) de l'élément, data-driven via [`VfxParams`].
    pub fn rgb(self, v: &VfxParams) -> [f32; 3] {
        match self {
            Element::Fire => v.fire_rgb,
            Element::Poison => v.poison_rgb,
            Element::Shock => v.shock_rgb,
            Element::ArmorPierce => v.armor_pierce_rgb,
        }
    }

    /// Nom FR (cartes de choix HUD, story-589).
    pub fn fr_name(self) -> &'static str {
        match self {
            Element::Fire => "FEU",
            Element::Poison => "POISON",
            Element::Shock => "ÉLECTRIQUE",
            Element::ArmorPierce => "PERFORANT",
        }
    }

    /// Tag court d'identité (sous-titre de carte).
    pub fn tag(self) -> &'static str {
        match self {
            Element::Fire => "brûlure (DoT)",
            Element::Poison => "poison stackant + shred",
            Element::Shock => "arc électrique + réactions",
            Element::ArmorPierce => "exécute les tanks",
        }
    }

    /// Popup `&'static str` au déblocage (story-589 ; `KillPopup.text` est statique).
    pub fn armed_popup(self) -> &'static str {
        match self {
            Element::Fire => "FEU ARMÉ !",
            Element::Poison => "POISON ARMÉ !",
            Element::Shock => "ÉLECTRIQUE ARMÉ !",
            Element::ArmorPierce => "PERFORANT ARMÉ !",
        }
    }

    /// Itère les 4 éléments (ordre stable = ordre d'`idx`).
    pub fn all() -> [Element; 4] {
        [
            Element::Fire,
            Element::Poison,
            Element::Shock,
            Element::ArmorPierce,
        ]
    }
}

// ─── Config genome (mtime, miroir poi.rs) ───────────────────────────────────

/// Multiplicateur d'efficacité par type d'ennemi.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Matchup {
    pub tank: f32,
    pub runner: f32,
    pub sniper: f32,
    pub boss: f32,
}

impl Matchup {
    pub fn for_archetype(&self, a: EnemyArchetype) -> f32 {
        match a {
            EnemyArchetype::Tank => self.tank,
            EnemyArchetype::Runner => self.runner,
            EnemyArchetype::Sniper => self.sniper,
            EnemyArchetype::Boss => self.boss,
        }
    }
}

/// Table d'efficacité complète (un `Matchup` par élément).
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct MatchupTable {
    pub fire: Matchup,
    pub poison: Matchup,
    pub shock: Matchup,
    pub armor_pierce: Matchup,
}

impl MatchupTable {
    pub fn for_element(&self, e: Element) -> &Matchup {
        match e {
            Element::Fire => &self.fire,
            Element::Poison => &self.poison,
            Element::Shock => &self.shock,
            Element::ArmorPierce => &self.armor_pierce,
        }
    }
}

/// Mapping arme → clé d'élément (string, parsée via [`Element::from_key`]).
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct WeaponMapping {
    pub modern_ar: String,
    pub assault_rifle: String,
    pub shotgun: String,
    pub rocket_launcher: String,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct BurnParams {
    pub dps: f32,
    pub duration: f32,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct PoisonParams {
    pub dps_per_stack: f32,
    pub duration: f32,
    pub max_stacks: u32,
    /// +dégâts reçus par stack (amplifie le bonus de matchup) — le "shred armure".
    pub shred_per_stack: f32,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct AoeParams {
    pub radius: f32,
    pub damage_factor: f32,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct ExecuteParams {
    /// Fraction de PV max sous laquelle un hit perforant exécute (instakill).
    pub hp_ratio_threshold: f32,
}

/// Marque électrique (`Element::Shock`, story-641 Inc.2). Debuff de
/// **vulnérabilité** : les dégâts subis sont ×`vuln_mul` tant que `StatusShock`
/// tient (`duration` s). Non-stackant (un hit électrique rafraîchit la durée).
/// Le +10 % est appliqué au **bonus élémentaire** (matchup) et aux **réactions**
/// (Combustion/Miasma/Surcharge) ; le +10 % sur le **hit de base** est différé à
/// P0-4 (re-route via `DefenseLayer`). Miroir de la section `[shock]` du genome.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ShockParams {
    /// Durée (s) de la marque électrique (rafraîchie à chaque hit électrique).
    pub duration: f32,
    /// Multiplicateur de dégâts subis tant que la marque tient (1.1 = +10 %).
    pub vuln_mul: f32,
}

impl Default for ShockParams {
    fn default() -> Self {
        // Miroir EXACT de la section [shock] de roguelite_elements.toml.
        Self { duration: 4.0, vuln_mul: 1.1 }
    }
}

/// Réaction **Combustion** (Feu + Poison co-présents sur la cible). Voie "Gunfire
/// Reborn" (genre-validée 2026-06-23) : burst AOE dont les dégâts = **% du tir
/// déclencheur** (PAS des stacks), qui **garde** les statuts (re-pulse aux hits
/// suivants), throttlé par `retrigger_cooldown` par cible (anti-spam fire-rate).
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct CombustionParams {
    /// Master switch (le sensor flag si off).
    pub enabled: bool,
    /// Dégâts sur la cible = `target_pct × dégâts du tir` (2.0 = 200%, modèle Gunfire).
    pub target_pct: f32,
    /// Dégâts aux voisins dans le rayon = `area_pct × dégâts du tir` (1.0 = 100%).
    pub area_pct: f32,
    /// Rayon (m) du splash autour de la cible.
    pub radius: f32,
    /// Délai min (s) entre deux combustions sur la MÊME cible (anti-spam fire-rate).
    pub retrigger_cooldown: f32,
}

impl Default for CombustionParams {
    fn default() -> Self {
        // Miroir EXACT de la section [combustion] de roguelite_elements.toml.
        Self {
            enabled: true,
            target_pct: 2.0,
            area_pct: 1.0,
            radius: 3.5,
            retrigger_cooldown: 0.8,
        }
    }
}

/// Réaction **Miasma** (Électrique + Poison co-présents, story-641 Inc.3). DoT
/// **stackant en % des PV MAX** de la cible (donc mordant sur les gros PV : bosses,
/// tanks). Un déclenchement ajoute un stack + rafraîchit la durée. Miroir `[miasma]`.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct MiasmaParams {
    /// Master switch (le sensor flag si off).
    pub enabled: bool,
    /// % des PV max infligés **par seconde et par stack** (0.03 = 3 %/s/stack).
    pub pct_max_hp_per_sec: f32,
    /// Durée (s) du nuage (rafraîchie à chaque déclenchement).
    pub duration: f32,
    /// Plafond de stacks.
    pub max_stacks: u32,
    /// Délai min (s) entre deux déclenchements sur la MÊME cible (anti-spam).
    pub retrigger_cooldown: f32,
}

impl Default for MiasmaParams {
    fn default() -> Self {
        // Miroir EXACT de la section [miasma] de roguelite_elements.toml.
        Self {
            enabled: true,
            pct_max_hp_per_sec: 0.03,
            duration: 4.0,
            max_stacks: 5,
            retrigger_cooldown: 0.8,
        }
    }
}

/// Réaction **Surcharge** (Feu + Électrique co-présents, story-641 Inc.3). Décharge
/// AOE instantanée — même forme que la Combustion (burst % du tir), throttlée par
/// cible. Identité « arc qui explose ». Miroir `[surcharge]`.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SurchargeParams {
    /// Master switch (le sensor flag si off).
    pub enabled: bool,
    /// Dégâts sur la cible = `target_pct × dégâts du tir`.
    pub target_pct: f32,
    /// Dégâts aux voisins dans le rayon = `area_pct × dégâts du tir`.
    pub area_pct: f32,
    /// Rayon (m) de la décharge.
    pub radius: f32,
    /// Délai min (s) entre deux décharges sur la MÊME cible (anti-spam).
    pub retrigger_cooldown: f32,
}

impl Default for SurchargeParams {
    fn default() -> Self {
        // Miroir EXACT de la section [surcharge] de roguelite_elements.toml.
        Self {
            enabled: true,
            target_pct: 1.5,
            area_pct: 1.5,
            radius: 4.0,
            retrigger_cooldown: 0.8,
        }
    }
}

/// Affinité d'un élément contre les 3 couches (story-642 P0-4). Gène genome
/// (Deserialize) miroir de [`forgia_damage::ElementAffinity`] — `mult > 1` = fort,
/// `mult < 1` = faible. Converti en `ElementAffinity` à la consommation.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct AffinityGene {
    pub shield: f32,
    pub armor: f32,
    pub life: f32,
}

impl AffinityGene {
    pub fn to_affinity(self) -> ElementAffinity {
        ElementAffinity::new(self.shield, self.armor, self.life)
    }
}

/// Table d'affinité élément→couche (un `AffinityGene` par élément). Data-driven :
/// chaque élément est **fort** contre sa couche favorite (×1.5) et **faible**
/// contre les autres (×0.75) — critère masterplan « Feu +50 % Vie / −25 % Bouclier ».
/// Feu→Vie, Électrique→Bouclier, Perforant+Poison→Armure.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AffinityTable {
    pub fire: AffinityGene,
    pub poison: AffinityGene,
    pub shock: AffinityGene,
    pub armor_pierce: AffinityGene,
}

impl Default for AffinityTable {
    fn default() -> Self {
        // Miroir EXACT de la section [affinity.*] de roguelite_elements.toml.
        Self {
            fire: AffinityGene { shield: 0.75, armor: 0.75, life: 1.5 },
            poison: AffinityGene { shield: 0.75, armor: 1.5, life: 0.75 },
            shock: AffinityGene { shield: 1.5, armor: 0.75, life: 0.75 },
            armor_pierce: AffinityGene { shield: 0.75, armor: 1.5, life: 0.75 },
        }
    }
}

/// Paramètres VFX (story-588) — couleurs + tailles du flash d'impact et du pulse
/// DoT. `#[serde(default)]` : si la section `[vfx]` manque du TOML, fallback sur
/// ces valeurs (backward-compat avec les genomes Phase A). Hot-reload : les
/// couleurs sont ré-appliquées en place sur les matériaux partagés.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct VfxParams {
    /// Master switch des VFX éléments (le sensor flag si off).
    pub enabled: bool,
    /// Rayon (m) de la sphère de flash à l'impact d'un hit normal.
    pub impact_scale: f32,
    /// Durée de vie (s) d'un flash d'impact (fade par scale → 0).
    pub impact_ttl: f32,
    /// Multiplicateur de taille du flash pour un hit électrique (arc visible).
    pub arc_scale: f32,
    /// Intensité (lumens) de la lumière d'impact (fade avec le flash).
    pub light_intensity: f32,
    /// Portée (m) de la lumière d'impact.
    pub light_range: f32,
    /// Rayon (m) du pulse coloré sur un ennemi en DoT (burn/poison).
    pub dot_pulse_scale: f32,
    /// Période (s) entre deux pulses DoT.
    pub dot_pulse_period: f32,
    /// Hauteur (m) de l'aura de statut (flamme/poison) au-dessus de l'origine du
    /// mob (≈ pieds). Hot-reload : règle l'anti-occlusion (trop bas = caché dans
    /// le mesh ; capsule mob ~2 m). Lu par `status_vfx`.
    pub status_y: f32,
    pub fire_rgb: [f32; 3],
    pub poison_rgb: [f32; 3],
    pub shock_rgb: [f32; 3],
    pub armor_pierce_rgb: [f32; 3],
}

impl Default for VfxParams {
    fn default() -> Self {
        // Miroir EXACT de la section [vfx] de roguelite_elements.toml.
        Self {
            enabled: true,
            impact_scale: 0.55,
            impact_ttl: 0.28,
            arc_scale: 2.4,
            light_intensity: 40_000.0,
            light_range: 6.0,
            dot_pulse_scale: 0.3,
            dot_pulse_period: 0.45,
            status_y: 1.4,
            fire_rgb: [1.0, 0.42, 0.06],
            poison_rgb: [0.42, 1.0, 0.16],
            // Électrique — bleu électrique (ex-explosif jaune, remap story-641).
            shock_rgb: [0.35, 0.65, 1.0],
            armor_pierce_rgb: [0.30, 0.85, 1.0],
        }
    }
}

/// Config gameplay des éléments. Resource + parsée du TOML (mtime hot-reload).
#[derive(Resource, Deserialize, Clone, Debug, PartialEq)]
pub struct ElementConfig {
    /// **Override dev** (story-589) : `true` = les 4 éléments armés d'office
    /// (mode test Phase A). `false` (ship) = progression via [`ElementUnlocks`]
    /// (départ armé + déblocages au portail). N'est plus le gate runtime des
    /// hits : c'est `ElementUnlocks` qui gate ; `always_on` ne fait que remplir
    /// le Set au reset.
    pub always_on: bool,
    pub mapping: WeaponMapping,
    pub matchup: MatchupTable,
    pub burn: BurnParams,
    pub poison: PoisonParams,
    pub aoe: AoeParams,
    pub execute: ExecuteParams,
    /// VFX (story-588) — optionnel dans le TOML (backward-compat Phase A).
    #[serde(default)]
    pub vfx: VfxParams,
    /// Réaction Combustion (Feu+Poison) — optionnel dans le TOML (backward-compat).
    #[serde(default)]
    pub combustion: CombustionParams,
    /// Marque électrique + vulnérabilité (story-641 Inc.2) — optionnel (backward-compat).
    #[serde(default)]
    pub shock: ShockParams,
    /// Réaction Miasma (Élec+Poison → DoT %PV max stackant) — optionnel (backward-compat).
    #[serde(default)]
    pub miasma: MiasmaParams,
    /// Réaction Surcharge (Feu+Élec → décharge AOE) — optionnel (backward-compat).
    #[serde(default)]
    pub surcharge: SurchargeParams,
    /// Affinité élément→couche de défense (story-642 P0-4) — optionnel (backward-compat).
    #[serde(default)]
    pub affinity: AffinityTable,
}

impl Default for ElementConfig {
    fn default() -> Self {
        // Miroir EXACT de assets/genomes/roguelite/roguelite_elements.toml.
        Self {
            // Story-589 : défaut SHIP = progression (départ armé + déblocages
            // portail). `true` = override dev (4 éléments armés, test Phase A).
            always_on: false,
            mapping: WeaponMapping {
                // Pistolet Pépin = Électrique (story-641, ex-explosif).
                modern_ar: "shock".into(),
                assault_rifle: "fire".into(),
                shotgun: "armor_pierce".into(),
                rocket_launcher: "poison".into(),
            },
            matchup: MatchupTable {
                fire: Matchup { tank: 1.0, runner: 1.3, sniper: 1.1, boss: 1.0 },
                poison: Matchup { tank: 1.4, runner: 1.0, sniper: 1.1, boss: 1.2 },
                shock: Matchup { tank: 1.1, runner: 1.4, sniper: 1.2, boss: 1.0 },
                armor_pierce: Matchup { tank: 2.0, runner: 1.0, sniper: 1.3, boss: 1.5 },
            },
            burn: BurnParams { dps: 8.0, duration: 3.0 },
            poison: PoisonParams {
                dps_per_stack: 4.0,
                duration: 4.0,
                max_stacks: 5,
                shred_per_stack: 0.04,
            },
            aoe: AoeParams { radius: 3.5, damage_factor: 0.5 },
            execute: ExecuteParams { hp_ratio_threshold: 0.25 },
            vfx: VfxParams::default(),
            combustion: CombustionParams::default(),
            shock: ShockParams::default(),
            miasma: MiasmaParams::default(),
            surcharge: SurchargeParams::default(),
            affinity: AffinityTable::default(),
        }
    }
}

impl ElementConfig {
    /// Pur — testable headless. Fallback `Default` sur toute erreur de parse.
    pub fn parse_toml(content: &str) -> Self {
        toml::from_str(content).unwrap_or_else(|_| Self::default())
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }

    /// Élément signature d'une arme (None si l'arme n'est pas mappée).
    pub fn element_for(&self, w: WeaponType) -> Option<Element> {
        let key = match w {
            WeaponType::ModernAR => &self.mapping.modern_ar,
            WeaponType::AssaultRifle => &self.mapping.assault_rifle,
            WeaponType::Shotgun => &self.mapping.shotgun,
            WeaponType::RocketLauncher => &self.mapping.rocket_launcher,
            _ => return None,
        };
        Element::from_key(key)
    }

    pub fn matchup_for(&self, e: Element, a: EnemyArchetype) -> f32 {
        self.matchup.for_element(e).for_archetype(a)
    }

    /// Une réaction est-elle activée (master switch de sa section genome) ?
    pub fn reaction_enabled(&self, k: ReactionKind) -> bool {
        match k {
            ReactionKind::Combustion => self.combustion.enabled,
            ReactionKind::Miasma => self.miasma.enabled,
            ReactionKind::Surcharge => self.surcharge.enabled,
        }
    }

    /// Cooldown (s) anti-spam par cible d'une réaction.
    pub fn reaction_cooldown(&self, k: ReactionKind) -> f32 {
        match k {
            ReactionKind::Combustion => self.combustion.retrigger_cooldown,
            ReactionKind::Miasma => self.miasma.retrigger_cooldown,
            ReactionKind::Surcharge => self.surcharge.retrigger_cooldown,
        }
    }

    /// Affinité élément→couche (P0-4) sous forme [`ElementAffinity`] prête pour
    /// `DefenseLayer::absorb_elemental`.
    pub fn affinity_for(&self, e: Element) -> ElementAffinity {
        let g = match e {
            Element::Fire => self.affinity.fire,
            Element::Poison => self.affinity.poison,
            Element::Shock => self.affinity.shock,
            Element::ArmorPierce => self.affinity.armor_pierce,
        };
        g.to_affinity()
    }

    /// Params du burst AOE (`target_pct`, `area_pct`, `radius`) d'une réaction de
    /// type décharge (Combustion/Surcharge). Miasma n'est pas un burst (DoT) → 0.
    pub fn reaction_burst(&self, k: ReactionKind) -> (f32, f32, f32) {
        match k {
            ReactionKind::Combustion => (
                self.combustion.target_pct,
                self.combustion.area_pct,
                self.combustion.radius,
            ),
            ReactionKind::Surcharge => (
                self.surcharge.target_pct,
                self.surcharge.area_pct,
                self.surcharge.radius,
            ),
            ReactionKind::Miasma => (0.0, 0.0, 0.0),
        }
    }
}

// ─── Progression : éléments armés (story-589 Phase B) ───────────────────────

/// Éléments actuellement **armés** par le joueur dans le run. Un hit n'applique
/// son élément que si l'élément est dans ce set. Rempli au reset (départ = arme
/// de départ) puis étendu au portail de fin de zone (`loot_room::ZoneReward`).
/// L'override dev `always_on=true` remplit les 4 au reset (mode test Phase A).
#[derive(Resource, Default, Clone, Debug)]
pub struct ElementUnlocks(pub HashSet<Element>);

impl ElementUnlocks {
    pub fn is_unlocked(&self, e: Element) -> bool {
        self.0.contains(&e)
    }
    /// Arme un élément. Retourne `true` s'il ne l'était pas déjà.
    pub fn unlock(&mut self, e: Element) -> bool {
        self.0.insert(e)
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    /// Éléments encore verrouillés (ordre stable `idx`) — offerts au portail.
    pub fn locked(&self) -> Vec<Element> {
        Element::all()
            .into_iter()
            .filter(|e| !self.0.contains(e))
            .collect()
    }
}

/// Watch mtime du TOML pour hot-reload (miroir [`crate::poi::PoiGenomeWatch`]).
#[derive(Resource, Default, Debug)]
pub struct ElementGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

// ─── Status effects (DoT sur forgia_combat::Health) ─────────────────────────

/// Brûlure (élément Feu). DoT plat, NON-stackant : un nouveau hit rafraîchit
/// `secs_left` sans cumuler l'intensité.
#[derive(Component, Debug, Clone, Copy)]
pub struct StatusBurn {
    pub dps: f32,
    pub secs_left: f32,
    pub tick_accum: f32,
}

/// Poison (élément Corrosif). DoT STACKANT : `stacks × dps_per_stack` par
/// seconde. Un nouveau hit incrémente `stacks` (cap géré à l'application) et
/// rafraîchit `secs_left`. `stacks` est aussi lu pour le shred (ampli du bonus).
#[derive(Component, Debug, Clone, Copy)]
pub struct StatusPoison {
    pub stacks: u32,
    pub dps_per_stack: f32,
    pub secs_left: f32,
    pub tick_accum: f32,
}

// Marque électrique (ex-`StatusShock`) : remplacée story-642 P0-4 Inc.3 par le
// composant générique `forgia_damage::Vulnerability` — visible cross-crate pour que
// le HIT DE BASE (forgia-fps) soit aussi amplifié, pas seulement bonus/réactions.

/// Nuage Miasma (réaction Élec+Poison, story-641 Inc.3). DoT **stackant en % des
/// PV MAX** : `stacks × pct_max_hp_per_sec × max_hp` par seconde. Un déclenchement
/// incrémente `stacks` (cap à l'application) + rafraîchit `secs_left`. Tické par
/// [`sys_tick_element_status`] (groupé par `STATUS_TICK_INTERVAL`, comme burn/poison).
#[derive(Component, Debug, Clone, Copy)]
pub struct StatusMiasma {
    pub stacks: u32,
    pub pct_max_hp_per_sec: f32,
    pub secs_left: f32,
    pub tick_accum: f32,
}

// ─── Stats sensor ───────────────────────────────────────────────────────────

#[derive(Resource, Default, Debug, Clone)]
pub struct ElementStats {
    pub hits_fire: u32,
    pub hits_poison: u32,
    pub hits_shock: u32,
    pub hits_armor_pierce: u32,
    pub burns_applied: u32,
    pub poisons_applied: u32,
    pub shocks_applied: u32,
    pub aoe_hits: u32,
    pub executes: u32,
    pub combustions: u32,
    pub miasmas: u32,
    pub surcharges: u32,
    /// P0-4 — dégâts élémentaires absorbés par Bouclier/Armure (vs fuite vers la
    /// Vie). Couvre bonus de matchup (Inc.1) + bursts/arc/Miasma (Inc.2). Signal que
    /// le routage affinité→DefenseLayer est actif (>0 dès qu'un ennemi défendu est
    /// touché par un dégât élémentaire). Le hit de BASE (forgia-fps) reste hors compte
    /// (routé `Physical`, sans affinité — Inc.3).
    pub elem_absorbed: f32,
}

impl ElementStats {
    fn record_hit(&mut self, e: Element) {
        match e {
            Element::Fire => self.hits_fire = self.hits_fire.saturating_add(1),
            Element::Poison => self.hits_poison = self.hits_poison.saturating_add(1),
            Element::Shock => self.hits_shock = self.hits_shock.saturating_add(1),
            Element::ArmorPierce => {
                self.hits_armor_pierce = self.hits_armor_pierce.saturating_add(1)
            }
        }
    }

    fn record_reaction(&mut self, k: ReactionKind) {
        match k {
            ReactionKind::Combustion => self.combustions = self.combustions.saturating_add(1),
            ReactionKind::Miasma => self.miasmas = self.miasmas.saturating_add(1),
            ReactionKind::Surcharge => self.surcharges = self.surcharges.saturating_add(1),
        }
    }
}

// ─── Systems : genome load + hot-reload ─────────────────────────────────────

/// Startup : charge le TOML + initialise le watch mtime.
pub fn sys_init_element_genome(mut commands: Commands) {
    let cfg = ElementConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    info!(
        "[elements] genome loaded — always_on={} map[pistol={} smg={} sniper={} pompe={}] execute<{:.0}%",
        cfg.always_on,
        cfg.mapping.modern_ar,
        cfg.mapping.assault_rifle,
        cfg.mapping.shotgun,
        cfg.mapping.rocket_launcher,
        cfg.execute.hp_ratio_threshold * 100.0,
    );
    commands.insert_resource(cfg);
    commands.insert_resource(ElementGenomeWatch {
        last_mtime: mtime,
        ..default()
    });
}

/// Poll mtime 1Hz, re-parse si changé (hot-reload Shift+F12-like).
pub fn sys_hot_reload_element_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<ElementConfig>,
    mut watch: ResMut<ElementGenomeWatch>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;

    let Ok(meta) = fs::metadata(GENOME_PATH) else {
        return;
    };
    let Ok(mtime) = meta.modified() else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    let Ok(content) = fs::read_to_string(GENOME_PATH) else {
        return;
    };
    let new_cfg = ElementConfig::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_cfg != *cfg {
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!(
            "[elements] HOT-RELOADED #{} — always_on={} burn={:.0}dps/{:.0}s execute<{:.0}%",
            watch.reload_count,
            new_cfg.always_on,
            new_cfg.burn.dps,
            new_cfg.burn.duration,
            new_cfg.execute.hp_ratio_threshold * 100.0,
        );
        *cfg = new_cfg;
    }
}

/// OnEnter Roguelite — reset des compteurs sensor (run fraîche).
pub fn sys_reset_element_stats(mut stats: ResMut<ElementStats>) {
    *stats = ElementStats::default();
}

/// OnEnter Roguelite — reset des éléments armés (story-589). Slate vierge SAUF :
/// - **départ armé** : l'élément de l'arme de départ (`EquippedWeapons.current`)
///   est armé d'office (le joueur a toujours ≥1 élément actif),
/// - **override dev** `always_on=true` : les 4 éléments sont armés (mode test).
pub fn sys_reset_element_unlocks(
    config: Res<ElementConfig>,
    equipped: Option<Res<EquippedWeapons>>,
    mut unlocks: ResMut<ElementUnlocks>,
) {
    unlocks.0.clear();
    if config.always_on {
        for e in Element::all() {
            unlocks.0.insert(e);
        }
        info!("[elements] always_on → 4 éléments armés (mode dev)");
        return;
    }
    let start = equipped
        .as_deref()
        .and_then(|eq| config.element_for(eq.current));
    if let Some(e) = start {
        unlocks.0.insert(e);
        info!("[elements] départ armé : {} (arme de départ)", e.fr_name());
    } else {
        info!("[elements] départ sans élément (arme non mappée) — unlocks au portail");
    }
}

/// Override dev `always_on` **LIVE** : si le genome passe `always_on=true`
/// (hot-reload mtime), arme les 4 éléments immédiatement SANS relancer le run.
/// Cheap (check `count`). Ship = `always_on=false` → no-op total, la progression
/// (départ armé + portails) reste maître. Évite la friction « rien ne s'applique ».
pub fn sys_enforce_always_on(config: Res<ElementConfig>, mut unlocks: ResMut<ElementUnlocks>) {
    if config.always_on && unlocks.count() < Element::all().len() {
        for e in Element::all() {
            unlocks.0.insert(e);
        }
    }
}

// ─── Réactions élémentaires (moteur générique, story-641 Inc.3) ──────────────

/// Les 3 réactions de la direction Gunfire-lite. Chacune = un couple de statuts
/// co-présents sur la cible ([`ReactionKind::pair`]) → un effet (burst AOE ou DoT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReactionKind {
    /// Feu + Poison → burst AOE (% du tir). Voie Gunfire (story-611).
    Combustion,
    /// Électrique + Poison → DoT %PV max stackant (`StatusMiasma`).
    Miasma,
    /// Feu + Électrique → décharge AOE instantanée.
    Surcharge,
}

impl ReactionKind {
    /// Le couple d'éléments (statuts) co-présents qui déclenche la réaction.
    pub fn pair(self) -> (Element, Element) {
        match self {
            ReactionKind::Combustion => (Element::Fire, Element::Poison),
            ReactionKind::Miasma => (Element::Shock, Element::Poison),
            ReactionKind::Surcharge => (Element::Fire, Element::Shock),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ReactionKind::Combustion => "combustion",
            ReactionKind::Miasma => "miasma",
            ReactionKind::Surcharge => "surcharge",
        }
    }

    /// Ordre stable (sensor / itération de la table).
    pub fn all() -> [ReactionKind; 3] {
        [
            ReactionKind::Combustion,
            ReactionKind::Miasma,
            ReactionKind::Surcharge,
        ]
    }

    /// Élément dont l'**affinité** régit les dégâts de la réaction (P0-4 Inc.2). Une
    /// réaction naît d'un COUPLE de statuts (`pair()`) mais son burst/DoT a une seule
    /// nature de dégât — on choisit l'élément qui la caractérise :
    /// - **Combustion** (Feu+Poison) → **Feu** : le payoff est un burst explosif de type
    ///   brûlure (dégât direct), pas un DoT corrosif → affinité Feu, pas Poison.
    /// - **Surcharge** (Feu+Élec) → **Électrique** : décharge/arc.
    /// - **Miasma** (Élec+Poison) → **Poison** : nuage corrosif → armure.
    pub fn damage_element(self) -> Element {
        match self {
            ReactionKind::Combustion => Element::Fire,
            ReactionKind::Miasma => Element::Poison,
            ReactionKind::Surcharge => Element::Shock,
        }
    }
}

/// Table des réactions : mappe les statuts co-présents → les réactions déclenchées.
/// Pure/testable. `has_*` = le statut de l'élément est présent sur la cible
/// (pré-hit OU appliqué par le hit courant). L'activation reste data-driven :
/// chaque réaction est gatée par le `enabled` de sa section genome au moment de
/// l'application (cf [`ElementConfig::reaction_enabled`]).
pub struct ReactionTable;

impl ReactionTable {
    pub fn triggered(
        has_fire: bool,
        has_poison: bool,
        has_shock: bool,
    ) -> impl Iterator<Item = ReactionKind> {
        ReactionKind::all().into_iter().filter(move |k| {
            let (a, b) = k.pair();
            let present = |e: Element| match e {
                Element::Fire => has_fire,
                Element::Poison => has_poison,
                Element::Shock => has_shock,
                Element::ArmorPierce => false,
            };
            present(a) && present(b)
        })
    }
}

/// Émis quand une réaction de type **décharge** (Combustion/Surcharge) se
/// déclenche — consommé par `element_vfx::sys_spawn_reaction_vfx` pour le burst
/// visuel (couleurs des 2 éléments du couple). Miasma (DoT) n'émet pas de burst.
#[derive(Message, Debug, Clone, Copy)]
pub struct ReactionEvent {
    pub kind: ReactionKind,
    pub pos: Vec3,
    pub radius: f32,
}

/// Bundle du moteur de réactions (reste sous la limite de params Bevy — règle
/// scalability §params). `cooldowns` borné par (cible, réaction) : décrément +
/// purge/frame.
#[derive(SystemParam)]
pub struct ReactionCtx<'w, 's> {
    pub time: Res<'w, Time>,
    /// Présence d'un `StatusBurn` existant sur la cible (avant le hit courant).
    pub q_burn: Query<'w, 's, (), With<StatusBurn>>,
    /// Présence d'une `Vulnerability` (ex-`StatusShock`) sur la cible avant le hit —
    /// gate la vulnérabilité (+`vuln_mul`) + les réactions Surcharge/Miasma.
    pub q_shock: Query<'w, 's, (), With<Vulnerability>>,
    /// Nuage Miasma existant (stack + refresh à l'application).
    pub q_miasma: Query<'w, 's, &'static mut StatusMiasma>,
    /// Buffer voisins réutilisé (0 alloc/hit).
    pub buf: Local<'s, Vec<Entity>>,
    /// Cooldown par (cible, réaction) (s restantes) — anti-spam fire-rate.
    pub cooldowns: Local<'s, HashMap<(Entity, ReactionKind), f32>>,
    /// Émet le `ReactionEvent` pour le VFX (bursts Combustion/Surcharge).
    pub vfx_events: MessageWriter<'w, ReactionEvent>,
}

// ─── System : application des éléments au hit ───────────────────────────────

/// PUR — bonus élémentaire **brut** (avant routage couches) : `base × (matchup ×
/// shred − 1) × vuln`, clampé ≥ 0. Séparé de l'application pour permettre le
/// routage via `DefenseLayer` (P0-4). `vuln_mul` = vulnérabilité `StatusShock`.
pub fn elemental_bonus(base_damage: f32, matchup: f32, shred_amp: f32, vuln_mul: f32) -> f32 {
    (base_damage * (matchup * shred_amp - 1.0)).max(0.0) * vuln_mul
}

/// PUR — applique `leak` (résidu après absorption des couches, ou bonus brut si la
/// cible n'a pas de `DefenseLayer`) à la Vie + décide l'exécution perforante.
/// Exécution = un hit `ArmorPierce` qui amène une cible **survivante** sous le
/// seuil de PV → instakill (one-shot tank).
pub fn resolve_leak_to_health(
    element: Element,
    cur_hp: f32,
    max_hp: f32,
    leak: f32,
    execute_threshold: f32,
) -> (f32, bool) {
    let survives = cur_hp - leak;
    if element == Element::ArmorPierce && survives > 0.0 && survives < execute_threshold * max_hp {
        (0.0, true)
    } else {
        (survives.max(0.0), false)
    }
}

/// PUR (testable headless) — chemin **sans couche de défense** : compose
/// [`elemental_bonus`] + [`resolve_leak_to_health`]. `cur_hp` = PV de la cible
/// APRÈS le hit de base (déjà soustrait par forgia-fps). Conservé pour les cibles
/// sans `DefenseLayer` ; avec couche, le caller route le bonus via
/// `DefenseLayer::absorb_elemental` (affinité, P0-4) puis appelle `resolve_leak_to_health`.
#[allow(clippy::too_many_arguments)]
pub fn resolve_target_hit(
    element: Element,
    cur_hp: f32,
    max_hp: f32,
    base_damage: f32,
    matchup: f32,
    shred_amp: f32,
    vuln_mul: f32,
    execute_threshold: f32,
) -> (f32, bool) {
    let bonus = elemental_bonus(base_damage, matchup, shred_amp, vuln_mul);
    resolve_leak_to_health(element, cur_hp, max_hp, bonus, execute_threshold)
}

/// PUR (testable) — dégâts d'un burst de réaction (voie Gunfire) : `(cible, zone)`
/// = `(target_pct, area_pct) × dégâts du tir déclencheur`. Clampé ≥ 0. Partagé
/// Combustion + Surcharge (même forme, params distincts).
pub fn combustion_damage(base_damage: f32, target_pct: f32, area_pct: f32) -> (f32, f32) {
    (
        (base_damage * target_pct).max(0.0),
        (base_damage * area_pct).max(0.0),
    )
}

/// PUR (testable) — dégâts de Miasma sur un intervalle : `stacks ×
/// pct_max_hp_per_sec × max_hp × seconds`. En % des PV MAX → mord sur les gros PV.
/// Clampé ≥ 0.
pub fn miasma_damage(stacks: u32, pct_max_hp_per_sec: f32, max_hp: f32, seconds: f32) -> f32 {
    (stacks as f32 * pct_max_hp_per_sec * max_hp * seconds).max(0.0)
}

/// Applique `raw` dégâts **élémentaires** à un ennemi (P0-4 Inc.2) : draine sa
/// couche défensive (affinité par couche) puis la Vie ; sans couche → direct.
/// Cumule l'absorbé dans le sensor. Centralise le routage pour bursts/arc/Miasma
/// (le bonus de matchup reste inline car couplé à l'exécution perforante).
fn route_elemental_damage(
    target: Entity,
    raw: f32,
    aff: &ElementAffinity,
    q_health: &mut Query<&mut Health, With<EnemyArchetype>>,
    q_defense: &mut Query<&mut DefenseLayer, With<EnemyArchetype>>,
    stats: &mut ElementStats,
) -> bool {
    if raw <= 0.0 {
        return false;
    }
    let leak = if let Ok(mut dl) = q_defense.get_mut(target) {
        dl.note_hit();
        let leak = dl.absorb_elemental(raw, aff);
        stats.elem_absorbed += (raw - leak).max(0.0);
        leak
    } else {
        raw
    };
    if let Ok(mut hp) = q_health.get_mut(target) {
        hp.current = (hp.current - leak).max(0.0);
        true
    } else {
        false
    }
}

/// Lit `CombatHitEvent` (le hit de base est DÉJÀ appliqué par forgia-fps) et
/// ajoute la couche élémentaire sur `forgia_combat::Health` :
/// - **bonus de matchup** (× selon archetype, ampli par shred poison + vuln shock),
/// - **exécution** perforante (instakill sous seuil),
/// - **status** burn/poison/shock,
/// - **arc électrique** (splash aux voisins sur hit Shock),
/// - **réactions** (Combustion/Miasma/Surcharge) via le moteur générique.
///
/// **Drain de couche multi-passes (P0-4)** : un même tir peut drainer le `DefenseLayer`
/// de la cible en plusieurs passes INDÉPENDANTES (chacune avec sa propre affinité,
/// recalculée sur l'état courant — pas de double-comptage) : (1) hit de base `Physical`
/// (forgia-fps, hors ce fichier), (2) bonus de matchup (Inc.1), (3) burst de réaction
/// Combustion/Surcharge (Inc.2). C'est cumulatif par design (le bouclier fond plus vite
/// sous un tir « chargé »), pas un bug.
#[allow(clippy::too_many_arguments)]
pub fn sys_apply_elements_on_hit(
    mut events: MessageReader<CombatHitEvent>,
    config: Res<ElementConfig>,
    unlocks: Res<ElementUnlocks>,
    mut commands: Commands,
    mut stats: ResMut<ElementStats>,
    q_archetype: Query<&EnemyArchetype>,
    mut q_health: Query<&mut Health, With<EnemyArchetype>>,
    q_pos: Query<(Entity, &GlobalTransform), With<EnemyArchetype>>,
    mut q_poison: Query<&mut StatusPoison>,
    // P0-4 — couche défensive de la cible : le bonus élémentaire la draine (affinité)
    // avant la Vie. Composant distinct de Health → double emprunt disjoint OK.
    mut q_defense: Query<&mut DefenseLayer, With<EnemyArchetype>>,
    // Buffer AOE réutilisé (0 alloc dans le chemin combat — règle scalability §hot).
    mut aoe_buf: Local<Vec<Entity>>,
    // Moteur de réactions (Combustion/Miasma/Surcharge) — bundle sous la limite de params.
    mut react: ReactionCtx,
) {
    // Décrémente les cooldowns de réaction par (cible, kind) (borné : purge des expirés).
    let cd_dt = react.time.delta_secs();
    react.cooldowns.retain(|_, t| {
        *t -= cd_dt;
        *t > 0.0
    });
    for ev in events.read() {
        let Some(weapon) = ev.weapon else {
            continue;
        };
        let Some(element) = config.element_for(weapon) else {
            continue;
        };
        // Story-589 : l'élément n'agit que s'il est ARMÉ (départ + déblocages portail).
        if !unlocks.is_unlocked(element) {
            continue;
        }
        let Ok(archetype) = q_archetype.get(ev.target).copied() else {
            continue;
        };
        stats.record_hit(element);

        // Effets sur la CIBLE — seulement si le hit de base ne l'a pas DÉJÀ tuée
        // (`ev.is_kill`). Sinon : `try_insert` sur entité en cours de despawn +
        // stats faussées. Un bonus/exécution qui amène la cible à 0 PV est balayé
        // par `despawn_dead_cubes` (sweep par frame, forgia-fps) → DeathEvent/loot.
        // Présence des statuts AVANT ce hit (réactions = co-présence de 2 statuts).
        // Calculé HORS du guard is_kill : une réaction détone aussi au coup fatal.
        let had_burn = react.q_burn.contains(ev.target);
        let had_poison = q_poison.contains(ev.target);
        // Vulnérabilité électrique : la marque doit préexister (pré-hit) — un hit
        // électrique n'amplifie pas son PROPRE bonus (débuff persistant, cohérent
        // avec had_burn/had_poison des réactions).
        let had_shock = react.q_shock.contains(ev.target);

        if !ev.is_kill {
            let matchup = config.matchup_for(element, archetype);
            // Shred : tant que des stacks de poison sont actifs, le bonus est amplifié.
            let shred_amp = q_poison
                .get(ev.target)
                .map(|p| 1.0 + p.stacks as f32 * config.poison.shred_per_stack)
                .unwrap_or(1.0);
            // ×vuln si la cible portait déjà StatusShock (Inc.2).
            let vuln_mul = if had_shock { config.shock.vuln_mul } else { 1.0 };

            if let Ok(mut hp) = q_health.get_mut(ev.target) {
                // P0-4 — le bonus élémentaire draine d'abord le DefenseLayer (affinité
                // par couche) ; seul le résidu entame la Vie. Sans couche : bonus direct.
                let bonus = elemental_bonus(ev.damage, matchup, shred_amp, vuln_mul);
                let leak = if let Ok(mut dl) = q_defense.get_mut(ev.target) {
                    dl.note_hit();
                    let leak = dl.absorb_elemental(bonus, &config.affinity_for(element));
                    stats.elem_absorbed += (bonus - leak).max(0.0);
                    leak
                } else {
                    bonus
                };
                let (new_hp, executed) = resolve_leak_to_health(
                    element,
                    hp.current,
                    hp.max,
                    leak,
                    config.execute.hp_ratio_threshold,
                );
                hp.current = new_hp;
                if executed {
                    stats.executes = stats.executes.saturating_add(1);
                }
            }

            // Status effects (DoT) sur la cible.
            match element {
                Element::Fire => {
                    commands.entity(ev.target).try_insert(StatusBurn {
                        dps: config.burn.dps,
                        secs_left: config.burn.duration,
                        tick_accum: 0.0,
                    });
                    stats.burns_applied = stats.burns_applied.saturating_add(1);
                }
                Element::Poison => {
                    if let Ok(mut p) = q_poison.get_mut(ev.target) {
                        p.stacks = (p.stacks + 1).min(config.poison.max_stacks);
                        p.secs_left = config.poison.duration;
                        p.dps_per_stack = config.poison.dps_per_stack;
                    } else {
                        commands.entity(ev.target).try_insert(StatusPoison {
                            stacks: 1,
                            dps_per_stack: config.poison.dps_per_stack,
                            secs_left: config.poison.duration,
                            tick_accum: 0.0,
                        });
                    }
                    stats.poisons_applied = stats.poisons_applied.saturating_add(1);
                }
                Element::Shock => {
                    // Marque électrique (non-stackante) : try_insert écrase → rafraîchit.
                    // `Vulnerability` (forgia-damage) porte le mult → lu par le hit de
                    // base (forgia-fps) ET la couche élémentaire (P0-4 Inc.3).
                    commands
                        .entity(ev.target)
                        .try_insert(Vulnerability::new(config.shock.vuln_mul, config.shock.duration));
                    stats.shocks_applied = stats.shocks_applied.saturating_add(1);
                }
                Element::ArmorPierce => {}
            }
        }

        // ── Moteur de réactions (story-641 Inc.3) : Combustion (Feu+Poison) /
        //    Miasma (Élec+Poison) / Surcharge (Feu+Élec). Généralise l'ex-bloc
        //    combustion câblé. Une réaction GARDE les statuts (re-pulse aux tirs
        //    suivants), throttlée par (cible, kind). Guard `ev.damage > 0` : un tir
        //    sans dégât (roquette genome damage=0) ne déclenche rien. Event-driven,
        //    0 scan/frame.
        //
        //    3 statuts co-présents (Feu+Poison+Élec) → Combustion ET Surcharge
        //    détonent le même hit (dégâts additifs) : c'est le **payoff assumé** du
        //    triple élément (verrouillé par le test `reaction_table_all_statuses_*`).
        //    Chaque réaction reste bornée par son cooldown/cible aux hits suivants.
        if ev.damage > 0.0 {
            let now_fire = had_burn || element == Element::Fire;
            let now_poison = had_poison || element == Element::Poison;
            let now_shock = had_shock || element == Element::Shock;
            // Cible déjà électrisée → la réaction (décharge) est amplifiée (vuln Inc.2).
            let vuln = if had_shock { config.shock.vuln_mul } else { 1.0 };
            let origin = ev.hit_world_pos;
            for kind in ReactionTable::triggered(now_fire, now_poison, now_shock) {
                // Miasma = DoT sur la CIBLE : inutile (et stat trompeuse) si le hit
                // de base l'a tuée (elle despawn ce frame). Les bursts détonent quand
                // même : ils frappent les VOISINS (comportement voulu, = ex-combustion).
                if kind == ReactionKind::Miasma && ev.is_kill {
                    continue;
                }
                if !config.reaction_enabled(kind) {
                    continue;
                }
                let key = (ev.target, kind);
                if react.cooldowns.contains_key(&key) {
                    continue;
                }
                react.cooldowns.insert(key, config.reaction_cooldown(kind));
                match kind {
                    // Décharge AOE (Combustion/Surcharge) : burst = % du tir sur la
                    // cible + voisins dans le rayon, ×vuln. Buffer réutilisé (0 alloc).
                    ReactionKind::Combustion | ReactionKind::Surcharge => {
                        let (target_pct, area_pct, radius) = config.reaction_burst(kind);
                        let (tgt_dmg, area_dmg) =
                            combustion_damage(ev.damage, target_pct, area_pct);
                        let (tgt_dmg, area_dmg) = (tgt_dmg * vuln, area_dmg * vuln);
                        // P0-4 Inc.2 — le burst draine la couche défensive (affinité de
                        // l'élément de la réaction) avant la Vie : cible + voisins.
                        let aff = config.affinity_for(kind.damage_element());
                        route_elemental_damage(
                            ev.target, tgt_dmg, &aff, &mut q_health, &mut q_defense, &mut stats,
                        );
                        let r2 = radius * radius;
                        react.buf.clear();
                        react.buf.extend(q_pos.iter().filter_map(|(e, gt)| {
                            (e != ev.target && (gt.translation() - origin).length_squared() <= r2)
                                .then_some(e)
                        }));
                        for &e in &*react.buf {
                            route_elemental_damage(
                                e, area_dmg, &aff, &mut q_health, &mut q_defense, &mut stats,
                            );
                        }
                        react.vfx_events.write(ReactionEvent { kind, pos: origin, radius });
                    }
                    // Miasma : DoT stackant %PV max (pas d'instant-damage → pas de
                    // vuln ici, l'amplification passe par les stacks).
                    ReactionKind::Miasma => {
                        if let Ok(mut m) = react.q_miasma.get_mut(ev.target) {
                            m.stacks = (m.stacks + 1).min(config.miasma.max_stacks);
                            m.secs_left = config.miasma.duration;
                            m.pct_max_hp_per_sec = config.miasma.pct_max_hp_per_sec;
                        } else {
                            commands.entity(ev.target).try_insert(StatusMiasma {
                                stacks: 1,
                                pct_max_hp_per_sec: config.miasma.pct_max_hp_per_sec,
                                secs_left: config.miasma.duration,
                                tick_accum: 0.0,
                            });
                        }
                    }
                }
                stats.record_reaction(kind);
            }
        }

        // Arc électrique (ex-splash explosif, remap story-641) — saute aux VOISINS,
        // indépendant de la mort de la cible (un tir qui tue doit quand même arcer).
        // Collecte d'abord (q_pos immutable) puis applique via le DefenseLayer
        // (affinité Shock, P0-4 Inc.2) ; buffer réutilisé.
        if element == Element::Shock {
            let origin = ev.hit_world_pos;
            let r2 = config.aoe.radius * config.aoe.radius;
            let splash = (ev.damage * config.aoe.damage_factor).max(0.0);
            let aff = config.affinity_for(Element::Shock);
            aoe_buf.clear();
            aoe_buf.extend(q_pos.iter().filter_map(|(e, gt)| {
                (e != ev.target && (gt.translation() - origin).length_squared() <= r2)
                    .then_some(e)
            }));
            let mut hits = 0u32;
            for &e in &*aoe_buf {
                if route_elemental_damage(e, splash, &aff, &mut q_health, &mut q_defense, &mut stats)
                {
                    hits += 1;
                }
            }
            stats.aoe_hits = stats.aoe_hits.saturating_add(hits);
        }
    }
}

/// Tick des DoT élémentaires sur les ennemis. Dégâts groupés par
/// `STATUS_TICK_INTERVAL`, retrait du component à expiration. `despawn_dead_cubes`
/// (forgia-fps) gère la mort quand `current ≤ 0` (→ DeathEvent → loot/heal).
pub fn sys_tick_element_status(
    time: Res<Time>,
    mut commands: Commands,
    config: Res<ElementConfig>,
    mut stats: ResMut<ElementStats>,
    mut q: Query<
        (
            Entity,
            &mut Health,
            Option<&mut StatusBurn>,
            Option<&mut StatusPoison>,
            Option<&mut Vulnerability>,
            Option<&mut StatusMiasma>,
            Option<&mut DefenseLayer>,
        ),
        With<EnemyArchetype>,
    >,
) {
    let dt = time.delta_secs();
    // Miasma = nuage corrosif → affinité Poison (fort armure) — P0-4 Inc.2.
    let poison_aff = config.affinity_for(Element::Poison);
    for (e, mut hp, burn, poison, vuln, miasma, mut defense) in &mut q {
        // `total` = DoT « purs » (burn + poison) qui bypassent les couches
        // (TrueHealth) ; Miasma est routé à part via le DefenseLayer.
        let mut total = 0.0_f32;
        let max_hp = hp.max;

        if let Some(mut b) = burn {
            b.secs_left -= dt;
            b.tick_accum += dt;
            if b.tick_accum >= STATUS_TICK_INTERVAL {
                let ticks = (b.tick_accum / STATUS_TICK_INTERVAL).floor();
                b.tick_accum -= ticks * STATUS_TICK_INTERVAL;
                total += b.dps * STATUS_TICK_INTERVAL * ticks;
            }
            if b.secs_left <= 0.0 {
                commands.entity(e).try_remove::<StatusBurn>();
            }
        }

        if let Some(mut p) = poison {
            p.secs_left -= dt;
            p.tick_accum += dt;
            if p.tick_accum >= STATUS_TICK_INTERVAL {
                let ticks = (p.tick_accum / STATUS_TICK_INTERVAL).floor();
                p.tick_accum -= ticks * STATUS_TICK_INTERVAL;
                total += p.stacks as f32 * p.dps_per_stack * STATUS_TICK_INTERVAL * ticks;
            }
            if p.secs_left <= 0.0 {
                commands.entity(e).try_remove::<StatusPoison>();
            }
        }

        // Vulnérabilité électrique (ex-`StatusShock`, forgia-damage) : pas de DoT —
        // simple expiration de la marque (retrait à échéance, comme burn/poison).
        if let Some(mut v) = vuln {
            if v.tick(dt) {
                commands.entity(e).try_remove::<Vulnerability>();
            }
        }

        // Miasma (Élec+Poison) : DoT stackant en % des PV MAX (mord sur les gros PV).
        // Routé via le DefenseLayer (affinité Poison → armure) — P0-4 Inc.2. Le nuage
        // corrosif entame la couche (contrairement à burn/poison, DoT « purs »).
        if let Some(mut m) = miasma {
            m.secs_left -= dt;
            m.tick_accum += dt;
            let mut miasma_dmg = 0.0;
            if m.tick_accum >= STATUS_TICK_INTERVAL {
                let ticks = (m.tick_accum / STATUS_TICK_INTERVAL).floor();
                m.tick_accum -= ticks * STATUS_TICK_INTERVAL;
                miasma_dmg =
                    miasma_damage(m.stacks, m.pct_max_hp_per_sec, max_hp, STATUS_TICK_INTERVAL * ticks);
            }
            let expired = m.secs_left <= 0.0;
            if miasma_dmg > 0.0 {
                let leak = if let Some(dl) = defense.as_deref_mut() {
                    dl.note_hit();
                    let leak = dl.absorb_elemental(miasma_dmg, &poison_aff);
                    stats.elem_absorbed += (miasma_dmg - leak).max(0.0);
                    leak
                } else {
                    miasma_dmg
                };
                hp.current = (hp.current - leak).max(0.0);
            }
            if expired {
                commands.entity(e).try_remove::<StatusMiasma>();
            }
        }

        // DoT « purs » (burn + poison) — appliqués APRÈS Miasma, directement à la Vie.
        if total > 0.0 {
            hp.current = (hp.current - total).max(0.0);
        }
    }
}

// ─── Sensor forgia2_elements.json ───────────────────────────────────────────

/// Écrit `forgia2_elements.json` 1Hz : mapping par arme, hits par élément, DoT
/// actifs, executes, **éléments armés** (story-589). Severity `warn` si aucun
/// élément armé hors mode dev (le départ-armé a échoué → progression cassée).
pub fn sys_write_elements_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    config: Res<ElementConfig>,
    unlocks: Res<ElementUnlocks>,
    stats: Res<ElementStats>,
    q_burn: Query<(), With<StatusBurn>>,
    q_poison: Query<&StatusPoison>,
    q_shock: Query<(), With<Vulnerability>>,
    q_miasma: Query<&StatusMiasma>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;

    let active_burns = q_burn.iter().count();
    let active_poisons = q_poison.iter().count();
    let active_stacks: u32 = q_poison.iter().map(|p| p.stacks).sum();
    let active_shocks = q_shock.iter().count();
    let active_miasmas = q_miasma.iter().count();
    let active_miasma_stacks: u32 = q_miasma.iter().map(|m| m.stacks).sum();

    let (severity, next_step) = if config.always_on || unlocks.count() >= 1 {
        ("ok", "")
    } else {
        (
            "warn",
            "0 élément armé — départ-armé KO (EquippedWeapons absent/non mappé au reset Roguelite)",
        )
    };

    let json = format!(
        r#"{{"id":"elements","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"always_on":{},"unlocked":{{"fire":{},"poison":{},"shock":{},"armor_pierce":{}}},"unlocked_count":{},"mapping":{{"pistol":"{}","smg":"{}","sniper":"{}","pompe":"{}"}},"hits":{{"fire":{},"poison":{},"shock":{},"armor_pierce":{}}},"burns_applied":{},"poisons_applied":{},"shocks_applied":{},"aoe_hits":{},"executes":{},"elem_absorbed":{:.0},"reactions":{{"combustions":{},"miasmas":{},"surcharges":{}}},"active_burns":{active_burns},"active_poisons":{active_poisons},"active_poison_stacks":{active_stacks},"active_shocks":{active_shocks},"active_miasmas":{active_miasmas},"active_miasma_stacks":{active_miasma_stacks}}}"#,
        time.elapsed_secs(),
        config.always_on,
        unlocks.is_unlocked(Element::Fire),
        unlocks.is_unlocked(Element::Poison),
        unlocks.is_unlocked(Element::Shock),
        unlocks.is_unlocked(Element::ArmorPierce),
        unlocks.count(),
        config.mapping.modern_ar,
        config.mapping.assault_rifle,
        config.mapping.shotgun,
        config.mapping.rocket_launcher,
        stats.hits_fire,
        stats.hits_poison,
        stats.hits_shock,
        stats.hits_armor_pierce,
        stats.burns_applied,
        stats.poisons_applied,
        stats.shocks_applied,
        stats.aoe_hits,
        stats.executes,
        stats.elem_absorbed,
        stats.combustions,
        stats.miasmas,
        stats.surcharges,
    );

    if let Err(e) = std::fs::write(SENSOR_PATH, &json) {
        warn!("[elements] sensor write failed: {e}");
    }
}

// ─── Tests (logique pure) ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_maps_weapons_to_signature_elements() {
        let c = ElementConfig::default();
        assert_eq!(c.element_for(WeaponType::ModernAR), Some(Element::Shock));
        assert_eq!(c.element_for(WeaponType::AssaultRifle), Some(Element::Fire));
        assert_eq!(c.element_for(WeaponType::Shotgun), Some(Element::ArmorPierce));
        assert_eq!(
            c.element_for(WeaponType::RocketLauncher),
            Some(Element::Poison)
        );
    }

    #[test]
    fn unmapped_weapon_has_no_element() {
        let c = ElementConfig::default();
        assert_eq!(c.element_for(WeaponType::AK47), None);
        assert_eq!(c.element_for(WeaponType::Chainsaw), None);
    }

    #[test]
    fn armor_pierce_strongest_vs_tank() {
        let c = ElementConfig::default();
        let tank = c.matchup_for(Element::ArmorPierce, EnemyArchetype::Tank);
        let runner = c.matchup_for(Element::ArmorPierce, EnemyArchetype::Runner);
        assert!(tank >= 2.0, "perforant doit one-shot le Tank (×2.0)");
        assert!(tank > runner, "perforant plus fort vs Tank que vs Runner");
    }

    #[test]
    fn fire_strongest_vs_runner_shock_too() {
        let c = ElementConfig::default();
        assert!(
            c.matchup_for(Element::Fire, EnemyArchetype::Runner)
                > c.matchup_for(Element::Fire, EnemyArchetype::Tank)
        );
        assert!(
            c.matchup_for(Element::Shock, EnemyArchetype::Runner)
                > c.matchup_for(Element::Shock, EnemyArchetype::Boss)
        );
    }

    #[test]
    fn poison_strongest_vs_tank() {
        let c = ElementConfig::default();
        assert!(
            c.matchup_for(Element::Poison, EnemyArchetype::Tank)
                > c.matchup_for(Element::Poison, EnemyArchetype::Runner)
        );
    }

    #[test]
    fn from_key_tolerant_fr_en() {
        assert_eq!(Element::from_key("feu"), Some(Element::Fire));
        assert_eq!(Element::from_key("ÉLECTRIQUE"), Some(Element::Shock));
        assert_eq!(Element::from_key("shock"), Some(Element::Shock));
        assert_eq!(Element::from_key("perforant"), Some(Element::ArmorPierce));
        assert_eq!(Element::from_key("inconnu"), None);
    }

    #[test]
    fn parse_garbage_falls_back_to_default() {
        let c = ElementConfig::parse_toml("ceci n'est pas du TOML valide [[[");
        assert_eq!(c, ElementConfig::default());
    }

    #[test]
    fn default_is_progression_mode() {
        // Story-589 : défaut ship = progression (always_on=false → gate par ElementUnlocks).
        assert!(!ElementConfig::default().always_on);
    }

    #[test]
    fn unlocks_gate_and_locked_list() {
        let mut u = ElementUnlocks::default();
        assert_eq!(u.count(), 0);
        assert!(!u.is_unlocked(Element::Fire));
        assert_eq!(u.locked().len(), 4);
        assert!(u.unlock(Element::Fire));
        assert!(!u.unlock(Element::Fire), "re-unlock = false (déjà armé)");
        assert!(u.is_unlocked(Element::Fire));
        assert_eq!(u.count(), 1);
        let locked = u.locked();
        assert_eq!(locked.len(), 3);
        assert!(!locked.contains(&Element::Fire));
    }

    #[test]
    fn execute_threshold_sane() {
        let c = ElementConfig::default();
        assert!(c.execute.hp_ratio_threshold > 0.0 && c.execute.hp_ratio_threshold < 1.0);
    }

    // ── resolve_target_hit (logique de hit, story-582 QA hardening) ──

    #[test]
    fn armor_pierce_executes_full_tank_body_shot() {
        // Tank 120 PV, body shot Lenoir (base 50) → cur_hp post-base = 70.
        // matchup ×2.0 → bonus 50 → survives 20 < 0.25×120=30 → exécution.
        let (hp, exec) =
            resolve_target_hit(Element::ArmorPierce, 70.0, 120.0, 50.0, 2.0, 1.0, 1.0, 0.25);
        assert!(exec, "perforant doit exécuter un Tank affaibli sous le seuil");
        assert_eq!(hp, 0.0);
    }

    #[test]
    fn armor_pierce_does_not_execute_boss() {
        // Boss 800 PV, body 50 → cur 750, matchup boss 1.5 → bonus 25 → 725 ≫ 200.
        let (hp, exec) =
            resolve_target_hit(Element::ArmorPierce, 750.0, 800.0, 50.0, 1.5, 1.0, 1.0, 0.25);
        assert!(!exec, "perforant ne doit PAS one-shot le Boss");
        assert!((hp - 725.0).abs() < 1e-3);
    }

    #[test]
    fn fire_applies_matchup_bonus_no_execute() {
        // Runner, base 16 (SMG), matchup fire×runner 1.3 → bonus 4.8 → 30 → 25.2.
        let (hp, exec) = resolve_target_hit(Element::Fire, 30.0, 35.0, 16.0, 1.3, 1.0, 1.0, 0.25);
        assert!(!exec, "seul ArmorPierce exécute");
        assert!((hp - 25.2).abs() < 1e-3);
    }

    #[test]
    fn neutral_matchup_is_noop() {
        let (hp, exec) = resolve_target_hit(Element::Poison, 50.0, 100.0, 20.0, 1.0, 1.0, 1.0, 0.25);
        assert!(!exec);
        assert_eq!(hp, 50.0, "matchup 1.0 = aucun bonus");
    }

    #[test]
    fn poison_shred_amplifies_bonus() {
        // shred_amp 1.2 (5 stacks ×0.04) × poison×tank 1.4 = 1.68.
        // base 18, bonus = 18×(1.68−1)=12.24, cur 100 → 87.76.
        let (hp, _) = resolve_target_hit(Element::Poison, 100.0, 120.0, 18.0, 1.4, 1.2, 1.0, 0.25);
        assert!((hp - 87.76).abs() < 1e-2);
    }

    // ── Combustion (réaction Feu+Poison, voie Gunfire) ──

    #[test]
    fn combustion_scales_on_triggering_shot_not_stacks() {
        // Voie Gunfire : burst = % du tir. base 50 → cible ×2.0 = 100, zone ×1.0 = 50.
        let (tgt, area) = combustion_damage(50.0, 2.0, 1.0);
        assert_eq!(tgt, 100.0);
        assert_eq!(area, 50.0);
    }

    #[test]
    fn combustion_damage_clamps_non_negative() {
        let (tgt, area) = combustion_damage(-5.0, 2.0, 1.0);
        assert_eq!(tgt, 0.0);
        assert_eq!(area, 0.0);
    }

    #[test]
    fn combustion_enabled_by_default_gunfire_shape() {
        let c = ElementConfig::default();
        assert!(c.combustion.enabled);
        assert!(
            c.combustion.target_pct > c.combustion.area_pct,
            "cible > zone (modèle Gunfire 200/100%)"
        );
        assert!(
            c.combustion.retrigger_cooldown > 0.0,
            "throttle anti-spam fire-rate requis"
        );
    }

    // ── StatusShock + vulnérabilité (story-641 Inc.2) ──

    #[test]
    fn shock_params_default_sane() {
        let s = ShockParams::default();
        assert!(s.duration > 0.0, "la marque électrique doit durer");
        assert!(s.vuln_mul > 1.0, "vulnérabilité = dégâts subis amplifiés (>1.0)");
    }

    #[test]
    fn shock_vuln_amplifies_bonus() {
        // Runner, base 16 (SMG), matchup fire×runner 1.3 → bonus 4.8.
        // Sans marque (vuln 1.0) : cur 30 → 25.2. Avec marque (vuln 1.1) : bonus
        // 5.28 → 24.72. La cible électrisée encaisse plus.
        let (no_shock, _) = resolve_target_hit(Element::Fire, 30.0, 35.0, 16.0, 1.3, 1.0, 1.0, 0.25);
        let (shocked, _) = resolve_target_hit(Element::Fire, 30.0, 35.0, 16.0, 1.3, 1.0, 1.1, 0.25);
        assert!((no_shock - 25.2).abs() < 1e-3);
        assert!((shocked - 24.72).abs() < 1e-3);
        assert!(shocked < no_shock, "la marque électrique amplifie le bonus");
    }

    #[test]
    fn shock_vuln_pushes_execute_threshold() {
        // Tank 120 PV (seuil exécution 0.25×120=30), body Lenoir base 40, matchup
        // ×2.0. cur 72 → sans marque : bonus 40 → survit 32 ≥ 30 → PAS d'exécution.
        // Avec marque ×1.1 : bonus 44 → survit 28 < 30 → exécution.
        let (hp_no, exec_no) =
            resolve_target_hit(Element::ArmorPierce, 72.0, 120.0, 40.0, 2.0, 1.0, 1.0, 0.25);
        let (hp_yes, exec_yes) =
            resolve_target_hit(Element::ArmorPierce, 72.0, 120.0, 40.0, 2.0, 1.0, 1.1, 0.25);
        assert!(!exec_no, "sans marque, la cible survit au-dessus du seuil");
        assert!((hp_no - 32.0).abs() < 1e-3);
        assert!(exec_yes, "avec marque, la vulnérabilité pousse sous le seuil → exécution");
        assert_eq!(hp_yes, 0.0);
    }

    #[test]
    fn config_shock_field_defaults_when_section_absent() {
        // Un genome sans [shock] doit parser et obtenir ShockParams par défaut.
        let c = ElementConfig::parse_toml("always_on = true");
        assert_eq!(c.shock, ShockParams::default());
    }

    // ── Moteur de réactions générique (story-641 Inc.3) ──

    #[test]
    fn reaction_pairs_are_the_three_combos() {
        assert_eq!(
            ReactionKind::Combustion.pair(),
            (Element::Fire, Element::Poison)
        );
        assert_eq!(ReactionKind::Miasma.pair(), (Element::Shock, Element::Poison));
        assert_eq!(
            ReactionKind::Surcharge.pair(),
            (Element::Fire, Element::Shock)
        );
    }

    fn triggered(fire: bool, poison: bool, shock: bool) -> Vec<ReactionKind> {
        ReactionTable::triggered(fire, poison, shock).collect()
    }

    #[test]
    fn reaction_table_maps_status_pairs_to_kinds() {
        assert_eq!(triggered(true, true, false), vec![ReactionKind::Combustion]);
        assert_eq!(triggered(false, true, true), vec![ReactionKind::Miasma]);
        assert_eq!(triggered(true, false, true), vec![ReactionKind::Surcharge]);
    }

    #[test]
    fn reaction_table_all_statuses_fires_all_three() {
        let all = triggered(true, true, true);
        assert_eq!(all.len(), 3, "3 statuts co-présents → les 3 réactions");
        for k in ReactionKind::all() {
            assert!(all.contains(&k));
        }
    }

    #[test]
    fn reaction_table_single_status_fires_nothing() {
        assert!(triggered(true, false, false).is_empty());
        assert!(triggered(false, true, false).is_empty());
        assert!(triggered(false, false, true).is_empty());
        assert!(triggered(false, false, false).is_empty());
    }

    #[test]
    fn reaction_defaults_sane() {
        let c = ElementConfig::default();
        assert!(c.miasma.enabled && c.surcharge.enabled);
        assert!(c.miasma.pct_max_hp_per_sec > 0.0 && c.miasma.max_stacks > 0);
        assert!(c.miasma.retrigger_cooldown > 0.0);
        assert!(c.surcharge.target_pct > 0.0 && c.surcharge.radius > 0.0);
    }

    #[test]
    fn reaction_enabled_and_cooldown_read_config() {
        let c = ElementConfig::default();
        assert!(c.reaction_enabled(ReactionKind::Combustion));
        assert!(c.reaction_enabled(ReactionKind::Miasma));
        assert!(c.reaction_enabled(ReactionKind::Surcharge));
        assert_eq!(c.reaction_cooldown(ReactionKind::Miasma), c.miasma.retrigger_cooldown);
        assert_eq!(
            c.reaction_cooldown(ReactionKind::Surcharge),
            c.surcharge.retrigger_cooldown
        );
    }

    #[test]
    fn reaction_burst_params_per_kind() {
        let c = ElementConfig::default();
        assert_eq!(
            c.reaction_burst(ReactionKind::Combustion),
            (c.combustion.target_pct, c.combustion.area_pct, c.combustion.radius)
        );
        assert_eq!(
            c.reaction_burst(ReactionKind::Surcharge),
            (c.surcharge.target_pct, c.surcharge.area_pct, c.surcharge.radius)
        );
        assert_eq!(
            c.reaction_burst(ReactionKind::Miasma),
            (0.0, 0.0, 0.0),
            "Miasma est un DoT, pas un burst"
        );
    }

    #[test]
    fn miasma_damage_scales_on_max_hp_and_stacks() {
        // 3 stacks, 3%/s/stack, boss 800 PV, 0.5 s → 3×0.03×800×0.5 = 36.
        assert!((miasma_damage(3, 0.03, 800.0, 0.5) - 36.0).abs() < 1e-3);
        // % PV max : mord davantage sur un boss que sur un runner (même stacks).
        assert!(miasma_damage(1, 0.03, 800.0, 1.0) > miasma_damage(1, 0.03, 35.0, 1.0));
    }

    #[test]
    fn miasma_damage_clamps_non_negative() {
        assert_eq!(miasma_damage(0, 0.03, 800.0, 0.5), 0.0);
        assert_eq!(miasma_damage(3, -0.1, 800.0, 0.5), 0.0);
    }

    #[test]
    fn config_reaction_sections_default_when_absent() {
        // Un genome sans [miasma]/[surcharge] doit parser et obtenir les défauts.
        let c = ElementConfig::parse_toml("always_on = true");
        assert_eq!(c.miasma, MiasmaParams::default());
        assert_eq!(c.surcharge, SurchargeParams::default());
    }

    // ── Affinité élément→couche (story-642 P0-4 Inc.1) ──

    #[test]
    fn affinity_maps_each_element_to_its_favored_layer() {
        let c = ElementConfig::default();
        let fire = c.affinity_for(Element::Fire);
        assert!(fire.life_mult > 1.0 && fire.shield_mult < 1.0, "Feu → fort Vie, faible Bouclier");
        let shock = c.affinity_for(Element::Shock);
        assert!(shock.shield_mult > 1.0, "Électrique → fort Bouclier");
        assert!(c.affinity_for(Element::ArmorPierce).armor_mult > 1.0, "Perforant → forte Armure");
        assert!(c.affinity_for(Element::Poison).armor_mult > 1.0, "Poison → forte Armure");
    }

    #[test]
    fn affinity_gene_converts_to_element_affinity() {
        let g = AffinityGene { shield: 0.75, armor: 0.75, life: 1.5 };
        assert_eq!(g.to_affinity(), ElementAffinity::new(0.75, 0.75, 1.5));
    }

    #[test]
    fn elemental_bonus_matches_formula_and_vuln() {
        // base 16, matchup 1.3, shred 1.0 → 16×0.3 = 4.8. Vuln 1.1 → 5.28.
        assert!((elemental_bonus(16.0, 1.3, 1.0, 1.0) - 4.8).abs() < 1e-3);
        assert!((elemental_bonus(16.0, 1.3, 1.0, 1.1) - 5.28).abs() < 1e-3);
        // matchup ≤ 1 (neutre/faible) → aucun bonus (clamp ≥ 0).
        assert_eq!(elemental_bonus(20.0, 1.0, 1.0, 1.0), 0.0);
    }

    #[test]
    fn resolve_leak_executes_armor_pierce_under_threshold() {
        // Tank 120 PV (seuil 0.25×120=30), leak 45 → survit 75... non : leak 95 → 25 < 30 → exec.
        let (hp, exec) = resolve_leak_to_health(Element::ArmorPierce, 120.0, 120.0, 95.0, 0.25);
        assert!(exec);
        assert_eq!(hp, 0.0);
        // Feu même leak : pas d'exécution (seul ArmorPierce exécute).
        let (hp2, exec2) = resolve_leak_to_health(Element::Fire, 120.0, 120.0, 95.0, 0.25);
        assert!(!exec2);
        assert!((hp2 - 25.0).abs() < 1e-4);
    }

    #[test]
    fn resolve_target_hit_composes_bonus_and_leak() {
        // Sanity : le wrapper sans couche = bonus brut appliqué à la Vie.
        let bonus = elemental_bonus(16.0, 1.3, 1.0, 1.0);
        let (via_wrapper, _) = resolve_target_hit(Element::Fire, 30.0, 35.0, 16.0, 1.3, 1.0, 1.0, 0.25);
        let (via_split, _) = resolve_leak_to_health(Element::Fire, 30.0, 35.0, bonus, 0.25);
        assert_eq!(via_wrapper, via_split);
    }

    #[test]
    fn config_affinity_defaults_when_section_absent() {
        let c = ElementConfig::parse_toml("always_on = true");
        assert_eq!(c.affinity, AffinityTable::default());
    }

    #[test]
    fn armor_pierce_execute_blocked_by_full_shield() {
        // Cible 100 PV (max 120, seuil exécution 30), bouclier plein 200. Bonus
        // perforant brut 75 : en DIRECT il ramènerait à 25 (<30 → exécution). Routé
        // via le bouclier plein (perforant ×0.75 vs bouclier) → leak 0 → PAS
        // d'exécution, la Vie ne bouge pas (« on n'exécute pas à travers un bouclier »).
        let mut dl = DefenseLayer::new(200.0, 0.0, 20.0, 3.0);
        let aff = ElementConfig::default().affinity_for(Element::ArmorPierce);
        let leak = dl.absorb_elemental(75.0, &aff);
        assert_eq!(leak, 0.0, "bonus perforant absorbé par le bouclier plein");
        let (hp, exec) = resolve_leak_to_health(Element::ArmorPierce, 100.0, 120.0, leak, 0.25);
        assert!(!exec, "le bonus n'atteint pas la Vie → pas d'exécution");
        assert_eq!(hp, 100.0);
        // Contraste : le même bonus en DIRECT (cible sans couche) exécute bien.
        let (_, exec_direct) = resolve_leak_to_health(Element::ArmorPierce, 100.0, 120.0, 75.0, 0.25);
        assert!(exec_direct, "sans bouclier, 75 ramène sous le seuil → exécution");
    }

    // ── Routage bursts/arc/Miasma via DefenseLayer (story-642 P0-4 Inc.2) ──

    #[test]
    fn reaction_damage_element_maps_to_source_element() {
        assert_eq!(ReactionKind::Combustion.damage_element(), Element::Fire);
        assert_eq!(ReactionKind::Miasma.damage_element(), Element::Poison);
        assert_eq!(ReactionKind::Surcharge.damage_element(), Element::Shock);
    }

    #[test]
    fn burst_drains_shield_via_reaction_affinity() {
        // Même burst brut (40) sur un bouclier 100 : Surcharge (électrique ×1.5)
        // perce plus de bouclier que Combustion (feu ×0.75). Vérifie que le routage
        // burst→couche utilise l'affinité de l'élément de la réaction.
        let c = ElementConfig::default();
        let mut sh_surcharge = DefenseLayer::new(100.0, 0.0, 20.0, 3.0);
        let mut sh_combustion = DefenseLayer::new(100.0, 0.0, 20.0, 3.0);
        let leak_s = sh_surcharge
            .absorb_elemental(40.0, &c.affinity_for(ReactionKind::Surcharge.damage_element()));
        let leak_c = sh_combustion
            .absorb_elemental(40.0, &c.affinity_for(ReactionKind::Combustion.damage_element()));
        assert_eq!(leak_s, 0.0);
        assert_eq!(leak_c, 0.0);
        assert!((sh_surcharge.shield - 40.0).abs() < 1e-3, "40×1.5=60 retirés → 40");
        assert!((sh_combustion.shield - 70.0).abs() < 1e-3, "40×0.75=30 retirés → 70");
        assert!(sh_surcharge.shield < sh_combustion.shield, "surcharge perce mieux");
    }

    #[test]
    fn miasma_routes_via_poison_armor_affinity() {
        // Miasma = affinité poison (forte armure ×1.5). Tick de 20 sur armure 40 →
        // 20×1.5 = 30 retirés → armure 10, 0 fuite.
        let c = ElementConfig::default();
        let aff = c.affinity_for(ReactionKind::Miasma.damage_element());
        assert!(aff.armor_mult > 1.0, "Miasma → affinité forte armure");
        let mut armored = DefenseLayer::new(0.0, 40.0, 20.0, 3.0);
        let leak = armored.absorb_elemental(20.0, &aff);
        assert_eq!(leak, 0.0);
        assert!((armored.armor - 10.0).abs() < 1e-3);
    }

    // ── VFX (story-588) ──

    #[test]
    fn element_idx_is_stable_and_distinct() {
        let idx: Vec<usize> = [
            Element::Fire,
            Element::Poison,
            Element::Shock,
            Element::ArmorPierce,
        ]
        .iter()
        .map(|e| e.idx())
        .collect();
        assert_eq!(idx, vec![0, 1, 2, 3], "idx doit indexer [0..4] sans collision");
    }

    #[test]
    fn element_rgb_maps_to_signature_colors() {
        let v = VfxParams::default();
        assert_eq!(Element::Fire.rgb(&v), v.fire_rgb);
        assert_eq!(Element::Poison.rgb(&v), v.poison_rgb);
        assert_eq!(Element::Shock.rgb(&v), v.shock_rgb);
        assert_eq!(Element::ArmorPierce.rgb(&v), v.armor_pierce_rgb);
    }

    #[test]
    fn vfx_default_is_enabled_and_sane() {
        let v = VfxParams::default();
        assert!(v.enabled);
        assert!(v.impact_scale > 0.0 && v.impact_ttl > 0.0);
        assert!(v.arc_scale > 1.0, "l'arc électrique doit être plus gros");
        assert!(v.dot_pulse_period > 0.0);
    }

    #[test]
    fn config_vfx_field_defaults_when_section_absent() {
        // Un TOML Phase A (sans [vfx]) doit parser et obtenir le VfxParams par défaut.
        let toml = r#"
always_on = true
[mapping]
modern_ar = "shock"
assault_rifle = "fire"
shotgun = "armor_pierce"
rocket_launcher = "poison"
[matchup.fire]
tank = 1.0
runner = 1.3
sniper = 1.1
boss = 1.0
[matchup.poison]
tank = 1.4
runner = 1.0
sniper = 1.1
boss = 1.2
[matchup.shock]
tank = 1.1
runner = 1.4
sniper = 1.2
boss = 1.0
[matchup.armor_pierce]
tank = 2.0
runner = 1.0
sniper = 1.3
boss = 1.5
[burn]
dps = 8.0
duration = 3.0
[poison]
dps_per_stack = 4.0
duration = 4.0
max_stacks = 5
shred_per_stack = 0.04
[aoe]
radius = 3.5
damage_factor = 0.5
[execute]
hp_ratio_threshold = 0.25
"#;
        let c = ElementConfig::parse_toml(toml);
        assert_eq!(c.vfx, VfxParams::default(), "section [vfx] absente → default");
    }
}
