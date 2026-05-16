//! # forgia-asset-registry
//!
//! Scan `assets/models-v1/nature/` au Startup et tagge chaque GLB par convention
//! filename. Expose une `Resource AssetRegistry` queryable par les consommateurs
//! (foliage, rpg cavernes, villages, NPCs futurs).
//!
//! ## Conventions parsées (filename V1 nature)
//!
//! | Pattern              | Inférence                                  |
//! |----------------------|--------------------------------------------|
//! | `*_autumn_*`         | season=Autumn                              |
//! | `*_snow_*`           | season=Winter + biome_compat=Tundra/Mountain |
//! | `*_dead_*`           | condition=Dead                             |
//! | `cactus_*`           | category=Cactus + biome=Desert             |
//! | `bush*` / `*_bush_*` | category=Bush                              |
//! | `birch_tree_*`       | species=birch + category=Tree              |
//! | `common_tree_*`      | species=common + category=Tree             |
//! | `twisted_tree_*`     | species=twisted + category=Tree            |
//! | `pine_*` / `tree_pine_*` | species=pine + category=Tree           |
//! | `flower*` / `grass_*`| category=Ground                            |
//! | `stump_*` / `*_stump`| category=Stump                             |
//! | `rock_*`             | category=Rock                              |
//! | `*_palm_*`           | category=Tree + biome=Desert + tropical    |
//!
//! Filename sans pattern reconnu → category=Other (filtré par défaut).

use bevy::prelude::*;
use forgia_terrain::BiomeType;
use std::fs;
use std::path::{Path, PathBuf};

pub mod prelude {
    pub use crate::{
        AssetCategory, AssetEntry, AssetQuery, AssetRegistry, AssetSeason, BiomeCompat,
        ForgiaAssetRegistryPlugin,
    };
}

