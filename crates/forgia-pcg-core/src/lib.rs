//! # forgia-pcg-core
//!
//! Contrats PCG purs et headless : le plan TOML (`ContentSpec`), sa provenance
//! déterministe (`GenerationContext`) et les IR `LogicalPlan` / `SpatialPlan`.
//! Ce crate ne dépend ni de Bevy, ni de Blender, ni d'un kit concret.

use serde::{Deserialize, Serialize};
use std::fmt;

pub mod kit;
pub mod registry;
pub mod solver;
pub mod stream;
pub mod validate;

pub use kit::{KitManifest, SocketMismatch, SocketSpec};
pub use registry::{PcgRegistryLock, RegistryEntryKind};
pub use solver::{assemble_socket_chain, AttachRequest, ConstructiveError, RootPlacement};
pub use stream::{
    activate_order_sound, cell_of, compute_stream_cells, deactivate_order_sound, CellPhase,
    StreamingLayout,
};
pub use validate::{
    validate_spatial_plan, DeferredCheck, HardViolation, PlanMetrics, ValidationBudget,
    ValidationReport,
};

pub const CONTENT_SPEC_SCHEMA_V1: &str = "forgia.content-spec/v1";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PcgError {
    #[error("content-spec TOML invalid: {0}")]
    Parse(String),
    #[error("unsupported content-spec schema `{found}`; expected `{CONTENT_SPEC_SCHEMA_V1}`")]
    UnsupportedSchema { found: String },
    #[error("invalid stable id `{0}`")]
    InvalidId(String),
    #[error("generation seed must be explicit")]
    MissingSeed,
    #[error("zone `{0}` is duplicated")]
    DuplicateZone(String),
    #[error("zone `{0}` has an invalid size range")]
    InvalidZoneSize(String),
    #[error("zone `{zone}` references unknown parent zone `{parent}`")]
    UnknownParent { zone: String, parent: String },
}

/// Identifiant stable de spec, kit, zone ou passe (`namespace.name@semver`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct StableId(String);

impl StableId {
    pub fn parse(value: impl Into<String>) -> Result<Self, PcgError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 160
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-' | b'@')
            });
        if valid {
            Ok(Self(value))
        } else {
            Err(PcgError::InvalidId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Structure,
    Dungeon,
    Vehicle,
    Terrain,
    Creature,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ContentSpec {
    pub schema_version: String,
    pub id: String,
    pub kind: ContentKind,
    #[serde(default)]
    pub title: String,
    pub generation: GenerationSpec,
    #[serde(default)]
    pub intent: IntentSpec,
    #[serde(default)]
    pub bounds: Option<BoundsSpec>,
    #[serde(default)]
    pub zones: Vec<ZoneSpec>,
    /// Named per-target budgets (`[budgets.target_desktop]`). BTreeMap keeps a
    /// deterministic iteration order — never make a generation choice depend on
    /// HashMap ordering.
    #[serde(default)]
    pub budgets: std::collections::BTreeMap<String, BudgetSpec>,
    #[serde(default)]
    pub constraints: ConstraintsSpec,
    #[serde(default)]
    pub streaming: Option<StreamingSpec>,
}

/// Hard per-target budgets. Only the fields a `SpatialPlan` can actually prove
/// are enforced by [`crate::validate`]; triangle/VRAM budgets are proven offline
/// (Blender + gltf-transform), never guessed here.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BudgetSpec {
    #[serde(default)]
    pub max_stream_cells: Option<u32>,
    #[serde(default)]
    pub max_visible_meshes: Option<u32>,
    #[serde(default)]
    pub max_triangles: Option<u64>,
    #[serde(default)]
    pub max_materials: Option<u32>,
    #[serde(default)]
    pub max_texture_vram_mb: Option<u32>,
    #[serde(default)]
    pub max_collision_proxies: Option<u32>,
    #[serde(default)]
    pub max_load_p99_ms: Option<u32>,
    #[serde(default)]
    pub max_frame_p99_ms: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct ConstraintsSpec {
    #[serde(default)]
    pub hard: Vec<HardConstraint>,
}

/// One hard constraint. Kept as a permissive flat record so heterogeneous kinds
/// (`reachability`, `budget`, `clearance`, `dominance`, …) parse from one table
/// shape; [`crate::validate`] interprets the fields it understands and flags any
/// unknown kind instead of silently passing it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct HardConstraint {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub op: Option<String>,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub grant: Option<String>,
    #[serde(default)]
    pub require: Option<String>,
    #[serde(default)]
    pub radius_m: Option<f32>,
    #[serde(default)]
    pub height_m: Option<f32>,
}

/// Streaming layout for the offline→runtime frontier (cf pcg-methodology §6F).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct StreamingSpec {
    pub cell_size_m: [f32; 3],
    #[serde(default = "default_preload_neighbors")]
    pub preload_neighbors: u32,
    /// Activation order on load; the reverse must hold on unload. The runtime
    /// adapter enforces collision/navmesh **before** render.
    #[serde(default)]
    pub activate_order: Vec<String>,
    #[serde(default)]
    pub deactivate_order: Vec<String>,
}

