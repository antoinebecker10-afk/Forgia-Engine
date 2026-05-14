//! # forgia-terrain
//!
//! Terrain procédural OpenWorld — port verbatim V1 (`D:/Forgia/RUST/Forgia/Forgia/forgia-terrain/`).
//!
//! **DAG-libre** : ne dépend QUE de `forgia-core`.
//! **Désactivé en mode FPS Arena**, **activé en mode RPG OpenWorld**.
//!
//! Patterns clés :
//! - `BiomeGenomeOverrides` : struct bridge data-driven (pas de `Res<GenomeRegistry>`)
//! - Pipeline async via Bevy Tasks (`poll_one_mesh`)
//! - LRU cache 64 entries (`ChunkManager`)
//! - 16 tests headless (story-349 E2)
//!
//! Phase 1 : copy verbatim des 27 fichiers V1.
//! Phase RPG (M2) : ré-activé via `ForgiaTerrainPlugin` dans le binaire.

use bevy::prelude::*;
use forgia_core::prelude::*;

pub mod prelude {
    pub use crate::ForgiaTerrainPlugin;
}

pub struct ForgiaTerrainPlugin;

impl Plugin for ForgiaTerrainPlugin {
    fn build(&self, app: &mut App) {
        // Run only en mode RPG (gate via WorldMode + GameMode).
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
    // Aucun changement de logique — pure copy verbatim V1.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        // Phase 1 : importer les 16 tests V1 (chunk.rs).
        let _p = ForgiaTerrainPlugin;
    }
}
