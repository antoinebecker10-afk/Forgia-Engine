use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::chunk::TerrainConfig;

// ─────────────────────────── BiomeMode ───────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BiomeMode {
    #[default]
    Voronoi,
    Altitude,
    Single {
        biome: String,
    },
    Directional {
        center: String,
        north_west: String,
        north_east: String,
        south_east: String,
        south_west: String,
    },
    LandmarkVoronoi,
}

// ─────────────────────────── BiomeWeights ───────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BiomeWeights {
    pub plains: f32,
    pub forest: f32,
    pub desert: f32,
    pub mountain: f32,
    pub swamp: f32,
    pub tundra: f32,
    #[serde(default = "default_one")]
    pub savanna: f32,
    #[serde(default = "default_one")]
    pub jungle: f32,
    #[serde(default = "default_one")]
    pub volcanic: f32,
    #[serde(default = "default_one")]
    pub canyon: f32,
}

fn default_one() -> f32 {
    1.0
}

impl Default for BiomeWeights {
    fn default() -> Self {
        Self {
            plains: 1.0,
            forest: 1.0,
            desert: 1.0,
            mountain: 1.0,
            swamp: 1.0,
            tundra: 1.0,
            savanna: 1.0,
            jungle: 1.0,
            volcanic: 1.0,
            canyon: 1.0,
        }
    }
}

impl BiomeWeights {
    pub fn weight_for(&self, id: u8) -> f32 {
        match id {
            0 => self.plains,
            1 => self.forest,
            2 => self.desert,
            3 => self.mountain,
            4 => self.swamp,
            5 => self.tundra,
            6 => self.savanna,
            7 => self.jungle,
            8 => self.volcanic,
            9 => self.canyon,
            _ => 1.0,
        }
    }

    pub fn total(&self) -> f32 {
        self.plains
            + self.forest
            + self.desert
            + self.mountain
            + self.swamp
            + self.tundra
            + self.savanna
            + self.jungle
            + self.volcanic
            + self.canyon
    }
}

// ─────────────────────────── TerrainFeature ───────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TerrainFeature {
    MountainRange {
        center: [f32; 2],
        direction: [f32; 2],
        width: f32,
        height: f32,
    },
    Lake {
        center: [f32; 2],
        radius: f32,
        depth: f32,
    },
    River {
        start: [f32; 2],
        end: [f32; 2],
        width: f32,
        depth: f32,
    },
    Crater {
        center: [f32; 2],
        radius: f32,
        rim_height: f32,
        depth: f32,
    },
    Plateau {
        center: [f32; 2],
        radius: f32,
        height: f32,
    },
    LavaPool {
        center: [f32; 2],
        radius: f32,
    },
}

// ─────────────────────────── MapGenConfig ───────────────────────────

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct MapGenConfig {
    pub name: String,
    pub seed: u32,
    pub map_size: f32,
    pub max_height: f32,
    pub sea_level: f32,
    pub base_frequency: f32,
    pub octaves: u32,
    pub island_mode: bool,
    pub biome_mode: BiomeMode,
    pub features: Vec<TerrainFeature>,
    #[serde(default = "default_biome_cell_size")]
    pub biome_cell_size: f32,
    #[serde(default = "default_vegetation_density")]
    pub vegetation_density: f32,
    #[serde(default)]
    pub biome_weights: BiomeWeights,
    #[serde(default = "default_warp_strength")]
    pub warp_strength: f32,
    #[serde(default = "default_continental_scale")]
    pub continental_scale: f32,
    #[serde(default)]
    pub continental_strength: f32,
    #[serde(default = "default_slope_clamp_deg")]
    pub slope_clamp_deg: f32,
    #[serde(default = "default_slope_clamp_passes")]
    pub slope_clamp_passes: u32,
}

fn default_biome_cell_size() -> f32 {
    192.0
}
fn default_vegetation_density() -> f32 {
    1.0
}
fn default_warp_strength() -> f32 {
    1.0
}
fn default_continental_scale() -> f32 {
    0.0003
}
fn default_slope_clamp_deg() -> f32 {
    50.0
}
fn default_slope_clamp_passes() -> u32 {
    10
}

