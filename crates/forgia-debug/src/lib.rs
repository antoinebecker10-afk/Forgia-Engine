//! # forgia-debug — Dev-loop debug UX
//!
//! Live debug overlay multi-catégories pour le dev qui joue le build.
//! Distinct de `forgia-qa-*` (automation/CI/replay). Voir story-547.
//!
//! ## Architecture 3 couches
//!
//! ```text
//!                forgia-game (binary)
//!                       ▲
//!       ┌───────────────┼───────────────┐
//!  forgia-debug    forgia-qa-*    runtime crates
//!  (live UX)       (automation)
//!       └─── consomme ──┴── forgia-observability
//! ```
//!
//! ## Pattern catégories (Unreal GameplayDebugger style)
//!
//! - Trait `DebugCategory` : chaque module enregistre ses panels
//! - F3 = master toggle ; touches 1-6 togglent les catégories individuelles
//! - Lecture sensors via re-poll JSON 1Hz (pas de dep crate forgia-observability
//!   → blast radius minimal, build sans dep gameplay)
//!
//! ## Catégories MVP (story-547 phase 2)
//!
//! 1. System — FPS, memory, sensor health, recent alerts
//! 2. Combat — damage_dir, hitscan, screen_flash, bot_ai (story-545 canonique)
//! 3. Player — player_state, lifecycle, hp_diag (story-540)
//! 4. Terrain — chunks, lod, foliage, streaming (story-502)
//! 5. Anim — rex_bones, foot_ik, walk_pose, anim_layer
//! 6. Audio — audio, voicelines, music_state
//!
//! ## Hors scope (follow-ups identifiés)
//!
//! - Console runtime `:set`/`:spawn` (story-548) — pattern V1 console.rs 600 LOC
//! - Entity inspector via bevy-inspector-egui (story-549)
//! - Migration Shift+F12 ad-hoc (story-550)
//! - Invariants checker JSON ring buffer (story-551, pattern V1 invariants.rs 600 LOC)

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use std::collections::HashMap;

pub mod bindings;
pub mod categories;
pub mod console;
pub mod snapshot;

use bindings::{DebugAction, DebugBindings};
use categories::CategoryRegistry;
use console::ConsoleState;

pub mod prelude {
    pub use crate::bindings::{DebugAction, DebugBindings};
    pub use crate::categories::{CategoryId, CategoryRegistry, DebugCategory};
    pub use crate::console::{ConsoleEvent, ConsoleState};
    pub use crate::{DebugOverlayState, ForgiaDebugPlugin};
}

/// État runtime de l'overlay. Master toggle (F3) + visibility per-category.
#[derive(Resource, Debug)]
pub struct DebugOverlayState {
    pub master_visible: bool,
    pub categories_visible: HashMap<categories::CategoryId, bool>,
    pub last_poll_secs: f32,
}

impl Default for DebugOverlayState {
    fn default() -> Self {
        let mut categories_visible = HashMap::new();
        for id in categories::CategoryId::ALL {
            categories_visible.insert(*id, false);
        }
        // Default : category System visible quand overlay s'ouvre.
        categories_visible.insert(categories::CategoryId::System, true);
        Self {
            master_visible: false,
            categories_visible,
            last_poll_secs: 0.0,
        }
    }
}

/// Plugin principal. Idempotent : guard via `is_plugin_added`.
pub struct ForgiaDebugPlugin;

impl Plugin for ForgiaDebugPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        app.init_resource::<DebugBindings>()
            .init_resource::<DebugOverlayState>()
            .init_resource::<CategoryRegistry>()
            .init_resource::<ConsoleState>()
            .init_resource::<snapshot::SensorSnapshot>()
            .add_message::<console::ConsoleEvent>()
            .add_systems(
                Update,
                (
                    handle_bindings_system,
                    poll_sensors_system.after(handle_bindings_system),
                    draw_overlay_system.after(poll_sensors_system),
                    console::console_history_navigation_system.after(handle_bindings_system),
                    console::draw_console_system.after(handle_bindings_system),
                ),
            );
    }
}

fn handle_bindings_system(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<DebugBindings>,
    mut state: ResMut<DebugOverlayState>,
    mut console: ResMut<ConsoleState>,
) {
    for key in keys.get_just_pressed() {
        match bindings.action_for(*key) {
            Some(DebugAction::ToggleMaster) => {
                state.master_visible = !state.master_visible;
            }
            Some(DebugAction::ToggleCategory(cat)) => {
                // Skip Digit1-6 if console is focused (console captures input).
                if console.has_focus() {
                    continue;
                }
                let v = state.categories_visible.entry(cat).or_insert(false);
                *v = !*v;
            }
            Some(DebugAction::ToggleConsole) => {
                console::toggle_console(&mut console);
            }
            None => {}
        }
    }
}

fn poll_sensors_system(
    time: Res<Time>,
    mut state: ResMut<DebugOverlayState>,
    mut snap: ResMut<snapshot::SensorSnapshot>,
) {
    if !state.master_visible {
        return;
    }
    let now = time.elapsed_secs();
    if now - state.last_poll_secs < 1.0 {
        return;
    }
    state.last_poll_secs = now;
    *snap = snapshot::read_all();
}

fn draw_overlay_system(
    mut contexts: EguiContexts,
    state: Res<DebugOverlayState>,
    bindings: Res<DebugBindings>,
    registry: Res<CategoryRegistry>,
    snap: Res<snapshot::SensorSnapshot>,
) {
    if !state.master_visible {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Window::new("Forgia Debug")
        .default_pos([10.0, 10.0])
        .default_width(360.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for cat in categories::CategoryId::ALL {
                    let visible = state.categories_visible.get(cat).copied().unwrap_or(false);
                    let label = if visible {
                        format!("[{}] {}", cat.numpad_digit(), cat.name())
                    } else {
                        format!(" {}  {}", cat.numpad_digit(), cat.name())
                    };
                    ui.label(label);
                }
            });
            ui.separator();

            for cat in categories::CategoryId::ALL {
                if !state.categories_visible.get(cat).copied().unwrap_or(false) {
                    continue;
                }
                ui.collapsing(cat.name(), |ui| {
                    registry.draw_category(*cat, ui, &snap);
                });
            }

            ui.separator();
            ui.collapsing("bindings", |ui| {
                let mut entries: Vec<_> = bindings.iter().collect();
                entries.sort_by_key(|(k, _)| format!("{k:?}"));
                for (k, a) in entries {
                    ui.label(format!("{k:?} → {a:?}"));
                }
            });
        });
}
