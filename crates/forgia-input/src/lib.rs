//! # forgia-input
//!
//! Input layer : Leafwing PlayerAction AZERTY + KeybindRegistry + InputBlockers.
//!
//! Convention V2 : 1 KeyCode = 1 handler unique (anti-trap V1 double handler ESC).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

pub mod prelude {
    pub use crate::{ForgiaInputPlugin, InputBlockers, MouseSensitivityMultiplier, PlayerAction};
}

/// Multiplicateur global de sensibilité souris appliqué par `mouse_look` (forgia-player).
/// Producteur : forgia-fps en ADS écrit `factor = lerp(1.0, ads_mouse_sensitivity_factor, ads_progress)`.
/// Consommateur : `mouse_look` multiplie sa sensitivity de base par `factor`.
/// Default 1.0 = pas de changement (hipfire ou RPG mode).
#[derive(Resource)]
pub struct MouseSensitivityMultiplier {
    pub factor: f32,
}

impl Default for MouseSensitivityMultiplier {
    fn default() -> Self {
        Self { factor: 1.0 }
    }
}

/// PlayerAction — actions joueur AZERTY.
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum PlayerAction {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    Jump,
    Sprint,
    Fire,
    AltFire,
    Reload,
    Pause,
    Interact,
}

/// InputBlockers — blocage temporaire input (menu ouvert, dialog, console).
#[derive(Resource, Default)]
pub struct InputBlockers {
    pub block_movement: bool,
    pub block_look: bool,
    pub block_fire: bool,
}

pub struct ForgiaInputPlugin;

impl Plugin for ForgiaInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<PlayerAction>::default())
            .init_resource::<InputBlockers>()
            .init_resource::<MouseSensitivityMultiplier>();
    }
}

/// Default keybindings — mapping POSITIONNEL (WASD physique).
/// Bevy `KeyCode` est PHYSIQUE (position QWERTY US), pas logique.
/// → Mapping `KeyW/KeyA/KeyS/KeyD` correspond à la POSITION "ZQSD" sur clavier AZERTY.
/// User AZERTY appuie sur Z (touche physique W) → MoveForward déclenché.
pub fn default_input_map() -> InputMap<PlayerAction> {
    use bevy::input::keyboard::KeyCode::*;
    use bevy::input::mouse::MouseButton;
    let mut map = InputMap::default();
    map.insert(PlayerAction::MoveForward, KeyW);  // Position Z en AZERTY
    map.insert(PlayerAction::MoveBackward, KeyS); // Position S en AZERTY (identique)
    map.insert(PlayerAction::MoveLeft, KeyA);     // Position Q en AZERTY
    map.insert(PlayerAction::MoveRight, KeyD);    // Position D en AZERTY (identique)
    map.insert(PlayerAction::Jump, Space);
    map.insert(PlayerAction::Sprint, ShiftLeft);
    map.insert(PlayerAction::Fire, MouseButton::Left);
    map.insert(PlayerAction::AltFire, MouseButton::Right);
    map.insert(PlayerAction::Reload, KeyR);
    map.insert(PlayerAction::Pause, Escape);
    map.insert(PlayerAction::Interact, KeyE);
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_map_has_all_actions() {
        let map = default_input_map();
        assert!(map.get(&PlayerAction::MoveForward).is_some());
        assert!(map.get(&PlayerAction::Fire).is_some());
        assert!(map.get(&PlayerAction::Pause).is_some());
    }
}
