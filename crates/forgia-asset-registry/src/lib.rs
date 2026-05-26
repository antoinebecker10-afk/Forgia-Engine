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
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub mod pack_registry;

/// Chemin du manifest TOML versionnable. Généré au 1er boot, lu ensuite.
const REGISTRY_TOML_PATH: &str = "assets/asset_registry.toml";

pub mod prelude {
    pub use crate::pack_registry::{ForgiaPackRegistryPlugin, PackRegistry, PackRuntimeEntry};
    pub use crate::{
        target_size_for, AssetCategory, AssetEntry, AssetQuery, AssetRegistry, AssetSeason,
        BiomeCompat, ForgiaAssetRegistryPlugin, NeedsAssetCalibrate,
    };
}

/// Marker posé par les consommateurs (foliage, rpg cavernes...) au spawn
/// d'un SceneRoot dont l'entry registry a `measured_size_m = None`. Le
/// système `calibrate_assets` poll chaque frame jusqu'à ce que l'AABB Bevy
/// soit dispo, mesure, applique le scale et write back dans registry + TOML.
#[derive(Component, Debug, Clone)]
pub struct NeedsAssetCalibrate {
    /// Path de l'entry registry à mettre à jour après mesure.
    pub entry_path: String,
    /// Taille cible visuelle (m) — vient de `target_size_for(category)`.
    pub target_size_m: f32,
    /// Scale jitter user-side, appliqué EN PLUS du scale de calibration.
    /// Default 1.0 = pas de variation. 0.85-1.15 = variation naturelle.
    pub user_scale: f32,
}

// ─────────────────────────── Tag enums ───────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetCategory {
    Tree,
    Bush,
    Ground, // flowers, grass tufts
    Cactus,
    Stump,
    Rock,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetSeason {
    Default, // été/printemps neutre
    Autumn,
    Winter, // = snow
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BiomeCompat {
    Any,
    Desert,
    Tundra,   // = neige cold
    Forest,   // = humide tempéré
    Volcanic, // = brûlé dead
}

// ─────────────────────────── AssetEntry ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    /// Chemin asset relatif au workspace (utilisable par `asset_server.load`).
    pub path: String,
    pub species: String,
    pub category: AssetCategory,
    pub season: AssetSeason,
    pub biome_compat: BiomeCompat,
    #[serde(default)]
    pub is_dead: bool,
    /// `true` si le GLB pointé par `path` est introuvable sur disque. Skippé
    /// par `query()`. Permet de conserver l'entry dans le TOML pour history /
    /// résurrection si l'asset revient.
    #[serde(default)]
    pub missing: bool,
    /// `true` si l'utilisateur veut désactiver temporairement cet asset sans
    /// le supprimer (ex : tester un visuel sans les arbres morts).
    #[serde(default)]
    pub disabled: bool,
    /// Dimension max AABB (mètres) mesurée auto au 1er spawn. `None` = jamais
    /// calibré. Override manuel possible via édition TOML pour fix de mesh
    /// avec racine bizarre qui fausse l'AABB.
    #[serde(default)]
    pub measured_size_m: Option<f32>,
}

/// Wrapper TOML — array `[[asset]]` au top-level.
#[derive(Debug, Default, Serialize, Deserialize)]
struct RegistryToml {
    asset: Vec<AssetEntry>,
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
    pub fn new() -> Self {
        Self::default()
    }
    pub fn category(mut self, c: AssetCategory) -> Self {
        self.category = Some(c);
        self
    }
    pub fn biome(mut self, b: BiomeType) -> Self {
        self.biome = Some(b);
        self
    }
    pub fn season(mut self, s: AssetSeason) -> Self {
        self.season = Some(s);
        self
    }
    pub fn alive(mut self) -> Self {
        self.exclude_dead = true;
        self
    }
}

