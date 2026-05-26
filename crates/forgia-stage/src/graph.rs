//! forgia-stage-graph — Roguelite stage graph generation.
//!
//! Story-473 V7 P0 (NEW crate). Génère la structure de run roguelite :
//! `RunGraph` avec branching choices (modèle Hadès) + déterminisme seedé
//! (modèle Slay the Spire reproductible).
//!
//! Pattern :
//! - `total_stages` (default 5) = 4 combat-likes + 1 boss
//! - À chaque transition D → D+1, `branching` (default 2) variantes sont pré-générées
//! - Player choisit une variante au runtime (consommateur `forgia-mode-roguelite`)
//!
//! Sources :
//! - Slay the Spire seed = floor RNG : <https://forgottenarbiter.github.io/Correlated-Randomness/>
//! - Hadès branching choices : Kotaku "less random than it seems"

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_run.toml";
const SENSOR_PATH: &str = "forgia_stage_graph.json";
const SENSOR_WRITE_PERIOD_SEC: f64 = 1.0;

// ─── splitmix64 RNG (inline, déterministe, pas de dep) ─────────────────────

/// SplitMix64 — algorithme standard (Vigna 2014). Stateful, déterministe par seed.
#[inline]
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Roll u64 dans [0, max). Pas de modulo bias (utilise rejection).
#[inline]
fn roll_range(state: &mut u64, max: u64) -> u64 {
    if max == 0 {
        return 0;
    }
    let threshold = u64::MAX - (u64::MAX % max);
    loop {
        let r = splitmix64(state);
        if r < threshold {
            return r % max;
        }
    }
}

// ─── StageKind ─────────────────────────────────────────────────────────────

/// Type de stage dans un run roguelite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StageKind {
    Combat,
    Elite,
    Shop,
    Event,
    Treasure,
    /// Rest stage (heal + upgrade) — pattern Slay-the-Spire campsite.
    Rest,
    Boss,
}

impl StageKind {
    /// Identifiant TOML / debug string.
    pub fn as_str(&self) -> &'static str {
        match self {
            StageKind::Combat => "combat",
            StageKind::Elite => "elite",
            StageKind::Shop => "shop",
            StageKind::Event => "event",
            StageKind::Treasure => "treasure",
            StageKind::Rest => "rest",
            StageKind::Boss => "boss",
        }
    }

    /// Restreint anti-consecutif (StS pattern : pas d'Elite/Shop/Rest 2× consécutif).
    pub fn is_restricted_consecutive(&self) -> bool {
        matches!(self, StageKind::Elite | StageKind::Shop | StageKind::Rest)
    }
}

/// Distribution officielle Slay-the-Spire (reverse-engineered via wiki + arXiv 2504.03918).
/// Total = 100. Source : <https://slaythespire.wiki.gg/wiki/Map_Generation>.
const KIND_WEIGHTS_NONBOSS: &[(StageKind, u32)] = &[
    (StageKind::Combat, 53),
    (StageKind::Event, 22), // StS "Unknown" = Forgia Event
    (StageKind::Rest, 12),
    (StageKind::Elite, 8),
    (StageKind::Shop, 5),
];

/// Tire un kind non-boss en évitant `forbidden` (anti-consecutif).
/// Conservé pour tests headless — la prod utilise `pick_nonboss_kind_with_multi`.
#[cfg(test)]
fn pick_nonboss_kind_with_constraint(state: &mut u64, forbidden: Option<StageKind>) -> StageKind {
    pick_nonboss_kind_with_multi(
        state,
        &[forbidden].into_iter().flatten().collect::<Vec<_>>(),
    )
}

/// V7 M3 step 3 fix (2026-05-20) — tire un kind non-boss en évitant TOUS les
/// kinds listés dans `forbidden_set` (anti-consecutif depth precedent + diversité
/// intra-depth pour garantir un Portal UI significatif quand branching > 1).
///
/// Fallback Combat si tous les kinds sont forbid.
fn pick_nonboss_kind_with_multi(state: &mut u64, forbidden_set: &[StageKind]) -> StageKind {
    let total: u32 = KIND_WEIGHTS_NONBOSS
        .iter()
        .filter(|(k, _)| !forbidden_set.contains(k))
        .map(|(_, w)| *w)
        .sum();
    if total == 0 {
        return StageKind::Combat;
    }
    let roll = roll_range(state, u64::from(total)) as u32;
    let mut accum = 0u32;
    for (kind, w) in KIND_WEIGHTS_NONBOSS {
        if forbidden_set.contains(kind) {
            continue;
        }
        accum += *w;
        if roll < accum {
            return *kind;
        }
    }
    StageKind::Combat
}

