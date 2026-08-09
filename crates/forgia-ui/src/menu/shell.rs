//! Le SHELL du menu : caméra 2D permanente, échelle UI globale (toile 1080p,
//! story-692), le dispatcher `main_menu_ui`, le handler ESC unique et la
//! pause du temps virtuel.

use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_egui::{egui, EguiContexts};
use forgia_core::prelude::*;
use forgia_mode_roguelite::identity::IdentitySave;
use forgia_mode_roguelite::meta_shop::MetaShopSave;
use forgia_mode_roguelite::progress::PlayerProgress;
use forgia_ui_lib::pause_menu::UserSettings;

use crate::arena_backdrop::ArenaBackdropRtt;
use crate::currency_icons::CurrencyIcons;
use crate::menu::chrome::{
    draw_hub_smith_chip, draw_hub_souls_chip, hub_section_panel,
};
use crate::menu::cursor::FPS_GRAB_MODE;
use crate::menu::nav::{draw_hub_nav, HubBadges, MenuAction, MenuPage, NavStack};
use crate::menu::registry::{self, InlinePageCtx, PageDraw};
use crate::menu::pages::root::{draw_options_page, draw_root_landing};
use crate::menu_video::{self, MenuVideoState};

#[derive(Component)]
pub(crate) struct MenuCamera2d;