// ─────────────────────────── Tag enums ───────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetCategory {
    Tree,
    Bush,
    Ground,  // flowers, grass tufts
    Cactus,
    Stump,
    Rock,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetSeason {
    Default, // été/printemps neutre
    Autumn,
    Winter,  // = snow
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiomeCompat {
    Any,
    Desert,
    Tundra,    // = neige cold
    Forest,    // = humide tempéré
    Volcanic,  // = brûlé dead
}

// ─────────────────────────── AssetEntry ───────────────────────────

#[derive(Debug, Clone)]
pub struct AssetEntry {
    /// Chemin asset relatif au workspace (utilisable par `asset_server.load`).
    pub path: String,
    pub species: String,
    pub category: AssetCategory,
    pub season: AssetSeason,
    pub biome_compat: BiomeCompat,
    pub is_dead: bool,
}

// ─────────────────────────── Query ───────────────────────────

#[derive(Debug, Clone, Default)]
pub struct AssetQuery {
    pub category: Option<AssetCategory>,
    pub biome: Option<BiomeType>,
    pub season: Option<AssetSeason>,
    pub exclude_dead: bool,
}

impl AssetQuery {
    pub fn new() -> Self { Self::default() }
    pub fn category(mut self, c: AssetCategory) -> Self { self.category = Some(c); self }
    pub fn biome(mut self, b: BiomeType) -> Self { self.biome = Some(b); self }
    pub fn season(mut self, s: AssetSeason) -> Self { self.season = Some(s); self }
    pub fn alive(mut self) -> Self { self.exclude_dead = true; self }
}

/// Mapping `BiomeType` ↔ `BiomeCompat` (les biomes V2 → familles asset).
fn biome_to_compat(b: BiomeType) -> BiomeCompat {
    match b {
        BiomeType::Desert | BiomeType::Canyon | BiomeType::Savanna => BiomeCompat::Desert,
        BiomeType::Tundra | BiomeType::Mountain => BiomeCompat::Tundra,
        BiomeType::Forest | BiomeType::Jungle | BiomeType::Plains | BiomeType::Swamp => BiomeCompat::Forest,
        BiomeType::Volcanic => BiomeCompat::Volcanic,
    }
}

// ─────────────────────────── AssetRegistry ───────────────────────────

#[derive(Resource, Default)]
pub struct AssetRegistry {
    entries: Vec<AssetEntry>,
}

impl AssetRegistry {
    pub fn entries(&self) -> &[AssetEntry] { &self.entries }

    pub fn query(&self, q: &AssetQuery) -> Vec<&AssetEntry> {
        let biome_compat = q.biome.map(biome_to_compat);
        self.entries
            .iter()
            .filter(|e| q.category.is_none_or(|c| e.category == c))
            .filter(|e| q.season.is_none_or(|s| e.season == s))
            .filter(|e| {
                biome_compat.is_none_or(|bc| {
                    e.biome_compat == BiomeCompat::Any || e.biome_compat == bc
                })
            })
            .filter(|e| !q.exclude_dead || !e.is_dead)
            .collect()
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

// ─────────────────────────── Scanner ───────────────────────────

/// Tag un GLB d'après son filename. Pattern matching cumulatif (un asset peut
/// avoir plusieurs tags : `birch_tree_autumn_3` → species=birch, category=Tree,
/// season=Autumn).
fn tag_from_filename(stem: &str) -> AssetEntry {
    let lower = stem.to_lowercase();

    // Saison
    let season = if lower.contains("autumn") {
        AssetSeason::Autumn
    } else if lower.contains("snow") {
        AssetSeason::Winter
    } else {
        AssetSeason::Default
    };

    let is_dead = lower.contains("dead");

    // Catégorie + species (heuristique ordre = priorité)
    let (category, species, biome_compat) =
        if lower.contains("cactus") {
            (AssetCategory::Cactus, "cactus".to_string(), BiomeCompat::Desert)
        } else if lower.contains("palm") {
            (AssetCategory::Tree, "palm".to_string(), BiomeCompat::Desert)
        } else if lower.contains("stump") {
            (AssetCategory::Stump, extract_species(&lower), BiomeCompat::Any)
        } else if lower.contains("rock") || lower.contains("cliff") {
            (AssetCategory::Rock, "rock".to_string(), BiomeCompat::Any)
        } else if lower.starts_with("bush") || lower.contains("_bush") {
            let bc = if season == AssetSeason::Winter { BiomeCompat::Tundra } else { BiomeCompat::Any };
            (AssetCategory::Bush, "bush".to_string(), bc)
        } else if lower.contains("flower") || lower.contains("grass") {
            (AssetCategory::Ground, "ground".to_string(), BiomeCompat::Any)
        } else if lower.contains("birch") {
            let bc = if season == AssetSeason::Winter { BiomeCompat::Tundra } else { BiomeCompat::Forest };
            (AssetCategory::Tree, "birch".to_string(), bc)
        } else if lower.contains("pine") {
            let bc = if season == AssetSeason::Winter { BiomeCompat::Tundra } else { BiomeCompat::Forest };
            (AssetCategory::Tree, "pine".to_string(), bc)
        } else if lower.contains("twisted_tree") || lower.contains("twistedtree") {
            (AssetCategory::Tree, "twisted".to_string(), BiomeCompat::Forest)
        } else if lower.contains("common_tree") || lower.contains("commontree") {
            let bc = if season == AssetSeason::Winter { BiomeCompat::Tundra } else { BiomeCompat::Forest };
            (AssetCategory::Tree, "common".to_string(), bc)
        } else if lower.contains("tree") {
            (AssetCategory::Tree, extract_species(&lower), BiomeCompat::Forest)
        } else {
            (AssetCategory::Other, lower, BiomeCompat::Any)
        };

    AssetEntry {
        path: String::new(),
        species,
        category,
        season,
        biome_compat,
        is_dead,
    }
}

fn extract_species(lower: &str) -> String {
    lower.split(['_', '.']).next().unwrap_or("unknown").to_string()
}

/// Scan récursif d'un dossier pour les `.glb` et `.gltf`. Path résolu relatif
/// au CWD du process (workspace root en cargo run).
fn scan_dir(root: &Path, asset_prefix: &str) -> Vec<AssetEntry> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else { return out };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            let mut sub_prefix = PathBuf::from(asset_prefix);
            if let Some(name) = p.file_name() {
                sub_prefix.push(name);
            }
            out.extend(scan_dir(&p, sub_prefix.to_string_lossy().as_ref()));
            continue;
        }
        let Some(ext) = p.extension().and_then(|s| s.to_str()) else { continue };
        if ext != "glb" && ext != "gltf" { continue }
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
        let mut entry = tag_from_filename(stem);
        let filename = format!("{}.{}", stem, ext);
        entry.path = format!("{asset_prefix}/{filename}").replace('\\', "/");
        out.push(entry);
    }
    out
}

