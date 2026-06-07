//! Recipe layer — a settlement described as **data** (story-578 P2).
//!
//! A [`HamletRecipe`] is the genome of a small settlement: grid size, spacing, which module
//! roles to use, density, seed. It is loaded from TOML and **hot-reloadable** — edit the file,
//! press F7 (or Shift+F12) and the hamlet regenerates. This is the "recipe → world" brick:
//! no hardcoded layout, variety comes from the data.
//!
//! P2 keeps the layout deliberately simple (a jittered grid). Roads / parcels / grammar land
//! in P3-P5.

use crate::registry::AssetRole;
use serde::Deserialize;

/// Data-driven description of a small grid hamlet.
///
/// `#[serde(default)]` makes every field optional in the TOML — a partial or missing file
/// still yields a sane hamlet via [`Default`].
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct HamletRecipe {
    /// Deterministic seed (same seed → same hamlet).
    pub seed: u64,
    /// Grid columns / rows.
    pub grid_cols: u32,
    pub grid_rows: u32,
    /// Distance between cell centers (meters).
    pub cell_size: f32,
    /// Max random per-module offset within its cell (meters).
    pub jitter: f32,
    /// Uniform module scale.
    pub scale: f32,
    /// Probability an interior cell receives a building.
    pub fill_chance: f32,
    /// Random 90° yaw per module.
    pub yaw_random: bool,
    /// Roles drawn for the body of the hamlet (buildings / props).
    pub building_roles: Vec<AssetRole>,
    /// Place a module from `border_role` on the perimeter (a fence/wall ring).
    pub border: bool,
    /// Role used for the perimeter ring.
    pub border_role: AssetRole,
}

impl Default for HamletRecipe {
    fn default() -> Self {
        Self {
            seed: 1337,
            grid_cols: 6,
            grid_rows: 5,
            cell_size: 13.0,
            jitter: 2.5,
            scale: 0.6,
            fill_chance: 0.65,
            yaw_random: true,
            building_roles: vec![AssetRole::Prop, AssetRole::Pillar],
            border: true,
            border_role: AssetRole::Wall,
        }
    }
}

/// Load a recipe from a TOML file. Falls back to [`HamletRecipe::default`] on any read/parse
/// error (never blocks generation — the demo always produces something).
pub fn load_recipe(path: &str) -> HamletRecipe {
    match std::fs::read_to_string(path) {
        Ok(s) => match toml::from_str::<HamletRecipe>(&s) {
            Ok(r) => r,
            Err(e) => {
                bevy::log::error!("[worldgen] parse {path}: {e} → default recipe");
                HamletRecipe::default()
            }
        },
        Err(e) => {
            bevy::log::warn!("[worldgen] read {path}: {e} → default recipe");
            HamletRecipe::default()
        }
    }
}

/// Building grammar: how to assemble a parcel into a stacked building (story-578 P5).
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct BuildingTemplate {
    /// Role for the ground module.
    pub base_role: AssetRole,
    /// Role for each stacked floor.
    pub body_role: AssetRole,
    /// Role for the roof / cap.
    pub cap_role: AssetRole,
    /// Min / max number of body floors (varies per building, seeded).
    pub min_floors: u32,
    pub max_floors: u32,
}

impl Default for BuildingTemplate {
    fn default() -> Self {
        Self {
            base_role: AssetRole::Platform,
            body_role: AssetRole::Pillar,
            cap_role: AssetRole::Prop,
            min_floors: 1,
            max_floors: 3,
        }
    }
}

/// A hand-crafted landmark injected into the procedural city (story-578 P5). The fill reserves
/// the parcels around it.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Landmark {
    /// Explicit module id (registry / glTF mesh name).
    pub module_id: String,
    /// Position in layout space (meters, relative to the city center).
    pub x: f32,
    pub z: f32,
    /// Uniform scale (defaults to 1.0 when 0).
    pub scale: f32,
}

/// Data-driven description of a city layout (roads + parcels + buildings + landmarks). P3 + P5.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct CityRecipe {
    /// Deterministic seed (selects which building lands on each parcel).
    pub seed: u64,
    /// City side length (meters); the layout is centered on the placement origin.
    pub size: f32,
    /// Road grid orientation (degrees).
    pub base_angle_deg: f32,
    /// 0 = pure grid roads; >0 = ring roads near the center.
    pub radial_strength: f32,
    /// Radial influence falloff (meters).
    pub radial_radius: f32,
    /// Major road spacing = block size (meters).
    pub road_spacing: f32,
    /// Streamline trace step (meters).
    pub road_step: f32,
    /// Minimum separation between roads of the same family (meters).
    pub road_sep: f32,
    /// Target lot area when subdividing blocks (m²).
    pub parcel_target_area: f32,
    /// Road margin inset around each block (meters).
    pub parcel_inset: f32,
    /// Scale of the buildings placed on parcels.
    pub building_scale: f32,
    /// Grammar used to assemble each parcel into a stacked building (P5).
    pub building: BuildingTemplate,
    /// Hand-crafted landmarks injected into the city (P5).
    pub landmarks: Vec<Landmark>,
    /// Parcels whose center is within this distance of a landmark are reserved (left empty).
    pub landmark_reserve_radius: f32,
}

