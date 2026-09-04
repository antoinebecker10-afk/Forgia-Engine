//! # forgia-menu-hub — le hub-menu du Roguelite
//!
//! Tout ce qui faisait du menu-titre un hub de mode : la pile de navigation
//! ([`nav::NavStack`]), le registre de pages ([`registry`]), le chrome, les
//! pages, le diorama d'arène en fond et les aperçus 3D.
//!
//! Extrait de `forgia-ui` par la story-694 incrément 5. La frontière vivait à
//! l'envers : le shell partagé dépendait d'un mode de jeu, si bien que toute
//! crate touchant l'UI tirait les 28k lignes du roguelite. Désormais la flèche
//! pointe dans le bon sens — `forgia-menu-hub` → `forgia-ui`, jamais l'inverse.
//!
//! ## Ce que le shell fournit, et qu'on ne redéclare pas ici
//!
//! La `MenuCamera2d`, l'échelle UI, le fond vidéo, les verbes du curseur, et
//! surtout l'**unique** handler ESC/B. Ce dernier n'appelle pas la pile : il
//! émet `forgia_ui::MenuBackRequested`, que [`shell::sys_apply_menu_back`]
//! consomme. C'est la seule chose que `forgia-ui` sait du hub — un message
//! sans champ.

use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use forgia_core::prelude::*;
use forgia_ui::MenuShellSet;

pub mod arena_backdrop;
pub mod chrome;
pub mod currency_icons;
pub mod gamepad_nav;
pub mod lobby_gate;
pub mod menu_hub_sensor;
pub mod nav;
pub mod pages;
pub mod registry;
pub mod shell;
pub mod slot_glyph;
pub mod weapon_preview;

pub use nav::{MenuPage, NavStack};

pub mod prelude {
    pub use crate::ForgiaMenuHubPlugin;
    pub use crate::{MenuPage, NavStack};
}

use arena_backdrop::ArenaBackdropPlugin;
use currency_icons::CurrencyIconsPlugin;
use lobby_gate::{sys_auto_start_when_warm, sys_lobby_loading_overlay};
use nav::{reset_menu_page, sys_hub_badges, HubBadges};
use pages::armes::sys_menu_armes;
use pages::enclume::sys_menu_enclume;
use pages::forgeron::sys_menu_forgeron;
use pages::livre::sys_menu_livre;
use pages::marketplace::sys_menu_marketplace;
use pages::root::sys_menu_root_dashboard;
use shell::{
    main_menu_ui, sys_apply_menu_back, sys_mark_identity_shown, sys_publish_backdrop_covered,
};
use weapon_preview::WeaponPreviewPlugin;

pub struct ForgiaMenuHubPlugin;

impl Plugin for ForgiaMenuHubPlugin {
    fn build(&self, app: &mut App) {
        // Le hub ne tourne pas sans son shell : caméra, échelle, ESC et fond
        // vidéo en viennent. Idempotent — `forgia-game` peut aussi l'ajouter.
        if !app.is_plugin_added::<forgia_ui::ForgiaUiPlugin>() {
            app.add_plugins(forgia_ui::ForgiaUiPlugin);
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
            // Le shell gèle sa vidéo quand le diorama couvre le fond — il
            // apprend le fait, pas sa cause. PreUpdate < Update = avant le tick.
            .add_systems(PreUpdate, sys_publish_backdrop_covered)
            .init_resource::<menu_hub_sensor::MenuHubSensorState>()
            .add_systems(
                Update,
                menu_hub_sensor::sys_write_menu_hub_sensor.in_set(GameSet::Sensors),
            )
            .add_systems(OnEnter(AppMode::Menu), gamepad_nav::sys_style_gamepad_focus)
            // Le retour de navigation demandé par l'unique handler ESC/B du
            // shell. `.after(Escape)` le rend SAME-FRAME : sans cet ordre, le
            // message serait lu au tour suivant et la remontée traînerait.
            .add_systems(Update, sys_apply_menu_back.after(MenuShellSet::Escape))
            // Phase 4 — pastilles de la sidebar (état réel, marquage « vu »).
            .add_systems(Update, (sys_hub_badges, sys_mark_identity_shown))
            // Retour à la page racine à chaque retour au menu.
            .add_systems(OnEnter(AppMode::Menu), reset_menu_page)
            // Hub-menu : menu principal + sections interactives (Enclume cliquable,
            // Forgeron/identité). Areas indépendantes, chaînées pour un ordre de
            // dessin déterministe — et après la préparation du contexte par le
            // shell (échelle globale, hauteur utile), dont elles dépendent.
            .add_systems(
                EguiPrimaryContextPass,
                (
                    main_menu_ui,
                    sys_menu_root_dashboard,
                    gamepad_nav::sys_menu_gamepad_hints,
                    sys_menu_livre,
                    sys_menu_enclume,
                    sys_menu_forgeron,
                    sys_menu_marketplace,
                    sys_menu_armes,
                )
                    .chain()
                    .after(MenuShellSet::Prepare),
            )
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
        let _p = ForgiaMenuHubPlugin;
    }
}