// ─────────────────────────── Plugin ───────────────────────────

pub struct ForgiaAssetRegistryPlugin;

impl Plugin for ForgiaAssetRegistryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetRegistry>()
            .add_systems(Startup, populate_registry);
    }
}

fn populate_registry(mut registry: ResMut<AssetRegistry>) {
    // Scan le dossier nature V1 via la junction V2 `assets/models-v1/`.
    let root = Path::new("assets/models-v1/nature");
    if !root.exists() {
        warn!("[asset-registry] {:?} introuvable — scan skipped. CWD must be workspace root.", root);
        return;
    }
    registry.entries = scan_dir(root, "models-v1/nature");

    // Stats summary
    let total = registry.entries.len();
    let trees = registry.entries.iter().filter(|e| e.category == AssetCategory::Tree).count();
    let autumn = registry.entries.iter().filter(|e| e.season == AssetSeason::Autumn).count();
    let snow = registry.entries.iter().filter(|e| e.season == AssetSeason::Winter).count();
    let dead = registry.entries.iter().filter(|e| e.is_dead).count();
    info!(
        "[asset-registry] Scanned {total} GLBs : {trees} trees ({autumn} autumn, {snow} snow, {dead} dead)"
    );

    // Sensor JSON pour debug (observability-required).
    let json = format!(
        "{{\"total\":{total},\"trees\":{trees},\"autumn\":{autumn},\"snow\":{snow},\"dead\":{dead}}}"
    );
    let _ = fs::write("forgia_asset_registry.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_autumn_birch_is_tree_autumn_forest() {
        let e = tag_from_filename("birch_tree_autumn_3");
        assert_eq!(e.category, AssetCategory::Tree);
        assert_eq!(e.season, AssetSeason::Autumn);
        assert_eq!(e.species, "birch");
        assert_eq!(e.biome_compat, BiomeCompat::Forest);
        assert!(!e.is_dead);
    }

    #[test]
    fn tag_cactus_is_desert() {
        let e = tag_from_filename("cactus_4");
        assert_eq!(e.category, AssetCategory::Cactus);
        assert_eq!(e.biome_compat, BiomeCompat::Desert);
    }

    #[test]
    fn tag_snow_birch_is_tundra_winter() {
        let e = tag_from_filename("birch_tree_snow_2");
        assert_eq!(e.season, AssetSeason::Winter);
        assert_eq!(e.biome_compat, BiomeCompat::Tundra);
    }

    #[test]
    fn tag_dead_is_flagged() {
        let e = tag_from_filename("common_tree_dead_1");
        assert!(e.is_dead);
        assert_eq!(e.category, AssetCategory::Tree);
    }

    #[test]
    fn query_filters_by_category_and_biome() {
        let mut reg = AssetRegistry::default();
        reg.entries = vec![
            tag_from_filename("birch_tree_autumn_1"),
            tag_from_filename("cactus_2"),
            tag_from_filename("bush_1"),
        ];
        let q = AssetQuery::new()
            .category(AssetCategory::Tree)
            .biome(BiomeType::Forest);
        let res = reg.query(&q);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].species, "birch");
    }

    #[test]
    fn query_alive_filters_dead() {
        let mut reg = AssetRegistry::default();
        reg.entries = vec![
            tag_from_filename("common_tree_dead_1"),
            tag_from_filename("common_tree_1"),
        ];
        let q = AssetQuery::new().category(AssetCategory::Tree).alive();
        let res = reg.query(&q);
        assert_eq!(res.len(), 1);
        assert!(!res[0].is_dead);
    }
}
