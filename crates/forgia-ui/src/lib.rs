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
    cartoon_btn, glass_btn, glass_frame_hero, C_PRIMARY, C_TEXT_MUTED, FORGE_CREME, FORGE_OR,
    FORGE_PANEL, HAIR_GOLD_STRONG,
};
use forgia_ui_lib::theme::display_text;
// Hub-menu (story-menu-hub) : le menu-titre devient le hub roguelite complet.
// `forgia-ui` → `forgia-mode-roguelite` dep existe (pas de cycle) → on lit les Res
// persistées au Startup (présentes dès le menu) et on réutilise les helpers de
// sections déjà écrits dans `hub.rs` (zéro duplication).
use forgia_mode_roguelite::hub::{draw_codex_section, section_intro};
use forgia_mode_roguelite::identity::{draw_identity_content, IdentityConfig, IdentitySave};
use forgia_mode_roguelite::meta_shop::{
    apply_meta_purchase, draw_enclume_panel, MetaShopCatalogue, MetaShopSave,
};
use forgia_mode_roguelite::elements::ElementConfig;
use forgia_mode_roguelite::pipeline_warmup::WarmupState;
use forgia_mode_roguelite::progress::PlayerProgress;
use forgia_mode_roguelite::run::MetaSouls;
use forgia_mode_roguelite::StartRunEvent;
use forgia_mode_roguelite::weapon_select::{
    draw_weapon_menu_panel, StartingWeaponChoice, WeaponCards,
};
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

/// Aperçus 3D (arme + bras) au hub-menu (render-to-texture). Story-menu-hub étape 5b.
mod weapon_preview;
use weapon_preview::{ArmPreviewRtt, WeaponPreviewPlugin, WeaponPreviewRtt};

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
            // Hub-menu : menu principal + sections interactives (Enclume cliquable,
            // Forgeron/identité). Areas indépendantes, chaînées pour un ordre de
            // dessin déterministe.
            .add_systems(
                EguiPrimaryContextPass,
                (
                    main_menu_ui,
                    sys_menu_enclume,
                    sys_menu_forgeron,
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

/// Étape 6 hub-menu — lancement direct : le Lobby n'est plus un hub interactif mais
/// un **gate de chargement**. Dès que le warmup PBR est prêt (`WarmupState.done`),
/// auto-fire `StartRunEvent` → combat, sans action utilisateur (tout est configuré
/// au menu). L'anti-double-spawn de `sys_start_run` + `run_if(Lobby)` (qui coupe
/// après la transition InRun) bornent le tir. Replay (Defeat/Victory → Lobby) :
/// `done` reste vrai → lancement instantané.
fn sys_auto_start_when_warm(
    warmup: Option<Res<WarmupState>>,
    mut start_run: MessageWriter<StartRunEvent>,
) {
    if warmup.as_ref().is_some_and(|w| w.done) {
        start_run.write(StartRunEvent { seed: None });
    }
}

/// Overlay de chargement plein écran pendant le gate Lobby — couvre l'ancien hub
/// interactif le temps du warmup, avant l'auto-lancement. Layer Foreground (au-
/// dessus des panneaux du hub, ordre Middle).
fn sys_lobby_loading_overlay(mut contexts: EguiContexts, warmup: Option<Res<WarmupState>>) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let ready = warmup.as_ref().is_some_and(|w| w.done);
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("lobby_loading_overlay"),
    ));
    painter.rect_filled(screen, 0.0, egui::Color32::from_rgb(12, 9, 16));
    painter.text(
        screen.center(),
        egui::Align2::CENTER_CENTER,
        if ready {
            "La Forge est prête…"
        } else {
            "Préparation de la Forge…"
        },
        egui::FontId::proportional(34.0),
        FORGE_CREME,
    );
    painter.text(
        screen.center() + egui::vec2(0.0, 44.0),
        egui::Align2::CENTER_CENTER,
        "◆ ◆ ◆",
        egui::FontId::proportional(18.0),
        FORGE_OR,
    );
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

