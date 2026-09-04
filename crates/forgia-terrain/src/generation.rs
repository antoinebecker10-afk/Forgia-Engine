//! Terrain generation pipeline — module file orchestrateur.
//!
//! Decompose en 7 sous-modules dans `generation/` (story-334 M5 W4) :
//!
//! | Sous-module | Couche | Contenu principal |
//! |---|---|---|
//! | [`noise`] | 1 / 1b | domain warping + 5 primitives noise (ridged/billow/cellular/swiss/FBm) + per-biome layer recipes |
//! | [`redistribution`] | 2 | s-curve + redistribute par biome |
//! | [`erosion`] | 3 | valley_carve + erode_heightmap_variable + slope_limit + padding ring save/restore |
//! | [`droplet`] | 3 | droplet erosion Beyer/Lague + thermal erosion talus |
//! | [`heightmap`] | 4 + features | heightmap_at/_gen/_ext/_fast + procedural_sdf_at + micro_roughness + feature_height |
//! | [`caves`] | — | 3D Perlin threshold + CaveWormParams + worm network + Skyrim-style village caves + `carve_sphere` primitive |
//! | [`chunk_sdf`] | — | pipeline orchestrator `generate_chunk_lod` (GenDetail LOD 3-tier) |
//!
//! Ce fichier garde les types communs partages entre sous-modules
//! ([`BiomeNoiseLayers`], [`BiomeGenomeOverrides`], [`CastleFootprint`]) +
//! les re-exports publics pour les callers externes (forgia-game, siblings
//! forgia-terrain).

use bevy::prelude::*;

mod caves;
mod droplet;
mod erosion;
mod heightmap;
mod noise;
mod redistribution;
// W1 — chunk_sdf désactivé : pipeline voxel/SDF non utilisé en heightmap-grid path.
// Réactiver quand pipeline_diag.rs sera porté V1 (story-future).
// mod chunk_sdf;
mod island_mask;

// Re-export API publique du split (callers externes : forgia-game + siblings).
#[allow(unused_imports)]
pub(crate) use caves::carve_sphere; // used by cave_network.rs full V1 port (dormant W1)
pub use caves::{carve_cave_worms, carve_village_caves, CaveWormParams};
pub use droplet::HydroErosionParams;
pub use heightmap::{
    heightmap_at, heightmap_at_gen, heightmap_at_gen_ext, heightmap_at_gen_ext_fast,
    procedural_sdf_at,
};
pub use island_mask::{island_mask_at, IslandMaskParams};
// pub use chunk_sdf::{generate_chunk, generate_chunk_lod, generate_initial_chunks, GenDetail};

// ─────────────────────────── Noise Layer System ───────────────────────────

/// Per-biome noise layer recipe — controls which noise types are blended.
/// Weights don't need to sum to 1 — they're normalized at blend time.
/// FBm weight = 1 - ridged - billow (implicit, always present as base).
#[derive(Clone, Debug)]
pub struct BiomeNoiseLayers {
    /// Ridged multifractal weight (sharp peaks, ridges). 0 = off.
    pub ridged_weight: f32,
    /// Billow noise weight (rounded dunes, soft hills). 0 = off.
    pub billow_weight: f32,
    /// Cellular/Worley noise weight (rocky formations, lava flows). 0 = off.
    pub worley_weight: f32,
    /// Frequency multiplier for ridged layer (relative to base_frequency).
    pub ridged_freq_mult: f32,
    /// Frequency multiplier for Worley layer.
    pub worley_freq_mult: f32,
    /// Slope-dependent amplitude: steep=more detail, flat=smooth. 0=off, 1=full.
    pub slope_amp_factor: f32,
    /// Swiss turbulence weight (glacier-eroded crests via derivative feedback). 0 = off.
    pub swiss_weight: f32,
    /// Swiss warp factor — derivative feedback intensity (0.3=subtle, 1.5=extreme). Default: 0.8.
    pub swiss_warp: f32,
    /// Lacunarity override (default 2.5).
    pub lacunarity: f32,
    /// Persistence override (default 0.45).
    pub persistence: f32,
}

impl Default for BiomeNoiseLayers {
    fn default() -> Self {
        Self {
            ridged_weight: 0.0,
            billow_weight: 0.0,
            worley_weight: 0.0,
            ridged_freq_mult: 1.0,
            worley_freq_mult: 1.0,
            slope_amp_factor: 0.0,
            swiss_weight: 0.0,
            swiss_warp: 0.8,
            lacunarity: 2.5,
            persistence: 0.45,
        }
    }
}

// ─────────────────────────── Biome Genome Overrides ───────────────────────────

