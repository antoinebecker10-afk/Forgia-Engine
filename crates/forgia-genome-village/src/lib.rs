//! # forgia-genome-village
//!
//! Typed [`VillageGenome`] — TOML data-driven description of a procedurally
//! generated village. **Pure data, no Bevy dep** : usable from generator,
//! validator, tests, editor.
//!
//! ## Couche `definition` (concept-first §0)
//!
//! Editing a village's character (size, density, layout, tier) happens here
//! by editing `config/genomes/villages/<id>.toml` — never by touching Rust.
//! Hot-reloadable via Bevy `file_watcher` feature (already enabled workspace).
//!
//! ## Reproducibility
//!
//! `seed: u64` is the single source of randomness. `forgia-village-generator`
//! threads it through every algorithm step (Poisson, Voronoi, road graph).
//! Same seed → bit-identical village. Setting `seed = 0` triggers a health
//! warning (suspicious uninitialized).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum VillageGenomeError {
    #[error("village genome not found at {0}")]
    Missing(PathBuf),
    #[error("village genome parse error at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("village genome read error at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Top-level VillageGenome
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level village description — `config/genomes/villages/<id>.toml`.
///
/// Every field has a serde default to keep TOMLs forward-compatible : adding
/// a field doesn't break existing genomes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VillageGenome {
    pub meta: VillageMeta,
    pub scale: ScaleDef,
    #[serde(default)]
    pub kit_mix: KitMixDef,
    #[serde(default)]
    pub ramparts: RampartGenome,
    #[serde(default)]
    pub buildings: BuildingsGenome,
    #[serde(default)]
    pub roads: RoadsGenome,
    #[serde(default)]
    pub npcs: NpcsGenome,
    #[serde(default)]
    pub spawn: SpawnDef,
}

/// Player spawn point — local XZ offset from village center.
///
/// **No hardcode** : caller (forgia-rpg) supplies village `world_center` via
/// `LoadVillageGenomeRequest` ; this offset is applied to land the player on
/// a known good spot (e.g. next to the fountain). All distances in meters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnDef {
    /// XZ offset (m) from village center where the player spawns.
    #[serde(default = "default_spawn_offset")]
    pub local_offset: [f32; 2],
    /// Initial yaw (degrees, 0 = +Z).
    #[serde(default)]
    pub yaw_deg: f32,
}

impl Default for SpawnDef {
    fn default() -> Self {
        Self {
            local_offset: default_spawn_offset(),
            yaw_deg: 0.0,
        }
    }
}

