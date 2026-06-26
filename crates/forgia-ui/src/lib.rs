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

use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_input::prelude::InputBlockers;
use forgia_ui_lib::pause_menu::{draw_settings_controls, save_user_settings, UserSettings};
use forgia_ui_lib::style::{
    cartoon_btn, C_PRIMARY, FORGE_BOIS_CLAIR, FORGE_CREME, FORGE_METAL_CHAUD, FORGE_OR,
};
use forgia_ui_lib::theme::display_text;
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
mod menu_video;
use menu_video::MenuVideoState;

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
        app.init_resource::<MenuPage>()
            .add_systems(Startup, (spawn_menu_camera_permanent, menu_video::setup_menu_video))
            // Fond vidéo menu : tick (avance frame) + sensor (forgia2_menu_video.json).
            .add_systems(
                Update,
                (menu_video::menu_video_tick, menu_video::menu_video_sensor),
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
            .add_systems(EguiPrimaryContextPass, main_menu_ui)
            .add_systems(Update, escape_handler.in_set(GameSet::UI));
    }
}

#[derive(Component)]
struct MenuCamera2d;

/// MenuCamera2d permanente — spawn une fois au Startup, JAMAIS despawn.
fn spawn_menu_camera_permanent(mut commands: Commands, q: Query<Entity, With<MenuCamera2d>>) {
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

/// Sous-page du menu titre (design: roguelite-home-hub-proposal 2026-06-26, P1).
/// Navigation purement UI-locale : PAS un variant d'`AppMode` (qui vit dans
/// forgia-core et est partagé). Reset à `Root` sur OnEnter(AppMode::Menu).
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
enum MenuPage {
    #[default]
    Root,
    Options,
}

/// Revient à la page racine du menu à chaque entrée dans le menu (retour jeu→menu).
fn reset_menu_page(mut page: ResMut<MenuPage>) {
    *page = MenuPage::Root;
}

fn main_menu_ui(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut exit: MessageWriter<AppExit>,
    mut video: Option<ResMut<MenuVideoState>>,
    asset_server: Res<AssetServer>,
    mut page: ResMut<MenuPage>,
    mut settings: ResMut<UserSettings>,
    meta_save: Option<Res<forgia_mode_roguelite::meta_shop::MetaShopSave>>,
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

    // Fond vidéo : maintient le cache LRU + preroll, renvoie le TextureId de la
    // frame courante. None → fallback fond sombre uni (preroll en cours / asset
    // absent). Calculé AVANT `ctx_mut` (libère le &mut EguiContexts).
    let bg_id = video
        .as_deref_mut()
        .and_then(|v| menu_video::ensure_menu_video_frame(&mut contexts, v, &asset_server));

    let Ok(ctx) = contexts.ctx_mut() else {
        warn!("[forgia-ui] main_menu_ui: egui ctx not found (no Camera2d?)");
        return;
    };
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(8, 8, 12)))
        .show(ctx, |ui| {
            // Fond vidéo plein écran + scrim dégradé vertical (story-596 —
            // remplace le voile plat alpha 90 : haut léger, bas dense pour
            // asseoir les boutons sans éteindre la vidéo).
            if let Some(id) = bg_id {
                let rect = ui.max_rect();
                ui.painter().image(
                    id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                let mut scrim = egui::Mesh::default();
                let top = egui::Color32::from_black_alpha(40);
                let bottom = egui::Color32::from_black_alpha(170);
                scrim.colored_vertex(rect.left_top(), top);
                scrim.colored_vertex(rect.right_top(), top);
                scrim.colored_vertex(rect.right_bottom(), bottom);
                scrim.colored_vertex(rect.left_bottom(), bottom);
                scrim.add_triangle(0, 1, 2);
                scrim.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(scrim));
            }
            match *page {
                // ── Page racine : menu joueur Roguelite (design home-hub P1) ──
                MenuPage::Root => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(96.0);
                        // titre display (Lilita One) + orange Forgia canon.
                        ui.heading(display_text("FORGIA", 84.0, C_PRIMARY).strong());
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("ROGUELITE")
                                .size(22.0)
                                .color(FORGE_CREME)
                                .strong(),
                        );
                        ui.add_space(44.0);

                        // « A déjà joué » = méta persistée (Âmes / upgrades achetés /
                        // armes débloquées au-delà de Pépin). Pilote Continuer actif +
                        // quel bouton porte le CTA principal (or).
                        let has_save = meta_save.as_ref().is_some_and(|s| {
                            s.souls_total > 0
                                || !s.ranks.is_empty()
                                || s.unlocked_weapons.len() > 1
                        });

                        // CONTINUER — relance avec la progression méta (L'Enclume).
                        // Grisé tant qu'aucune partie n'a laissé de méta.
                        let mut continue_clicked = false;
                        ui.add_enabled_ui(has_save, |ui| {
                            continue_clicked =
                                cartoon_btn(ui, "▶  CONTINUER", FORGE_OR).clicked();
                        });
                        if continue_clicked {
                            next_game.set(GameMode::Roguelite);
                            next_app.set(AppMode::InGame);
                        }
                        ui.add_space(16.0);

                        // NOUVELLE PARTIE — entre au Lobby (le wizard nom+style est la
                        // phase suivante du design). CTA or quand pas de save, sinon
                        // bois pour laisser Continuer primer.
                        let nouvelle_color = if has_save { FORGE_BOIS_CLAIR } else { FORGE_OR };
                        if cartoon_btn(ui, "✦  NOUVELLE PARTIE", nouvelle_color).clicked() {
                            next_game.set(GameMode::Roguelite);
                            next_app.set(AppMode::InGame);
                        }
                        ui.add_space(16.0);

                        if cartoon_btn(ui, "⚙  OPTIONS", FORGE_BOIS_CLAIR).clicked() {
                            *page = MenuPage::Options;
                        }
                        ui.add_space(16.0);

                        if cartoon_btn(ui, "✕  QUITTER", FORGE_METAL_CHAUD).clicked() {
                            exit.write(AppExit::Success);
                        }

                        // Démos moteur (dev) — secondaires/discrètes. Conservées pour
                        // l'accès dev (RPG openworld / stress-test GLB cyber city),
                        // hors parcours joueur Roguelite.
                        ui.add_space(32.0);
                        ui.label(
                            egui::RichText::new("— Démos moteur (dev) —")
                                .size(12.0)
                                .color(egui::Color32::from_gray(150)),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new("🗺 RPG").size(15.0))
                                        .min_size(egui::vec2(120.0, 32.0)),
                                )
                                .clicked()
                            {
                                next_game.set(GameMode::Rpg);
                                next_app.set(AppMode::InGame);
                            }
                            ui.add_space(10.0);
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("🏙 Cyber City").size(15.0),
                                    )
                                    .min_size(egui::vec2(140.0, 32.0)),
                                )
                                .clicked()
                            {
                                next_game.set(GameMode::CyberCity);
                                next_app.set(AppMode::InGame);
                            }
                        });
                    });
                }
                // ── Page Options : réutilise les contrôles du pause menu (DRY) ──
                MenuPage::Options => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(52.0);
                        ui.heading(display_text("OPTIONS", 56.0, C_PRIMARY).strong());
                    });
                    ui.add_space(14.0);
                    // Conteneur centré largeur bornée (lisibilité des sliders).
                    let avail = ui.available_width();
                    let panel_w = 520.0_f32.min((avail - 40.0).max(280.0));
                    let margin = ((avail - panel_w) * 0.5).max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(margin);
                        ui.vertical(|ui| {
                            ui.set_max_width(panel_w);
                            // Anti-crash : bypass + set_changed() SEULEMENT si un
                            // contrôle change (sinon apply_window_settings boucle →
                            // resize → race wgpu, cf draw_pause_menu).
                            let dirty =
                                draw_settings_controls(ui, settings.bypass_change_detection());
                            if dirty {
                                settings.set_changed();
                            }
                            ui.add_space(16.0);
                            let mut save_clicked = false;
                            let mut back_clicked = false;
                            ui.horizontal(|ui| {
                                save_clicked =
                                    cartoon_btn(ui, "💾 Sauvegarder", FORGE_BOIS_CLAIR).clicked();
                                ui.add_space(12.0);
                                back_clicked =
                                    cartoon_btn(ui, "← Retour", FORGE_METAL_CHAUD).clicked();
                            });
                            if save_clicked {
                                save_user_settings(settings.bypass_change_detection());
                            }
                            if back_clicked {
                                *page = MenuPage::Root;
                            }
                        });
                    });
                }
            }
        });
}