/// Floors fixes (StS pattern) :
/// - depth 0 = Combat (start safe)
/// - depth boss = Boss
/// - depth boss-1 = Rest (heal avant boss) si total >= 5
/// - depth total/2 = Treasure (midpoint reward) si total >= 8
pub fn forced_kind_for_depth(depth: u8, total: u8) -> Option<StageKind> {
    let boss_depth = total.saturating_sub(1);
    if depth == 0 {
        Some(StageKind::Combat)
    } else if depth == boss_depth {
        Some(StageKind::Boss)
    } else if total >= 5 && depth == boss_depth.saturating_sub(1) {
        Some(StageKind::Rest)
    } else if total >= 8 && depth == total / 2 {
        Some(StageKind::Treasure)
    } else {
        None
    }
}

// ─── StageNode + RunGraph ──────────────────────────────────────────────────

/// Nœud de stage : (kind, budget, depth, variant).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StageNode {
    pub kind: StageKind,
    pub difficulty_budget: u32,
    pub depth: u8,
    pub variant_index: u8,
}

/// Graph d'un run généré. `stages[depth][variant]` → `StageNode`.
///
/// Invariants :
/// - `stages.len() == total_stages as usize`
/// - `stages[depth=0]` = 1 variant (point de départ unique)
/// - `stages[depth=boss]` = 1 variant (boss fixe)
/// - `stages[depth=1..boss]` = `branching` variants chacun
#[derive(Debug, Clone, Resource, Serialize)]
pub struct RunGraph {
    pub seed: u64,
    pub total_stages: u8,
    pub branching: u8,
    pub stages: Vec<Vec<StageNode>>,
}

impl RunGraph {
    /// Profondeur du boss (= total_stages - 1).
    pub fn boss_depth(&self) -> u8 {
        self.total_stages.saturating_sub(1)
    }

    /// Compte les nodes par kind toutes profondeurs confondues.
    pub fn kind_distribution(&self) -> Vec<(StageKind, u32)> {
        let mut counts = [
            (StageKind::Combat, 0u32),
            (StageKind::Elite, 0),
            (StageKind::Shop, 0),
            (StageKind::Event, 0),
            (StageKind::Treasure, 0),
            (StageKind::Rest, 0),
            (StageKind::Boss, 0),
        ];
        for depth_nodes in &self.stages {
            for node in depth_nodes {
                if let Some(entry) = counts.iter_mut().find(|(k, _)| *k == node.kind) {
                    entry.1 += 1;
                }
            }
        }
        counts.to_vec()
    }
}

// ─── RunGraphConfig (parsed from genome TOML) ──────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct GeneToml {
    id: String,
    default: f32,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RunGenomeToml {
    #[serde(default, rename = "genes")]
    genes: Vec<GeneToml>,
}

/// Config Resource — parsée depuis `roguelite_run.toml`.
#[derive(Debug, Clone, Resource)]
pub struct RunGraphConfig {
    pub total_stages: u8,
    pub boss_stage_index: u8,
    pub branching: u8,
    pub director_credits_base: f32,
    pub director_credits_stage_mult: f32,
}

impl Default for RunGraphConfig {
    fn default() -> Self {
        Self {
            total_stages: 5, // 4 combat + 1 boss
            boss_stage_index: 4,
            branching: 2,
            director_credits_base: 2.0,
            director_credits_stage_mult: 1.25,
        }
    }
}