/// Sous-page du menu titre — devenu **hub roguelite complet** (story-menu-hub).
/// Navigation purement UI-locale : PAS un variant d'`AppMode` (qui vit dans
/// forgia-core et est partagé). Reset à `Root` sur OnEnter(AppMode::Menu).
///
/// `Root` = accueil (titre FORGIA + CONTINUER/NOUVELLE PARTIE). Les autres = les
/// sections du hub, navigables depuis la sidebar verre sans lancer de run.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
enum MenuPage {
    #[default]
    Root,
    Forgeron,
    Armes,
    Talents,
    Enclume,
    Codex,
    Missions,
    Succes,
    Stats,
    Options,
}

impl MenuPage {
    /// Ordre de la sidebar de navigation (Accueil en tête, Options en pied).
    const NAV: [MenuPage; 10] = [
        MenuPage::Root,
        MenuPage::Forgeron,
        MenuPage::Armes,
        MenuPage::Talents,
        MenuPage::Enclume,
        MenuPage::Codex,
        MenuPage::Missions,
        MenuPage::Succes,
        MenuPage::Stats,
        MenuPage::Options,
    ];

    /// Libellé de l'onglet dans la sidebar (icône + nom court).
    fn nav_label(self) -> &'static str {
        match self {
            MenuPage::Root => "⌂  Accueil",
            MenuPage::Forgeron => "⚒  Forgeron",
            MenuPage::Armes => "🗡  Armes",
            MenuPage::Talents => "✦  Talents",
            MenuPage::Enclume => "🔨  Enclume",
            MenuPage::Codex => "📖  Codex",
            MenuPage::Missions => "🎯  Missions",
            MenuPage::Succes => "🏆  Succès",
            MenuPage::Stats => "📊  Stats",
            MenuPage::Options => "⚙  Options",
        }
    }

    /// Titre display (Lilita) affiché en tête du panneau de section.
    fn section_title(self) -> &'static str {
        match self {
            MenuPage::Root => "FORGIA",
            MenuPage::Forgeron => "TON FORGERON",
            MenuPage::Armes => "TES ARMES",
            MenuPage::Talents => "TALENTS",
            MenuPage::Enclume => "L'ENCLUME DES ÂMES",
            MenuPage::Codex => "CODEX · BESTIAIRE",
            MenuPage::Missions => "MISSIONS",
            MenuPage::Succes => "HAUTS FAITS",
            MenuPage::Stats => "STATISTIQUES",
            MenuPage::Options => "OPTIONS",
        }
    }
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
    // Données persistées au Startup → présentes dès le menu (hub roguelite).
    meta_save: Option<Res<MetaShopSave>>,
    progress: Option<Res<PlayerProgress>>,
    identity: Option<Res<IdentitySave>>,
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
        });

    // ── Données persistées (chargées au Startup → lisibles dès le menu) ──
    let souls_n = meta_save.as_ref().map(|s| s.souls_total).unwrap_or(0);
    // « A déjà joué » = méta persistée (Âmes / upgrades / armes au-delà de Pépin).
    let has_save = meta_save
        .as_ref()
        .is_some_and(|s| s.souls_total > 0 || !s.ranks.is_empty() || s.unlocked_weapons.len() > 1);
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
    let (level, xp, xp_next, frac) = progress
        .as_ref()
        .map(|p| {
            (
                p.level,
                p.xp,
                PlayerProgress::xp_to_next(p.level),
                p.xp_fraction(),
            )
        })
        .unwrap_or((1, 0, 80, 0.0));

    // ── Chrome persistant : bandeau Âmes (droite) + sidebar de navigation (gauche) ──
    draw_hub_souls_chip(ctx, souls_n);
    draw_hub_nav(ctx, &mut page, name, level, xp, xp_next, frac);

    // ── Panneau de la section active ──
    // Les sections rendent leur contenu et renvoient une action de navigation
    // d'état (lancer une run / quitter) que l'on applique ici — évite de passer
    // NextState/MessageWriter dans chaque helper.
    let action = match *page {
        MenuPage::Root => draw_root_landing(ctx, has_save),
        MenuPage::Codex => {
            hub_section_panel(ctx, "hub_sec_codex", MenuPage::Codex.section_title(), 720.0, |ui| {
                draw_codex_section(ui);
            });
            MenuAction::None
        }
        MenuPage::Talents => {
            let pts = progress.as_ref().map(|p| p.talent_points).unwrap_or(0);
            hub_section_panel(
                ctx,
                "hub_sec_talents",
                MenuPage::Talents.section_title(),
                640.0,
                |ui| {
                    section_intro(
                        ui,
                        "Arbres de talents",
                        "Choisis ton style — Feu · Givre · Éclair · Poison — et débloque des \
                         combos signature en jouant.",
                    );
                    ui.add_space(8.0);
                    ui.label(display_text(
                        format!("{pts} point(s) de talent en attente"),
                        18.0,
                        FORGE_OR,
                    ));
                },
            );
            MenuAction::None
        }
        MenuPage::Missions => {
            hub_section_panel(
                ctx,
                "hub_sec_missions",
                MenuPage::Missions.section_title(),
                640.0,
                |ui| {
                    section_intro(
                        ui,
                        "Missions",
                        "Des défis quotidiens & hebdomadaires. Accomplis-les pour gagner des Âmes \
                         et débloquer des titres.",
                    );
                },
            );
            MenuAction::None
        }
        MenuPage::Succes => {
            hub_section_panel(
                ctx,
                "hub_sec_succes",
                MenuPage::Succes.section_title(),
                640.0,
                |ui| {
                    section_intro(
                        ui,
                        "Hauts faits",
                        "Repousse tes limites — chaque haut fait débloqué rapporte des Âmes et un \
                         titre à porter.",
                    );
                },
            );
            MenuAction::None
        }
        MenuPage::Stats => {
            hub_section_panel(
                ctx,
                "hub_sec_stats",
                MenuPage::Stats.section_title(),
                560.0,
                |ui| draw_stats_section(ui, meta_save.as_deref(), level),
            );
            MenuAction::None
        }
        // Forgeron / Armes / Enclume : panneaux interactifs dessinés par leurs
        // systèmes dédiés (`sys_menu_forgeron` / `sys_menu_armes` / `sys_menu_enclume`).
        MenuPage::Forgeron | MenuPage::Armes | MenuPage::Enclume => MenuAction::None,
        MenuPage::Options => {
            draw_options_page(ctx, &mut page, &mut settings);
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

/// Action de navigation d'état demandée par une section du hub-menu (appliquée par
/// `main_menu_ui`). Découple le rendu des sections des `NextState`/`MessageWriter`.
enum MenuAction {
    None,
    /// Entrer InGame dans le mode donné (Roguelite = run, Rpg/CyberCity = démos dev).
    Launch(GameMode),
    Quit,
}

/// Cadre « chip » cohérent avec le hub (verre aubergine + liseré or).
fn hub_chip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(FORGE_PANEL)
        .inner_margin(egui::Margin::symmetric(14, 8))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
}

/// Bandeau Âmes (méta persistante) — haut-droite du hub-menu.
fn draw_hub_souls_chip(ctx: &egui::Context, souls_n: u32) {
    egui::Area::new(egui::Id::new("menu_hub_souls"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-24.0, 18.0))
        .show(ctx, |ui| {
            hub_chip_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!("◇ {souls_n}  Âmes"))
                        .size(20.0)
                        .color(FORGE_OR)
                        .strong(),
                );
            });
        });
}

