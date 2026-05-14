//! Runtime biome registry — loads `config/biomes/*.toml` and provides typed
//! accessors with hardcoded fallbacks for every biome.
//!
//! Hot-reload via Shift+F12 (`biome_hotreload_input_system`).
//!
//! Certifié zone propre story-349 E2 : 7 tests couvrent la traduction
//! id→BiomeType, fallbacks pour les 10 biomes sans TOML, bandes sanes pour
//! enemy_modifiers + road_config.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::biomes::BiomeType;
use crate::biome_spec::{BiomeRoadConfig, BiomeSpec, load_all_biome_specs};

// ─────────────────────────── BiomeRegistry ───────────────────────────

#[derive(Resource, Default)]
pub struct BiomeRegistry {
    specs: HashMap<BiomeType, BiomeSpec>,
}

impl BiomeRegistry {
    pub fn load() -> Self {
        let dir = crate::config_dir::find_config_dir().join("biomes");
        let specs_vec = load_all_biome_specs(&dir);

        let mut specs = HashMap::new();
        for spec in specs_vec {
            if let Some(bt) = biome_type_from_id(&spec.id) {
                specs.insert(bt, spec);
            } else {
                warn!("BiomeRegistry: unknown biome id '{}', skipping", spec.id);
            }
        }

        info!("BiomeRegistry: loaded {} biome specs from TOML", specs.len());
        Self { specs }
    }

    pub fn get(&self, biome: BiomeType) -> Option<&BiomeSpec> {
        self.specs.get(&biome)
    }

    pub fn color(&self, biome: BiomeType) -> Color {
        if let Some(spec) = self.specs.get(&biome) {
            if let Some([r, g, b]) = spec.color {
                return Color::srgb(r, g, b);
            }
        }
        biome.color()
    }

    pub fn roughness(&self, biome: BiomeType) -> f32 {
        if let Some(spec) = self.specs.get(&biome) {
            if let Some(r) = spec.roughness {
                return r;
            }
        }
        biome.roughness()
    }

    pub fn display_name_fr(&self, biome: BiomeType) -> &str {
        if let Some(spec) = self.specs.get(&biome) {
            if !spec.display_name_fr.is_empty() {
                return &spec.display_name_fr;
            }
        }
        match biome {
            BiomeType::Plains => "Plaines",
            BiomeType::Forest => "Foret",
            BiomeType::Desert => "Desert",
            BiomeType::Mountain => "Montagne",
            BiomeType::Swamp => "Marais",
            BiomeType::Tundra => "Toundra",
            BiomeType::Savanna => "Savane",
            BiomeType::Jungle => "Jungle",
            BiomeType::Volcanic => "Volcanique",
            BiomeType::Canyon => "Canyon",
        }
    }

    pub fn preview_rgb(&self, biome: BiomeType) -> (u8, u8, u8) {
        if let Some(spec) = self.specs.get(&biome) {
            if let Some([r, g, b]) = spec.preview_rgb {
                return (r, g, b);
            }
        }
        match biome {
            BiomeType::Plains   => (115, 166, 64),
            BiomeType::Forest   => (51, 107, 38),
            BiomeType::Desert   => (199, 173, 107),
            BiomeType::Mountain => (133, 128, 122),
            BiomeType::Swamp    => (77, 97, 56),
            BiomeType::Tundra   => (209, 224, 235),
            BiomeType::Savanna  => (184, 166, 89),
            BiomeType::Jungle   => (25, 89, 20),
            BiomeType::Volcanic => (64, 46, 38),
            BiomeType::Canyon   => (173, 107, 64),
        }
    }

