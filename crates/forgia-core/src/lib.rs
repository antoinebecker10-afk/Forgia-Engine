//! # forgia-core
//!
//! Types core de Forgia V2 : States (AppMode, GameMode, WorldMode), GameSet ordering,
//! Resources globales (RespawnPoint, ActiveMap).
//!
//! **Règle inviolable** : ce crate ne dépend de RIEN dans le workspace.
//! Pattern DAG-libre hérité de forgia-terrain V1.

use bevy::prelude::*;

pub mod prelude {
    pub use crate::fps_feel::FpsFeelMetrics;
    pub use crate::states::{AppMode, GameMode, WorldMode};
    pub use crate::system_set::GameSet;
    pub use crate::ForgiaCorePlugin;
}

pub mod fps_feel {
    use bevy::prelude::*;

    /// Resource counters FPS feel — Story-528 phase 1.
    /// Producteurs : forgia-player (dash), forgia-effects (hit feedbacks),
    /// forgia-fps (aim assist). Lecteur : forgia-observability fps_feel_sensor.
    /// Placée ici (foundation DAG-libre) pour éviter cycle observability ↔ player.
    #[derive(Resource, Default)]
    pub struct FpsFeelMetrics {
        pub dash_uses_total: u64,
        pub hit_feedbacks_total: u64,
        pub aim_assist_engagements_total: u64,
    }
}

pub mod states {
    use bevy::prelude::*;

    /// AppMode — gate l'UI et le flow joueur.
    #[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
    pub enum AppMode {
        #[default]
        Boot,
        Menu,
        InGame,
        Paused,
    }

    /// GameMode — joueur a choisi FPS ou RPG depuis le menu.
    /// Gate les plugins forgia-fps vs forgia-rpg dynamiquement.
    #[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
    pub enum GameMode {
        #[default]
        None,
        Fps,
        Rpg,
        // Story-470 V7 M1 — 3e jeu Forgia : roguelite FPS coop 1-3j (cible Next Fest)
        Roguelite,
    }

    /// WorldMode — gate la simulation (Editor désactive AI/physics).
    /// Pattern Mass Entity dual phase manager (Epic).
    #[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
    pub enum WorldMode {
        #[default]
        Game,
        Editor,
        Test,
    }
}

pub mod system_set {
    use bevy::prelude::*;

    /// GameSet — chaîne ordering canonique Forgia V2.
    /// Hérité V1 chaîne 7 étapes + ajouts `Network` (entre Input et Movement)
    /// et `Sensors` (entre Effects et UI). Lock L7 reborn.
    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    pub enum GameSet {
        Network,
        Input,
        Movement,
        Physics,
        Camera,
        Combat,
        Effects,
        Sensors,
        UI,
    }
}

/// Plugin agrégateur — init States + configure GameSet ordering.
pub struct ForgiaCorePlugin;

impl Plugin for ForgiaCorePlugin {
    fn build(&self, app: &mut App) {
        use system_set::GameSet;
        // ⚠️ ForgiaCorePlugin DOIT être ajouté APRÈS DefaultPlugins (Bevy 0.18 quirk).
        // DefaultPlugins inclut StatesPlugin qui fournit StateTransition schedule.
        // init_state panique si StateTransition pas encore enregistré.
        // Voir crates/forgia-game/src/main.rs ordre plugins.
        app.init_state::<states::AppMode>()
            .init_state::<states::GameMode>()
            .init_state::<states::WorldMode>()
            .init_resource::<fps_feel::FpsFeelMetrics>()
            .configure_sets(
                Update,
                (
                    GameSet::Network,
                    GameSet::Input,
                    GameSet::Movement,
                    GameSet::Physics,
                    GameSet::Camera,
                    GameSet::Combat,
                    GameSet::Effects,
                    GameSet::Sensors,
                    GameSet::UI,
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_default_correct() {
        assert_eq!(states::AppMode::default(), states::AppMode::Boot);
        assert_eq!(states::GameMode::default(), states::GameMode::None);
        assert_eq!(states::WorldMode::default(), states::WorldMode::Game);
    }

    #[test]
    fn plugin_builds_without_panic() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.add_plugins(ForgiaCorePlugin);
    }
}