fn default_spawn_offset() -> [f32; 2] {
    // 3m diagonal SE of center — next to a typical fountain placement without
    // overlapping it. Genome can override per village.
    [3.0, 3.0]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VillageMeta {
    /// Unique identifier — also the filename stem.
    pub id: String,
    /// `hamlet` | `village` | `capital` — dispatches algorithm.
    pub layout_type: LayoutType,
    /// Seed for reproducibility. `0` triggers a runtime warning.
    pub seed: u64,
    /// `poor` | `standard` | `rich` — V1 only affects kit color choice.
    #[serde(default)]
    pub tier: EconomicTier,
    /// Biome tags this village fits naturally. Used by future world-placer.
    #[serde(default)]
    pub biome_affinity: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LayoutType {
    Hamlet,
    Village,
    /// V1 → `unimplemented!` clean error. Reserved for story-444.
    Capital,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EconomicTier {
    Poor,
    #[default]
    Standard,
    Rich,
}

// ─────────────────────────────────────────────────────────────────────────────
// Scale
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleDef {
    /// Target number of buildings. Generator may place fewer if Poisson fails
    /// to find non-overlapping spots within `bounding_radius`.
    pub target_building_count: usize,
    /// `0.0` sparse → `1.0` dense. Affects Poisson min-distance.
    pub density: f32,
    /// World-space radius (m) within which all buildings are placed.
    pub bounding_radius: f32,
    /// Uniform mesh scale applied to every building piece. KayKit Medieval
    /// Hexagon native scale = ~1m raw, so 3.5 yields ~4m buildings.
    #[serde(default = "default_unit_scale")]
    pub unit_scale: f32,
    /// Clearance (m) buildings must keep from road centerlines. Computed
    /// against road `half_width + clearance` (story-446 / Anno setback).
    /// Default 1.5m — building stays out of road plus footpath margin.
    #[serde(default = "default_clearance_m")]
    pub road_clearance_m: f32,
    /// Setback (m) used by `road_anchor` placement (CityEngine `setback` op).
    /// When a building has road_anchor flag, it's placed at `setback_m` from
    /// the nearest road perpendicular, facing the road. Default 2.5m.
    #[serde(default = "default_setback_m")]
    pub setback_m: f32,
    /// Building "footprint half-extent" used for Poisson min-separation.
    /// Default 2.5m (5m total) — empirical fit for KayKit Hexagon @ scale 3.5.
    #[serde(default = "default_footprint_m")]
    pub footprint_half_m: f32,
}

fn default_unit_scale() -> f32 {
    3.5
}
fn default_clearance_m() -> f32 {
    1.5
}
fn default_setback_m() -> f32 {
    2.5
}
fn default_footprint_m() -> f32 {
    2.5
}

// ─────────────────────────────────────────────────────────────────────────────
// Kit mix — weighted choice of `(kit_id, color)` per building
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitMixDef {
    /// Kit ID used by the resolver (e.g. `kaykit_medieval_hexagon`).
    /// V1 single kit only. V2+ : multiple kits with weights.
    #[serde(default = "default_kit")]
    pub kit: String,
    /// Color weights — generator picks a color per building via weighted_pick.
    /// Example: `{ "red" = 0.7, "blue" = 0.3 }`.
    #[serde(default)]
    pub colors: std::collections::BTreeMap<String, f32>,
}

impl Default for KitMixDef {
    fn default() -> Self {
        Self {
            kit: default_kit(),
            colors: std::collections::BTreeMap::new(),
        }
    }
}

fn default_kit() -> String {
    "kaykit_medieval_hexagon".into()
}

// ─────────────────────────────────────────────────────────────────────────────
// Ramparts
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RampartGenome {
    /// `none` | `fence_only` | `partial_wall` | `full_rampart`.
    #[serde(default = "default_rampart_style")]
    pub style: RampartStyle,
    /// Gate count (auto-placed evenly around perimeter).
    #[serde(default = "default_gate_count")]
    pub gate_count: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RampartStyle {
    #[default]
    None,
    FenceOnly,
    PartialWall,
    FullRampart,
}

impl Default for RampartGenome {
    fn default() -> Self {
        Self {
            style: RampartStyle::None,
            gate_count: 1,
        }
    }
}

fn default_rampart_style() -> RampartStyle {
    RampartStyle::None
}
fn default_gate_count() -> u32 {
    1
}

// ─────────────────────────────────────────────────────────────────────────────
// Buildings — required vs optional pool
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildingsGenome {
    /// Pieces that MUST be present (e.g. `["building_well"]` for a village).
    #[serde(default)]
    pub required: Vec<String>,
    /// Pieces eligible to fill remaining slots — weighted_pick.
    /// Map `piece_name → weight`.
    #[serde(default)]
    pub optional: std::collections::BTreeMap<String, f32>,
    /// Pieces that should be **road-anchored** (placed snapped to the nearest
    /// road, facing it). Pattern Anno 1800 edge-adjacency + CityEngine setback.
    /// Example : `["building_market", "building_tavern"]` — public services
    /// face the road for visual logic.
    #[serde(default)]
    pub road_anchored: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Roads
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadsGenome {
    /// `urban` | `primary` | `secondary` | `trail` — RoadTier for external paths.
    #[serde(default = "default_external_tier")]
    pub external_tier: String,
    /// Number of radial roads leaving the village. `[min, max]` (inclusive).
    #[serde(default = "default_radial_range")]
    pub radial_count_range: [u32; 2],
    /// External road length (m).
    #[serde(default = "default_external_length")]
    pub external_length: f32,
}

impl Default for RoadsGenome {
    fn default() -> Self {
        Self {
            external_tier: default_external_tier(),
            radial_count_range: default_radial_range(),
            external_length: default_external_length(),
        }
    }
}

fn default_external_tier() -> String {
    "urban".into()
}
fn default_radial_range() -> [u32; 2] {
    [2, 4]
}
fn default_external_length() -> f32 {
    50.0
}

// ─────────────────────────────────────────────────────────────────────────────
// NPCs — V1 only declares density. Spawner crate (story-445) consumes this.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpcsGenome {
    /// NPCs per building (0.0 = empty village, 1.0 = 1 NPC/building).
    #[serde(default)]
    pub density: f32,
    /// Allowed NPC roles. Empty = generator picks defaults.
    #[serde(default)]
    pub roles: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading
// ─────────────────────────────────────────────────────────────────────────────

impl VillageGenome {
    /// Load a village genome from a TOML file.
    pub fn load_from_path(path: impl Into<PathBuf>) -> Result<Self, VillageGenomeError> {
        let path = path.into();
        if !path.exists() {
            return Err(VillageGenomeError::Missing(path));
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|source| VillageGenomeError::Read { path: path.clone(), source })?;
        toml::from_str(&raw).map_err(|source| VillageGenomeError::Parse {
            path,
            source: Box::new(source),
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_genome_parses() {
        let toml_src = r#"
[meta]
id = "test"
layout_type = "hamlet"
seed = 42

[scale]
target_building_count = 5
density = 0.4
bounding_radius = 15.0
"#;
        let g: VillageGenome = toml::from_str(toml_src).unwrap();
        assert_eq!(g.meta.id, "test");
        assert_eq!(g.meta.layout_type, LayoutType::Hamlet);
        assert_eq!(g.meta.seed, 42);
        assert_eq!(g.meta.tier, EconomicTier::Standard);
        assert_eq!(g.scale.target_building_count, 5);
        // Default unit_scale = 3.5
        assert!((g.scale.unit_scale - 3.5).abs() < 1e-4);
        // Defaults applied to omitted sections
        assert_eq!(g.kit_mix.kit, "kaykit_medieval_hexagon");
        assert_eq!(g.ramparts.style, RampartStyle::None);
        assert_eq!(g.roads.radial_count_range, [2, 4]);
    }

    #[test]
    fn full_genome_parses() {
        let toml_src = r#"
[meta]
id = "starter_hamlet"
layout_type = "hamlet"
seed = 42
tier = "standard"
biome_affinity = ["temperate_forest", "grassland"]

[scale]
target_building_count = 6
density = 0.5
bounding_radius = 18.0
unit_scale = 3.5

[kit_mix]
kit = "kaykit_medieval_hexagon"
colors = { red = 0.7, blue = 0.3 }

[ramparts]
style = "fence_only"
gate_count = 1

[buildings]
required = ["building_well"]
optional = { building_tavern = 1.0, building_market = 1.0, building_home_A = 2.0 }

[roads]
external_tier = "urban"
radial_count_range = [2, 4]
external_length = 60.0

[npcs]
density = 0.5
roles = ["villager", "merchant"]
"#;
        let g: VillageGenome = toml::from_str(toml_src).unwrap();
        assert_eq!(g.kit_mix.colors.len(), 2);
        assert_eq!(g.buildings.required, vec!["building_well".to_string()]);
        assert_eq!(g.buildings.optional.len(), 3);
        assert_eq!(g.ramparts.style, RampartStyle::FenceOnly);
        assert_eq!(g.npcs.density, 0.5);
    }

    #[test]
    fn layout_type_serde_roundtrip() {
        for lt in &[LayoutType::Hamlet, LayoutType::Village, LayoutType::Capital] {
            let s = toml::to_string(&LayoutWrap { lt: *lt }).unwrap();
            let back: LayoutWrap = toml::from_str(&s).unwrap();
            assert_eq!(*lt, back.lt);
        }
    }

    #[derive(Serialize, Deserialize)]
    struct LayoutWrap {
        lt: LayoutType,
    }
}
