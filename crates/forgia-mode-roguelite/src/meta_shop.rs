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
use forgia_ui_lib::style::{
    C_HP_HIGH, C_TEXT_MUTED, FORGE_AME, FORGE_OR, FORGE_PANEL, FORGE_TEAL, HAIR_GOLD_STRONG,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
    /// Story-680 cran 2 — **la face alternative**, modèle Miroir de Nuit d'Hades.
    ///
    /// Chaque ligne a deux visages et **un seul est actif**. Les rangs achetés
    /// sont conservés en permutant : on choisit ce qu'ils FONT, pas combien on
    /// en a. Hades permet de repermuter librement — pas de punition à changer
    /// d'avis, sinon le choix devient une peur et plus une décision.
    ///
    /// `None` = ligne sans alternative (rétrocompatible).
    pub alt: Option<MetaFace>,
}

/// Une face d'amélioration : un nom, une description, un effet.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaFace {
    pub name: String,
    pub desc: String,
    pub effect: MetaEffect,
}

impl MetaUpgrade {
    /// La face active : `face == 0` → face principale, `1` → alternative.
    ///
    /// Une ligne sans alternative renvoie toujours sa face principale, quelle
    /// que soit la valeur enregistrée — un save corrompu ne peut pas produire
    /// une ligne sans effet.
    pub fn face(&self, face: u8) -> (&str, &str, MetaEffect) {
        match (face, self.alt.as_ref()) {
            (0, _) | (_, None) => (&self.name, &self.desc, self.effect),
            (_, Some(a)) => (&a.name, &a.desc, a.effect),
        }
    }

    pub fn has_alt(&self) -> bool {
        self.alt.is_some()
    }
}

impl MetaUpgrade {
    pub fn max_rank(&self) -> u32 {
        self.costs.len() as u32
    }

    /// Gain effectif au rang `rank` — **rendements décroissants avec plancher**
    /// (story-680, cran 3, modèle Pom of Power d'Hades).
    ///
    /// Avant : `amount × rank`, cinq fois le même gain. Les coûts croissaient
    /// (`25 / 50 / 85 / 130 / 190`) mais pas le gain — le rang 5 coûtait 7,6×
    /// le rang 1 pour la même chose. Aucun arbitrage possible : soit on montait
    /// tout, soit rien.
    ///
    /// Maintenant chaque rang supplémentaire rapporte moins, **sans jamais
    /// tomber à zéro** (Hades : « la chute a un plancher »). Résultat : le
    /// choix devient « j'approfondis cette ligne, ou j'en ouvre une autre ? ».
    pub fn total_amount(&self, amount: f32, rank: u32) -> f32 {
        (1..=rank)
            .map(|r| amount * Self::rank_falloff(r))
            .sum::<f32>()
    }

    /// Facteur du `r`-ième rang. 1,00 · 0,75 · 0,58 · 0,47 · 0,40 → plancher.
    ///
    /// Le premier rang vaut plein pot ; le cinquième vaut encore 40 %, jamais
    /// moins. Un plancher trop bas ferait des rangs décoratifs, et un rang
    /// décoratif qu'on fait payer est pire qu'un rang absent.
    fn rank_falloff(r: u32) -> f32 {
        const FLOOR: f32 = 0.40;
        if r <= 1 {
            return 1.0;
        }
        (1.0 / (r as f32).sqrt()).max(FLOOR)
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

/// Maîtrise d'arme : bonus par niveau + PLAFOND. Data-driven (`[mastery]` du genome).
///
/// Sans plafond, +4 % par run terminée (défaite comprise) est une progression
/// permanente non bornée : à la 25e run avec la même arme elle vaut +96 %, ce qui
/// annule le scaling ennemi (+35 % PV/salle) et rend la courbe de difficulté
/// intenable. Le plafond est donc un invariant de balance, pas un confort.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MasteryConfig {
    /// Niveau maximum atteignable par arme (1 = pas de progression).
    pub max_level: u32,
    /// Bonus de dégâts (fraction) par niveau AU-DESSUS de 1.
    pub damage_per_level: f32,
}

impl Default for MasteryConfig {
    /// Miroir EXACT de `[mastery]` dans le genome — 6 niveaux × 4 % = +20 % au cap.
    fn default() -> Self {
        Self {
            max_level: Self::DEFAULT_MAX_LEVEL,
            damage_per_level: Self::DEFAULT_DAMAGE_PER_LEVEL,
        }
    }
}

impl MasteryConfig {
    pub const DEFAULT_MAX_LEVEL: u32 = 6;
    pub const DEFAULT_DAMAGE_PER_LEVEL: f32 = 0.04;
    /// Bornes de sécurité créateur (`genome-code.md` : « chaque gene a une valeur
    /// min/max raisonnable » ; `creator-simplicity.md` : « un créateur ne doit jamais
    /// casser son jeu en bougeant un slider »). Le piège visé est l'erreur d'UNITÉ :
    /// `damage_per_level = 10` (lu « 10 % ») donnerait ×51 de dégâts permanents.
    const MAX_LEVEL_BOUNDS: (u32, u32) = (1, 20);
    const DAMAGE_PER_LEVEL_BOUNDS: (f32, f32) = (0.0, 0.25);

    /// Construit depuis le genome en BORNANT, et en le DISANT (une correction
    /// silencieuse est aussi opaque que le défaut qu'elle répare).
    pub fn from_genome(max_level: u32, damage_per_level: f32) -> Self {
        let (lo, hi) = Self::MAX_LEVEL_BOUNDS;
        let (dlo, dhi) = Self::DAMAGE_PER_LEVEL_BOUNDS;
        let clamped = Self {
            max_level: max_level.clamp(lo, hi),
            // `clamp` panique sur NaN → on le neutralise d'abord.
            damage_per_level: if damage_per_level.is_finite() {
                damage_per_level.clamp(dlo, dhi)
            } else {
                Self::DEFAULT_DAMAGE_PER_LEVEL
            },
        };
        if clamped.max_level != max_level {
            warn!(
                "[meta-shop] genome [mastery] max_level={max_level} hors bornes {lo}..={hi} → {}",
                clamped.max_level
            );
        }
        if clamped.damage_per_level != damage_per_level {
            warn!(
                "[meta-shop] genome [mastery] damage_per_level={damage_per_level} hors bornes \
                 {dlo}..={dhi} (c'est une FRACTION : 0.04 = +4 %) → {}",
                clamped.damage_per_level
            );
        }
        clamped
    }

    /// Multiplicateur de dégâts d'une arme au niveau donné, CLAMPÉ AU PLAFOND À LA
    /// LECTURE — jamais à l'écriture, pour qu'un save antérieur au plafond (ou un
    /// plafond relevé plus tard en genome) ne perde jamais sa progression réelle.
    /// PUR — testable sans App.
    pub fn damage_mul(&self, level: u32) -> f32 {
        1.0 + self.effective_level(level).saturating_sub(1) as f32 * self.damage_per_level
    }

    /// Niveau EFFECTIF (= stocké, borné au plafond). C'est ce que l'UI doit afficher :
    /// sinon un save legacy montre « Niveau 13/6 », un compteur illisible.
    pub fn effective_level(&self, stored: u32) -> u32 {
        stored.min(self.max_level.max(1))
    }

