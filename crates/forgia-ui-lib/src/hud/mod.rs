//! # forgia-ui-hud
//!
//! HUD gameplay FPS arena : player HP bar, floating bot HP + damage popups,
//! wave counter retro arcade. Style cartoon Fortnite/Overwatch — couleurs
//! saturées, outlines chunky, monospace pour effet arcade rétro.
//!
//! Plug : `app.add_plugins(ForgiaUiHudPlugin)`.

use bevy::prelude::*;

// Story-457 (2026-05-19) : `bot_hp_floaters` retiré au profit du crate dédié
// `forgia-enemy-nameplate` (3D billboard world-space, custom Material possible).
// L'egui screen-space ne pouvait pas occluder derrière les murs ni gérer
// les distances en world units.
mod coffre_forgeron;
mod energy;
mod player_hp;
mod wave_counter;

pub mod prelude {
    pub use super::ForgiaUiHudPlugin;
}

pub struct ForgiaUiHudPlugin;

impl Plugin for ForgiaUiHudPlugin {
    fn build(&self, app: &mut App) {
        // Story-528 AC4 — energy overlay Roguelite (rename HP→Énergie + cœurs cartoon
        // + warm fade < 30% + voiceline épuisement). Overlay non-destructif au-dessus
        // de player_hp (gated GameMode::Roguelite).
        app.add_plugins((
            player_hp::PlayerHpPlugin,
            wave_counter::WaveCounterPlugin,
            energy::EnergyOverlayPlugin,
            // Story-529 Phase 2 — Coffre du Forgeron (3 cartes boons après wave clear).
            // Gated GameMode::Roguelite + CoffreSession::is_open. Sans CoffreSession
            // Resource (ForgiaBoonsPlugin pas wiré), system early-return silencieux
            // après refactor — voir Phase 3.
            coffre_forgeron::CoffreForgeronPlugin,
        ));
    }
}