/// Per-biome generation parameters from genome TOMLs.
/// Built by forgia-game from GenomeRegistry, passed to terrain generation.
/// All fields are Optional — None means use hardcoded defaults.
#[derive(Clone, Debug, Default)]
pub struct BiomeGenomeOverrides {
    /// Per biome (index = BiomeType as u8): (erosion_passes, erosion_rate)
    pub erosion: [Option<(usize, f32)>; 10],
    /// Per biome: micro_roughness amplitude multiplier
    pub micro_roughness_amp: [Option<f32>; 10],
    /// Per biome: warp_strength override
    pub warp_strength: [Option<f32>; 10],
    /// Per biome: enemy (hp_mult, speed_mult, dmg_mult)
    pub enemy_mults: [Option<(f32, f32, f32)>; 10],
    /// Per biome: noise layer recipe (ridged/billow/worley weights, slope-amp, etc.)
    pub noise_layers: [Option<BiomeNoiseLayers>; 10],
    /// Per biome: final height multiplier (applied after FBM + redistribution, before features).
    pub height_mult: [Option<f32>; 10],
    /// Hydraulic erosion parameters (global, from erosion_advanced.toml)
    pub hydro_erosion: Option<HydroErosionParams>,
    /// Thermal erosion talus angle override
    pub thermal_talus_angle: Option<f32>,
    /// Droplet count scale (multiplier on default per-chunk count)
    pub hydro_droplet_scale: Option<f32>,
    /// Per-biome cave probability overrides (from caves_default.toml)
    pub cave_probabilities: [Option<f32>; 10],
    /// Per-biome slope-limiting max (m/m) override.
    pub slope_max: [Option<f32>; 10],
    /// Per-biome thermal erosion pass count override.
    pub thermal_passes: [Option<u32>; 10],
    /// Story 350 V2 — paramètres de la silhouette d'île procgen.
    pub island_mask: Option<IslandMaskParams>,
}

// ─────────────────────────── Castle Footprint ───────────────────────────

/// Flat zone for procedural castle placement.
#[derive(Clone, Debug, Resource)]
pub struct CastleFootprint {
    pub center: Vec2,
    pub radius: f32,
    pub target_height: f32,
}

const CASTLE_FLAT_FRACTION: f32 = 0.70;

impl CastleFootprint {
    /// Returns normalized distance (0 = center, 1 = edge) if (x,z) is within the footprint.
    pub fn influence(&self, x: f32, z: f32) -> Option<f32> {
        let dist = Vec2::new(x, z).distance(self.center);
        let outer = self.radius * 1.2;
        if dist < outer {
            Some(dist / self.radius.max(0.01))
        } else {
            None
        }
    }

    pub fn flattened_height_at(&self, _x: f32, _z: f32) -> f32 {
        self.target_height
    }

    /// Flattening blend factor: 1.0 inside flat zone, smoothstep falloff outside.
    pub fn flatten_blend(norm_dist: f32) -> f32 {
        if norm_dist <= CASTLE_FLAT_FRACTION {
            1.0
        } else {
            let t =
                ((norm_dist - CASTLE_FLAT_FRACTION) / (1.0 - CASTLE_FLAT_FRACTION)).clamp(0.0, 1.0);
            let s = 1.0 - t;
            s * s * (3.0 - 2.0 * s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp() -> CastleFootprint {
        CastleFootprint {
            center: Vec2::ZERO,
            radius: 100.0,
            target_height: 42.0,
        }
    }

    #[test]
    fn influence_at_center_is_zero() {
        let inf = fp().influence(0.0, 0.0).expect("center is inside");
        assert!(inf.abs() < 1e-6, "expected ~0 at center, got {inf}");
    }

    #[test]
    fn influence_outside_outer_ring_is_none() {
        assert!(fp().influence(130.0, 0.0).is_none());
    }

    #[test]
    fn influence_inside_outer_ring_is_some_above_one() {
        let inf = fp().influence(110.0, 0.0).expect("inside outer ring");
        assert!(
            inf > 1.0 && inf < 1.2,
            "expected 1.0 < inf < 1.2 at dist 110/r100, got {inf}"
        );
    }

    #[test]
    fn flattened_height_returns_target_inside() {
        assert_eq!(fp().flattened_height_at(10.0, 20.0), 42.0);
    }

    #[test]
    fn blend_is_one_inside_flat_fraction() {
        assert_eq!(CastleFootprint::flatten_blend(0.0), 1.0);
        assert_eq!(CastleFootprint::flatten_blend(CASTLE_FLAT_FRACTION), 1.0);
    }

    #[test]
    fn blend_monotone_decreasing_outside_flat() {
        let a = CastleFootprint::flatten_blend(0.75);
        let b = CastleFootprint::flatten_blend(0.90);
        let c = CastleFootprint::flatten_blend(1.00);
        assert!(a > b, "expected decreasing: a={a} b={b}");
        assert!(b > c, "expected decreasing: b={b} c={c}");
        assert!(
            (c - 0.0).abs() < 1e-4,
            "blend at norm_dist=1.0 should be ~0, got {c}"
        );
    }

    #[test]
    fn default_noise_layers_are_inert() {
        let d = BiomeNoiseLayers::default();
        assert_eq!(d.ridged_weight, 0.0);
        assert_eq!(d.billow_weight, 0.0);
        assert_eq!(d.worley_weight, 0.0);
        assert_eq!(d.swiss_weight, 0.0);
        assert_eq!(d.lacunarity, 2.5);
        assert_eq!(d.persistence, 0.45);
    }

    #[test]
    fn biome_genome_overrides_default_all_none() {
        let o = BiomeGenomeOverrides::default();
        assert!(o.erosion.iter().all(|e| e.is_none()));
        assert!(o.noise_layers.iter().all(|n| n.is_none()));
        assert!(o.height_mult.iter().all(|h| h.is_none()));
        assert!(o.hydro_erosion.is_none());
        assert!(o.thermal_talus_angle.is_none());
    }
}
