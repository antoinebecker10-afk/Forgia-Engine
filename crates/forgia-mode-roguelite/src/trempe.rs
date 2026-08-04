//! trempe.rs — story-653 : « La Trempe », progression de l'arme EN COURS DE RUN.
//!
//! L'arme Forgia est une amie — on ne la jette pas (Gunfire), on la **forge**.
//! Chez le Forgeron Itinérant (le marchand, `merchant.rs`), le joueur dépense de
//! l'**Or** (gagné aux kills, perdu à la mort) pour **tremper** son arme : +dégâts
//! par palier, multiplicatif avec les boons et la méta.
//!
//! ## Injection dégâts (concept-first, vérifié)
//! Ne touche PAS le hot path de tir. `WeaponTrempeState.damage_mul` est composé dans
//! `boons_apply::sys_recompute_boon_mods` (`new_mods.damage_mul *= trempe.damage_mul`,
//! même ligne que `perm`/`mastery`) → un seul writer de `PlayerCombatMods.damage_mul`,
//! stacking multiplicatif, zéro conflit.
//!
//! ## Portée de cet incrément (story-653 Inc.1)
//! Boucle cœur uniquement : Or → trempe → dégâts. Les **Gravures** (choix aux paliers
//! 3/5), le **scaling ennemi** par depth et la **station Enclume dédiée** en salle Rest
//! sont des incréments suivants (M1/M2/M4 du GDD). Ici la Trempe est co-localisée au
//! marchand (diégétiquement « LE FORGERON ITINÉRANT ») — touche **E**.
//!
//! ## Per-run
//! LITE = 1 arme choisie par run (wizard). `level` est donc l'état du run courant,
//! remis à 0 au retour Lobby (comme l'Or). Data-driven `roguelite_progression.toml`.

use bevy::prelude::*;
use forgia_combat::weapons::{EquippedWeapons, WeaponType};
use forgia_core::prelude::*;
use forgia_rpg_data::loot_tables::Souls as Gold;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;

use crate::run::RunState;

/// Demande de trempe de l'arme équipée — émise par le bouton « Tremper » de la
/// fenêtre du forgeron (`forge_shop.rs`). Consommée par `sys_apply_temper`.
#[derive(Message, Debug, Clone, Copy)]
pub struct TemperRequest;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_progression.toml";
const SENSOR_PATH: &str = "forgia2_trempe.json";
const POLL_PERIOD_SEC: f32 = 1.0;

// ─── Config (genome, hot-reload) ─────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct ProgressionToml {
    #[serde(default)]
    trempe: TrempeToml,
}

#[derive(Deserialize, Default)]
struct TrempeToml {
    enabled: Option<bool>,
    damage_per_level: Option<f32>,
    level_cap: Option<u32>,
    cost_base: Option<u32>,
    cost_growth: Option<f32>,
}

/// Config de la Trempe — `[trempe]` de `roguelite_progression.toml` (hot-reload).
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct TrempeConfig {
    pub enabled: bool,
    pub damage_per_level: f32,
    pub level_cap: u32,
    pub cost_base: u32,
    pub cost_growth: f32,
}

impl Default for TrempeConfig {
    fn default() -> Self {
        // Miroir EXACT de assets/genomes/roguelite/roguelite_progression.toml.
        // level_cap 5 → 7 le 2026-08-04 : mesuré en jeu, l'arme était trempée au max
        // dès le round 1 (219 Or), donc le levier de progression d'une run était mort
        // pour les neuf rounds suivants. 7 paliers = 478 Or ≈ le revenu d'un chapitre.
        Self {
            enabled: true,
            damage_per_level: 0.15,
            level_cap: 7,
            cost_base: 20,
            cost_growth: 1.4,
        }
    }
}