impl RunGraphConfig {
    /// Parse genome TOML — fallback Default si échec.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: RunGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut c = Self::default();
        for gene in &parsed.genes {
            match gene.id.as_str() {
                "roguelite_stage_count" => c.total_stages = gene.default.clamp(1.0, 12.0) as u8,
                "roguelite_boss_stage_index" => {
                    c.boss_stage_index = gene.default.clamp(0.0, 11.0) as u8;
                }
                "roguelite_branching_choices" => c.branching = gene.default.clamp(1.0, 4.0) as u8,
                "roguelite_director_credits_per_sec_base" => c.director_credits_base = gene.default,
                "roguelite_director_credits_stage_mult" => {
                    c.director_credits_stage_mult = gene.default;
                }
                _ => {}
            }
        }
        c
    }

    fn load_or_default() -> Self {
        let path = PathBuf::from(GENOME_PATH);
        match fs::read_to_string(&path) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }

    /// Budget Director (credits/sec) pour un stage à profondeur `depth`.
    pub fn director_budget_for_depth(&self, depth: u8) -> u32 {
        let mult = self.director_credits_stage_mult.max(1.0);
        let budget = self.director_credits_base * mult.powi(i32::from(depth));
        budget.max(0.0).round() as u32
    }
}

// ─── Generation (PURE — testable) ───────────────────────────────────────────

/// Génère un `RunGraph` complet déterministe pour un seed donné.
///
/// Structure : stages[0] = 1 start, stages[1..boss-1] = `branching` variants, stages[boss] = 1 boss.
///
/// Patterns Slay-the-Spire appliqués :
/// - Floors fixes (start=Combat, boss-1=Rest si total≥5, total/2=Treasure si total≥8, last=Boss)
/// - Anti-consecutif : variant à depth D ne réutilise pas un kind restreint (Elite/Shop/Rest)
///   présent au depth D-1.
pub fn generate_run_graph(config: &RunGraphConfig, seed: u64) -> RunGraph {
    let total = config.total_stages.max(2); // au minimum start + boss
    let boss_depth = total.saturating_sub(1);
    let branching = config.branching.max(1);

    let mut stages: Vec<Vec<StageNode>> = Vec::with_capacity(usize::from(total));

    for depth in 0..total {
        let is_start = depth == 0;
        let is_boss = depth == boss_depth;

        let variant_count = if is_start || is_boss { 1 } else { branching };

        // Anti-consecutif : si depth-1 contient un kind restreint (Elite/Shop/Rest),
        // on l'interdit pour ce depth. Approche conservative (avec branching, le joueur
        // pourrait avoir choisi n'importe quelle variant ; on interdit si AU MOINS UNE
        // variant prev était restreinte). Pas appliqué sur floors fixes.
        let forced = forced_kind_for_depth(depth, total);
        let forbidden = if forced.is_some() {
            None
        } else {
            stages.last().and_then(|prev| {
                prev.iter()
                    .map(|n| n.kind)
                    .find(|k| k.is_restricted_consecutive())
            })
        };

        let mut variants: Vec<StageNode> = Vec::with_capacity(usize::from(variant_count));
        // V7 M3 step 3 fix (2026-05-20) — diversité intra-depth pour garantir
        // un Portal UI significatif. Variant N tire un kind ≠ des variants 0..N-1.
        // Sans cette diversité, ~35% des depths avaient 2 variants identiques
        // (Combat 53% × Combat 53% = 28%, etc.) → portal jamais affiché.
        let mut intra_depth_kinds: Vec<StageKind> = Vec::with_capacity(usize::from(variant_count));
        for variant_idx in 0..variant_count {
            // Seed dérivé : (run_seed, depth, variant) → RNG indépendant par cellule.
            let mut state = seed
                ^ (u64::from(depth).wrapping_mul(0x9E3779B97F4A7C15))
                ^ (u64::from(variant_idx).wrapping_mul(0xBF58476D1CE4E5B9));

            let kind = if let Some(forced_kind) = forced {
                // Stage forcé (depth 0, midpoint Treasure, boss-1 Rest, last Boss) :
                // tous les variants partagent le même kind par design.
                forced_kind
            } else {
                // Forbid set = anti-consecutif (prev depth) + intra-depth diversity.
                let mut forbid_set: Vec<StageKind> = intra_depth_kinds.clone();
                if let Some(f) = forbidden {
                    if !forbid_set.contains(&f) {
                        forbid_set.push(f);
                    }
                }
                pick_nonboss_kind_with_multi(&mut state, &forbid_set)
            };
            intra_depth_kinds.push(kind);

            let budget = config.director_budget_for_depth(depth);

            variants.push(StageNode {
                kind,
                difficulty_budget: budget,
                depth,
                variant_index: variant_idx,
            });
        }
        stages.push(variants);
    }

    RunGraph {
        seed,
        total_stages: total,
        branching,
        stages,
    }
}