impl Default for MapGenConfig {
    fn default() -> Self {
        preset_forgia_showcase()
    }
}

impl MapGenConfig {
    pub fn apply_to_terrain_config(&self, tc: &mut TerrainConfig) {
        tc.seed = self.seed;
        tc.map_size = self.map_size;
        tc.max_height = self.max_height;
        tc.sea_level = self.sea_level;
        tc.streaming_radius = ((self.map_size / 192.0).ceil() as i32).clamp(4, 12);
    }
}

// ─────────────────────────── Presets ───────────────────────────

pub fn preset_demo_showcase() -> MapGenConfig {
    MapGenConfig {
        name: "Demo Showcase".into(),
        seed: 2026,
        map_size: 1024.0,
        max_height: 90.0,
        sea_level: 15.0,
        base_frequency: 0.003,
        octaves: 5,
        island_mode: true,
        biome_mode: BiomeMode::Voronoi,
        features: vec![
            TerrainFeature::MountainRange {
                center: [0.4, 0.5],
                direction: [0.8, 0.4],
                width: 0.12,
                height: 35.0,
            },
            TerrainFeature::Lake {
                center: [0.6, 0.4],
                radius: 0.07,
                depth: 20.0,
            },
        ],
        biome_cell_size: 160.0,
        vegetation_density: 1.5,
        biome_weights: BiomeWeights {
            forest: 1.5,
            mountain: 1.3,
            ..BiomeWeights::default()
        },
        warp_strength: 1.3,
        continental_scale: 0.0003,
        continental_strength: 0.0,
        slope_clamp_deg: 50.0,
        slope_clamp_passes: 8,
    }
}

pub fn preset_island() -> MapGenConfig {
    MapGenConfig {
        name: "Island".into(),
        seed: 42,
        map_size: 1024.0,
        max_height: 55.0,
        sea_level: 20.0,
        base_frequency: 0.003,
        octaves: 4,
        island_mode: true,
        biome_mode: BiomeMode::Voronoi,
        features: vec![TerrainFeature::Lake {
            center: [0.45, 0.55],
            radius: 0.05,
            depth: 12.0,
        }],
        biome_cell_size: 160.0,
        vegetation_density: 1.8,
        biome_weights: BiomeWeights {
            plains: 2.0,
            forest: 1.5,
            jungle: 3.0,
            savanna: 2.0,
            swamp: 0.5,
            mountain: 0.0,
            tundra: 0.0,
            desert: 0.0,
            volcanic: 0.0,
            canyon: 0.3,
        },
        warp_strength: 0.8,
        continental_scale: 0.0003,
        continental_strength: 0.0,
        slope_clamp_deg: 50.0,
        slope_clamp_passes: 8,
    }
}

pub fn preset_mountain_range() -> MapGenConfig {
    MapGenConfig {
        name: "Mountain Range".into(),
        seed: 77,
        map_size: 1024.0,
        max_height: 120.0,
        sea_level: 5.0,
        base_frequency: 0.004,
        octaves: 6,
        island_mode: false,
        biome_mode: BiomeMode::Altitude,
        features: vec![
            TerrainFeature::MountainRange {
                center: [0.5, 0.5],
                direction: [1.0, 0.3],
                width: 0.15,
                height: 40.0,
            },
            TerrainFeature::MountainRange {
                center: [0.3, 0.7],
                direction: [0.8, -0.6],
                width: 0.10,
                height: 25.0,
            },
        ],
        biome_cell_size: 256.0,
        vegetation_density: 0.5,
        biome_weights: BiomeWeights {
            mountain: 2.0,
            tundra: 1.5,
            ..BiomeWeights::default()
        },
        warp_strength: 1.0,
        continental_scale: 0.0003,
        continental_strength: 0.0,
        slope_clamp_deg: 50.0,
        slope_clamp_passes: 8,
    }
}

