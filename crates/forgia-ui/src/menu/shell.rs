//! Le SHELL NEUTRE du menu : caméra 2D permanente, échelle UI globale (toile
//! 1080p, story-692), le handler ESC unique et la pause du temps virtuel.
//!
//! Ce module ne connaît AUCUN mode de jeu. Le dessin du hub (chrome, pages,
//! diorama) vit dans `forgia-menu-hub` depuis la story-694 incrément 5 ; le
//! seul lien est [`MenuBackRequested`], émis ici et consommé là-bas.

use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_egui::{egui, EguiContexts};
use forgia_core::prelude::*;

use crate::menu::cursor::FPS_GRAB_MODE;

#[derive(Component)]
pub struct MenuCamera2d;

/// MenuCamera2d permanente — spawn une fois au Startup, JAMAIS despawn.
pub(crate) fn spawn_menu_camera_permanent(
    mut commands: Commands,
    q: Query<Entity, With<MenuCamera2d>>,
) {
    if q.is_empty() {
        commands.spawn((
            Camera2d,
            Camera {
                order: 10,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            MenuCamera2d,
            Name::new("MenuCamera2d (permanent)"),
        ));
        info!("[forgia-ui] MenuCamera2d spawned (permanent, order=10, clear=None)");
    }
}

/// Story-678 Phase 2 — pousse le toggle motion persisté (UserSettings) vers la
/// mémoire egui où vivent les helpers (`forgia_ui_lib::motion`). Un insert de
/// bool par frame : idempotent, trop bon marché pour mériter une garde.
pub(crate) fn sys_mirror_ui_motion(
    mut contexts: EguiContexts,
    settings: Option<Res<forgia_ui_lib::pause_menu::UserSettings>>,
) {
    let Some(settings) = settings else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    forgia_ui_lib::motion::set_motion_enabled(ctx, settings.ui_motion_enabled);
}

/// Hauteur de référence du design : toute l'UI egui du jeu est dessinée comme
/// si la fenêtre faisait 1080 points de haut (story-692). Les px absolus des
/// layouts existants sont donc des POINTS de cette toile, plus des pixels.
const UI_REFERENCE_HEIGHT: f32 = 1080.0;
/// Bornes du facteur d'échelle — sous 0.65 le texte passe sous les planchers
/// de lisibilité (XAG : 18 px @1080p), au-delà de 1.6 la nav ne tient plus.
const UI_SCALE_MIN: f32 = 0.65;
const UI_SCALE_MAX: f32 = 1.6;

/// Le facteur d'échelle pour une hauteur de fenêtre donnée — SOURCE UNIQUE :
/// `sys_apply_ui_scale` l'applique, `sys_publish_viewport_h` le déduit. Écrit
/// deux fois, les plafonds de scroll et l'échelle réelle divergeraient.
fn ui_scale_for(window_h: f32) -> f32 {
    (window_h / UI_REFERENCE_HEIGHT).clamp(UI_SCALE_MIN, UI_SCALE_MAX)
}

/// Applique l'échelle globale au contexte egui (story-692).
///
/// AVANT : le hub était une maquette 1080p en px absolus — au preset 1280×720
/// offert dans les Options, la nav (~1330 px intrinsèques) débordait de l'écran
/// et recouvrait les deux chips ; à Windows 125 % pareil. Plutôt que de réécrire
/// ~148 littéraux, UN multiplicateur (`EguiContextSettings.scale_factor`
/// compose avec le scale OS) ramène toute fenêtre à la toile 1080p. Vaut aussi
/// pour le HUD in-game : même contexte, même toile.
pub(crate) fn sys_apply_ui_scale(
    q_win: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut q_egui: Query<&mut bevy_egui::EguiContextSettings>,
) {
    let Ok(win) = q_win.single() else { return };
    let scale = ui_scale_for(win.height());
    for mut s in &mut q_egui {
        if (s.scale_factor - scale).abs() > 1e-4 {
            s.scale_factor = scale;
        }
    }
}

/// Hauteur utile en POINTS egui, publiée dans la mémoire egui.
///
/// La hauteur vient de la SOURCE (`Window` côté Bevy) plutôt que d'une API de
/// contexte egui, dont la valeur dépend des panels déjà posés cette frame —
/// puis se convertit en points via `ui_scale_for` (story-692) : depuis que
/// l'échelle globale existe, les consommateurs (`hub_section_panel`) raisonnent
/// en points de la toile 1080, pas en pixels logiques de la fenêtre.
///
/// ⚠️ Ce n'est PAS ce qui coupait les pages à mi-écran — cf. `set_max_height`
/// dans `hub_section_panel`. Trois correctifs successifs ont visé cette mesure
/// alors que le limiteur était ailleurs ; c'est une sonde, pas un raisonnement,
/// qui l'a montré.
pub(crate) fn sys_publish_viewport_h(
    mut contexts: EguiContexts,
    q_win: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut last: Local<f32>,
) {
    let Ok(win) = q_win.single() else { return };
    let h = win.height() / ui_scale_for(win.height());
    let Ok(ctx) = contexts.ctx_mut() else { return };
    if (h - *last).abs() > 1.0 {
        info!("[hub] hauteur utile = {h:.0} points");
        *last = h;
    }
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("forgia_viewport_h"), h));
}