/// Sidebar de navigation (gauche) : en-tête forgeron (nom + niveau + XP) puis les
/// onglets des sections. `page` est muté au clic.
fn draw_hub_nav(
    ctx: &egui::Context,
    page: &mut MenuPage,
    name: &str,
    level: u32,
    xp: u32,
    xp_next: u32,
    frac: f32,
) {
    egui::Area::new(egui::Id::new("menu_hub_nav"))
        .anchor(egui::Align2::LEFT_CENTER, egui::vec2(24.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(FORGE_PANEL)
                .inner_margin(egui::Margin::symmetric(12, 14))
                .corner_radius(egui::CornerRadius::same(14))
                .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
                .show(ui, |ui| {
                    ui.set_min_width(204.0);
                    // En-tête forgeron.
                    ui.label(
                        egui::RichText::new(name)
                            .size(18.0)
                            .color(FORGE_CREME)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!("Niveau {level}"))
                            .size(13.0)
                            .color(C_TEXT_MUTED),
                    );
                    ui.add_space(4.0);
                    ui.add_sized(
                        egui::vec2(200.0, 9.0),
                        egui::ProgressBar::new(frac).fill(FORGE_OR).text(
                            egui::RichText::new(format!("{xp} / {xp_next} XP"))
                                .size(9.0)
                                .color(FORGE_CREME),
                        ),
                    );
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);
                    // Onglets.
                    for tab in MenuPage::NAV {
                        let selected = *page == tab;
                        let resp = ui.add_sized(
                            egui::vec2(200.0, 30.0),
                            egui::Button::selectable(
                                selected,
                                egui::RichText::new(tab.nav_label())
                                    .size(16.0)
                                    .color(if selected { C_PRIMARY } else { FORGE_CREME })
                                    .strong(),
                            ),
                        );
                        if resp.clicked() {
                            *page = tab;
                        }
                        ui.add_space(3.0);
                    }
                });
        });
}