/// MenuCamera2d permanente — spawn une fois au Startup, JAMAIS despawn.
pub(crate) fn spawn_menu_camera_permanent(mut commands: Commands, q: Query<Entity, With<MenuCamera2d>>) {
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

/// Story-693 — nourrit le capteur d'identité : « l'édition du nom/couleur a
/// été montrée ». Son poseur historique (panneau Lobby) est supprimé — la
/// fiche Forgeron du menu est LA surface d'édition. Marqué à la page plutôt
/// qu'au call-site : le système de la fiche est au plafond des 16 params Bevy.
pub(crate) fn sys_mark_identity_shown(
    app_state: Res<State<AppMode>>,
    nav: Res<NavStack>,
    shown: Option<ResMut<forgia_mode_roguelite::identity::IdentityPanelShown>>,
) {
    if *app_state.get() != AppMode::Menu || nav.current() != MenuPage::Forgeron {
        return;
    }
    if let Some(mut s) = shown {
        if !s.0 {
            s.0 = true;
        }
    }
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

pub(crate) fn main_menu_ui(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut exit: MessageWriter<AppExit>,
    mut video: Option<ResMut<MenuVideoState>>,
    asset_server: Res<AssetServer>,
    mut nav: ResMut<NavStack>,
    mut settings: ResMut<UserSettings>,
    // Données persistées au Startup → présentes dès le menu (hub roguelite).
    meta_save: Option<Res<MetaShopSave>>,
    progress: Option<Res<PlayerProgress>>,
    identity: Option<Res<IdentitySave>>,
    // Story-678 Phase 4 — pastilles calculées par `sys_hub_badges`.
    badges: Res<HubBadges>,
    // Story-678 Phase 5 — le fond d'arène du chapitre atteint (diorama RTT).
    backdrop: Option<Res<ArenaBackdropRtt>>,
    // Story-678 — icônes des monnaies (absentes tant qu'egui n'a pas de contexte).
    icons: Option<Res<CurrencyIcons>>,
    mut last_state: Local<Option<AppMode>>,
) {
    let current = app_state.get().clone();
    if last_state.as_ref() != Some(&current) {
        info!("[forgia-ui] main_menu_ui state = {current:?}");
        *last_state = Some(current.clone());
    }
    if current != AppMode::Menu {
        return;
    }

    // ── Quel fond ? ──
    // Story-678 Phase 5 — l'arène du chapitre atteint remplace la vidéo de
    // branding. On ne bascule QUE si le diorama a réellement posé des props :
    // une palette vide donnerait un écran nu, et le repli vidéo vaut mieux
    // qu'un fond noir. Le capteur publie ce compte.
    let arena_bg = backdrop
        .as_deref()
        .filter(|b| b.is_showing())
        .map(|b| b.tex_id);
    // Fond vidéo : maintient le cache LRU + preroll, renvoie le TextureId de la
    // frame courante. None → fallback fond sombre uni (preroll en cours / asset
    // absent). Calculé AVANT `ctx_mut` (libère le &mut EguiContexts).
    //
    // Sous l'arène, le pipeline vidéo est GELÉ (story-691) : le tick n'avance
    // plus la frame et on ne touche plus au cache — les 8 frames restent en
    // VRAM et `prerolled` reste vrai, donc couper `ui_backdrop_enabled` à chaud
    // réaffiche la vidéo immédiatement, sans re-preroll ni décodage à vide.
    let video_bg = if arena_bg.is_some() {
        None
    } else {
        video
            .as_deref_mut()
            .and_then(|v| menu_video::ensure_menu_video_frame(&mut contexts, v, &asset_server))
    };
    let bg_id = arena_bg.or(video_bg);
    // Le scrim s'adapte : sur l'arène il épargne le tiers droit (le personnage
    // y est), sur la vidéo il reste le dégradé vertical d'origine.
    let bg_is_arena = arena_bg.is_some();

    let Ok(ctx) = contexts.ctx_mut() else {
        warn!("[forgia-ui] main_menu_ui: egui ctx not found (no Camera2d?)");
        return;
    };

    // ── Couche background : fond vidéo plein écran + scrim dégradé vertical ──
    // (story-596 — haut léger, bas dense pour asseoir l'UI sans éteindre la vidéo).
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(8, 8, 12)))
        .show(ctx, |ui| {
            if let Some(id) = bg_id {
                let rect = ui.max_rect();
                ui.painter().image(
                    id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                // Scrim : assied l'UI sans éteindre le fond.
                //
                // Sur l'ARÈNE il devient DIAGONAL — dense à gauche, où vivent
                // la carte de chapitre et son texte ; presque nul à droite, où
                // le personnage se tient. Un dégradé purement vertical
                // (celui de la vidéo) noyait le personnage dans le même voile
                // que le texte, alors qu'il est le sujet de ce côté-là.
                let mut scrim = egui::Mesh::default();
                let (lt, rt, rb, lb) = if bg_is_arena {
                    (
                        egui::Color32::from_black_alpha(130),
                        egui::Color32::from_black_alpha(25),
                        egui::Color32::from_black_alpha(85),
                        egui::Color32::from_black_alpha(185),
                    )
                } else {
                    let top = egui::Color32::from_black_alpha(40);
                    let bottom = egui::Color32::from_black_alpha(170);
                    (top, top, bottom, bottom)
                };
                scrim.colored_vertex(rect.left_top(), lt);
                scrim.colored_vertex(rect.right_top(), rt);
                scrim.colored_vertex(rect.right_bottom(), rb);
                scrim.colored_vertex(rect.left_bottom(), lb);
                scrim.add_triangle(0, 1, 2);
                scrim.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(scrim));
            }
            // Story-678 — le fantôme plein écran de l'avatar a été remplacé par
            // le portrait CADRÉ de l'écran de préparation (le personnage géant
            // débordait du cadre, retour utilisateur 2026-08-05). Le portrait
            // vit dans `sys_menu_root_dashboard` (colonne droite).
        });

    // ── Données persistées (chargées au Startup → lisibles dès le menu) ──
    let souls_n = meta_save.as_ref().map(|s| s.souls_total).unwrap_or(0);
    let shards_n = meta_save.as_ref().map(|s| s.shards_total).unwrap_or(0);
    let name = identity
        .as_ref()
        .map(|i| {
            if i.player_name.is_empty() {
                "Forgeron"
            } else {
                i.player_name.as_str()
            }
        })
        .unwrap_or("Forgeron");
    // Story-680 cran 1 — le niveau est la SOMME DES RANGS achetés à l'Enclume
    // (modèle Gunfire Reborn). Plus d'XP « 40 + secondes de run », qui payait le
    // temps passé et rien d'autre. La barre mesure la part de l'Enclume
    // réellement débloquée — une information vraie.
    let (level, remaining, frac) = progress
        .as_ref()
        .map(|p| (p.level, p.ranks_remaining, p.completion()))
        .unwrap_or((1, 0, 0.0));

    // ── Chrome persistant du haut d'écran ──
    // Il se lit de gauche à droite comme une phrase : QUI je suis · OÙ je vais ·
    // CE QUE j'ai. Les trois vivent sur la même bande depuis que la navigation
    // est passée à l'horizontale.
    draw_hub_smith_chip(ctx, name, level, remaining, frac);
    draw_hub_nav(ctx, &mut nav, *badges);
    draw_hub_souls_chip(ctx, souls_n, shards_n, icons.as_deref());

    // ── Panneau de la section active — piloté par LA table (story-694 incr. 4).
    // Root et Options restent câblés ici (Root rend une MenuAction appliquée
    // plus bas — évite de passer NextState/MessageWriter dans chaque helper ;
    // Options mute la pile) ; les pages INLINE tirent leur dessinateur du
    // registre ; les pages OwnSystem sont dessinées par leur système auto-gaté
    // — rien à faire dans ce match. Ajouter une page inline = 1 PageDecl + 1 fn.
    let action = match nav.current() {
        MenuPage::Root => draw_root_landing(ctx),
        MenuPage::Options => {
            draw_options_page(ctx, &mut nav, &mut settings);
            MenuAction::None
        }
        page => {
            let d = registry::decl(page);
            if let PageDraw::Inline {
                panel_id,
                panel_width,
                draw,
            } = d.draw
            {
                let ictx = InlinePageCtx {
                    level,
                    ranks_remaining: remaining,
                    meta_save: meta_save.as_deref(),
                };
                hub_section_panel(ctx, panel_id, d.section_title, panel_width, |ui| {
                    draw(&ictx, ui)
                });
            }
            MenuAction::None
        }
    };

    match action {
        MenuAction::Launch(mode) => {
            next_game.set(mode);
            next_app.set(AppMode::InGame);
        }
        MenuAction::Quit => {
            exit.write(AppExit::Success);
        }
        MenuAction::None => {}
    }
}


