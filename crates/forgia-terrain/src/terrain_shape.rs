//! Terrain shape genome — paramètres data-driven du heightmap (octaves Perlin,
//! falloff de bord, max_height), **hot-reloadable**.
//!
//! Fichier : `config/genomes/terrain_shape.toml`. Permet de tuner la forme du
//! relief (rolling vs dramatique, largeur des collines, hauteur) **sans rebuild**
//! (appliqué à l'entrée du monde ; Shift+F12 live = increment 2).
//!
//! Lu par `forgia-rpg::make_terrain_config` → alimente `TerrainConfig.octaves /
//! edge_falloff_m / max_height`, consommés par `heightmap_at` (toutes les
//! surfaces : chunks, LOD2, leveling village, spawn, foliage Y re-samplent).

use serde::Deserialize;
use std::path::Path;

/// Octaves Perlin par défaut — retune 2026-06-06 (rolling/traversable, pentes
/// ~40° sous le KCC 50°). `(freq, amp)`. Base 0.003=1.0 = grande forme du monde.
pub fn default_octaves() -> Vec<(f64, f32, bool)> {
    // (freq, amp, ridged). Tout smooth par défaut (rolling). Passer une octave
    // moyenne à ridged=true via le TOML pour des chaînes de montagnes acérées.
    vec![
        (0.003, 1.0, false),
        (0.008, 0.40, false),
        (0.025, 0.08, false),
        (0.060, 0.025, false),
    ]
}

fn default_schema() -> u32 {
    1
}
fn default_max_height() -> f32 {
    80.0
}
fn default_edge_falloff() -> f32 {
    80.0
}
/// Domain warping (IQ) : amplitude (m) du déplacement des coords d'échantillonnage.
/// 0 = désactivé (Perlin brut, blobs alignés grille). ~40 = terrain naturel/sinueux.
fn default_warp_strength() -> f32 {
    40.0
}
/// Fréquence du bruit de warp (basse → grandes sinuosités, haute → détail tortueux).
fn default_warp_freq() -> f64 {
    0.006
}

fn default_false() -> bool {
    false
}

#[derive(Debug, Clone, Deserialize)]
pub struct OctaveSpec {
    pub freq: f64,
    pub amp: f32,
    /// Ridged noise (`1-|perlin|`) → crêtes acérées (montagnes) au lieu de
    /// collines douces. Défaut false. Mettre `ridged = true` sur une octave
    /// moyenne (ex freq 0.008-0.025) pour des chaînes de montagnes.
    #[serde(default = "default_false")]
    pub ridged: bool,
}

/// Genome de forme du terrain. Tous les champs ont un défaut serde → un TOML
/// partiel (ou absent) tombe sur les valeurs retune par défaut, pas de crash.
#[derive(Debug, Clone, Deserialize)]
pub struct TerrainShapeGenome {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    #[serde(default = "default_max_height")]
    pub max_height: f32,
    #[serde(default = "default_edge_falloff")]
    pub edge_falloff_m: f32,
    #[serde(default = "default_warp_strength")]
    pub warp_strength: f32,
    #[serde(default = "default_warp_freq")]
    pub warp_freq: f64,
    #[serde(default)]
    pub octaves: Vec<OctaveSpec>,
}

impl Default for TerrainShapeGenome {
    fn default() -> Self {
        Self {
            schema_version: 1,
            max_height: default_max_height(),
            edge_falloff_m: default_edge_falloff(),
            warp_strength: default_warp_strength(),
            warp_freq: default_warp_freq(),
            octaves: default_octaves()
                .into_iter()
                .map(|(freq, amp, ridged)| OctaveSpec { freq, amp, ridged })
                .collect(),
        }
    }
}

impl TerrainShapeGenome {
    /// Octaves en `(f64, f32)` pour `TerrainConfig` ; fallback retune si vide.
    pub fn octave_pairs(&self) -> Vec<(f64, f32, bool)> {
        if self.octaves.is_empty() {
            default_octaves()
        } else {
            self.octaves
                .iter()
                .map(|o| (o.freq, o.amp, o.ridged))
                .collect()
        }
    }

    /// Charge le genome depuis le TOML, ou défaut (retune) si absent/invalide.
    /// Robuste : un TOML cassé logge un warn et tombe sur le défaut (pas de crash).
    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        let p = path.as_ref();
        match std::fs::read_to_string(p) {
            Ok(s) => match toml::from_str::<Self>(&s) {
                Ok(g) => {
                    bevy::log::info!(
                        "[terrain-shape] genome chargé : max_height={:.0} edge_falloff={:.0} octaves={}",
                        g.max_height,
                        g.edge_falloff_m,
                        g.octave_pairs().len()
                    );
                    g
                }
                Err(e) => {
                    bevy::log::warn!(
                        "[terrain-shape] parse {} échoué ({e}) — défaut retune",
                        p.display()
                    );
                    Self::default()
                }
            },
            Err(_) => {
                bevy::log::info!(
                    "[terrain-shape] pas de {} — défaut retune (rolling)",
                    p.display()
                );
                Self::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_four_octaves_and_retune_values() {
        let g = TerrainShapeGenome::default();
        assert_eq!(g.max_height, 80.0);
        assert_eq!(g.edge_falloff_m, 80.0);
        let oc = g.octave_pairs();
        assert_eq!(oc.len(), 4);
        assert_eq!(oc[0], (0.003, 1.0, false));
        assert_eq!(oc[1].1, 0.40);
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let g: TerrainShapeGenome = toml::from_str("max_height = 120.0\n").unwrap();
        assert_eq!(g.max_height, 120.0);
        assert_eq!(g.edge_falloff_m, 80.0); // défaut
        assert!(g.octaves.is_empty()); // → octave_pairs() tombe sur défaut
        assert_eq!(g.octave_pairs().len(), 4);
    }

    #[test]
    fn full_toml_overrides_octaves() {
        let toml_str = r#"
max_height = 100.0
edge_falloff_m = 120.0
[[octaves]]
freq = 0.002
amp = 1.0
[[octaves]]
freq = 0.01
amp = 0.3
ridged = true
"#;
        let g: TerrainShapeGenome = toml::from_str(toml_str).unwrap();
        assert_eq!(g.max_height, 100.0);
        let oc = g.octave_pairs();
        assert_eq!(oc.len(), 2);
        assert_eq!(oc[0], (0.002, 1.0, false));
        assert!(oc[1].2, "2e octave doit être ridged=true");
    }

    #[test]
    fn missing_file_returns_default() {
        let g = TerrainShapeGenome::load_or_default("this/does/not/exist.toml");
        assert_eq!(g.max_height, 80.0);
        assert_eq!(g.octave_pairs().len(), 4);
    }
}