/// Overlay PAUSED legacy (remplacé story-455 Phase G par forgia-ui-pause-menu cliquable).
/// Conservé en `#[allow(dead_code)]` pour référence courte ; à supprimer story-457.
#[allow(dead_code)]
fn paused_overlay_ui(app_state: Res<State<AppMode>>, mut ctx: EguiContexts) {
    if !matches!(app_state.get(), AppMode::Paused) {
        return;
    }
    let Ok(ctx) = ctx.ctx_mut() else { return };
    egui::Area::new(egui::Id::new("paused_overlay"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(180))
                .inner_margin(egui::Margin::symmetric(48, 32))
                .corner_radius(egui::CornerRadius::same(8))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(4.0);
                        ui.heading(
                            egui::RichText::new("PAUSED")
                                .size(56.0)
                                .color(egui::Color32::from_rgb(255, 230, 100))
                                .strong(),
                        );
                        ui.add_space(18.0);
                        ui.label(
                            egui::RichText::new("ESC — Resume")
                                .size(20.0)
                                .color(egui::Color32::WHITE),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Q   — Quit to Menu")
                                .size(20.0)
                                .color(egui::Color32::from_gray(200)),
                        );
                        ui.add_space(4.0);
                    });
                });
        });
}

/// Handler ESC unique :
///  - InGame  → Paused (pause gameplay, libère curseur)
///  - Paused  → InGame (resume)
///  - Paused + Q → Menu (quit to menu)
fn escape_handler(
    keys: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
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
                    opts.grab_mode = CursorGrabMode::Locked;
                    opts.visible = false;
                }
            }
            other => {
                info!("[forgia-ui] ESC pressed in {other:?} — no transition");
            }
        }
    }
    // Q en Paused → quitte au Menu.
    if keys.just_pressed(KeyCode::KeyQ) && matches!(from, AppMode::Paused) {
        info!("[forgia-ui] Q pressed (Paused → Menu)");
        next_app.set(AppMode::Menu);
        next_game.set(GameMode::None);
    }
}