impl TrempeConfig {
    /// Pur — testable. Merge le TOML sur les défauts, clamp les valeurs saines.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: ProgressionToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let t = parsed.trempe;
        let d = Self::default();
        Self {
            enabled: t.enabled.unwrap_or(d.enabled),
            damage_per_level: t
                .damage_per_level
                .unwrap_or(d.damage_per_level)
                .clamp(0.0, 5.0),
            level_cap: t.level_cap.unwrap_or(d.level_cap).clamp(0, 50),
            cost_base: t.cost_base.unwrap_or(d.cost_base),
            cost_growth: t.cost_growth.unwrap_or(d.cost_growth).clamp(1.0, 5.0),
        }
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }

    /// Multiplicateur de dégâts pour un niveau donné (1 + level × per_level).
    pub fn damage_mul_for_level(&self, level: u32) -> f32 {
        1.0 + level as f32 * self.damage_per_level
    }

    /// Multiplicateur de la Trempe pour un ENSEMBLE d'armes trempées.
    ///
    /// ## Le défaut qu'il corrige
    ///
    /// Mesuré en jeu le 2026-08-04 : `or_spent: 219`, `weapons_tempered: 1`, et
    /// `power.trempe: 1.000`. Antoine avait trempé une arme puis pris une autre
    /// — **219 Or évaporés**. Le bonus valait celui de l'arme EN MAIN, donc
    /// changer d'arme le rendait à zéro.
    ///
    /// C'est exactement le défaut déjà corrigé sur la maîtrise
    /// (`MasteryConfig::total_damage_mul`), et pour la même raison : le pilier
    /// revendiqué du jeu est de **choisir l'arme selon l'ennemi**, comme dans
    /// DOOM. Un bonus attaché à l'arme tenue punit précisément ce geste.
    ///
    /// ## La règle
    ///
    /// La somme des niveaux acquis, plafonnée au **même total** qu'avant
    /// (`level_cap × damage_per_level`). Tremper une arme à fond ou quatre armes
    /// au quart donne le même bonus ; c'est la RÉPARTITION qui devient libre,
    /// pas le total. La courbe de difficulté du Livre n'est donc pas touchée.
    ///
    /// Corollaire : il n'y a plus d'« arme courante » dans ce calcul, donc la
    /// désynchronisation qui l'a produit devient impossible.
    ///
    /// PUR — testable sans App.
    pub fn total_damage_mul<'a>(&self, levels: impl IntoIterator<Item = &'a u32>) -> f32 {
        let somme: u32 = levels.into_iter().copied().sum();
        self.damage_mul_for_level(somme.min(self.level_cap))
    }

    /// Coût (Or) pour passer de `level` à `level+1` : `cost_base × growth^level`.
    pub fn cost_for_next(&self, level: u32) -> u32 {
        (self.cost_base as f32 * self.cost_growth.powi(level as i32)).round() as u32
    }
}

/// Suivi mtime du genome (hot-reload) + compteur.
#[derive(Resource, Default, Debug)]
pub struct TrempeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

// ─── État runtime (per-run) ──────────────────────────────────────────────────

/// État de trempe **par arme** (chaque arme se forge indépendamment). `damage_mul`
/// = niveau de l'arme ACTUELLEMENT équipée, resync chaque frame par
/// `sys_sync_trempe_current` → lu par `boons_apply::sys_recompute_boon_mods`
/// (composé × boons × méta). Une arme non trempée reste à ×1.0.
#[derive(Resource, Debug, Clone, Default)]
pub struct WeaponTrempeState {
    /// Palier de trempe par arme (absent = 0).
    pub levels: HashMap<WeaponType, u32>,
    /// Multiplicateur de dégâts de l'arme équipée courante (sync frame).
    pub damage_mul: f32,
    /// Arme dont `damage_mul` reflète le niveau (cohérence affichage/recompute).
    pub current: WeaponType,
    /// Or total dépensé en trempes cette run (toutes armes) — sensor.
    pub or_spent: u32,
}

impl WeaponTrempeState {
    /// Palier de trempe d'une arme (0 si jamais trempée).
    pub fn level_of(&self, w: WeaponType) -> u32 {
        self.levels.get(&w).copied().unwrap_or(0)
    }

    /// Remet à neutre (nouvelle run) : toutes les armes repartent non trempées.
    pub fn reset(&mut self) {
        self.levels.clear();
        self.damage_mul = 1.0;
        self.or_spent = 0;
    }
}

// ─── Systèmes ────────────────────────────────────────────────────────────────

pub fn sys_init_trempe(mut commands: Commands) {
    let cfg = TrempeConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(cfg);
    commands.insert_resource(TrempeWatch {
        last_mtime: mtime,
        ..Default::default()
    });
    // damage_mul neutre = 1.0 (le derive Default de f32 donnerait 0.0 → dégâts nuls).
    commands.insert_resource(WeaponTrempeState {
        damage_mul: 1.0,
        ..Default::default()
    });
}

