//! # forgia-ui — le SHELL NEUTRE du menu
//!
//! Ce que cette crate possède, et rien d'autre :
//! - `MenuCamera2d` permanente (anti-trap V1 : jamais de frame sans caméra) ;
//! - le curseur — capture, libération, blocage look/fire ;
//! - l'**unique** handler ESC/B (anti-trap V1 « 1 KeyCode = 1 handler ») ;
//! - le fond vidéo du menu ;
//! - le point d'injection : [`MenuBackRequested`].
//!
//! Le hub roguelite (pile de navigation, registre de pages, chrome, diorama,
//! aperçus 3D) vit dans `forgia-menu-hub` depuis la story-694 incrément 5. Un
//! second mode peut donc avoir son propre menu sans toucher à ce shell.
//!
//! **Reste à faire (incrément 5d)** : les deux réconciliateurs curseur
//! `sys_sync_cursor_with_coffre` et `sys_force_lobby_cursor_free` sont des
//! systèmes IN-GAME qui lisent l'état du roguelite. Ils n'ont rien à faire ici
//! et sont la dernière raison pour laquelle cette crate dépend encore de
//! `forgia-mode-roguelite` — donc la dernière marche avant AC5.
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
use bevy_egui::EguiPlugin;
use forgia_core::prelude::*;
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

/// Fond vidéo du menu (frames webp pré-extraites → cache LRU egui). Porté V1.
/// Reste le REPLI quand `ui_backdrop_enabled = 0` ou que le diorama n'a rien posé.
///
/// **Public** depuis l'incrément 5 : c'est la crate qui DESSINE le menu
/// (`forgia-menu-hub`) qui compose le fond, alors que le pipeline vidéo est une
/// responsabilité du shell.
pub mod menu_video;

/// Le shell découpé : curseur + caméra/échelle/ESC.
mod menu;
use menu::cursor::{
    block_look_off, block_look_on, grab_cursor, release_cursor, sys_force_lobby_cursor_free,
    sys_regrab_cursor_on_focus, sys_sync_cursor_with_coffre,
};
use menu::shell::{
    escape_handler, pause_time, resume_time, spawn_menu_camera_permanent, sys_apply_ui_scale,
    sys_mirror_ui_motion, sys_publish_viewport_h,
};

/// Le point d'injection du shell — voir [`menu::shell::MenuBackRequested`].
pub use menu::shell::{MenuBackRequested, MenuCamera2d};

/// « Quelque chose d'opaque couvre le fond du menu. »
///
/// Le pipeline vidéo se GÈLE quand c'est vrai (story-691) : avancer la frame
/// forcerait le décodage de ~24 WebP 1280×720/s pour une image que personne ne
/// voit. Le shell n'a pas à savoir CE QUI couvre — c'est à celui qui couvre de
/// le dire. Avant l'incrément 5, `menu_video_tick` lisait directement le
/// diorama d'arène du hub : le shell dépendait donc du mode de jeu pour une
/// décision qui ne parle que d'opacité.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct MenuBackdropCovered(pub bool);

/// Les points d'ancrage d'ordonnancement du shell.
///
/// L'ordre se déclare TOUJOURS dans ce sens : le shell publie ses ancres, la
/// crate qui dessine le menu s'ordonne `.after(...)`. L'inverse ferait dépendre
/// `forgia-ui` de son consommateur — un cycle, et la fin du shell neutre.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MenuShellSet {
    /// L'unique handler ESC/B (Update). Le consommateur de
    /// [`MenuBackRequested`] s'ordonne après, sinon la remontée coûte une frame.
    Escape,
    /// Prépare le contexte egui du menu — échelle globale et hauteur utile
    /// (`EguiPrimaryContextPass`). Tout dessin de menu vient après.
    Prepare,
}

/// Les verbes du curseur, exposés pour que le propriétaire d'un état de jeu
/// câble lui-même ses transitions (au lieu que le shell connaisse ses états).
pub mod cursor {
    pub use crate::menu::cursor::{
        block_look_off, block_look_on, grab_cursor, release_cursor, FPS_GRAB_MODE,
    };
}

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
        // MenuCamera2d permanente : spawn 1 fois Startup, JAMAIS despawn.
        // Ordre explicite high pour render egui par-dessus la Camera3d gameplay.
        // Anti-trap V1 : éviter le frame où aucune caméra n'existe (ESC bug).
        app.add_message::<MenuBackRequested>()
            .init_resource::<MenuBackdropCovered>()
            .add_systems(
                Startup,
                (spawn_menu_camera_permanent, menu_video::setup_menu_video),
            )
            // Fond vidéo menu : tick (avance frame) + sensor (forgia2_menu_video.json).
            .add_systems(
                Update,
                (menu_video::menu_video_tick, menu_video::menu_video_sensor),
            )
            // Story-678 Phase 2 — miroir UserSettings → mémoire egui, chaque
            // frame et sans garde is_changed (leçon « boons inertes ») : les
            // impulsions de boutons vivent aussi in-game (coffre, pause).
            // Story-692 — l'échelle globale tourne PARTOUT (le HUD in-game vit
            // sur la même toile 1080p que le hub) : compare-and-write, coût nul.
            .add_systems(Update, (sys_mirror_ui_motion, sys_apply_ui_scale))
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                sys_publish_viewport_h.in_set(MenuShellSet::Prepare),
            )
            // Menu titre : curseur libre.
            .add_systems(OnEnter(AppMode::Menu), release_cursor)
            .add_systems(OnEnter(AppMode::InGame), grab_cursor)
            .add_systems(OnEnter(AppMode::Paused), (release_cursor, pause_time))
            .add_systems(OnExit(AppMode::Paused), resume_time)
            // Story-528 follow-up — Roguelite Defeat/Victory : cursor libre pour
            // cliquer "Nouvelle Run" / "Retour Menu" du defeat_overlay. Sans ça,
            // mouse_look continue de pivoter la caméra pendant l'écran fin de run.
            //
            // ⚠️ INCRÉMENT 5d — ces six câblages nomment des états du roguelite
            // depuis le shell neutre. Ils partiront avec les réconciliateurs.
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
            .add_systems(
                Update,
                (sys_sync_cursor_with_coffre, sys_regrab_cursor_on_focus),
            )
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
            .add_systems(
                Update,
                escape_handler
                    .in_set(GameSet::UI)
                    .in_set(MenuShellSet::Escape),
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
