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

use bevy::prelude::*;
use forgia_core::prelude::*;

// pub mod biome_registry;
// pub mod biome_spec;
// pub mod biomes;
// pub mod cave_network;
// pub mod chunk;
// pub mod collision;
// pub mod config_dir;
// pub mod generation;
// pub mod map_gen_config;
// pub mod paths;
// pub mod pipeline_diag;
// pub mod sampling;
// pub mod village_data;
// pub mod worldmap;

pub mod prelude {
    pub use crate::ForgiaTerrainPlugin;
}

pub struct ForgiaTerrainPlugin;

impl Plugin for ForgiaTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            terrain_tick
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Rpg)),
        );
    }
}

fn terrain_tick() {
    // Phase 1 : importer ChunkManager + BiomeMap + streaming systèmes V1.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaTerrainPlugin;
    }
}