    pub fn enemy_modifiers(&self, biome: BiomeType) -> (f32, f32, f32) {
        if let Some(spec) = self.specs.get(&biome) {
            return (
                spec.enemy_modifiers.hp_mult,
                spec.enemy_modifiers.speed_mult,
                spec.enemy_modifiers.dmg_mult,
            );
        }
        match biome {
            BiomeType::Plains   => (1.0, 1.0, 1.0),
            BiomeType::Forest   => (1.1, 0.9, 1.1),
            BiomeType::Desert   => (0.8, 1.2, 0.9),
            BiomeType::Mountain => (1.3, 0.8, 1.2),
            BiomeType::Swamp    => (1.0, 0.7, 1.3),
            BiomeType::Tundra   => (0.9, 1.1, 1.0),
            BiomeType::Savanna  => (0.9, 1.1, 0.8),
            BiomeType::Jungle   => (1.2, 0.8, 1.2),
            BiomeType::Volcanic => (1.4, 0.9, 1.5),
            BiomeType::Canyon   => (1.1, 1.0, 1.1),
        }
    }

    pub fn spawn_weight(&self, biome: BiomeType) -> f32 {
        self.specs.get(&biome).map(|s| s.spawn_weight).unwrap_or(1.0)
    }

    pub fn height_mult_for(&self, biome: BiomeType) -> f32 {
        self.specs.get(&biome).and_then(|s| s.height_mult).unwrap_or(1.0)
    }

    pub fn lacunarity_for(&self, biome: BiomeType) -> Option<f32> {
        self.specs.get(&biome).and_then(|s| s.lacunarity)
    }

    pub fn persistence_for(&self, biome: BiomeType) -> Option<f32> {
        self.specs.get(&biome).and_then(|s| s.persistence)
    }

    pub fn slope_max_for(&self, biome: BiomeType) -> Option<f32> {
        self.specs.get(&biome).and_then(|s| s.slope_max)
    }

    pub fn thermal_passes_for(&self, biome: BiomeType) -> Option<u32> {
        self.specs.get(&biome).and_then(|s| s.thermal_passes)
    }

    pub fn road_config(&self, biome: BiomeType) -> BiomeRoadConfig {
        if let Some(spec) = self.specs.get(&biome) {
            return spec.road.clone();
        }
        match biome {
            BiomeType::Forest | BiomeType::Jungle => BiomeRoadConfig {
                width_mult: 0.8,
                depression_mult: 0.6,
                edge_noise: 1.5,
                vegetation_encroachment: 0.3,
            },
            BiomeType::Plains | BiomeType::Savanna => BiomeRoadConfig {
                width_mult: 1.2,
                depression_mult: 1.0,
                edge_noise: 0.8,
                vegetation_encroachment: 0.1,
            },
            BiomeType::Desert | BiomeType::Canyon => BiomeRoadConfig {
                width_mult: 1.1,
                depression_mult: 0.5,
                edge_noise: 0.6,
                vegetation_encroachment: 0.0,
            },
            BiomeType::Mountain | BiomeType::Tundra => BiomeRoadConfig {
                width_mult: 0.9,
                depression_mult: 1.2,
                edge_noise: 1.0,
                vegetation_encroachment: 0.0,
            },
            BiomeType::Swamp => BiomeRoadConfig {
                width_mult: 0.7,
                depression_mult: 0.4,
                edge_noise: 2.0,
                vegetation_encroachment: 0.5,
            },
            BiomeType::Volcanic => BiomeRoadConfig {
                width_mult: 1.0,
                depression_mult: 0.8,
                edge_noise: 0.7,
                vegetation_encroachment: 0.0,
            },
        }
    }
}

// ─────────────────────────── Helpers ───────────────────────────

pub fn biome_type_from_id(id: &str) -> Option<BiomeType> {
    match id.to_lowercase().as_str() {
        "plains" => Some(BiomeType::Plains),
        "forest" => Some(BiomeType::Forest),
        "desert" => Some(BiomeType::Desert),
        "mountain" => Some(BiomeType::Mountain),
        "swamp" => Some(BiomeType::Swamp),
        "tundra" => Some(BiomeType::Tundra),
        "savanna" => Some(BiomeType::Savanna),
        "jungle" => Some(BiomeType::Jungle),
        "volcanic" => Some(BiomeType::Volcanic),
        "canyon" => Some(BiomeType::Canyon),
        _ => None,
    }
}

// ─────────────────────────── Hot-Reload ───────────────────────────

#[derive(Resource)]
pub struct BiomeRegistryReloadRequest;

