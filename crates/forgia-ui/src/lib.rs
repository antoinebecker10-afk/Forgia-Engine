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
        app.add_systems(Startup, (spawn_menu_camera_permanent, menu_video::setup_menu_video))
            // Fond vidéo menu : tick (avance frame) + sensor (forgia2_menu_video.json).
            .add_systems(
                Update,
                (menu_video::menu_video_tick, menu_video::menu_video_sensor),
            )
            .add_systems(OnEnter(AppMode::Menu), release_cursor)
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
            .add_systems(Update, sys_sync_cursor_with_coffre)
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

fn main_menu_ui(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut exit: MessageWriter<AppExit>,
    mut start_run: MessageWriter<forgia_mode_roguelite::StartRunEvent>,
    mut video: Option<ResMut<MenuVideoState>>,
    asset_server: Res<AssetServer>,
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
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                // Story-596 — titre display (Lilita One) + orange Forgia canon.
                ui.heading(display_text("FORGIA", 84.0, C_PRIMARY).strong());
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("Choisis ton mode")
                        .size(24.0)
                        .color(FORGE_CREME),
                );
                ui.add_space(60.0);

                // Mode "FPS Arena" retiré du menu (2026-06-04, décision user) :
                // l'arène nue était redondante avec Roguelite (arène décorée =
                // seule arène de jeu désormais). Le variant `GameMode::Fps` reste
                // dans l'enum : de nombreux systèmes partagés (HUD ammo/hp/wave,
                // viewmodel, killfeed, screen-flash, sensors, système de vagues)
                // sont gatés `Fps | Roguelite`, et `forgia-mode-roguelite` réutilise
                // `forgia-mode-fps-arena` (TargetCube/WaveState). Sans entrée menu,
                // `OnEnter(Fps)` ne tire plus jamais → l'arène nue ne se spawn pas.
                // Suppression complète du crate = refactor séparé (Roguelite en dépend).
                // Story-596 — boutons cartoon partagés (bois / or CTA / métal).
                if cartoon_btn(ui, "🗺  RPG OPENWORLD", FORGE_BOIS_CLAIR).clicked() {
                    next_game.set(GameMode::Rpg);
                    next_app.set(AppMode::InGame);
                }
                ui.add_space(20.0);
                if cartoon_btn(ui, "🎲  ROGUELITE RUN", FORGE_OR).clicked() {
                    next_game.set(GameMode::Roguelite);
                    next_app.set(AppMode::InGame);
                    start_run.write(forgia_mode_roguelite::StartRunEvent { seed: None });
                }
                ui.add_space(20.0);
                // Démo perf moteur (2026-06-15) — charge un GLB lourd (cyberpunk
                // city) + flycam libre pour stress-tester rendu/VRAM. Bleu cyber
                // pour la distinguer des modes de jeu.
                if cartoon_btn(ui, "🏙  CYBER CITY DÉMO", egui::Color32::from_rgb(70, 130, 200))
                    .clicked()
                {
                    next_game.set(GameMode::CyberCity);
                    next_app.set(AppMode::InGame);
                }
                ui.add_space(40.0);
                if cartoon_btn(ui, "QUITTER", FORGE_METAL_CHAUD).clicked() {
                    exit.write(AppExit::Success);
                }
            });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaUiPlugin;
    }
}