/// Freeze `Time<Virtual>` quand on entre en Paused (animations + locomotion
/// + physics gated par Time<Virtual> stoppent). Pattern Bevy standard.
fn pause_time(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.pause();
    info!("[forgia-ui] Time<Virtual> paused");
}

/// Reprend le temps virtuel quand on sort de Paused.
fn resume_time(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.unpause();
    info!("[forgia-ui] Time<Virtual> resumed");
}

/// Lock cursor au centre + invisible quand on entre InGame (pour mouse_look).
fn grab_cursor(mut q: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut opts) = q.single_mut() {
        opts.grab_mode = CursorGrabMode::Locked;
        opts.visible = false;
        info!("[forgia-ui] Cursor grabbed (Locked + invisible)");
    } else {
        warn!("[forgia-ui] grab_cursor: PrimaryWindow CursorOptions not found");
    }
}

/// Story-528 follow-up — bloque mouse_look + block fire pendant Roguelite
/// Defeat/Victory pour que la souris puisse cliquer les boutons end-of-run
/// sans pivoter la caméra ni tirer.
fn block_look_on(mut blockers: ResMut<InputBlockers>) {
    blockers.block_look = true;
    blockers.block_fire = true;
    info!("[forgia-ui] InputBlockers: look+fire ON (Roguelite end-of-run)");
}

fn block_look_off(mut blockers: ResMut<InputBlockers>) {
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
fn sys_sync_cursor_with_coffre(
    app_state: Res<State<AppMode>>,
    session: Option<Res<forgia_rpg_data::boons::CoffreSession>>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut blockers: ResMut<InputBlockers>,
    mut was_open: Local<bool>,
) {
    if *app_state.get() != AppMode::InGame {
        return;
    }
    let is_open = session.as_ref().is_some_and(|s| s.is_open);
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
            info!("[forgia-ui] Coffre OPEN — cursor released, look+fire blocked");
        } else {
            opts.grab_mode = CursorGrabMode::Locked;
            opts.visible = false;
            blockers.block_look = false;
            blockers.block_fire = false;
            info!("[forgia-ui] Coffre CLOSED — cursor grabbed, look+fire unblocked");
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
fn sys_regrab_cursor_on_focus(
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
        opts.grab_mode = CursorGrabMode::Locked;
        opts.visible = false;
        info!("[forgia-ui] focus regagné — curseur re-grabbed (anti alt-tab)");
    }
}

/// Release cursor (visible + free) quand on entre Menu.
fn release_cursor(mut q: Query<&mut CursorOptions, With<PrimaryWindow>>) {
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
fn sys_force_lobby_cursor_free(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaUiPlugin;
    }
}
