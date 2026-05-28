//! F-keys catalog — pattern V1 `KeybindRegistry` adapté.
//!
//! Centralisation des bindings touches debug. Modifiable runtime via
//! `DebugBindings::register()`.

use crate::categories::CategoryId;
use bevy::prelude::*;
use std::collections::HashMap;

/// Action déclenchée par un binding.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum DebugAction {
    /// F3 — toggle master overlay window.
    ToggleMaster,
    /// 1-6 numpad / digit keys — toggle category visibility.
    ToggleCategory(CategoryId),
    /// Backquote (~) / F1 — toggle runtime console (story-548).
    ToggleConsole,
}

/// Resource de binding key → action. Modifiable runtime.
#[derive(Resource, Debug, Clone)]
pub struct DebugBindings {
    bindings: HashMap<KeyCode, DebugAction>,
}

impl Default for DebugBindings {
    fn default() -> Self {
        let mut bindings = HashMap::new();
        // Master toggle.
        bindings.insert(KeyCode::F3, DebugAction::ToggleMaster);
        // Category toggles 1-6 (digit row, pas numpad pour AZERTY compat).
        bindings.insert(KeyCode::Digit1, DebugAction::ToggleCategory(CategoryId::System));
        bindings.insert(KeyCode::Digit2, DebugAction::ToggleCategory(CategoryId::Combat));
        bindings.insert(KeyCode::Digit3, DebugAction::ToggleCategory(CategoryId::Player));
        bindings.insert(KeyCode::Digit4, DebugAction::ToggleCategory(CategoryId::Terrain));
        bindings.insert(KeyCode::Digit5, DebugAction::ToggleCategory(CategoryId::Anim));
        bindings.insert(KeyCode::Digit6, DebugAction::ToggleCategory(CategoryId::Audio));
        bindings.insert(KeyCode::Digit7, DebugAction::ToggleCategory(CategoryId::Physics));
        // Console runtime (story-548).
        bindings.insert(KeyCode::Backquote, DebugAction::ToggleConsole);
        bindings.insert(KeyCode::F1, DebugAction::ToggleConsole);
        Self { bindings }
    }
}

impl DebugBindings {
    pub fn register(&mut self, key: KeyCode, action: DebugAction) {
        self.bindings.insert(key, action);
    }

    pub fn unregister(&mut self, key: &KeyCode) {
        self.bindings.remove(key);
    }

    pub fn action_for(&self, key: KeyCode) -> Option<DebugAction> {
        self.bindings.get(&key).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&KeyCode, &DebugAction)> {
        self.bindings.iter()
    }
}
