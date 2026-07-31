//! wave_comp.rs — composition de vague DÉRIVÉE (story-669).
//!
//! Remplace `waves::wave_composition(wave: u8)`, une table Rust en dur qui ne
//! recevait ni la salle, ni le type de salle, ni la graine. Quatre ruptures de la
//! boucle roguelite partageaient cette unique cause :
//!
//! | Rupture | Cause |
//! |---|---|
//! | les 3 salles de combat sont identiques | pas de `stage` |
//! | le choix de porte ne change rien | pas de `kind` (`room_kind` écrit, jamais lu) |
//! | les positions de spawn sont figées | pas de `run_seed` (graine constante) |
//! | la difficulté ne monte que par les PV | `difficulty_budget` calculé puis jeté |
//!
//! La dérivation :
//! ```text
//! count = round(base × densité(salle) × modificateur(type de salle)),  total >= 1
//! densité(salle) = budget_director(salle) / budget_director(0)
//! ```
//!
//! Tout vit en couche definition : `assets/genomes/roguelite/roguelite_waves.toml`
//! (hot-reload 1 Hz). Les `Default` Rust en sont le miroir exact.

use bevy::prelude::*;
use forgia_stage::graph::StageKind;
use serde::Deserialize;
use std::fs;
use std::time::SystemTime;

use crate::enemies::EnemyArchetype;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_waves.toml";
const POLL_PERIOD_SEC: f32 = 1.0;

/// Effectifs de référence d'une vague (salle 0, type Combat).
/// `Default` = tout à zéro : un archétype non listé dans une section `[base.*]`
/// n'apparaît simplement pas dans la vague.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct BaseCounts {
    pub tank: u32,
    pub runner: u32,
    pub sniper: u32,
    pub boss: u32,
}

impl BaseCounts {
    fn get(&self, a: EnemyArchetype) -> u32 {
        match a {
            EnemyArchetype::Tank => self.tank,
            EnemyArchetype::Runner => self.runner,
            EnemyArchetype::Sniper => self.sniper,
            EnemyArchetype::Boss => self.boss,
        }
    }
}

/// Multiplicateurs par archétype pour un type de salle. 1.0 = référence.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default)]
pub struct KindMods {
    pub tank: f32,
    pub runner: f32,
    pub sniper: f32,
    pub boss: f32,
    /// `false` = salle SANS COMBAT (story-670). La composition est alors vide et
    /// l'orchestrateur clôt la salle immédiatement, au lieu d'attendre des ennemis
    /// qui n'arriveront jamais.
    ///
    /// Le plancher d'1 ennemi ne s'applique qu'aux salles de COMBAT : c'est un
    /// garde anti-run-figée, pas une raison de trahir le nom d'une salle. Une porte
    /// « Repos » qui donne un combat rompt le contrat du nom
    /// (`map-design-intention.md` §5.1) — constaté en jeu le 2026-07-31.
    pub spawns_enemies: bool,
}

impl Default for KindMods {
    fn default() -> Self {
        Self {
            tank: 1.0,
            runner: 1.0,
            sniper: 1.0,
            boss: 1.0,
            spawns_enemies: true,
        }
    }
}

impl KindMods {
    fn get(&self, a: EnemyArchetype) -> f32 {
        match a {
            EnemyArchetype::Tank => self.tank,
            EnemyArchetype::Runner => self.runner,
            EnemyArchetype::Sniper => self.sniper,
            // Le boss n'est jamais multiplié par un type de salle : sa salle EST
            // la salle boss, et 0 boss serait une run sans fin.
            EnemyArchetype::Boss => 1.0,
        }
    }
}

/// Rayons d'anneau de spawn + dispersion.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default)]
pub struct RingCfg {
    pub tank: f32,
    pub runner: f32,
    pub sniper: f32,
    pub boss: f32,
    /// La vague 2 est plus dense → anneaux élargis d'autant (m).
    pub wave2_bonus_m: f32,
    /// Dispersion du rayon (m, ±), tirée de la graine de RUN. 0 = anneaux parfaits.
    pub jitter_m: f32,
}

impl Default for RingCfg {
    fn default() -> Self {
        Self {
            tank: 12.0,
            runner: 25.0,
            sniper: 50.0,
            boss: 12.0,
            wave2_bonus_m: 2.5,
            jitter_m: 2.0,
        }
    }
}

