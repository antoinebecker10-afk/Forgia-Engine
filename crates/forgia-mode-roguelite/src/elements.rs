//! elements.rs — Story-582 Phase A (2026-06-07). Système d'éléments par-arme.
//!
//! Chaque arme du loadout Roguelite a un **élément signature** (feu/poison/
//! explosif/perforant) avec :
//! - une **efficacité par type d'ennemi** (matchups Tank/Runner/Sniper/Boss),
//! - des **status effects** (burn = DoT plat, poison = DoT stackant + shred),
//! - de l'**AOE** (explosif) et de l'**exécution** (perforant : one-shot tank).
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

use bevy::prelude::*;
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_combat::weapons::WeaponType;
use forgia_combat::Health;
use serde::Deserialize;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    /// Feu — burn DoT plat. Identité SMG (Bourrasque).
    Fire,
    /// Poison/Corrosif — DoT stackant + shred armure. Identité pompe (Boucherie).
    Poison,
    /// Explosif — AOE splash autour de l'impact. Identité pistolet (Pépin).
    Explosive,
    /// Perforant — gros bonus vs Tank + exécution sous seuil. Identité sniper (Lenoir).
    ArmorPierce,
}

impl Element {
    pub fn label(self) -> &'static str {
        match self {
            Element::Fire => "fire",
            Element::Poison => "poison",
            Element::Explosive => "explosive",
            Element::ArmorPierce => "armor_pierce",
        }
    }

    /// Parse une clé TOML (tolérante FR/EN) vers un élément.
    pub fn from_key(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "fire" | "feu" => Some(Element::Fire),
            "poison" | "corrosive" | "corrosif" => Some(Element::Poison),
            "explosive" | "explosif" => Some(Element::Explosive),
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
            Element::Explosive => 2,
            Element::ArmorPierce => 3,
        }
    }

    /// Couleur RGB (linéaire) de l'élément, data-driven via [`VfxParams`].
    pub fn rgb(self, v: &VfxParams) -> [f32; 3] {
        match self {
            Element::Fire => v.fire_rgb,
            Element::Poison => v.poison_rgb,
            Element::Explosive => v.explosive_rgb,
            Element::ArmorPierce => v.armor_pierce_rgb,
        }
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
    pub explosive: Matchup,
    pub armor_pierce: Matchup,
}

