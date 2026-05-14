//! Biome TOML specification — data-driven definition of a biome's identity,
//! terrain parameters, and enemy modifiers.
//!
//! Files live under `config/biomes/*.toml` and are loaded at startup by
//! [`BiomeRegistry`](crate::biome_registry::BiomeRegistry). Editing a TOML and
//! pressing Shift+F12 hot-reloads the registry without rebuilding.
//!
//! Certifié zone propre story-349 E2 : 8 tests couvrent les defaults de
//! `BiomeEnemyModifiers`/`BiomeGrassConfig`/`BiomeRoadConfig`, la parsing
//! minimale + riche via serde, et les loaders (file/dir missing).

use serde::{Deserialize, Serialize};
use std::path::Path;

// ─────────────────────────── BiomeStatus ───────────────────────────

/// Editorial status of a biome spec.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BiomeStatus {
    #[default]
    Draft,
    PendingReview,
    Approved,
    Rejected,
    NeedsAdjustment,
}

// ─────────────────────────── BiomePalette ───────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BiomePalette {
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub ground: String,
}

// ─────────────────────────── BiomeEnemyModifiers ───────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BiomeEnemyModifiers {
    #[serde(default = "default_one")]
    pub hp_mult: f32,
    #[serde(default = "default_one")]
    pub speed_mult: f32,
    #[serde(default = "default_one")]
    pub dmg_mult: f32,
}

impl Default for BiomeEnemyModifiers {
    fn default() -> Self {
        Self { hp_mult: 1.0, speed_mult: 1.0, dmg_mult: 1.0 }
    }
}

fn default_one() -> f32 { 1.0 }
fn default_grass_density() -> u32 { 200 }
fn default_grass_height_min() -> f32 { 0.5 }
fn default_grass_height_max() -> f32 { 1.2 }
fn default_grass_color() -> [f32; 3] { [0.25, 0.55, 0.15] }

// ─────────────────────────── BiomeGrassConfig ───────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BiomeGrassConfig {
    #[serde(default = "default_grass_density")]
    pub density: u32,
    #[serde(default = "default_grass_height_min")]
    pub height_min: f32,
    #[serde(default = "default_grass_height_max")]
    pub height_max: f32,
    #[serde(default = "default_grass_color")]
    pub color: [f32; 3],
    #[serde(default = "default_one")]
    pub wind_sway: f32,
}

impl Default for BiomeGrassConfig {
    fn default() -> Self {
        Self {
            density: 200,
            height_min: 0.5,
            height_max: 1.2,
            color: [0.25, 0.55, 0.15],
            wind_sway: 1.0,
        }
    }
}

// ─────────────────────────── BiomeRoadConfig ───────────────────────────

fn default_road_width_mult() -> f32 { 1.0 }
fn default_road_depression_mult() -> f32 { 1.0 }
fn default_road_edge_noise() -> f32 { 1.0 }
fn default_road_vegetation_encroachment() -> f32 { 0.0 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BiomeRoadConfig {
    #[serde(default = "default_road_width_mult")]
    pub width_mult: f32,
    #[serde(default = "default_road_depression_mult")]
    pub depression_mult: f32,
    #[serde(default = "default_road_edge_noise")]
    pub edge_noise: f32,
    #[serde(default = "default_road_vegetation_encroachment")]
    pub vegetation_encroachment: f32,
}

impl Default for BiomeRoadConfig {
    fn default() -> Self {
        Self {
            width_mult: 1.0,
            depression_mult: 1.0,
            edge_noise: 1.0,
            vegetation_encroachment: 0.0,
        }
    }
}

// ─────────────────────────── BiomeSpec ───────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BiomeSpec {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<[f32; 3]>,
    #[serde(default)]
    pub roughness: Option<f32>,
    #[serde(default)]
    pub display_name_fr: String,
    #[serde(default)]
    pub preview_rgb: Option<[u8; 3]>,
    #[serde(default)]
    pub enemy_modifiers: BiomeEnemyModifiers,
    #[serde(default = "default_one")]
    pub spawn_weight: f32,
    #[serde(default)]
    pub allowed_neighbors: Vec<String>,
    #[serde(default)]
    pub height_range: Option<[f32; 2]>,
    #[serde(default)]
    pub warp_strength: Option<f32>,
    #[serde(default)]
    pub redistribution: Option<String>,
    #[serde(default)]
    pub erosion_passes: Option<u32>,
    #[serde(default)]
    pub erosion_rate: Option<f32>,
    #[serde(default)]
    pub micro_roughness_amp: Option<f32>,
    #[serde(default)]
    pub cliff_color: Option<[f32; 3]>,
    #[serde(default)]
    pub snow_altitude: Option<f32>,
    #[serde(default)]
    pub beach_width: Option<f32>,
    #[serde(default)]
    pub height_mult: Option<f32>,
    #[serde(default)]
    pub lacunarity: Option<f32>,
    #[serde(default)]
    pub persistence: Option<f32>,
    #[serde(default)]
    pub slope_max: Option<f32>,
    #[serde(default)]
    pub thermal_passes: Option<u32>,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub palette: BiomePalette,
    #[serde(default)]
    pub mood: String,
    #[serde(default)]
    pub visual_references: Vec<String>,
    #[serde(default)]
    pub status: BiomeStatus,
    #[serde(default)]
    pub review_notes: Vec<String>,
    #[serde(default)]
    pub grass: BiomeGrassConfig,
    #[serde(default)]
    pub road: BiomeRoadConfig,
}

