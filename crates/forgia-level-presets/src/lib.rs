//! forgia-level-presets — Module palette data layer (story-485 phase 2).
//!
//! Data-driven palette of arena "modules" (cover clusters, sniper perches,
//! melee pits, flank routes) chargée depuis `assets/genomes/level_modules.toml`.
//! Consommé par `forgia-stage-arena` qui place les modules dans l'arène hex
//! avec invariants spatiaux (sight-line, cover spacing, edge/center constraints).
//!
//! **Mode-agnostic** : aucun `run_if(in_state(GameMode::X))` au plugin niveau.
//! Caller (mode plugin) configure via `LevelModulesHandles` Resource.
//!
//! ## Architecture
//!
//! - `LevelModulesGenome` : Asset wrapper `{ modules: HashMap<String, ModuleDef> }`
//! - `ModuleDef` : un module palette (CoverCluster, SniperPerch, ...)
//! - `PropEntry` : un prop GLB pondéré + height class
//! - `ModuleKind`, `HeightClass` : enums data-typed
//! - Sensor `forgia2_level_modules.json` 1Hz : modules_count, loaded, severity, next_step
//!
//! ## Sources
//!
//! - Uncharted 4 (Naughty Dog) — cover heights standardisés 1.0/1.25/1.75 m
//! - Resistance (Insomniac) — verticality increments 3/6 m
//! - Risk of Rain 2 (Hopoo) — module palette + randomisation count per stage
//! - Forgia memory `[[reference-pattern-genome-driven-plugin-with-sensor]]`
//! - Forgia memory `[[reference-v2-genome-t-registration-pattern]]`

use bevy::prelude::*;
use forgia_anchor::AnchorKind;
use forgia_genome_core::{Genome, GenomeLoader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── AnchorKind string mapping (helper consommé par stage-arena phase 3) ────

/// Convertit un string `anchor_kind` du TOML vers `forgia_anchor::AnchorKind`.
/// Strings supportés : "cover_low", "cover_high", "sniper_perch", "melee_pit",
/// "flank_route" (story-485 variants) + 6 baselines story-483.
/// Retourne `None` si string inconnu (consumer log warn + skip).
pub fn parse_anchor_kind(s: &str) -> Option<AnchorKind> {
    match s {
        "player_spawn" => Some(AnchorKind::PlayerSpawn),
        "poi_slot" => Some(AnchorKind::PoiSlot),
        "landmark" => Some(AnchorKind::Landmark),
        "boss_pad" => Some(AnchorKind::BossPad),
        "teleporter" => Some(AnchorKind::Teleporter),
        "loot_zone" => Some(AnchorKind::LootZone),
        "cover_low" => Some(AnchorKind::CoverLow),
        "cover_high" => Some(AnchorKind::CoverHigh),
        "sniper_perch" => Some(AnchorKind::SniperPerch),
        "melee_pit" => Some(AnchorKind::MeleePit),
        "flank_route" => Some(AnchorKind::FlankRoute),
        _ => None,
    }
}

const LEVEL_MODULES_GENOME_PATH: &str = "genomes/level_modules.toml";
const SENSOR_PATH: &str = "forgia2_level_modules.json";
const SENSOR_WRITE_PERIOD_SEC: f64 = 1.0;

// ─── ModuleKind ─────────────────────────────────────────────────────────────

/// Type de module — pilote l'algo de placement côté `forgia-stage-arena`.
/// Les enum strings (TOML) sont en PascalCase pour matcher l'idiom Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum ModuleKind {
    /// Cluster de cover bas (crates, barrels). Densité élevée, spacing ≥ 3 m.
    CoverCluster,
    /// Mur / pilier cover haut, conceal full body. Densité basse.
    CoverWall,
    /// Plateforme overlook ~6 m. Typically 0-1 par stage, placé en bord.
    SniperPerch,
    /// Pit central close-combat (cramped, low cover). 0-1 par stage.
    MeleePit,
    /// Path waypoint pour AI flanking. Pas de mesh, pure marker.
    FlankRoute,
}