pub fn preset_desert_canyon() -> MapGenConfig {
    MapGenConfig {
        name: "Desert Canyon".into(),
        seed: 123,
        map_size: 1024.0,
        max_height: 60.0,
        sea_level: 0.0,
        base_frequency: 0.005,
        octaves: 4,
        island_mode: false,
        biome_mode: BiomeMode::Single {
            biome: "Desert".into(),
        },
        features: vec![
            TerrainFeature::River {
                start: [0.1, 0.5],
                end: [0.9, 0.45],
                width: 0.03,
                depth: 25.0,
            },
            TerrainFeature::River {
                start: [0.5, 0.1],
                end: [0.55, 0.9],
                width: 0.02,
                depth: 20.0,
            },
            TerrainFeature::Plateau {
                center: [0.7, 0.3],
                radius: 0.08,
                height: 15.0,
            },
        ],
        biome_cell_size: 192.0,
        vegetation_density: 0.3,
        biome_weights: BiomeWeights {
            desert: 3.0,
            plains: 0.0,
            forest: 0.0,
            swamp: 0.0,
            tundra: 0.0,
            ..BiomeWeights::default()
        },
        warp_strength: 1.0,
        continental_scale: 0.0003,
        continental_strength: 0.0,
        slope_clamp_deg: 50.0,
        slope_clamp_passes: 8,
    }
}

pub fn preset_forest_valley() -> MapGenConfig {
    MapGenConfig {
        name: "Forest Valley".into(),
        seed: 256,
        map_size: 1024.0,
        max_height: 38.0,
        sea_level: 8.0,
        base_frequency: 0.003,
        octaves: 3,
        island_mode: false,
        biome_mode: BiomeMode::Single {
            biome: "Forest".into(),
        },
        features: vec![
            TerrainFeature::River {
                start: [0.15, 0.5],
                end: [0.85, 0.5],
                width: 0.015,
                depth: 10.0,
            },
            TerrainFeature::Lake {
                center: [0.5, 0.5],
                radius: 0.06,
                depth: 12.0,
            },
        ],
        biome_cell_size: 128.0,
        vegetation_density: 2.0,
        biome_weights: BiomeWeights {
            forest: 3.0,
            plains: 0.5,
            ..BiomeWeights::default()
        },
        warp_strength: 0.4,
        continental_scale: 0.0003,
        continental_strength: 0.0,
        slope_clamp_deg: 50.0,
        slope_clamp_passes: 8,
    }
}

pub fn preset_volcanic() -> MapGenConfig {
    MapGenConfig {
        name: "Volcanic".into(),
        seed: 666,
        map_size: 1024.0,
        max_height: 100.0,
        sea_level: 15.0,
        base_frequency: 0.003,
        octaves: 5,
        island_mode: true,
        biome_mode: BiomeMode::Altitude,
        features: vec![
            TerrainFeature::Crater {
                center: [0.5, 0.5],
                radius: 0.08,
                rim_height: 30.0,
                depth: 40.0,
            },
            TerrainFeature::MountainRange {
                center: [0.5, 0.5],
                direction: [0.0, 1.0],
                width: 0.20,
                height: 35.0,
            },
        ],
        biome_cell_size: 192.0,
        vegetation_density: 0.2,
        biome_weights: BiomeWeights {
            mountain: 2.5,
            desert: 1.5,
            ..BiomeWeights::default()
        },
        warp_strength: 1.0,
        continental_scale: 0.0003,
        continental_strength: 0.0,
        slope_clamp_deg: 50.0,
        slope_clamp_passes: 8,
    }
}

