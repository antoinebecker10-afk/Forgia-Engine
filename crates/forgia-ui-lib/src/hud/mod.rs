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
mod barks;
mod coffre_forgeron;
mod energy;
mod perf_overlay;
mod player_hp;
mod wave_counter;

pub mod prelude {
    pub use super::ForgiaUiHudPlugin;
}

pub struct ForgiaUiHudPlugin;

impl Plugin for ForgiaUiHudPlugin {
    fn build(&self, app: &mut App) {
        // Story-596 Phase A — thème global Forge (fonts + style egui), apply-once.
        // Branché ici car ForgiaUiHudPlugin est le plugin ui-lib wiré par forgia-game.
        if !app.is_plugin_added::<crate::theme::ForgeThemePlugin>() {
            app.add_plugins(crate::theme::ForgeThemePlugin);
        }
        // Story-528 AC4 — energy overlay Roguelite : fade warm < 30% + voiceline
        // épuisement. Le label « ÉNERGIE » + cœurs sont désormais dans la carte
        // vitals (forgia-mode-roguelite), plus dans un overlay séparé (2026-07-22).
        app.add_plugins((
            player_hp::PlayerHpPlugin,
            wave_counter::WaveCounterPlugin,
            energy::EnergyOverlayPlugin,
            // Story-529 Phase 2 — Coffre du Forgeron (3 cartes boons après wave clear).
            // Gated GameMode::Roguelite + CoffreSession::is_open. Sans CoffreSession
            // Resource (ForgiaBoonsPlugin pas wiré), system early-return silencieux
            // après refactor — voir Phase 3.
            coffre_forgeron::CoffreForgeronPlugin,
            // Story-531 AC5-7 incrément kill — barks armes parlantes (bulle
            // persona sur kill, consomme roguelite_dialogue.toml).
            barks::WeaponBarksPlugin,
            // Overlay perf live (FPS + frame time ms) — visible en continu en jeu.
            perf_overlay::PerfOverlayPlugin,
        ));
    }
}
