//! # forgia-rpg
//!
//! Mode RPG OpenWorld : quêtes, NPCs, dialog, inventaire (LOCK-INV-1 80 slots).
//!
//! **Squelette V1** — dev complet V2.M2 (post-ship Bots Brawl).
//! Phase 0 : juste plugin squelette pour que le menu puisse proposer "RPG" même si vide.

use bevy::prelude::*;
use forgia_core::prelude::*;

pub mod prelude {
    pub use crate::ForgiaRpgPlugin;
}

pub struct ForgiaRpgPlugin;

impl Plugin for ForgiaRpgPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameMode::Rpg), spawn_world)
            .add_systems(OnExit(GameMode::Rpg), cleanup_world);
    }
}

fn spawn_world(mut _commands: Commands) {
    // M2 : spawn terrain procédural + NPCs + quêtes initiales.
    info!("[forgia-rpg] spawn_world — Phase 0 placeholder, full impl M2");
}

fn cleanup_world(mut _commands: Commands) {
    info!("[forgia-rpg] cleanup_world — Phase 0 placeholder");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaRpgPlugin;
    }
}