pub fn preset_archipelago() -> MapGenConfig {
    MapGenConfig {
        name: "Archipelago".into(),
        seed: 314,
        map_size: 1024.0,
        max_height: 50.0,
        sea_level: 30.0,
        base_frequency: 0.006,
        octaves: 3,
        island_mode: true,
        biome_mode: BiomeMode::Voronoi,
        features: vec![
            TerrainFeature::Plateau {
                center: [0.3, 0.3],
                radius: 0.08,
                height: 20.0,
            },
            TerrainFeature::Plateau {
                center: [0.7, 0.4],
                radius: 0.06,
                height: 15.0,
            },
            TerrainFeature::Plateau {
                center: [0.5, 0.7],
                radius: 0.10,
                height: 25.0,
            },
            TerrainFeature::Plateau {
                center: [0.2, 0.7],
                radius: 0.05,
                height: 18.0,
            },
            TerrainFeature::Plateau {
                center: [0.8, 0.75],
                radius: 0.07,
                height: 22.0,
            },
        ],
        biome_cell_size: 140.0,
        vegetation_density: 1.5,
        biome_weights: BiomeWeights {
            plains: 2.0,
            forest: 2.0,
            jungle: 2.5,
            savanna: 1.5,
            swamp: 1.0,
            mountain: 0.3,
            tundra: 0.0,
            desert: 0.3,
            volcanic: 0.0,
            canyon: 0.5,
        },
        warp_strength: 0.8,
        continental_scale: 0.0003,
        continental_strength: 0.0,
        slope_clamp_deg: 50.0,
        slope_clamp_passes: 8,
    }
}

pub fn preset_trembling_forest() -> MapGenConfig {
    MapGenConfig {
        name: "Les arbres tremblaient".into(),
        seed: 888,
        map_size: 1024.0,
        max_height: 45.0,
        sea_level: 5.0,
        base_frequency: 0.004,
        octaves: 4,
        island_mode: false,
        biome_mode: BiomeMode::Voronoi,
        features: vec![
            TerrainFeature::River {
                start: [0.1, 0.3],
                end: [0.9, 0.7],
                width: 0.012,
                depth: 8.0,
            },
            TerrainFeature::Lake {
                center: [0.35, 0.6],
                radius: 0.04,
                depth: 10.0,
            },
        ],
        biome_cell_size: 160.0,
        vegetation_density: 3.0,
        biome_weights: BiomeWeights {
            forest: 4.0,
            swamp: 1.5,
            jungle: 0.5,
            plains: 0.3,
            ..BiomeWeights::default()
        },
        warp_strength: 2.0,
        continental_scale: 0.0003,
        continental_strength: 0.0,
        slope_clamp_deg: 50.0,
        slope_clamp_passes: 8,
    }
}

