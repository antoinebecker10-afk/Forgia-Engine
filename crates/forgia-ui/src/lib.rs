//! # forgia-ui
//!
//! Menu (Start + choix FPS/RPG + Pause + Settings) + HUD partagé.
//!
//! **Anti-traps V1 enforced** :
//! - 1 seul handler ESC
//! - `MenuCamera2d` isolé OnEnter(Menu)/OnExit(Menu)
//! - `Time<Real>` pour sensors UI
//!
//! Crates atomiques wire-up :
//! - `forgia-crosshair` : crosshair + sniper scope overlay
//! - `forgia-hitmarker` : hit confirm visual

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use forgia_core::prelude::*;
// Hub-menu (story-menu-hub) : le menu-titre devient le hub roguelite complet.
// `forgia-ui` → `forgia-mode-roguelite` dep existe (pas de cycle) → on lit les Res
// persistées au Startup (présentes dès le menu) et on réutilise les helpers de
// sections déjà écrits dans `hub.rs` (zéro duplication).
// Re-exports backward compat (déplacés vers crates atomiques 2026-05-16)
pub use forgia_crosshair::CrosshairMode;
pub use forgia_effects::hitmarker::HitmarkerState;

pub mod prelude {
    pub use crate::ForgiaUiPlugin;
    /// Re-export backward compat — préférer `forgia_crosshair::CrosshairMode` direct.
    pub use forgia_crosshair::CrosshairMode;
    /// Re-export backward compat — préférer `forgia_effects::hitmarker::HitmarkerState` direct.
    pub use forgia_effects::hitmarker::HitmarkerState;
}

/// Fond du menu = l'arène du chapitre atteint (diorama RTT). Story-678 Phase 5.
mod arena_backdrop;
/// Icônes des deux monnaies (Âmes / Éclats) — story-678.
mod currency_icons;
/// Silhouettes des cinq types d'équipement, tracées en egui — story-678.
mod slot_glyph;
/// Fond vidéo du menu (frames webp pré-extraites → cache LRU egui). Porté V1.
/// Reste le REPLI quand `ui_backdrop_enabled = 0` ou que le diorama n'a rien posé.
mod gamepad_nav;
mod menu_hub_sensor;
mod menu_video;
use arena_backdrop::ArenaBackdropPlugin;
use currency_icons::CurrencyIconsPlugin;

/// Aperçus 3D (arme + bras) au hub-menu (render-to-texture). Story-menu-hub étape 5b.
mod weapon_preview;
use weapon_preview::WeaponPreviewPlugin;

/// Hub-menu découpé en modules (story-694, incrément 2) — zéro changement de comportement.
mod menu;
use menu::cursor::{
    block_look_off, block_look_on, grab_cursor, release_cursor, sys_force_lobby_cursor_free,
    sys_regrab_cursor_on_focus, sys_sync_cursor_with_coffre,
};
use menu::lobby_gate::{sys_auto_start_when_warm, sys_lobby_loading_overlay};
use menu::nav::{reset_menu_page, sys_hub_badges, HubBadges};
use menu::pages::armes::sys_menu_armes;
use menu::pages::enclume::sys_menu_enclume;
use menu::pages::forgeron::sys_menu_forgeron;
use menu::pages::livre::sys_menu_livre;
use menu::pages::marketplace::sys_menu_marketplace;
use menu::pages::root::sys_menu_root_dashboard;
use menu::shell::{
    escape_handler, main_menu_ui, pause_time, resume_time, spawn_menu_camera_permanent,
    sys_apply_ui_scale, sys_mark_identity_shown, sys_mirror_ui_motion, sys_publish_viewport_h,
};
/// Re-exports internes : `menu_hub_sensor`, `weapon_preview` et les pages
/// lisent la page courante via `crate::NavStack` (`MenuPage` reste exporté
/// pour les comparaisons).
pub(crate) use menu::nav::{MenuPage, NavStack};

pub struct ForgiaUiPlugin;