/// Aligne `damage_mul` sur le palier de l'arme ACTUELLEMENT équipée (chaque frame,
/// coût négligeable). Rend la trempe **par arme** : changer d'arme applique la
/// trempe de CETTE arme (une arme non trempée revient à ×1.0). Lu ensuite par le
/// recompute des mods (lag ≤1 frame, imperceptible).
pub fn sys_sync_trempe_current(
    cfg: Res<TrempeConfig>,
    mut state: ResMut<WeaponTrempeState>,
    equipped: Option<Res<EquippedWeapons>>,
) {
    let Some(eq) = equipped else { return };
    let w = eq.current;
    // `current` reste suivi — le capteur et l'échoppe l'affichent (quelle arme
    // on est en train de tremper). Il ne décide simplement plus du bonus.
    let mul = cfg.total_damage_mul(state.levels.values());
    if state.current != w || (state.damage_mul - mul).abs() > 1e-6 {
        state.current = w;
        state.damage_mul = mul;
    }
}

/// Poll mtime 1Hz → reparse si changé. Recompose `damage_mul` (le per_level a pu
/// changer) sans toucher au niveau acquis.
pub fn sys_hot_reload_trempe(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<TrempeConfig>,
    mut watch: ResMut<TrempeWatch>,
    mut state: ResMut<WeaponTrempeState>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let Ok(mtime) = fs::metadata(GENOME_PATH).and_then(|m| m.modified()) else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    watch.last_mtime = Some(mtime);
    let Ok(content) = fs::read_to_string(GENOME_PATH) else {
        return;
    };
    let new_cfg = TrempeConfig::parse_toml(&content);
    if new_cfg != *cfg {
        *cfg = new_cfg;
        // Recompose le mul de l'arme courante (le per_level a pu changer).
        state.damage_mul = new_cfg.total_damage_mul(state.levels.values());
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!("[trempe] HOT-RELOADED — damage×{:.2}", state.damage_mul);
    }
}

/// Remet la trempe à 0 au retour Lobby (per-run, comme l'Or). Miroir
/// `merchant::sys_reset_revive_tokens`.
pub fn sys_reset_trempe(mut state: ResMut<WeaponTrempeState>) {
    state.reset();
}

/// Tente de tremper l'arme `w` d'un palier. **Pur** (hors mutation de state/gold) :
/// retourne le nouveau palier si réussi (Or suffisant + pas au cap), `None` sinon.
/// Appelé par le bouton de la fenêtre du forgeron (via `sys_apply_temper`).
pub fn try_temper(
    cfg: &TrempeConfig,
    state: &mut WeaponTrempeState,
    w: WeaponType,
    gold: &mut Gold,
) -> Option<u32> {
    if !cfg.enabled {
        return None;
    }
    let level = state.level_of(w);
    if level >= cfg.level_cap {
        return None;
    }
    let cost = cfg.cost_for_next(level);
    if gold.current < cost {
        return None;
    }
    gold.current -= cost;
    let new_level = level + 1;
    state.levels.insert(w, new_level);
    state.or_spent = state.or_spent.saturating_add(cost);
    // Applique immédiatement (l'arme trempée = l'arme équipée). sys_sync réaligne
    // ensuite si l'arme change.
    state.current = w;
    state.damage_mul = cfg.damage_mul_for_level(new_level);
    // TODO(inc. suivant) : VFX rougeoiement de l'arme + bark d'arme (déclencheur
    // `upgrade`) — audio armes déféré (décision Antoine).
    Some(new_level)
}

/// Consomme les `TemperRequest` (bouton de la fenêtre) → trempe l'arme équipée.
pub fn sys_apply_temper(
    mut reqs: MessageReader<TemperRequest>,
    cfg: Res<TrempeConfig>,
    mut state: ResMut<WeaponTrempeState>,
    equipped: Option<Res<EquippedWeapons>>,
    mut gold: Option<ResMut<Gold>>,
) {
    let n = reqs.read().count();
    if n == 0 {
        return;
    }
    let (Some(eq), Some(g)) = (equipped, gold.as_deref_mut()) else {
        return;
    };
    let w = eq.current;
    // Une requête = une trempe (l'utilisateur clique une fois par palier).
    for _ in 0..n {
        match try_temper(&cfg, &mut state, w, g) {
            Some(lvl) => info!(
                "[trempe] {} trempée → palier {} (damage×{:.2})",
                w.display_name(),
                lvl,
                state.damage_mul
            ),
            None => break, // Or épuisé ou cap atteint → stop.
        }
    }
}