/// Mapping `BiomeType` ↔ `BiomeCompat` (les biomes V2 → familles asset).
fn biome_to_compat(b: BiomeType) -> BiomeCompat {
    match b {
        BiomeType::Desert | BiomeType::Canyon | BiomeType::Savanna => BiomeCompat::Desert,
        BiomeType::Tundra | BiomeType::Mountain => BiomeCompat::Tundra,
        BiomeType::Forest | BiomeType::Jungle | BiomeType::Plains | BiomeType::Swamp => {
            BiomeCompat::Forest
        }
        BiomeType::Volcanic => BiomeCompat::Volcanic,
    }
}

// ─────────────────────────── AssetRegistry ───────────────────────────

#[derive(Resource, Default)]
pub struct AssetRegistry {
    entries: Vec<AssetEntry>,
    /// Dirty flag levé par les calibrations runtime, consommé par le système
    /// `save_dirty_registry` (débounce 2s).
    dirty: bool,
}

impl AssetRegistry {
    pub fn entries(&self) -> &[AssetEntry] {
        &self.entries
    }

    /// Lookup par path exact. Retourne `None` si absent.
    pub fn find(&self, path: &str) -> Option<&AssetEntry> {
        self.entries.iter().find(|e| e.path == path)
    }

    /// Met à jour `measured_size_m` pour une entry. Lève le dirty flag pour
    /// déclencher save_toml différé.
    pub fn set_measured_size(&mut self, path: &str, size_m: f32) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.path == path) {
            e.measured_size_m = Some(size_m);
            self.dirty = true;
        }
    }

    pub fn query(&self, q: &AssetQuery) -> Vec<&AssetEntry> {
        let biome_compat = q.biome.map(biome_to_compat);
        self.entries
            .iter()
            .filter(|e| !e.missing && !e.disabled) // skip disabled/missing
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

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
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
    let (category, species, biome_compat) = if lower.contains("cactus") {
        (
            AssetCategory::Cactus,
            "cactus".to_string(),
            BiomeCompat::Desert,
        )
    } else if lower.contains("palm") {
        (AssetCategory::Tree, "palm".to_string(), BiomeCompat::Desert)
    } else if lower.contains("stump") {
        (
            AssetCategory::Stump,
            extract_species(&lower),
            BiomeCompat::Any,
        )
    } else if lower.contains("rock") || lower.contains("cliff") {
        (AssetCategory::Rock, "rock".to_string(), BiomeCompat::Any)
    } else if lower.starts_with("bush") || lower.contains("_bush") {
        let bc = if season == AssetSeason::Winter {
            BiomeCompat::Tundra
        } else {
            BiomeCompat::Any
        };
        (AssetCategory::Bush, "bush".to_string(), bc)
    } else if lower.contains("flower") || lower.contains("grass") {
        (
            AssetCategory::Ground,
            "ground".to_string(),
            BiomeCompat::Any,
        )
    } else if lower.contains("birch") {
        let bc = if season == AssetSeason::Winter {
            BiomeCompat::Tundra
        } else {
            BiomeCompat::Forest
        };
        (AssetCategory::Tree, "birch".to_string(), bc)
    } else if lower.contains("pine") {
        let bc = if season == AssetSeason::Winter {
            BiomeCompat::Tundra
        } else {
            BiomeCompat::Forest
        };
        (AssetCategory::Tree, "pine".to_string(), bc)
    } else if lower.contains("twisted_tree") || lower.contains("twistedtree") {
        (
            AssetCategory::Tree,
            "twisted".to_string(),
            BiomeCompat::Forest,
        )
    } else if lower.contains("common_tree") || lower.contains("commontree") {
        let bc = if season == AssetSeason::Winter {
            BiomeCompat::Tundra
        } else {
            BiomeCompat::Forest
        };
        (AssetCategory::Tree, "common".to_string(), bc)
    } else if lower.contains("tree") {
        (
            AssetCategory::Tree,
            extract_species(&lower),
            BiomeCompat::Forest,
        )
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
        missing: false,
        disabled: false,
        measured_size_m: None,
    }
}