pub fn preset_forgia_showcase() -> MapGenConfig {
    MapGenConfig {
        name: "Forgia".into(),
        seed: 20_260_322,
        map_size: 4096.0,
        max_height: 180.0,
        sea_level: 18.0,
        base_frequency: 0.0014,
        octaves: 7,
        island_mode: true,
        biome_mode: BiomeMode::Directional {
            center: "Plains".into(),
            north_west: "Tundra".into(),
            north_east: "Forest".into(),
            south_east: "Volcanic".into(),
            south_west: "Jungle".into(),
        },
        features: vec![
            TerrainFeature::Plateau {
                center: [0.5, 0.5],
                radius: 0.07,
                height: 42.0,
            },
            TerrainFeature::MountainRange {
                center: [0.22, 0.22],
                direction: [0.65, 0.75],
                width: 0.13,
                height: 130.0,
            },
            TerrainFeature::MountainRange {
                center: [0.15, 0.30],
                direction: [0.95, 0.05],
                width: 0.07,
                height: 80.0,
            },
            TerrainFeature::MountainRange {
                center: [0.30, 0.15],
                direction: [0.20, 0.98],
                width: 0.06,
                height: 55.0,
            },
            TerrainFeature::Lake {
                center: [0.20, 0.18],
                radius: 0.025,
                depth: 8.0,
            },
            TerrainFeature::Plateau {
                center: [0.76, 0.24],
                radius: 0.10,
                height: 38.0,
            },
            TerrainFeature::Lake {
                center: [0.72, 0.16],
                radius: 0.05,
                depth: 6.0,
            },
            TerrainFeature::MountainRange {
                center: [0.83, 0.28],
                direction: [0.3, 0.95],
                width: 0.06,
                height: 35.0,
            },
            TerrainFeature::Lake {
                center: [0.20, 0.76],
                radius: 0.065,
                depth: 5.0,
            },
            TerrainFeature::Lake {
                center: [0.28, 0.83],
                radius: 0.038,
                depth: 4.0,
            },
            TerrainFeature::Plateau {
                center: [0.26, 0.72],
                radius: 0.04,
                height: 22.0,
            },
            TerrainFeature::MountainRange {
                center: [0.14, 0.68],
                direction: [0.10, 0.99],
                width: 0.05,
                height: 28.0,
            },
            TerrainFeature::Crater {
                center: [0.76, 0.76],
                radius: 0.085,
                rim_height: 85.0,
                depth: 70.0,
            },
            TerrainFeature::MountainRange {
                center: [0.68, 0.72],
                direction: [0.35, 0.93],
                width: 0.07,
                height: 50.0,
            },
            TerrainFeature::LavaPool {
                center: [0.84, 0.80],
                radius: 0.030,
            },
            TerrainFeature::LavaPool {
                center: [0.76, 0.87],
                radius: 0.025,
            },
            TerrainFeature::LavaPool {
                center: [0.90, 0.86],
                radius: 0.020,
            },
            TerrainFeature::LavaPool {
                center: [0.80, 0.94],
                radius: 0.016,
            },
            TerrainFeature::Crater {
                center: [0.88, 0.70],
                radius: 0.035,
                rim_height: 30.0,
                depth: 25.0,
            },
            TerrainFeature::Plateau {
                center: [0.64, 0.52],
                radius: 0.055,
                height: 30.0,
            },
            TerrainFeature::Crater {
                center: [0.72, 0.56],
                radius: 0.045,
                rim_height: 20.0,
                depth: 18.0,
            },
            TerrainFeature::Lake {
                center: [0.48, 0.42],
                radius: 0.030,
                depth: 5.0,
            },
            TerrainFeature::Lake {
                center: [0.60, 0.38],
                radius: 0.018,
                depth: 3.0,
            },
            TerrainFeature::River {
                start: [0.12, 0.28],
                end: [0.50, 0.48],
                width: 0.010,
                depth: 12.0,
            },
            TerrainFeature::River {
                start: [0.50, 0.48],
                end: [0.88, 0.72],
                width: 0.009,
                depth: 10.0,
            },
            TerrainFeature::River {
                start: [0.72, 0.18],
                end: [0.58, 0.42],
                width: 0.007,
                depth: 8.0,
            },
            TerrainFeature::River {
                start: [0.22, 0.76],
                end: [0.04, 0.62],
                width: 0.008,
                depth: 7.0,
            },
            TerrainFeature::River {
                start: [0.20, 0.20],
                end: [0.36, 0.36],
                width: 0.005,
                depth: 9.0,
            },
        ],
        biome_cell_size: 220.0,
        vegetation_density: 2.0,
        biome_weights: BiomeWeights::default(),
        warp_strength: 0.7,
        continental_scale: 0.00025,
        continental_strength: 0.20,
        slope_clamp_deg: 50.0,
        slope_clamp_passes: 10,
    }
}

pub fn all_presets() -> Vec<MapGenConfig> {
    vec![preset_forgia_showcase()]
}

// ─────────────────────────── Load / Save ───────────────────────────

fn presets_path() -> std::path::PathBuf {
    std::path::PathBuf::from("config/map_presets.json")
}

pub fn load_presets() -> Vec<MapGenConfig> {
    let path = presets_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(presets) = serde_json::from_str::<Vec<MapGenConfig>>(&data) {
            return presets;
        }
        warn!("Failed to parse map presets JSON, using defaults");
    }
    let presets = all_presets();
    save_presets(&presets);
    presets
}

