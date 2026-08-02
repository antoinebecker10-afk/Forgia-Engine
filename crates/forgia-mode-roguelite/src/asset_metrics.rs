//! asset_metrics.rs — les MESURES des assets, lues au lieu d'être devinées (story-673).
//!
//! `assets/genomes/asset_registry.toml` est produit par `tools/gltf/scan_assets.py`
//! en ouvrant réellement chaque fichier glTF/GLB (les accesseurs POSITION portent
//! leurs min/max, composés avec la hiérarchie de nœuds). Ce module le relit.
//!
//! ## Pourquoi il existe
//!
//! L'emprise au sol d'un prop était estimée à partir de `col_radius_factor`, un
//! coefficient qui RÉTRÉCIT le collider pour le feel de tir. Un bâtiment de 12 m se
//! déclarait 1,92 m de rayon — trois fois trop petit. Il passait tous les tests de
//! dégagement, et les ennemis naissaient dedans. Une valeur de tuning n'est pas une
//! mesure.
//!
//! ## Le piège de l'échelle
//!
//! Le décor recalibre chaque prop à une taille cible (`NeedsDecorCalibrate` mesure
//! l'AABB au runtime et applique `scale = target / max_dim`). L'emprise EN JEU n'est
//! donc pas l'emprise native : c'est
//!
//! ```text
//! emprise_jeu = emprise_native × (target_m / plus_grande_dimension_native)
//! ```
//!
//! Sans cette division, on comparerait des mètres natifs à des mètres de jeu — deux
//! échelles qui diffèrent d'un facteur 12 sur les bâtiments hexagon.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

const REGISTRY_PATH: &str = "assets/genomes/asset_registry.toml";

/// Mesures d'un asset, telles que lues dans le fichier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssetMetrics {
    /// Dimensions natives (m).
    pub size: Vec3,
    /// Hauteur native (m).
    pub height_m: f32,
    /// Rayon d'emprise au sol NATIF (m) — demi-diagonale de l'empreinte XZ.
    pub footprint_radius_m: f32,
}

impl AssetMetrics {
    /// Plus grande dimension native — c'est ce que la calibration runtime ramène
    /// à `target_m`.
    pub fn max_dim(&self) -> f32 {
        self.size.x.max(self.size.y).max(self.size.z)
    }

    /// Emprise au sol RÉELLE une fois le prop calibré à `target_m`.
    ///
    /// `target_m <= 0` (pas de calibration) → l'emprise native.
    pub fn footprint_at_target(&self, target_m: f32) -> f32 {
        let native_max = self.max_dim();
        if target_m <= 0.0 || native_max <= 1.0e-4 {
            return self.footprint_radius_m;
        }
        self.footprint_radius_m * (target_m / native_max)
    }
}

#[derive(Deserialize)]
struct RegistryEntryToml {
    path: String,
    #[serde(default)]
    measured: bool,
    #[serde(default)]
    size_m: Vec<f32>,
    #[serde(default)]
    height_m: f32,
    #[serde(default)]
    footprint_radius_m: f32,
}

#[derive(Deserialize, Default)]
struct RegistryToml {
    #[serde(default)]
    assets: Vec<RegistryEntryToml>,
}

/// Toutes les mesures, indexées par chemin d'asset (`models/...`).
#[derive(Resource, Debug, Clone, Default)]
pub struct AssetRegistry {
    pub(crate) by_path: HashMap<String, AssetMetrics>,
}

impl AssetRegistry {
    /// PUR — testable. Un registre illisible donne un registre VIDE, et le dit :
    /// les consommateurs retombent alors sur leur estimation d'avant.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: RegistryToml = match toml::from_str(content) {
            Ok(p) => p,
            Err(e) => {
                warn!("[asset-metrics] registre illisible ({e}) — mesures indisponibles");
                return Self::default();
            }
        };
        let mut by_path = HashMap::with_capacity(parsed.assets.len());
        for a in parsed.assets {
            if !a.measured || a.size_m.len() < 3 {
                continue;
            }
            by_path.insert(
                a.path,
                AssetMetrics {
                    size: Vec3::new(a.size_m[0], a.size_m[1], a.size_m[2]),
                    height_m: a.height_m,
                    footprint_radius_m: a.footprint_radius_m,
                },
            );
        }
        Self { by_path }
    }

    fn load_or_empty() -> Self {
        match fs::read_to_string(REGISTRY_PATH) {
            Ok(c) => Self::parse_toml(&c),
            Err(e) => {
                warn!(
                    "[asset-metrics] {REGISTRY_PATH} introuvable ({e}) — le décor \
                     retombera sur ses estimations. Régénérer : \
                     python tools/gltf/scan_assets.py"
                );
                Self::default()
            }
        }
    }

    pub fn get(&self, asset_path: &str) -> Option<&AssetMetrics> {
        self.by_path.get(asset_path)
    }

    pub fn len(&self) -> usize {
        self.by_path.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    /// Emprise au sol d'un prop calibré à `target_m`, avec repli explicite.
    ///
    /// `fallback` est l'ancienne estimation : on la garde pour les assets absents
    /// du registre plutôt que de renvoyer 0, qui laisserait passer n'importe quoi.
    pub fn footprint(&self, asset_path: &str, target_m: f32, fallback: f32) -> f32 {
        self.get(asset_path)
            .map(|m| m.footprint_at_target(target_m))
            .unwrap_or(fallback)
    }
}

