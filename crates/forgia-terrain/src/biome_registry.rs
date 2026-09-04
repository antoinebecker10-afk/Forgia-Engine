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

use crate::biome_spec::{load_all_biome_specs, BiomeRoadConfig, BiomeSpec};
use crate::biomes::BiomeType;

// ─────────────────────────── BiomeRegistry ───────────────────────────

#[derive(Resource, Default)]
pub struct BiomeRegistry {
    specs: HashMap<BiomeType, BiomeSpec>,
}

impl BiomeRegistry {
    pub fn load() -> Self {
        let dir = crate::config_dir::find_config_dir().join("biomes");
        let reg = Self::from_specs(load_all_biome_specs(&dir));
        info!(
            "BiomeRegistry: loaded {} biome specs from TOML",
            reg.specs.len()
        );
        reg
    }

    /// Construit le registry depuis des specs déjà chargées (sans I/O disque).
    /// Utilisé par `load()`, le hot-reload et les tests de validation TOML
    /// (story-575 : prouve que les TOML miroir == fallbacks Rust).
    pub fn from_specs(specs_vec: Vec<BiomeSpec>) -> Self {
        let mut specs = HashMap::new();
        for spec in specs_vec {
            if let Some(bt) = biome_type_from_id(&spec.id) {
                specs.insert(bt, spec);
            } else {
                warn!("BiomeRegistry: unknown biome id '{}', skipping", spec.id);
            }
        }
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
            BiomeType::Plains => (115, 166, 64),
            BiomeType::Forest => (51, 107, 38),
            BiomeType::Desert => (199, 173, 107),
            BiomeType::Mountain => (133, 128, 122),
            BiomeType::Swamp => (77, 97, 56),
            BiomeType::Tundra => (209, 224, 235),
            BiomeType::Savanna => (184, 166, 89),
            BiomeType::Jungle => (25, 89, 20),
            BiomeType::Volcanic => (64, 46, 38),
            BiomeType::Canyon => (173, 107, 64),
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
            BiomeType::Plains => (1.0, 1.0, 1.0),
            BiomeType::Forest => (1.1, 0.9, 1.1),
            BiomeType::Desert => (0.8, 1.2, 0.9),
            BiomeType::Mountain => (1.3, 0.8, 1.2),
            BiomeType::Swamp => (1.0, 0.7, 1.3),
            BiomeType::Tundra => (0.9, 1.1, 1.0),
            BiomeType::Savanna => (0.9, 1.1, 0.8),
            BiomeType::Jungle => (1.2, 0.8, 1.2),
            BiomeType::Volcanic => (1.4, 0.9, 1.5),
            BiomeType::Canyon => (1.1, 1.0, 1.1),
        }
    }

    pub fn spawn_weight(&self, biome: BiomeType) -> f32 {
        self.specs
            .get(&biome)
            .map(|s| s.spawn_weight)
            .unwrap_or(1.0)
    }

    pub fn height_mult_for(&self, biome: BiomeType) -> f32 {
        self.specs
            .get(&biome)
            .and_then(|s| s.height_mult)
            .unwrap_or(1.0)
    }

    /// Multiplicateur d'amplitude du relief par biome (Mountain montagneux, Plains
    /// plat). TOML `config/biomes/<id>.toml::amplitude_mult` si présent, sinon
    /// fallback hardcodé `BiomeType::amplitude_mult_default()`. Story-576 Incr.6.
    pub fn amplitude_mult_for(&self, biome: BiomeType) -> f32 {
        self.specs
            .get(&biome)
            .and_then(|s| s.amplitude_mult)
            .unwrap_or_else(|| biome.amplitude_mult_default())
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
    if _request.is_none() {
        return;
    }
    commands.remove_resource::<BiomeRegistryReloadRequest>();
    commands.insert_resource(BiomeRegistry::load());
    info!("BiomeRegistry: hot-reloaded from TOML files");
}