impl RingCfg {
    fn radius(&self, a: EnemyArchetype, wave: u8) -> f32 {
        let base = match a {
            EnemyArchetype::Tank => self.tank,
            EnemyArchetype::Runner => self.runner,
            EnemyArchetype::Sniper => self.sniper,
            EnemyArchetype::Boss => self.boss,
        };
        if wave == 2 {
            base + self.wave2_bonus_m
        } else {
            base
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default)]
pub struct DensityCfg {
    pub enabled: bool,
    /// Garde-fou de budget de frame : une salle ne dépasse jamais ce facteur.
    pub max_factor: f32,
}

impl Default for DensityCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            max_factor: 2.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default)]
struct BaseTable {
    wave1: BaseCounts,
    wave2: BaseCounts,
    boss: BaseCounts,
}

impl Default for BaseTable {
    /// Miroir EXACT de l'ancienne table Rust : 8 puis 12 ennemis, boss + 4 Runners.
    fn default() -> Self {
        Self {
            wave1: BaseCounts {
                tank: 3,
                runner: 3,
                sniper: 2,
                boss: 0,
            },
            wave2: BaseCounts {
                tank: 4,
                runner: 4,
                sniper: 4,
                boss: 0,
            },
            boss: BaseCounts {
                tank: 0,
                runner: 4,
                sniper: 0,
                boss: 1,
            },
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct KindTable {
    combat: Option<KindMods>,
    elite: Option<KindMods>,
    event: Option<KindMods>,
    shop: Option<KindMods>,
    rest: Option<KindMods>,
    treasure: Option<KindMods>,
}

/// Configuration complète de composition (Resource, hot-reload).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct WaveCompConfig {
    base: BaseTable,
    pub ring: RingCfg,
    pub density: DensityCfg,
    kinds: KindTable,
}

impl PartialEq for KindTable {
    fn eq(&self, o: &Self) -> bool {
        self.combat == o.combat
            && self.elite == o.elite
            && self.event == o.event
            && self.shop == o.shop
            && self.rest == o.rest
            && self.treasure == o.treasure
    }
}

impl Default for WaveCompConfig {
    /// Miroir EXACT de `roguelite_waves.toml`.
    fn default() -> Self {
        Self {
            base: BaseTable::default(),
            ring: RingCfg::default(),
            density: DensityCfg::default(),
            kinds: KindTable {
                combat: Some(KindMods::default()),
                elite: Some(KindMods {
                    tank: 1.7,
                    runner: 0.4,
                    sniper: 1.0,
                    boss: 1.0,
                    spawns_enemies: true,
                }),
                event: Some(KindMods {
                    tank: 0.5,
                    runner: 1.8,
                    sniper: 0.6,
                    boss: 1.0,
                    spawns_enemies: true,
                }),
                shop: Some(KindMods {
                    tank: 0.5,
                    runner: 0.5,
                    sniper: 0.5,
                    boss: 1.0,
                    spawns_enemies: true,
                }),
                // Story-670 — le Repos ne fait PAS combattre. C'est le battement du
                // genre (feu de camp Slay the Spire, fontaine d'Hadès).
                rest: Some(KindMods {
                    tank: 0.0,
                    runner: 0.0,
                    sniper: 0.0,
                    boss: 1.0,
                    spawns_enemies: false,
                }),
                treasure: Some(KindMods {
                    tank: 0.6,
                    runner: 0.6,
                    sniper: 1.6,
                    boss: 1.0,
                    spawns_enemies: true,
                }),
            },
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CompToml {
    base: Option<BaseTable>,
    ring: Option<RingCfg>,
    density: Option<DensityCfg>,
    room_kind: Option<KindTable>,
}

impl WaveCompConfig {
    /// PUR — testable. Fallback PAR CHAMP : un genome partiel ne perd pas le reste.
    pub fn parse_toml(content: &str) -> Self {
        let parsed = match toml::from_str::<CompToml>(content) {
            Ok(p) => p,
            Err(e) => {
                // Un fallback muet serait indiscernable d'un chargement réussi.
                warn!("[wave-comp] genome illisible ({e}) — MIROIR RUST utilisé");
                return Self::default();
            }
        };
        let d = Self::default();
        Self {
            base: parsed.base.unwrap_or(d.base),
            ring: parsed.ring.unwrap_or(d.ring),
            density: parsed.density.unwrap_or(d.density),
            kinds: parsed.room_kind.unwrap_or(d.kinds),
        }
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(c) => Self::parse_toml(&c),
            Err(e) => {
                warn!("[wave-comp] genome {GENOME_PATH} illisible ({e}) — miroir Rust utilisé");
                Self::default()
            }
        }
    }

    fn base_for(&self, wave: u8) -> BaseCounts {
        match wave {
            1 => self.base.wave1,
            2 => self.base.wave2,
            _ => self.base.boss,
        }
    }

    /// Modificateurs du type de salle. `None` (graph absent) ou type non déclaré
    /// → neutre : on ne change pas silencieusement l'équilibre par défaut.
    fn mods_for(&self, kind: Option<StageKind>) -> KindMods {
        let t = &self.kinds;
        match kind {
            Some(StageKind::Combat) => t.combat,
            Some(StageKind::Elite) => t.elite,
            Some(StageKind::Event) => t.event,
            Some(StageKind::Shop) => t.shop,
            Some(StageKind::Rest) => t.rest,
            Some(StageKind::Treasure) => t.treasure,
            // Le boss ne se module pas.
            Some(StageKind::Boss) | None => None,
        }
        .unwrap_or_default()
    }

    /// Facteur de densité borné, ou 1.0 si la densité est désactivée.
    pub fn density_factor(&self, raw: f32) -> f32 {
        if !self.density.enabled || !raw.is_finite() || raw <= 0.0 {
            return 1.0;
        }
        raw.clamp(1.0, self.density.max_factor.max(1.0))
    }
}

/// Une ligne de composition : archétype, effectif, rayon d'anneau (m).
pub type CompLine = (EnemyArchetype, u32, f32);

/// PUR — facteur de densité d'une salle depuis son budget de director.
///
/// `room_budget` vient de `StageNode.difficulty_budget` (le champ que story-669 a
/// enfin branché) ; `base_budget` est celui de la salle 0, la référence. Repli sur
/// 1.0 quand le graph est absent : on ne change jamais l'équilibre en silence.
pub fn density_from_budget(room_budget: u32, base_budget: u32) -> f32 {
    if room_budget == 0 || base_budget == 0 {
        return 1.0;
    }
    room_budget as f32 / base_budget as f32
}

/// Une salle de ce type fait-elle combattre ? `None` (graph absent) = oui, par
/// prudence : on ne transforme jamais une salle en salle vide par accident.
pub fn room_spawns_enemies(cfg: &WaveCompConfig, kind: Option<StageKind>) -> bool {
    kind.is_none() || cfg.mods_for(kind).spawns_enemies
}

/// Ordre STABLE d'itération — le spawn doit être déterministe à graine égale.
const ARCHETYPE_ORDER: [EnemyArchetype; 4] = [
    EnemyArchetype::Tank,
    EnemyArchetype::Runner,
    EnemyArchetype::Sniper,
    EnemyArchetype::Boss,
];

/// PUR — la composition d'une vague. C'est LA fonction que story-669 a ouverte :
/// elle reçoit enfin la salle (via `density`) et le type de salle (via `kind`).
///
/// Invariant non négociable : le total est **toujours >= 1**. Une vague à 0 ennemi
/// ne déclencherait jamais `seen_alive`, donc la salle ne se nettoierait jamais et
/// la run se figerait (`waves::clear_detection_armed`).
pub fn compose(cfg: &WaveCompConfig, wave: u8, kind: Option<StageKind>, density: f32) -> Vec<CompLine> {
    let base = cfg.base_for(wave);
    let mods = cfg.mods_for(kind);
    // Salle sans combat : rien à spawner, et surtout PAS de plancher.
    if !mods.spawns_enemies {
        return Vec::new();
    }
    let d = cfg.density_factor(density);

    let mut out: Vec<CompLine> = Vec::with_capacity(ARCHETYPE_ORDER.len());
    let mut total = 0u32;
    for a in ARCHETYPE_ORDER {
        let b = base.get(a);
        if b == 0 {
            continue;
        }
        // Le boss n'est ni densifié ni modulé : il est unique par construction.
        let count = if matches!(a, EnemyArchetype::Boss) {
            b
        } else {
            ((b as f32) * d * mods.get(a)).round().max(0.0) as u32
        };
        total += count;
        if count > 0 {
            out.push((a, count, cfg.ring.radius(a, wave)));
        }
    }

    if total == 0 {
        // Plancher : on repeuple la ligne de base la plus fournie avec 1 ennemi.
        // Sans ça, la salle ne se nettoie jamais et la run se fige.
        let fallback = ARCHETYPE_ORDER
            .iter()
            .copied()
            .max_by_key(|a| base.get(*a))
            .unwrap_or(EnemyArchetype::Runner);
        out.push((fallback, 1, cfg.ring.radius(fallback, wave)));
    }
    out
}

// ─── Systems : load + hot-reload ─────────────────────────────────────────────

#[derive(Resource, Default, Debug)]
pub struct WaveCompWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

pub fn sys_init_wave_comp_genome(mut commands: Commands) {
    let cfg = WaveCompConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    info!(
        "[wave-comp] genome chargé — densité {} (max ×{:.2}), jitter {:.1} m",
        if cfg.density.enabled { "ON" } else { "OFF" },
        cfg.density.max_factor,
        cfg.ring.jitter_m,
    );
    commands.insert_resource(cfg);
    commands.insert_resource(WaveCompWatch {
        last_mtime: mtime,
        reload_count: 0,
    });
}

/// Poll mtime 1 Hz — `genome-code.md` : « tout gene DOIT fonctionner avec Shift+F12 ».
pub fn sys_hot_reload_wave_comp_genome(
    time: Res<Time<Real>>,
    mut cfg: ResMut<WaveCompConfig>,
    mut watch: ResMut<WaveCompWatch>,
    mut cooldown: Local<f32>,
) {
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = POLL_PERIOD_SEC;
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
    let next = WaveCompConfig::parse_toml(&content);
    if next == *cfg {
        return;
    }
    *cfg = next;
    watch.reload_count = watch.reload_count.saturating_add(1);
    info!(
        "[wave-comp] genome HOT-RELOADED (#{}) — prend effet à la PROCHAINE vague",
        watch.reload_count
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(lines: &[CompLine]) -> u32 {
        lines.iter().map(|(_, c, _)| *c).sum()
    }

    #[test]
    fn defaults_reproduce_the_historical_table() {
        let c = WaveCompConfig::default();
        // Salle 0, type Combat → l'équilibre d'avant story-669, à l'unité près.
        assert_eq!(total(&compose(&c, 1, Some(StageKind::Combat), 1.0)), 8);
        assert_eq!(total(&compose(&c, 2, Some(StageKind::Combat), 1.0)), 12);
        assert_eq!(total(&compose(&c, 3, None, 1.0)), 5);
    }

    #[test]
    fn the_door_choice_now_changes_the_fight() {
        let c = WaveCompConfig::default();
        let combat = compose(&c, 1, Some(StageKind::Combat), 1.0);
        let elite = compose(&c, 1, Some(StageKind::Elite), 1.0);
        let event = compose(&c, 1, Some(StageKind::Event), 1.0);
        assert_ne!(combat, elite, "Élite doit différer de Combat");
        assert_ne!(combat, event, "Événement doit différer de Combat");
        assert_ne!(elite, event, "Élite et Événement doivent différer entre eux");

        let tanks = |l: &[CompLine]| {
            l.iter()
                .find(|(a, _, _)| matches!(a, EnemyArchetype::Tank))
                .map(|(_, c, _)| *c)
                .unwrap_or(0)
        };
        let runners = |l: &[CompLine]| {
            l.iter()
                .find(|(a, _, _)| matches!(a, EnemyArchetype::Runner))
                .map(|(_, c, _)| *c)
                .unwrap_or(0)
        };
        assert!(tanks(&elite) > tanks(&combat), "Élite = mur de tanks");
        assert!(runners(&event) > runners(&combat), "Événement = essaim");
    }

    #[test]
    fn density_grows_with_depth_and_is_bounded() {
        let c = WaveCompConfig::default();
        let s0 = total(&compose(&c, 1, Some(StageKind::Combat), 1.0));
        let s1 = total(&compose(&c, 1, Some(StageKind::Combat), 1.25));
        let s2 = total(&compose(&c, 1, Some(StageKind::Combat), 1.5625));
        assert!(s0 < s1 && s1 <= s2, "la densité monte avec la profondeur");
        // Garde-fou de budget de frame.
        let insane = total(&compose(&c, 1, Some(StageKind::Combat), 100.0));
        let capped = total(&compose(&c, 1, Some(StageKind::Combat), c.density.max_factor));
        assert_eq!(insane, capped, "densité bornée par max_factor");
    }

    #[test]
    fn density_disabled_pins_every_room_to_the_reference() {
        let mut c = WaveCompConfig::default();
        c.density.enabled = false;
        assert_eq!(
            total(&compose(&c, 1, Some(StageKind::Combat), 2.0)),
            total(&compose(&c, 1, Some(StageKind::Combat), 1.0)),
        );
    }

    /// INVARIANT DE NON-BLOCAGE : une vague à 0 ennemi figerait la run pour de bon
    /// (`seen_alive` jamais posé → `clear_detection_armed` jamais armé).
    #[test]
    fn a_wave_can_never_be_empty_whatever_the_genome_says() {
        let mut c = WaveCompConfig::default();
        // Une salle de COMBAT dont tous les modificateurs tomberaient à 0.
        c.kinds.combat = Some(KindMods {
            tank: 0.0,
            runner: 0.0,
            sniper: 0.0,
            boss: 0.0,
            spawns_enemies: true,
        });
        let lines = compose(&c, 1, Some(StageKind::Combat), 1.0);
        assert!(total(&lines) >= 1, "plancher d'1 ennemi respecté");
    }

    /// Story-670 — le plancher ne s'applique PAS à une salle sans combat : sinon la
    /// porte « Repos » donne un combat, et le nom ment (`map-design-intention` §5.1).
    #[test]
    fn a_non_combat_room_spawns_nothing_and_the_floor_does_not_apply() {
        let c = WaveCompConfig::default();
        assert!(
            !room_spawns_enemies(&c, Some(StageKind::Rest)),
            "Repos est déclaré sans combat"
        );
        assert!(
            compose(&c, 1, Some(StageKind::Rest), 1.0).is_empty(),
            "aucun ennemi dans une salle de Repos, plancher compris"
        );
        // …et toutes les autres restent des salles de combat.
        for k in [
            StageKind::Combat,
            StageKind::Elite,
            StageKind::Event,
            StageKind::Shop,
            StageKind::Treasure,
        ] {
            assert!(room_spawns_enemies(&c, Some(k)), "{k:?} doit faire combattre");
            assert!(total(&compose(&c, 1, Some(k), 1.0)) >= 1, "{k:?} non vide");
        }
        // Graph absent → prudence : on ne vide jamais une salle par accident.
        assert!(room_spawns_enemies(&c, None));
    }

    #[test]
    fn the_boss_is_never_scaled_nor_modulated() {
        let c = WaveCompConfig::default();
        let bosses = |l: Vec<CompLine>| {
            l.iter()
                .find(|(a, _, _)| matches!(a, EnemyArchetype::Boss))
                .map(|(_, c, _)| *c)
                .unwrap_or(0)
        };
        assert_eq!(bosses(compose(&c, 3, None, 1.0)), 1);
        assert_eq!(
            bosses(compose(&c, 3, Some(StageKind::Elite), 2.5)),
            1,
            "un seul boss, quelle que soit la densité"
        );
    }

    #[test]
    fn an_unknown_or_absent_kind_stays_neutral() {
        let c = WaveCompConfig::default();
        assert_eq!(
            compose(&c, 1, None, 1.0),
            compose(&c, 1, Some(StageKind::Combat), 1.0),
            "graph absent → équilibre de référence, pas de dérive silencieuse"
        );
    }

    #[test]
    fn a_partial_genome_keeps_the_rust_mirror_for_the_rest() {
        let c = WaveCompConfig::parse_toml("[density]\nenabled = false\n");
        assert!(!c.density.enabled, "le champ fourni est lu");
        assert_eq!(
            total(&compose(&c, 1, Some(StageKind::Combat), 1.0)),
            8,
            "les effectifs restent ceux du miroir Rust"
        );
        assert_eq!(c.ring.jitter_m, RingCfg::default().jitter_m);
    }

    /// Le TOML livré DOIT être le miroir exact du `Default` Rust — sinon le jeu se
    /// comporte différemment selon qu'il trouve le fichier ou non (build distribué).
    ///
    /// `cargo test` tourne avec le CWD sur la crate, le jeu sur la racine du
    /// workspace : on essaie les deux, et on ÉCHOUE si aucun ne répond — un test
    /// qui se saute en silence est un capteur aveugle.
    #[test]
    fn the_real_genome_file_parses_and_keeps_the_reference_balance() {
        let content = std::fs::read_to_string(GENOME_PATH)
            .or_else(|_| std::fs::read_to_string(format!("../../{GENOME_PATH}")))
            .expect("roguelite_waves.toml introuvable depuis la crate ET depuis la racine");
        let c = WaveCompConfig::parse_toml(&content);
        assert_eq!(c, WaveCompConfig::default(), "le TOML est le miroir du Rust");
    }

    #[test]
    fn wave_two_rings_are_wider() {
        let c = WaveCompConfig::default();
        let r1 = compose(&c, 1, Some(StageKind::Combat), 1.0);
        let r2 = compose(&c, 2, Some(StageKind::Combat), 1.0);
        let radius = |l: &[CompLine], a: EnemyArchetype| {
            l.iter().find(|(x, _, _)| *x == a).map(|(_, _, r)| *r).unwrap()
        };
        assert!(
            radius(&r2, EnemyArchetype::Tank) > radius(&r1, EnemyArchetype::Tank),
            "vague 2 plus dense → anneaux élargis"
        );
    }
}