    /// Le bonus de maîtrise **TOTAL**, toutes armes confondues.
    ///
    /// ## Pourquoi ce n'est plus « l'arme équipée » (2026-08-04)
    ///
    /// Le bonus valait celui de l'arme en main. Passer de Pépin (niveau 6, ×1,20)
    /// à Boucherie (niveau 2, ×1,04) coûtait **13 % de dégâts** — quand la table
    /// de matchup élémentaire, elle, rend au mieux ×2,0 et seulement contre le
    /// bon archétype. Le jeu PUNISSAIT donc précisément le geste qu'il prétend
    /// récompenser : changer d'arme selon l'ennemi.
    ///
    /// Maintenant : la somme des niveaux acquis, plafonnée au **même total**.
    /// Monter une arme à fond ou quatre armes à un quart donne le même bonus, et
    /// changer d'arme en plein combat ne coûte plus rien — le bonus ne dépend
    /// plus de ce qu'on tient.
    ///
    /// Le plafond est inchangé (`max_level - 1` niveaux × `damage_per_level`),
    /// donc la courbe de difficulté du Livre n'est pas touchée : c'est la
    /// RÉPARTITION qui devient libre, pas le total.
    ///
    /// Effet de bord heureux : la désynchronisation corrigée plus tôt (maîtrise
    /// figée sur l'arme du wizard) devient **impossible** — il n'y a plus d'arme
    /// courante dans ce calcul.
    ///
    /// PUR — testable sans App.
    pub fn total_damage_mul(&self, weapon_levels: &HashMap<String, u32>) -> f32 {
        let acquis: u32 = weapon_levels
            .values()
            .map(|lvl| self.effective_level(*lvl).saturating_sub(1))
            .sum();
        let plafond = self.max_level.saturating_sub(1);
        1.0 + acquis.min(plafond) as f32 * self.damage_per_level
    }
}

#[derive(Resource, Clone, Debug)]
pub struct MetaShopCatalogue {
    pub upgrades: Vec<MetaUpgrade>,
    /// Armes déblocables en Âmes (story-613).
    pub weapon_unlocks: Vec<WeaponUnlock>,
    /// Paliers d'atouts (boons) déblocables en Âmes (story-616) — réutilise key/name/cost.
    pub boon_tier_unlocks: Vec<WeaponUnlock>,
    /// Maîtrise d'arme : bonus/niveau + plafond (genome `[mastery]`).
    pub mastery: MasteryConfig,
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
                    // Encaisser plus, ou encaisser mieux ? Les PV rendent les
                    // gros coups survivables, la réduction rend l'attrition
                    // supportable. Deux styles, un seul actif.
                    alt: Some(MetaFace {
                        name: "Cuirasse".into(),
                        desc: "+4 % de réduction de dégâts (au lieu des PV)".into(),
                        effect: MetaEffect::DamageReduction(0.04),
                    }),
                },
                MetaUpgrade {
                    id: "damage".into(),
                    name: "Puissance".into(),
                    desc: "+8% dégâts".into(),
                    effect: MetaEffect::DamageMul(0.08),
                    costs: vec![25, 50, 85, 130, 190],
                    // Frapper plus fort, ou partir plus riche ? La puissance
                    // paie tout de suite, l'or paie en Trempe et en achats —
                    // plus fort, mais plus tard.
                    alt: Some(MetaFace {
                        name: "Fortune".into(),
                        desc: "+45 Or de départ (au lieu des dégâts)".into(),
                        effect: MetaEffect::StartGold(45),
                    }),
                },
                MetaUpgrade {
                    id: "armor".into(),
                    name: "Armure".into(),
                    desc: "+5% réduction de dégâts".into(),
                    effect: MetaEffect::DamageReduction(0.05),
                    costs: vec![30, 60, 100, 150],
                    // L'armure protège de tout un peu ; les PV donnent une
                    // marge brute que rien ne perce.
                    alt: Some(MetaFace {
                        name: "Endurance".into(),
                        desc: "+12 PV max (au lieu de la réduction)".into(),
                        effect: MetaEffect::MaxHp(12.0),
                    }),
                },
                MetaUpgrade {
                    id: "gold".into(),
                    name: "Pactole".into(),
                    desc: "+50 Or de départ".into(),
                    effect: MetaEffect::StartGold(50),
                    costs: vec![15, 35, 60],
                    // Commencer riche, ou commencer fort ?
                    alt: Some(MetaFace {
                        name: "Étincelle".into(),
                        desc: "+6 % de dégâts (au lieu de l'Or)".into(),
                        effect: MetaEffect::DamageMul(0.06),
                    }),
                },
            ],
            // Miroir EXACT des [[weapon_unlocks]] du genome (story-613).
            weapon_unlocks: vec![
                WeaponUnlock {
                    key: "bourrasque".into(),
                    name: "Bourrasque".into(),
                    cost: 60,
                },
                WeaponUnlock {
                    key: "madame_lenoir".into(),
                    name: "Madame Lenoir".into(),
                    cost: 150,
                },
                WeaponUnlock {
                    key: "boucherie".into(),
                    name: "Boucherie".into(),
                    cost: 250,
                },
            ],
            // Miroir EXACT des [[boon_tier_unlocks]] du genome (story-616).
            boon_tier_unlocks: vec![
                WeaponUnlock {
                    key: "uncommon".into(),
                    name: "Atouts Peu communs".into(),
                    cost: 80,
                },
                WeaponUnlock {
                    key: "rare".into(),
                    name: "Atouts Rares".into(),
                    cost: 200,
                },
                WeaponUnlock {
                    key: "legendary".into(),
                    name: "Atouts Légendaires".into(),
                    cost: 400,
                },
            ],
            // Miroir EXACT de [mastery] du genome.
            mastery: MasteryConfig::default(),
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
    /// Story-680 cran 2 — face alternative exclusive. Absente = ligne simple.
    #[serde(default)]
    alt: Option<MetaFaceToml>,
}

#[derive(Deserialize)]
struct MetaFaceToml {
    name: String,
    desc: String,
    effect: String,
    amount: f32,
}

#[derive(Deserialize)]
struct WeaponUnlockToml {
    key: String,
    name: String,
    cost: u32,
}

/// Les deux champs ont un `default` : une section `[mastery]` PARTIELLE ne doit pas
/// faire échouer serde. Sans ça, `toml::from_str` échoue sur le DOCUMENT ENTIER et
/// tout le catalogue (upgrades, prix d'armes, prix de paliers) retombe en silence
/// sur le miroir Rust — indiscernable d'un chargement réussi.
#[derive(Deserialize)]
struct MasteryToml {
    #[serde(default = "default_mastery_max_level")]
    max_level: u32,
    #[serde(default = "default_mastery_damage_per_level")]
    damage_per_level: f32,
}

fn default_mastery_max_level() -> u32 {
    MasteryConfig::DEFAULT_MAX_LEVEL
}

fn default_mastery_damage_per_level() -> f32 {
    MasteryConfig::DEFAULT_DAMAGE_PER_LEVEL
}

#[derive(Deserialize)]
struct CatalogueToml {
    #[serde(default)]
    upgrades: Vec<UpgradeToml>,
    #[serde(default)]
    weapon_unlocks: Vec<WeaponUnlockToml>,
    #[serde(default)]
    boon_tier_unlocks: Vec<WeaponUnlockToml>,
    /// Absent → `MasteryConfig::default()` (miroir Rust).
    #[serde(default)]
    mastery: Option<MasteryToml>,
}

impl MetaShopCatalogue {
    /// Pur — testable. Fallback `Default` si parse KO ou liste vide.
    pub fn parse_toml(content: &str) -> Self {
        let parsed = match toml::from_str::<CatalogueToml>(content) {
            Ok(p) => p,
            Err(e) => {
                // Le fallback muet était indiscernable d'un chargement réussi : le
                // miroir Rust a lui aussi 4 upgrades, donc le log de succès mentait.
                warn!("[meta-shop] genome illisible ({e}) — MIROIR RUST utilisé (le TOML est ignoré en entier)");
                return Self::default();
            }
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
                    // Une face alternative dont l'effet est inconnu est
                    // SILENCIEUSEMENT ignorée serait un piège : on la rejette
                    // en le disant, sinon la ligne perdrait sa moitié sans que
                    // personne ne le sache.
                    alt: u.alt.and_then(|a| match MetaEffect::from_key(&a.effect, a.amount) {
                        Some(effect) => Some(MetaFace {
                            name: a.name,
                            desc: a.desc,
                            effect,
                        }),
                        None => {
                            warn!(
                                "[meta-shop] face alternative '{}' : effet '{}' inconnu — face IGNORÉE",
                                a.name, a.effect
                            );
                            None
                        }
                    }),
                })
            })
            .collect();
        let weapon_unlocks: Vec<WeaponUnlock> = parsed
            .weapon_unlocks
            .into_iter()
            .map(|w| WeaponUnlock {
                key: w.key,
                name: w.name,
                cost: w.cost,
            })
            .collect();
        let boon_tier_unlocks: Vec<WeaponUnlock> = parsed
            .boon_tier_unlocks
            .into_iter()
            .map(|w| WeaponUnlock {
                key: w.key,
                name: w.name,
                cost: w.cost,
            })
            .collect();
        // Fallback PAR CHAMP : un genome partiel ne perd pas les autres listes.
        let d = Self::default();
        Self {
            upgrades: if upgrades.is_empty() {
                d.upgrades
            } else {
                upgrades
            },
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
            // Bornage + log si la valeur du créateur a dû être corrigée.
            mastery: match parsed.mastery {
                Some(m) => MasteryConfig::from_genome(m.max_level, m.damage_per_level),
                None => d.mastery,
            },
        }
    }

    fn load_or_default() -> Self {
        match std::fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(e) => {
                // Chemin RELATIF au CWD : en build distribué le fichier n'est pas
                // trouvé. Le fallback est correct (miroir Rust identique) mais il
                // doit se VOIR, sinon un tuning de genome semble « ne rien faire ».
                warn!("[meta-shop] genome {GENOME_PATH} illisible ({e}) — miroir Rust utilisé");
                Self::default()
            }
        }
    }
}

// ─── Save disque (source de vérité des Âmes accumulées) ─────────────────────

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct MetaShopSave {
    pub version: u32,
    pub souls_total: u32,
    /// Story-678 — les ÉCLATS, monnaie **cosmétique**, séparée des Âmes.
    ///
    /// Décision du 2026-08-06 : un cosmétique payé en Âmes est un rang
    /// d'Enclume non acheté, et la courbe de puissance est en cours de
    /// recalibrage. Deux monnaies = deux lectures qui ne se brouillent pas.
    ///
    /// Contrairement aux Âmes, il n'y a PAS de miroir vif : les Éclats ne se
    /// dépensent qu'au menu, jamais en run. Une seule vérité, donc aucun risque
    /// de désynchronisation (c'est ce miroir qui a produit « l'achat sans
    /// effet » côté Âmes).
    #[serde(default)]
    pub shards_total: u32,
    pub ranks: HashMap<String, u32>,
    /// Story-680 cran 2 — face active par ligne (0 = principale, 1 = alternative).
    /// Absente = face principale, donc les saves d'avant restent valides.
    #[serde(default)]
    pub faces: HashMap<String, u8>,
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
    /// R3.3 (story-645) — meilleure victoire (s). 0.0 = aucune victoire encore.
    #[serde(default)]
    pub best_victory_secs: f32,
    /// R3.3 — total de runs jouées (Defeat + Victory).
    #[serde(default)]
    pub runs_played: u32,
    /// R3.3 — total de victoires.
    #[serde(default)]
    pub victories: u32,
    /// 2026-08-04 — **chapitres terminés**, 1-indexé (0 = aucun).
    ///
    /// Le Livre s'ouvre chapitre par chapitre : on ne peut entrer dans le
    /// suivant qu'après avoir battu le boss du précédent. `#[serde(default)]` :
    /// les sauvegardes antérieures repartent à 0 sans migration.
    #[serde(default)]
    pub chapters_cleared: u32,
    /// Story-678 Phase 3 — résumé de la DERNIÈRE run, affiché par le tableau
    /// de bord de l'accueil. `None` = aucune run terminée depuis la feature.
    #[serde(default)]
    pub last_run: Option<LastRunSummary>,
    /// Story-678 Phase 4 — valeur de `chapters_cleared` la dernière fois que la
    /// page Livre a été OUVERTE. `chapters_cleared > seen` = pastille sur l'onglet.
    #[serde(default)]
    pub seen_chapters_cleared: u32,
}