impl ModuleKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            ModuleKind::CoverCluster => "CoverCluster",
            ModuleKind::CoverWall => "CoverWall",
            ModuleKind::SniperPerch => "SniperPerch",
            ModuleKind::MeleePit => "MeleePit",
            ModuleKind::FlankRoute => "FlankRoute",
        }
    }

    pub const fn all() -> [ModuleKind; 5] {
        [
            ModuleKind::CoverCluster,
            ModuleKind::CoverWall,
            ModuleKind::SniperPerch,
            ModuleKind::MeleePit,
            ModuleKind::FlankRoute,
        ]
    }
}

// ─── HeightClass ────────────────────────────────────────────────────────────

/// Classe de hauteur du prop. Standards industry sourcés Uncharted 4 + Resistance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum HeightClass {
    /// 1.0–1.25 m — crouch behind, peek over (Uncharted 4 low cover).
    Low,
    /// ≥ 1.75 m — full body conceal, blocks ranged sight-line.
    High,
    /// ~6 m — perch/vantage point (Resistance verticality).
    Tall,
}

impl HeightClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            HeightClass::Low => "Low",
            HeightClass::High => "High",
            HeightClass::Tall => "Tall",
        }
    }

    /// Hauteur représentative (m) — utilisé par sight-line solver pour
    /// déterminer si un module casse une ligne de vue donnée.
    pub const fn height_m(self) -> f32 {
        match self {
            HeightClass::Low => 1.125, // mid 1.0–1.25
            HeightClass::High => 1.85, // ≥ 1.75
            HeightClass::Tall => 6.0,  // Resistance perch
        }
    }
}

// ─── PropEntry ──────────────────────────────────────────────────────────────

/// Un prop dans le palette d'un module. RNG selection pondérée par `weight`.
#[derive(Debug, Clone, Deserialize)]
pub struct PropEntry {
    /// Chemin GLB relatif à `assets/` (e.g. "models/kaykit/dungeon/crate_small.glb").
    pub prefab: String,
    /// Poids RNG. Sum des weights d'un module = base de tirage pondéré.
    pub weight: f32,
    /// Type d'anchor émis lors du spawn (mappé vers `AnchorKind` côté consumer).
    /// Strings : "cover_low", "cover_high", "sniper_perch", "melee_pit", "flank_route".
    pub anchor_kind: String,
    /// Classe de hauteur — pilote sight-line solver.
    pub height_class: HeightClass,
}

// ─── ModuleDef ──────────────────────────────────────────────────────────────

/// Définition d'un module palette, chargée depuis `level_modules.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct ModuleDef {
    /// Type de module — pilote l'algo de placement côté stage-arena.
    pub kind: ModuleKind,
    /// Palette de props (RNG weighted selection).
    pub prop_palette: Vec<PropEntry>,
    /// Densité de props par m² intra-module footprint.
    /// 0.0 = singleton (SniperPerch / MeleePit). 0.05–0.5 typique pour clusters.
    #[serde(default)]
    pub density_per_m2: f32,
    /// Espacement minimal entre props du même module (Level Design Book rule).
    /// ≥ 3.0 m pour cover bas (Uncharted 4 / "less cover is better").
    pub min_spacing_m: f32,
    /// Rayon d'exclusion footprint (m) — aucun autre module dans ce cercle.
    pub footprint_radius_m: f32,
    /// Biomes autorisés (strings matchant `forgia-terrain` BiomeKind). Vide = tous.
    #[serde(default)]
    pub allowed_biomes: Vec<String>,
    /// AnchorKind strings émis par ce module au spawn time.
    /// Used by `forgia-stage-arena` pour `AnchorStats::record(kind)`.
    pub anchor_kinds_emitted: Vec<String>,
}

// ─── ModulePaletteEntry (consumed by StageDef) ──────────────────────────────