pub fn biome_registry_reload_system(
    mut commands: Commands,
    _request: Option<Res<BiomeRegistryReloadRequest>>,
) {
    if _request.is_none() { return; }
    commands.remove_resource::<BiomeRegistryReloadRequest>();
    commands.insert_resource(BiomeRegistry::load());
    info!("BiomeRegistry: hot-reloaded from TOML files");
}

pub fn biome_hotreload_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    if keyboard.pressed(KeyCode::ShiftLeft) && keyboard.just_pressed(KeyCode::F12) {
        commands.insert_resource(BiomeRegistryReloadRequest);
        info!("Biome hot-reload requested (Shift+F12)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_BIOMES: [BiomeType; 10] = [
        BiomeType::Plains, BiomeType::Forest, BiomeType::Desert,
        BiomeType::Mountain, BiomeType::Swamp, BiomeType::Tundra,
        BiomeType::Savanna, BiomeType::Jungle, BiomeType::Volcanic,
        BiomeType::Canyon,
    ];

    #[test]
    fn biome_type_from_id_covers_all_canonical_ids() {
        let ids = ["plains", "forest", "desert", "mountain", "swamp",
                   "tundra", "savanna", "jungle", "volcanic", "canyon"];
        for id in ids {
            assert!(biome_type_from_id(id).is_some(), "canonical id {id} must map to a BiomeType");
        }
    }

    #[test]
    fn biome_type_from_id_is_case_insensitive() {
        assert_eq!(biome_type_from_id("FOREST"), Some(BiomeType::Forest));
        assert_eq!(biome_type_from_id("ForEsT"), Some(BiomeType::Forest));
    }

    #[test]
    fn biome_type_from_id_unknown_returns_none() {
        assert_eq!(biome_type_from_id("atlantis"), None);
        assert_eq!(biome_type_from_id(""), None);
    }

    #[test]
    fn default_registry_color_falls_back_without_toml() {
        let reg = BiomeRegistry::default();
        for b in ALL_BIOMES {
            let c = reg.color(b);
            assert_eq!(c, b.color(), "fallback color differs for {b:?}");
        }
    }

    #[test]
    fn display_name_fr_non_empty_for_all_biomes() {
        let reg = BiomeRegistry::default();
        for b in ALL_BIOMES {
            let name = reg.display_name_fr(b);
            assert!(!name.is_empty(), "empty display_name_fr for {b:?}");
        }
    }

    #[test]
    fn road_config_is_sensible_for_all_biomes() {
        let reg = BiomeRegistry::default();
        for b in ALL_BIOMES {
            let rc = reg.road_config(b);
            assert!(rc.width_mult > 0.0 && rc.width_mult.is_finite(),
                    "{b:?}: width_mult {} must be > 0", rc.width_mult);
            assert!(rc.depression_mult >= 0.0 && rc.depression_mult.is_finite(),
                    "{b:?}: depression_mult {}", rc.depression_mult);
            assert!((0.0..=1.0).contains(&rc.vegetation_encroachment),
                    "{b:?}: vegetation_encroachment {} outside [0, 1]", rc.vegetation_encroachment);
        }
    }

    #[test]
    fn enemy_modifiers_within_sane_bands() {
        let reg = BiomeRegistry::default();
        for b in ALL_BIOMES {
            let (hp, speed, dmg) = reg.enemy_modifiers(b);
            assert!((0.5..=2.0).contains(&hp), "{b:?}: hp_mult {hp} outside [0.5, 2.0]");
            assert!((0.5..=2.0).contains(&speed), "{b:?}: speed_mult {speed} outside [0.5, 2.0]");
            assert!((0.5..=2.0).contains(&dmg), "{b:?}: dmg_mult {dmg} outside [0.5, 2.0]");
        }
    }

    #[test]
    fn spawn_weight_default_is_one() {
        let reg = BiomeRegistry::default();
        for b in ALL_BIOMES {
            assert_eq!(reg.spawn_weight(b), 1.0, "default spawn_weight for {b:?}");
        }
    }
}