/// Story-678 Phase 4 — y a-t-il AU MOINS un achat possible à l'Enclume avec
/// `souls` ? Pilote la pastille de l'onglet — elle doit dire vrai, jamais
/// « peut-être » (un badge menteur éduque le joueur à l'ignorer).
pub fn enclume_affordable(cat: &MetaShopCatalogue, save: &MetaShopSave, souls: u32) -> bool {
    let upgrade = cat
        .upgrades
        .iter()
        .any(|u| u.cost_for_next(save.rank(&u.id)).is_some_and(|c| c <= souls));
    let weapon = cat
        .weapon_unlocks
        .iter()
        .any(|w| !save.unlocked_weapons.iter().any(|k| k == &w.key) && w.cost <= souls);
    let tier = cat
        .boon_tier_unlocks
        .iter()
        .any(|t| !save.unlocked_boon_tiers.iter().any(|k| k == &t.key) && t.cost <= souls);
    upgrade || weapon || tier
}

/// Ce que l'accueil raconte de la dernière run (bandeau « DERNIÈRE RUN »).
/// Écrit UNIQUEMENT par [`sys_record_run_stats`] — une seule définition de la
/// grandeur, aux deux sorties de run (Defeat ET Victory).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct LastRunSummary {
    /// Chapitre joué (1-indexé).
    pub chapter: u32,
    pub victory: bool,
    /// Round canonique atteint (`RogueliteWave::round`, 1..=10).
    pub rounds_reached: u32,
    pub duration_secs: f32,
    /// Âmes rapportées par cette run (`MetaSouls::earned_run` au moment du gel).
    pub souls_earned: u32,
    /// La run a battu le record de meilleure victoire.
    pub new_best: bool,
}

/// Nombre de chapitres d'un Livre.
///
/// 10 pour l'instant (décision du 2026-08-04). C'est la longueur de la campagne,
/// pas celle d'une run : une run EST un chapitre.
pub const CHAPTERS_PER_BOOK: u32 = 10;

/// Le chapitre `chapter` (1-indexé) est-il jouable ?
///
/// Règle : on peut rejouer tout ce qu'on a déjà fini, plus **exactement un**
/// chapitre d'avance — le premier non battu. Pas de saut.
///
/// PUR — testable sans App.
pub fn chapter_unlocked(chapter: u32, chapters_cleared: u32) -> bool {
    chapter >= 1 && chapter <= CHAPTERS_PER_BOOK && chapter <= chapters_cleared + 1
}

/// Le chapitre le plus avancé qu'on puisse lancer — celui que le Lobby propose
/// par défaut.
pub fn furthest_playable_chapter(chapters_cleared: u32) -> u32 {
    (chapters_cleared + 1).min(CHAPTERS_PER_BOOK)
}

/// Enregistre qu'un chapitre vient d'être battu.
///
/// **Ne recule jamais** : rejouer le chapitre 3 alors qu'on a fini le 7 ne doit
/// pas re-verrouiller les quatre suivants. C'est un maximum, pas un curseur.
///
/// PUR — testable sans App.
pub fn record_chapter_cleared(save: &mut MetaShopSave, chapter: u32) {
    if chapter == 0 || chapter > CHAPTERS_PER_BOOK {
        return;
    }
    save.chapters_cleared = save.chapters_cleared.max(chapter);
}

/// Le chapitre choisi au Lobby pour la run à venir (1-indexé).
///
/// Borné à ce que la sauvegarde autorise : une valeur héritée d'une session
/// précédente ne doit pas ouvrir un chapitre verrouillé.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedChapter(pub u32);

impl Default for SelectedChapter {
    fn default() -> Self {
        Self(1)
    }
}

impl SelectedChapter {
    /// Ramène la sélection dans ce que la progression autorise.
    pub fn clamped(self, chapters_cleared: u32) -> u32 {
        self.0.clamp(1, furthest_playable_chapter(chapters_cleared))
    }
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
            shards_total: 0,
            ranks: HashMap::new(),
            faces: HashMap::new(),
            unlocked_weapons: default_unlocked_weapons(),
            unlocked_boon_tiers: Vec::new(),
            weapon_levels: HashMap::new(),
            best_victory_secs: 0.0,
            runs_played: 0,
            victories: 0,
            chapters_cleared: 0,
            last_run: None,
            seen_chapters_cleared: 0,
        }
    }
}

#[cfg(test)]
mod chapter_tests {
    use super::*;

    /// La règle de ton Livre : on rejoue ce qu'on a fini, plus UN chapitre
    /// d'avance. Jamais de saut.
    #[test]
    fn a_fresh_save_opens_only_the_first_chapter() {
        assert!(chapter_unlocked(1, 0));
        assert!(!chapter_unlocked(2, 0), "on ne saute pas le premier boss");
        assert_eq!(furthest_playable_chapter(0), 1);
    }

    #[test]
    fn beating_a_boss_opens_exactly_the_next_chapter() {
        let mut save = MetaShopSave::default();
        record_chapter_cleared(&mut save, 1);
        assert!(chapter_unlocked(2, save.chapters_cleared));
        assert!(!chapter_unlocked(3, save.chapters_cleared), "un seul cran");
        assert_eq!(furthest_playable_chapter(save.chapters_cleared), 2);
    }

    /// Rejouer un chapitre déjà battu ne doit RIEN re-verrouiller — sinon on
    /// punirait le joueur de revenir farmer, ce qui est exactement l'usage.
    #[test]
    fn replaying_an_old_chapter_never_takes_progress_away() {
        let mut save = MetaShopSave::default();
        record_chapter_cleared(&mut save, 7);
        record_chapter_cleared(&mut save, 3);
        assert_eq!(save.chapters_cleared, 7);
        assert!(chapter_unlocked(8, save.chapters_cleared));
    }

    #[test]
    fn the_book_stops_at_its_last_chapter() {
        let mut save = MetaShopSave::default();
        record_chapter_cleared(&mut save, CHAPTERS_PER_BOOK);
        assert!(chapter_unlocked(CHAPTERS_PER_BOOK, save.chapters_cleared));
        assert!(!chapter_unlocked(
            CHAPTERS_PER_BOOK + 1,
            save.chapters_cleared
        ));
        assert_eq!(
            furthest_playable_chapter(save.chapters_cleared),
            CHAPTERS_PER_BOOK
        );
        // Un chapitre hors Livre ne s'enregistre pas.
        record_chapter_cleared(&mut save, 99);
        assert_eq!(save.chapters_cleared, CHAPTERS_PER_BOOK);
    }

    /// Une sélection héritée d'une session précédente ne doit pas ouvrir un
    /// chapitre verrouillé — le Lobby borne, il ne fait pas confiance.
    #[test]
    fn a_stale_selection_cannot_unlock_anything() {
        assert_eq!(SelectedChapter(9).clamped(0), 1);
        assert_eq!(SelectedChapter(9).clamped(4), 5);
        assert_eq!(SelectedChapter(0).clamped(4), 1, "pas de chapitre 0");
    }
}

/// PUR (testable) — enregistre une fin de run dans le save : compteurs + record.
/// Retourne `true` si `secs` établit un NOUVEAU record de victoire.
pub fn record_run_result(save: &mut MetaShopSave, secs: f32, victory: bool) -> bool {
    save.runs_played = save.runs_played.saturating_add(1);
    if !victory {
        return false;
    }
    save.victories = save.victories.saturating_add(1);
    if save.best_victory_secs <= 0.0 || secs < save.best_victory_secs {
        save.best_victory_secs = secs;
        return true;
    }
    false
}

/// R3.3 — stats de la run qui vient de se terminer, lues par les overlays
/// Victory/Defeat (chrono + « NOUVEAU RECORD »). Écrite par [`sys_record_run_stats`].
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LastRunStats {
    pub secs: f32,
    pub new_best: bool,
}

/// OnEnter(Defeat|Victory) — fige le chrono de la run dans les stats persistantes
/// (runs jouées, victoires, meilleure victoire) + expose `LastRunStats` aux overlays.
/// Ordonné `.before(sys_flush_meta_save)` pour que le flush écrive les compteurs à jour.
pub fn sys_record_run_stats(
    run_state: Res<State<RunState>>,
    timer: Res<crate::run::RunTimer>,
    // 2026-08-04 — quel chapitre vient d'être joué : c'est LUI qu'on débloque.
    chapter: Res<SelectedChapter>,
    mut save: ResMut<MetaShopSave>,
    mut last: ResMut<LastRunStats>,
    // Story-678 Phase 3 — le round atteint et les Âmes de la run, pour le
    // bandeau de l'accueil. Optionnels : le gel doit survivre à leur absence.
    wave: Option<Res<crate::waves::RogueliteWave>>,
    graph_cfg: Option<Res<forgia_stage::graph::RunGraphConfig>>,
    meta: Option<Res<crate::run::MetaSouls>>,
    // Story-678 — le barème des Éclats (monnaie cosmétique).
    cosmetics: Option<Res<crate::cosmetics::CosmeticsConfig>>,
) {
    let victory = matches!(run_state.get(), RunState::Victory);
    last.secs = timer.secs;
    last.new_best = record_run_result(&mut save, timer.secs, victory);
    // Une victoire ouvre le chapitre suivant. On enregistre le chapitre JOUÉ,
    // pas « le suivant » : `record_chapter_cleared` prend un maximum, donc
    // rejouer un chapitre déjà battu ne re-verrouille rien.
    if victory {
        let joue = chapter.clamped(save.chapters_cleared);
        record_chapter_cleared(&mut save, joue);
        info!(
            "[meta-shop] CHAPITRE {joue} terminé — {} / {CHAPTERS_PER_BOOK} ouverts",
            furthest_playable_chapter(save.chapters_cleared)
        );
    }
    info!(
        "[meta-shop] run #{} — {} en {:.0}s{} (victoires {}, record {:.0}s)",
        save.runs_played,
        if victory { "VICTOIRE" } else { "défaite" },
        timer.secs,
        if last.new_best {
            " — NOUVEAU RECORD"
        } else {
            ""
        },
        save.victories,
        save.best_victory_secs,
    );
    // Story-678 Phase 3 — le bandeau « DERNIÈRE RUN » de l'accueil. Gelé ICI et
    // nulle part ailleurs : même source que les compteurs, mêmes deux sorties.
    let rounds_reached = wave
        .as_deref()
        .zip(graph_cfg.as_deref())
        .map(|(w, g)| w.round(g.waves_per_stage))
        .unwrap_or(0);
    save.last_run = Some(LastRunSummary {
        chapter: chapter.clamped(save.chapters_cleared),
        victory,
        rounds_reached,
        duration_secs: timer.secs,
        souls_earned: meta.as_deref().map(|m| m.earned_run).unwrap_or(0),
        new_best: last.new_best,
    });

    // Story-678 — les ÉCLATS de la run. Ici et nulle part ailleurs : c'est la
    // seule fonction qui voit les DEUX sorties de run (mort et victoire) et qui
    // connaît déjà le round atteint. Un second site de gain divergerait.
    //
    // Le barème vit dans le génome des cosmétiques (`[shards]`) : il récompense
    // la PROFONDEUR, jamais le temps passé — un revenu au temps paierait le
    // farm passif.
    let gagnes = cosmetics
        .as_deref()
        .map(|c| c.shards)
        .unwrap_or_default()
        .earned(rounds_reached, victory);
    if gagnes > 0 {
        save.shards_total = save.shards_total.saturating_add(gagnes);
        info!(
            "[cosmetics] +{gagnes} Éclats (round {rounds_reached}{}) — total {}",
            if victory { ", chapitre bouclé" } else { "" },
            save.shards_total
        );
    }
}