// ─────────────────────────── Loaders ───────────────────────────

pub fn load_biome_spec(path: &Path) -> Result<BiomeSpec, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    toml::from_str::<BiomeSpec>(&content)
        .map_err(|e| format!("Failed to parse {}: {e}", path.display()))
}

pub fn load_all_biome_specs(dir: &Path) -> Vec<BiomeSpec> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut specs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            match load_biome_spec(&path) {
                Ok(spec) => { specs.push(spec); }
                Err(e) => { bevy::log::warn!("Skipping biome spec: {e}"); }
            }
        }
    }
    specs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enemy_modifiers_default_is_neutral() {
        let m = BiomeEnemyModifiers::default();
        assert_eq!(m.hp_mult, 1.0);
        assert_eq!(m.speed_mult, 1.0);
        assert_eq!(m.dmg_mult, 1.0);
    }

    #[test]
    fn grass_config_default_values_are_sane() {
        let g = BiomeGrassConfig::default();
        assert!(g.density > 0);
        assert!(g.height_min > 0.0 && g.height_min < g.height_max,
                "height_min {} must be < height_max {}", g.height_min, g.height_max);
        assert!(g.color.iter().all(|c| (0.0..=1.0).contains(c)),
                "grass color {:?} must be in [0, 1]", g.color);
        assert!(g.wind_sway >= 0.0);
    }

    #[test]
    fn road_config_default_values_are_sane() {
        let r = BiomeRoadConfig::default();
        assert_eq!(r.width_mult, 1.0);
        assert_eq!(r.depression_mult, 1.0);
        assert_eq!(r.edge_noise, 1.0);
        assert_eq!(r.vegetation_encroachment, 0.0);
    }

    #[test]
    fn biome_status_default_is_draft() {
        assert_eq!(BiomeStatus::default(), BiomeStatus::Draft);
    }

    #[test]
    fn biome_spec_parses_minimal_toml() {
        let toml_str = r#"
            id = "forest"
            name = "Forest"
        "#;
        let spec: BiomeSpec = toml::from_str(toml_str).expect("minimal TOML should parse");
        assert_eq!(spec.id, "forest");
        assert_eq!(spec.name, "Forest");
        assert!(spec.color.is_none());
        assert_eq!(spec.spawn_weight, 1.0);
        assert_eq!(spec.status, BiomeStatus::Draft);
    }

    #[test]
    fn biome_spec_parses_rich_toml() {
        let toml_str = r#"
            id = "volcanic"
            name = "Volcanic"
            color = [0.3, 0.15, 0.1]
            roughness = 0.85
            spawn_weight = 0.7
            status = "Approved"
            [enemy_modifiers]
            hp_mult = 1.4
            dmg_mult = 1.5
            [grass]
            density = 50
            height_min = 0.3
            height_max = 0.6
            [road]
            width_mult = 1.1
        "#;
        let spec: BiomeSpec = toml::from_str(toml_str).expect("rich TOML should parse");
        assert_eq!(spec.color, Some([0.3, 0.15, 0.1]));
        assert_eq!(spec.roughness, Some(0.85));
        assert_eq!(spec.spawn_weight, 0.7);
        assert_eq!(spec.status, BiomeStatus::Approved);
        assert_eq!(spec.enemy_modifiers.hp_mult, 1.4);
        assert_eq!(spec.enemy_modifiers.dmg_mult, 1.5);
        assert_eq!(spec.enemy_modifiers.speed_mult, 1.0);
        assert_eq!(spec.grass.density, 50);
        assert_eq!(spec.road.width_mult, 1.1);
    }

    #[test]
    fn load_biome_spec_reports_missing_file() {
        let err = load_biome_spec(Path::new("this/does/not/exist.toml"))
            .expect_err("missing file must return Err");
        assert!(err.contains("Failed to read"), "unexpected error: {err}");
    }

    #[test]
    fn load_all_biome_specs_empty_for_missing_dir() {
        let specs = load_all_biome_specs(Path::new("this/does/not/exist"));
        assert!(specs.is_empty());
    }
}