// `draw_arena_test_section` retirée avec l'onglet (2026-07-30, temporaire) — elle
// n'avait plus qu'un appelant, supprimé lui aussi. Son contenu est dans
// l'historique git de ce fichier ; la restaurer suffit à rouvrir le banc.
// Rien n'a été touché côté moteur.

/// Handler ESC unique :
///  - Menu    → remonte d'un niveau de la pile de navigation (story-694 incr. 3)
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
    mut nav: ResMut<NavStack>,
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
                menu_back(&mut nav, &mut contexts);
            }
            AppMode::Boot => {
                info!("[forgia-ui] ESC pressed in Boot — no transition");
            }
        }
    }
    // Bloc « Q en Paused » dupliqué SUPPRIMÉ (story-694 incr. 2, constat n°9 de
    // l'audit 2026-08-07) : le même test vit en tête de fonction avec un `return`,
    // cette copie était inatteignable quand il tirait.

    // B manette — menu uniquement : in-game, East appartient au gameplay.
    // `!Escape` : certains mappings (Steam Input) émettent B ET Échap la même
    // frame — sans ce garde, les deux branches popperaient chacune un niveau.
    if matches!(from, AppMode::Menu)
        && !keys.just_pressed(KeyCode::Escape)
        && pads.iter().any(|p| p.just_pressed(GamepadButton::East))
    {
        menu_back(&mut nav, &mut contexts);
    }
}

/// ESC/B au menu : remonte d'un niveau de la pile — SAUF quand egui édite du
/// texte (le champ du nom de la fiche consomme ESC pour rendre son focus : le
/// premier ESC sort du champ, le second remonte) et sauf au Root, où `back()`
/// rend `false` et on ne fait rien (Quitter est un bouton explicite).
///
/// ⚠️ Couplage d'ordre : le « premier ESC sort du champ » suppose que ce code
/// (Update) lit `wants_keyboard_input` AVANT que le pass egui de la frame ne
/// traite la touche — garanti par le `MainScheduleOrder` fixe de Bevy
/// (Update < PostUpdate, où vit `EguiPrimaryContextPass`). Déplacer
/// `escape_handler` hors d'Update casserait ce 2-temps en silence.
fn menu_back(nav: &mut NavStack, contexts: &mut EguiContexts) {
    let egui_edits_text = contexts
        .ctx_mut()
        .map(|c| c.wants_keyboard_input())
        .unwrap_or(false);
    if egui_edits_text {
        return;
    }
    if nav.back() {
        info!("[forgia-ui] retour nav (ESC/B) → {:?}", nav.current());
    }
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

