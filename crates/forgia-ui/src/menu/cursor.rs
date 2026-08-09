//! Curseur souris — capture/libération, blocage look+fire, et les deux
//! réconciliateurs (modal Coffre/portail, retour de focus, gate Lobby).

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use forgia_core::prelude::*;
use forgia_input::prelude::InputBlockers;

/// Le mode de capture du curseur qui MARCHE sur la plateforme courante.
///
/// ## Pourquoi ce n'est pas `Locked` partout (2026-08-04)
///
/// winit **ne supporte pas `Locked` sur Windows** : la demande échoue, et comme
/// rien ne remonte l'erreur côté jeu, la souris sortait simplement de la fenêtre.
/// Rapporté deux fois en playtest — « la souris ne doit jamais pouvoir sortir de
/// l'écran ».
///
/// `Confined` est le mode supporté sur Windows : le curseur est borné au cadre.
/// Le mouse-look n'en souffre pas, il lit le mouvement **brut** du périphérique,
/// qui continue d'arriver même curseur collé à un bord.
///
/// macOS / Wayland / X11 gardent `Locked` (verrouillage au centre), plus précis
/// là où il existe.
#[cfg(target_os = "windows")]
pub(crate) const FPS_GRAB_MODE: CursorGrabMode = CursorGrabMode::Confined;
#[cfg(not(target_os = "windows"))]
pub(crate) const FPS_GRAB_MODE: CursorGrabMode = CursorGrabMode::Locked;

/// Capture le curseur + invisible quand on entre InGame (pour mouse_look).
pub(crate) fn grab_cursor(mut q: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut opts) = q.single_mut() {
        opts.grab_mode = FPS_GRAB_MODE;
        opts.visible = false;
        info!("[forgia-ui] Cursor grabbed (Locked + invisible)");
    } else {
        warn!("[forgia-ui] grab_cursor: PrimaryWindow CursorOptions not found");
    }
}

/// Story-528 follow-up — bloque mouse_look + block fire pendant Roguelite
/// Defeat/Victory pour que la souris puisse cliquer les boutons end-of-run
/// sans pivoter la caméra ni tirer.
pub(crate) fn block_look_on(mut blockers: ResMut<InputBlockers>) {
    blockers.block_look = true;
    blockers.block_fire = true;
    info!("[forgia-ui] InputBlockers: look+fire ON (Roguelite end-of-run)");
}

pub(crate) fn block_look_off(mut blockers: ResMut<InputBlockers>) {
    blockers.block_look = false;
    blockers.block_fire = false;
    info!("[forgia-ui] InputBlockers: look+fire OFF");
}

/// Story-558 Phase 7 follow-up (2026-05-29) — toggle cursor + InputBlockers
/// selon `CoffreSession.is_open`. Quand le Coffre s'ouvre (fin de wave),
/// libère la souris pour cliquer cartes/Skip/Reroll. Quand il se ferme
/// (pick ou skip), re-grab pour reprendre l'aim FPS.
///
/// Tracked via `Local<bool>` (front montant/descendant) — évite spam each
/// frame. Gated AppMode::InGame uniquement (pas perturber Menu/Paused).
pub(crate) fn sys_sync_cursor_with_coffre(
    app_state: Res<State<AppMode>>,
    session: Option<Res<forgia_rpg_data::boons::CoffreSession>>,
    // Story-646 Inc.2 fix caméra (2026-07-02) — le CHOIX DE PORTE est un modal au
    // même titre que le Coffre : curseur libre pendant, re-grab + look/fire rendus
    // au pick. Sans ça : personne ne re-verrouillait après le portail (caméra morte)
    // et ce sync re-grabbait PENDANT le choix (bagarre avec l'override du hud).
    wave: Option<Res<forgia_mode_roguelite::RogueliteWave>>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut blockers: ResMut<InputBlockers>,
    mut was_open: Local<bool>,
) {
    if *app_state.get() != AppMode::InGame {
        return;
    }
    let coffre_open = session.as_ref().is_some_and(|s| s.is_open);
    let portal_open = wave
        .as_ref()
        .is_some_and(|w| !w.portal_choices.is_empty() && w.portal_pick.is_none());
    let is_open = coffre_open || portal_open;
    if is_open == *was_open {
        return;
    }
    *was_open = is_open;
    if let Ok(mut opts) = q_cursor.single_mut() {
        if is_open {
            opts.grab_mode = CursorGrabMode::None;
            opts.visible = true;
            blockers.block_look = true;
            blockers.block_fire = true;
            info!("[forgia-ui] Modal (coffre/portail) OPEN — cursor released, look+fire blocked");
        } else {
            opts.grab_mode = FPS_GRAB_MODE;
            opts.visible = false;
            blockers.block_look = false;
            blockers.block_fire = false;
            info!("[forgia-ui] Modal CLOSED — cursor grabbed, look+fire unblocked");
        }
    }
}