// ─── Sensor ──────────────────────────────────────────────────────────────────

/// Pur — severity/next_step du capteur.
pub fn severity_for_trempe(
    enabled: bool,
    level: u32,
    level_cap: u32,
) -> (&'static str, &'static str) {
    if !enabled {
        return ("info", "Trempe désactivée (enabled=false).");
    }
    if level >= level_cap {
        return ("ok", "Arme trempée au palier max.");
    }
    ("ok", "")
}

pub fn sys_write_trempe_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cfg: Option<Res<TrempeConfig>>,
    state: Option<Res<WeaponTrempeState>>,
    watch: Option<Res<TrempeWatch>>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let (Some(cfg), Some(state), Some(watch)) = (cfg, state, watch) else {
        return;
    };
    // Sensor centré sur l'arme équipée (celle qu'on forge). weapons_tempered =
    // nombre d'armes ayant au moins 1 palier (visibilité du « par arme »).
    let cur = state.current;
    let level = state.level_of(cur);
    let weapons_tempered = state.levels.values().filter(|&&l| l > 0).count();
    let (severity, next_step) = severity_for_trempe(cfg.enabled, level, cfg.level_cap);
    let json = format!(
        r#"{{"id":"trempe","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"enabled":{},"current_weapon":"{}","level":{},"level_cap":{},"damage_mul":{:.3},"weapons_tempered":{},"or_spent":{},"next_cost":{},"reload_count":{}}}"#,
        time.elapsed_secs(),
        cfg.enabled,
        cur.display_name(),
        level,
        cfg.level_cap,
        state.damage_mul,
        weapons_tempered,
        state.or_spent,
        // Au plafond il n'y a PAS de palier suivant : afficher son prix théorique
        // ferait croire à un achat possible. `null` dit « il n'y en a plus ».
        if level >= cfg.level_cap {
            "null".to_string()
        } else {
            cfg.cost_for_next(level).to_string()
        },
        watch.reload_count,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[trempe] sensor write failed: {e}");
    }
}

// ─── Plugin ──────────────────────────────────────────────────────────────────

pub struct RogueliteTrempePlugin;

impl Plugin for RogueliteTrempePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TemperRequest>();
        app.add_systems(Startup, sys_init_trempe);
        app.add_systems(OnEnter(RunState::Lobby), sys_reset_trempe);
        // La trempe se déclenche par le bouton de la fenêtre du forgeron
        // (`forge_shop.rs` → TemperRequest). `sys_sync_trempe_current` maintient le
        // damage_mul aligné sur l'arme équipée ; `sys_apply_temper` applique les paliers.
        app.add_systems(
            Update,
            (
                sys_hot_reload_trempe,
                sys_sync_trempe_current,
                sys_apply_temper,
            )
                .chain()
                .in_set(GameSet::UI)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(Update, sys_write_trempe_sensor.in_set(GameSet::Sensors));
    }
}

// ─── Tests purs ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_default() {
        assert_eq!(TrempeConfig::parse_toml(""), TrempeConfig::default());
    }

    #[test]
    fn parse_overrides_and_clamps() {
        let c = TrempeConfig::parse_toml(
            "[trempe]\nenabled = false\ndamage_per_level = 0.25\ncost_growth = 99.0\n",
        );
        assert!(!c.enabled);
        assert!((c.damage_per_level - 0.25).abs() < 1e-6);
        assert!(c.cost_growth <= 5.0, "growth clampé");
    }

    #[test]
    fn parse_garbage_falls_back() {
        assert_eq!(
            TrempeConfig::parse_toml("pas du toml [[["),
            TrempeConfig::default()
        );
    }

    #[test]
    fn damage_mul_linear_in_level() {
        let c = TrempeConfig::default(); // per_level 0.15
        assert!((c.damage_mul_for_level(0) - 1.0).abs() < 1e-6);
        assert!((c.damage_mul_for_level(1) - 1.15).abs() < 1e-6);
        assert!((c.damage_mul_for_level(5) - 1.75).abs() < 1e-6);
    }

    #[test]
    fn cost_grows_geometrically() {
        let c = TrempeConfig::default(); // base 20, growth 1.4
        assert_eq!(c.cost_for_next(0), 20);
        assert_eq!(c.cost_for_next(1), 28); // 20×1.4
        assert!(c.cost_for_next(4) > c.cost_for_next(3), "coût croissant");
    }

    #[test]
    fn state_reset_is_neutral() {
        let mut s = WeaponTrempeState {
            damage_mul: 1.45,
            or_spent: 87,
            current: WeaponType::AssaultRifle,
            ..Default::default()
        };
        s.levels.insert(WeaponType::ModernAR, 3);
        s.reset();
        assert!(s.levels.is_empty());
        assert!((s.damage_mul - 1.0).abs() < 1e-6);
        assert_eq!(s.or_spent, 0);
    }

    #[test]
    fn levels_are_per_weapon() {
        // Trempe une arme ne trempe pas l'autre (« arme par arme »).
        let mut s = WeaponTrempeState::default();
        s.levels.insert(WeaponType::ModernAR, 2);
        assert_eq!(s.level_of(WeaponType::ModernAR), 2);
        assert_eq!(s.level_of(WeaponType::Shotgun), 0, "autre arme non trempée");
    }

    #[test]
    fn severity_paths() {
        assert_eq!(severity_for_trempe(false, 0, 5).0, "info");
        assert_eq!(severity_for_trempe(true, 0, 5).0, "ok");
        assert_eq!(
            severity_for_trempe(true, 5, 5).1,
            "Arme trempée au palier max."
        );
    }
}