/// Panneau de section centré (verre + liseré or) — chrome commun aux sections
/// data du hub. Décalé à droite pour ne pas passer sous la sidebar.
fn hub_section_panel(
    ctx: &egui::Context,
    id: &'static str,
    title: &str,
    max_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Area::new(egui::Id::new(id))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(100.0, 0.0))
        .show(ctx, |ui| {
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(40, 30))
                .show(ui, |ui| {
                    ui.set_max_width(max_width);
                    ui.vertical_centered(|ui| {
                        ui.label(display_text(title, 40.0, FORGE_OR).strong());
                        ui.add_space(16.0);
                        add_contents(ui);
                    });
                });
        });
}

/// Accueil (page racine) : titre FORGIA + CONTINUER / NOUVELLE PARTIE / QUITTER
/// (+ démos moteur en dev). Renvoie l'action de lancement/quit à appliquer.
fn draw_root_landing(ctx: &egui::Context, has_save: bool) -> MenuAction {
    let mut action = MenuAction::None;
    egui::Area::new(egui::Id::new("menu_hub_root"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(100.0, 0.0))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading(display_text("FORGIA", 84.0, C_PRIMARY).strong());
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("ROGUELITE")
                        .size(22.0)
                        .color(FORGE_CREME)
                        .strong(),
                );
                ui.add_space(40.0);

                // CONTINUER — grisé tant qu'aucune partie n'a laissé de méta.
                let mut continue_clicked = false;
                ui.add_enabled_ui(has_save, |ui| {
                    continue_clicked = cartoon_btn(ui, "▶  CONTINUER", FORGE_OR).clicked();
                });
                if continue_clicked {
                    action = MenuAction::Launch(GameMode::Roguelite);
                }
                ui.add_space(16.0);

                // NOUVELLE PARTIE — CTA or quand pas de save, sinon verre (Continuer prime).
                let nouvelle = if has_save {
                    glass_btn(ui, "✦  NOUVELLE PARTIE")
                } else {
                    cartoon_btn(ui, "✦  NOUVELLE PARTIE", FORGE_OR)
                };
                if nouvelle.clicked() {
                    action = MenuAction::Launch(GameMode::Roguelite);
                }
                ui.add_space(16.0);

                // Hall de Forgia — hub social 3D walkable (château importé). Zone
                // neutre, point de rassemblement (multijoueur à terme). C'est une
                // feature du jeu, pas une démo dev → toujours visible.
                if glass_btn(ui, "🏰  Hall de Forgia").clicked() {
                    action = MenuAction::Launch(GameMode::CastleHub);
                }
                ui.add_space(8.0);

                if glass_btn(ui, "✕  QUITTER").clicked() {
                    action = MenuAction::Quit;
                }
                // Démos moteur RPG / Cyber City retirées du menu roguelite (2026-07-22,
                // demande user « roguelite pur »). Modes conservés dans le code
                // (GameMode::Rpg/CyberCity + plugins) → récupérables en re-câblant un
                // bouton ici. Hall de Forgia (CastleHub) gardé (feature du jeu).
            });
        });
    action
}

/// Section Stats — synthèse de la méta-progression (records + compteurs de runs).
fn draw_stats_section(ui: &mut egui::Ui, save: Option<&MetaShopSave>, level: u32) {
    let (runs, wins, best, souls, weapons) = save
        .map(|s| {
            (
                s.runs_played,
                s.victories,
                s.best_victory_secs,
                s.souls_total,
                s.unlocked_weapons.len(),
            )
        })
        .unwrap_or((0, 0, 0.0, 0, 1));
    let win_rate = if runs > 0 {
        (wins as f32 / runs as f32 * 100.0).round() as u32
    } else {
        0
    };
    let best_str = if best > 0.0 {
        let m = (best / 60.0).floor() as u32;
        let s = (best % 60.0).floor() as u32;
        format!("{m} min {s:02} s")
    } else {
        "—".to_string()
    };
    stat_row(ui, "Runs jouées", runs.to_string());
    stat_row(ui, "Victoires", format!("{wins}  ({win_rate} %)"));
    stat_row(ui, "Meilleure victoire", best_str);
    stat_row(ui, "Âmes accumulées", souls.to_string());
    stat_row(ui, "Armes débloquées", weapons.to_string());
    stat_row(ui, "Niveau", level.to_string());
}