impl MetaShopSave {
    pub fn rank(&self, id: &str) -> u32 {
        self.ranks.get(id).copied().unwrap_or(0)
    }

    /// Niveau de maîtrise de l'arme (P3) — défaut 1.
    pub fn weapon_level(&self, key: &str) -> u32 {
        self.weapon_levels.get(key).copied().unwrap_or(1)
    }

    /// +1 niveau de maîtrise (run terminée avec l'arme), tant que le plafond n'est
    /// pas atteint. Retourne le niveau après application.
    ///
    /// **NE FAIT JAMAIS DESCENDRE une valeur existante.** Le plafond n'existait pas
    /// avant story-668 : un save réel peut contenir `pepin = 13`. Un `.min(cap)` à
    /// l'écriture aurait réécrit 13 → 6 sur le disque à la fin de la run suivante,
    /// détruisant l'information définitivement — et pour rien, puisque le bonus est
    /// déjà borné à la LECTURE par `MasteryConfig::damage_mul`. Conserver la valeur
    /// stockée est aussi ce qui permet de relever `max_level` en genome plus tard
    /// sans avoir amputé les joueurs entre-temps.
    pub fn level_up_weapon(&mut self, key: &str, max_level: u32) -> u32 {
        let cap = max_level.max(1);
        let lvl = self.weapon_levels.entry(key.to_string()).or_insert(1);
        if *lvl < cap {
            *lvl = lvl.saturating_add(1);
        }
        *lvl
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
        crate::persist::save_dir().join(SAVE_FILE)
    }

    pub fn load_or_default() -> Self {
        crate::persist::load_toml_migrating(SAVE_FILE)
    }

    pub fn save(&self) {
        crate::persist::save_toml_atomic(&Self::save_path(), self, "meta-shop");
    }

    // ── Bonus cumulés (lus au run-start) ──
    //
    // Story-680 cran 3 — les rangs passent par `total_amount`, qui applique des
    // rendements DÉCROISSANTS avec plancher. Avant c'était `amount × rank` :
    // cinq fois le même gain pour des coûts qui, eux, croissaient de 7,6×.

    /// Story-680 cran 1 — **le niveau du joueur EST la somme de ses rangs.**
    ///
    /// Modèle Gunfire Reborn : « le système de niveau tourne autour de la
    /// dépense d'une monnaie en talents plutôt que de la collecte d'expérience ;
    /// un talent 2/5 donne 2 niveaux ». Le niveau ne peut donc plus être creux —
    /// il EST la somme des choix faits, par construction.
    ///
    /// Avant : un niveau gagné avec `40 + secondes de run`, des points de talent
    /// qui s'accumulaient et que RIEN ne dépensait, et un écran qui promettait
    /// des arbres inexistants.
    pub fn player_level(&self, cat: &MetaShopCatalogue) -> u32 {
        let ranks: u32 = cat.upgrades.iter().map(|u| self.rank(&u.id)).sum();
        let unlocks = self.unlocked_weapons.len().saturating_sub(1) as u32
            + self.unlocked_boon_tiers.len() as u32;
        // Niveau 1 au départ : personne ne commence « niveau 0 ».
        1 + ranks + unlocks
    }

    /// Rangs restants à acheter — « il te reste N choses à débloquer », qui est
    /// une information utile, contrairement à « N points en attente » quand rien
    /// ne les dépense.
    pub fn ranks_remaining(&self, cat: &MetaShopCatalogue) -> u32 {
        cat.upgrades
            .iter()
            .map(|u| u.max_rank().saturating_sub(self.rank(&u.id)))
            .sum()
    }
    /// Face active d'une ligne. Repli sur la face principale : un save qui
    /// pointe une face inexistante ne doit pas produire une ligne sans effet.
    pub fn face_of(&self, id: &str) -> u8 {
        self.faces.get(id).copied().unwrap_or(0)
    }

    /// Permute la face d'une ligne. Les RANGS SONT CONSERVÉS — on choisit ce
    /// qu'ils font, pas combien on en a. Hades laisse repermuter librement :
    /// punir le changement d'avis transforme le choix en peur.
    pub fn toggle_face(&mut self, id: &str) {
        let next = 1 - self.face_of(id);
        self.faces.insert(id.to_string(), next);
    }

    /// Somme d'un effet sur toutes les lignes, **face active seulement**.
    fn sum_effect(&self, cat: &MetaShopCatalogue, pick: impl Fn(MetaEffect) -> Option<f32>) -> f32 {
        cat.upgrades
            .iter()
            .filter_map(|u| {
                let (_, _, effect) = u.face(self.face_of(&u.id));
                pick(effect).map(|a| u.total_amount(a, self.rank(&u.id)))
            })
            .sum()
    }

    pub fn max_hp_bonus(&self, cat: &MetaShopCatalogue) -> f32 {
        self.sum_effect(cat, |e| match e {
            MetaEffect::MaxHp(a) => Some(a),
            _ => None,
        })
    }
    pub fn damage_mul(&self, cat: &MetaShopCatalogue) -> f32 {
        1.0 + self.sum_effect(cat, |e| match e {
            MetaEffect::DamageMul(a) => Some(a),
            _ => None,
        })
    }
    pub fn damage_reduction(&self, cat: &MetaShopCatalogue) -> f32 {
        self.sum_effect(cat, |e| match e {
            MetaEffect::DamageReduction(a) => Some(a),
            _ => None,
        })
        .min(0.85)
    }
    pub fn start_gold(&self, cat: &MetaShopCatalogue) -> u32 {
        self.sum_effect(cat, |e| match e {
            MetaEffect::StartGold(a) => Some(a as f32),
            _ => None,
        }) as u32
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
        Self {
            damage_mul: 1.0,
            damage_reduction: 0.0,
        }
    }
}

/// Bonus de dégâts issu du NIVEAU de maîtrise de l'arme équipée (P3). Recalculé au
/// run-start (`sys_apply_weapon_choice`) puis **réaligné sur l'arme équipée** à
/// chaque frame par `weapon_select::sys_sync_mastery_current` ; composé dans
/// `PlayerCombatMods.damage_mul` par `boons_apply`.
///
/// ## Pourquoi le resync existe (2026-08-04)
///
/// Avant, ce bonus n'était calculé qu'au **run-start** : changer d'arme en jeu
/// (Digit1-4) gardait la maîtrise de l'arme de départ. Mesuré sur une run réelle
/// via `forgia2_power.json` : arme Boucherie (niveau 2) portant le ×1,20 de Pépin,
/// puis arme Pépin (niveau 13, plafonné) ne touchant que le ×1,04 de Boucherie —
/// parfaitement inversé. Le défaut coupait dans les deux sens, et il punissait
/// précisément le geste que le jeu veut encourager : changer d'arme selon l'ennemi.
///
/// La Trempe faisait déjà ce resync (`trempe::sys_sync_trempe_current`) ; les deux
/// progressions d'arme se comportent désormais pareil.
#[derive(Resource, Debug, Clone, Copy)]
pub struct WeaponMasteryMods {
    pub damage_mul: f32,
    /// Arme dont `damage_mul` reflète le niveau (cohérence affichage/recompute).
    pub current: forgia_combat::weapons::WeaponType,
}