/// Une entrée du `module_palette` d'un stage : référence un `ModuleDef` par
/// son id + le nombre d'instances à placer dans l'arène.
///
/// Story-485 §5.3 — Stage palette per-stage. Consommé par
/// `forgia-stage-arena::StageDef.module_palette: Vec<ModulePaletteEntry>`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModulePaletteEntry {
    /// Id du module dans `LevelModulesGenome.modules` (e.g. "cover_low_cluster").
    pub id: String,
    /// Nombre d'instances à placer. 0 = skip (palette annotation pour debug).
    pub count: u32,
}

// ─── LevelModulesGenome (asset wrapper) ─────────────────────────────────────

/// Genome TOML : `[modules.<id>] ModuleDef`. Hot-reloadable via Shift+F12.
#[derive(Debug, Default, Clone, Deserialize, TypePath)]
pub struct LevelModulesGenome {
    #[serde(default)]
    pub modules: HashMap<String, ModuleDef>,
}

// ─── Handles Resource ───────────────────────────────────────────────────────

/// Storage du handle genome — assure que l'asset reste loaded entre frames.
#[derive(Resource, Default)]
pub struct LevelModulesHandles {
    pub modules: Handle<Genome<LevelModulesGenome>>,
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

/// Bevy Plugin. Add via `app.add_plugins(ForgiaLevelPresetsPlugin)`.
///
/// **Mode-agnostic** — pas de gate `run_if(in_state(...))` au niveau plugin.
/// Caller (mode plugin) consomme `LevelModulesHandles` quand il en a besoin.
pub struct ForgiaLevelPresetsPlugin;

impl Plugin for ForgiaLevelPresetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Genome<LevelModulesGenome>>()
            .register_asset_loader(GenomeLoader::<LevelModulesGenome>::default())
            .init_resource::<LevelModulesHandles>()
            .add_systems(Startup, load_level_modules_genome)
            .add_systems(
                Update,
                write_level_modules_sensor.in_set(forgia_core::prelude::GameSet::Sensors),
            );
        info!(
            "[forgia-level-presets] Plugin loaded — genome path: {}",
            LEVEL_MODULES_GENOME_PATH
        );
    }
}

// ─── Startup : load genome ──────────────────────────────────────────────────

fn load_level_modules_genome(
    asset_server: Res<AssetServer>,
    mut handles: ResMut<LevelModulesHandles>,
) {
    handles.modules = asset_server.load(LEVEL_MODULES_GENOME_PATH);
    info!(
        "[forgia-level-presets] Genome handle loading: {}",
        LEVEL_MODULES_GENOME_PATH
    );
}

// ─── Sensor writer 1Hz ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct LevelModulesSensorJson<'a> {
    id: &'a str,
    severity: &'a str,
    timestamp_secs: f64,
    modules_count: usize,
    loaded: bool,
    next_step: &'a str,
}

fn write_level_modules_sensor(
    handles: Res<LevelModulesHandles>,
    genomes: Res<Assets<Genome<LevelModulesGenome>>>,
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

    let genome_opt = genomes.get(&handles.modules);
    let loaded = genome_opt.is_some();
    let modules_count = genome_opt.map(|g| g.data.modules.len()).unwrap_or(0);

    let severity = severity_for_level_modules(loaded, modules_count);
    let next_step = next_step_for_level_modules(loaded, modules_count);

    let payload = LevelModulesSensorJson {
        id: "level_modules",
        severity,
        timestamp_secs: now,
        modules_count,
        loaded,
        next_step,
    };

    if let Ok(json) = serde_json::to_string_pretty(&payload) {
        let _ = fs::write(SENSOR_PATH, json);
    }
}

// ─── Severity / Next-step (pure, testable) ──────────────────────────────────

/// Severity pure :
/// - `info` : pas encore loaded (startup transitoire)
/// - `warn` : loaded mais 0 modules
/// - `ok` : loaded avec ≥ 1 module
pub fn severity_for_level_modules(loaded: bool, modules_count: usize) -> &'static str {
    if !loaded {
        "info"
    } else if modules_count == 0 {
        "warn"
    } else {
        "ok"
    }
}