/// Une ligne « libellé … valeur » de la section Stats (verre + liseré or).
fn stat_row(ui: &mut egui::Ui, label: &str, value: String) {
    egui::Frame::new()
        .fill(FORGE_PANEL)
        .inner_margin(egui::Margin::symmetric(16, 9))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
        .show(ui, |ui| {
            ui.set_min_width(460.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).size(16.0).color(FORGE_CREME));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(value)
                            .size(18.0)
                            .color(FORGE_OR)
                            .strong(),
                    );
                });
            });
        });
    ui.add_space(8.0);
}

/// Page Options — réutilise les contrôles du pause menu (DRY), dans un panneau
/// verre centré. « Retour » ramène à l'accueil.
fn draw_options_page(
    ctx: &egui::Context,
    page: &mut ResMut<MenuPage>,
    settings: &mut ResMut<UserSettings>,
) {
    egui::Area::new(egui::Id::new("menu_hub_options"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(100.0, 0.0))
        .show(ctx, |ui| {
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(40, 26))
                .show(ui, |ui| {
                    ui.set_max_width(520.0);
                    ui.vertical_centered(|ui| {
                        ui.label(display_text("OPTIONS", 44.0, FORGE_OR).strong());
                    });
                    ui.add_space(14.0);
                    // Anti-crash : bypass + set_changed() SEULEMENT si un contrôle
                    // change (sinon apply_window_settings boucle → resize → race wgpu).
                    let dirty = draw_settings_controls(ui, settings.bypass_change_detection());
                    if dirty {
                        settings.set_changed();
                    }
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if glass_btn(ui, "💾 Sauvegarder").clicked() {
                            save_user_settings(settings.bypass_change_detection());
                        }
                        ui.add_space(12.0);
                        if glass_btn(ui, "← Retour").clicked() {
                            **page = MenuPage::Root;
                        }
                    });
                });
        });
}

/// Section Enclume au menu-titre — méta-shop en **cartes cliquables** (souris).
/// Système séparé (garde `main_menu_ui` sous la limite de params) gaté
/// `AppMode::Menu` + onglet `Enclume`. Réutilise le rendu (`draw_enclume_panel`) et
/// la logique d'achat (`apply_meta_purchase`) de forgia-mode-roguelite — zéro
/// duplication. Solde = `MetaSouls.current` (chargé au Startup, comme au Lobby).
fn sys_menu_enclume(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    page: Res<MenuPage>,
    cat: Res<MetaShopCatalogue>,
    mut save: ResMut<MetaShopSave>,
    mut meta: ResMut<MetaSouls>,
) {
    if *app_state.get() != AppMode::Menu || *page != MenuPage::Enclume {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let souls = meta.current;
    let mut intent = None;
    egui::Area::new(egui::Id::new("menu_hub_enclume"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(100.0, 0.0))
        .show(ctx, |ui| {
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(40, 28))
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    ui.vertical_centered(|ui| {
                        ui.label(display_text("L'ENCLUME DES ÂMES", 38.0, FORGE_OR).strong());
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("Dépense tes Âmes en améliorations permanentes.")
                                .size(13.0)
                                .color(C_TEXT_MUTED),
                        );
                        ui.add_space(14.0);
                        intent = draw_enclume_panel(ui, &cat, &save, souls);
                    });
                });
        });
    // Applique l'achat cliqué (mute save/souls + sauve). Le `&save` du rendu est
    // relâché à la fermeture de l'Area, donc l'emprunt `&mut save` est libre ici.
    if let Some(purchase) = intent {
        apply_meta_purchase(&cat, &mut save, &mut meta, purchase);
    }
}