// ─────────────────────────── Target size par catégorie ───────────────────────────

/// Cible visuelle en mètres pour chaque catégorie d'asset. Source de vérité
/// transitoire (structural visual exception au no-hardcode rule) — à migrer
/// vers `vegetation_default.toml` quand `forgia-genome-registry` sortira du
/// stub.
pub fn target_size_for(category: AssetCategory) -> f32 {
    match category {
        AssetCategory::Tree => 4.5,   // arbre adulte mâture (cèdre/birch ~5m)
        AssetCategory::Bush => 1.0,   // arbuste taille humaine
        AssetCategory::Ground => 0.4, // fleurs, touffes herbe
        AssetCategory::Cactus => 2.0, // cactus saguaro géant
        AssetCategory::Stump => 0.7,  // souche basse
        AssetCategory::Rock => 1.5,   // rocher décor
        AssetCategory::Other => 1.5,  // fallback safe
    }
}

fn extract_species(lower: &str) -> String {
    lower
        .split(['_', '.'])
        .next()
        .unwrap_or("unknown")
        .to_string()
}

/// Scan récursif d'un dossier pour les `.glb` et `.gltf`. Path résolu relatif
/// au CWD du process (workspace root en cargo run).
fn scan_dir(root: &Path, asset_prefix: &str) -> Vec<AssetEntry> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
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
        let Some(ext) = p.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if ext != "glb" && ext != "gltf" {
            continue;
        }
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
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
            .add_systems(Startup, populate_registry)
            .add_systems(
                Update,
                (hot_reload_input, calibrate_assets, save_dirty_registry),
            );
    }
}

/// Poll les entités `NeedsAssetCalibrate` chaque frame, attend que Bevy ait
/// chargé la GLB scene et calculé les `Aabb` sur les Mesh3d children, puis
/// mesure l'AABB max, calcule scale = target / max_dim, applique au Transform
/// et persiste dans le registry.
fn calibrate_assets(
    mut commands: Commands,
    mut registry: ResMut<AssetRegistry>,
    q_needs: Query<(Entity, &NeedsAssetCalibrate)>,
    q_aabb: Query<&bevy::camera::primitives::Aabb>,
    q_children: Query<&bevy::ecs::hierarchy::Children>,
    mut q_transform: Query<&mut Transform>,
) {
    for (entity, needs) in &q_needs {
        let Some(max_dim) = compute_aabb_max_dim(entity, &q_aabb, &q_children) else {
            continue; // Scene pas encore chargée, retry next frame
        };
        if max_dim <= 0.0 || !max_dim.is_finite() {
            warn!(
                "[asset-registry] {} : AABB invalide ({max_dim}) — skip calibration",
                needs.entry_path
            );
            commands.entity(entity).remove::<NeedsAssetCalibrate>();
            continue;
        }
        let scale = needs.target_size_m / max_dim * needs.user_scale;
        if let Ok(mut tf) = q_transform.get_mut(entity) {
            tf.scale = Vec3::splat(scale);
        }
        registry.set_measured_size(&needs.entry_path, max_dim);
        info!(
            "[asset-registry] Calibrated {} : measured {:.2}m → scale {:.3} (target {:.1}m)",
            needs.entry_path, max_dim, scale, needs.target_size_m
        );
        commands.entity(entity).remove::<NeedsAssetCalibrate>();
    }
}

/// Walk récursif des Children pour trouver le premier `Aabb` disponible.
/// Retourne `max(half_extents) * 2` (dimension max bounding box).
fn compute_aabb_max_dim(
    root: Entity,
    q_aabb: &Query<&bevy::camera::primitives::Aabb>,
    q_children: &Query<&bevy::ecs::hierarchy::Children>,
) -> Option<f32> {
    // Direct sur root
    if let Ok(a) = q_aabb.get(root) {
        return Some(a.half_extents.max_element() * 2.0);
    }
    // Sinon descend dans les children
    let children = q_children.get(root).ok()?;
    let mut max: f32 = 0.0;
    let mut found = false;
    for child in children.iter() {
        if let Some(d) = compute_aabb_max_dim(child, q_aabb, q_children) {
            max = max.max(d);
            found = true;
        }
    }
    if found {
        Some(max)
    } else {
        None
    }
}