// ─── Runtime tracker (sensor source) ────────────────────────────────────────

/// Resource ECS — État runtime exposé au sensor.
#[derive(Resource, Debug, Default)]
pub struct StageGraphState {
    pub current_run_depth: u8,
    pub total_graphs_generated_session: u32,
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

/// Plugin Bevy — Add to App via `app.add_plugins(ForgiaStageGraphPlugin)`.
///
/// Charge la config depuis genome TOML + ajoute Resources + sensor writer 1Hz.
/// Le caller (forgia-mode-roguelite) doit appeler `generate_run_graph(config, seed)`
/// au début d'une run et stocker `RunGraph` dans un Resource (ou émettre un Message).
pub struct ForgiaStageGraphPlugin;

impl Plugin for ForgiaStageGraphPlugin {
    fn build(&self, app: &mut App) {
        let config = RunGraphConfig::load_or_default();
        info!(
            "[forgia-stage-graph] Loaded config: total_stages={} branching={} base_credits={:.1}",
            config.total_stages, config.branching, config.director_credits_base
        );
        app.insert_resource(config)
            .insert_resource(StageGraphState::default())
            .add_systems(Update, write_stage_graph_sensor);
    }
}

// ─── Sensor writer 1Hz ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct StageGraphSensorJson<'a> {
    id: &'a str,
    severity: &'a str,
    timestamp_secs: f64,
    total_stages: u8,
    branching: u8,
    current_run_depth: u8,
    total_graphs_generated_session: u32,
    next_step: &'a str,
}

fn write_stage_graph_sensor(
    config: Res<RunGraphConfig>,
    state: Res<StageGraphState>,
    mut last_write: Local<f64>,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    if now - *last_write < SENSOR_WRITE_PERIOD_SEC {
        return;
    }
    *last_write = now;

    let severity = severity_for_stage_graph(&config, &state);
    let next_step = next_step_for_stage_graph(&config, &state);

    let payload = StageGraphSensorJson {
        id: "stage_graph",
        severity,
        timestamp_secs: now,
        total_stages: config.total_stages,
        branching: config.branching,
        current_run_depth: state.current_run_depth,
        total_graphs_generated_session: state.total_graphs_generated_session,
        next_step,
    };

    if let Ok(json) = serde_json::to_string_pretty(&payload) {
        let _ = fs::write(SENSOR_PATH, json);
    }
}

/// Severity pure (testable).
pub fn severity_for_stage_graph(config: &RunGraphConfig, _state: &StageGraphState) -> &'static str {
    if config.total_stages < 2 {
        "warn"
    } else {
        "ok"
    }
}