fn default_preload_neighbors() -> u32 {
    1
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct GenerationSpec {
    pub seed: Option<u64>,
    #[serde(default = "default_solver")]
    pub solver: SolverKind,
    #[serde(default)]
    pub registry_lock: Option<String>,
}

fn default_solver() -> SolverKind {
    SolverKind::Auto
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SolverKind {
    Auto,
    Constructive,
    CspWfc,
    Search,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct IntentSpec {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub forbids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BoundsSpec {
    pub max_extent_m: [f32; 3],
    #[serde(default)]
    pub anchor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ZoneSpec {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub size_m: Option<SizeRange>,
    #[serde(default)]
    pub stream_cell: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SizeRange {
    pub min: [f32; 3],
    #[serde(default)]
    pub preferred: Option<[f32; 3]>,
}

impl ContentSpec {
    pub fn parse_toml(source: &str) -> Result<Self, PcgError> {
        let spec: Self =
            toml::from_str(source).map_err(|error| PcgError::Parse(error.to_string()))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn stable_id(&self) -> Result<StableId, PcgError> {
        StableId::parse(self.id.clone())
    }

    pub fn validate(&self) -> Result<(), PcgError> {
        if self.schema_version != CONTENT_SPEC_SCHEMA_V1 {
            return Err(PcgError::UnsupportedSchema {
                found: self.schema_version.clone(),
            });
        }
        self.stable_id()?;
        if self.generation.seed.is_none() {
            return Err(PcgError::MissingSeed);
        }
        let mut ids = self
            .zones
            .iter()
            .map(|zone| zone.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        if ids.windows(2).any(|pair| pair[0] == pair[1]) {
            let duplicate = ids
                .windows(2)
                .find_map(|pair| (pair[0] == pair[1]).then_some(pair[0]))
                .unwrap_or_default();
            return Err(PcgError::DuplicateZone(duplicate.to_string()));
        }
        for zone in &self.zones {
            StableId::parse(zone.id.clone())?;
            if let Some(parent) = &zone.parent {
                StableId::parse(parent.clone())?;
                // `ids` is sorted above; a parent must name a declared zone, else
                // the LogicalPlan would carry a silent dangling reference (gap G1).
                if ids.binary_search(&parent.as_str()).is_err() {
                    return Err(PcgError::UnknownParent {
                        zone: zone.id.clone(),
                        parent: parent.clone(),
                    });
                }
            }
            if let Some(size) = &zone.size_m {
                if size
                    .min
                    .iter()
                    .any(|value| !value.is_finite() || *value <= 0.0)
                    || size.preferred.is_some_and(|preferred| {
                        preferred
                            .iter()
                            .zip(size.min)
                            .any(|(p, min)| !p.is_finite() || *p < min)
                    })
                {
                    return Err(PcgError::InvalidZoneSize(zone.id.clone()));
                }
            }
        }
        Ok(())
    }
}

/// Provenance immuable d'une génération. Les sous-seeds dépendent uniquement
/// de cette valeur et d'un chemin stable, jamais de l'ordre d'une HashMap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GenerationContext {
    pub root_seed: u64,
    pub spec_id: StableId,
    pub generator_version: String,
    pub registry_lock: Option<String>,
}

impl GenerationContext {
    pub fn from_spec(
        spec: &ContentSpec,
        generator_version: impl Into<String>,
    ) -> Result<Self, PcgError> {
        spec.validate()?;
        Ok(Self {
            root_seed: spec.generation.seed.ok_or(PcgError::MissingSeed)?,
            spec_id: spec.stable_id()?,
            generator_version: generator_version.into(),
            registry_lock: spec.generation.registry_lock.clone(),
        })
    }

    pub fn derive_seed(&self, path: &str) -> u64 {
        stable_hash64(&[
            &self.root_seed.to_le_bytes(),
            self.spec_id.as_str().as_bytes(),
            self.generator_version.as_bytes(),
            path.as_bytes(),
        ])
    }

    pub fn rng_for(&self, path: &str) -> forgia_rng::Rng {
        forgia_rng::Rng::new(self.derive_seed(path))
    }
}

/// FNV-1a 64 stable, explicit and portable. Cryptographic hashing is not
/// required: this is a derivation namespace, not a security boundary.
fn stable_hash64(parts: &[&[u8]]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in *part {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LogicalPlan {
    pub context: GenerationContext,
    pub kind: ContentKind,
    pub zones: Vec<LogicalZone>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LogicalZone {
    pub id: StableId,
    pub kind: String,
    pub parent: Option<StableId>,
    pub capabilities: Vec<String>,
}

/// Compile une spec validée en graphe sémantique stable, sans référence à un
/// GLB. Les zones sont triées pour rendre le hash/les diagnostics reproductibles.
pub fn compile_logical_plan(
    spec: &ContentSpec,
    generator_version: impl Into<String>,
) -> Result<LogicalPlan, PcgError> {
    let context = GenerationContext::from_spec(spec, generator_version)?;
    let mut zones = spec
        .zones
        .iter()
        .map(|zone| {
            Ok(LogicalZone {
                id: StableId::parse(zone.id.clone())?,
                kind: zone.kind.clone(),
                parent: zone.parent.clone().map(StableId::parse).transpose()?,
                capabilities: sorted_unique(&zone.requires),
            })
        })
        .collect::<Result<Vec<_>, PcgError>>()?;
    zones.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    Ok(LogicalPlan {
        context,
        kind: spec.kind,
        zones,
    })
}

fn sorted_unique(values: &[String]) -> Vec<String> {
    let mut values = values.to_vec();
    values.sort();
    values.dedup();
    values
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SpatialPlan {
    pub logical: LogicalPlan,
    pub instances: Vec<KitInstance>,
    pub bindings: Vec<SocketBinding>,
    pub stream_cells: Vec<StreamCell>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KitInstance {
    pub id: StableId,
    pub kit_piece: StableId,
    pub zone: StableId,
    pub translation_m: [f32; 3],
    pub yaw_deg: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SocketBinding {
    pub from_instance: StableId,
    pub from_socket: String,
    pub to_instance: StableId,
    pub to_socket: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StreamCell {
    pub id: StableId,
    pub bounds_min_m: [f32; 3],
    pub bounds_max_m: [f32; 3],
    pub dependencies: Vec<StableId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALL_SPEC: &str = r#"
schema_version = "forgia.content-spec/v1"
id = "forgia.hall.highlands@0.1.0"
kind = "structure"
title = "Hall"
[generation]
seed = 42
solver = "constructive"
registry_lock = "pcg-registry/registry.lock.toml"
[[zones]]
id = "great_hall"
kind = "space.hall"
requires = ["nav.walkable", "social.safe", "nav.walkable"]
size_m = { min = [40.0, 12.0, 32.0], preferred = [64.0, 18.0, 48.0] }
[[zones]]
id = "west_entry"
kind = "space.entrance"
parent = "great_hall"
"#;

    #[test]
    fn parses_and_compiles_stable_logical_plan() {
        let spec = ContentSpec::parse_toml(HALL_SPEC).expect("valid content spec");
        let plan = compile_logical_plan(&spec, "pcg-core/0.1").expect("compiles");
        assert_eq!(plan.zones.len(), 2);
        assert_eq!(plan.zones[0].id.as_str(), "great_hall");
        assert_eq!(plan.zones[0].capabilities, ["nav.walkable", "social.safe"]);
        assert_eq!(
            plan.zones[1].parent.as_ref().map(StableId::as_str),
            Some("great_hall")
        );
    }

    #[test]
    fn derived_seed_is_stable_and_path_scoped() {
        let spec = ContentSpec::parse_toml(HALL_SPEC).expect("valid content spec");
        let context = GenerationContext::from_spec(&spec, "pcg-core/0.1").expect("context");
        assert_eq!(
            context.derive_seed("layout/hall.main"),
            context.derive_seed("layout/hall.main")
        );
        assert_ne!(
            context.derive_seed("layout/hall.main"),
            context.derive_seed("layout/hall.decor")
        );
        assert_eq!(
            context.rng_for("layout/hall.main").next_u64(),
            context.rng_for("layout/hall.main").next_u64()
        );
    }

    #[test]
    fn rejects_missing_seed_duplicate_zone_and_bad_range() {
        let missing_seed = HALL_SPEC.replace("seed = 42\n", "");
        assert_eq!(
            ContentSpec::parse_toml(&missing_seed),
            Err(PcgError::MissingSeed)
        );
        let duplicate = HALL_SPEC.replace("id = \"west_entry\"", "id = \"great_hall\"");
        assert_eq!(
            ContentSpec::parse_toml(&duplicate),
            Err(PcgError::DuplicateZone("great_hall".into()))
        );
        let bad_range = HALL_SPEC.replace("min = [40.0, 12.0, 32.0]", "min = [0.0, 12.0, 32.0]");
        assert_eq!(
            ContentSpec::parse_toml(&bad_range),
            Err(PcgError::InvalidZoneSize("great_hall".into()))
        );
    }
}