/// Save TOML debounced toutes les 2s si dirty flag levé. Évite N writes par
/// frame quand plusieurs calibrations terminent en même temps.
fn save_dirty_registry(
    time: Res<Time>,
    mut registry: ResMut<AssetRegistry>,
    mut last_save: Local<f32>,
) {
    if !registry.dirty {
        return;
    }
    let now = time.elapsed_secs();
    if now - *last_save < 2.0 {
        return;
    }
    *last_save = now;
    if let Err(e) = save_toml(&registry.entries) {
        warn!("[asset-registry] save_dirty failed : {e}");
    } else {
        registry.dirty = false;
    }
}

/// Charge le TOML manifest s'il existe, sinon scan + write au 1er boot.
/// Détecte les nouveaux GLBs (présents sur disque, absents du TOML) → ajoutés.
/// Détecte les entries fantômes (présentes au TOML, GLB disparu) → marquées
/// `missing = true` (skippées par query, mais conservées pour history/restore).
fn populate_registry(mut registry: ResMut<AssetRegistry>) {
    let mut entries = match load_toml() {
        Some(loaded) => {
            info!(
                "[asset-registry] Loaded {} entries from {REGISTRY_TOML_PATH}",
                loaded.len()
            );
            loaded
        }
        None => {
            info!("[asset-registry] No TOML manifest → first-boot scan");
            let scanned = scan_nature_root();
            // Écriture initiale du TOML pour que l'user puisse l'éditer.
            if !scanned.is_empty() {
                if let Err(e) = save_toml(&scanned) {
                    warn!("[asset-registry] Failed to write initial TOML : {e}");
                }
            }
            scanned
        }
    };

    // Réconciliation filesystem ↔ TOML.
    reconcile_with_filesystem(&mut entries);

    // Stats + sensor.
    let total = entries.len();
    let active = entries.iter().filter(|e| !e.missing && !e.disabled).count();
    let trees = entries
        .iter()
        .filter(|e| e.category == AssetCategory::Tree && !e.missing)
        .count();
    let autumn = entries
        .iter()
        .filter(|e| e.season == AssetSeason::Autumn && !e.missing)
        .count();
    let snow = entries
        .iter()
        .filter(|e| e.season == AssetSeason::Winter && !e.missing)
        .count();
    let dead = entries.iter().filter(|e| e.is_dead).count();
    let missing = entries.iter().filter(|e| e.missing).count();
    let disabled = entries.iter().filter(|e| e.disabled).count();
    info!(
        "[asset-registry] Active {active}/{total} : {trees} trees ({autumn} autumn, {snow} snow, {dead} dead) | missing={missing} disabled={disabled}"
    );

    let json = format!(
        "{{\"total\":{total},\"active\":{active},\"trees\":{trees},\"autumn\":{autumn},\"snow\":{snow},\"dead\":{dead},\"missing\":{missing},\"disabled\":{disabled}}}"
    );
    let _ = fs::write("forgia_asset_registry.json", json);

    registry.entries = entries;
}

/// Scan filesystem `assets/models-v1/nature/` → entries fraîchement taggés
/// par convention filename.
fn scan_nature_root() -> Vec<AssetEntry> {
    let root = Path::new("assets/models-v1/nature");
    if !root.exists() {
        warn!(
            "[asset-registry] {:?} introuvable — scan skipped. CWD must be workspace root.",
            root
        );
        return Vec::new();
    }
    scan_dir(root, "models-v1/nature")
}