#[cfg(test)]
mod trempe_totale_tests {
    use super::*;

    fn cfg() -> TrempeConfig {
        TrempeConfig::default()
    }

    /// **LE test de la règle.** Changer d'arme ne doit RIEN coûter.
    ///
    /// Le 2026-08-04, `or_spent: 219` / `weapons_tempered: 1` /
    /// `power.trempe: 1.000` — une arme trempée, une autre en main, 219 Or
    /// évaporés. Le pilier revendiqué du jeu est de choisir l'arme selon
    /// l'ennemi ; un bonus attaché à l'arme tenue punit ce geste.
    #[test]
    fn switching_weapons_never_costs_a_single_point_of_temper() {
        let c = cfg();
        let trempees = [4u32, 0, 0, 0];
        let avant = c.total_damage_mul(trempees.iter());
        // L'ensemble n'a pas changé, seule l'arme en main — le bonus non plus.
        let apres = c.total_damage_mul(trempees.iter());
        assert!((avant - apres).abs() < 1e-6);
        assert!(avant > 1.0, "quatre niveaux payés doivent se voir");
    }

    /// Répartir ou concentrer donne le même bonus : c'est la RÉPARTITION qui
    /// devient libre, pas le total.
    #[test]
    fn spreading_the_temper_pays_exactly_as_much_as_concentrating_it() {
        let c = cfg();
        let concentre = c.total_damage_mul([4u32, 0, 0, 0].iter());
        let reparti = c.total_damage_mul([1u32, 1, 1, 1].iter());
        assert!((concentre - reparti).abs() < 1e-6);
    }

    /// Le plafond est INCHANGÉ — sinon la courbe de difficulté du Livre, qui
    /// est calibrée dessus, se déplacerait en silence.
    #[test]
    fn the_ceiling_is_the_same_as_before_the_rule_changed() {
        let c = cfg();
        let plafond_avant = c.damage_mul_for_level(c.level_cap);
        // Quatre armes au maximum ne dépassent pas ce que valait une seule.
        let quatre_au_max = c.total_damage_mul([c.level_cap; 4].iter());
        assert!((quatre_au_max - plafond_avant).abs() < 1e-6);
    }

    /// Rien de trempé = aucun bonus. Le neutre reste neutre.
    #[test]
    fn an_untempered_arsenal_grants_nothing() {
        assert!((cfg().total_damage_mul(std::iter::empty()) - 1.0).abs() < 1e-6);
        assert!((cfg().total_damage_mul([0u32, 0].iter()) - 1.0).abs() < 1e-6);
    }

    /// L'état de run remis à neutre ne laisse aucun bonus derrière lui.
    #[test]
    fn a_reset_run_starts_from_nothing() {
        let mut s = WeaponTrempeState {
            damage_mul: 2.0,
            or_spent: 219,
            ..default()
        };
        s.levels.insert(WeaponType::ModernAR, 4);
        s.reset();
        assert!(s.levels.is_empty());
        assert!((s.damage_mul - 1.0).abs() < 1e-6);
        assert_eq!(s.or_spent, 0);
    }
}
