//! Story-678 Phase 6 — navigation MANETTE du hub-menu.
//!
//! Principe : plutôt que réinventer un système de focus, on TRADUIT la manette
//! en événements clavier egui (D-Pad → flèches, A → Entrée, B → Échap) injectés
//! dans [`EguiInput`] entre `ProcessInput` et `BeginPass` — le focus natif
//! d'egui (fléchage, Tab, activation Entrée) fait tout le travail. LB/RB
//! changent d'onglet directement (ressource [`MenuPage`]). La souris reste
//! pleinement fonctionnelle : la barre de hints ne s'affiche que quand la
//! DERNIÈRE entrée est manette.
//!
//! v1 assumée : D-Pad seulement (le stick exige des timers de répétition —
//! incrément suivant), menu uniquement (la pause viendra avec).

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiInput, PrimaryEguiContext};
use forgia_core::prelude::*;
use forgia_ui_lib::theme::display_text;

use crate::{MenuPage, NavStack};

/// Quelle famille de périphérique a parlé en dernier — pilote l'affichage des
/// hints et (plus tard) le masquage du curseur.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LastInputKind {
    #[default]
    Mouse,
    Gamepad,
}

/// Boutons surveillés pour basculer en mode manette.
///
/// LB/RB (LeftTrigger/RightTrigger) en font partie : ce sont EUX qui changent
/// d'onglet (`sys_gamepad_menu_nav`) — les omettre laissait un joueur naviguer
/// 100 % aux bumpers avec des hints et un capteur `last_input` restés « mouse »
/// (audit 2026-08-07).
const WATCHED: [GamepadButton; 11] = [
    GamepadButton::South,
    GamepadButton::East,
    GamepadButton::North,
    GamepadButton::West,
    GamepadButton::DPadUp,
    GamepadButton::DPadDown,
    GamepadButton::DPadLeft,
    GamepadButton::DPadRight,
    GamepadButton::LeftTrigger,
    GamepadButton::RightTrigger,
    GamepadButton::Start,
];

/// Suit la dernière famille d'entrée active. Tourne partout (pas que au menu) :
/// la bascule doit être déjà juste quand on ARRIVE au menu.
pub fn sys_track_input_kind(
    mut kind: ResMut<LastInputKind>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mouse_btn: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
) {
    let mouse_spoke =
        mouse_motion.read().next().is_some() || mouse_btn.get_just_pressed().next().is_some();
    let pad_spoke = pads
        .iter()
        .any(|p| WATCHED.iter().any(|b| p.just_pressed(*b)));
    // La manette PRIME sur la souris dans la même frame : un pad qui parle est
    // un choix actif, un micro-mouvement de souris est souvent un frôlement.
    let next = if pad_spoke {
        LastInputKind::Gamepad
    } else if mouse_spoke {
        LastInputKind::Mouse
    } else {
        return;
    };
    if *kind != next {
        *kind = next;
    }
}

/// Traduit la manette en événements clavier egui (menu uniquement).
///
/// Hook documenté par bevy_egui : `after(ProcessInput).before(BeginPass)`.
pub fn sys_gamepad_menu_nav(
    app_state: Res<State<AppMode>>,
    pads: Query<&Gamepad>,
    mut contexts: EguiContexts,
    mut inputs: Query<&mut EguiInput, With<PrimaryEguiContext>>,
    mut nav: ResMut<NavStack>,
) {
    if *app_state.get() != AppMode::Menu {
        return;
    }
    let Ok(mut input) = inputs.single_mut() else {
        return;
    };
    // Y a-t-il déjà un widget focus ? Sans focus, la première pression
    // directionnelle doit AMORCER le fléchage (egui ignore les flèches à vide).
    let has_focus = contexts
        .ctx_mut()
        .ok()
        .is_some_and(|ctx| ctx.memory(|m| m.focused().is_some()));

    let mut push_key = |key: egui::Key| {
        input.0.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
    };

    for pad in &pads {
        if pad.just_pressed(GamepadButton::DPadDown) {
            push_key(if has_focus {
                egui::Key::ArrowDown
            } else {
                egui::Key::Tab
            });
        }
        if pad.just_pressed(GamepadButton::DPadUp) {
            push_key(if has_focus {
                egui::Key::ArrowUp
            } else {
                egui::Key::Tab
            });
        }
        if pad.just_pressed(GamepadButton::DPadLeft) && has_focus {
            push_key(egui::Key::ArrowLeft);
        }
        if pad.just_pressed(GamepadButton::DPadRight) && has_focus {
            push_key(egui::Key::ArrowRight);
        }
        if pad.just_pressed(GamepadButton::South) {
            push_key(egui::Key::Enter);
        }
        if pad.just_pressed(GamepadButton::East) {
            push_key(egui::Key::Escape);
        }
        // LB / RB — onglet précédent / suivant, avec le son Tab (front réel :
        // just_pressed, pas de répétition). Un onglet est un FRÈRE : switch_tab
        // (pile `[Root, tab]`), pas push — sinon cycler aux bumpers empilerait
        // tout l'historique d'onglets sous ESC.
        let tabs = MenuPage::NAV;
        let idx = tabs.iter().position(|t| *t == nav.current()).unwrap_or(0);
        let mut moved = None;
        if pad.just_pressed(GamepadButton::LeftTrigger) {
            moved = Some(tabs[(idx + tabs.len() - 1) % tabs.len()]);
        }
        if pad.just_pressed(GamepadButton::RightTrigger) {
            moved = Some(tabs[(idx + 1) % tabs.len()]);
        }
        if let Some(next) = moved {
            if next != nav.current() {
                if let Ok(ctx) = contexts.ctx_mut() {
                    forgia_ui_lib::ui_sfx::push_ui_sfx(ctx, forgia_ui_lib::ui_sfx::UiSfxKind::Tab);
                }
                nav.switch_tab(next);
            }
        }
    }
}

/// Anneau de focus DORÉ — la sélection egui par défaut (bleue) jure avec la DA.
/// Appliqué une fois à l'entrée du menu.
pub fn sys_style_gamepad_focus(mut contexts: EguiContexts) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    ctx.style_mut(|s| {
        s.visuals.selection.stroke = egui::Stroke::new(2.5, egui::Color32::from_rgb(240, 196, 82));
    });
}

/// Barre de hints en pied d'écran — seulement quand la dernière entrée est
/// manette (un joueur souris n'a pas besoin qu'on lui explique sa souris).
pub fn sys_menu_gamepad_hints(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    kind: Res<LastInputKind>,
) {
    if *app_state.get() != AppMode::Menu || *kind != LastInputKind::Gamepad {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    egui::Area::new(egui::Id::new("forgia_gamepad_hints"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -14.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(20, 14, 20, 210))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.label(display_text(
                        "Ⓐ Valider   ·   Ⓑ Annuler   ·   ✚ Naviguer   ·   LB / RB Onglets",
                        15.0,
                        egui::Color32::from_rgb(240, 235, 225),
                    ));
                });
        });
}