impl Default for CityRecipe {
    fn default() -> Self {
        Self {
            seed: 7,
            size: 120.0,
            base_angle_deg: 0.0,
            radial_strength: 0.0,
            radial_radius: 50.0,
            road_spacing: 30.0,
            road_step: 4.0,
            road_sep: 6.0,
            parcel_target_area: 180.0,
            parcel_inset: 3.0,
            building_scale: 0.5,
            building: BuildingTemplate::default(),
            landmarks: Vec::new(),
            landmark_reserve_radius: 14.0,
        }
    }
}

/// Load a city recipe from TOML. Falls back to [`CityRecipe::default`] on any error.
pub fn load_city_recipe(path: &str) -> CityRecipe {
    match std::fs::read_to_string(path) {
        Ok(s) => match toml::from_str::<CityRecipe>(&s) {
            Ok(r) => r,
            Err(e) => {
                bevy::log::error!("[worldgen] parse {path}: {e} → default city");
                CityRecipe::default()
            }
        },
        Err(e) => {
            bevy::log::warn!("[worldgen] read {path}: {e} → default city");
            CityRecipe::default()
        }
    }
}

/// Data-driven streaming parameters (story-578 P4): an endless city streamed around the player.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct StreamRecipe {
    /// World seed — same seed → same city, always.
    pub world_seed: u64,
    /// Chunk side length (meters).
    pub chunk_size: f32,
    /// How many chunks around the player are kept loaded (Chebyshev radius).
    pub view_radius: u32,
    /// Buildings generated per chunk.
    pub buildings_per_chunk: u32,
    /// Role drawn for buildings.
    pub building_role: AssetRole,
    /// Uniform building scale.
    pub scale: f32,
}

impl Default for StreamRecipe {
    fn default() -> Self {
        Self {
            world_seed: 2024,
            chunk_size: 40.0,
            view_radius: 2,
            buildings_per_chunk: 5,
            building_role: AssetRole::Prop,
            scale: 0.5,
        }
    }
}

/// Load a streaming recipe from TOML. Falls back to [`StreamRecipe::default`] on any error.
pub fn load_stream_recipe(path: &str) -> StreamRecipe {
    match std::fs::read_to_string(path) {
        Ok(s) => match toml::from_str::<StreamRecipe>(&s) {
            Ok(r) => r,
            Err(e) => {
                bevy::log::error!("[worldgen] parse {path}: {e} → default stream");
                StreamRecipe::default()
            }
        },
        Err(e) => {
            bevy::log::warn!("[worldgen] read {path}: {e} → default stream");
            StreamRecipe::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_recipe_is_sane() {
        let r = HamletRecipe::default();
        assert!(r.grid_cols >= 1 && r.grid_rows >= 1);
        assert!(r.cell_size > 0.0 && r.scale > 0.0);
        assert!(!r.building_roles.is_empty());
        assert!((0.0..=1.0).contains(&r.fill_chance));
    }

    #[test]
    fn shipped_hamlet_toml_parses() {
        let s = include_str!("../../../assets/genomes/worldgen/hamlet.toml");
        let r: HamletRecipe = toml::from_str(s).expect("hamlet.toml must parse");
        assert!(!r.building_roles.is_empty());
        assert!(r.grid_cols >= 1 && r.grid_rows >= 1);
    }

    #[test]
    fn default_city_is_sane() {
        let c = CityRecipe::default();
        assert!(c.size > 0.0 && c.road_spacing > 0.0 && c.parcel_target_area > 0.0);
        assert!(c.building_scale > 0.0);
    }

    #[test]
    fn shipped_city_toml_parses() {
        let s = include_str!("../../../assets/genomes/worldgen/city.toml");
        let c: CityRecipe = toml::from_str(s).expect("city.toml must parse");
        assert!(c.size > 0.0 && c.road_spacing > 0.0);
        // P5: the building grammar + the forge landmark must load (catches a [[landmark]] vs
        // [[landmarks]] field-name mismatch).
        assert!(c.building.max_floors >= c.building.min_floors);
        assert_eq!(c.landmarks.len(), 1, "the forge landmark must load");
        assert_eq!(c.landmarks[0].module_id, "Cube.001");
    }
}