/// Section Forgeron au menu-titre — identité (nom + couleurs + bras) avec un
/// **avatar statique** (disque de la couleur équipée) à la place de l'aperçu 3D
/// du Lobby (pas de scène 3D au menu). Réutilise `draw_identity_content` de
/// forgia-mode-roguelite (zéro duplication). Système séparé (params + Local).
fn sys_menu_forgeron(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    page: Res<MenuPage>,
    cfg: Res<IdentityConfig>,
    mut save: ResMut<IdentitySave>,
    mut arm_cosmetics: ResMut<ArmCosmetics>,
    mut editing: Local<bool>,
    rtt: Option<Res<ArmPreviewRtt>>,
) {
    if *app_state.get() != AppMode::Menu || *page != MenuPage::Forgeron {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    // Aperçu 3D des bras (image RTT, l'arme tourne). Fallback = disque de la couleur
    // équipée si le RTT n'a pas encore spawné (lu dans les presets, données pub).
    let arm_image = rtt.as_ref().map(|r| r.tex_id);
    let rgb = cfg
        .colors
        .iter()
        .find(|c| c.id == save.equipped_color)
        .map(|c| c.rgb)
        .unwrap_or([0.6, 0.6, 0.6]);
    let disc_col = egui::Color32::from_rgb(
        (rgb[0] * 255.0) as u8,
        (rgb[1] * 255.0) as u8,
        (rgb[2] * 255.0) as u8,
    );
    egui::Area::new(egui::Id::new("menu_hub_forgeron"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(100.0, 0.0))
        .show(ctx, |ui| {
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(40, 28))
                .show(ui, |ui| {
                    ui.set_max_width(460.0);
                    ui.vertical_centered(|ui| {
                        ui.label(display_text("TON FORGERON", 38.0, FORGE_OR).strong());
                        ui.add_space(12.0);
                        // Aperçu 3D des bras (RTT) OU disque couleur en fallback.
                        match arm_image {
                            Some(tex) => {
                                ui.add(egui::Image::new(egui::load::SizedTexture::new(
                                    tex,
                                    egui::vec2(210.0, 210.0),
                                )));
                            }
                            None => {
                                let (r, _) = ui.allocate_exact_size(
                                    egui::vec2(96.0, 96.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(r.center(), 46.0, disc_col);
                                ui.painter().circle_stroke(
                                    r.center(),
                                    46.0,
                                    egui::Stroke::new(2.5, HAIR_GOLD_STRONG),
                                );
                            }
                        }
                        ui.add_space(12.0);
                        draw_identity_content(ui, &cfg, &mut save, &mut arm_cosmetics, &mut editing);
                    });
                });
        });
}

/// Section Armes au menu-titre — carte d'arme (stats / élément / matchup +
/// sélecteur ‹ › + déblocage) avec **aperçu 3D live** : l'image RTT de
/// `weapon_preview` (l'arme tourne). Réutilise `draw_weapon_menu_panel` de
/// forgia-mode-roguelite. Système séparé (params + `WeaponPreviewRtt`).
fn sys_menu_armes(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    page: Res<MenuPage>,
    mut choice: ResMut<StartingWeaponChoice>,
    cards: Res<WeaponCards>,
    elem_cfg: Res<ElementConfig>,
    mut save: ResMut<MetaShopSave>,
    cat: Res<MetaShopCatalogue>,
    mut meta: ResMut<MetaSouls>,
    rtt: Option<Res<WeaponPreviewRtt>>,
) {
    if *app_state.get() != AppMode::Menu || *page != MenuPage::Armes {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    // Image RTT de l'aperçu 3D (None si le plugin n'a pas encore spawné la caméra).
    let weapon_image = rtt.as_ref().map(|r| r.tex_id);
    egui::Area::new(egui::Id::new("menu_hub_armes"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(100.0, 0.0))
        .show(ctx, |ui| {
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(36, 24))
                .show(ui, |ui| {
                    ui.set_max_width(500.0);
                    ui.vertical_centered(|ui| {
                        ui.label(display_text("TES ARMES", 36.0, FORGE_OR).strong());
                        ui.add_space(10.0);
                        draw_weapon_menu_panel(
                            ui,
                            &mut choice,
                            &cards,
                            &elem_cfg,
                            &mut save,
                            &cat,
                            &mut meta,
                            weapon_image,
                            220.0,
                        );
                    });
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
            opts.grab_mode = CursorGrabMode::Locked;
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
