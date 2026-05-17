//! # forgia-terrain
//!
//! Terrain procédural OpenWorld — port verbatim V1 (en cours).
//!
//! ## État du port
//!
//! Fichiers portés sur disque (mais pas encore dans le compile graph) :
//! - `biome_registry.rs`, `biome_spec.rs`, `biomes.rs`
//! - `chunk.rs`, `collision.rs`, `config_dir.rs`
//! - `generation.rs` + sous-dossier `generation/` (8 fichiers, ~110KB)
//! - `map_gen_config.rs`, `sampling.rs`
//! - stubs : `worldmap.rs`, `village_data.rs`, `paths.rs`,
//!   `pipeline_diag.rs`, `cave_network.rs`
//!
//! Fichiers MANQUANTS pour activer le compile graph :
//! - `meshing.rs` (43K, surface-nets)
//! - `terrain_material.rs` (17K, shader)
//! - `grass_material.rs` (9K)
//! - `modular.rs` (24K)
//!
//! ## Activation
//!
//! Décommenter les `pub mod` ci-dessous quand les fichiers manquants sont
//! portés. En attendant, ce plugin est un no-op et forgia-rpg fournit un
//! heightmap inline (voir crates/forgia-rpg/src/lib.rs).

// Sous-modules dormants tant que `streaming` complet (W2+) ne consomme pas la
// pipeline génération full V1. Ils compilent mais leurs symboles ne sont pas
// référencés sur le chemin heightmap-grid actuel (W1).
#![allow(dead_code)]

use bevy::prelude::*;
use forgia_core::prelude::*;

pub use chunk::{ChunkCoord, ChunkManager, TerrainConfig, CHUNK_X, CHUNK_Z};
pub use biomes::{BiomeMap, BiomeType};
pub use map_gen_config::{BiomeMode, MapGenConfig, preset_island, preset_forgia_showcase};
pub use generation::heightmap_at;
pub use meshing_heightmap::{build_chunk_mesh, spawn_chunk_entity, ChunkMeshData, VERTS_PER_AXIS};
pub use terrain_material::{init_terrain_material, TerrainSharedMaterial};
pub use lod::{
    ChunkLod, Lod2TileManager, LodSampleOffset, LodStats, LOD0_MAX_M, LOD1_MAX_M, LOD2_MAX_M,
};
pub use paths::{build_path_network, build_path_segment, PathNetwork, PathPolyline, PathSample, RoadTier};

pub mod biome_registry;
pub mod biome_spec;
pub mod biomes;
pub mod cave_network;
pub mod chunk;
pub mod collision;
pub mod config_dir;
pub mod generation;
pub mod map_gen_config;
pub mod paths;
pub mod pipeline_diag;
pub mod sampling;
pub mod village_data;
pub mod worldmap;

// W1 — heightmap-grid mesher (industry-standard RPG) + minimal PBR material.
pub mod meshing_heightmap;
pub mod terrain_material;
// W5 — LOD 3-niveaux GTA5 style (chunks + mega-tiles).
pub mod lod;

pub mod prelude {
    pub use crate::{
        BiomeMap, BiomeType, BiomeMode, ChunkCoord, ChunkManager, ForgiaTerrainPlugin,
        MapGenConfig, TerrainConfig, TerrainSharedMaterial, build_chunk_mesh,
        spawn_chunk_entity,
    };
}

pub struct ForgiaTerrainPlugin;

impl Plugin for ForgiaTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<lod::LodStats>()
            .init_resource::<lod::Lod2TileManager>()
            .add_systems(Startup, terrain_material::init_terrain_material)
            .add_systems(
                Update,
                (
                    lod::update_chunk_lod,
                    lod::build_lod2_tiles_system,
                    lod::export_lod_sensor_system,
                )
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Rpg)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaTerrainPlugin;
    }
}