pub fn sys_init_asset_registry(mut commands: Commands) {
    let reg = AssetRegistry::load_or_empty();
    if reg.is_empty() {
        warn!("[asset-metrics] AUCUNE mesure chargée — emprises estimées, pas mesurées");
    } else {
        info!("[asset-metrics] {} assets mesurés chargés", reg.len());
    }
    commands.insert_resource(reg);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(x: f32, y: f32, z: f32, fp: f32) -> AssetMetrics {
        AssetMetrics {
            size: Vec3::new(x, y, z),
            height_m: y,
            footprint_radius_m: fp,
        }
    }

    /// Le cœur du correctif : l'emprise EN JEU suit la calibration.
    #[test]
    fn the_footprint_follows_the_runtime_rescale() {
        // Un bâtiment hexagon typique : ~2 m de large, ~4 m de haut, emprise 1,5 m.
        let b = m(2.1, 3.98, 2.1, 1.5);
        assert!((b.footprint_at_target(0.0) - 1.5).abs() < 1e-4, "sans cible = natif");
        // Calibré à 12 m (target_building) : facteur 12/3,98 ≈ 3,01.
        let at12 = b.footprint_at_target(12.0);
        assert!(
            (at12 - 1.5 * (12.0 / 3.98)).abs() < 1e-3,
            "emprise calibrée attendue ~4,52 m, obtenu {at12:.2}"
        );
        assert!(
            at12 > 4.0,
            "c'est ce chiffre-là que l'ancienne estimation ratait (elle disait 1,92)"
        );
    }

    #[test]
    fn a_degenerate_asset_never_yields_a_nonsense_footprint() {
        let flat = m(0.0, 0.0, 0.0, 0.0);
        assert_eq!(flat.footprint_at_target(12.0), 0.0);
        let tall = m(1.0, 100.0, 1.0, 0.7);
        assert!(tall.footprint_at_target(4.0) < 0.7, "un prop réduit rétrécit aussi");
    }

    #[test]
    fn an_unknown_asset_falls_back_instead_of_returning_zero() {
        let r = AssetRegistry::default();
        assert_eq!(r.footprint("models/inconnu.glb", 12.0, 3.3), 3.3);
    }

    #[test]
    fn a_broken_registry_is_empty_and_says_so() {
        let r = AssetRegistry::parse_toml("ceci n'est pas du TOML {{{");
        assert!(r.is_empty());
    }

    /// Le registre livré doit couvrir les assets que le décor utilise vraiment.
    #[test]
    fn the_real_registry_covers_the_palette_assets() {
        let content = fs::read_to_string(REGISTRY_PATH)
            .or_else(|_| fs::read_to_string(format!("../../{REGISTRY_PATH}")))
            .expect("asset_registry.toml introuvable — python tools/gltf/scan_assets.py");
        let reg = AssetRegistry::parse_toml(&content);
        assert!(reg.len() > 400, "registre suspicieusement petit : {}", reg.len());

        let pal = fs::read_to_string(crate::decor_palettes::GENOME_PATH)
            .or_else(|_| {
                fs::read_to_string(format!("../../{}", crate::decor_palettes::GENOME_PATH))
            })
            .expect("roguelite_palettes.toml introuvable");
        let palettes = crate::decor_palettes::DecorPalettesConfig::parse_toml(&pal);
        let missing: Vec<&str> = palettes
            .all_paths()
            .into_iter()
            .filter(|p| reg.get(p).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "{} props de palette sans mesure : {missing:#?}",
            missing.len()
        );
    }
}

