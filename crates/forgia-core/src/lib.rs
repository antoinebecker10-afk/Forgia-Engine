//! # forgia-core
//!
//! Types core de Forgia V2 : States (AppMode, GameMode, WorldMode), GameSet ordering,
//! Resources globales (RespawnPoint, ActiveMap).
//!
//! **Règle inviolable** : ce crate ne dépend de RIEN dans le workspace.
//! Pattern DAG-libre hérité de forgia-terrain V1.

use bevy::prelude::*;

pub mod prelude {
    pub use crate::cosmetics::{ArmCosmetics, ArmStyle, ViewmodelForcedVisible};
    pub use crate::fps_feel::FpsFeelMetrics;
    pub use crate::hud_visibility::{gameplay_hud_visible, GameplayHudVisible};
    pub use crate::states::{AppMode, GameMode, WorldMode};
    pub use crate::system_set::GameSet;
    pub use crate::ForgiaCorePlugin;
}

pub mod hud_visibility {
    use bevy::prelude::*;

    /// Visibilité du HUD de GAMEPLAY (ammo, PV, énergie, confiance, viewmodel…).
    /// `false` = écran-menu in-game (ex. Lobby Roguelite) → masquer tout le HUD de
    /// combat pour ne garder que l'UI de menu. `true` partout ailleurs.
    ///
    /// Vit dans forgia-core (DAG-libre) pour que les crates HUD partagées
    /// (forgia-ui-lib, forgia-viewmodel) la lisent SANS dépendre du crate de mode
    /// (forgia-mode-roguelite) qui la pilote — évite un cycle de dépendances.
    #[derive(Resource, Debug, Clone, Copy)]
    pub struct GameplayHudVisible(pub bool);

    impl Default for GameplayHudVisible {
        fn default() -> Self {
            Self(true)
        }
    }

    /// Run-condition : le HUD de gameplay doit-il s'afficher ? (défaut `true` si la
    /// resource n'est pas encore initialisée — ordre d'init safe).
    pub fn gameplay_hud_visible(v: Option<Res<GameplayHudVisible>>) -> bool {
        v.map(|r| r.0).unwrap_or(true)
    }
}

pub mod cosmetics {
    use bevy::prelude::*;

    /// Style esthétique des bras procéduraux (variation de matériau). Choisi dans
    /// l'onglet Forge. `key()`/`from_key()` = (dé)sérialisation TOML/save.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum ArmStyle {
        /// Mat doux (peau).
        #[default]
        Peau,
        /// Métal (gantelet de forge).
        Gantelet,
        /// Émissif (gant cyber).
        Cyber,
    }

    impl ArmStyle {
        pub fn from_key(s: &str) -> Self {
            match s {
                "gantelet" => ArmStyle::Gantelet,
                "cyber" => ArmStyle::Cyber,
                _ => ArmStyle::Peau,
            }
        }
        pub fn key(self) -> &'static str {
            match self {
                ArmStyle::Peau => "peau",
                ArmStyle::Gantelet => "gantelet",
                ArmStyle::Cyber => "cyber",
            }
        }
        pub fn label(self) -> &'static str {
            match self {
                ArmStyle::Peau => "Peau",
                ArmStyle::Gantelet => "Gantelet",
                ArmStyle::Cyber => "Cyber",
            }
        }
    }

    /// Cosmétique des bras procéduraux (couleur + style). Pilotée par l'onglet Forge
    /// (forgia-mode-roguelite), appliquée au matériau par forgia-viewmodel — via
    /// forgia-core (DAG-libre) pour éviter un cycle de dépendances.
    #[derive(Resource, Debug, Clone, Copy, PartialEq)]
    pub struct ArmCosmetics {
        /// Teinte (sRGB) appliquée à la peau/aux gants.
        pub color: [f32; 3],
        pub style: ArmStyle,
    }
    impl Default for ArmCosmetics {
        fn default() -> Self {
            Self {
                color: [0.93, 0.73, 0.57],
                style: ArmStyle::Peau,
            }
        }
    }

    /// Force l'affichage du viewmodel (bras) hors gameplay — ex. APERÇU dans l'onglet
    /// Forge du hub. `false` = le viewmodel suit `GameplayHudVisible` normalement.
    #[derive(Resource, Debug, Clone, Copy, Default)]
    pub struct ViewmodelForcedVisible(pub bool);
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
        /// Démo perf moteur (2026-06-15) — charge un GLB lourd (cyberpunk city)
        /// avec flycam libre pour stress-tester rendu/VRAM. Pas de gameplay.
        /// Géré par `forgia_game::cyber_city::CyberCityDemoPlugin`.
        CyberCity,
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
            .init_resource::<hud_visibility::GameplayHudVisible>()
            .init_resource::<cosmetics::ArmCosmetics>()
            .init_resource::<cosmetics::ViewmodelForcedVisible>()
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