/// Next-step actionnable.
pub fn next_step_for_stage_graph(config: &RunGraphConfig, state: &StageGraphState) -> &'static str {
    if config.total_stages < 2 {
        "Config invalid: total_stages < 2. Fix roguelite_run.toml gene roguelite_stage_count."
    } else if state.total_graphs_generated_session == 0 {
        "Stage graph ready. Caller (forgia-mode-roguelite) must call generate_run_graph at run start."
    } else {
        "Stage graph generation active."
    }
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_deterministic_per_seed() {
        let mut a = 12345u64;
        let mut b = 12345u64;
        for _ in 0..10 {
            assert_eq!(splitmix64(&mut a), splitmix64(&mut b));
        }
    }

    #[test]
    fn splitmix64_different_seeds_diverge() {
        let mut a = 12345u64;
        let mut b = 12346u64;
        // After 1 step, sequences should differ
        let ra = splitmix64(&mut a);
        let rb = splitmix64(&mut b);
        assert_ne!(ra, rb);
    }

    #[test]
    fn roll_range_within_bounds() {
        let mut state = 42u64;
        for _ in 0..1000 {
            let r = roll_range(&mut state, 5);
            assert!(r < 5);
        }
    }

    #[test]
    fn roll_range_zero_max_returns_zero() {
        let mut state = 42u64;
        assert_eq!(roll_range(&mut state, 0), 0);
    }

    #[test]
    fn config_default_safe() {
        let c = RunGraphConfig::default();
        assert_eq!(c.total_stages, 5);
        assert_eq!(c.branching, 2);
        assert!(c.director_credits_base > 0.0);
    }

    #[test]
    fn config_parse_toml_partial() {
        let toml_str = r#"
[[genes]]
id = "roguelite_stage_count"
default = 6.0

[[genes]]
id = "roguelite_branching_choices"
default = 3.0
"#;
        let c = RunGraphConfig::parse_toml(toml_str);
        assert_eq!(c.total_stages, 6);
        assert_eq!(c.branching, 3);
        assert!((c.director_credits_base - 2.0).abs() < 0.01); // default kept
    }

    #[test]
    fn config_parse_toml_invalid_falls_back() {
        let c = RunGraphConfig::parse_toml("garbage!!!");
        assert_eq!(c.total_stages, 5); // default
        assert_eq!(c.branching, 2);
    }

    #[test]
    fn config_clamps_stage_count() {
        let toml_str = r#"
[[genes]]
id = "roguelite_stage_count"
default = 999.0
"#;
        let c = RunGraphConfig::parse_toml(toml_str);
        assert!(c.total_stages <= 12);
    }

    #[test]
    fn director_budget_grows_with_depth() {
        let c = RunGraphConfig::default();
        let b0 = c.director_budget_for_depth(0);
        let b1 = c.director_budget_for_depth(1);
        let b4 = c.director_budget_for_depth(4);
        assert!(b1 >= b0);
        assert!(b4 > b0);
    }

    #[test]
    fn generation_deterministic_same_seed() {
        let c = RunGraphConfig::default();
        let g1 = generate_run_graph(&c, 0xDEADBEEF);
        let g2 = generate_run_graph(&c, 0xDEADBEEF);
        assert_eq!(g1.stages, g2.stages);
    }

    #[test]
    fn generation_differs_between_seeds() {
        let c = RunGraphConfig::default();
        let g1 = generate_run_graph(&c, 1);
        let g2 = generate_run_graph(&c, 2);
        // At least one stage variant differs between seeds (high probability)
        let differs = (0..g1.stages.len()).any(|d| g1.stages[d] != g2.stages[d]);
        assert!(differs, "Different seeds should produce different graphs");
    }

    #[test]
    fn generation_structure_invariants() {
        let c = RunGraphConfig::default();
        let g = generate_run_graph(&c, 0x12345);
        // total_stages variants of vec
        assert_eq!(g.stages.len(), c.total_stages as usize);
        // Start = 1 variant
        assert_eq!(g.stages[0].len(), 1);
        // Boss = 1 variant
        let boss_depth = (c.total_stages - 1) as usize;
        assert_eq!(g.stages[boss_depth].len(), 1);
        // Middle = branching variants
        for d in 1..boss_depth {
            assert_eq!(g.stages[d].len(), c.branching as usize);
        }
    }

    /// V7 M3 step 3 fix (2026-05-20) — variants à un depth branchant **non-forcé**
    /// DOIVENT avoir des kinds distincts (sinon Portal UI n'a aucun choix significatif).
    /// Depths forcés (start/boss/treasure/rest-pré-boss) sont skip — par design tous
    /// les variants y partagent le même kind forcé.
    /// Test 50 seeds différents pour couvrir l'espace probabilistique.
    #[test]
    fn generation_branching_depths_have_distinct_kinds() {
        let c = RunGraphConfig::default();
        let total = c.total_stages;
        let boss_depth = (total - 1) as usize;
        let branching_depths: Vec<u8> = (1..boss_depth as u8).collect();
        for seed in 0..50_u64 {
            let g = generate_run_graph(&c, seed.wrapping_mul(0xDEAD));
            for &d in &branching_depths {
                let variants = &g.stages[usize::from(d)];
                if variants.len() <= 1 {
                    continue;
                }
                // Skip depths forcés (tous variants partagent le forced kind).
                if forced_kind_for_depth(d, total).is_some() {
                    continue;
                }
                // Au moins 2 kinds distincts parmi les variants à ce depth.
                let kinds: std::collections::HashSet<_> = variants.iter().map(|n| n.kind).collect();
                assert!(
                    kinds.len() >= 2,
                    "seed={seed} depth={d} : {} variants tous de kind identique → portal vide",
                    variants.len()
                );
            }
        }
    }

    #[test]
    fn generation_boss_at_last_depth() {
        let c = RunGraphConfig::default();
        let g = generate_run_graph(&c, 99);
        let boss_depth = (c.total_stages - 1) as usize;
        assert_eq!(g.stages[boss_depth][0].kind, StageKind::Boss);
    }

    #[test]
    fn generation_start_is_combat() {
        let c = RunGraphConfig::default();
        let g = generate_run_graph(&c, 99);
        assert_eq!(g.stages[0][0].kind, StageKind::Combat);
    }

    #[test]
    fn generation_no_boss_before_last() {
        let c = RunGraphConfig::default();
        let g = generate_run_graph(&c, 99);
        let boss_depth = (c.total_stages - 1) as usize;
        for (d, variants) in g.stages.iter().enumerate() {
            if d == boss_depth {
                continue;
            }
            for node in variants {
                assert_ne!(node.kind, StageKind::Boss, "Boss appeared at depth {d}");
            }
        }
    }

    #[test]
    fn generation_depth_and_variant_index_correct() {
        let c = RunGraphConfig::default();
        let g = generate_run_graph(&c, 7);
        for (d, variants) in g.stages.iter().enumerate() {
            for (i, node) in variants.iter().enumerate() {
                assert_eq!(node.depth as usize, d);
                assert_eq!(node.variant_index as usize, i);
            }
        }
    }

    #[test]
    fn generation_min_2_stages_when_config_too_small() {
        let c = RunGraphConfig {
            total_stages: 1,
            ..Default::default()
        };
        let g = generate_run_graph(&c, 1);
        assert!(g.total_stages >= 2);
    }

    #[test]
    fn weighted_kind_distribution_combat_majority() {
        // StS officiel : Combat 53% — sur 200 samples, attendu ~106 ± noise.
        let mut state = 0xABCDEFu64;
        let mut combat_count = 0u32;
        for _ in 0..200 {
            if pick_nonboss_kind_with_constraint(&mut state, None) == StageKind::Combat {
                combat_count += 1;
            }
        }
        // Loose bounds (variance RNG 200 samples)
        assert!(
            combat_count > 60 && combat_count < 150,
            "Combat distribution {combat_count} out of expected range (53% target)"
        );
    }

    #[test]
    fn weights_total_100_percent() {
        let total: u32 = KIND_WEIGHTS_NONBOSS.iter().map(|(_, w)| *w).sum();
        assert_eq!(total, 100, "StS weights must sum to 100");
    }

    #[test]
    fn pick_with_constraint_excludes_forbidden() {
        let mut state = 0xCAFEu64;
        // 500 samples avec Elite forbidden → 0 Elite
        for _ in 0..500 {
            let k = pick_nonboss_kind_with_constraint(&mut state, Some(StageKind::Elite));
            assert_ne!(k, StageKind::Elite, "Forbidden kind appeared");
        }
    }

    #[test]
    fn forced_kind_start_is_combat() {
        assert_eq!(forced_kind_for_depth(0, 5), Some(StageKind::Combat));
    }

    #[test]
    fn forced_kind_last_is_boss() {
        assert_eq!(forced_kind_for_depth(4, 5), Some(StageKind::Boss));
        assert_eq!(forced_kind_for_depth(11, 12), Some(StageKind::Boss));
    }

    #[test]
    fn forced_kind_penultimate_is_rest_when_total_geq_5() {
        // total=5 → boss_depth=4 → penultimate=3
        assert_eq!(forced_kind_for_depth(3, 5), Some(StageKind::Rest));
        // total=4 → no Rest (total < 5)
        assert_ne!(forced_kind_for_depth(2, 4), Some(StageKind::Rest));
    }

    #[test]
    fn forced_kind_midpoint_treasure_when_total_geq_8() {
        // total=10 → midpoint=5 → Treasure
        assert_eq!(forced_kind_for_depth(5, 10), Some(StageKind::Treasure));
        // total=5 → no midpoint Treasure (total < 8)
        assert_ne!(forced_kind_for_depth(2, 5), Some(StageKind::Treasure));
    }

    #[test]
    fn forced_kind_returns_none_for_intermediate() {
        // total=10 : depth=1,2,3 ne sont pas forced
        assert_eq!(forced_kind_for_depth(1, 10), None);
        assert_eq!(forced_kind_for_depth(2, 10), None);
        assert_eq!(forced_kind_for_depth(3, 10), None);
    }

    #[test]
    fn is_restricted_consecutive_set() {
        assert!(StageKind::Elite.is_restricted_consecutive());
        assert!(StageKind::Shop.is_restricted_consecutive());
        assert!(StageKind::Rest.is_restricted_consecutive());
        assert!(!StageKind::Combat.is_restricted_consecutive());
        assert!(!StageKind::Event.is_restricted_consecutive());
        assert!(!StageKind::Treasure.is_restricted_consecutive());
        assert!(!StageKind::Boss.is_restricted_consecutive());
    }

    #[test]
    fn generated_has_rest_at_penultimate_when_total_5plus() {
        let c = RunGraphConfig::default(); // total=5
        let g = generate_run_graph(&c, 42);
        let penultimate = g.stages[3].iter().any(|n| n.kind == StageKind::Rest);
        assert!(penultimate, "Rest should be forced at penultimate depth");
    }

    #[test]
    fn anti_consecutive_no_elite_after_elite() {
        let c = RunGraphConfig {
            total_stages: 8,
            branching: 2,
            ..Default::default()
        };
        // Sample N seeds, verify Elite/Shop/Rest not consecutive
        for seed in 1..50 {
            let g = generate_run_graph(&c, seed);
            for depth in 1..g.stages.len() {
                let prev_has_restricted = g.stages[depth - 1]
                    .iter()
                    .any(|n| n.kind.is_restricted_consecutive());
                if !prev_has_restricted {
                    continue;
                }
                // forced kinds (Treasure at midpoint, Rest at penultimate) are allowed
                // even if prev was restricted — anti-consecutive applies only to RNG-picked.
                let forced = forced_kind_for_depth(depth as u8, g.total_stages);
                if forced.is_some() {
                    continue;
                }
                for variant in &g.stages[depth] {
                    let prev_kind = g.stages[depth - 1][0].kind;
                    if prev_kind.is_restricted_consecutive() {
                        assert_ne!(
                            variant.kind, prev_kind,
                            "Consecutive {:?} at depth {depth} seed={seed}",
                            prev_kind
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn kind_distribution_total_matches_node_count() {
        let c = RunGraphConfig::default();
        let g = generate_run_graph(&c, 0x42);
        let dist = g.kind_distribution();
        let total_from_dist: u32 = dist.iter().map(|(_, n)| *n).sum();
        let total_nodes: u32 = g.stages.iter().map(|v| v.len() as u32).sum();
        assert_eq!(total_from_dist, total_nodes);
    }

    #[test]
    fn boss_depth_matches_total_stages_minus_one() {
        let c = RunGraphConfig::default();
        let g = generate_run_graph(&c, 0);
        assert_eq!(g.boss_depth(), c.total_stages - 1);
    }

    #[test]
    fn severity_warn_when_total_stages_invalid() {
        let c = RunGraphConfig {
            total_stages: 1,
            ..Default::default()
        };
        let s = StageGraphState::default();
        assert_eq!(severity_for_stage_graph(&c, &s), "warn");
    }

    #[test]
    fn severity_ok_default() {
        let c = RunGraphConfig::default();
        let s = StageGraphState::default();
        assert_eq!(severity_for_stage_graph(&c, &s), "ok");
    }

    #[test]
    fn next_step_mentions_caller_when_no_graph_generated() {
        let c = RunGraphConfig::default();
        let s = StageGraphState::default();
        let ns = next_step_for_stage_graph(&c, &s);
        assert!(ns.contains("forgia-mode-roguelite") || ns.contains("generate"));
    }

    #[test]
    fn stage_kind_as_str_roundtrip_concept() {
        for k in [
            StageKind::Combat,
            StageKind::Elite,
            StageKind::Shop,
            StageKind::Event,
            StageKind::Treasure,
            StageKind::Boss,
        ] {
            assert!(!k.as_str().is_empty());
        }
    }
}