fn load_toml() -> Option<Vec<AssetEntry>> {
    let content = fs::read_to_string(REGISTRY_TOML_PATH).ok()?;
    match toml::from_str::<RegistryToml>(&content) {
        Ok(r) => Some(r.asset),
        Err(e) => {
            warn!("[asset-registry] TOML parse error : {e}. Rescanning filesystem.");
            None
        }
    }
}

fn save_toml(entries: &[AssetEntry]) -> std::io::Result<()> {
    let wrapper = RegistryToml {
        asset: entries.to_vec(),
    };
    let body = toml::to_string_pretty(&wrapper)
        .map_err(|e| std::io::Error::other(format!("toml serialize : {e}")))?;
    let header = "# forgia-asset-registry — manifest auto-généré au 1er boot.\n# Éditable à la main. Hot-reload via Shift+F12 en cours de jeu.\n# Les entries `missing = true` sont conservées pour history (le GLB peut\n# revenir). `disabled = true` désactive temporairement sans supprimer.\n\n";
    fs::write(REGISTRY_TOML_PATH, format!("{header}{body}"))
}

/// Détecte les nouveaux GLBs (filesystem sans entry TOML) → ajoute taggés.
/// Détecte les entries TOML sans GLB → `missing = true`.
/// Écrit le TOML mis à jour si changements.
fn reconcile_with_filesystem(entries: &mut Vec<AssetEntry>) {
    let scanned = scan_nature_root();
    let mut changed = false;

    // 1. Marque missing les TOML entries dont le GLB n'existe plus.
    let scanned_paths: std::collections::HashSet<&str> =
        scanned.iter().map(|e| e.path.as_str()).collect();
    for e in entries.iter_mut() {
        let prev = e.missing;
        e.missing = !scanned_paths.contains(e.path.as_str());
        if e.missing != prev {
            changed = true;
        }
    }

    // 2. Ajoute les nouveaux scannés (filesystem présents, TOML absents).
    let existing_paths: std::collections::HashSet<String> =
        entries.iter().map(|e| e.path.clone()).collect();
    let mut added = 0;
    for s in scanned {
        if !existing_paths.contains(&s.path) {
            entries.push(s);
            added += 1;
            changed = true;
        }
    }
    if added > 0 {
        info!("[asset-registry] {added} new assets detected and added to TOML");
    }

    if changed {
        if let Err(e) = save_toml(entries) {
            warn!("[asset-registry] Failed to write updated TOML : {e}");
        }
    }
}

/// Shift+F12 → reload TOML à chaud. Permet d'éditer asset_registry.toml en
/// cours de jeu et voir les nouveaux tags appliqués au prochain spawn chunk.
fn hot_reload_input(keys: Res<ButtonInput<KeyCode>>, mut registry: ResMut<AssetRegistry>) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if shift && keys.just_pressed(KeyCode::F12) {
        match load_toml() {
            Some(mut new_entries) => {
                reconcile_with_filesystem(&mut new_entries);
                let n = new_entries.len();
                registry.entries = new_entries;
                info!("[asset-registry] Hot-reloaded TOML : {n} entries");
            }
            None => warn!("[asset-registry] Hot-reload failed (TOML invalid or missing)"),
        }
    }
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
        let reg = AssetRegistry {
            entries: vec![
                tag_from_filename("birch_tree_autumn_1"),
                tag_from_filename("cactus_2"),
                tag_from_filename("bush_1"),
            ],
            ..Default::default()
        };
        let q = AssetQuery::new()
            .category(AssetCategory::Tree)
            .biome(BiomeType::Forest);
        let res = reg.query(&q);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].species, "birch");
    }

    #[test]
    fn query_alive_filters_dead() {
        let reg = AssetRegistry {
            entries: vec![
                tag_from_filename("common_tree_dead_1"),
                tag_from_filename("common_tree_1"),
            ],
            ..Default::default()
        };
        let q = AssetQuery::new().category(AssetCategory::Tree).alive();
        let res = reg.query(&q);
        assert_eq!(res.len(), 1);
        assert!(!res[0].is_dead);
    }
}
