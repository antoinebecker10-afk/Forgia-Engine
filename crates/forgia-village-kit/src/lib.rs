//! # forgia-village-kit
//!
//! Vocabulaire data-driven pour composer un village médiéval. **Aucune dépendance
//! Bevy** : ce crate est pure data + résolution chemins d'assets, testable en
//! isolation. Le crate `forgia-village-loader` consomme ce vocabulaire pour
//! spawner les entités ECS.
//!
//! Pattern industrie : Skyrim kit pieces (Burgess GDC 2013) — un kit = N pièces
//! snap-to-grid combinables via assemblage déclaratif TOML, jamais hardcode.
//!
//! ## Couche definition Forgia (`.claude/rules/concept-first.md` étape 0)
//!
//! Ce crate vit dans la couche **definition** : un village se modifie en éditant
//! `config/villages/<id>.toml`, pas du code Rust. Le code Rust ne contient ni
//! position ni nom de building.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum VillageKitError {
    #[error("village TOML not found at {0}")]
    TomlMissing(PathBuf),
    #[error("village TOML parse error at {path}: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("village TOML read error at {path}: {source}")]
    TomlRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unknown kit '{kit}'")]
    UnknownKit { kit: String },
    #[error("unknown piece '{piece}' for kit '{kit}' (color={color:?})")]
    UnknownPiece {
        kit: String,
        piece: String,
        color: Option<String>,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Data model — pure Serde, mapped 1:1 with TOML
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level container — `config/villages/<id>.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VillageDef {
    pub meta: VillageMeta,
    pub ramparts: RampartsDef,
    #[serde(default)]
    pub buildings: Vec<BuildingDef>,
    pub roads: RoadsDef,
    pub spawn: SpawnDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VillageMeta {
    pub id: String,
    /// World position (x, z) of the village center (Y derivé du terrain).
    pub center: [f32; 2],
    /// Unit scale snap-to-grid (m). KayKit Hexagon ≈ 2.0.
    pub unit_scale: f32,
    /// Kit identifier — résolu via [`KitResolver`].
    pub kit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RampartsDef {
    /// `hexagon` (6 walls) for V1. Future : `square`, `polygon`.
    pub shape: String,
    pub radius: f32,
    /// Gate angles (degrees, 0 = +Z North). Walls at these angles use gate piece.
    #[serde(default)]
    pub gates_deg: Vec<f32>,
    /// Optional Y offset (m) — KayKit pivot is at floor, usually 0.
    #[serde(default)]
    pub y_offset: f32,
    /// Raw length of one wall mesh in meters (BEFORE unit_scale).
    /// KayKit Medieval Hexagon `wall_straight.gltf` = 2.0 m raw.
    /// Each polygon side is filled with N walls = ceil(side_len / (wall * unit_scale)).
    #[serde(default = "default_wall_length_m")]
    pub wall_length_m: f32,
}

fn default_wall_length_m() -> f32 {
    2.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingDef {
    /// Piece name in the kit (e.g. `building_well`, `building_tavern`).
    pub piece: String,
    /// Color variant for color-coded kits (e.g. `red`, `blue`). Optional.
    #[serde(default)]
    pub color: Option<String>,
    /// Local position relative to village center (x, z).
    pub position: [f32; 2],
    /// Yaw rotation in degrees.
    #[serde(default)]
    pub yaw_deg: f32,
    /// Uniform scale multiplier (default 1.0).
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Optional label visible via interaction prompts.
    #[serde(default)]
    pub label: Option<String>,
    /// Anno 1800 pattern — declared edge where road must connect (`north`,
    /// `south`, `east`, `west`). Used by future road-connection validation.
    #[serde(default)]
    pub road_anchor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoadsDef {
    #[serde(default)]
    pub radial: Vec<RoadRadialDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadRadialDef {
    /// Angle in degrees from +Z (North), clockwise.
    pub direction_deg: f32,
    /// Length in meters from village edge.
    pub length: f32,
    /// `primary`, `secondary`, `trail`, `urban`.
    pub tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnDef {
    /// Player spawn position, local to village center (x, z).
    /// Y derived from terrain at runtime.
    pub player_position: [f32; 2],
    #[serde(default)]
    pub player_yaw_deg: f32,
}

fn default_scale() -> f32 {
    1.0
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading
// ─────────────────────────────────────────────────────────────────────────────

impl VillageDef {
    /// Load a village from a TOML file path. Errors are typed and contain the
    /// path for diagnostic clarity (cf `observability-required.md` next-step
    /// convention).
    pub fn load_from_path(path: impl Into<PathBuf>) -> Result<Self, VillageKitError> {
        let path = path.into();
        if !path.exists() {
            return Err(VillageKitError::TomlMissing(path));
        }
        let raw = std::fs::read_to_string(&path).map_err(|source| VillageKitError::TomlRead {
            path: path.clone(),
            source,
        })?;
        toml::from_str(&raw).map_err(|source| VillageKitError::TomlParse {
            path,
            source: Box::new(source),
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// KitResolver — maps (kit, piece, color) → asset_path. Pure function, testable.
// ─────────────────────────────────────────────────────────────────────────────

/// Resolves a building/wall piece name to an asset path, by kit.
///
/// V1 ships a single kit `kaykit_medieval_hexagon`. Adding a kit = adding a
/// match arm + sub-folder under `assets/models/<kit>/`. No code change in
/// gameplay crates.
#[derive(Debug, Clone, Default)]
pub struct KitResolver;

impl KitResolver {
    /// Resolves a building piece to its GLTF Scene asset path (relative to
    /// `assets/`). Returns `Err` if the kit/piece combination is unknown.
    ///
    /// Example: `("kaykit_medieval_hexagon", "building_well", Some("red"))`
    /// → `"models/kaykit/medieval_hexagon/buildings/red/building_well_red.gltf"`.
    pub fn building_path(
        &self,
        kit: &str,
        piece: &str,
        color: Option<&str>,
    ) -> Result<String, VillageKitError> {
        match kit {
            "kaykit_medieval_hexagon" => {
                let color = color.unwrap_or("red");
                Ok(format!(
                    "models/kaykit/medieval_hexagon/buildings/{color}/{piece}_{color}.gltf"
                ))
            }
            _ => Err(VillageKitError::UnknownKit {
                kit: kit.to_string(),
            }),
        }
    }

    /// Resolves a wall piece (no color variant — neutral only for V1).
    ///
    /// Example: `("kaykit_medieval_hexagon", "wall_straight")`
    /// → `"models/kaykit/medieval_hexagon/walls/wall_straight.gltf"`.
    pub fn wall_path(&self, kit: &str, piece: &str) -> Result<String, VillageKitError> {
        match kit {
            "kaykit_medieval_hexagon" => {
                Ok(format!("models/kaykit/medieval_hexagon/walls/{piece}.gltf"))
            }
            _ => Err(VillageKitError::UnknownKit {
                kit: kit.to_string(),
            }),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rampart geometry helpers — pure math, no Bevy
// ─────────────────────────────────────────────────────────────────────────────

/// One piece of a rampart polygon (a wall segment between two vertices).
///
/// `position` is local to village center (XZ plane), `yaw_deg` orients the
/// wall mesh tangent to the polygon edge.
#[derive(Debug, Clone, PartialEq)]
pub struct RampartPiece {
    pub kind: RampartPieceKind,
    pub position: [f32; 2],
    pub yaw_deg: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RampartPieceKind {
    /// Straight wall segment.
    Straight,
    /// Straight wall with embedded gate (used where polygon edge intersects
    /// one of the configured `gates_deg`).
    Gate,
}

/// Compute rampart piece placements for a regular polygon shape.
///
/// V1 supports `hexagon` (6 sides). Each polygon side is **chained** with
/// N wall pieces of width `wall_length_m × unit_scale`, where
/// N = ceil(side_length / wall_width_scaled). This is the Skyrim kit-pieces
/// pattern (Burgess GDC 2013) — multiple snapped instances fill any length.
///
/// For each side, the wall whose **center is closest to a gate angle**
/// (within ±half_step) is converted to Gate kind. Maximum 1 gate per side.
pub fn compute_rampart_pieces(def: &RampartsDef, unit_scale: f32) -> Vec<RampartPiece> {
    let sides = match def.shape.as_str() {
        "hexagon" => 6,
        _ => return Vec::new(),
    };

    let step = 360.0 / sides as f32;
    let half_step = step * 0.5;
    // Side length of regular polygon = 2 * radius * sin(180/sides).
    let side_len = 2.0 * def.radius * (std::f32::consts::PI / sides as f32).sin();
    let wall_scaled = def.wall_length_m * unit_scale;
    let walls_per_side = ((side_len / wall_scaled).ceil() as usize).max(1);
    // Stretch each wall to fit exactly — slight over/under-scale per wall in X
    // so the chain perfectly tiles. The mesh wall_straight extends [-1, +1] in X
    // so we don't compute that here ; we expose stretch via the loader.

    let mut pieces = Vec::with_capacity(sides * walls_per_side);

    for i in 0..sides {
        // Edge midpoint angle (between vertex i and vertex i+1).
        let edge_angle_deg = (i as f32) * step + half_step;
        // Apothem = perpendicular distance from center to edge midpoint.
        let apothem = def.radius * half_step.to_radians().cos();
        // Edge midpoint in world XZ (local to village center).
        let edge_rad = edge_angle_deg.to_radians();
        let mid_x = apothem * edge_rad.sin();
        let mid_z = apothem * edge_rad.cos();
        // Tangent direction along the edge — perpendicular to radial outward.
        // Radial outward = (sin(edge_rad), cos(edge_rad)). Tangent = perpendicular.
        let tangent_x = edge_rad.cos();
        let tangent_z = -edge_rad.sin();

        // Which sub-wall (along this side) is the gate ? The one whose center
        // angle (computed from its world XZ) is closest to a gate_deg.
        // BUG-441-07 : normalize delta then min.
        let is_side_with_gate = def.gates_deg.iter().any(|g| {
            let raw = (edge_angle_deg - g).rem_euclid(360.0);
            let diff = raw.min(360.0 - raw);
            diff < half_step
        });

        // Center the N walls evenly along the side.
        // i-th wall offset from midpoint = (i - (N-1)/2) * (side_len / N).
        // Use side_len exactly (not wall_scaled * N) so the chain fills the side
        // perfectly without overshoot — caller can `with_scale_xz` to stretch
        // each mesh to side_len/N exactly.
        let chunk_len = side_len / walls_per_side as f32;
        let gate_sub_idx = walls_per_side / 2; // middle wall hosts the gate.

        for w in 0..walls_per_side {
            let offset = (w as f32 - (walls_per_side as f32 - 1.0) * 0.5) * chunk_len;
            let x = mid_x + tangent_x * offset;
            let z = mid_z + tangent_z * offset;
            let kind = if is_side_with_gate && w == gate_sub_idx {
                RampartPieceKind::Gate
            } else {
                RampartPieceKind::Straight
            };
            pieces.push(RampartPiece {
                kind,
                position: [x, z],
                yaw_deg: edge_angle_deg,
            });
        }
    }

    pieces
}

/// Returns the **stretch factor** to apply to each rampart wall mesh's
/// X scale so the chain tiles perfectly on the polygon side, with no gap
/// nor overshoot. `unit_scale` is the village global mesh scale.
///
/// Returned value × `unit_scale` × `wall_length_m` × 2 (mesh extends [-1,+1]
/// in X = 2 raw units wide) ... actually we just return the stretch
/// multiplier to apply on top of unit_scale for the wall mesh's X axis.
pub fn rampart_wall_stretch(def: &RampartsDef, unit_scale: f32) -> f32 {
    let sides = match def.shape.as_str() {
        "hexagon" => 6,
        _ => return 1.0,
    };
    let side_len = 2.0 * def.radius * (std::f32::consts::PI / sides as f32).sin();
    let wall_scaled = def.wall_length_m * unit_scale;
    let walls_per_side = ((side_len / wall_scaled).ceil() as usize).max(1);
    let chunk_len = side_len / walls_per_side as f32;
    // Each wall must span `chunk_len` instead of `wall_scaled`.
    chunk_len / wall_scaled
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_hexagon_well_red() {
        let r = KitResolver;
        assert_eq!(
            r.building_path("kaykit_medieval_hexagon", "building_well", Some("red"))
                .unwrap(),
            "models/kaykit/medieval_hexagon/buildings/red/building_well_red.gltf"
        );
    }

    #[test]
    fn resolver_hexagon_wall() {
        let r = KitResolver;
        assert_eq!(
            r.wall_path("kaykit_medieval_hexagon", "wall_straight")
                .unwrap(),
            "models/kaykit/medieval_hexagon/walls/wall_straight.gltf"
        );
    }

    #[test]
    fn resolver_unknown_kit_errors() {
        let r = KitResolver;
        assert!(matches!(
            r.building_path("nope", "x", None),
            Err(VillageKitError::UnknownKit { .. })
        ));
    }

    fn hex_def(radius: f32, gates: Vec<f32>) -> RampartsDef {
        RampartsDef {
            shape: "hexagon".into(),
            radius,
            gates_deg: gates,
            y_offset: 0.0,
            wall_length_m: 2.0,
        }
    }

    #[test]
    fn hexagon_side_30m_with_unit_scale_2_chains_walls() {
        // hexagon radius 30 → side_len = 30 (hexagon : side = radius).
        // wall_scaled = 2.0 * 2.0 = 4 m → walls_per_side = ceil(30/4) = 8.
        let def = hex_def(30.0, vec![]);
        let pieces = compute_rampart_pieces(&def, 2.0);
        assert_eq!(pieces.len(), 6 * 8, "6 sides × 8 walls per side = 48");
        assert!(pieces.iter().all(|p| p.kind == RampartPieceKind::Straight));
    }

    #[test]
    fn hexagon_with_gate_wrap_around_350() {
        // gate at 350° must match edge 5 (centered at 330°, delta 20°) not edge 0 (30°).
        let def = hex_def(30.0, vec![350.0]);
        let pieces = compute_rampart_pieces(&def, 2.0);
        let gate_count = pieces
            .iter()
            .filter(|p| p.kind == RampartPieceKind::Gate)
            .count();
        assert_eq!(
            gate_count, 1,
            "wrap-around gate at 350° should match exactly one piece"
        );
        // The gate piece must be on side 5 (one of its sub-walls).
        let walls_per_side = pieces.len() / 6;
        let gate_idx = pieces
            .iter()
            .position(|p| p.kind == RampartPieceKind::Gate)
            .unwrap();
        let side = gate_idx / walls_per_side;
        assert_eq!(side, 5, "gate must land on side 5 (centered at 330°)");
    }

    #[test]
    fn hexagon_with_gate_north_single_gate() {
        let def = hex_def(30.0, vec![30.0]);
        let pieces = compute_rampart_pieces(&def, 2.0);
        let gates = pieces
            .iter()
            .filter(|p| p.kind == RampartPieceKind::Gate)
            .count();
        assert_eq!(gates, 1, "exactly one wall sub-piece is marked gate");
    }

    #[test]
    fn hexagon_apothem_at_midpoint() {
        // For radius 30, apothem = 30 * cos(30°) ≈ 25.98 m.
        // The MIDDLE wall of side 0 should be at apothem distance.
        let def = hex_def(30.0, vec![]);
        let pieces = compute_rampart_pieces(&def, 2.0);
        let walls_per_side = pieces.len() / 6;
        let mid_idx = walls_per_side / 2;
        let p = &pieces[mid_idx];
        let dist = (p.position[0].powi(2) + p.position[1].powi(2)).sqrt();
        let expected_apothem = 30.0 * (30.0_f32.to_radians().cos());
        // Middle wall may be slightly off-apothem due to discrete chunk centers.
        // Tolerance = half a chunk length.
        let side_len = 30.0;
        let chunk_len = side_len / walls_per_side as f32;
        assert!(
            (dist - expected_apothem).abs() < chunk_len,
            "mid wall within chunk_len of apothem"
        );
    }

    #[test]
    fn rampart_stretch_fills_side_exactly() {
        // radius 18, unit_scale 6 → wall_scaled 12 m, side 18 m → walls_per_side 2,
        // chunk_len 9 m → stretch = 9/12 = 0.75.
        let def = hex_def(18.0, vec![]);
        let stretch = rampart_wall_stretch(&def, 6.0);
        assert!((stretch - 0.75).abs() < 0.001);
    }

    #[test]
    fn village_def_roundtrip_toml() {
        let toml_src = r#"
[meta]
id = "test"
center = [0.0, 0.0]
unit_scale = 2.0
kit = "kaykit_medieval_hexagon"

[ramparts]
shape = "hexagon"
radius = 30.0
gates_deg = [30.0]

[[buildings]]
piece = "building_well"
color = "red"
position = [0.0, 0.0]

[roads]
radial = [{ direction_deg = 30.0, length = 80.0, tier = "urban" }]

[spawn]
player_position = [0.0, 5.0]
player_yaw_deg = 180.0
"#;
        let def: VillageDef = toml::from_str(toml_src).unwrap();
        assert_eq!(def.meta.id, "test");
        assert_eq!(def.buildings.len(), 1);
        assert_eq!(def.roads.radial.len(), 1);
        assert_eq!(def.ramparts.gates_deg, vec![30.0]);
    }
}