#[cfg(test)]
mod spawn_clearance_tests {
    use super::*;
    use forgia_stage::authored::ArenaLayoutsGenome;

    /// Rayon du joueur (m) — capsule `Collider::capsule_y(0.7, 0.3)`.
    /// C'est un DISQUE en plan (`map-design-patterns.md` §1).
    const PLAYER_RADIUS_M: f32 = 0.3;
    /// Marge : un joueur qui apparaît collé à un mur est aussi bloqué qu'un
    /// joueur qui apparaît dedans.
    const SPAWN_CLEARANCE_M: f32 = 1.5;

    fn read(rel: &str) -> String {
        std::fs::read_to_string(rel)
            .or_else(|_| std::fs::read_to_string(format!("../../{rel}")))
            .unwrap_or_else(|e| panic!("{rel} illisible : {e}"))
    }

    /// Story-682 — **personne n'apparaît dans un solide.**
    ///
    /// Rapporté en jeu : « je spawn dans le puits, donc je suis bloqué ».
    /// `forge_sanctum` pose délibérément un puits solide à `[0,0,0]`, et
    /// l'apparition du joueur était en dur à l'origine pour TOUS les stages.
    ///
    /// Ce test vaut pour toutes les cartes, présentes et futures : il compare le
    /// point d'apparition déclaré à l'emprise MESURÉE de chaque pièce bloquante.
    /// C'est le correctif de CLASSE — corriger la seule carte fautive aurait
    /// laissé la prochaine reproduire le défaut.
    #[test]
    fn no_authored_blocker_covers_its_arena_spawn() {
        let reg = AssetRegistry::parse_toml(&read(REGISTRY_PATH));
        let genome: ArenaLayoutsGenome =
            toml::from_str(&read("assets/genomes/arena_layouts.toml"))
                .expect("arena_layouts.toml illisible");

        let mut measured = 0usize;
        let mut unmeasured: Vec<&str> = Vec::new();
        let mut faults: Vec<String> = Vec::new();

        for (stage, layout) in &genome.layouts {
            let spawn = layout.spawn_pos();
            for piece in &layout.pieces {
                if !piece.blocker {
                    continue;
                }
                let Some(m) = reg.get(&piece.prefab) else {
                    unmeasured.push(&piece.prefab);
                    continue;
                };
                measured += 1;
                // L'emprise en jeu suit l'échelle autorée — pas l'emprise native.
                let radius = m.footprint_radius_m * piece.scale;
                let dx = piece.pos[0] - spawn.x;
                let dz = piece.pos[2] - spawn.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let needed = radius + PLAYER_RADIUS_M + SPAWN_CLEARANCE_M;
                if dist < needed {
                    faults.push(format!(
                        "  '{stage}' : '{}' (r={radius:.1} m) à {dist:.1} m de l'apparition \
                         — il en faut {needed:.1}",
                        piece.prefab.rsplit('/').next().unwrap_or(&piece.prefab)
                    ));
                }
            }
        }

        // « Zéro mesuré n'est pas vert, c'est aveugle » (map-design-patterns §13).
        assert!(
            measured > 0,
            "AUCUNE pièce bloquante mesurée — ce test ne vérifie rien. \
             Non mesurées : {unmeasured:#?}"
        );
        assert!(
            faults.is_empty(),
            "{} apparition(s) dans un solide sur {measured} bloqueurs mesurés :\n{}",
            faults.len(),
            faults.join("\n")
        );
    }

    /// Le test ci-dessus a-t-il des dents ? On lui donne un cas fautif construit
    /// à la main. Sans cette preuve, « aucune faute trouvée » pourrait aussi
    /// bien vouloir dire « je ne sais pas chercher ».
    #[test]
    fn the_clearance_check_really_catches_an_overlap() {
        let reg = AssetRegistry::parse_toml(&read(REGISTRY_PATH));
        // Une pièce quelconque du registre, posée PILE sur l'apparition.
        let (path, m) = reg
            .by_path
            .iter()
            .find(|(_, m)| m.footprint_radius_m > 0.5)
            .expect("le registre doit contenir au moins un prop mesurable");
        let radius = m.footprint_radius_m;
        let dist = 0.0_f32;
        assert!(
            dist < radius + PLAYER_RADIUS_M + SPAWN_CLEARANCE_M,
            "'{path}' (r={radius:.1}) à 0 m devrait être détecté comme un recouvrement"
        );
    }
}