/// Next-step actionnable.
pub fn next_step_for_level_modules(loaded: bool, modules_count: usize) -> &'static str {
    if !loaded {
        "Genome not yet loaded. Awaiting asset_server (check assets/genomes/level_modules.toml exists)."
    } else if modules_count == 0 {
        "Genome loaded but 0 modules parsed. Check `assets/genomes/level_modules.toml` syntax — TOML root must have `[modules.<id>]` sections."
    } else {
        "Level modules palette loaded. Stage-arena can consume via LevelModulesHandles."
    }
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_TOML: &str = r#"
[modules.cover_low_cluster]
kind = "CoverCluster"
density_per_m2 = 0.08
min_spacing_m = 3.0
footprint_radius_m = 6.0
allowed_biomes = ["Volcanic", "Plains"]
anchor_kinds_emitted = ["cover_low"]

[[modules.cover_low_cluster.prop_palette]]
prefab = "models/kaykit/dungeon/crate_small.glb"
weight = 0.6
anchor_kind = "cover_low"
height_class = "Low"

[[modules.cover_low_cluster.prop_palette]]
prefab = "models/kaykit/dungeon/barrel.glb"
weight = 0.4
anchor_kind = "cover_low"
height_class = "Low"
"#;

    #[test]
    fn parse_level_modules_toml_minimal() {
        let genome: LevelModulesGenome = toml::from_str(MINIMAL_TOML).expect("parse genome");
        assert_eq!(genome.modules.len(), 1);
        let m = genome
            .modules
            .get("cover_low_cluster")
            .expect("module present");
        assert_eq!(m.kind, ModuleKind::CoverCluster);
        assert!((m.density_per_m2 - 0.08).abs() < 1e-6);
        assert!((m.min_spacing_m - 3.0).abs() < 1e-6);
        assert!((m.footprint_radius_m - 6.0).abs() < 1e-6);
        assert_eq!(
            m.allowed_biomes,
            vec!["Volcanic".to_string(), "Plains".to_string()]
        );
        assert_eq!(m.anchor_kinds_emitted, vec!["cover_low".to_string()]);
        assert_eq!(m.prop_palette.len(), 2);
    }

    #[test]
    fn parse_prop_palette_height_class() {
        let genome: LevelModulesGenome = toml::from_str(MINIMAL_TOML).expect("parse");
        let m = genome.modules.get("cover_low_cluster").unwrap();
        assert_eq!(m.prop_palette[0].height_class, HeightClass::Low);
        assert!((m.prop_palette[0].weight - 0.6).abs() < 1e-6);
        assert_eq!(m.prop_palette[0].anchor_kind, "cover_low");
        assert_eq!(m.prop_palette[1].height_class, HeightClass::Low);
    }

    #[test]
    fn module_kind_all_covers_5_variants() {
        assert_eq!(ModuleKind::all().len(), 5);
        // Distincts par as_str
        let mut seen = std::collections::HashSet::new();
        for k in ModuleKind::all() {
            seen.insert(k.as_str());
        }
        assert_eq!(seen.len(), 5);
    }

    #[test]
    fn module_kind_serde_roundtrip() {
        for k in ModuleKind::all() {
            let s = format!("\"{}\"", k.as_str());
            let parsed: ModuleKind =
                serde_json::from_str(&s).unwrap_or_else(|e| panic!("parse {}: {}", s, e));
            assert_eq!(parsed, k);
        }
    }

    #[test]
    fn height_class_serde_roundtrip() {
        for h in [HeightClass::Low, HeightClass::High, HeightClass::Tall] {
            let s = format!("\"{}\"", h.as_str());
            let parsed: HeightClass =
                serde_json::from_str(&s).unwrap_or_else(|e| panic!("parse {}: {}", s, e));
            assert_eq!(parsed, h);
        }
    }

    #[test]
    fn height_class_values_match_industry_sources() {
        // Uncharted 4 low cover : 1.0–1.25 m
        assert!(HeightClass::Low.height_m() >= 1.0 && HeightClass::Low.height_m() <= 1.25);
        // ≥ 1.75 m full conceal
        assert!(HeightClass::High.height_m() >= 1.75);
        // Resistance perch 6 m
        assert!((HeightClass::Tall.height_m() - 6.0).abs() < 0.5);
    }

    #[test]
    fn prop_entry_weights_sum_positive() {
        let genome: LevelModulesGenome = toml::from_str(MINIMAL_TOML).expect("parse");
        let m = genome.modules.get("cover_low_cluster").unwrap();
        let sum: f32 = m.prop_palette.iter().map(|p| p.weight).sum();
        assert!(sum > 0.0, "weight sum must be > 0 for RNG selection");
    }

    #[test]
    fn empty_genome_parses_to_zero_modules() {
        let genome: LevelModulesGenome = toml::from_str("").expect("parse empty");
        assert_eq!(genome.modules.len(), 0);
    }

    #[test]
    fn severity_not_loaded_is_info() {
        assert_eq!(severity_for_level_modules(false, 0), "info");
    }

    #[test]
    fn severity_loaded_empty_is_warn() {
        assert_eq!(severity_for_level_modules(true, 0), "warn");
    }

    #[test]
    fn severity_loaded_with_modules_is_ok() {
        assert_eq!(severity_for_level_modules(true, 4), "ok");
    }

    #[test]
    fn next_step_loaded_empty_points_to_toml() {
        let s = next_step_for_level_modules(true, 0);
        assert!(s.contains("level_modules.toml"));
    }

    #[test]
    fn next_step_not_loaded_mentions_assets() {
        let s = next_step_for_level_modules(false, 0);
        assert!(s.contains("level_modules.toml"));
    }

    #[test]
    fn parse_anchor_kind_covers_all_11_strings() {
        // Stabilité du mapping cross-crate string → AnchorKind enum
        let cases = [
            ("player_spawn", AnchorKind::PlayerSpawn),
            ("poi_slot", AnchorKind::PoiSlot),
            ("landmark", AnchorKind::Landmark),
            ("boss_pad", AnchorKind::BossPad),
            ("teleporter", AnchorKind::Teleporter),
            ("loot_zone", AnchorKind::LootZone),
            ("cover_low", AnchorKind::CoverLow),
            ("cover_high", AnchorKind::CoverHigh),
            ("sniper_perch", AnchorKind::SniperPerch),
            ("melee_pit", AnchorKind::MeleePit),
            ("flank_route", AnchorKind::FlankRoute),
        ];
        for (s, expected) in cases {
            assert_eq!(parse_anchor_kind(s), Some(expected), "string={}", s);
        }
        assert_eq!(parse_anchor_kind("unknown_xyz"), None);
    }

    #[test]
    fn parse_anchor_kind_matches_as_str_inverse() {
        // round-trip : AnchorKind::as_str() doit re-parser au même enum
        for k in AnchorKind::all() {
            assert_eq!(parse_anchor_kind(k.as_str()), Some(k));
        }
    }

    #[test]
    fn module_palette_entry_parses() {
        let s = r#"id = "cover_low_cluster"
count = 3
"#;
        let entry: ModulePaletteEntry = toml::from_str(s).expect("parse");
        assert_eq!(entry.id, "cover_low_cluster");
        assert_eq!(entry.count, 3);
    }

    #[test]
    fn module_def_singleton_pattern_density_zero() {
        // SniperPerch / MeleePit utilisent density=0 (singleton)
        let toml_singleton = r#"
[modules.sniper_perch]
kind = "SniperPerch"
density_per_m2 = 0.0
min_spacing_m = 0.0
footprint_radius_m = 4.0
allowed_biomes = ["Volcanic"]
anchor_kinds_emitted = ["sniper_perch"]

[[modules.sniper_perch.prop_palette]]
prefab = "models/kaykit/dungeon/tower_section.glb"
weight = 1.0
anchor_kind = "sniper_perch"
height_class = "Tall"
"#;
        let genome: LevelModulesGenome = toml::from_str(toml_singleton).expect("parse");
        let m = genome.modules.get("sniper_perch").unwrap();
        assert_eq!(m.kind, ModuleKind::SniperPerch);
        assert_eq!(m.density_per_m2, 0.0);
        assert_eq!(m.prop_palette[0].height_class, HeightClass::Tall);
    }
}