/// Re-grab le curseur au RETOUR DE FOCUS fenêtre (alt-tab). winit relâche le
/// grab `Locked` à la perte de focus, mais `CursorOptions.grab_mode` reste à
/// `Locked` → Bevy ne détecte aucun changement et ne ré-pousse rien à winit → le
/// curseur reste libre au retour (il « sort de l'écran »). On force la ré-
/// application (l'accès `&mut` marque le composant changé) UNIQUEMENT en gameplay
/// actif : `AppMode::InGame` + `!block_look` — ce qui exclut Pause / Coffre /
/// Lobby / écran fin-de-run, où le curseur DOIT rester libre (`block_look` y est
/// déjà à `true`).
pub(crate) fn sys_regrab_cursor_on_focus(
    mut focus: MessageReader<bevy::window::WindowFocused>,
    app_state: Res<State<AppMode>>,
    blockers: Res<InputBlockers>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let regained = focus.read().any(|ev| ev.focused);
    if !regained || *app_state.get() != AppMode::InGame || blockers.block_look {
        return;
    }
    if let Ok(mut opts) = q_cursor.single_mut() {
        opts.grab_mode = FPS_GRAB_MODE;
        opts.visible = false;
        info!("[forgia-ui] focus regagné — curseur re-grabbed (anti alt-tab)");
    }
}

/// Release cursor (visible + free) quand on entre Menu.
pub(crate) fn release_cursor(mut q: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut opts) = q.single_mut() {
        opts.grab_mode = CursorGrabMode::None;
        opts.visible = true;
        info!("[forgia-ui] Cursor released (None + visible)");
    } else {
        warn!("[forgia-ui] release_cursor: PrimaryWindow CursorOptions not found");
    }
}

/// Réconciliateur curseur du Lobby Roguelite — fix « pas de souris au lancement »
/// (design home-hub 2026-06-26, P1). À l'entrée Roguelite, `grab_cursor`
/// (OnEnter InGame) et `release_cursor` (OnEnter RunState::Lobby) tirent la même
/// frame sur deux schedules SANS ordre → le grab pouvait gagner, curseur
/// verrouillé sous le wizard d'arme. Ce système est l'unique source de vérité du
/// curseur AU LOBBY : par-frame, set-if-different (zéro churn), il garantit
/// curseur libre + look/fire bloqués quelle que soit l'ordre des OnEnter ou le
/// timing de 1ʳᵉ activation du SubState. Gaté (InGame + Roguelite + Lobby) au
/// wire-up → no-op partout ailleurs.
pub(crate) fn sys_force_lobby_cursor_free(
    mut q: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut blockers: ResMut<InputBlockers>,
) {
    if let Ok(mut opts) = q.single_mut() {
        if opts.grab_mode != CursorGrabMode::None || !opts.visible {
            opts.grab_mode = CursorGrabMode::None;
            opts.visible = true;
        }
    }
    if !blockers.block_look {
        blockers.block_look = true;
    }
    if !blockers.block_fire {
        blockers.block_fire = true;
    }
}