impl Plugin for ForgiaUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        // Crates atomiques (règle fine-grained-crates) — idempotent.
        if !app.is_plugin_added::<forgia_crosshair::ForgiaCrosshairPlugin>() {
            app.add_plugins(forgia_crosshair::ForgiaCrosshairPlugin);
        }
        if !app.is_plugin_added::<forgia_effects::hitmarker::ForgiaHitmarkerPlugin>() {
            app.add_plugins(forgia_effects::hitmarker::ForgiaHitmarkerPlugin);
        }
        // Aperçu 3D d'arme au hub-menu (RTT) — cycle de vie sur AppMode::Menu.
        if !app.is_plugin_added::<WeaponPreviewPlugin>() {
            app.add_plugins(WeaponPreviewPlugin);
        }
        // Fond d'arène du menu (RTT). APRÈS l'aperçu personnage : sa caméra rend
        // le layer du personnage, qui doit donc exister.
        if !app.is_plugin_added::<ArenaBackdropPlugin>() {
            app.add_plugins(ArenaBackdropPlugin);
        }
        if !app.is_plugin_added::<CurrencyIconsPlugin>() {
            app.add_plugins(CurrencyIconsPlugin);
        }
        // MenuCamera2d permanente : spawn 1 fois Startup, JAMAIS despawn.
        // Ordre explicite high pour render egui par-dessus la Camera3d gameplay.
        // Anti-trap V1 : éviter le frame où aucune caméra n'existe (ESC bug).
        app.init_resource::<NavStack>()
            .init_resource::<HubBadges>()
            .init_resource::<gamepad_nav::LastInputKind>()
            // Story-678 Phase 6 — manette : traduction en événements clavier
            // egui, injectée entre ProcessInput et BeginPass (hook documenté).
            .add_systems(
                PreUpdate,
                gamepad_nav::sys_gamepad_menu_nav
                    .after(bevy_egui::EguiPreUpdateSet::ProcessInput)
                    .before(bevy_egui::EguiPreUpdateSet::BeginPass),
            )
            .add_systems(Update, gamepad_nav::sys_track_input_kind)
            .init_resource::<menu_hub_sensor::MenuHubSensorState>()
            .add_systems(
                Update,
                menu_hub_sensor::sys_write_menu_hub_sensor.in_set(GameSet::Sensors),
            )
            .add_systems(OnEnter(AppMode::Menu), gamepad_nav::sys_style_gamepad_focus)
            .add_systems(Startup, (spawn_menu_camera_permanent, menu_video::setup_menu_video))
            // Fond vidéo menu : tick (avance frame) + sensor (forgia2_menu_video.json).
            .add_systems(
                Update,
                (menu_video::menu_video_tick, menu_video::menu_video_sensor),
            )
            // Story-678 Phase 2 — miroir UserSettings → mémoire egui, chaque
            // frame et sans garde is_changed (leçon « boons inertes ») : les
            // impulsions de boutons vivent aussi in-game (coffre, pause).
            // Phase 4 — pastilles de la sidebar (état réel, marquage « vu »).
            // Story-692 — l'échelle globale tourne PARTOUT (le HUD in-game vit
            // sur la même toile 1080p que le hub) : compare-and-write, coût nul.
            .add_systems(
                Update,
                (
                    sys_mirror_ui_motion,
                    sys_hub_badges,
                    sys_apply_ui_scale,
                    sys_mark_identity_shown,
                ),
            )
            // Menu titre : curseur libre + reset à la page racine à chaque retour menu.
            .add_systems(OnEnter(AppMode::Menu), (release_cursor, reset_menu_page))
            .add_systems(OnEnter(AppMode::InGame), grab_cursor)
            .add_systems(OnEnter(AppMode::Paused), (release_cursor, pause_time))
            .add_systems(OnExit(AppMode::Paused), resume_time)
            // Story-528 follow-up — Roguelite Defeat/Victory : cursor libre pour
            // cliquer "Nouvelle Run" / "Retour Menu" du defeat_overlay. Sans ça,
            // mouse_look continue de pivoter la caméra pendant l'écran fin de run.
            .add_systems(
                OnEnter(forgia_mode_roguelite::RunState::Defeat),
                (release_cursor, block_look_on),
            )
            .add_systems(
                OnEnter(forgia_mode_roguelite::RunState::Victory),
                (release_cursor, block_look_on),
            )
            .add_systems(
                OnExit(forgia_mode_roguelite::RunState::Defeat),
                (grab_cursor, block_look_off),
            )
            .add_systems(
                OnExit(forgia_mode_roguelite::RunState::Victory),
                (grab_cursor, block_look_off),
            )
            // Story-596 Phase B — Lobby (Enclume) : curseur libre pour cliquer
            // cartes d'upgrade + FORGER. Gated Roguelite : RunState est global,
            // au boot/RPG GameMode ≠ Roguelite → no-op (sinon block_look
            // fuiterait dans le RPG).
            .add_systems(
                OnEnter(forgia_mode_roguelite::RunState::Lobby),
                (release_cursor, block_look_on).run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(
                OnExit(forgia_mode_roguelite::RunState::Lobby),
                (grab_cursor, block_look_off).run_if(in_state(GameMode::Roguelite)),
            )
            // Story-558 Phase 7 follow-up (2026-05-29) — sync cursor avec
            // CoffreSession.is_open : pendant le break Coffre, libérer la
            // souris pour cliquer Skip/Reroll/cartes sans pivoter la caméra.
            .add_systems(Update, (sys_sync_cursor_with_coffre, sys_regrab_cursor_on_focus))
            // Fix « pas de souris au lancement » (design: roguelite-home-hub-proposal
            // 2026-06-26, P1) : à l'entrée Roguelite, OnEnter(InGame)→grab_cursor (LOCK)
            // et OnEnter(RunState::Lobby)→release_cursor (FREE) tirent la même frame sur
            // deux schedules SANS ordre → le grab gagnait, curseur verrouillé alors que
            // le wizard d'arme est affiché. Ce réconciliateur par-frame est l'unique
            // source de vérité du curseur au Lobby (set-if-different, zéro churn).
            .add_systems(
                Update,
                sys_force_lobby_cursor_free
                    .run_if(in_state(AppMode::InGame))
                    .run_if(in_state(GameMode::Roguelite))
                    .run_if(in_state(forgia_mode_roguelite::RunState::Lobby)),
            )
            // Story-455 Phase G — paused_overlay_ui retiré (remplacé par forgia-ui-pause-menu
            // cliquable Resume / Settings / Quit). Le handler ESC/Q reste ici (escape_handler).
            // Hub-menu : menu principal + sections interactives (Enclume cliquable,
            // Forgeron/identité). Areas indépendantes, chaînées pour un ordre de
            // dessin déterministe.
            .add_systems(
                EguiPrimaryContextPass,
                (
                    sys_publish_viewport_h,
                    main_menu_ui,
                    sys_menu_root_dashboard,
                    gamepad_nav::sys_menu_gamepad_hints,
                    sys_menu_livre,
                    sys_menu_enclume,
                    sys_menu_forgeron,
                    sys_menu_marketplace,
                    sys_menu_armes,
                )
                    .chain(),
            )
            .add_systems(Update, escape_handler.in_set(GameSet::UI))
            // Étape 6 hub-menu — lancement direct : le Lobby est un GATE DE
            // CHARGEMENT (le hub est au menu). Auto-start dès warmup PBR prêt +
            // overlay de chargement qui couvre l'ancien hub le temps du warmup.
            .add_systems(
                Update,
                sys_auto_start_when_warm
                    // Ordonné AVANT le reader `sys_start_run` (autre crate) : le
                    // `StartRunEvent` écrit est lu la même frame → transition Lobby→
                    // InRun immédiate (lève l'ambiguïté d'ordre intra-frame, qa-lead M1).
                    .before(forgia_mode_roguelite::run::sys_start_run)
                    .run_if(in_state(AppMode::InGame))
                    .run_if(in_state(GameMode::Roguelite))
                    .run_if(in_state(forgia_mode_roguelite::RunState::Lobby)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                sys_lobby_loading_overlay
                    .run_if(in_state(AppMode::InGame))
                    .run_if(in_state(GameMode::Roguelite))
                    .run_if(in_state(forgia_mode_roguelite::RunState::Lobby)),
            );
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaUiPlugin;
    }
}