/// « Le menu doit remonter d'un niveau. »
///
/// C'est LE point d'injection du shell (story-694 incrément 5). Le shell
/// possède la touche — anti-trap V1 « 1 KeyCode = 1 handler » — mais pas la
/// pile : celle-ci vit dans la crate du hub, qui consomme ce message. Un
/// second mode peut donc réutiliser ESC sans redéclarer de handler, et sans
/// que `forgia-ui` ait à connaître ses pages.
///
/// Le garde « egui édite du texte » est évalué À L'ÉMISSION, ici : c'est un
/// fait egui, pas un fait de hub. Le consommateur n'a donc rien à re-vérifier.
#[derive(Message, Debug, Clone, Copy)]
pub struct MenuBackRequested;

/// Handler ESC unique :
///  - Menu    → émet [`MenuBackRequested`] (la pile vit dans `forgia-menu-hub`)
///  - InGame  → Paused (pause gameplay, libère curseur)
///  - Paused  → InGame (resume)
///  - Paused + Q → Menu (quit to menu)
///
/// B manette au menu = le même geste (le hint promet « Ⓑ Annuler ») — lu côté
/// bevy ICI même, dans l'unique handler (anti-trap V1 « 2 handlers ESC ») : la
/// traduction egui de B (`gamepad_nav`) ne fait que rendre le focus des widgets.
pub(crate) fn escape_handler(
    keys: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut back: MessageWriter<MenuBackRequested>,
    mut contexts: EguiContexts,
    pads: Query<&Gamepad>,
) {
    let from = app_state.get().clone();

    // Q during Paused = quit to Menu
    if matches!(from, AppMode::Paused) && keys.just_pressed(KeyCode::KeyQ) {
        info!("[forgia-ui] Q pressed (Paused → Menu)");
        next_app.set(AppMode::Menu);
        next_game.set(GameMode::None);
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        match &from {
            AppMode::InGame => {
                info!("[forgia-ui] ESC pressed (InGame → Paused)");
                next_app.set(AppMode::Paused);
                if let Ok(mut opts) = q_cursor.single_mut() {
                    opts.grab_mode = CursorGrabMode::None;
                    opts.visible = true;
                }
            }
            AppMode::Paused => {
                info!("[forgia-ui] ESC pressed (Paused → InGame)");
                next_app.set(AppMode::InGame);
                if let Ok(mut opts) = q_cursor.single_mut() {
                    opts.grab_mode = FPS_GRAB_MODE;
                    opts.visible = false;
                }
            }
            AppMode::Menu => {
                request_menu_back(&mut back, &mut contexts);
            }
            AppMode::Boot => {
                info!("[forgia-ui] ESC pressed in Boot — no transition");
            }
        }
    }

    // B manette — menu uniquement : in-game, East appartient au gameplay.
    // `!Escape` : certains mappings (Steam Input) émettent B ET Échap la même
    // frame — sans ce garde, les deux branches en demanderaient chacune une.
    if matches!(from, AppMode::Menu)
        && !keys.just_pressed(KeyCode::Escape)
        && pads.iter().any(|p| p.just_pressed(GamepadButton::East))
    {
        request_menu_back(&mut back, &mut contexts);
    }
}

/// ESC/B au menu : demande de remonter d'un niveau — SAUF quand egui édite du
/// texte (le champ du nom de la fiche consomme ESC pour rendre son focus : le
/// premier ESC sort du champ, le second remonte).
///
/// ⚠️ Couplage d'ordre : le « premier ESC sort du champ » suppose que ce code
/// (Update) lit `wants_keyboard_input` AVANT que le pass egui de la frame ne
/// traite la touche — garanti par le `MainScheduleOrder` fixe de Bevy
/// (Update < PostUpdate, où vit `EguiPrimaryContextPass`). Déplacer
/// `escape_handler` hors d'Update casserait ce 2-temps en silence.
///
/// C'est aussi pourquoi le garde reste ICI et ne part pas avec la pile : le
/// consommateur du message tourne plus tard, quand egui a déjà vu la touche.
fn request_menu_back(back: &mut MessageWriter<MenuBackRequested>, contexts: &mut EguiContexts) {
    let egui_edits_text = contexts
        .ctx_mut()
        .map(|c| c.wants_keyboard_input())
        .unwrap_or(false);
    if egui_edits_text {
        return;
    }
    back.write(MenuBackRequested);
}

/// Freeze `Time<Virtual>` quand on entre en Paused (animations + locomotion
/// + physics gated par Time<Virtual> stoppent). Pattern Bevy standard.
pub(crate) fn pause_time(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.pause();
    info!("[forgia-ui] Time<Virtual> paused");
}

/// Reprend le temps virtuel quand on sort de Paused.
pub(crate) fn resume_time(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.unpause();
    info!("[forgia-ui] Time<Virtual> resumed");
}