pub fn save_presets(presets: &[MapGenConfig]) {
    let path = presets_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("mkdir {:?}: {}", parent, e);
        }
    }
    match serde_json::to_string_pretty(presets) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!("Failed to save map presets: {}", e);
            }
        }
        Err(e) => warn!("Failed to serialize map presets: {}", e),
    }
}

// ─────────────────────────── Regeneration Trigger ───────────────────────────

#[derive(Resource)]
pub struct MapRegenerateRequest;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::TerrainConfig;

    #[test]
    fn biome_weights_default_all_ones() {
        let w = BiomeWeights::default();
        for id in 0u8..10 {
            assert_eq!(w.weight_for(id), 1.0);
        }
    }

    #[test]
    fn biome_weights_total_is_ten_for_defaults() {
        let total = BiomeWeights::default().total();
        assert!(
            (total - 10.0).abs() < 1e-5,
            "default total should be 10.0, got {total}"
        );
    }

    #[test]
    fn biome_weights_out_of_range_id_returns_one() {
        assert_eq!(BiomeWeights::default().weight_for(255), 1.0);
    }

    #[test]
    fn all_presets_have_valid_names_and_sizes() {
        for p in all_presets() {
            assert!(!p.name.is_empty());
            assert!(p.map_size > 0.0);
            assert!(p.max_height > 0.0);
            assert!(p.sea_level >= 0.0);
            assert!(p.octaves >= 1);
        }
    }

    #[test]
    fn apply_to_terrain_config_syncs_core_fields() {
        let cfg = preset_island();
        let mut tc = TerrainConfig::default();
        cfg.apply_to_terrain_config(&mut tc);
        assert_eq!(tc.seed, cfg.seed);
        assert_eq!(tc.map_size, cfg.map_size);
        assert_eq!(tc.max_height, cfg.max_height);
        assert_eq!(tc.sea_level, cfg.sea_level);
    }

    #[test]
    fn streaming_radius_scales_with_map_size() {
        let mut tc = TerrainConfig::default();
        let small = MapGenConfig {
            map_size: 512.0,
            ..preset_forgia_showcase()
        };
        small.apply_to_terrain_config(&mut tc);
        assert_eq!(tc.streaming_radius, 4);
        let large = MapGenConfig {
            map_size: 4096.0,
            ..preset_forgia_showcase()
        };
        large.apply_to_terrain_config(&mut tc);
        assert_eq!(tc.streaming_radius, 12);
    }

    #[test]
    fn preset_forgia_showcase_is_valid() {
        let p = preset_forgia_showcase();
        assert!(!p.name.is_empty());
        assert!(p.biome_weights.total() > 0.0);
        assert!(p.vegetation_density > 0.0);
    }

    #[test]
    fn biome_mode_single_serializes_round_trip() {
        let mode = BiomeMode::Single {
            biome: "Forest".into(),
        };
        let json = serde_json::to_string(&mode).expect("serialize");
        let back: BiomeMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mode, back);
    }

    #[test]
    fn all_presets_have_valid_slope_clamp() {
        for p in all_presets() {
            assert!(
                p.slope_clamp_deg >= 30.0 && p.slope_clamp_deg <= 90.0,
                "preset '{}' slope_clamp_deg={}° hors plage",
                p.name,
                p.slope_clamp_deg
            );
            assert!(
                p.slope_clamp_passes >= 1 && p.slope_clamp_passes <= 16,
                "preset '{}' slope_clamp_passes={} hors plage",
                p.name,
                p.slope_clamp_passes
            );
        }
    }

    #[test]
    fn map_gen_config_serde_back_compat_missing_slope_fields() {
        let original = preset_forgia_showcase();
        let mut value = serde_json::to_value(&original).expect("serialize");
        value.as_object_mut().unwrap().remove("slope_clamp_deg");
        value.as_object_mut().unwrap().remove("slope_clamp_passes");
        let back: MapGenConfig = serde_json::from_value(value).expect("legacy JSON loadable");
        assert_eq!(back.slope_clamp_deg, 50.0);
        assert_eq!(back.slope_clamp_passes, 10);
    }
}