pub fn biome_hotreload_input_system(keyboard: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    if keyboard.pressed(KeyCode::ShiftLeft) && keyboard.just_pressed(KeyCode::F12) {
        commands.insert_resource(BiomeRegistryReloadRequest);
        info!("Biome hot-reload requested (Shift+F12)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_BIOMES: [BiomeType; 10] = [
        BiomeType::Plains,
        BiomeType::Forest,
        BiomeType::Desert,
        BiomeType::Mountain,
        BiomeType::Swamp,
        BiomeType::Tundra,
        BiomeType::Savanna,
        BiomeType::Jungle,
        BiomeType::Volcanic,
        BiomeType::Canyon,
    ];

    #[test]
    fn biome_type_from_id_covers_all_canonical_ids() {
        let ids = [
            "plains", "forest", "desert", "mountain", "swamp", "tundra", "savanna", "jungle",
            "volcanic", "canyon",
        ];
        for id in ids {
            assert!(
                biome_type_from_id(id).is_some(),
                "canonical id {id} must map to a BiomeType"
            );
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
            assert!(
                rc.width_mult > 0.0 && rc.width_mult.is_finite(),
                "{b:?}: width_mult {} must be > 0",
                rc.width_mult
            );
            assert!(
                rc.depression_mult >= 0.0 && rc.depression_mult.is_finite(),
                "{b:?}: depression_mult {}",
                rc.depression_mult
            );
            assert!(
                (0.0..=1.0).contains(&rc.vegetation_encroachment),
                "{b:?}: vegetation_encroachment {} outside [0, 1]",
                rc.vegetation_encroachment
            );
        }
    }

    #[test]
    fn enemy_modifiers_within_sane_bands() {
        let reg = BiomeRegistry::default();
        for b in ALL_BIOMES {
            let (hp, speed, dmg) = reg.enemy_modifiers(b);
            assert!(
                (0.5..=2.0).contains(&hp),
                "{b:?}: hp_mult {hp} outside [0.5, 2.0]"
            );
            assert!(
                (0.5..=2.0).contains(&speed),
                "{b:?}: speed_mult {speed} outside [0.5, 2.0]"
            );
            assert!(
                (0.5..=2.0).contains(&dmg),
                "{b:?}: dmg_mult {dmg} outside [0.5, 2.0]"
            );
        }
    }

    #[test]
    fn spawn_weight_default_is_one() {
        let reg = BiomeRegistry::default();
        for b in ALL_BIOMES {
            assert_eq!(reg.spawn_weight(b), 1.0, "default spawn_weight for {b:?}");
        }
    }

    /// Story-575 (audit procgen 2026-06-05) : les 10 config/biomes/*.toml sont un
    /// MIROIR EXACT des fallbacks Rust. Ce test charge les vrais TOML et prouve
    /// que chaque accessor renvoie la même valeur qu'un registry vide (fallbacks)
    /// → zero-régression garantie au `cargo test`, sans lancer le jeu.
    #[test]
    fn config_biomes_toml_mirror_rust_fallbacks_exactly() {
        use crate::biome_spec::load_all_biome_specs;
        use std::path::Path;

        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/biomes");
        let specs = load_all_biome_specs(&dir);
        assert_eq!(
            specs.len(),
            10,
            "attendu 10 biome TOMLs dans {dir:?}, trouvé {}",
            specs.len()
        );

        let toml = BiomeRegistry::from_specs(specs);
        let fb = BiomeRegistry::default();

        for b in ALL_BIOMES {
            assert!(toml.get(b).is_some(), "{b:?} manquant dans les TOML");
            assert_eq!(toml.color(b), fb.color(b), "color {b:?}");
            assert!(
                (toml.roughness(b) - fb.roughness(b)).abs() < 1e-6,
                "roughness {b:?}"
            );
            assert_eq!(toml.preview_rgb(b), fb.preview_rgb(b), "preview_rgb {b:?}");
            assert_eq!(
                toml.display_name_fr(b),
                fb.display_name_fr(b),
                "display_name_fr {b:?}"
            );
            assert_eq!(
                toml.enemy_modifiers(b),
                fb.enemy_modifiers(b),
                "enemy_modifiers {b:?}"
            );
            assert!(
                (toml.spawn_weight(b) - fb.spawn_weight(b)).abs() < 1e-6,
                "spawn_weight {b:?}"
            );
            let (rt, rf) = (toml.road_config(b), fb.road_config(b));
            assert!(
                (rt.width_mult - rf.width_mult).abs() < 1e-6
                    && (rt.depression_mult - rf.depression_mult).abs() < 1e-6
                    && (rt.edge_noise - rf.edge_noise).abs() < 1e-6
                    && (rt.vegetation_encroachment - rf.vegetation_encroachment).abs() < 1e-6,
                "road_config {b:?}: TOML {rt:?} != fallback {rf:?}"
            );
            // Champs Option volontairement omis → doivent rester None / 1.0.
            assert_eq!(
                toml.lacunarity_for(b),
                None,
                "lacunarity {b:?} doit rester None"
            );
            assert_eq!(toml.persistence_for(b), None, "persistence {b:?}");
            assert_eq!(toml.slope_max_for(b), None, "slope_max {b:?}");
            assert!(
                (toml.height_mult_for(b) - 1.0).abs() < 1e-6,
                "height_mult {b:?} doit rester 1.0"
            );
            // Story-576 Incr.6 : amplitude_mult des TOML == fallback hardcodé.
            assert!(
                (toml.amplitude_mult_for(b) - fb.amplitude_mult_for(b)).abs() < 1e-6
                    && (toml.amplitude_mult_for(b) - b.amplitude_mult_default()).abs() < 1e-6,
                "amplitude_mult {b:?}: TOML {} != fallback {}",
                toml.amplitude_mult_for(b),
                b.amplitude_mult_default()
            );
        }
    }
}