impl MatchupTable {
    pub fn for_element(&self, e: Element) -> &Matchup {
        match e {
            Element::Fire => &self.fire,
            Element::Poison => &self.poison,
            Element::Explosive => &self.explosive,
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
    /// Multiplicateur de taille du flash pour un hit explosif (splash visible).
    pub explosive_scale: f32,
    /// Intensité (lumens) de la lumière d'impact (fade avec le flash).
    pub light_intensity: f32,
    /// Portée (m) de la lumière d'impact.
    pub light_range: f32,
    /// Rayon (m) du pulse coloré sur un ennemi en DoT (burn/poison).
    pub dot_pulse_scale: f32,
    /// Période (s) entre deux pulses DoT.
    pub dot_pulse_period: f32,
    pub fire_rgb: [f32; 3],
    pub poison_rgb: [f32; 3],
    pub explosive_rgb: [f32; 3],
    pub armor_pierce_rgb: [f32; 3],
}

impl Default for VfxParams {
    fn default() -> Self {
        // Miroir EXACT de la section [vfx] de roguelite_elements.toml.
        Self {
            enabled: true,
            impact_scale: 0.55,
            impact_ttl: 0.28,
            explosive_scale: 2.4,
            light_intensity: 40_000.0,
            light_range: 6.0,
            dot_pulse_scale: 0.3,
            dot_pulse_period: 0.45,
            fire_rgb: [1.0, 0.42, 0.06],
            poison_rgb: [0.42, 1.0, 0.16],
            explosive_rgb: [1.0, 0.82, 0.12],
            armor_pierce_rgb: [0.30, 0.85, 1.0],
        }
    }
}

/// Config gameplay des éléments. Resource + parsée du TOML (mtime hot-reload).
#[derive(Resource, Deserialize, Clone, Debug, PartialEq)]
pub struct ElementConfig {
    /// Phase A : tous les éléments actifs par défaut (test arène). Phase B :
    /// passera à un débloquage par-arme via le choix 1-parmi-3 au portail.
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
}

impl Default for ElementConfig {
    fn default() -> Self {
        // Miroir EXACT de assets/genomes/roguelite/roguelite_elements.toml.
        Self {
            always_on: true,
            mapping: WeaponMapping {
                modern_ar: "explosive".into(),
                assault_rifle: "fire".into(),
                shotgun: "armor_pierce".into(),
                rocket_launcher: "poison".into(),
            },
            matchup: MatchupTable {
                fire: Matchup { tank: 1.0, runner: 1.3, sniper: 1.1, boss: 1.0 },
                poison: Matchup { tank: 1.4, runner: 1.0, sniper: 1.1, boss: 1.2 },
                explosive: Matchup { tank: 1.1, runner: 1.4, sniper: 1.2, boss: 1.0 },
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

// ─── Stats sensor ───────────────────────────────────────────────────────────

#[derive(Resource, Default, Debug, Clone)]
pub struct ElementStats {
    pub hits_fire: u32,
    pub hits_poison: u32,
    pub hits_explosive: u32,
    pub hits_armor_pierce: u32,
    pub burns_applied: u32,
    pub poisons_applied: u32,
    pub aoe_hits: u32,
    pub executes: u32,
}

impl ElementStats {
    fn record_hit(&mut self, e: Element) {
        match e {
            Element::Fire => self.hits_fire = self.hits_fire.saturating_add(1),
            Element::Poison => self.hits_poison = self.hits_poison.saturating_add(1),
            Element::Explosive => self.hits_explosive = self.hits_explosive.saturating_add(1),
            Element::ArmorPierce => {
                self.hits_armor_pierce = self.hits_armor_pierce.saturating_add(1)
            }
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

// ─── System : application des éléments au hit ───────────────────────────────

/// Décision PURE (testable headless) d'un hit élémentaire sur la cible. `cur_hp`
/// = PV de la cible APRÈS le hit de base (déjà soustrait par forgia-fps).
/// Retourne `(nouveaux_pv, exécuté)`. Exécution = un hit `ArmorPierce` qui amène
/// une cible **survivante** au hit sous le seuil de PV → instakill (one-shot tank).
pub fn resolve_target_hit(
    element: Element,
    cur_hp: f32,
    max_hp: f32,
    base_damage: f32,
    matchup: f32,
    shred_amp: f32,
    execute_threshold: f32,
) -> (f32, bool) {
    let bonus = (base_damage * (matchup * shred_amp - 1.0)).max(0.0);
    let survives = cur_hp - bonus;
    if element == Element::ArmorPierce && survives > 0.0 && survives < execute_threshold * max_hp {
        (0.0, true)
    } else {
        (survives.max(0.0), false)
    }
}

/// Lit `CombatHitEvent` (le hit de base est DÉJÀ appliqué par forgia-fps) et
/// ajoute la couche élémentaire sur `forgia_combat::Health` :
/// - **bonus de matchup** (× selon archetype, ampli par shred poison),
/// - **exécution** perforante (instakill sous seuil),
/// - **status** burn/poison,
/// - **AOE** explosif autour du point d'impact.
#[allow(clippy::too_many_arguments)]
pub fn sys_apply_elements_on_hit(
    mut events: MessageReader<CombatHitEvent>,
    config: Res<ElementConfig>,
    mut commands: Commands,
    mut stats: ResMut<ElementStats>,
    q_archetype: Query<&EnemyArchetype>,
    mut q_health: Query<&mut Health, With<EnemyArchetype>>,
    q_pos: Query<(Entity, &GlobalTransform), With<EnemyArchetype>>,
    mut q_poison: Query<&mut StatusPoison>,
    // Buffer AOE réutilisé (0 alloc dans le chemin combat — règle scalability §hot).
    mut aoe_buf: Local<Vec<Entity>>,
) {
    if !config.always_on {
        return;
    }
    for ev in events.read() {
        let Some(weapon) = ev.weapon else {
            continue;
        };
        let Some(element) = config.element_for(weapon) else {
            continue;
        };
        let Ok(archetype) = q_archetype.get(ev.target).copied() else {
            continue;
        };
        stats.record_hit(element);

        // Effets sur la CIBLE — seulement si le hit de base ne l'a pas DÉJÀ tuée
        // (`ev.is_kill`). Sinon : `try_insert` sur entité en cours de despawn +
        // stats faussées. Un bonus/exécution qui amène la cible à 0 PV est balayé
        // par `despawn_dead_cubes` (sweep par frame, forgia-fps) → DeathEvent/loot.
        if !ev.is_kill {
            let matchup = config.matchup_for(element, archetype);
            // Shred : tant que des stacks de poison sont actifs, le bonus est amplifié.
            let shred_amp = q_poison
                .get(ev.target)
                .map(|p| 1.0 + p.stacks as f32 * config.poison.shred_per_stack)
                .unwrap_or(1.0);

            if let Ok(mut hp) = q_health.get_mut(ev.target) {
                let (new_hp, executed) = resolve_target_hit(
                    element,
                    hp.current,
                    hp.max,
                    ev.damage,
                    matchup,
                    shred_amp,
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
                Element::Explosive | Element::ArmorPierce => {}
            }
        }

        // AOE explosif — touche les VOISINS, indépendant de la mort de la cible
        // (un tir qui tue doit quand même produire son splash). Collecte d'abord
        // (q_pos immutable) puis applique (q_health mutable) ; buffer réutilisé.
        if element == Element::Explosive {
            let origin = ev.hit_world_pos;
            let r2 = config.aoe.radius * config.aoe.radius;
            let splash = (ev.damage * config.aoe.damage_factor).max(0.0);
            aoe_buf.clear();
            aoe_buf.extend(q_pos.iter().filter_map(|(e, gt)| {
                (e != ev.target && (gt.translation() - origin).length_squared() <= r2)
                    .then_some(e)
            }));
            let mut hits = 0u32;
            for &e in &*aoe_buf {
                if let Ok(mut hp) = q_health.get_mut(e) {
                    hp.current = (hp.current - splash).max(0.0);
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
    mut q: Query<
        (
            Entity,
            &mut Health,
            Option<&mut StatusBurn>,
            Option<&mut StatusPoison>,
        ),
        With<EnemyArchetype>,
    >,
) {
    let dt = time.delta_secs();
    for (e, mut hp, burn, poison) in &mut q {
        let mut total = 0.0_f32;

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

        if total > 0.0 {
            hp.current = (hp.current - total).max(0.0);
        }
    }
}

// ─── Sensor forgia2_elements.json ───────────────────────────────────────────

/// Écrit `forgia2_elements.json` 1Hz : mapping par arme, hits par élément, DoT
/// actifs, executes. Severity `warn` si `always_on=0` (aucun élément actif).
pub fn sys_write_elements_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    config: Res<ElementConfig>,
    stats: Res<ElementStats>,
    q_burn: Query<(), With<StatusBurn>>,
    q_poison: Query<&StatusPoison>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;

    let active_burns = q_burn.iter().count();
    let active_poisons = q_poison.iter().count();
    let active_stacks: u32 = q_poison.iter().map(|p| p.stacks).sum();

    let (severity, next_step) = if config.always_on {
        ("ok", "")
    } else {
        (
            "warn",
            "always_on=0 — aucun élément actif (set 1 dans roguelite_elements.toml)",
        )
    };

    let json = format!(
        r#"{{"id":"elements","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"always_on":{},"mapping":{{"pistol":"{}","smg":"{}","sniper":"{}","pompe":"{}"}},"hits":{{"fire":{},"poison":{},"explosive":{},"armor_pierce":{}}},"burns_applied":{},"poisons_applied":{},"aoe_hits":{},"executes":{},"active_burns":{active_burns},"active_poisons":{active_poisons},"active_poison_stacks":{active_stacks}}}"#,
        time.elapsed_secs(),
        config.always_on,
        config.mapping.modern_ar,
        config.mapping.assault_rifle,
        config.mapping.shotgun,
        config.mapping.rocket_launcher,
        stats.hits_fire,
        stats.hits_poison,
        stats.hits_explosive,
        stats.hits_armor_pierce,
        stats.burns_applied,
        stats.poisons_applied,
        stats.aoe_hits,
        stats.executes,
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
        assert_eq!(c.element_for(WeaponType::ModernAR), Some(Element::Explosive));
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
    fn fire_strongest_vs_runner_explosive_too() {
        let c = ElementConfig::default();
        assert!(
            c.matchup_for(Element::Fire, EnemyArchetype::Runner)
                > c.matchup_for(Element::Fire, EnemyArchetype::Tank)
        );
        assert!(
            c.matchup_for(Element::Explosive, EnemyArchetype::Runner)
                > c.matchup_for(Element::Explosive, EnemyArchetype::Boss)
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
        assert_eq!(Element::from_key("EXPLOSIVE"), Some(Element::Explosive));
        assert_eq!(Element::from_key("perforant"), Some(Element::ArmorPierce));
        assert_eq!(Element::from_key("inconnu"), None);
    }

    #[test]
    fn parse_garbage_falls_back_to_default() {
        let c = ElementConfig::parse_toml("ceci n'est pas du TOML valide [[[");
        assert_eq!(c, ElementConfig::default());
    }

    #[test]
    fn default_is_always_on() {
        assert!(ElementConfig::default().always_on);
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
        let (hp, exec) = resolve_target_hit(Element::ArmorPierce, 70.0, 120.0, 50.0, 2.0, 1.0, 0.25);
        assert!(exec, "perforant doit exécuter un Tank affaibli sous le seuil");
        assert_eq!(hp, 0.0);
    }

    #[test]
    fn armor_pierce_does_not_execute_boss() {
        // Boss 800 PV, body 50 → cur 750, matchup boss 1.5 → bonus 25 → 725 ≫ 200.
        let (hp, exec) = resolve_target_hit(Element::ArmorPierce, 750.0, 800.0, 50.0, 1.5, 1.0, 0.25);
        assert!(!exec, "perforant ne doit PAS one-shot le Boss");
        assert!((hp - 725.0).abs() < 1e-3);
    }

    #[test]
    fn fire_applies_matchup_bonus_no_execute() {
        // Runner, base 16 (SMG), matchup fire×runner 1.3 → bonus 4.8 → 30 → 25.2.
        let (hp, exec) = resolve_target_hit(Element::Fire, 30.0, 35.0, 16.0, 1.3, 1.0, 0.25);
        assert!(!exec, "seul ArmorPierce exécute");
        assert!((hp - 25.2).abs() < 1e-3);
    }

    #[test]
    fn neutral_matchup_is_noop() {
        let (hp, exec) = resolve_target_hit(Element::Poison, 50.0, 100.0, 20.0, 1.0, 1.0, 0.25);
        assert!(!exec);
        assert_eq!(hp, 50.0, "matchup 1.0 = aucun bonus");
    }

    #[test]
    fn poison_shred_amplifies_bonus() {
        // shred_amp 1.2 (5 stacks ×0.04) × poison×tank 1.4 = 1.68.
        // base 18, bonus = 18×(1.68−1)=12.24, cur 100 → 87.76.
        let (hp, _) = resolve_target_hit(Element::Poison, 100.0, 120.0, 18.0, 1.4, 1.2, 0.25);
        assert!((hp - 87.76).abs() < 1e-2);
    }

    // ── VFX (story-588) ──

    #[test]
    fn element_idx_is_stable_and_distinct() {
        let idx: Vec<usize> = [
            Element::Fire,
            Element::Poison,
            Element::Explosive,
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
        assert_eq!(Element::Explosive.rgb(&v), v.explosive_rgb);
        assert_eq!(Element::ArmorPierce.rgb(&v), v.armor_pierce_rgb);
    }

    #[test]
    fn vfx_default_is_enabled_and_sane() {
        let v = VfxParams::default();
        assert!(v.enabled);
        assert!(v.impact_scale > 0.0 && v.impact_ttl > 0.0);
        assert!(v.explosive_scale > 1.0, "le splash explosif doit être plus gros");
        assert!(v.dot_pulse_period > 0.0);
    }

    #[test]
    fn config_vfx_field_defaults_when_section_absent() {
        // Un TOML Phase A (sans [vfx]) doit parser et obtenir le VfxParams par défaut.
        let toml = r#"
always_on = true
[mapping]
modern_ar = "explosive"
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
[matchup.explosive]
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