impl Default for WeaponMasteryMods {
    fn default() -> Self {
        Self {
            damage_mul: 1.0,
            current: forgia_combat::weapons::WeaponType::default(),
        }
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
    // 2026-08-05 — le carrousel de l'accueil s'ouvre sur le chapitre le plus
    // avancé JOUABLE. `SelectedChapter` ne vit qu'en mémoire (`Default` = 1) et
    // `clamped()` ne borne qu'en HAUT : sans cette ligne, un joueur à 6/10
    // retrouvait « CHAPITRE 1 » à chaque démarrage, et le bouton CONTINUER
    // relançait un chapitre déjà battu. Le mot doit dire ce qu'il fait.
    commands.insert_resource(SelectedChapter(furthest_playable_chapter(
        save.chapters_cleared,
    )));
    commands.insert_resource(save);
    commands.insert_resource(cat);
}

/// Réconcilie + sauve (OnExit Roguelite + OnEnter Victory/Defeat).
pub fn sys_flush_meta_save(meta: Res<MetaSouls>, mut save: ResMut<MetaShopSave>) {
    save.souls_total = meta.current;
    save.save();
}

/// Intervalle de l'autosave des Âmes (s). Plomberie de persistance, PAS un levier
/// de gameplay : ni exposé au créateur (cf `creator-simplicity`), ni lu par un
/// système de jeu — même statut que `SAVE_VERSION` / `SAVE_FILE` ci-dessus.
/// 10 s borne la perte maximale à une poignée d'Âmes.
const AUTOSAVE_INTERVAL_SECS: f32 = 10.0;

/// Autosave périodique des Âmes PENDANT la run.
///
/// Avant : `sys_flush_meta_save` n'était câblé que sur `OnExit(GameMode::Roguelite)`,
/// `OnEnter(Victory)` et `OnEnter(Defeat)` — les Âmes gagnées en run (vagues, wisps,
/// pièces/étoiles du parcours) ne vivaient que dans la Resource `MetaSouls`, et un
/// alt-F4 en pleine run perdait TOUT le revenu de la run.
///
/// Écriture disque uniquement si le total a MONTÉ, au plus une fois toutes les
/// `AUTOSAVE_INTERVAL_SECS` — donc zéro I/O quand rien ne change, et pas de churn de
/// change-detection sur `MetaShopSave`.
/// `Time<Real>` : l'autosave ne doit pas être gelé par une pause de gameplay.
///
/// **Uniquement à la hausse, et c'est délibéré.** Le seul débit d'Âmes en run est le
/// « Second souffle » du marchand (`merchant.rs`, `Currency::Ames`), dont la
/// contrepartie — le jeton de revive — est un état de run NON persisté. Persister le
/// débit sans sa contrepartie ferait perdre au joueur les Âmes ET l'objet s'il quitte
/// juste après l'achat. Le solde exact est de toute façon scellé par
/// `sys_flush_meta_save` à la fin de la run.
pub fn sys_autosave_meta_souls(
    time: Res<Time<Real>>,
    meta: Res<MetaSouls>,
    mut save: ResMut<MetaShopSave>,
    mut cooldown: Local<f32>,
) {
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = AUTOSAVE_INTERVAL_SECS;
    if meta.current <= save.souls_total {
        return; // rien gagné depuis le dernier flush → pas d'écriture disque
    }
    save.souls_total = meta.current;
    save.save();
}

/// Hot-reload 1 Hz du genome méta-shop (patron de `ultimate_config.rs` / `poi.rs`).
///
/// `genome-code.md` : « tout gene DOIT fonctionner avec Shift+F12 ». `[mastery]` est
/// précisément le gène le plus destiné à être itéré en passe de balance ; sans ça,
/// chaque essai coûte un rebuild + relance au lieu d'une sauvegarde de fichier.
pub fn sys_hot_reload_meta_shop_genome(
    time: Res<Time<Real>>,
    mut cat: ResMut<MetaShopCatalogue>,
    mut cooldown: Local<f32>,
    mut last_mtime: Local<Option<std::time::SystemTime>>,
) {
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = 1.0;
    let Ok(mtime) = std::fs::metadata(GENOME_PATH).and_then(|m| m.modified()) else {
        return; // fichier absent (build distribué) → on garde la Resource en place
    };
    if *last_mtime == Some(mtime) {
        return;
    }
    let first = last_mtime.is_none();
    *last_mtime = Some(mtime);
    if first {
        return; // 1er passage = simple prise d'empreinte, pas un rechargement
    }
    let Ok(content) = std::fs::read_to_string(GENOME_PATH) else {
        return;
    };
    *cat = MetaShopCatalogue::parse_toml(&content);
    info!(
        "[meta-shop] genome HOT-RELOADED — maîtrise {}×{:.0}%",
        cat.mastery.max_level,
        cat.mastery.damage_per_level * 100.0
    );
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
    warmup: Option<Res<crate::pipeline_warmup::WarmupState>>,
    mut start_run: MessageWriter<StartRunEvent>,
) {
    // Lancer la run (ENTRÉE) — réconcilie + sauve d'abord.
    let launch_requested =
        keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let warmup_ready = warmup.as_ref().is_some_and(|state| state.done);
    if launch_requested && warmup_ready {
        save.souls_total = meta.current;
        save.save();
        start_run.write(StartRunEvent { seed: None });
        return;
    }
    // Déblocage paliers d'atouts (Digit5/6/7) — story-616. Logique partagée avec
    // le hub-menu cliquable (`apply_meta_purchase`).
    let tier_idx = [KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7]
        .iter()
        .position(|k| keys.just_pressed(*k));
    if let Some(ti) = tier_idx {
        apply_meta_purchase(&cat, &mut save, &mut meta, MetaPurchase::BoonTier(ti));
        return;
    }
    // Story-681 — F1..F4 : CHANGER DE VOIE. Le choix de face (story-680 cran 2)
    // n'était offert qu'au clic dans le hub-menu ; au Lobby, il n'existait pas.
    // Une décision qu'on ne peut pas prendre depuis l'écran où on décide n'est
    // pas une décision.
    //
    // Touches F et non chiffres : les chiffres ACHÈTENT, et confondre « acheter
    // un rang » avec « changer ce que font mes rangs » coûterait des âmes.
    let swap = [KeyCode::F1, KeyCode::F2, KeyCode::F3, KeyCode::F4]
        .iter()
        .position(|k| keys.just_pressed(*k));
    if let Some(i) = swap {
        apply_meta_purchase(&cat, &mut save, &mut meta, MetaPurchase::ToggleFace(i));
        return;
    }
    // Achat upgrades 1..=4 (même logique partagée).
    let idx = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .iter()
    .position(|k| keys.just_pressed(*k));
    if let Some(i) = idx {
        apply_meta_purchase(&cat, &mut save, &mut meta, MetaPurchase::Upgrade(i));
    }
}

/// Un achat possible à l'Enclume (améliration de stat OU palier d'atouts).
/// Détecté au clic (`draw_enclume_panel`) ou au clavier (`sys_meta_shop_input`).
#[derive(Clone, Copy, Debug)]
pub enum MetaPurchase {
    /// Améliration de stat `cat.upgrades[i]` (rang +1).
    Upgrade(usize),
    /// Palier d'atouts `cat.boon_tier_unlocks[i]` (déblocage permanent).
    BoonTier(usize),
    /// Story-680 cran 2 — permute la face de `cat.upgrades[i]`. GRATUIT et
    /// réversible : Hades laisse repermuter le Miroir librement, parce que
    /// punir le changement d'avis transforme un choix en peur.
    ToggleFace(usize),
}

/// Applique un achat Enclume : mute `save`/`meta` + sauve sur disque. Logique
/// PARTAGÉE entre le clavier (Lobby) et le clic (hub-menu). Retourne `true` si
/// l'achat a bien eu lieu (assez d'âmes + pas déjà max/débloqué).
/// **Dépenser des Âmes — la seule façon.** Rend `false` si la bourse ne suit pas.
///
/// Une dépense, c'est TROIS écritures indissociables : le solde vif
/// (`MetaSouls.current`), son miroir persisté (`MetaShopSave.souls_total`) et
/// l'écriture disque. En oublier une ne se voit pas tout de suite : l'autosave
/// porte le garde `if meta.current <= save.souls_total { return }` — il ne
/// pousse que les GAINS. Une dépense qui ne met pas `souls_total` à jour n'est
/// donc jamais persistée, le compteur affiché (qui lit le miroir) ne bouge pas,
/// et les Âmes reviennent au relancement. C'est exactement le défaut rapporté
/// sur la galerie de décors le 2026-08-06 : « achat sans effet ».
///
/// Le geste était déjà écrit deux fois dans `apply_meta_purchase` ; la galerie
/// en aurait fait un troisième site. Une grandeur écrite N fois finit toujours
/// par diverger (`feedback_une_grandeur_ecrite_deux_fois`).
pub fn spend_souls(save: &mut MetaShopSave, meta: &mut MetaSouls, cost: u32) -> bool {
    if !debit_souls(save, meta, cost) {
        return false;
    }
    save.save();
    true
}

/// Débloque une arme CONTRE des Âmes — le seul chemin d'achat d'arme.
///
/// Même contrat que [`spend_souls`], PLUS le déblocage : solde vif, miroir,
/// arme et disque partent ensemble, en UNE écriture. Rend `false` si la
/// bourse ne suit pas. Trois sites de `weapon_select` recopiaient ce geste à
/// la main — c'est la classe « une grandeur écrite N fois » que le doc de
/// [`spend_souls`] documente ; ce helper la referme.
pub fn unlock_weapon_paid(
    save: &mut MetaShopSave,
    meta: &mut MetaSouls,
    key: &str,
    cost: u32,
) -> bool {
    if !debit_souls(save, meta, cost) {
        return false;
    }
    save.unlock_weapon(key);
    save.save();
    true
}

/// PUR — les deux soldes bougent ENSEMBLE, ou aucun ne bouge.
///
/// Séparé de l'écriture disque pour être testable : un test de
/// [`spend_souls`] écrirait dans la vraie sauvegarde du joueur.
fn debit_souls(save: &mut MetaShopSave, meta: &mut MetaSouls, cost: u32) -> bool {
    if meta.current < cost {
        return false;
    }
    meta.current -= cost;
    save.souls_total = meta.current;
    true
}

pub fn apply_meta_purchase(
    cat: &MetaShopCatalogue,
    save: &mut MetaShopSave,
    meta: &mut MetaSouls,
    purchase: MetaPurchase,
) -> bool {
    match purchase {
        MetaPurchase::Upgrade(i) => {
            let Some(up) = cat.upgrades.get(i) else {
                return false;
            };
            let rank = save.rank(&up.id);
            // Story-681 — les logs nomment la FACE ACTIVE, pas la principale :
            // un diagnostic qui ne dit pas ce que le joueur avait sous les yeux
            // envoie chercher le défaut au mauvais endroit.
            let (face_name, _, _) = up.face(save.face_of(&up.id));
            let Some(cost) = up.cost_for_next(rank) else {
                info!("[meta-shop] {face_name} déjà au rang max");
                return false;
            };
            if meta.current < cost {
                info!(
                    "[meta-shop] pas assez d'âmes pour {face_name} ({}/{})",
                    meta.current, cost
                );
                return false;
            }
            *save.ranks.entry(up.id.clone()).or_insert(0) += 1;
            spend_souls(save, meta, cost);
            info!(
                "[meta-shop] acheté {face_name} rang {} (-{cost} âmes, reste {})",
                rank + 1,
                meta.current
            );
            true
        }
        MetaPurchase::ToggleFace(i) => {
            let Some(up) = cat.upgrades.get(i) else {
                return false;
            };
            if !up.has_alt() {
                return false;
            }
            save.toggle_face(&up.id);
            save.save();
            let (name, desc, _) = up.face(save.face_of(&up.id));
            info!(
                "[meta-shop] {} → face « {name} » ({desc}) — {} rang(s) conservé(s)",
                up.id,
                save.rank(&up.id)
            );
            true
        }
        MetaPurchase::BoonTier(ti) => {
            let Some(bt) = cat.boon_tier_unlocks.get(ti) else {
                return false;
            };
            if save.is_boon_tier_unlocked(&bt.key) {
                info!("[meta-shop] palier d'atouts {} déjà débloqué", bt.name);
                return false;
            }
            if meta.current < bt.cost {
                info!(
                    "[meta-shop] pas assez d'âmes pour {} ({}/{})",
                    bt.name, meta.current, bt.cost
                );
                return false;
            }
            save.unlock_boon_tier(&bt.key);
            spend_souls(save, meta, bt.cost);
            info!(
                "[meta-shop] palier d'atouts débloqué : {} (-{} âmes, reste {})",
                bt.name, bt.cost, meta.current
            );
            true
        }
    }
}

/// Rend l'Enclume en **cartes cliquables** dans un `Ui` donné (souris) — réutilisé
/// par le hub-menu (`forgia-ui`). PUR affichage + détection de clic (aucune
/// mutation) : retourne l'achat cliqué s'il y en a un, à appliquer par l'appelant
/// via [`apply_meta_purchase`]. `souls` = solde d'Âmes courant (`MetaSouls.current`).
pub fn draw_enclume_panel(
    ui: &mut egui::Ui,
    cat: &MetaShopCatalogue,
    save: &MetaShopSave,
    souls: u32,
) -> Option<MetaPurchase> {
    let mut intent = None;
    ui.label(
        // FORGE_AME comme le chip du haut : une monnaie = une couleur, partout
        // (l'audit 2026-08-07 relevait deux soldes d'Âmes côte à côte, l'un or
        // l'autre teal — le joueur voyait deux monnaies là où il n'y en a une).
        egui::RichText::new(format!("◇ {souls}  Âmes"))
            .size(22.0)
            .strong()
            .color(FORGE_AME),
    );
    ui.add_space(12.0);
    // ── Améliorations de stat (cliquables si abordables) ──
    for (i, up) in cat.upgrades.iter().enumerate() {
        let rank = save.rank(&up.id);
        let max = up.max_rank();
        // Story-680 cran 2 — la ligne affiche sa FACE ACTIVE. Les rangs sont
        // partagés entre les deux faces : on choisit ce qu'ils font, pas
        // combien on en a.
        let (name, desc, _) = up.face(save.face_of(&up.id));
        match up.cost_for_next(rank) {
            Some(cost) => {
                let afford = souls >= cost;
                let btn = egui::Button::new(
                    egui::RichText::new(format!("{name}   rang {rank}/{max}   ·   {cost} ◇"))
                        .size(16.0)
                        .color(if afford { FORGE_OR } else { C_TEXT_MUTED }),
                )
                .min_size(egui::vec2(440.0, 0.0));
                if ui.add_enabled(afford, btn).clicked() {
                    intent = Some(MetaPurchase::Upgrade(i));
                }
            }
            None => {
                let _ = ui.add_enabled(
                    false,
                    egui::Button::new(
                        egui::RichText::new(format!("{name}   MAX {max}/{max}"))
                            .size(16.0)
                            .color(C_HP_HIGH),
                    )
                    .min_size(egui::vec2(440.0, 0.0)),
                );
            }
        }
        ui.label(egui::RichText::new(desc).size(12.0).color(C_TEXT_MUTED));
        // Le bouton de permutation nomme l'AUTRE face : le joueur doit voir ce
        // qu'il abandonne et ce qu'il gagne avant de cliquer, pas après.
        if up.has_alt() {
            let (other_name, other_desc, _) = up.face(1 - save.face_of(&up.id));
            let swap = egui::Button::new(
                egui::RichText::new(format!("⇄  basculer en « {other_name} » — {other_desc}"))
                    .size(12.0)
                    .color(FORGE_TEAL),
            )
            .min_size(egui::vec2(440.0, 0.0));
            if ui.add(swap).clicked() {
                intent = Some(MetaPurchase::ToggleFace(i));
            }
        }
        ui.add_space(6.0);
    }
    // ── Paliers d'atouts (boons) ──
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("— ATOUTS (paliers de boons) —")
            .size(15.0)
            .strong()
            .color(FORGE_TEAL),
    );
    ui.add_space(4.0);
    for (i, bt) in cat.boon_tier_unlocks.iter().enumerate() {
        if save.is_boon_tier_unlocked(&bt.key) {
            let _ = ui.add_enabled(
                false,
                egui::Button::new(
                    egui::RichText::new(format!("{} — DÉBLOQUÉ", bt.name))
                        .size(16.0)
                        .color(C_HP_HIGH),
                )
                .min_size(egui::vec2(440.0, 0.0)),
            );
        } else {
            let afford = souls >= bt.cost;
            let btn = egui::Button::new(
                egui::RichText::new(format!("{}   ·   {} ◇", bt.name, bt.cost))
                    .size(16.0)
                    .color(if afford { FORGE_OR } else { C_TEXT_MUTED }),
            )
            .min_size(egui::vec2(440.0, 0.0));
            if ui.add_enabled(afford, btn).clicked() {
                intent = Some(MetaPurchase::BoonTier(i));
            }
        }
        ui.add_space(4.0);
    }
    intent
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
                .stroke(egui::Stroke::new(1.5, HAIR_GOLD_STRONG))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        // Story-681 — le NIVEAU au Lobby. Depuis story-680, le
                        // niveau EST la somme des rangs achetés ici : c'est donc
                        // l'écran où il veut dire quelque chose. L'afficher
                        // ailleurs sans l'afficher ici serait absurde.
                        ui.label(
                            egui::RichText::new(format!(
                                "Niveau {}",
                                save.player_level(&cat)
                            ))
                            .size(26.0)
                            .strong()
                            .color(FORGE_OR),
                        );
                        let remaining = save.ranks_remaining(&cat);
                        ui.label(
                            egui::RichText::new(if remaining == 0 {
                                "Enclume complète".to_string()
                            } else {
                                format!("{remaining} amélioration(s) encore disponible(s)")
                            })
                            .size(15.0)
                            .color(C_TEXT_MUTED),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(format!("◇ Âmes : {}", meta.current))
                                .size(24.0)
                                .strong()
                                .color(FORGE_TEAL),
                        );
                        ui.add_space(16.0);
                        // Story-681 — le Lobby lisait `up.name` / `up.desc`, donc
                        // il affichait TOUJOURS la face principale : un joueur
                        // ayant basculé sur « Cuirasse » voyait quand même
                        // « Vitalité +15 PV ». Même classe de mensonge que le
                        // « SALLE 5 / 4 » — deux écrans, une seule vérité.
                        for (i, up) in cat.upgrades.iter().enumerate() {
                            let rank = save.rank(&up.id);
                            let max = up.max_rank();
                            let (name, desc, _) = up.face(save.face_of(&up.id));
                            let (text, col) = match up.cost_for_next(rank) {
                                Some(cost) => {
                                    let afford = meta.current >= cost;
                                    (
                                        format!(
                                            "[{}]  {name} — {desc}  (rang {rank}/{max})  ·  {cost} âmes",
                                            i + 1
                                        ),
                                        if afford { FORGE_OR } else { C_TEXT_MUTED },
                                    )
                                }
                                None => (
                                    format!("[—]  {name} — {desc}  (MAX {max}/{max})"),
                                    C_HP_HIGH,
                                ),
                            };
                            ui.label(egui::RichText::new(text).size(19.0).color(col));
                            // La face alternative est ANNONCÉE : sans ça, le
                            // choix existerait sans jamais être offert au Lobby.
                            if up.has_alt() {
                                let (other, other_desc, _) = up.face(1 - save.face_of(&up.id));
                                ui.label(
                                    egui::RichText::new(format!(
                                        "        ⇄ [F{}]  {other} — {other_desc}",
                                        i + 1
                                    ))
                                    .size(15.0)
                                    .color(FORGE_TEAL),
                                );
                            }
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
                            egui::RichText::new(
                                "1-4 acheter · F1-F4 changer de voie · 5-7 atouts · ENTRÉE = lancer",
                            )
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
        app.init_resource::<WeaponMasteryMods>();
        // 2026-08-04 — le chapitre choisi au Lobby. Sans cette ligne,
        // `sys_record_run_stats` échoue à la validation de ses paramètres en fin
        // de run : une Resource non enregistrée ne se voit qu'au runtime.
        app.init_resource::<SelectedChapter>();
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
        // Autosave EN run : sans lui, un alt-F4 en pleine run perd toutes les Âmes
        // gagnées (les 3 flushes ci-dessus ne couvrent que les sorties propres).
        app.add_systems(
            Update,
            sys_autosave_meta_souls.run_if(in_state(GameMode::Roguelite)),
        );
        // Hot-reload du genome (dont [mastery]) — aligné sur les autres genomes du crate.
        app.add_systems(Update, sys_hot_reload_meta_shop_genome);
        // R3.3 (story-645) — stats persistantes (runs/victoires/record) figées AVANT
        // le flush pour partir sur disque dans la même frame.
        app.init_resource::<LastRunStats>();
        app.add_systems(
            OnEnter(RunState::Victory),
            sys_record_run_stats.before(sys_flush_meta_save),
        );
        app.add_systems(
            OnEnter(RunState::Defeat),
            sys_record_run_stats.before(sys_flush_meta_save),
        );
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Maîtrise d'arme : plafond (anti-progression infinie) ──

    #[test]
    fn mastery_damage_mul_grows_then_stops_at_cap() {
        let m = MasteryConfig {
            max_level: 6,
            damage_per_level: 0.04,
        };
        assert_eq!(m.damage_mul(1), 1.0, "niveau 1 = aucun bonus");
        assert!((m.damage_mul(6) - 1.20).abs() < 1e-6, "cap = +20 %");
        assert_eq!(
            m.damage_mul(50),
            m.damage_mul(6),
            "au-delà du plafond, le bonus n'augmente plus"
        );
    }

    #[test]
    fn level_up_weapon_stops_at_cap() {
        let mut s = MetaShopSave::default();
        for _ in 0..20 {
            s.level_up_weapon("pepin", 6);
        }
        assert_eq!(
            s.weapon_level("pepin"),
            6,
            "20 runs ne doivent pas dépasser le plafond de 6"
        );
    }

    #[test]
    fn level_up_weapon_returns_unchanged_level_at_cap() {
        let mut s = MetaShopSave::default();
        assert_eq!(s.level_up_weapon("pepin", 2), 2);
        assert_eq!(
            s.level_up_weapon("pepin", 2),
            2,
            "au plafond, le niveau retourné est inchangé (→ pas d'écriture disque)"
        );
    }

    /// RÉGRESSION (QA story-668) : un save réel contenait `pepin = 13` AVANT que le
    /// plafond n'existe. Un `.min(cap)` à l'écriture réécrivait 13 → 6 sur le disque
    /// à la fin de la run suivante, détruisant la progression pour toujours.
    #[test]
    fn level_up_weapon_never_lowers_a_legacy_level() {
        let mut s = MetaShopSave::default();
        s.weapon_levels.insert("pepin".to_string(), 13);
        assert_eq!(
            s.level_up_weapon("pepin", 6),
            13,
            "une progression au-dessus du plafond est CONSERVÉE, jamais rabaissée"
        );
        assert_eq!(s.weapon_level("pepin"), 13, "le disque garde 13");
        // …et le bonus, lui, EST bien borné à la lecture.
        let m = MasteryConfig::default();
        assert_eq!(m.damage_mul(13), m.damage_mul(6), "bonus borné au plafond");
        assert_eq!(m.effective_level(13), 6, "l'UI affiche 6/6, pas 13/6");
    }

    #[test]
    fn mastery_comes_from_the_genome_and_falls_back_when_absent() {
        let with =
            MetaShopCatalogue::parse_toml("[mastery]\nmax_level = 3\ndamage_per_level = 0.1");
        assert_eq!(with.mastery.max_level, 3);
        assert!((with.mastery.damage_mul(3) - 1.20).abs() < 1e-6);

        let without = MetaShopCatalogue::parse_toml("");
        assert_eq!(
            without.mastery,
            MasteryConfig::default(),
            "genome sans [mastery] → miroir Rust"
        );
    }

    /// RÉGRESSION (QA story-668) : sans `#[serde(default)]` sur les champs de
    /// `MasteryToml`, une section `[mastery]` PARTIELLE faisait échouer serde sur le
    /// DOCUMENT ENTIER → tout le catalogue (upgrades, prix d'armes, prix de paliers)
    /// retombait en silence sur le miroir Rust.
    #[test]
    fn a_partial_mastery_section_does_not_kill_the_whole_document() {
        let cat = MetaShopCatalogue::parse_toml(
            r#"
[[upgrades]]
id = "max_hp"
name = "Vitalité"
desc = "+15 PV max"
effect = "max_hp"
amount = 15.0
costs = [10, 20]

[mastery]
max_level = 8

[[weapon_unlocks]]
key = "bourrasque"
name = "Bourrasque"
cost = 7
"#,
        );
        assert_eq!(cat.mastery.max_level, 8, "le champ fourni est lu");
        assert_eq!(
            cat.mastery.damage_per_level,
            MasteryConfig::DEFAULT_DAMAGE_PER_LEVEL,
            "le champ absent prend le défaut, il ne fait pas échouer le parse"
        );
        assert_eq!(
            cat.upgrades[0].costs,
            vec![10, 20],
            "les coûts sont bien lus"
        );
        assert_eq!(
            cat.weapon_unlock("bourrasque").map(|w| w.cost),
            Some(7),
            "les sections SUIVANTES survivent"
        );
    }

    /// `genome-code.md` : « chaque gene a une valeur min/max raisonnable ».
    /// Le piège visé : `damage_per_level = 10` lu comme « 10 % » → ×51 de dégâts.
    #[test]
    fn genome_values_are_bounded_so_a_creator_cannot_break_the_game() {
        let insane = MasteryConfig::from_genome(1000, 10.0);
        assert_eq!(insane.max_level, 20);
        assert!((insane.damage_per_level - 0.25).abs() < 1e-6);

        let negative = MasteryConfig::from_genome(0, -0.5);
        assert_eq!(negative.max_level, 1, "un plafond nul est ramené à 1");
        assert_eq!(
            negative.damage_per_level, 0.0,
            "pas de multiplicateur de dégâts négatif"
        );
        assert!(
            negative.damage_mul(50) >= 1.0,
            "le multiplicateur reste >= 1 quoi qu'écrive le créateur"
        );

        let nan = MasteryConfig::from_genome(6, f32::NAN);
        assert_eq!(
            nan.damage_per_level,
            MasteryConfig::DEFAULT_DAMAGE_PER_LEVEL
        );
    }

    // ── record_run_result (R3.3, story-645) ──

    #[test]
    fn defeat_counts_run_but_no_victory_nor_record() {
        let mut s = MetaShopSave::default();
        assert!(!record_run_result(&mut s, 300.0, false));
        assert_eq!((s.runs_played, s.victories), (1, 0));
        assert_eq!(s.best_victory_secs, 0.0, "défaite ne pose pas de record");
    }

    #[test]
    fn first_victory_sets_record_then_only_faster_beats_it() {
        let mut s = MetaShopSave::default();
        assert!(
            record_run_result(&mut s, 900.0, true),
            "1re victoire = record"
        );
        assert!(
            !record_run_result(&mut s, 1000.0, true),
            "plus lent ≠ record"
        );
        assert!(
            record_run_result(&mut s, 600.0, true),
            "plus rapide = record"
        );
        assert_eq!((s.runs_played, s.victories), (3, 3));
        assert_eq!(s.best_victory_secs, 600.0);
    }

    /// Story-680 cran 3 — les bonus ne sont plus LINÉAIRES (`amount × rank`),
    /// ils ont des rendements DÉCROISSANTS. Ce test encodait l'ancien
    /// comportement ; il vérifie maintenant le nouveau contrat.
    #[test]
    fn cumulative_bonuses_grow_with_rank_but_with_diminishing_returns() {
        let cat = MetaShopCatalogue::default();
        let mut save = MetaShopSave::default();
        save.ranks.insert("max_hp".into(), 3);
        save.ranks.insert("armor".into(), 1);
        // Le rang 1 vaut plein pot : c'est le plancher du contrat.
        assert!((save.damage_reduction(&cat) - 0.05).abs() < 1e-5);
        // 3 rangs de PV valent PLUS qu'1 seul, mais MOINS que 3 fois 1.
        let three = save.max_hp_bonus(&cat);
        save.ranks.insert("max_hp".into(), 1);
        let one = save.max_hp_bonus(&cat);
        assert!(three > one, "monter doit rapporter : {one} → {three}");
        assert!(
            three < one * 3.0,
            "le linéaire est de retour ({three} vs {} attendu strictement moins)",
            one * 3.0
        );
    }

    /// Le gain MARGINAL décroît à chaque rang — mais ne tombe jamais à zéro.
    /// Un rang décoratif qu'on fait payer est pire qu'un rang absent (Hades :
    /// « la chute a un plancher »).
    #[test]
    fn the_marginal_gain_shrinks_but_never_vanishes() {
        let cat = MetaShopCatalogue::default();
        let up = cat
            .upgrades
            .iter()
            .find(|u| u.id == "max_hp")
            .expect("max_hp existe");
        let mut prev_marginal = f32::INFINITY;
        for rank in 1..=up.max_rank() {
            let marginal = up.total_amount(15.0, rank) - up.total_amount(15.0, rank - 1);
            assert!(
                marginal > 0.0,
                "rang {rank} ne rapporte RIEN — rang décoratif"
            );
            assert!(
                marginal <= prev_marginal + 1e-4,
                "rang {rank} rapporte PLUS que le précédent ({marginal} > {prev_marginal})"
            );
            prev_marginal = marginal;
        }
        // Plancher : le dernier rang vaut encore au moins 40 % du premier.
        let first = up.total_amount(15.0, 1);
        let last = up.total_amount(15.0, up.max_rank()) - up.total_amount(15.0, up.max_rank() - 1);
        assert!(
            last >= first * 0.39,
            "plancher percé : {last} vs {}",
            first * 0.4
        );
    }

    /// Le coût croît fortement (25 → 190, ×7,6) alors que le gain décroît :
    /// c'est CE rapport qui crée l'arbitrage « j'approfondis ou j'ouvre ailleurs ».
    #[test]
    fn deep_ranks_cost_more_and_give_less_which_is_the_whole_point() {
        let cat = MetaShopCatalogue::default();
        let up = cat.upgrades.iter().find(|u| u.id == "damage").unwrap();
        let cost_1 = up.cost_for_next(0).unwrap();
        let cost_last = up.cost_for_next(up.max_rank() - 1).unwrap();
        let gain_1 = up.total_amount(0.08, 1);
        let gain_last =
            up.total_amount(0.08, up.max_rank()) - up.total_amount(0.08, up.max_rank() - 1);
        assert!(cost_last > cost_1 * 3, "les coûts doivent vraiment monter");
        assert!(gain_last < gain_1, "les gains doivent vraiment descendre");
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
        assert_eq!(
            c.upgrades.len(),
            MetaShopCatalogue::default().upgrades.len()
        );
    }

    #[test]
    fn une_depense_bouge_le_solde_et_son_miroir_persiste() {
        // Le défaut du 2026-08-06 (« achat sans effet ») : la galerie de décors
        // décrémentait `MetaSouls.current` sans toucher `souls_total`. Or
        // l'autosave ne pousse que les GAINS (`if meta.current <= souls_total
        // { return }`) — la dépense n'était donc jamais persistée, le compteur
        // affiché (qui lit le miroir) ne bougeait pas, et les Âmes revenaient au
        // relancement. Les deux DOIVENT bouger ensemble.
        let mut save = MetaShopSave::default();
        let mut meta = MetaSouls::default();
        meta.current = 1000;
        save.souls_total = 1000;

        assert!(debit_souls(&mut save, &mut meta, 300));
        assert_eq!(meta.current, 700);
        assert_eq!(
            save.souls_total, 700,
            "le miroir persisté doit suivre, sinon l'autosave ne sauvera jamais la dépense"
        );

        // Bourse insuffisante : RIEN ne bouge, ni l'un ni l'autre.
        assert!(!debit_souls(&mut save, &mut meta, 701));
        assert_eq!(meta.current, 700);
        assert_eq!(save.souls_total, 700);

        // Le solde exact passe (pas de `<` de trop qui interdirait le dernier achat).
        assert!(debit_souls(&mut save, &mut meta, 700));
        assert_eq!(meta.current, 0);
        assert_eq!(save.souls_total, 0);
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
            save.unlocked_weapons
                .iter()
                .filter(|k| *k == "bourrasque")
                .count(),
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

#[cfg(test)]
mod face_tests {
    use super::*;

    /// LE point du cran 2 : un choix n'existe que si prendre A INTERDIT B.
    /// Avant, tout se cumulait — donc aucune décision.
    #[test]
    fn only_the_active_face_applies_never_both() {
        let cat = MetaShopCatalogue::default();
        let mut save = MetaShopSave::default();
        save.ranks.insert("max_hp".into(), 3);

        // Face principale : des PV, pas de réduction.
        let hp_a = save.max_hp_bonus(&cat);
        let dr_a = save.damage_reduction(&cat);
        assert!(
            hp_a > 0.0 && dr_a == 0.0,
            "face A doit donner SEULEMENT des PV"
        );

        // Face alternative : de la réduction, plus de PV.
        save.toggle_face("max_hp");
        let hp_b = save.max_hp_bonus(&cat);
        let dr_b = save.damage_reduction(&cat);
        assert!(dr_b > 0.0, "face B doit donner de la réduction");
        assert!(
            hp_b < hp_a,
            "face B ne doit PLUS donner les PV de la face A"
        );
    }

    /// Permuter conserve les rangs : on choisit ce qu'ils font, pas combien on
    /// en a. Perdre ses rangs en changeant d'avis rendrait le choix irréversible
    /// de fait, donc effrayant.
    #[test]
    fn swapping_a_face_keeps_the_ranks() {
        let cat = MetaShopCatalogue::default();
        let mut save = MetaShopSave::default();
        save.ranks.insert("damage".into(), 4);
        save.toggle_face("damage");
        assert_eq!(save.rank("damage"), 4, "les rangs sont conservés");
        assert_eq!(save.face_of("damage"), 1);
        // Et c'est réversible sans coût.
        save.toggle_face("damage");
        assert_eq!(save.face_of("damage"), 0);
        assert_eq!(save.rank("damage"), 4);
        let _ = cat;
    }

    /// Le niveau ne doit PAS changer en permutant : permuter n'est pas un achat.
    #[test]
    fn swapping_does_not_change_the_player_level() {
        let cat = MetaShopCatalogue::default();
        let mut save = MetaShopSave::default();
        save.ranks.insert("armor".into(), 2);
        let before = save.player_level(&cat);
        save.toggle_face("armor");
        assert_eq!(save.player_level(&cat), before);
    }

    /// Un save d'avant story-680 n'a pas de champ `faces` : il doit rester
    /// valide et se comporter comme avant (face principale partout).
    #[test]
    fn a_save_from_before_the_feature_still_works() {
        let cat = MetaShopCatalogue::default();
        let save = MetaShopSave::default();
        assert!(save.faces.is_empty());
        assert_eq!(save.face_of("max_hp"), 0, "repli sur la face principale");
        let _ = save.max_hp_bonus(&cat);
    }

    /// Le génome livré doit déclarer les mêmes faces que le miroir Rust —
    /// sinon le repli change le jeu au lieu de le préserver.
    #[test]
    fn the_shipped_genome_declares_the_same_faces_as_the_rust_mirror() {
        let content = std::fs::read_to_string(GENOME_PATH)
            .or_else(|_| std::fs::read_to_string(format!("../../{GENOME_PATH}")))
            .expect("roguelite_meta_shop.toml introuvable");
        let from_toml = MetaShopCatalogue::parse_toml(&content);
        let mirror = MetaShopCatalogue::default();
        assert_eq!(
            from_toml.upgrades.len(),
            mirror.upgrades.len(),
            "le TOML et le miroir n'ont pas le même nombre de lignes"
        );
        for (a, b) in from_toml.upgrades.iter().zip(mirror.upgrades.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(
                a.alt.is_some(),
                b.alt.is_some(),
                "ligne '{}' : face alternative présente d'un côté seulement",
                a.id
            );
            assert_eq!(a.alt, b.alt, "ligne '{}' : les faces ont divergé", a.id);
        }
    }

    /// Chaque ligne DOIT avoir une alternative : une ligne sans alternative est
    /// un achat sans décision, ce que cette story existe pour supprimer.
    #[test]
    fn every_line_offers_a_real_choice() {
        for u in &MetaShopCatalogue::default().upgrades {
            assert!(u.has_alt(), "la ligne '{}' n'offre aucun choix", u.id);
            let (na, _, ea) = u.face(0);
            let (nb, _, eb) = u.face(1);
            assert_ne!(
                na, nb,
                "ligne '{}' : les deux faces portent le même nom",
                u.id
            );
            assert_ne!(
                std::mem::discriminant(&ea),
                std::mem::discriminant(&eb),
                "ligne '{}' : les deux faces font la MÊME chose — ce n'est pas un choix",
                u.id
            );
        }
    }
}

// ─── La maîtrise ne dépend plus de l'arme en main (2026-08-04) ───────────────

#[cfg(test)]
mod mastery_total_tests {
    use super::*;

    fn cat() -> MetaShopCatalogue {
        MetaShopCatalogue::default()
    }

    fn levels(pairs: &[(&str, u32)]) -> HashMap<String, u32> {
        pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
    }

    /// **LE test de la règle.** Changer d'arme ne doit RIEN coûter — sinon le jeu
    /// punit le geste que la table de matchup récompense, et le joueur optimal
    /// ignore le matchup.
    #[test]
    fn switching_weapons_costs_nothing() {
        let c = cat();
        let save = levels(&[("pepin", 6), ("boucherie", 2)]);
        // Le bonus est le même quelle que soit l'arme en main : il n'en dépend
        // plus du tout. C'est ça, la correction.
        let bonus = c.mastery.total_damage_mul(&save);
        assert!(bonus > 1.0, "il y a bien un bonus à avoir");
        // Avant, ces deux lectures différaient de 13 points de pourcentage.
        let ancien_pepin = c.mastery.damage_mul(6);
        let ancien_boucherie = c.mastery.damage_mul(2);
        assert!(
            (ancien_pepin - ancien_boucherie).abs() > 0.1,
            "le défaut d'origine : 13 % d'écart selon l'arme tenue"
        );
    }

    /// Monter UNE arme à fond ou QUATRE armes à un quart donne le même total.
    /// La répartition devient libre ; le plafond, lui, ne bouge pas.
    #[test]
    fn spreading_across_weapons_is_as_good_as_focusing() {
        let c = cat();
        let concentre = c.mastery.total_damage_mul(&levels(&[("pepin", 6)]));
        let etale = c.mastery.total_damage_mul(&levels(&[
            ("pepin", 3),
            ("bourrasque", 2),
            ("madame_lenoir", 2),
            ("boucherie", 2),
        ]));
        // 5 niveaux acquis d'un côté, 2+1+1+1 = 5 de l'autre.
        assert!((concentre - etale).abs() < 1e-5, "{concentre} vs {etale}");
    }

    /// Le plafond est INCHANGÉ : la courbe de difficulté du Livre s'appuie dessus
    /// (`the_last_chapter_stays_within_reach_of_a_maxed_account` compte ×1,20).
    #[test]
    fn the_ceiling_is_untouched_by_the_new_distribution() {
        let c = cat();
        let attendu = 1.0 + (c.mastery.max_level - 1) as f32 * c.mastery.damage_per_level;
        // Quatre armes au plafond ne donnent pas quatre fois le bonus.
        let toutes_max = c.mastery.total_damage_mul(&levels(&[
            ("pepin", 99),
            ("bourrasque", 99),
            ("madame_lenoir", 99),
            ("boucherie", 99),
        ]));
        assert!(
            (toutes_max - attendu).abs() < 1e-5,
            "{toutes_max} vs {attendu}"
        );
        assert!((c.mastery.damage_mul(c.mastery.max_level) - attendu).abs() < 1e-5);
    }

    /// Une sauvegarde neuve n'a aucun bonus, et une sauvegarde legacy au-dessus du
    /// plafond ne le dépasse pas (le clamp est à la LECTURE, jamais à l'écriture).
    #[test]
    fn a_fresh_save_has_no_bonus_and_a_legacy_one_stays_capped() {
        let c = cat();
        assert_eq!(c.mastery.total_damage_mul(&HashMap::new()), 1.0);
        let legacy = c.mastery.total_damage_mul(&levels(&[("pepin", 13)]));
        let plafond = 1.0 + (c.mastery.max_level - 1) as f32 * c.mastery.damage_per_level;
        assert!(
            (legacy - plafond).abs() < 1e-5,
            "niveau 13 borné au plafond"
        );
    }
}
